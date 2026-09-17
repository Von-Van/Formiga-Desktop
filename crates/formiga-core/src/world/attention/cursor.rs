use super::*;
use crate::cursor::CursorInterest;

fn cursor_confidence(creature: &Creature) -> f32 {
    (creature.personality.boldness + f32::from(creature.tendencies.cursor_trust) / 100.0 * 0.2)
        .clamp(0.0, 1.0)
}

/// A race alongside the cursor is brief by design.
const RACE_SECONDS: f32 = 4.0;

impl World {
    /// Keep a racing creature alongside the cursor while it is still there to race.
    pub(super) fn update_cursor_races(&mut self, desktop: &DesktopSnapshot) {
        let racers: Vec<_> = self
            .attention
            .plans
            .iter()
            .filter_map(|(&id, p)| {
                matches!(p.role, Role::Cursor { racing: true, .. }).then_some(id)
            })
            .collect();
        for id in racers {
            let Some(creature) = self.save.creatures.iter().find(|c| c.id == id) else {
                continue;
            };
            // The race is over when the cursor is gone, off this display, or far away — not
            // merely because it happens to be passing overhead this instant.
            let over = !self.save.settings.cursor_reactions
                || !self.cursor_observer.safe
                || !desktop.cursor.available
                || !desktop.monitors.iter().any(|m| {
                    m.id == creature.state.surface.monitor_id
                        && m.bounds.contains(desktop.cursor.position)
                })
                || (desktop.cursor.position.x - creature.state.position.x).abs() > 400.0;
            let reach =
                (desktop.cursor.position.x - creature.state.position.x).clamp(-110.0, 110.0);
            let goal = (!over)
                .then(|| {
                    motion::play_goal(self, creature, creature.state.position.x + reach, desktop)
                })
                .flatten();
            let Some(plan) = self.attention.plans.get_mut(&id) else {
                continue;
            };
            if over {
                plan.walk = None;
                if let Role::Cursor { racing, .. } = &mut plan.role {
                    *racing = false;
                }
                continue;
            }
            if let Some(destination) = goal {
                plan.walk = Some(ShortWalk::playing(destination));
                plan.travel_elapsed = 0.0;
            }
        }
    }

    pub(super) fn cursor_reaction_valid(
        &self,
        creature: &Creature,
        investigate: bool,
        anchor: Point,
        desktop: &DesktopSnapshot,
    ) -> bool {
        self.save.settings.cursor_reactions
            && self.cursor_observer.safe
            && desktop.cursor.available
            && desktop.monitors.iter().any(|m| {
                m.id == creature.state.surface.monitor_id
                    && m.bounds.contains(desktop.cursor.position)
            })
            && desktop
                .cursor
                .position
                .distance(head_point(creature, &self.save.settings, desktop))
                <= 280.0
            && (!investigate || desktop.cursor.position.distance(anchor) <= 80.0)
    }

    pub(super) fn try_cursor_attention(&mut self, desktop: &DesktopSnapshot) {
        if !self.save.settings.cursor_reactions
            || !self.cursor_observer.safe
            || self.attention.colony_cooldown > 0.0
        {
            return;
        }
        let Some(cue) = self.cursor_observer.cue() else {
            return;
        };
        let target = if cue.kind == CursorInterest::Fast {
            desktop.cursor.position
        } else {
            cue.point
        };
        let candidate = self
            .save
            .creatures
            .iter()
            .filter(|c| {
                self.attention_eligible(c, desktop)
                    && c.state.cursor_cooldown <= 0.0
                    && c.state.surface.monitor_id == cue.monitor
                    && c.personality.cursor_interest >= 0.25
            })
            .filter_map(|c| {
                let distance = head_point(c, &self.save.settings, desktop).distance(target);
                (distance <= 220.0
                    && (cue.kind == CursorInterest::Fast || c.personality.curiosity >= 0.35))
                    .then_some((distance - c.personality.cursor_interest * 80.0, c.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, id)) = candidate else {
            return;
        };
        let creature = self.save.creatures.iter().find(|c| c.id == id).unwrap();
        let wary = cursor_confidence(creature) < 0.4;
        let direction = if creature.state.position.x < target.x {
            -1.0
        } else {
            1.0
        };
        // A playful creature with an eye for the cursor races a fast pass instead of just
        // watching it go by.
        let racing = cue.kind == CursorInterest::Fast
            && !wary
            && !self.save.settings.reduce_motion
            && creature.personality.cursor_interest > 0.5
            && creature.personality.playfulness > 0.5
            && creature.state.drives.energy > 0.45;
        let desired = if wary && creature.state.position.distance(target) < 140.0 {
            Some(creature.state.position.x + direction * 48.0)
        } else if racing {
            // Chase it by one dash at a time, never in one impossible stride.
            Some(
                creature.state.position.x
                    + (target.x - creature.state.position.x).clamp(-110.0, 110.0),
            )
        } else if cue.kind == CursorInterest::Local && cursor_confidence(creature) >= 0.4 {
            Some(target.x + direction * 40.0)
        } else {
            None
        };
        let walk = if self.save.settings.reduce_motion {
            None
        } else {
            desired
                .and_then(|x| motion::safe_goal(self, creature, x, desktop))
                .map(ShortWalk::new)
        };
        let brief = cue.kind == CursorInterest::Fast && walk.is_none();
        let reaction = Reaction {
            ride: None,
            cue: None,
            origin: Origin::Cursor(cue.origin),
            role: Role::Cursor {
                investigate: cue.kind == CursorInterest::Local,
                anchor: target,
                racing: racing && walk.is_some(),
            },
            target,
            emotion: if wary {
                AttentionEmotion::Concerned
            } else {
                AttentionEmotion::Curious
            },
            elapsed: 0.0,
            seconds: if brief {
                1.4
            } else if racing {
                RACE_SECONDS
            } else {
                REACTION_SECONDS
            },
            delay: 0.0,
            travel_elapsed: 0.0,
            walk,
            display_walk: None,
            surface: (
                creature.state.surface.monitor_id,
                creature.state.surface.window_key,
            ),
            action: ActionKind::InspectScreen,
        };
        self.begin_attention(id, reaction);
        let cooldown = if brief { 2.5 } else { REACTION_COOLDOWN };
        self.attention.cooldowns.insert(id, cooldown);
        creature_mut(&mut self.save.creatures, id)
            .unwrap()
            .state
            .cursor_cooldown = cooldown;
        // Only visible investigation/withdrawal recruits companions; a passing glance stays small.
        if !brief {
            self.recruit_attention_observers(id, reaction.origin, desktop);
        }
        self.attention.colony_cooldown = if brief { 2.5 } else { 7.0 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(boldness: f32) -> (World, DesktopSnapshot, OffsetDateTime) {
        let (mut world, desktop, now) = super::super::tests::open_scene();
        world.save.creatures.truncate(1);
        let creature = &mut world.save.creatures[0];
        creature.personality.cursor_interest = 1.0;
        creature.personality.boldness = boldness;
        creature.tendencies.cursor_trust = 0;
        (world, desktop, now)
    }

    fn point(
        world: &mut World,
        desktop: &mut DesktopSnapshot,
        now: OffsetDateTime,
        millis: u64,
        x: f32,
        y: f32,
    ) {
        desktop.cursor.available = true;
        desktop.cursor.position = Point { x, y };
        desktop.cursor_sample_millis = Some(millis);
        world.tick(now + Duration::milliseconds(millis as i64), 0.05, desktop);
    }

    fn circle(world: &mut World, desktop: &mut DesktopSnapshot, now: OffsetDateTime) {
        for step in 0..=20 {
            let angle = step as f32 * std::f32::consts::FRAC_PI_4;
            point(
                world,
                desktop,
                now,
                step * 50,
                560.0 + angle.cos() * 24.0,
                800.0 + angle.sin() * 24.0,
            );
        }
    }

    #[test]
    fn local_motion_draws_a_bold_creature_closer_and_sends_a_wary_one_back() {
        for bold in [false, true] {
            let (mut world, mut desktop, now) = scene(if bold { 0.9 } else { 0.1 });
            circle(&mut world, &mut desktop, now);
            let id = world.save.creatures[0].id;
            assert!(matches!(
                world.attention.plans[&id].role,
                Role::Cursor {
                    investigate: true,
                    ..
                }
            ));
            for step in 21..=40 {
                point(&mut world, &mut desktop, now, step * 50, 560.0, 800.0);
            }
            let creature = &world.save.creatures[0];
            if bold {
                assert!(creature.state.position.x > 490.0);
            } else {
                assert!(creature.state.position.x < 470.0);
            }
            assert!(creature.state.cursor_cooldown > 5.0);
            assert_eq!(creature.state.surface.kind, SurfaceKind::ScreenFloor);
            assert!(creature.state.attention.is_some());
        }
    }

    #[test]
    fn a_fast_pass_gets_a_short_live_glance_and_a_warp_or_disabled_preference_cancels_it() {
        for disable in [false, true] {
            let (mut world, mut desktop, now) = scene(0.9);
            for step in 0..=2 {
                point(
                    &mut world,
                    &mut desktop,
                    now,
                    step * 50,
                    530.0 + step as f32 * 40.0,
                    800.0,
                );
            }
            let id = world.save.creatures[0].id;
            assert_eq!(world.attention.plans[&id].seconds, 1.4);
            point(&mut world, &mut desktop, now, 150, 640.0, 800.0);
            assert_eq!(
                world.save.creatures[0].state.attention.unwrap().target.x,
                640.0
            );
            assert_eq!(world.save.creatures[0].state.position.x, 480.0);
            if disable {
                world.save.settings.cursor_reactions = false;
            }
            point(
                &mut world,
                &mut desktop,
                now,
                200,
                if disable { 640.0 } else { 1_300.0 },
                800.0,
            );
            assert!(!world.attention.owns(id));
            assert!(world.save.creatures[0].state.attention.is_none());
            assert!(world.topology.invitation().is_none());
        }
    }

    #[test]
    fn reduced_motion_and_blocked_withdrawal_keep_the_reaction_stationary() {
        for reduced in [false, true] {
            let (mut world, mut desktop, now) = scene(0.1);
            world.save.settings.reduce_motion = reduced;
            if !reduced {
                world.save.settings.habitat.zones.push(HabitatZone {
                    id: 500,
                    display: desktop.monitors[0].display_key,
                    kind: HabitatZoneKind::Excluded,
                    enabled: true,
                    normalized_bounds: DesktopRect {
                        x: 440.0 / 1440.0,
                        y: 0.0,
                        width: 24.0 / 1440.0,
                        height: 1.0,
                    },
                });
            }
            circle(&mut world, &mut desktop, now);
            let id = world.save.creatures[0].id;
            assert!(world.attention.plans[&id].walk.is_none());
            for step in 21..=40 {
                point(&mut world, &mut desktop, now, step * 50, 560.0, 800.0);
            }
            assert_eq!(world.save.creatures[0].state.position.x, 480.0);
            assert!(world.save.creatures[0].state.attention.is_some());
        }
    }

    /// Interest in the cursor is something a colony notices. Companions watch the creature that
    /// is creeping up on it or running alongside it, not the cursor itself.
    #[test]
    fn companions_watch_the_one_investigating_or_racing_the_cursor() {
        for racing in [false, true] {
            let (mut world, mut desktop, now) = super::super::tests::open_scene();
            for (index, creature) in world.save.creatures.iter_mut().enumerate() {
                creature.personality.cursor_interest = if index == 0 { 1.0 } else { 0.0 };
                creature.personality.boldness = 0.9;
                creature.personality.playfulness = if index == 0 { 0.9 } else { 0.0 };
                creature.tendencies.cursor_trust = 0;
            }
            let id = world.save.creatures[0].id;
            let start = if racing {
                for step in 0..=3 {
                    point(
                        &mut world,
                        &mut desktop,
                        now,
                        step * 50,
                        530.0 + step as f32 * 40.0,
                        800.0,
                    );
                }
                assert!(matches!(
                    world.attention.plans[&id].role,
                    Role::Cursor { racing: true, .. }
                ));
                200
            } else {
                circle(&mut world, &mut desktop, now);
                assert!(matches!(
                    world.attention.plans[&id].role,
                    Role::Cursor {
                        investigate: true,
                        ..
                    }
                ));
                1_050
            };
            let mut watched = false;
            for step in 1..=50 {
                point(
                    &mut world,
                    &mut desktop,
                    now,
                    start + step * 50,
                    620.0,
                    800.0,
                );
                let head = head_point(&world.save.creatures[0], &world.save.settings, &desktop);
                watched |=
                    world.save.creatures.iter().skip(1).any(|other| {
                        world.attention.plans.get(&other.id).is_some_and(
                            |p| matches!(p.role, Role::Observer { actor } if actor == id),
                        ) && other
                            .state
                            .attention
                            .is_some_and(|pose| (pose.target.x - head.x).abs() < 4.0)
                    });
            }
            assert!(watched, "racing {racing}");
        }
    }

    #[test]
    fn cursor_learning_is_bounded_and_can_shift_an_uncertain_creature() {
        let (mut world, _, _) = scene(0.4);
        let creature = &mut world.save.creatures[0];
        creature.tendencies.cursor_trust = -100;
        let wary = cursor_confidence(creature);
        creature.tendencies.cursor_trust = 100;
        let trusting = cursor_confidence(creature);
        assert!(wary < 0.4 && trusting > 0.4);
        creature.personality.boldness = 1.0;
        assert_eq!(cursor_confidence(creature), 1.0);
    }

    #[test]
    fn sleeping_creatures_and_disabled_cursor_reactions_ignore_local_motion() {
        for asleep in [false, true] {
            let (mut world, mut desktop, now) = scene(0.9);
            if asleep {
                world.save.creatures[0].state.action = ActionKind::Sleep;
            } else {
                world.save.settings.cursor_reactions = false;
            }
            circle(&mut world, &mut desktop, now);
            assert!(world.attention.plans.is_empty());
            assert!(world.save.creatures[0].state.attention.is_none());
        }
    }

    #[test]
    fn a_playful_creature_races_a_fast_cursor_and_stops_when_it_is_gone() {
        for disabled in [false, true] {
            let (mut world, mut desktop, now) = scene(0.9);
            world.save.creatures[0].personality.playfulness = 0.9;
            world.save.settings.cursor_reactions = !disabled;
            let id = world.save.creatures[0].id;
            let start = world.save.creatures[0].state.position.x;
            // A cursor sweeping past at speed.
            for step in 0..=3 {
                point(
                    &mut world,
                    &mut desktop,
                    now,
                    step * 50,
                    530.0 + step as f32 * 40.0,
                    800.0,
                );
            }
            let racing = world
                .attention
                .plans
                .get(&id)
                .is_some_and(|p| matches!(p.role, Role::Cursor { racing: true, .. }));
            assert_eq!(racing, !disabled, "cursor reactions gate the race");
            if disabled {
                continue;
            }
            assert_eq!(world.attention.plans[&id].seconds, RACE_SECONDS);
            // It keeps pace while the cursor sweeps quickly back and forth nearby.
            for step in 4..=30 {
                let sweep = ((step as f32 * 28.0) % 600.0 - 300.0).abs();
                point(
                    &mut world,
                    &mut desktop,
                    now,
                    step * 50,
                    400.0 + sweep,
                    800.0,
                );
            }
            let chased = world.save.creatures[0].state.position.x;
            assert!(
                (chased - start).abs() > 8.0,
                "it runs after the cursor: {start} -> {chased}"
            );
            // The cursor leaves; the race stops rather than following it away.
            desktop.cursor.available = false;
            point(&mut world, &mut desktop, now, 1_600, 1_300.0, 800.0);
            assert!(
                !world
                    .attention
                    .plans
                    .get(&id)
                    .is_some_and(|p| matches!(p.role, Role::Cursor { racing: true, .. }))
            );
            let settled = world.save.creatures[0].state.position.x;
            for step in 1..=16 {
                point(
                    &mut world,
                    &mut desktop,
                    now,
                    1_600 + step * 50,
                    1_300.0,
                    800.0,
                );
            }
            assert!(
                (world.save.creatures[0].state.position.x - settled).abs() < 260.0,
                "it does not chase a cursor that is gone"
            );
        }
    }

    /// Having crept up on the cursor, a bold creature reaches out for it, facing what it is
    /// reaching at. A wary one, which has backed away from the same cursor, keeps its distance
    /// and its hands to itself.
    #[test]
    fn a_bold_creature_reaches_for_the_cursor_it_crept_up_on() {
        for bold in [0.9f32, 0.1] {
            let (mut world, mut desktop, now) = scene(bold);
            let mut poses = super::super::tests::Poses::default();
            for step in 0..=20u64 {
                let angle = step as f32 * std::f32::consts::FRAC_PI_4;
                poses.tick(&mut world, |world| {
                    point(
                        world,
                        &mut desktop,
                        now,
                        step * 50,
                        560.0 + angle.cos() * 24.0,
                        800.0 + angle.sin() * 24.0,
                    );
                });
            }
            let mut reached = 0;
            for step in 21..=80u64 {
                poses.tick(&mut world, |world| {
                    point(world, &mut desktop, now, step * 50, 560.0, 800.0);
                });
                let creature = &world.save.creatures[0];
                if creature.state.attention.and_then(|pose| pose.gesture) == Some(Gesture::Reach) {
                    assert!(creature.state.facing_right, "reaching away from the cursor");
                    assert!(
                        (creature.state.position.x - 560.0).abs() <= CURSOR_REACH,
                        "reaching for a cursor it never got near"
                    );
                    reached += 1;
                }
            }
            assert_eq!(
                reached > 0,
                bold > 0.5,
                "bold {bold} reached {reached} times: {poses:?}"
            );
        }
    }
}
