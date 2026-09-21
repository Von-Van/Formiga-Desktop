use super::*;
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
    fn welcome_arrival(&mut self, mut creature: Creature, queued: &mut u8) {
        creature.state.arrival_delay_secs = f32::from(*queued) * ARRIVAL_STAGGER_SECS;
        let id = creature.id;
        let parent_id = creature.role.parent_id();
        let is_adult = creature.role.is_adult();
        self.register_creature_runtime(&creature);
        // Anyone still waiting their turn is announced when the wait runs out, not now.
        if creature.state.arrival_delay_secs == 0.0 {
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
                self.welcome_arrival(creature, queued);
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
            self.welcome_arrival(mini, queued);
        }
    }
}
