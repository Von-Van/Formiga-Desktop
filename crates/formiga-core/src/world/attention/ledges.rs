use super::super::surfaces::drop_below;
use super::*;

impl World {
    pub(super) fn try_ledge_attention(&mut self, desktop: &DesktopSnapshot) {
        if self.surface_memory.inspect_in > 0.0
            || self.attention.colony_cooldown > 0.0
            || !self.save.settings.window_ledges
        {
            return;
        }
        // One bounded attempt per interval, including attempts with no suitable surface.
        self.surface_memory.inspect_in = self.ambient_rng.random_range(35.0..65.0);
        if self.try_gap_attention(desktop) {
            return;
        }
        let candidate = self
            .save
            .creatures
            .iter()
            .filter(|c| {
                self.attention_eligible(c, desktop)
                    && matches!(c.state.action, ActionKind::Idle | ActionKind::Perch)
                    && c.personality.curiosity >= 0.35
            })
            .filter_map(|c| {
                let key = c.state.surface.window_key?;
                let window = desktop
                    .windows
                    .iter()
                    .take(MAX_TOPOLOGY_WINDOWS)
                    .find(|w| w.key == key && w.visible && !w.minimized)?;
                let bounds = window.bounds;
                let gap = (bounds.width * 0.05).max(16.0);
                if bounds.width < gap * 2.0 + 24.0 {
                    return None;
                }
                let right = c.state.position.x > bounds.x + bounds.width * 0.5;
                let edge = if right { bounds.right() } else { bounds.x };
                let goal = edge + if right { -gap } else { gap };
                let familiar = self
                    .surface_memory
                    .nearby(c, desktop)
                    .filter(|p| (p.x - c.state.position.x).abs() >= 24.0)
                    .and_then(|p| motion::safe_goal(self, c, p.x, desktop));
                let resting = familiar.is_some();
                let destination = if resting {
                    familiar
                } else if (goal - c.state.position.x).abs() <= 8.0 {
                    None
                } else {
                    Some(motion::safe_goal(self, c, goal, desktop)?)
                };
                let drop = drop_below(
                    c,
                    edge + if right { 14.0 } else { -14.0 },
                    desktop,
                    &self.save.settings,
                );
                if !resting && drop < 12.0 {
                    return None;
                }
                let target = if resting {
                    familiar.unwrap()
                } else {
                    Point {
                        x: edge + if right { 20.0 } else { -20.0 },
                        y: bounds.y + drop.min(180.0),
                    }
                };
                Some((
                    c.personality.curiosity + f32::from(resting) * 0.15,
                    c.id,
                    key,
                    bounds,
                    drop,
                    resting,
                    destination,
                    target,
                ))
            })
            .max_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, id, window, bounds, drop, resting, destination, target)) = candidate else {
            return;
        };
        let creature = self.save.creatures.iter().find(|c| c.id == id).unwrap();
        let commute_destination = if !resting
            && !self.save.settings.reduce_motion
            && creature.personality.boldness > 0.75
            && creature.personality.window_tolerance > 0.6
            && self.ambient_rng.random_ratio(1, 3)
        {
            let side = if creature.state.position.x > bounds.x + bounds.width * 0.5 {
                -1.0
            } else {
                1.0
            };
            motion::safe_goal(
                self,
                creature,
                creature.state.position.x + side * 36.0,
                desktop,
            )
        } else {
            None
        };
        let commute = commute_destination.is_some();
        let destination = commute_destination.or(destination);
        self.surface_memory.next_origin = self.surface_memory.next_origin.wrapping_add(1);
        let origin = Origin::Surface(self.surface_memory.next_origin);
        let emotion = if !resting && drop > 140.0 && creature.personality.boldness < 0.4 {
            AttentionEmotion::Concerned
        } else if resting || creature.personality.boldness > 0.7 {
            AttentionEmotion::Enjoying
        } else {
            AttentionEmotion::Curious
        };
        let reaction = Reaction {
            ride: None,
            origin,
            role: Role::Ledge {
                window,
                bounds,
                drop,
                resting,
                declined: false,
                commute,
            },
            target,
            emotion,
            elapsed: 0.0,
            seconds: REACTION_SECONDS,
            delay: 0.0,
            travel_elapsed: 0.0,
            display_walk: None,
            walk: if self.save.settings.reduce_motion {
                None
            } else {
                destination.map(|destination| ShortWalk::watching(destination, bounds))
            },
            surface: (creature.state.surface.monitor_id, Some(window)),
            action: ActionKind::InspectScreen,
            cue: None,
        };
        self.begin_attention(id, reaction);
        if !resting {
            self.recruit_attention_observers(id, origin, desktop);
        }
        self.attention.colony_cooldown = 12.0;
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::tests::scene;
    use super::*;

    fn tick(world: &mut World, desktop: &mut DesktopSnapshot, now: OffsetDateTime, step: u64) {
        desktop.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
        world.tick(
            now + Duration::milliseconds(step as i64 * 50),
            0.05,
            desktop,
        );
    }

    pub(in super::super) fn edge_scene(
        gap: bool,
        bold: bool,
    ) -> (World, DesktopSnapshot, OffsetDateTime) {
        let (mut world, mut desktop, now) = scene();
        world.save.settings.display_scale = 2;
        for (index, c) in world.save.creatures.iter_mut().enumerate() {
            if index == 0 {
                c.personality.boldness = f32::from(bold);
                c.personality.playfulness = 1.0;
                c.state.position.x = 770.0;
                c.state.surface.relative_x = 0.95;
            } else {
                c.personality.playfulness = if index == 1 { 1.0 } else { 0.0 };
            }
        }
        if gap {
            let mut window = desktop.windows[0].clone();
            window.key = 702;
            window.bounds.x = 870.0;
            window.bounds.width = 300.0;
            desktop.windows.push(window);
        }
        world.geometry_observer = Default::default();
        tick(&mut world, &mut desktop, now, 1);
        world.surface_memory.inspect_in = 0.0;
        tick(&mut world, &mut desktop, now, 2);
        (world, desktop, now)
    }

    #[test]
    fn peeking_keeps_foot_contact_and_gathers_one_origin_audience() {
        let (mut world, mut desktop, now) = edge_scene(false, false);
        let id = world.save.creatures[0].id;
        assert!(matches!(
            world.attention.plans[&id].role,
            Role::Ledge { resting: false, .. }
        ));
        let origin = world.attention.plans[&id].origin;
        for step in 3..70 {
            tick(&mut world, &mut desktop, now, step);
            assert_eq!(
                world.save.creatures[0].state.position,
                Point { x: 770.0, y: 600.0 }
            );
            assert!(world.attention.plans.values().all(|p| p.origin == origin));
        }
        assert!(world.save.creatures[0].state.attention.is_none());
        assert!(world.attention.plans.len() <= 3);
    }

    /// A peek is a small public event. Companions turn toward the creature on the edge rather
    /// than at the window behind it, and one with no head for heights turns its gaze away.
    #[test]
    fn a_peek_over_a_high_edge_turns_companions_toward_it_and_a_timid_one_looks_away() {
        let (mut world, mut desktop, now) = edge_scene(false, false);
        let actor = world.save.creatures[0].id;
        let mut seen: BTreeMap<CreatureId, Vec<AttentionEmotion>> = BTreeMap::new();
        let mut watched_the_actor = false;
        let mut covered_eyes = false;
        for step in 3..70 {
            tick(&mut world, &mut desktop, now, step);
            let head = head_point(&world.save.creatures[0], &world.save.settings, &desktop);
            for creature in world.save.creatures.iter().skip(1) {
                let watching =
                    world.attention.plans.get(&creature.id).is_some_and(
                        |p| matches!(p.role, Role::Observer { actor: a } if a == actor),
                    );
                let Some(pose) = creature.state.attention.filter(|_| watching) else {
                    continue;
                };
                let log = seen.entry(creature.id).or_default();
                if log.last() != Some(&pose.emotion) {
                    log.push(pose.emotion);
                }
                watched_the_actor |= pose.target == head;
                covered_eyes |= pose.emotion == AttentionEmotion::Averting
                    && (pose.target.x - creature.state.position.x).signum()
                        != (head.x - creature.state.position.x).signum();
            }
        }
        assert!(
            watched_the_actor && covered_eyes,
            "{watched_the_actor} {covered_eyes}"
        );
        assert!(seen.len() >= 2, "more than one companion looks up");
        for log in seen.values() {
            assert_eq!(log[0], AttentionEmotion::Curious, "everyone notices first");
            assert!(
                matches!(
                    log.last(),
                    Some(AttentionEmotion::Relieved | AttentionEmotion::Enjoying)
                ),
                "watching ends calmly: {log:?}"
            );
        }
    }

    /// How far it is to the next support changes where a creature looks over an edge and how it
    /// feels about being there. Only the height differs between these two ledges.
    #[test]
    fn the_height_below_an_edge_changes_where_a_creature_looks_and_how_it_feels() {
        let mut looks = Vec::new();
        for ground_below in [false, true] {
            let (mut world, mut desktop, _) = scene();
            world.clear_attention();
            world.surface_memory.inspect_in = 0.0;
            let creature = &mut world.save.creatures[0];
            creature.state.action = ActionKind::Perch;
            creature.state.position = Point { x: 770.0, y: 600.0 };
            creature.state.surface.relative_x = 0.95;
            if ground_below {
                let mut below = desktop.windows[0].clone();
                below.key = 703;
                below.z_order = 5;
                below.bounds = DesktopRect {
                    x: 780.0,
                    y: 700.0,
                    width: 300.0,
                    height: 200.0,
                };
                desktop.windows.push(below);
            }
            world.try_ledge_attention(&desktop);
            let plan = world.attention.plans[&world.save.creatures[0].id];
            let Role::Ledge { drop, .. } = plan.role else {
                panic!("expected a peek, got {:?}", plan.role);
            };
            looks.push((drop, plan.target.y, plan.emotion));
        }
        let (long_fall, short_fall) = (looks[0], looks[1]);
        assert_eq!(long_fall.0, 246.0);
        assert_eq!(short_fall.0, 100.0);
        assert!(long_fall.1 > short_fall.1, "a longer fall is looked down");
        assert_eq!(long_fall.2, AttentionEmotion::Concerned);
        assert_eq!(short_fall.2, AttentionEmotion::Curious);
    }

    /// A place a creature has spent real time on draws it back to it. Going to sit somewhere
    /// familiar is private: it is not the kind of thing that gathers a crowd.
    #[test]
    fn a_favorite_ledge_spot_draws_a_creature_back_without_gathering_an_audience() {
        let (mut world, mut desktop, now) = scene();
        world.clear_attention();
        let id = world.save.creatures[0].id;
        let creature = &mut world.save.creatures[0];
        creature.personality.activity = 1.0;
        creature.state.action = ActionKind::Perch;
        creature.state.action_duration = 100.0;
        creature.state.position = Point { x: 320.0, y: 600.0 };
        creature.state.surface.relative_x = 0.2;
        // A minute of quiet time in one place is what makes it a favorite.
        for _ in 0..65 {
            world
                .surface_memory
                .update(&world.save.creatures, &desktop, 1.0, true);
        }
        let creature = &mut world.save.creatures[0];
        creature.state.position.x = 420.0;
        creature.state.surface.relative_x = (420.0 - 200.0) / 600.0;
        world.try_ledge_attention(&desktop);
        assert!(matches!(
            world.attention.plans[&id].role,
            Role::Ledge { resting: true, .. }
        ));
        assert_eq!(
            world.attention.plans[&id].emotion,
            AttentionEmotion::Enjoying
        );
        assert_eq!(world.attention.plans.len(), 1, "a visit, not a spectacle");
        let mut arrived = false;
        for step in 1..100 {
            tick(&mut world, &mut desktop, now, step);
            let creature = &world.save.creatures[0];
            arrived |= world.attention.owns(id) && (creature.state.position.x - 320.0).abs() < 1.0;
            assert_eq!(creature.state.surface.window_key, Some(701));
            assert!(world.attention.plans.len() <= 1);
        }
        assert!(arrived);
        // Standing on the favorite spot already, there is nothing to walk back to.
        world.clear_attention();
        world.surface_memory.inspect_in = 0.0;
        let creature = &mut world.save.creatures[0];
        creature.state.action = ActionKind::Perch;
        creature.state.position = Point { x: 320.0, y: 600.0 };
        creature.state.surface.relative_x = 0.2;
        world.try_ledge_attention(&desktop);
        assert!(!matches!(
            world.attention.plans.get(&id).map(|p| p.role),
            Some(Role::Ledge { resting: true, .. })
        ));
    }

    #[test]
    fn a_moving_peek_surface_cancels_the_whole_audience_without_success() {
        let (mut world, mut desktop, now) = edge_scene(false, false);
        world.drain_events().for_each(drop);
        desktop.windows[0].bounds.x += 10.0;
        tick(&mut world, &mut desktop, now, 3);
        assert!(world.attention.plans.is_empty());
        assert_eq!(world.save.creatures[0].state.position.y, 600.0);
        assert!(
            !world
                .drain_events()
                .any(|e| matches!(e, WorldEvent::ActionCompleted { .. }))
        );
    }

    #[test]
    fn bold_gap_has_anticipation_airtime_and_an_outcome_audience() {
        let (mut world, mut desktop, now) = edge_scene(true, true);
        let id = world.save.creatures[0].id;
        assert!(matches!(
            world.window_journeys.get(&id),
            Some(WindowJourney::Gap(_))
        ));
        let mut backed_up = false;
        let mut airborne = false;
        let mut audience_celebrated = false;
        for step in 3..120 {
            tick(&mut world, &mut desktop, now, step);
            let c = &world.save.creatures[0];
            backed_up |= c.state.position.x < 760.0;
            airborne |= c.state.position.x > 800.0
                && c.state.position.x < 870.0
                && c.state.position.y < 595.0;
            audience_celebrated |= world.save.creatures[1].state.action == ActionKind::Greet
                && world.save.creatures[1]
                    .state
                    .attention
                    .is_some_and(|p| p.emotion == AttentionEmotion::Enjoying);
        }
        assert!(
            backed_up && airborne && audience_celebrated,
            "{backed_up} {airborne} {audience_celebrated}"
        );
        assert_eq!(world.save.creatures[0].state.surface.window_key, Some(702));
        assert!(world.attention.plans.is_empty());
    }

    #[test]
    fn a_timid_creature_declines_the_same_gap_and_observers_do_not_cheer() {
        let (mut world, mut desktop, now) = edge_scene(true, false);
        let id = world.save.creatures[0].id;
        assert!(!world.window_journeys.contains_key(&id));
        assert!(matches!(
            world.attention.plans[&id].role,
            Role::Ledge { declined: true, .. }
        ));
        let start = world.save.creatures[0].state.position;
        let mut relieved = false;
        for step in 3..80 {
            tick(&mut world, &mut desktop, now, step);
            assert_eq!(world.save.creatures[0].state.position, start);
            assert_ne!(world.save.creatures[1].state.action, ActionKind::Greet);
            relieved |= world.save.creatures[1]
                .state
                .attention
                .is_some_and(|p| p.emotion == AttentionEmotion::Relieved);
        }
        assert!(relieved);
    }

    #[test]
    fn lost_landing_midflight_recovers_and_never_publishes_victory() {
        let (mut world, mut desktop, now) = edge_scene(true, true);
        let id = world.save.creatures[0].id;
        for step in 3..42 {
            tick(&mut world, &mut desktop, now, step);
        }
        assert!(world.window_journeys.contains_key(&id));
        desktop.windows[1].bounds.x += 80.0;
        world.drain_events().for_each(drop);
        tick(&mut world, &mut desktop, now, 42);
        assert!(!world.window_journeys.contains_key(&id));
        assert!(world.attention.plans.is_empty());
        assert!(!world.drain_events().any(|e| matches!(
            e,
            WorldEvent::ActionCompleted {
                action: ActionKind::Landing,
                ..
            }
        )));
    }

    #[test]
    fn reduced_motion_and_new_occupants_interrupt_attempts_safely() {
        for reduced in [false, true] {
            let (mut world, mut desktop, now) = edge_scene(true, true);
            if reduced {
                world.save.settings.reduce_motion = true;
            } else {
                let c = &mut world.save.creatures[1];
                c.state.surface.window_key = Some(702);
                c.state.surface.relative_x = 16.0 / 300.0;
                c.state.position = Point { x: 886.0, y: 600.0 };
            }
            tick(&mut world, &mut desktop, now, 3);
            tick(&mut world, &mut desktop, now, 4);
            assert!(world.window_journeys.is_empty());
            assert!(world.attention.plans.is_empty());
        }
    }

    fn marginal_scene(helper: bool) -> (World, DesktopSnapshot, OffsetDateTime) {
        let (mut world, desktop, now) = edge_scene(true, true);
        world.clear_attention();
        world.window_journeys.clear();
        let c = &mut world.save.creatures[0];
        c.state.action = ActionKind::Perch;
        c.state.position = Point { x: 770.0, y: 600.0 };
        c.personality.boldness = 0.65;
        c.state.drives.energy = 0.8;
        if helper {
            let c = &mut world.save.creatures[1];
            c.state.surface.window_key = Some(702);
            c.state.surface.relative_x = 110.0 / 300.0;
            c.state.position = Point { x: 980.0, y: 600.0 };
            c.state.action = ActionKind::Perch;
        }
        assert!(world.try_gap_attention(&desktop));
        assert!(
            matches!(world.window_journeys.get(&world.save.creatures[0].id), Some(WindowJourney::Gap(g)) if g.catch)
        );
        (world, desktop, now)
    }

    #[test]
    fn marginal_jump_catches_slips_and_pulls_up_before_the_audience_celebrates() {
        let (mut world, mut desktop, now) = marginal_scene(false);
        let mut caught = false;
        let mut pulled = false;
        let mut recovered = false;
        for step in 3..170 {
            tick(&mut world, &mut desktop, now, step);
            let c = &world.save.creatures[0];
            caught |= c.state.action == ActionKind::Dangle
                && c.state.attention.is_some_and(|p| p.hanging > 0.99);
            pulled |= caught
                && c.state.action == ActionKind::ClimbWindow
                && c.state
                    .attention
                    .is_some_and(|p| p.hanging > 0.0 && p.hanging < 0.8);
            recovered |= c.state.action == ActionKind::Greet
                && c.state.attention.is_some_and(|p| p.hanging == 0.0);
            if c.state.attention.is_some_and(|p| p.hanging > 0.0) {
                assert_eq!(c.state.surface.window_key, Some(702));
                assert_eq!(c.state.position.y, 600.0);
                assert_ne!(world.save.creatures[1].state.action, ActionKind::Greet);
            }
        }
        assert!(
            caught && pulled && recovered,
            "{caught} {pulled} {recovered}"
        );
    }

    #[test]
    fn a_watcher_can_become_a_spaced_helper_and_release_the_role_after_recovery() {
        let (mut world, mut desktop, now) = marginal_scene(true);
        let actor = world.save.creatures[0].id;
        let helper = world.save.creatures[1].id;
        let origin = world.attention.plans[&actor].origin;
        let mut offered = false;
        let mut helped = false;
        let mut colony_flinched = false;
        for step in 3..175 {
            tick(&mut world, &mut desktop, now, step);
            if let Some(WindowJourney::Gap(g)) = world.window_journeys.get_mut(&actor) {
                g.assistance_slip = false;
            }
            if world
                .attention
                .plans
                .get(&helper)
                .is_some_and(|p| matches!(p.role, Role::Helper { .. }))
            {
                offered = true;
                assert_eq!(world.attention.plans[&helper].origin, origin);
                assert!(
                    world.save.creatures[0]
                        .state
                        .position
                        .distance(world.save.creatures[1].state.position)
                        >= 30.0
                );
            }
            helped |= world
                .window_journeys
                .get(&actor)
                .is_some_and(|j| matches!(j, WindowJourney::Gap(g) if g.helped));
            // The companions who did not go to help still watch the rescue happen.
            colony_flinched |=
                world.save.creatures[2..].iter().any(|other| {
                    world.attention.plans.get(&other.id).is_some_and(
                        |p| matches!(p.role, Role::Observer { actor: a } if a == actor),
                    ) && other.state.attention.is_some_and(|pose| {
                        matches!(
                            pose.emotion,
                            AttentionEmotion::Averting
                                | AttentionEmotion::Startled
                                | AttentionEmotion::Concerned
                        )
                    })
                });
        }
        assert!(
            offered && helped && colony_flinched,
            "{offered} {helped} {colony_flinched}"
        );
        assert!(!world.attention.owns(helper));
        assert_eq!(world.save.creatures[0].state.surface.window_key, Some(702));
    }

    #[test]
    fn losing_the_handhold_cancels_the_helper_and_every_spectator() {
        let (mut world, mut desktop, now) = marginal_scene(true);
        let id = world.save.creatures[0].id;
        let mut caught = false;
        for step in 3..100 {
            tick(&mut world, &mut desktop, now, step);
            if world.save.creatures[0]
                .state
                .attention
                .is_some_and(|p| p.hanging > 0.5)
            {
                caught = true;
                desktop.windows[1].bounds.y += 60.0;
                world.drain_events().for_each(drop);
                tick(&mut world, &mut desktop, now, step + 1);
                assert!(!world.window_journeys.contains_key(&id));
                assert!(world.attention.plans.is_empty());
                assert!(!world.drain_events().any(|e| matches!(
                    e,
                    WorldEvent::ActionCompleted {
                        action: ActionKind::ClimbWindow,
                        ..
                    }
                )));
                break;
            }
        }
        assert!(caught);
    }

    #[test]
    fn failed_assistance_has_two_safe_falls_and_relief_without_false_victory() {
        let (mut world, mut desktop, now) = marginal_scene(true);
        let actor = world.save.creatures[0].id;
        let helper = world.save.creatures[1].id;
        let mut fell_together = false;
        let mut relieved = false;
        let mut colony_gasped = false;
        let mut last_y = world.save.creatures[0].state.position.y;
        for step in 3..230 {
            if let Some(WindowJourney::Gap(g)) = world.window_journeys.get_mut(&actor) {
                g.assistance_slip = true;
            }
            tick(&mut world, &mut desktop, now, step);
            let falling = world.tosses.contains_key(&actor);
            fell_together |= falling && world.tosses.contains_key(&helper);
            if falling {
                assert!((world.save.creatures[0].state.position.y - last_y).abs() < 65.0);
            }
            last_y = world.save.creatures[0].state.position.y;
            relieved |= fell_together
                && world.save.creatures[0]
                    .state
                    .attention
                    .is_some_and(|p| p.emotion == AttentionEmotion::Relieved);
            if fell_together {
                for c in &world.save.creatures[..2] {
                    if c.state.attention.is_some() {
                        assert_ne!(c.state.action, ActionKind::Greet);
                    }
                }
            }
            // Whoever was only watching sees two companions come off the edge together.
            colony_gasped |= fell_together
                && world.save.creatures[2..].iter().any(|other| {
                    other.state.attention.is_some_and(|pose| {
                        matches!(
                            pose.emotion,
                            AttentionEmotion::Startled | AttentionEmotion::Concerned
                        )
                    })
                });
        }
        assert!(fell_together && relieved && colony_gasped);
        assert!(world.tosses.is_empty());
        assert!(world.attention.plans.is_empty());
        assert!(
            world.save.creatures[0]
                .state
                .position
                .distance(world.save.creatures[1].state.position)
                >= 40.0
        );
        assert_eq!(world.save.creatures[0].memory.times_tossed, 0);
        assert_eq!(world.save.creatures[1].memory.times_tossed, 0);
    }

    #[test]
    fn pausing_a_shared_tumble_settles_both_creatures_without_replaying_it() {
        let (mut world, mut desktop, now) = marginal_scene(true);
        let actor = world.save.creatures[0].id;
        let mut paused_mid_fall = false;
        for step in 3..150 {
            if let Some(WindowJourney::Gap(g)) = world.window_journeys.get_mut(&actor) {
                g.assistance_slip = true;
            }
            tick(&mut world, &mut desktop, now, step);
            if !world.tosses.is_empty() {
                world.save.settings.paused = true;
                tick(&mut world, &mut desktop, now, step + 1);
                assert!(world.tosses.is_empty());
                assert!(world.attention.plans.is_empty());
                assert!(
                    world
                        .save
                        .creatures
                        .iter()
                        .all(|c| c.state.action != ActionKind::Tossed)
                );
                paused_mid_fall = true;
                break;
            }
        }
        assert!(paused_mid_fall);
    }
}
