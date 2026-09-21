use super::*;

pub(super) const RITUAL_APPROACH_SECS: f32 = 8.0;
const RITUAL_MIN_CREATURES: usize = 2;
/// How near its own slot a participant has to stop for the colony to count as gathered. The
/// lineup carries twice this on top of shoulder-to-shoulder so the slack cannot close a face.
const RITUAL_ARRIVED: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RitualPhase {
    Approach,
    Ceremony,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RitualParticipant {
    pub(super) creature_id: CreatureId,
    approach_target: Point,
    ceremony_target: Point,
    pub(super) ceremony_action: ActionKind,
}

#[derive(Clone, Debug)]
pub(super) struct ColonyPlan {
    pub(super) kind: RitualKind,
    monitor_id: MonitorId,
    usable_bounds: DesktopRect,
    pub(super) participants: Vec<RitualParticipant>,
    phase: RitualPhase,
    remaining_secs: f32,
}

impl ColonyPlan {
    pub(super) fn geometry_is_valid(
        &self,
        desktop: &DesktopSnapshot,
        policy: &HabitatPolicy,
    ) -> bool {
        let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == self.monitor_id)
        else {
            return false;
        };
        monitor.usable_bounds == self.usable_bounds
            && self.participants.iter().all(|participant| {
                monitor.bounds.contains(participant.approach_target)
                    && monitor.bounds.contains(participant.ceremony_target)
                    && habitat_contains(policy, monitor, participant.approach_target)
                    && habitat_contains(policy, monitor, participant.ceremony_target)
            })
    }
}

pub(crate) fn scheduled_ritual_at(
    colony_seed: [u8; 32],
    ordinal: u32,
    from: OffsetDateTime,
) -> OffsetDateTime {
    let streams = SeedStream::new(colony_seed);
    let mut rng = streams.rng("ritual-schedule", u64::from(ordinal));
    from + Duration::minutes(rng.random_range(12 * 60..=48 * 60))
}

fn interrupted_ritual_at(
    colony_seed: [u8; 32],
    ordinal: u32,
    from: OffsetDateTime,
) -> OffsetDateTime {
    let streams = SeedStream::new(colony_seed);
    let mut rng = streams.rng("ritual-interruption", u64::from(ordinal));
    from + Duration::minutes(rng.random_range(2 * 60..=6 * 60))
}

fn ritual_ceremony_duration(kind: RitualKind) -> f32 {
    match kind {
        RitualKind::GroupNap => 30.0,
        RitualKind::LateNightSleepPile => 45.0,
        RitualKind::QuietDayHuddle => 18.0,
        RitualKind::Picnic | RitualKind::ShelterGathering => 14.0,
        RitualKind::FloorRace
        | RitualKind::Catch
        | RitualKind::GroupPresentation
        | RitualKind::HatchDay => 10.0,
    }
}

impl World {
    pub(super) fn eligible_ritual_kinds(
        &self,
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
        shelter_available: bool,
    ) -> Vec<RitualKind> {
        let local_now = local_time_or_utc(now);
        let local_created = self.save.created_at_utc.to_offset(local_now.offset());
        let hatch_day_due = local_now.year() > local_created.year()
            && local_now.month() == local_created.month()
            && local_now.day() == local_created.day()
            && self.save.ritual.hatch_day_acknowledged_year != Some(local_now.year());
        if hatch_day_due {
            return vec![RitualKind::HatchDay];
        }

        let late_night = local_now.hour() >= 22 || local_now.hour() < 5;
        let current_windows: BTreeMap<_, _> = desktop
            .windows
            .iter()
            .map(|window| (window.key, window.bounds))
            .collect();
        let quiet_day = desktop.idle_duration >= std::time::Duration::from_secs(10 * 60)
            && current_windows == self.last_windows;
        RitualKind::ALL
            .into_iter()
            .filter(|kind| match kind {
                RitualKind::FloorRace => !self.save.settings.reduce_motion,
                RitualKind::ShelterGathering => shelter_available,
                RitualKind::HatchDay => false,
                RitualKind::QuietDayHuddle => quiet_day,
                RitualKind::LateNightSleepPile => late_night,
                _ => true,
            })
            .collect()
    }

    fn choose_ritual_kind(
        &self,
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
        shelter_available: bool,
    ) -> Option<RitualKind> {
        let mut eligible = self.eligible_ritual_kinds(now, desktop, shelter_available);
        if eligible.len() > 1
            && let Some(previous) = self.save.ritual.last_kind
        {
            eligible.retain(|kind| *kind != previous);
        }
        if eligible.is_empty() {
            return None;
        }
        let streams = SeedStream::new(self.save.colony_seed);
        let mut rng = streams.rng("ritual-kind", u64::from(self.save.ritual.ordinal));
        let index = rng.random_range(0..eligible.len());
        eligible.get(index).copied()
    }

    pub(super) fn try_start_colony_plan(
        &mut self,
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
    ) -> bool {
        if !self.save.settings.visible
            || self.save.settings.paused
            || self.save.home.is_active()
            || self.interaction.is_some()
            || !self.tosses.is_empty()
            || !self.window_journeys.is_empty()
        {
            return false;
        }
        let revealed_count = self
            .save
            .creatures
            .iter()
            .filter(|creature| creature.state.arrival_delay_secs <= 0.0)
            .count();
        let mut available: Vec<_> = self
            .save
            .creatures
            .iter()
            .filter(|creature| creature.state.arrival_delay_secs <= 0.0)
            .filter(|creature| {
                creature.state.surface.kind == SurfaceKind::ScreenFloor
                    && !matches!(
                        creature.state.action,
                        ActionKind::Dragged | ActionKind::Tossed | ActionKind::Homebound
                    )
            })
            .map(|creature| {
                (
                    creature.colony_order,
                    creature.id,
                    creature.state.surface.monitor_id,
                )
            })
            .collect();
        if available.len() < RITUAL_MIN_CREATURES || available.len() != revealed_count {
            return false;
        }
        available.sort_unstable();
        let monitor_id = available[0].2;
        if available.iter().any(|(_, _, id)| *id != monitor_id) {
            return false;
        }
        let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == monitor_id)
            .cloned()
        else {
            return false;
        };
        let mut regions = accessible_regions(&self.save.settings.habitat, &monitor);
        regions.sort_by(|a, b| {
            (b.width * b.height)
                .total_cmp(&(a.width * a.height))
                .then_with(|| a.x.total_cmp(&b.x))
        });
        let Some(region) = regions.first().copied() else {
            return false;
        };
        let shelter_anchor = (self.save.home.display == Some(monitor.display_key))
            .then(|| {
                resolved_home_anchor(
                    &self.save.home,
                    &monitor,
                    self.save.settings.display_scale,
                    &self.save.settings.habitat,
                )
            })
            .flatten();
        let kind = match self.choose_ritual_kind(now, desktop, shelter_anchor.is_some()) {
            Some(kind) => kind,
            None => return false,
        };
        if kind == RitualKind::Catch {
            available.truncate(2);
        }

        let count = available.len();
        let creature_width =
            spacing::frame_width(self.save.settings.display_scale, monitor.scale_factor);
        // Shoulder to shoulder, plus room for the tolerance the gather accepts at either end, so
        // that two neighbours who each stop `RITUAL_ARRIVED` short on opposite sides are still
        // face-clear when the ceremony freezes them where they stand.
        let spacing = creature_width * spacing::FACE_CLEAR_RATIO + RITUAL_ARRIVED * 2.0;
        let total_width = spacing * (count.saturating_sub(1)) as f32;
        let region_margin = (creature_width * 0.55).max(12.0);
        // A race lines up at one end and finishes at the other, so it needs both spans to fit.
        let needed_width = if kind == RitualKind::FloorRace {
            total_width * 2.0
        } else {
            total_width
        };
        if region.width < needed_width + region_margin * 2.0 {
            return false;
        }
        let average_x = available
            .iter()
            .filter_map(|(_, creature_id, _)| {
                self.save
                    .creatures
                    .iter()
                    .find(|creature| creature.id == *creature_id)
                    .map(|creature| creature.state.position.x)
            })
            .sum::<f32>()
            / count as f32;
        let default_center = average_x.clamp(
            region.x + region_margin + total_width * 0.5,
            region.right() - region_margin - total_width * 0.5,
        );
        let center = if kind == RitualKind::ShelterGathering {
            shelter_anchor
                .expect("shelter ritual has a resolved anchor")
                .x
                .clamp(
                    region.x + region_margin + total_width * 0.5,
                    region.right() - region_margin - total_width * 0.5,
                )
        } else {
            default_center
        };
        let floor_y = region.bottom() - 4.0;
        let start_x = center - total_width * 0.5;
        let race_start = region.x + region_margin;
        let race_finish = region.right() - region_margin;
        let mut participants = Vec::with_capacity(count);
        for (index, (_, creature_id, _)) in available.iter().enumerate() {
            let lineup = Point {
                x: start_x + index as f32 * spacing,
                y: floor_y,
            };
            let (approach_target, ceremony_target, ceremony_action) = match kind {
                RitualKind::Picnic => (
                    lineup,
                    lineup,
                    if index % 2 == 0 {
                        ActionKind::Eat
                    } else {
                        ActionKind::Drink
                    },
                ),
                RitualKind::GroupNap | RitualKind::LateNightSleepPile => {
                    (lineup, lineup, ActionKind::Sleep)
                }
                // Runners wait on a starting line and then stand about at the finish, and both are
                // places they hold, so both are spaced like any other lineup.
                RitualKind::FloorRace => (
                    Point {
                        x: race_start + index as f32 * spacing,
                        y: floor_y,
                    },
                    Point {
                        x: race_finish - index as f32 * spacing,
                        y: floor_y,
                    },
                    ActionKind::Sprint,
                ),
                RitualKind::ShelterGathering => (lineup, lineup, ActionKind::Homebound),
                RitualKind::Catch => (lineup, lineup, ActionKind::SocialPlay),
                RitualKind::GroupPresentation => (
                    lineup,
                    Point {
                        x: center,
                        y: floor_y,
                    },
                    if index == 0 {
                        ActionKind::PresentDiscovery
                    } else {
                        ActionKind::InspectScreen
                    },
                ),
                RitualKind::HatchDay => (lineup, lineup, ActionKind::Greet),
                RitualKind::QuietDayHuddle => (lineup, lineup, ActionKind::Idle),
            };
            participants.push(RitualParticipant {
                creature_id: *creature_id,
                approach_target,
                ceremony_target,
                ceremony_action,
            });
        }

        self.action_choices.clear();
        self.bond_plans.clear();
        self.pending_home_greetings.clear();
        for participant in &participants {
            let Some(creature) = creature_mut(&mut self.save.creatures, participant.creature_id)
            else {
                return false;
            };
            let old = creature.state.action;
            if old == ActionKind::Sleep {
                let elapsed = self
                    .sleep_elapsed
                    .remove(&creature.id)
                    .unwrap_or(creature.state.action_elapsed)
                    .max(0.0) as u32;
                Self::emit(
                    &mut self.events,
                    WorldEvent::CreatureRested {
                        creature_id: creature.id,
                        uninterrupted_seconds: elapsed,
                    },
                );
                Self::emit(
                    &mut self.events,
                    WorldEvent::CreatureWoke {
                        creature_id: creature.id,
                    },
                );
            }
            Self::emit(
                &mut self.events,
                WorldEvent::ActionCompleted {
                    creature_id: creature.id,
                    action: old,
                },
            );
            creature.state.action = ActionKind::Traverse;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = f32::MAX;
            creature.state.velocity = Point::default();
            creature.state.surface.window_key = None;
            creature.state.surface.kind = SurfaceKind::ScreenFloor;
            creature.state.surface.monitor_id = monitor_id;
            self.action_choices.insert(
                creature.id,
                ActionChoice {
                    action: ActionKind::Traverse,
                    target_creature: None,
                    target_point: Some(participant.approach_target),
                },
            );
            Self::emit(
                &mut self.events,
                WorldEvent::ActionStarted {
                    creature_id: creature.id,
                    action: ActionKind::Traverse,
                },
            );
        }
        if kind == RitualKind::ShelterGathering {
            self.save.home.active_since_utc = Some(now);
            Self::emit(&mut self.events, WorldEvent::HomeAppeared);
        }
        let local_now = local_time_or_utc(now);
        if kind == RitualKind::HatchDay {
            self.save.ritual.hatch_day_acknowledged_year = Some(local_now.year());
        }
        self.save.ritual.last_kind = Some(kind);
        self.save.ritual.ordinal = self.save.ritual.ordinal.saturating_add(1);
        self.save.ritual.next_at_utc =
            scheduled_ritual_at(self.save.colony_seed, self.save.ritual.ordinal, now);
        self.colony_plan = Some(ColonyPlan {
            kind,
            monitor_id,
            usable_bounds: monitor.usable_bounds,
            participants,
            phase: RitualPhase::Approach,
            remaining_secs: RITUAL_APPROACH_SECS,
        });
        Self::emit(&mut self.events, WorldEvent::RitualStarted { kind });
        true
    }

    pub(super) fn advance_colony_plan(&mut self, now: OffsetDateTime, dt: f32) {
        let Some(plan) = &mut self.colony_plan else {
            return;
        };
        plan.remaining_secs = (plan.remaining_secs - dt.max(0.0)).max(0.0);
        let gathered = plan.phase == RitualPhase::Approach
            && plan.participants.iter().all(|participant| {
                self.save
                    .creatures
                    .iter()
                    .find(|creature| creature.id == participant.creature_id)
                    .is_some_and(|creature| {
                        (creature.state.position.x - participant.approach_target.x).abs()
                            <= RITUAL_ARRIVED
                    })
            });
        if plan.phase == RitualPhase::Approach && (gathered || plan.remaining_secs <= 0.0) {
            let kind = plan.kind;
            let participants = plan.participants.clone();
            plan.phase = RitualPhase::Ceremony;
            plan.remaining_secs = ritual_ceremony_duration(kind);
            for participant in participants {
                let Some(creature) =
                    creature_mut(&mut self.save.creatures, participant.creature_id)
                else {
                    self.interrupt_colony_plan(now);
                    return;
                };
                let old = creature.state.action;
                Self::emit(
                    &mut self.events,
                    WorldEvent::ActionCompleted {
                        creature_id: creature.id,
                        action: old,
                    },
                );
                creature.state.action = participant.ceremony_action;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = f32::MAX;
                creature.state.velocity = Point::default();
                if participant.ceremony_action == ActionKind::PresentDiscovery {
                    creature.state.activity_variant =
                        (self.save.ritual.ordinal as u8).wrapping_sub(1) % 8;
                }
                self.action_choices.insert(
                    creature.id,
                    ActionChoice {
                        action: participant.ceremony_action,
                        target_creature: None,
                        target_point: Some(participant.ceremony_target),
                    },
                );
                Self::emit(
                    &mut self.events,
                    WorldEvent::ActionStarted {
                        creature_id: creature.id,
                        action: participant.ceremony_action,
                    },
                );
                if participant.ceremony_action == ActionKind::Sleep {
                    Self::emit(
                        &mut self.events,
                        WorldEvent::CreatureSlept {
                            creature_id: creature.id,
                        },
                    );
                }
            }
            return;
        }
        if plan.phase != RitualPhase::Ceremony || plan.remaining_secs > 0.0 {
            return;
        }

        let plan = self.colony_plan.take().expect("active ritual exists");
        let ids: Vec<_> = plan
            .participants
            .iter()
            .map(|participant| participant.creature_id)
            .collect();
        for participant in &plan.participants {
            self.action_choices.remove(&participant.creature_id);
            if let Some(creature) = creature_mut(&mut self.save.creatures, participant.creature_id)
            {
                let old = creature.state.action;
                let old_elapsed = creature.state.action_elapsed;
                Self::emit(
                    &mut self.events,
                    WorldEvent::ActionCompleted {
                        creature_id: creature.id,
                        action: old,
                    },
                );
                if old == ActionKind::Sleep {
                    let uninterrupted_seconds = self
                        .sleep_elapsed
                        .remove(&creature.id)
                        .unwrap_or(old_elapsed)
                        .max(0.0) as u32;
                    Self::emit(
                        &mut self.events,
                        WorldEvent::CreatureRested {
                            creature_id: creature.id,
                            uninterrupted_seconds,
                        },
                    );
                    Self::emit(
                        &mut self.events,
                        WorldEvent::CreatureWoke {
                            creature_id: creature.id,
                        },
                    );
                }
                creature.state.action = ActionKind::Idle;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = 2.5;
                creature.state.velocity = Point::default();
                creature.state.activity_variant = 0;
                Self::emit(
                    &mut self.events,
                    WorldEvent::ActionStarted {
                        creature_id: creature.id,
                        action: ActionKind::Idle,
                    },
                );
            }
        }
        let experience = match plan.kind {
            RitualKind::GroupNap | RitualKind::QuietDayHuddle | RitualKind::LateNightSleepPile => {
                RelationshipExperience::SharedRest
            }
            RitualKind::FloorRace | RitualKind::Catch | RitualKind::HatchDay => {
                RelationshipExperience::PositivePlay
            }
            RitualKind::Picnic | RitualKind::ShelterGathering | RitualKind::GroupPresentation => {
                RelationshipExperience::Greeting
            }
        };
        for (index, first) in ids.iter().copied().enumerate() {
            for second in ids.iter().copied().skip(index + 1) {
                Self::emit(
                    &mut self.events,
                    WorldEvent::BondInteraction {
                        a: first,
                        b: second,
                        experience,
                    },
                );
            }
        }
        if plan.kind == RitualKind::ShelterGathering {
            self.dismiss_home(now, false);
        }
        Self::emit(
            &mut self.events,
            WorldEvent::RitualCompleted { kind: plan.kind },
        );
    }

    pub(super) fn interrupt_colony_plan(&mut self, now: OffsetDateTime) {
        let Some(plan) = self.colony_plan.take() else {
            return;
        };
        for participant in &plan.participants {
            self.action_choices.remove(&participant.creature_id);
            self.bond_plans.remove(&participant.creature_id);
            if let Some(creature) = creature_mut(&mut self.save.creatures, participant.creature_id)
                && !matches!(
                    creature.state.action,
                    ActionKind::Dragged | ActionKind::Tossed
                )
            {
                creature.state.action = ActionKind::Idle;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = 2.5;
                creature.state.velocity = Point::default();
                creature.state.activity_variant = 0;
            }
        }
        if plan.kind == RitualKind::ShelterGathering && self.save.home.is_active() {
            self.dismiss_home(now, true);
        }
        self.save.ritual.next_at_utc =
            interrupted_ritual_at(self.save.colony_seed, self.save.ritual.ordinal, now);
        Self::emit(
            &mut self.events,
            WorldEvent::RitualInterrupted { kind: plan.kind },
        );
    }
}
