use super::super::surfaces::support_below;
use super::*;

/// How a creature leaves converging window edges. Gaps are inferred only from rectangles.
#[derive(Clone, Debug)]
enum Escape {
    Walk(Point),
    Drop(Point, SurfaceAttachment),
    Brace,
}

impl World {
    /// A front window edge closing on the creature relative to its own (possibly moving) support.
    fn crowding_escape(
        &self,
        c: &Creature,
        desktop: &DesktopSnapshot,
    ) -> Option<(WindowKey, Point, Escape)> {
        let support_key = c.state.surface.window_key?;
        let support = desktop
            .windows
            .iter()
            .take(MAX_TOPOLOGY_WINDOWS)
            .find(|w| w.key == support_key && w.visible && !w.minimized)?;
        let unit = creature_unit(c, &self.save.settings, desktop);
        let head = head_point(c, &self.save.settings, desktop);
        let relative = c.state.surface.relative_x.clamp(0.05, 0.95);
        let old_support = self
            .last_windows
            .get(&support_key)
            .copied()
            .unwrap_or(support.bounds);
        let old_x = old_support.x + old_support.width * relative;
        let mut free = (support.bounds.x + 12.0, support.bounds.right() - 12.0);
        let mut closing: [Option<(WindowKey, Point)>; 2] = [None, None];
        for w in desktop
            .windows
            .iter()
            .take(MAX_TOPOLOGY_WINDOWS)
            .filter(|w| {
                // Only windows in front of the support can cover a creature standing on it.
                w.key != support_key
                    && w.visible
                    && !w.minimized
                    && w.z_order < support.z_order
                    && head.y >= w.bounds.y
                    && head.y <= w.bounds.bottom()
            })
        {
            let old = self.last_windows.get(&w.key).copied();
            if w.bounds.x >= head.x {
                free.1 = free.1.min(w.bounds.x);
                let gap = w.bounds.x - head.x;
                if gap < 70.0 * unit && old.is_some_and(|old| (old.x - old_x) - gap >= 8.0) {
                    closing[1] = Some((w.key, w.bounds.clamp(head)));
                }
            } else if w.bounds.right() <= head.x {
                free.0 = free.0.max(w.bounds.right());
                let gap = head.x - w.bounds.right();
                if gap < 70.0 * unit && old.is_some_and(|old| (old_x - old.right()) - gap >= 8.0) {
                    closing[0] = Some((w.key, w.bounds.clamp(head)));
                }
            }
        }
        let (window, target) = closing[1].or(closing[0])?;
        let margin = 24.0 * unit;
        let walk = match closing {
            [Some(_), Some(_)] => None,
            _ if self.save.settings.reduce_motion || free.1 - free.0 < margin * 2.0 => None,
            _ => {
                let direction = if closing[1].is_some() { -1.0 } else { 1.0 };
                let x = (c.state.position.x + direction * 60.0 * unit)
                    .clamp(free.0 + margin, free.1 - margin);
                ((x - c.state.position.x) * direction >= 16.0 * unit)
                    .then(|| motion::safe_goal(self, c, x, desktop))
                    .flatten()
            }
        };
        let escape = if let Some(destination) = walk {
            Escape::Walk(destination)
        } else if !self.save.settings.reduce_motion
            && let Some((landing, surface)) =
                support_below(c, c.state.position.x, desktop, &self.save.settings)
            && landing.y - c.state.position.y >= 12.0
            && desktop.monitors.iter().any(|m| {
                m.id == c.state.surface.monitor_id
                    && accessible_regions(&self.save.settings.habitat, m)
                        .iter()
                        .any(|r| r.contains(c.state.position) && r.contains(landing))
            })
        {
            Escape::Drop(landing, surface)
        } else {
            Escape::Brace
        };
        Some((window, target, escape))
    }

    pub(super) fn try_crowding_attention(&mut self, desktop: &DesktopSnapshot) -> bool {
        let candidate = self
            .save
            .creatures
            .iter()
            .filter(|c| c.state.surface.window_key.is_some() && self.attention_eligible(c, desktop))
            .find_map(|c| {
                self.crowding_escape(c, desktop)
                    .map(|escape| (c.id, escape))
            });
        let Some((id, (window, target, escape))) = candidate else {
            return false;
        };
        let c = self.save.creatures.iter().find(|c| c.id == id).unwrap();
        self.surface_memory.next_origin = self.surface_memory.next_origin.wrapping_add(1);
        let origin = Origin::Surface(self.surface_memory.next_origin);
        let mut reaction = Reaction {
            origin,
            role: Role::Actor {
                window,
                vanished: false,
                companion: None,
            },
            target,
            emotion: AttentionEmotion::Startled,
            elapsed: 0.0,
            seconds: REACTION_SECONDS,
            delay: 0.0,
            travel_elapsed: 0.0,
            walk: None,
            display_walk: None,
            surface: (c.state.surface.monitor_id, c.state.surface.window_key),
            action: ActionKind::InspectScreen,
            cue: None,
            ride: None,
        };
        match escape {
            Escape::Walk(destination) => {
                reaction.walk = Some(ShortWalk::new(destination));
            }
            Escape::Brace => {}
            Escape::Drop(landing, surface) => {
                // Both exits are closing: a short crouch, then a hop down to exposed support.
                let journey = WindowJourney::Hop(HopJourney {
                    start: c.state.position,
                    target: landing,
                    surface: surface.clone(),
                    elapsed: -0.3,
                    duration: (c.state.position.distance(landing) / 280.0).clamp(0.6, 1.6),
                });
                reaction.role = Role::Journey {
                    target_window: surface.window_key,
                    target_bounds: surface.window_key.and_then(|key| {
                        desktop
                            .windows
                            .iter()
                            .find(|w| w.key == key)
                            .map(|w| w.bounds)
                    }),
                    stage: Stage::Act,
                    hanging: 0.0,
                    rewarding: false,
                    escape: true,
                    since: 0.0,
                    caught: false,
                };
                // Look where it will land; arrival there begins the relieved recovery.
                reaction.target = landing;
                reaction.seconds = 0.3 + 1.6 + 1.8;
                reaction.action = ActionKind::Landing;
                let creature = creature_mut(&mut self.save.creatures, id).unwrap();
                creature.state.action = ActionKind::Landing;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = f32::MAX;
                creature.state.velocity = Point::default();
                self.window_journeys.insert(id, journey);
                self.window_routes.remove(&id);
            }
        }
        self.begin_attention(id, reaction);
        self.recruit_attention_observers(id, origin, desktop);
        self.attention.colony_cooldown = 10.0;
        true
    }

    pub(super) fn try_accidental_launch(&mut self, desktop: &DesktopSnapshot, dt: f32) {
        if self.save.settings.reduce_motion {
            return;
        }
        let candidate = self.attention.plans.iter().find_map(|(&id, p)| {
            if p.elapsed >= 0.15 || p.elapsed + dt < 0.1 || p.walk.is_some() {
                return None;
            }
            let Role::Actor {
                window,
                vanished: false,
                ..
            } = p.role
            else {
                return None;
            };
            let c = self.save.creatures.iter().find(|c| {
                c.id == id
                    && c.state.surface.window_key == Some(window)
                    && c.personality.boldness > 0.85
                    && c.personality.activity > 0.7
            })?;
            let motion = self.ride_memory.pose(id)?;
            if motion.velocity.distance(Point::default()) < 2400.0 {
                return None;
            }
            let vx = motion.velocity.x.clamp(-140.0, 140.0);
            let ahead = Point {
                x: c.state.position.x + vx * 0.4,
                y: c.state.position.y - 80.0,
            };
            let monitor = desktop
                .monitors
                .iter()
                .find(|m| m.id == c.state.surface.monitor_id)?;
            if !accessible_regions(&self.save.settings.habitat, monitor)
                .iter()
                .any(|r| r.contains(c.state.position) && r.contains(ahead))
            {
                return None;
            }
            Some((id, p.origin, vx))
        });
        let Some((id, origin, vx)) = candidate else {
            return;
        };
        self.attention.record_setback(id, None);
        let c = creature_mut(&mut self.save.creatures, id).unwrap();
        self.tosses.insert(
            id,
            TossState {
                elapsed: 0.0,
                bounces: 0,
                last_safe_position: c.state.position,
                last_safe_surface: c.state.surface.clone(),
            },
        );
        c.state.action = ActionKind::Tossed;
        c.state.action_elapsed = 0.0;
        c.state.velocity = Point { x: vx, y: -260.0 };
        let reaction = Reaction {
            origin,
            role: Role::Tumble { stage: Stage::Act },
            target: c.state.position,
            emotion: AttentionEmotion::Startled,
            elapsed: 0.0,
            seconds: 4.5,
            delay: 0.0,
            travel_elapsed: 0.0,
            walk: None,
            display_walk: None,
            surface: (c.state.surface.monitor_id, c.state.surface.window_key),
            action: ActionKind::Tossed,
            cue: None,
            ride: None,
        };
        self.begin_attention(id, reaction);
        for plan in self.attention.plans.values_mut() {
            if matches!(plan.role,Role::Observer {actor} if actor == id) {
                plan.seconds = 5.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn compatible_close_edges_form_a_walkable_bridge_without_a_victory_loop() {
        let (mut world, mut desktop, now) = scene();
        world.save.settings.display_scale = 2;
        world.save.creatures[0].state.position.x = 770.0;
        world.save.creatures[0].state.surface.relative_x = 0.95;
        let mut window = desktop.windows[0].clone();
        window.key = 702;
        window.bounds.x = 812.0;
        window.bounds.width = 300.0;
        desktop.windows.push(window);
        assert!(world.try_gap_attention(&desktop));
        let id = world.save.creatures[0].id;
        assert!(
            matches!(world.window_journeys.get(&id),Some(WindowJourney::Gap(g)) if g.bridge && !g.catch)
        );
        let mut crossed = false;
        for i in 1..100 {
            tick(&mut world, &mut desktop, now, i);
            let c = &world.save.creatures[0];
            crossed |= (800.0..812.0).contains(&c.state.position.x);
            assert_eq!(c.state.position.y, 600.0);
            if c.state.attention.is_some() {
                assert_ne!(c.state.action, ActionKind::Greet);
            }
        }
        assert!(crossed);
        assert_eq!(world.save.creatures[0].state.surface.window_key, Some(702));
    }

    #[test]
    fn dangling_commute_keeps_the_hand_anchor_on_the_surface_until_pull_up() {
        let (mut world, mut desktop, now) = scene();
        let id = world.save.creatures[0].id;
        world.save.settings.display_scale = 2;
        let mut started = false;
        for _ in 0..20 {
            world.clear_attention();
            let c = &mut world.save.creatures[0];
            c.state.position = Point { x: 770.0, y: 600.0 };
            c.state.surface.relative_x = 0.95;
            c.personality.boldness = 1.0;
            c.personality.window_tolerance = 1.0;
            c.state.action = ActionKind::Perch;
            world.surface_memory.inspect_in = 0.0;
            world.try_ledge_attention(&desktop);
            if world
                .attention
                .plans
                .get(&id)
                .is_some_and(|p| matches!(p.role, Role::Ledge { commute: true, .. }))
            {
                started = true;
                break;
            }
        }
        assert!(started);
        let mut travelled_underneath = false;
        let mut pulled_up = false;
        for i in 1..140 {
            tick(&mut world, &mut desktop, now, i);
            let c = &world.save.creatures[0];
            travelled_underneath |= c.state.position.x < 760.0
                && c.state.action == ActionKind::Dangle
                && c.state.attention.is_some_and(|p| p.hanging > 0.99);
            pulled_up |= travelled_underneath && c.state.action == ActionKind::ClimbWindow;
            assert_eq!(c.state.position.y, 600.0);
        }
        assert!(travelled_underneath && pulled_up);
        assert!(!world.attention.owns(id));
    }

    /// A bridge is only a bridge while both edges still line up. When the far side slides away
    /// mid-crossing the creature gives it up and ends the moment on something solid.
    #[test]
    fn a_bridge_that_stops_lining_up_is_abandoned_on_solid_footing() {
        let (mut world, mut desktop, now) = scene();
        world.save.settings.display_scale = 2;
        world.save.creatures[0].state.position.x = 770.0;
        world.save.creatures[0].state.surface.relative_x = 0.95;
        let mut window = desktop.windows[0].clone();
        window.key = 702;
        window.bounds.x = 812.0;
        window.bounds.width = 300.0;
        desktop.windows.push(window);
        assert!(world.try_gap_attention(&desktop));
        let id = world.save.creatures[0].id;
        for i in 1..16 {
            tick(&mut world, &mut desktop, now, i);
        }
        assert!(world.window_journeys.contains_key(&id));
        world.drain_events().for_each(drop);
        desktop.windows[1].bounds.x += 60.0;
        tick(&mut world, &mut desktop, now, 16);
        assert!(world.window_journeys.is_empty());
        assert!(world.attention.plans.is_empty());
        assert!(
            !world
                .drain_events()
                .any(|e| matches!(e, WorldEvent::ActionCompleted { .. }))
        );
        let c = &world.save.creatures[0];
        assert!(c.state.surface.window_key == Some(701) || c.state.surface.window_key.is_none());
        assert!(habitat_contains(
            &world.save.settings.habitat,
            &desktop.monitors[0],
            c.state.position
        ));
    }

    /// A launch is the surface's doing, not the creature's: a cautious rider holds on, reduced
    /// motion never throws anyone, and when it does happen the colony is alarmed for its friend.
    #[test]
    fn an_abrupt_surface_alarms_the_colony_and_never_throws_a_cautious_rider() {
        for reason in 0..3 {
            let (mut world, mut desktop, now) = scene();
            for (index, c) in world.save.creatures.iter_mut().enumerate() {
                c.personality.window_tolerance = 1.0;
                c.personality.boldness = if index > 0 {
                    0.0
                } else if reason == 1 {
                    0.5
                } else {
                    1.0
                };
                c.personality.activity = if index == 0 { 1.0 } else { 0.0 };
            }
            world.save.settings.reduce_motion = reason == 2;
            let id = world.save.creatures[0].id;
            desktop.windows[0].bounds.x += 125.0;
            let mut airborne = false;
            let mut startled_in_the_air = false;
            let mut relieved_for_it = false;
            for i in 1..115 {
                tick(&mut world, &mut desktop, now, i);
                let c = &world.save.creatures[0];
                let tossed = world.tosses.contains_key(&id);
                airborne |= tossed && c.state.position.y < 595.0;
                startled_in_the_air |= tossed
                    && c.state
                        .attention
                        .is_some_and(|pose| pose.emotion == AttentionEmotion::Startled);
                let head = head_point(c, &world.save.settings, &desktop);
                for other in world.save.creatures.iter().skip(1) {
                    let watching =
                        world.attention.plans.get(&other.id).is_some_and(
                            |p| matches!(p.role, Role::Observer { actor } if actor == id),
                        );
                    let Some(pose) = other.state.attention.filter(|_| watching) else {
                        continue;
                    };
                    // A fall is never a triumph, whoever is watching it.
                    assert_ne!(other.state.action, ActionKind::Greet);
                    relieved_for_it |= airborne
                        && pose.emotion == AttentionEmotion::Relieved
                        && (pose.target.x - head.x).abs() < 30.0;
                }
            }
            assert_eq!(airborne, reason == 0, "reason {reason}");
            assert_eq!(startled_in_the_air, reason == 0, "reason {reason}");
            assert_eq!(relieved_for_it, reason == 0, "reason {reason}");
            assert!(world.tosses.is_empty());
            assert_eq!(world.save.creatures[0].memory.times_tossed, 0);
        }
    }

    /// A commute under the ledge holds on to the surface it is under. If that surface moves the
    /// creature is back on its feet at once, with no half-finished climb to its name.
    #[test]
    fn a_commute_under_the_ledge_lets_go_safely_when_the_surface_moves() {
        let (mut world, mut desktop, now) = scene();
        let id = world.save.creatures[0].id;
        world.save.settings.display_scale = 2;
        let mut started = false;
        for _ in 0..20 {
            world.clear_attention();
            let c = &mut world.save.creatures[0];
            c.state.position = Point { x: 770.0, y: 600.0 };
            c.state.surface.relative_x = 0.95;
            c.personality.boldness = 1.0;
            c.personality.window_tolerance = 1.0;
            c.state.action = ActionKind::Perch;
            world.surface_memory.inspect_in = 0.0;
            world.try_ledge_attention(&desktop);
            if world
                .attention
                .plans
                .get(&id)
                .is_some_and(|p| matches!(p.role, Role::Ledge { commute: true, .. }))
            {
                started = true;
                break;
            }
        }
        assert!(started);
        let mut hung = false;
        for i in 1..140 {
            tick(&mut world, &mut desktop, now, i);
            if world.save.creatures[0]
                .state
                .attention
                .is_some_and(|p| p.hanging > 0.99)
            {
                hung = true;
                world.drain_events().for_each(drop);
                desktop.windows[0].bounds.x += 10.0;
                tick(&mut world, &mut desktop, now, i + 1);
                let c = &world.save.creatures[0];
                assert_eq!(c.state.position.y, 600.0);
                assert_ne!(c.state.action, ActionKind::Dangle);
                assert!(c.state.attention.is_none_or(|p| p.hanging == 0.0));
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
        assert!(hung);
    }

    #[test]
    fn a_converging_window_prompts_a_retreat_along_the_existing_support() {
        let (mut world, mut desktop, now) = scene();
        let c = &mut world.save.creatures[0];
        c.state.position.x = 770.0;
        c.state.surface.relative_x = 0.95;
        // The intruder is in front of the support; a window behind it could never cover the ledge.
        desktop.windows[0].z_order = 1;
        let mut intruder = desktop.windows[0].clone();
        intruder.key = 777;
        intruder.z_order = 0;
        intruder.bounds = DesktopRect {
            x: 850.0,
            y: 520.0,
            width: 300.0,
            height: 200.0,
        };
        world.last_windows.insert(intruder.key, intruder.bounds);
        intruder.bounds.x = 810.0;
        desktop.windows.push(intruder);
        assert!(world.try_crowding_attention(&desktop));
        for i in 1..100 {
            tick(&mut world, &mut desktop, now, i);
        }
        let c = &world.save.creatures[0];
        assert!(c.state.position.x < 730.0);
        assert_eq!(c.state.surface.window_key, Some(701));
    }

    /// Two front windows 80 points apart around a creature standing at x=500 on window 701.
    fn pinched_scene(reduced_motion: bool) -> (World, DesktopSnapshot, OffsetDateTime) {
        let (mut world, mut desktop, now) = scene();
        world.save.settings.reduce_motion = reduced_motion;
        for (index, c) in world.save.creatures.iter_mut().enumerate() {
            c.personality.activity = 0.7;
            if index == 0 {
                c.state.position.x = 500.0;
                c.state.surface.relative_x = 0.5;
            } else {
                c.state.surface = SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id: 1,
                    window_key: None,
                    relative_x: 0.5,
                };
                c.state.position = Point {
                    x: 540.0 + index as f32 * 80.0,
                    y: 846.0,
                };
            }
        }
        desktop.windows[0].z_order = 2;
        for (key, x, old_x) in [(776, 160.0, 140.0), (777, 540.0, 560.0)] {
            let mut w = desktop.windows[0].clone();
            w.key = key;
            w.z_order = 0;
            w.bounds = DesktopRect {
                x: old_x,
                y: 520.0,
                width: 300.0,
                height: 200.0,
            };
            world.last_windows.insert(key, w.bounds);
            w.bounds.x = x;
            desktop.windows.push(w);
        }
        (world, desktop, now)
    }

    #[test]
    fn when_both_exits_close_the_creature_hops_down_to_exposed_support_and_recovers() {
        let (mut world, mut desktop, now) = pinched_scene(false);
        let id = world.save.creatures[0].id;
        assert!(world.try_crowding_attention(&desktop));
        assert!(matches!(
            world.attention.plans[&id].role,
            Role::Journey { escape: true, .. }
        ));
        let mut startled_in_air = false;
        let mut relieved = false;
        let mut audience_reacted = false;
        for i in 1..90 {
            tick(&mut world, &mut desktop, now, i);
            let c = &world.save.creatures[0];
            startled_in_air |= world.window_journeys.contains_key(&id)
                && c.state.position.y > 600.0
                && c.state
                    .attention
                    .is_some_and(|p| p.emotion == AttentionEmotion::Startled);
            relieved |= c.state.surface.kind == SurfaceKind::ScreenFloor
                && c.state
                    .attention
                    .is_some_and(|p| p.emotion == AttentionEmotion::Relieved);
            audience_reacted |= world.save.creatures[1..].iter().any(|o| {
                o.state.attention.is_some_and(|p| {
                    matches!(
                        p.emotion,
                        AttentionEmotion::Concerned | AttentionEmotion::Averting
                    )
                })
            });
        }
        let c = &world.save.creatures[0];
        assert!(startled_in_air && relieved && audience_reacted);
        assert_eq!(c.state.surface.kind, SurfaceKind::ScreenFloor);
        assert_eq!(c.state.position, Point { x: 500.0, y: 846.0 });
        assert!(world.attention.plans.is_empty());
        assert!(world.window_journeys.is_empty());
    }

    #[test]
    fn reduced_motion_braces_in_place_instead_of_walking_or_dropping() {
        let (mut world, mut desktop, now) = pinched_scene(true);
        let id = world.save.creatures[0].id;
        assert!(world.try_crowding_attention(&desktop));
        for i in 1..30 {
            tick(&mut world, &mut desktop, now, i);
            let c = &world.save.creatures[0];
            assert_eq!(c.state.position, Point { x: 500.0, y: 600.0 });
            assert!(!world.window_journeys.contains_key(&id));
        }
    }

    #[test]
    fn a_support_carrying_the_creature_into_a_still_window_counts_as_converging() {
        let (mut world, mut desktop, now) = scene();
        world.clear_attention();
        let c = &mut world.save.creatures[0];
        c.personality.activity = 0.7;
        c.state.position.x = 770.0;
        c.state.surface.relative_x = 0.95;
        desktop.windows[0].z_order = 1;
        let mut wall = desktop.windows[0].clone();
        wall.key = 778;
        wall.z_order = 0;
        wall.bounds = DesktopRect {
            x: 850.0,
            y: 520.0,
            width: 300.0,
            height: 200.0,
        };
        desktop.windows.push(wall);
        world.last_windows = desktop.windows.iter().map(|w| (w.key, w.bounds)).collect();
        // Only the support moves (carrying its rider 40 points); the wall itself is stationary.
        desktop.windows[0].bounds.x += 40.0;
        world.save.creatures[0].state.position.x = 810.0;
        let escape = world.crowding_escape(&world.save.creatures[0], &desktop);
        assert!(matches!(escape, Some((778, _, Escape::Walk(point))) if point.x < 770.0));
        // A window behind the support cannot cover a creature standing on it.
        desktop.windows[1].z_order = 5;
        assert!(
            world
                .crowding_escape(&world.save.creatures[0], &desktop)
                .is_none()
        );
        let _ = now;
    }

    #[test]
    fn abrupt_surface_motion_can_launch_a_bold_rider_and_settle_without_a_user_toss() {
        let (mut world, mut desktop, now) = scene();
        let c = &mut world.save.creatures[0];
        c.personality.boldness = 1.0;
        c.personality.activity = 1.0;
        c.personality.window_tolerance = 1.0;
        let id = c.id;
        desktop.windows[0].bounds.x += 125.0;
        let mut airborne = false;
        for i in 1..115 {
            tick(&mut world, &mut desktop, now, i);
            airborne |=
                world.tosses.contains_key(&id) && world.save.creatures[0].state.position.y < 590.0;
        }
        assert!(airborne);
        assert!(world.tosses.is_empty());
        assert_eq!(world.save.creatures[0].memory.times_tossed, 0);
    }
}
