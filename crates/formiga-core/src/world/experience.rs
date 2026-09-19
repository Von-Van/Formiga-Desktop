use super::*;

pub(super) const OBSERVATION_INTERVAL_SECS: f32 = 60.0;

impl World {
    pub(super) fn sample_observations(&mut self, dt: f32, desktop: &DesktopSnapshot) {
        if !self.save.settings.visible || self.save.settings.paused {
            return;
        }
        self.observation_elapsed += dt.max(0.0);
        if self.observation_elapsed < OBSERVATION_INTERVAL_SECS {
            return;
        }
        self.observation_elapsed %= OBSERVATION_INTERVAL_SECS;
        let views = self.save.creatures.clone();
        for creature in &self.save.creatures {
            if creature.state.arrival_delay_secs > 0.0 {
                continue;
            }
            let Some((display, region)) = display_region(
                desktop,
                creature.state.surface.monitor_id,
                creature.state.position,
            ) else {
                continue;
            };
            let nearby_creature = views
                .iter()
                .filter(|other| other.id != creature.id && other.state.arrival_delay_secs <= 0.0)
                .map(|other| {
                    (
                        creature.state.position.distance(other.state.position),
                        other.id,
                    )
                })
                .filter(|(distance, _)| *distance <= 120.0)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id)| id);
            Self::emit(
                &mut self.events,
                WorldEvent::ObservationElapsed {
                    creature_id: creature.id,
                    display,
                    region,
                    on_ledge: creature.state.surface.kind == SurfaceKind::WindowLedge,
                    riding_window: creature.state.action == ActionKind::RideWindow,
                    nearby_creature,
                    active_seconds: OBSERVATION_INTERVAL_SECS as u8,
                },
            );
        }

        let mut calm_pairs = BTreeSet::new();
        for (index, first) in views.iter().enumerate() {
            for second in views.iter().skip(index + 1) {
                if first.state.arrival_delay_secs > 0.0
                    || second.state.arrival_delay_secs > 0.0
                    || first.state.surface.monitor_id != second.state.surface.monitor_id
                    || first.state.position.distance(second.state.position) > 120.0
                    || !calm_for_proximity(first.state.action)
                    || !calm_for_proximity(second.state.action)
                {
                    continue;
                }
                let pair = canonical_creature_pair(first.id, second.id).expect("distinct pair");
                calm_pairs.insert(pair);
                let elapsed = self.calm_proximity_seconds.entry(pair).or_default();
                *elapsed = elapsed.saturating_add(OBSERVATION_INTERVAL_SECS as u16);
                if *elapsed >= 5 * 60 {
                    *elapsed -= 5 * 60;
                    Self::emit(
                        &mut self.events,
                        WorldEvent::BondInteraction {
                            a: pair.0,
                            b: pair.1,
                            experience: RelationshipExperience::CalmProximity,
                        },
                    );
                }
            }
        }
        self.calm_proximity_seconds
            .retain(|pair, _| calm_pairs.contains(pair));
    }

    pub(super) fn project_events(&mut self, now: OffsetDateTime) {
        if self.projected_events >= self.events.len() {
            return;
        }
        let pending = self.events[self.projected_events..].to_vec();
        self.projected_events = self.events.len();
        let mut changed_profiles = Vec::new();

        for event in pending {
            // A first find of each trinket goes in the scrapbook, with the finder's name kept so
            // the record still reads if that creature later leaves.
            if let WorldEvent::ActionCompleted {
                creature_id,
                action: ActionKind::PresentDiscovery,
            } = event
                && let Some(finder) = self.save.creatures.iter().find(|c| c.id == creature_id)
            {
                let (variant, name) = (finder.state.activity_variant, finder.name.clone());
                self.save
                    .companion
                    .remember_discovery(variant, creature_id, name, now);
            }
            self.save.companion.record(&event, now);
            match event {
                WorldEvent::CreaturePetted { creature_id } => {
                    if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                        creature.memory.times_petted =
                            creature.memory.times_petted.saturating_add(1);
                        LearnedTendencies::adjust(&mut creature.tendencies.cursor_trust, 3);
                        LearnedTendencies::adjust(&mut creature.tendencies.sociability, 2);
                        changed_profiles.push(creature_id);
                    }
                }
                WorldEvent::OfferAnswered {
                    creature_id,
                    accepted,
                    ..
                } => {
                    // Being asked is the kind part, and a creature is allowed to say no. Taking
                    // what was offered is the warmer of the two, so it moves a little further.
                    if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                        let warmth = if accepted { 3 } else { 1 };
                        LearnedTendencies::adjust(&mut creature.tendencies.cursor_trust, warmth);
                        if accepted {
                            LearnedTendencies::adjust(&mut creature.tendencies.sociability, 2);
                        }
                        changed_profiles.push(creature_id);
                    }
                }
                WorldEvent::DragEnded {
                    creature_id,
                    outcome: DragReleaseKind::Tossed { .. },
                } => {
                    if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                        creature.memory.times_tossed =
                            creature.memory.times_tossed.saturating_add(1);
                        LearnedTendencies::adjust(&mut creature.tendencies.cursor_trust, -8);
                        changed_profiles.push(creature_id);
                    }
                }
                WorldEvent::CreaturePlaced {
                    creature_id,
                    display,
                    region,
                } => {
                    if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                        creature.memory.placements = creature.memory.placements.saturating_add(1);
                        match &mut creature.memory.preferred_region {
                            Some(preferred)
                                if preferred.display == display && preferred.cell == region =>
                            {
                                preferred.confidence = preferred.confidence.saturating_add(4);
                                LearnedTendencies::adjust(&mut creature.tendencies.routine, 2);
                            }
                            Some(preferred) if preferred.confidence > 2 => {
                                preferred.confidence = preferred.confidence.saturating_sub(2);
                            }
                            preferred => {
                                *preferred = Some(PreferredRegionMemory {
                                    display,
                                    cell: region.min(8),
                                    confidence: 4,
                                });
                            }
                        }
                        changed_profiles.push(creature_id);
                    }
                }
                WorldEvent::SleepInterrupted { creature_id, .. } => {
                    if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                        creature.memory.sleep_interruptions =
                            creature.memory.sleep_interruptions.saturating_add(1);
                        LearnedTendencies::adjust(&mut creature.tendencies.sleep_security, -6);
                        changed_profiles.push(creature_id);
                    }
                }
                WorldEvent::CreatureRested {
                    creature_id,
                    uninterrupted_seconds,
                } => {
                    if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                        creature.memory.longest_sleep_seconds = creature
                            .memory
                            .longest_sleep_seconds
                            .max(uninterrupted_seconds);
                        if uninterrupted_seconds >= 15 * 60 {
                            LearnedTendencies::adjust(&mut creature.tendencies.sleep_security, 2);
                            changed_profiles.push(creature_id);
                        }
                    }
                }
                WorldEvent::ActionCompleted {
                    creature_id,
                    action,
                } => {
                    if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                        match action {
                            ActionKind::ClimbWindow => {
                                creature.memory.window_climbs =
                                    creature.memory.window_climbs.saturating_add(1);
                                LearnedTendencies::adjust(&mut creature.tendencies.climbing, 2);
                                changed_profiles.push(creature_id);
                            }
                            ActionKind::PresentDiscovery => {
                                creature.memory.discoveries_found =
                                    creature.memory.discoveries_found.saturating_add(1);
                                LearnedTendencies::adjust(&mut creature.tendencies.exploration, 3);
                                changed_profiles.push(creature_id);
                            }
                            ActionKind::SoloPlay | ActionKind::SocialPlay => {
                                creature.memory.play_sessions =
                                    creature.memory.play_sessions.saturating_add(1);
                                LearnedTendencies::adjust(&mut creature.tendencies.play, 2);
                                changed_profiles.push(creature_id);
                            }
                            _ => {}
                        }
                    }
                }
                WorldEvent::ObservationElapsed {
                    creature_id,
                    display,
                    on_ledge,
                    riding_window,
                    active_seconds,
                    region,
                    ..
                } => {
                    if let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) {
                        if self
                            .surface_memory
                            .familiar(creature_id, creature.state.surface.window_key)
                        {
                            match &mut creature.memory.preferred_region {
                                Some(place) if place.display == display && place.cell == region => {
                                    place.confidence = place.confidence.saturating_add(1);
                                }
                                Some(place) if place.confidence > 0 => place.confidence -= 1,
                                place => {
                                    *place = Some(PreferredRegionMemory {
                                        display,
                                        cell: region.min(8),
                                        confidence: 1,
                                    })
                                }
                            }
                        } else if let Some(place) = &mut creature.memory.preferred_region {
                            place.confidence = place.confidence.saturating_sub(1);
                        }
                        creature.memory.milestone_cooldown_active_seconds = creature
                            .memory
                            .milestone_cooldown_active_seconds
                            .saturating_add(u32::from(active_seconds));
                        if on_ledge {
                            let previous = creature.memory.ledge_seconds / 300;
                            creature.memory.ledge_seconds = creature
                                .memory
                                .ledge_seconds
                                .saturating_add(u32::from(active_seconds));
                            let earned = creature.memory.ledge_seconds / 300 - previous;
                            for _ in 0..earned {
                                LearnedTendencies::adjust(&mut creature.tendencies.climbing, 1);
                            }
                            if earned > 0 {
                                changed_profiles.push(creature_id);
                            }
                        }
                        if riding_window {
                            creature.memory.window_ride_seconds = creature
                                .memory
                                .window_ride_seconds
                                .saturating_add(u32::from(active_seconds));
                        }
                        match &mut creature.memory.favorite_display {
                            Some(favorite) if favorite.display == display => {
                                favorite.confidence = favorite.confidence.saturating_add(1);
                            }
                            Some(favorite) if favorite.confidence > 0 => {
                                favorite.confidence = favorite.confidence.saturating_sub(1);
                            }
                            favorite => {
                                *favorite = Some(FavoriteDisplayMemory {
                                    display,
                                    confidence: 1,
                                });
                            }
                        }
                    }
                }
                WorldEvent::BondInteraction { a, b, experience } => {
                    if let Some(relationship) =
                        relationship_mut_or_insert(&mut self.save.relationships, a, b)
                    {
                        let before = relationship.affinity;
                        relationship.apply(experience);
                        if before < discovery::CLOSE_FRIENDSHIP_AFFINITY
                            && relationship.affinity >= discovery::CLOSE_FRIENDSHIP_AFFINITY
                        {
                            self.save.companion.remember(
                                Some(a.min(b)),
                                crate::JournalMoment::Friendship(a.max(b)),
                                now,
                            );
                        }
                    }
                }
                WorldEvent::HomeAppeared => {
                    for creature in self
                        .save
                        .creatures
                        .iter_mut()
                        .filter(|creature| creature.state.arrival_delay_secs <= 0.0)
                    {
                        creature.memory.home_visits = creature.memory.home_visits.saturating_add(1);
                        LearnedTendencies::adjust(&mut creature.tendencies.home_affinity, 2);
                        changed_profiles.push(creature.id);
                    }
                }
                _ => {}
            }
        }

        changed_profiles.sort_unstable();
        changed_profiles.dedup();
        let can_show_milestone = self.save.settings.visible && !self.save.settings.paused;
        for creature_id in changed_profiles {
            let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) else {
                continue;
            };
            let previous_flags = creature.memory.descriptor_flags;
            if !update_descriptor_flags(&mut creature.memory, creature.tendencies) {
                continue;
            }
            let new_descriptor = ProfileDescriptor::ALL.into_iter().find(|descriptor| {
                previous_flags & descriptor.flag() == 0
                    && creature.memory.descriptor_flags & descriptor.flag() != 0
            });
            let bubble_ready = !creature.memory.milestone_bubble_shown
                || creature.memory.milestone_cooldown_active_seconds >= 12 * 60 * 60;
            let show_milestone = can_show_milestone && new_descriptor.is_some() && bubble_ready;
            if show_milestone {
                creature.memory.milestone_bubble_shown = true;
                creature.memory.milestone_cooldown_active_seconds = 0;
            }
            self.save.companion.record(
                &WorldEvent::ProfileChanged {
                    creature_id,
                    new_descriptor,
                    show_milestone,
                },
                now,
            );
            Self::emit(
                &mut self.events,
                WorldEvent::ProfileChanged {
                    creature_id,
                    new_descriptor,
                    show_milestone,
                },
            );
        }
        self.projected_events = self.events.len();
    }
}
