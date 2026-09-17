//! Borderline gaps: look, back up, come forward, and reconsider at most twice, then either jump
//! from the edge (a near miss that catches) or retreat. Every phase boundary revalidates.
use super::*;

const LOOK_SECONDS: f32 = 0.9;
const RECONSIDER_SECONDS: f32 = 1.0;
const RECOVERY_SECONDS: f32 = 1.2;
/// Hard bound on the whole scene, including two attempts and the retreat.
const MAX_HESITATION_SECONDS: f32 = 16.0;
const MAX_ATTEMPTS: u8 = 2;

impl World {
    pub(super) fn begin_hesitation(
        &mut self,
        id: CreatureId,
        journey: GapJourney,
        drop: f32,
        desktop: &DesktopSnapshot,
    ) {
        let c = self.save.creatures.iter().find(|c| c.id == id).unwrap();
        let Some(target_window) = journey.hop.surface.window_key else {
            return;
        };
        // Braver and more playful creatures are likelier to try again and to go in the end.
        let nerve = c.personality.boldness * 0.5 + c.personality.playfulness * 0.25;
        let retry = self.ambient_rng.random_range(0.0..1.0) < 0.35 + nerve * 0.5;
        let commit = self.ambient_rng.random_range(0.0..1.0) < 0.2 + nerve * 0.6;
        self.surface_memory.next_origin = self.surface_memory.next_origin.wrapping_add(1);
        let origin = Origin::Surface(self.surface_memory.next_origin);
        let reaction = Reaction {
            origin,
            role: Role::Hesitate {
                source: journey.source,
                source_bounds: journey.source_bounds,
                target_window,
                target_bounds: journey.target_bounds,
                edge: journey.hop.start,
                runup: journey.runup,
                landing: journey.hop.target,
                drop,
                attempt: 1,
                phase: HesitatePhase::Look,
                phase_elapsed: 0.0,
                retry,
                commit,
            },
            target: journey.hop.target,
            emotion: AttentionEmotion::Curious,
            elapsed: 0.0,
            seconds: MAX_HESITATION_SECONDS,
            delay: 0.0,
            travel_elapsed: 0.0,
            walk: None,
            display_walk: None,
            surface: (c.state.surface.monitor_id, Some(journey.source)),
            action: ActionKind::InspectScreen,
            cue: None,
            ride: None,
        };
        self.begin_attention(id, reaction);
        self.recruit_attention_observers(id, origin, desktop);
        self.extend_audience(id, MAX_HESITATION_SECONDS);
        self.attention.colony_cooldown = 15.0;
    }

    /// Keep the actor's spectators for `remaining` more seconds of their own timeline.
    pub(super) fn extend_audience(&mut self, actor: CreatureId, remaining: f32) {
        for p in self.attention.plans.values_mut() {
            if matches!(p.role, Role::Observer { actor: a } | Role::Helper { actor: a, .. } if a == actor)
            {
                p.seconds = (p.elapsed - p.delay - p.travel_elapsed).max(0.0) + remaining + 0.5;
            }
        }
    }

    pub(super) fn update_hesitation(&mut self, desktop: &DesktopSnapshot, dt: f32) {
        let ids: Vec<_> = self
            .attention
            .plans
            .iter()
            .filter_map(|(&id, p)| matches!(p.role, Role::Hesitate { .. }).then_some(id))
            .collect();
        for id in ids {
            let Some(plan) = self.attention.plans.get(&id).copied() else {
                continue;
            };
            let Role::Hesitate {
                source,
                source_bounds,
                target_window,
                edge,
                runup,
                attempt,
                phase,
                phase_elapsed,
                retry,
                commit,
                ..
            } = plan.role
            else {
                continue;
            };
            let elapsed = phase_elapsed + dt;
            let arrived = plan.walk.is_none();
            let next = match phase {
                HesitatePhase::Look if elapsed >= LOOK_SECONDS => Some(HesitatePhase::BackUp),
                HesitatePhase::BackUp if arrived && elapsed >= 0.3 => Some(HesitatePhase::Approach),
                HesitatePhase::Approach if arrived && elapsed >= 0.3 => {
                    Some(HesitatePhase::Reconsider)
                }
                HesitatePhase::Reconsider if elapsed >= RECONSIDER_SECONDS => {
                    if retry && attempt < MAX_ATTEMPTS {
                        Some(HesitatePhase::BackUp)
                    } else if commit && self.commit_hesitant_gap(id, target_window, desktop) {
                        continue;
                    } else {
                        // Chickening out is remembered briefly so the same gap is not retried at once.
                        self.attention
                            .record_setback(id, Some((source, target_window)));
                        Some(HesitatePhase::Retreat)
                    }
                }
                HesitatePhase::Retreat if arrived && elapsed >= 0.3 => {
                    let plan = self.attention.plans.get_mut(&id).unwrap();
                    // Finish after a short relieved recovery; spectators end with it.
                    plan.seconds =
                        (plan.elapsed - plan.delay - plan.travel_elapsed) + RECOVERY_SECONDS;
                    if let Role::Hesitate { phase_elapsed, .. } = &mut plan.role {
                        *phase_elapsed = f32::NEG_INFINITY;
                    }
                    self.extend_audience(id, RECOVERY_SECONDS);
                    None
                }
                _ => None,
            };
            let walk_to = |world: &World, x: f32| {
                let c = world.save.creatures.iter().find(|c| c.id == id)?;
                motion::safe_goal(world, c, x, desktop)
                    .map(|destination| ShortWalk::watching(destination, source_bounds))
            };
            let walk = match next {
                Some(HesitatePhase::BackUp | HesitatePhase::Retreat) => walk_to(self, runup.x),
                Some(HesitatePhase::Approach) => walk_to(self, edge.x),
                _ => None,
            };
            let plan = self.attention.plans.get_mut(&id).unwrap();
            let Role::Hesitate {
                phase,
                phase_elapsed,
                attempt,
                ..
            } = &mut plan.role
            else {
                continue;
            };
            match next {
                Some(next) => {
                    if next == HesitatePhase::BackUp && *phase == HesitatePhase::Reconsider {
                        *attempt += 1;
                    }
                    *phase = next;
                    *phase_elapsed = 0.0;
                    plan.walk = walk;
                }
                None => *phase_elapsed += dt,
            }
        }
    }

    /// Commit from the edge after reconsidering: revalidate the whole gap now, skip preparation
    /// (it was just watched), and keep the same origin so the audience sees one continuous scene.
    fn commit_hesitant_gap(
        &mut self,
        id: CreatureId,
        target_window: WindowKey,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let Some(c) = self.save.creatures.iter().find(|c| c.id == id) else {
            return false;
        };
        // The attempt's own plan is not a reservation against itself.
        let Some((mut gap, decision, _)) = self.gap_candidate(c, desktop) else {
            return false;
        };
        if decision == gaps::GapDecision::Refuse
            || gap.bridge
            || gap.hop.surface.window_key != Some(target_window)
        {
            return false;
        }
        let Some(plan) = self.attention.plans.get(&id).copied() else {
            return false;
        };
        gap.preparation = 0.0;
        gap.runup = gap.hop.start;
        // Committing below its confidence margin is a near miss that must catch the edge.
        gap.catch = true;
        let seconds = gap.hop.duration + 2.0 + 2.4;
        let reaction = Reaction {
            origin: plan.origin,
            role: Role::Journey {
                target_window: Some(target_window),
                target_bounds: Some(gap.target_bounds),
                stage: Stage::Act,
                hanging: 0.0,
                rewarding: true,
                escape: false,
                since: 0.0,
                caught: false,
            },
            target: gap.hop.target,
            emotion: AttentionEmotion::Concerned,
            elapsed: 0.0,
            seconds,
            delay: 0.0,
            travel_elapsed: 0.0,
            walk: None,
            display_walk: None,
            surface: plan.surface,
            action: ActionKind::InspectScreen,
            cue: None,
            ride: None,
        };
        self.attention.plans.remove(&id);
        let creature = creature_mut(&mut self.save.creatures, id).unwrap();
        creature.state.action = ActionKind::InspectScreen;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = f32::MAX;
        self.window_journeys.insert(id, WindowJourney::Gap(gap));
        self.begin_attention(id, reaction);
        self.extend_audience(id, seconds);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::ledges::tests::edge_scene;
    use super::*;

    fn tick(world: &mut World, desktop: &mut DesktopSnapshot, now: OffsetDateTime, step: u64) {
        desktop.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
        world.tick(
            now + Duration::milliseconds(step as i64 * 50),
            0.05,
            desktop,
        );
    }

    /// A creature whose confidence sits just below the risk of the fixture's 70-point gap.
    fn hesitant_scene(retry: bool, commit: bool) -> (World, DesktopSnapshot, OffsetDateTime) {
        let (mut world, desktop, now) = edge_scene(true, true);
        world.clear_attention();
        world.window_journeys.clear();
        let c = &mut world.save.creatures[0];
        c.state.action = ActionKind::Perch;
        c.state.position = Point { x: 770.0, y: 600.0 };
        c.personality.boldness = 0.5;
        c.state.drives.energy = 0.8;
        let id = c.id;
        assert!(world.try_gap_attention(&desktop));
        let plan = world.attention.plans.get_mut(&id).unwrap();
        let Role::Hesitate {
            retry: r,
            commit: m,
            ..
        } = &mut plan.role
        else {
            panic!("expected a hesitation, got {:?}", plan.role);
        };
        (*r, *m) = (retry, commit);
        (world, desktop, now)
    }

    fn phases(world: &World, id: CreatureId) -> Option<(HesitatePhase, u8)> {
        world.attention.plans.get(&id).and_then(|p| match p.role {
            Role::Hesitate { phase, attempt, .. } => Some((phase, attempt)),
            _ => None,
        })
    }

    #[test]
    fn a_cautious_creature_prepares_reconsiders_twice_and_chickens_out_without_a_loop() {
        let (mut world, mut desktop, now) = hesitant_scene(true, false);
        let id = world.save.creatures[0].id;
        let mut seen = Vec::new();
        let mut backed_up = false;
        let mut audience_relieved = false;
        let mut audience_worried = false;
        for step in 3..400 {
            tick(&mut world, &mut desktop, now, step);
            if let Some(phase) = phases(&world, id)
                && seen.last() != Some(&phase)
            {
                seen.push(phase);
            }
            let c = &world.save.creatures[0];
            backed_up |= c.state.position.x < 760.0 && c.state.action == ActionKind::Traverse;
            assert!(!world.window_journeys.contains_key(&id), "never jumps");
            assert_eq!(c.state.surface.window_key, Some(701));
            let observer = &world.save.creatures[1];
            audience_worried |= observer.state.attention.is_some_and(|p| {
                matches!(
                    p.emotion,
                    AttentionEmotion::Concerned | AttentionEmotion::Averting
                )
            });
            audience_relieved |= observer
                .state
                .attention
                .is_some_and(|p| p.emotion == AttentionEmotion::Relieved);
            if observer.state.attention.is_some() {
                assert_ne!(
                    observer.state.action,
                    ActionKind::Greet,
                    "no cheering a retreat"
                );
            }
            if !seen.is_empty() && phases(&world, id).is_none() {
                break;
            }
        }
        use HesitatePhase::*;
        assert_eq!(
            seen,
            [
                (Look, 1),
                (BackUp, 1),
                (Approach, 1),
                (Reconsider, 1),
                (BackUp, 2),
                (Approach, 2),
                (Reconsider, 2),
                (Retreat, 2)
            ]
        );
        assert!(backed_up && audience_worried && audience_relieved);
        assert!(!world.attention.owns(id));
        // The same gap is not immediately retried; the setback also raises risk elsewhere.
        let setback = world.attention.setback(id).unwrap();
        assert_eq!(setback.route, Some((701, 702)));
        world.save.creatures[0].state.position = Point { x: 770.0, y: 600.0 };
        world.attention.cooldowns.clear();
        world.attention.colony_cooldown = 0.0;
        assert!(
            world
                .gap_candidate(&world.save.creatures[0], &desktop)
                .is_none()
        );
    }

    #[test]
    fn committing_after_reconsidering_keeps_the_origin_and_catches_the_edge() {
        let (mut world, mut desktop, now) = hesitant_scene(false, true);
        let id = world.save.creatures[0].id;
        let origin = world.attention.plans[&id].origin;
        let mut jumped = false;
        let mut caught = false;
        let mut gasped = false;
        for step in 3..400 {
            tick(&mut world, &mut desktop, now, step);
            if let Some(WindowJourney::Gap(gap)) = world.window_journeys.get_mut(&id) {
                gap.assistance_slip = false;
                jumped = true;
                assert_eq!(gap.preparation, 0.0);
                assert_eq!(world.attention.plans[&id].origin, origin);
            }
            caught |= world.save.creatures[0]
                .state
                .attention
                .is_some_and(|p| p.hanging > 0.99);
            gasped |= caught
                && world.save.creatures[1..].iter().any(|o| {
                    o.state
                        .attention
                        .is_some_and(|p| p.emotion == AttentionEmotion::Startled)
                });
            if jumped && !world.window_journeys.contains_key(&id) && !world.attention.owns(id) {
                break;
            }
        }
        assert!(jumped && caught && gasped, "{jumped} {caught} {gasped}");
        assert_eq!(world.save.creatures[0].state.surface.window_key, Some(702));
    }

    #[test]
    fn a_moved_destination_or_reduced_motion_ends_hesitation_in_place() {
        for reduced in [false, true] {
            let (mut world, mut desktop, now) = hesitant_scene(true, true);
            for step in 3..30 {
                tick(&mut world, &mut desktop, now, step);
            }
            if reduced {
                world.save.settings.reduce_motion = true;
            } else {
                desktop.windows[1].bounds.x += 30.0;
            }
            tick(&mut world, &mut desktop, now, 30);
            assert!(world.attention.plans.is_empty(), "reduced={reduced}");
            assert!(world.window_journeys.is_empty());
            assert_eq!(world.save.creatures[0].state.surface.window_key, Some(701));
        }
        // Reduced motion never begins a hesitation: a borderline gap is simply declined.
        let (mut world, desktop, _) = edge_scene(true, true);
        world.clear_attention();
        world.save.settings.reduce_motion = true;
        let c = &mut world.save.creatures[0];
        c.personality.boldness = 0.5;
        c.state.drives.energy = 0.8;
        c.state.action = ActionKind::Perch;
        assert!(!world.try_gap_attention(&desktop));
    }

    #[test]
    fn recent_setbacks_raise_risk_and_decay() {
        let (mut world, desktop, _) = edge_scene(true, true);
        world.clear_attention();
        world.window_journeys.clear();
        let c = &mut world.save.creatures[0];
        c.state.action = ActionKind::Perch;
        c.state.position = Point { x: 770.0, y: 600.0 };
        c.personality.boldness = 0.65;
        c.state.drives.energy = 0.8;
        let id = c.id;
        let decide = |world: &World| {
            world
                .gap_candidate(&world.save.creatures[0], &desktop)
                .map(|(_, decision, _)| decision)
        };
        assert_eq!(decide(&world), Some(gaps::GapDecision::Commit));
        world.attention.record_setback(id, None);
        world.attention.record_setback(id, None);
        assert_eq!(decide(&world), Some(gaps::GapDecision::Hesitate));
        world.attention.decay_setbacks(SETBACK_SECONDS + 1.0);
        assert_eq!(decide(&world), Some(gaps::GapDecision::Commit));
        // Sleepiness lowers confidence too.
        world.save.creatures[0].state.drives.sleep_pressure = 0.8;
        assert_ne!(decide(&world), Some(gaps::GapDecision::Commit));
    }

    /// Changing its mind about a gap is something a creature does with its whole body: fretting
    /// while it looks the drop over and while it waits at the run-up, teetering on the very edge
    /// each time it leans out over it, and showing nothing at all while it is walking.
    #[test]
    fn reconsidering_a_gap_frets_at_the_drop_and_teeters_on_the_edge() {
        let (mut world, mut desktop, now) = hesitant_scene(true, false);
        let id = world.save.creatures[0].id;
        let mut poses = super::super::tests::Poses::default();
        let mut leaned_out = 0;
        for step in 3..400 {
            poses.tick(&mut world, |w| tick(w, &mut desktop, now, step));
            let Some(gesture) = world.save.creatures[0]
                .state
                .attention
                .and_then(|pose| pose.gesture)
            else {
                continue;
            };
            let Some((phase, _)) = phases(&world, id) else {
                continue;
            };
            match gesture {
                Gesture::Balance => {
                    assert_eq!(phase, HesitatePhase::Reconsider);
                    leaned_out += 1;
                }
                Gesture::Worry => assert!(
                    matches!(
                        phase,
                        HesitatePhase::Look | HesitatePhase::BackUp | HesitatePhase::Approach
                    ),
                    "fretting in {phase:?}"
                ),
                other => panic!("a hesitation struck {other:?} in {phase:?}"),
            }
        }
        assert!(
            poses.by(id).contains(&Gesture::Worry) && leaned_out >= 10,
            "{poses:?}"
        );
        // Backing off for good is a relief, and relief is not a pose.
        assert_eq!(poses.by(id).last(), Some(&Gesture::Balance), "{poses:?}");
    }
}
