use super::*;
use crate::tuning::ENTRANCES;
use time::{Date, Month, PrimitiveDateTime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArrivalMilestone {
    Hours(i64),
    Days(i64),
    CalendarMonths(u8),
}

const ARRIVAL_MILESTONES: [ArrivalMilestone; 3] = [
    ArrivalMilestone::Hours(1),
    ArrivalMilestone::Days(7),
    ArrivalMilestone::CalendarMonths(1),
];

fn arrival_due_at(created_at_utc: OffsetDateTime, milestone: ArrivalMilestone) -> OffsetDateTime {
    match milestone {
        ArrivalMilestone::Hours(hours) => created_at_utc + Duration::hours(hours),
        ArrivalMilestone::Days(days) => created_at_utc + Duration::days(days),
        ArrivalMilestone::CalendarMonths(months) => add_calendar_months_utc(created_at_utc, months),
    }
}

pub(super) fn add_calendar_months_utc(value: OffsetDateTime, months: u8) -> OffsetDateTime {
    let value = value.to_offset(UtcOffset::UTC);
    let month_index = i64::from(value.year()) * 12 + i64::from(u8::from(value.month()) - 1);
    let destination = month_index + i64::from(months);
    let year = i32::try_from(destination.div_euclid(12)).expect("calendar year remains in range");
    let month = Month::try_from((destination.rem_euclid(12) + 1) as u8)
        .expect("calendar month is always in range");
    let day = value.day().min(month.length(year));
    let date = Date::from_calendar_date(year, month, day).expect("clamped calendar date is valid");
    PrimitiveDateTime::new(date, value.time()).assume_utc()
}

/// Two creatures that fall due together do not appear together: each waits this much longer
/// than the one before it, so the desk gets one arrival at a time to notice.
const ARRIVAL_STAGGER_SECS: f32 = 15.0;

impl World {
    /// Everyone owed an arrival right now, calendar milestones first and then the minis the
    /// colony's own adults are due. One queue runs through both, so an adult's mini turning up
    /// in the same tick as a calendar arrival waits its turn instead of landing on top of it.
    pub(super) fn process_arrivals(&mut self, now: OffsetDateTime, desktop: &DesktopSnapshot) {
        let mut queued = 0_u8;
        self.process_calendar_arrivals(now, desktop, &mut queued);
        self.process_adult_mini_arrivals(now, desktop, &mut queued);
    }

    /// Take one newly generated creature into the colony: give it its place in this tick's queue
    /// of arrivals, start its runtime, introduce it to everyone already here, and add it.
    fn welcome_arrival(
        &mut self,
        mut creature: Creature,
        queued: &mut u8,
        desktop: &DesktopSnapshot,
    ) {
        creature.state.arrival_delay_secs = f32::from(*queued) * ARRIVAL_STAGGER_SECS;
        let id = creature.id;
        let parent_id = creature.role.parent_id();
        let is_adult = creature.role.is_adult();
        self.register_creature_runtime(&creature);
        // Anyone still waiting their turn is announced when the wait runs out, not now.
        let now_here = creature.state.arrival_delay_secs == 0.0;
        if now_here {
            Self::emit(
                &mut self.events,
                WorldEvent::CreatureSpawned { creature_id: id },
            );
        }
        add_arrival_relationships(
            &mut self.save.relationships,
            &self.save.creatures,
            id,
            parent_id,
        );
        self.save.creatures.push(creature);
        // Only a new adult changes which adult each of the colony's minis belongs to.
        if is_adult {
            rebalance_minis(&mut self.save.creatures);
        }
        if now_here {
            self.begin_entrance(id, desktop);
        }
        *queued = queued.saturating_add(1);
    }

    fn process_calendar_arrivals(
        &mut self,
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
        queued: &mut u8,
    ) {
        for (index, milestone) in ARRIVAL_MILESTONES.into_iter().enumerate() {
            let due = arrival_due_at(self.save.created_at_utc, milestone);
            if !self.save.arrival_state.arrived[index] && self.save.maximum_seen_utc >= due {
                if self.save.creatures.len() >= MAX_COLONY_CREATURES {
                    continue;
                }
                let creature = if index < 2 {
                    let Some(parent_id) = balanced_parent_id(&self.save.creatures) else {
                        continue;
                    };
                    generate_mini_for_parent(
                        &self.save.creatures,
                        self.save.colony_seed,
                        parent_id,
                        index as u8 + 1,
                        now,
                        desktop,
                    )
                } else if adult_count(&self.save.creatures) < MAX_ADULT_CREATURES {
                    let streams = SeedStream::new(self.save.colony_seed);
                    let seed: [u8; 32] = streams.bytes("calendar-adult", 0);
                    let existing_names: Vec<_> = self
                        .save
                        .creatures
                        .iter()
                        .map(|creature| creature.name.clone())
                        .collect();
                    Some(generated_adult(
                        seed,
                        now,
                        desktop,
                        next_colony_order(&self.save.creatures),
                        &existing_names,
                        true,
                        self.generator,
                    ))
                } else {
                    let Some(parent_id) = balanced_parent_id(&self.save.creatures) else {
                        continue;
                    };
                    let child_number =
                        mini_count_for_parent(&self.save.creatures, parent_id) as u8 + 1;
                    generate_mini_for_parent(
                        &self.save.creatures,
                        self.save.colony_seed,
                        parent_id,
                        child_number,
                        now,
                        desktop,
                    )
                };
                let Some(creature) = creature else {
                    continue;
                };
                if self
                    .save
                    .creatures
                    .iter()
                    .any(|existing| existing.id == creature.id)
                {
                    self.save.arrival_state.arrived[index] = true;
                    continue;
                }
                self.welcome_arrival(creature, queued, desktop);
                self.save.arrival_state.arrived[index] = true;
            }
        }
    }

    fn process_adult_mini_arrivals(
        &mut self,
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
        queued: &mut u8,
    ) {
        if self.save.creatures.len() >= MAX_COLONY_CREATURES {
            return;
        }
        loop {
            if self.save.creatures.len() >= MAX_COLONY_CREATURES {
                break;
            }
            let mut candidates: Vec<_> = self
                .save
                .creatures
                .iter()
                .filter(|creature| {
                    creature.role.is_adult()
                        && creature.mini_arrivals.enabled
                        && mini_count_for_parent(&self.save.creatures, creature.id)
                            < MAX_MINIS_PER_ADULT
                })
                .filter_map(|creature| {
                    creature
                        .mini_arrivals
                        .arrived
                        .iter()
                        .enumerate()
                        .find(|(index, arrived)| {
                            !**arrived
                                && now
                                    >= creature.born_at_utc
                                        + if *index == 0 {
                                            Duration::hours(1)
                                        } else {
                                            Duration::days(7)
                                        }
                        })
                        .map(|(index, _)| {
                            (
                                mini_count_for_parent(&self.save.creatures, creature.id),
                                creature.born_at_utc,
                                creature.colony_order,
                                creature.id,
                                index,
                            )
                        })
                })
                .collect();
            candidates.sort_by_key(|candidate| *candidate);
            let Some((_, _, _, parent_id, arrival_index)) = candidates.first().copied() else {
                break;
            };
            let child_number = mini_count_for_parent(&self.save.creatures, parent_id) as u8 + 1;
            let Some(mini) = generate_mini_for_parent(
                &self.save.creatures,
                self.save.colony_seed,
                parent_id,
                child_number,
                now,
                desktop,
            ) else {
                break;
            };
            if let Some(parent) = self
                .save
                .creatures
                .iter_mut()
                .find(|creature| creature.id == parent_id)
            {
                parent.mini_arrivals.arrived[arrival_index] = true;
            }
            if self
                .save
                .creatures
                .iter()
                .any(|creature| creature.id == mini.id)
            {
                continue;
            }
            self.welcome_arrival(mini, queued, desktop);
        }
    }
}

impl World {
    /// A companion that has just turned up walks in rather than appearing where it is: out of its
    /// own house door when the village is out, and otherwise in along the floor from beyond the
    /// nearer edge of the display — the edge of the ground it may use, where that ground reaches
    /// the edge — to where it was going to appear, or as far toward it as a few seconds' walk
    /// takes it. Paused, hidden or with motion reduced it is simply there, as it always was, and
    /// so is one with no edge of open ground to come in by.
    ///
    /// The walk is a journey (`EntranceJourney`), so nothing else takes its feet on the way, and
    /// like every journey it lives only while the app runs: a colony saved part way through opens
    /// with the newcomer standing wherever it had got to.
    pub(super) fn begin_entrance(&mut self, id: CreatureId, desktop: &DesktopSnapshot) {
        let settings = &self.save.settings;
        if settings.paused || !settings.visible || settings.reduce_motion {
            return;
        }
        let Some(index) = self.save.creatures.iter().position(|c| c.id == id) else {
            return;
        };
        if self.out_of_its_door(index, desktop) {
            return;
        }
        self.in_from_the_edge(index, desktop);
    }

    /// Out of its own house door, when the village is out and the house has a door to come out
    /// of. Home for a visit, the village's own walk takes it from the doorstep to its place;
    /// otherwise it walks on out a couple of steps along the ground, coming down the path to the
    /// front of the grass as it goes, and the colony's life takes it from there. Returns whether
    /// it came out of a door.
    fn out_of_its_door(&mut self, index: usize, desktop: &DesktopSnapshot) -> bool {
        if !self.save.home.shown() {
            return false;
        }
        let creatures = &self.save.creatures;
        let slot = house_slot_for(&creatures[index], creatures, &self.save.home.cottage_order);
        let cottages = colony_cottage_list(creatures);
        let policy = &self.save.settings.habitat;
        let display_scale = self.save.settings.display_scale;
        let Some((monitor_id, doorstep)) = home_dwelling_position(
            &self.save.home,
            slot,
            cottages.as_slice(),
            &desktop.monitors,
            policy,
            display_scale,
        ) else {
            return false;
        };
        let Some(door) = self.village_paths(desktop).door(slot) else {
            return false;
        };
        let Some(monitor) = desktop.monitors.iter().find(|m| m.id == monitor_id) else {
            return false;
        };
        let Some(region) = accessible_regions(policy, monitor)
            .into_iter()
            .find(|region| region.contains(doorstep))
        else {
            return false;
        };
        let home_active = self.save.home.is_active();
        let inward = match self.save.home.corner {
            HomeCorner::BottomLeft => 1.0,
            HomeCorner::BottomRight => -1.0,
        };
        let frame = spacing::frame_width(display_scale, monitor.scale_factor);
        let (low, high) = standing_span(
            region.x,
            region.right(),
            body_half_width(monitor, display_scale),
        );
        let creature = &mut self.save.creatures[index];
        let at = Point {
            x: door.x,
            y: doorstep.y,
        };
        creature.state.position = at;
        creature.state.velocity = Point::default();
        creature.state.depth = door.depth;
        creature.state.indoors = false;
        creature.state.surface = SurfaceAttachment {
            kind: SurfaceKind::ScreenFloor,
            monitor_id,
            window_key: None,
            relative_x: ((at.x - region.x) / region.width.max(1.0)).clamp(0.0, 1.0),
        };
        if home_active {
            return true;
        }
        let target = Point {
            x: (at.x + inward * frame * ENTRANCES.door_walk_frames).clamp(low, high),
            y: at.y,
        };
        self.walk_in(index, at, target);
        true
    }

    /// In along the floor from beyond the nearer edge of the display the newcomer is on, where
    /// the ground it may use reaches that edge. Returns whether it is walking in.
    fn in_from_the_edge(&mut self, index: usize, desktop: &DesktopSnapshot) -> bool {
        let creature = &self.save.creatures[index];
        if creature.state.surface.kind != SurfaceKind::ScreenFloor {
            return false;
        }
        let spot = creature.state.position;
        let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|m| m.id == creature.state.surface.monitor_id)
        else {
            return false;
        };
        let policy = &self.save.settings.habitat;
        let Some(region) = accessible_regions(policy, monitor)
            .into_iter()
            .find(|region| region.contains(spot))
        else {
            return false;
        };
        let half = body_half_width(monitor, self.save.settings.display_scale);
        let (low, high) = standing_span(region.x, region.right(), half);
        let bounds = monitor.usable_bounds;
        // Which sides of its ground run to the display's edge, nearest first.
        let mut sides = [
            (
                spot.x - region.x,
                (region.x - bounds.x).abs() <= 1.0,
                region.x - half,
                1.0,
            ),
            (
                region.right() - spot.x,
                (bounds.right() - region.right()).abs() <= 1.0,
                region.right() + half,
                -1.0,
            ),
        ];
        sides.sort_by(|a, b| a.0.total_cmp(&b.0));
        let Some(&(_, _, edge, inward)) = sides.iter().find(|side| side.1) else {
            return false;
        };
        let reach = walking_pace(creature) * ENTRANCES.edge_walk_secs;
        let target_x = if inward > 0.0 {
            (edge + reach).min(spot.x).max(low)
        } else {
            (edge - reach).max(spot.x).min(high)
        };
        let start = Point { x: edge, y: spot.y };
        let target = Point {
            x: target_x.clamp(low, high),
            y: spot.y,
        };
        self.walk_in(index, start, target);
        true
    }

    /// Set the newcomer walking in from `start` to `target` along the ground it stands on.
    fn walk_in(&mut self, index: usize, start: Point, target: Point) {
        let creature = &mut self.save.creatures[index];
        let duration = start.distance(target) / walking_pace(creature);
        if duration <= 0.0 {
            return;
        }
        creature.state.position = start;
        creature.state.velocity = Point::default();
        creature.state.facing_right = target.x >= start.x;
        creature.state.action = ActionKind::Traverse;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = f32::MAX;
        let id = creature.id;
        let surface = creature.state.surface.clone();
        self.window_journeys.insert(
            id,
            WindowJourney::Entrance(EntranceJourney {
                start,
                target,
                surface,
                elapsed: 0.0,
                duration,
            }),
        );
    }
}

/// How fast a companion walks, in points a second: the pace of every walk it takes.
fn walking_pace(creature: &Creature) -> f32 {
    24.0 + creature.personality.activity * 34.0
}
