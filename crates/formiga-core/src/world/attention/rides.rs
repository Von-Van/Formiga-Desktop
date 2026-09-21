use super::*;

impl World {
    pub(super) fn try_jump_ship(&mut self, desktop: &DesktopSnapshot, dt: f32) {
        if self.save.settings.reduce_motion {
            return;
        }
        let candidate = self
            .attention
            .plans
            .iter()
            .filter_map(|(&id, p)| {
                if p.elapsed >= 0.65 || p.elapsed + dt < 0.6 || p.walk.is_some() || p.ride.is_none()
                {
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
                        && c.personality.boldness > 0.55
                        && c.state.drives.energy > 0.4
                })?;
                let ride = self.ride_memory.pose(id)?;
                if ride.velocity.distance(Point::default()) < 200.0 {
                    return None;
                }
                let (mut gap, decision, drop) = self.gap_candidate(c, desktop)?;
                // Leaving a moving platform needs a clear margin; there is no time to hesitate.
                if decision != gaps::GapDecision::Commit
                    || self.last_windows.get(&gap.hop.surface.window_key?)
                        != Some(&gap.target_bounds)
                {
                    return None;
                }
                // The rider has already watched the destination. Commit from its current contact;
                // a moving source is no longer a constraint once the creature is airborne.
                gap.preparation = 0.0;
                gap.runup = gap.hop.start;
                Some((id, gap, drop))
            })
            .next();
        if let Some((id, gap, drop)) = candidate {
            self.cancel_creature_attention(id);
            self.begin_gap_attention(id, gap, true, drop, desktop);
        }
    }

    pub(super) fn advance_riding_attention(&mut self, desktop: &DesktopSnapshot) {
        let changes: Vec<_> = self
            .attention
            .plans
            .iter()
            .filter_map(|(&id, plan)| {
                let Role::Actor {
                    window,
                    vanished: false,
                    ..
                } = plan.role
                else {
                    return None;
                };
                let creature = self
                    .save
                    .creatures
                    .iter()
                    .find(|c| c.id == id && c.state.surface.window_key == Some(window))?;
                let ride = self.ride_memory.pose(id);
                let walk = ride.and_then(|r| boundary_walk(self, creature, r.velocity, desktop));
                Some((id, ride, walk))
            })
            .collect();
        for (id, ride, walk) in changes {
            let plan = self.attention.plans.get_mut(&id).unwrap();
            if plan.elapsed < 1.0
                && plan
                    .ride
                    .is_some_and(|r| !matches!(r.kind, RideKind::Grip | RideKind::Dizzy))
                && ride.is_some_and(|r| matches!(r.kind, RideKind::Grip | RideKind::Stumble))
            {
                plan.ride = ride;
            }
            if let Some(walk) = walk {
                plan.walk = Some(walk);
                plan.emotion = AttentionEmotion::Startled;
            }
        }
        self.advance_shared_ride(desktop);
    }

    pub(super) fn try_dizzy_attention(&mut self, desktop: &DesktopSnapshot) -> bool {
        let id = self
            .save
            .creatures
            .iter()
            .find(|c| {
                self.ride_memory.dizzy(c.id)
                    && self.attention_eligible_with_cooldown(c, desktop, true)
            })
            .map(|c| c.id);
        let Some(id) = id else {
            return false;
        };
        let c = self.save.creatures.iter().find(|c| c.id == id).unwrap();
        let Some(window) = c.state.surface.window_key else {
            return false;
        };
        self.surface_memory.next_origin = self.surface_memory.next_origin.wrapping_add(1);
        let origin = Origin::Surface(self.surface_memory.next_origin);
        let direction = if c.state.surface.relative_x > 0.5 {
            -1.0
        } else {
            1.0
        };
        let walk = (!self.save.settings.reduce_motion)
            .then(|| motion::safe_goal(self, c, c.state.position.x + direction * 22.0, desktop))
            .flatten()
            .map(ShortWalk::new);
        let reaction = Reaction {
            origin,
            role: Role::Actor {
                window,
                vanished: false,
                companion: None,
            },
            target: c.state.position,
            emotion: AttentionEmotion::Concerned,
            elapsed: 0.0,
            seconds: REACTION_SECONDS,
            delay: 0.0,
            travel_elapsed: 0.0,
            walk,
            display_walk: None,
            surface: (c.state.surface.monitor_id, Some(window)),
            action: ActionKind::InspectScreen,
            cue: None,
            ride: Some(RidePose {
                kind: RideKind::Dizzy,
                velocity: Point::default(),
            }),
        };
        self.ride_memory.take_dizzy(id);
        self.begin_attention(id, reaction);
        self.recruit_attention_observers(id, origin, desktop);
        self.attention.colony_cooldown = 8.0;
        true
    }

    /// Where the later-drawn of two riders on one window should move to so that neither face is
    /// behind the other's body, or `None` when they already share the ledge comfortably.
    fn share_the_ledge(
        &self,
        id: CreatureId,
        other_id: CreatureId,
        position: Point,
        other: Point,
        desktop: &DesktopSnapshot,
    ) -> Option<Point> {
        let order = |wanted: CreatureId| {
            self.save
                .creatures
                .iter()
                .position(|creature| creature.id == wanted)
        };
        if order(id)? < order(other_id)? {
            return None;
        }
        let creature = self.save.creatures.iter().find(|c| c.id == id)?;
        let clear = super::super::spacing::face_clear_gap(
            creature,
            self.save.settings.display_scale,
            desktop,
        );
        if (position.x - other.x).abs() >= clear {
            return None;
        }
        let side = if position.x >= other.x { 1.0 } else { -1.0 };
        [side, -side].into_iter().find_map(|side| {
            motion::safe_goal(self, creature, other.x + side * clear * 1.05, desktop)
        })
    }

    /// Two riders on one moving window turn a steady stretch into a contest: they watch each
    /// other and show off between wobbles. The ride itself still owns their poses and contact.
    fn advance_shared_ride(&mut self, desktop: &DesktopSnapshot) {
        let riders: Vec<_> = self
            .attention
            .plans
            .iter()
            .filter_map(|(&id, plan)| {
                let Role::Actor {
                    window,
                    vanished: false,
                    ..
                } = plan.role
                else {
                    return None;
                };
                let creature = self
                    .save
                    .creatures
                    .iter()
                    .find(|c| c.id == id && c.state.surface.window_key == Some(window))?;
                let steady = matches!(
                    self.ride_memory.pose(id).map(|r| r.kind),
                    Some(RideKind::Balance | RideKind::Elevator)
                );
                Some((
                    id,
                    window,
                    plan.origin,
                    creature.state.position,
                    steady && creature.personality.window_tolerance > 0.6,
                ))
            })
            .collect();
        for &(id, window, origin, position, showing_off) in &riders {
            let Some(&(other_id, .., other_position, other_showing)) =
                riders
                    .iter()
                    .find(|(other, other_window, other_origin, ..)| {
                        *other != id && *other_window == window && *other_origin == origin
                    })
            else {
                continue;
            };
            // A window narrowing under two riders draws them together, and a ride lasts. The one
            // painted over the other moves along the ledge until both faces are clear again.
            let aside = self.share_the_ledge(id, other_id, position, other_position, desktop);
            let plan = self.attention.plans.get_mut(&id).unwrap();
            if let Some(goal) = aside {
                plan.walk = Some(ShortWalk::new(goal));
            }
            // Watch the one sharing the ride rather than the desktop behind it.
            if let Role::Actor { companion, .. } = &mut plan.role {
                *companion = Some(other_id);
            }
            if showing_off && other_showing && plan.elapsed > 1.2 {
                plan.emotion = AttentionEmotion::Enjoying;
            }
        }
    }
}

fn boundary_walk(
    world: &World,
    c: &Creature,
    velocity: Point,
    desktop: &DesktopSnapshot,
) -> Option<ShortWalk> {
    // The scramble this plans is a sideways one, so only a sideways threat warrants it. A window
    // travelling straight up carries its rider somewhere no horizontal walk can help, and that is
    // the dismount's business rather than this one.
    if world.save.settings.reduce_motion || velocity.x.abs() < 80.0 {
        return None;
    }
    let ahead = Point {
        x: c.state.position.x + velocity.x * 0.25,
        y: c.state.position.y,
    };
    // A continuous ride onto another permitted display is safe territory.
    if desktop
        .monitors
        .iter()
        .any(|m| habitat_contains(&world.save.settings.habitat, m, ahead))
    {
        return None;
    }
    let monitor = desktop
        .monitors
        .iter()
        .find(|m| m.id == c.state.surface.monitor_id)?;
    let source = desktop
        .windows
        .iter()
        .find(|w| Some(w.key) == c.state.surface.window_key)?;
    accessible_regions(&world.save.settings.habitat, monitor)
        .iter()
        .filter_map(|r| {
            if source.bounds.y < r.y || source.bounds.y > r.bottom() {
                return None;
            }
            let min = (source.bounds.x + 16.0).max(r.x + 40.0);
            let max = (source.bounds.right() - 16.0).min(r.right() - 40.0);
            if min >= max {
                return None;
            }
            let x = if velocity.x > 0.0 {
                (c.state.position.x - 70.0).clamp(min, max)
            } else {
                (c.state.position.x + 70.0).clamp(min, max)
            };
            motion::safe_goal(world, c, x, desktop)
        })
        .min_by(|a, b| {
            a.distance(c.state.position)
                .total_cmp(&b.distance(c.state.position))
        })
        .map(ShortWalk::new)
}

pub(super) fn present_ride(
    c: &Creature,
    plan: &Reaction,
    ride: RidePose,
    reduced: bool,
    walking: bool,
    action: &mut ActionKind,
    target: &mut Point,
) -> f32 {
    let elapsed = plan.elapsed - plan.delay - plan.travel_elapsed;
    if ride.kind == RideKind::Dizzy {
        target.x = c.state.position.x + (plan.elapsed * 5.0).sin() * 36.0;
        target.y = c.state.position.y - 35.0;
        if !walking {
            *action = if elapsed >= 2.15 {
                ActionKind::Perch
            } else {
                ActionKind::InspectScreen
            };
        }
        return 0.0;
    }
    if ride.kind == RideKind::Elevator {
        target.x = c.state.position.x + 10.0;
        target.y = c.state.position.y + if ride.velocity.y < 0.0 { -150.0 } else { 120.0 };
    }
    if reduced || walking || !(0.45..2.15).contains(&elapsed) {
        return 0.0;
    }
    match ride.kind {
        RideKind::Grip => {
            if elapsed < 1.55 {
                *action = ActionKind::Dangle;
                ((elapsed - 0.45) / 0.35).clamp(0.0, 1.0)
            } else {
                *action = ActionKind::ClimbWindow;
                (1.0 - (elapsed - 1.55) / 0.6).clamp(0.0, 1.0)
            }
        }
        RideKind::Stumble => {
            *action = if elapsed < 1.15 {
                ActionKind::ReactToWindow
            } else {
                ActionKind::RideWindow
            };
            0.0
        }
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::scene;
    use super::*;

    fn step(world: &mut World, desktop: &mut DesktopSnapshot, now: OffsetDateTime, millis: u64) {
        desktop.window_sample.as_mut().unwrap().monotonic_millis = millis;
        world.tick(now + Duration::milliseconds(millis as i64), 0.05, desktop);
    }

    #[test]
    fn actual_scan_motion_distinguishes_reversal_grip_and_elevator() {
        let (mut world, mut desktop, _) = scene();
        let id = world.save.creatures[0].id;
        let sample = |world: &mut World, desktop: &mut DesktopSnapshot, millis, dx, dy| {
            desktop.window_sample.as_mut().unwrap().monotonic_millis = millis;
            desktop.windows[0].bounds.x += dx;
            desktop.windows[0].bounds.y += dy;
            world
                .ride_memory
                .update(&world.save.creatures, desktop, 0.05, true);
        };
        sample(&mut world, &mut desktop, 250, 80.0, 0.0);
        assert_eq!(world.ride_memory.pose(id).unwrap().kind, RideKind::Balance);
        sample(&mut world, &mut desktop, 500, -80.0, 0.0);
        assert_eq!(world.ride_memory.pose(id).unwrap().kind, RideKind::Stumble);
        sample(&mut world, &mut desktop, 750, 180.0, 0.0);
        assert_eq!(world.ride_memory.pose(id).unwrap().kind, RideKind::Grip);
        world.save.creatures[0].personality.window_tolerance = 1.0;
        sample(&mut world, &mut desktop, 1000, 0.0, -80.0);
        assert_eq!(world.ride_memory.pose(id).unwrap().kind, RideKind::Elevator);
        let velocity = world.ride_memory.pose(id).unwrap().velocity;
        sample(&mut world, &mut desktop, 1000, 0.0, 50.0);
        assert_eq!(world.ride_memory.pose(id).unwrap().velocity, velocity);
        world
            .ride_memory
            .update(&world.save.creatures, &desktop, 0.05, false);
        assert!(world.ride_memory.pose(id).is_none());
        assert!(std::mem::size_of::<super::super::super::rides::RideMemory>() <= 1024);
    }

    #[test]
    fn grip_tracks_contact_without_restarting_then_mantles_back_to_standing() {
        let (mut world, mut desktop, now) = scene();
        let id = world.save.creatures[0].id;
        desktop.windows[0].bounds.x += 180.0;
        step(&mut world, &mut desktop, now, 250);
        let mut saw_grip = false;
        let mut saw_mantle = false;
        let mut previous_elapsed = 0.0;
        for i in 1..70 {
            if i < 16 {
                desktop.windows[0].bounds.x += 5.0;
            }
            step(&mut world, &mut desktop, now, 250 + i * 50);
            let c = &world.save.creatures[0];
            assert_eq!(c.state.position.y, desktop.windows[0].bounds.y);
            assert!(
                (c.state.position.x
                    - (desktop.windows[0].bounds.x
                        + desktop.windows[0].bounds.width * c.state.surface.relative_x))
                    .abs()
                    < 0.1
            );
            if c.state.action == ActionKind::Dangle {
                assert!(c.state.action_elapsed >= previous_elapsed);
                previous_elapsed = c.state.action_elapsed;
                saw_grip |= c.state.attention.unwrap().hanging > 0.99;
            }
            saw_mantle |= c.state.action == ActionKind::ClimbWindow
                && c.state
                    .attention
                    .is_some_and(|p| p.hanging > 0.0 && p.hanging < 1.0);
        }
        assert!(saw_grip && saw_mantle);
        assert!(!world.attention.owns(id));
        assert_eq!(world.save.creatures[0].state.action, ActionKind::Perch);
    }

    /// A window yanked back the other way costs a rider its balance for a moment. It catches
    /// itself on the same ledge it was already standing on, and the colony sees it happen.
    #[test]
    fn a_reversal_makes_a_rider_stumble_back_onto_its_feet_while_companions_watch() {
        let (mut world, mut desktop, now) = scene();
        let rider = world.save.creatures[0].id;
        desktop.windows[0].bounds.x += 120.0;
        step(&mut world, &mut desktop, now, 250);
        desktop.windows[0].bounds.x -= 120.0;
        step(&mut world, &mut desktop, now, 500);
        assert!(
            world.attention.plans[&rider]
                .ride
                .is_some_and(|r| r.kind == RideKind::Stumble)
        );
        let mut lost_its_balance = false;
        let mut back_on_its_feet = false;
        let mut watched = false;
        for i in 11..50 {
            step(&mut world, &mut desktop, now, i * 50);
            let c = &world.save.creatures[0];
            // The stumble never lets go of the surface the creature is standing on.
            assert_eq!(c.state.surface.window_key, Some(701));
            assert_eq!(c.state.position.y, desktop.windows[0].bounds.y);
            let stumbling = world
                .attention
                .plans
                .get(&rider)
                .is_some_and(|p| p.ride.is_some_and(|r| r.kind == RideKind::Stumble));
            lost_its_balance |= stumbling && c.state.action == ActionKind::ReactToWindow;
            back_on_its_feet |= lost_its_balance && c.state.action == ActionKind::RideWindow;
            let head = head_point(c, &world.save.settings, &desktop);
            watched |= lost_its_balance
                && world.save.creatures.iter().any(|other| {
                    world.attention.plans.get(&other.id).is_some_and(
                        |p| matches!(p.role, Role::Observer { actor } if actor == rider),
                    ) && other.state.attention.is_some_and(|pose| {
                        pose.target == head
                            && matches!(
                                pose.emotion,
                                AttentionEmotion::Startled | AttentionEmotion::Concerned
                            )
                    })
                });
        }
        assert!(
            lost_its_balance && back_on_its_feet && watched,
            "{lost_its_balance} {back_on_its_feet} {watched}"
        );
    }

    /// A window dragged straight up carries its rider with it. The rider looks where it is going
    /// rather than at the desktop, keeps its own edge underfoot, and settles when the ride stops.
    #[test]
    fn a_window_rising_under_a_rider_reads_as_an_elevator_it_looks_up_from() {
        let (mut world, mut desktop, now) = scene();
        let rider = world.save.creatures[0].id;
        world.save.creatures[0].personality.window_tolerance = 1.0;
        let mut looked_up = false;
        for i in 1u64..80 {
            // A modest rise: high enough to read as an elevator, not so high that the top of the
            // screen becomes the problem instead.
            if i <= 10 && i.is_multiple_of(5) {
                desktop.windows[0].bounds.y -= 60.0;
            }
            step(&mut world, &mut desktop, now, i * 50);
            let c = &world.save.creatures[0];
            assert_eq!(c.state.surface.window_key, Some(701));
            assert_eq!(c.state.position.y, desktop.windows[0].bounds.y);
            if world
                .attention
                .plans
                .get(&rider)
                .is_some_and(|p| p.ride.is_some_and(|r| r.kind == RideKind::Elevator))
                && let Some(pose) = c.state.attention
            {
                looked_up |= pose.target.y < c.state.position.y - 100.0;
            }
        }
        assert!(looked_up);
        assert!(
            !world.attention.owns(rider),
            "{:?}",
            world
                .attention
                .plans
                .get(&rider)
                .map(|p| (p.role, p.elapsed, p.seconds))
        );
        assert_eq!(world.save.creatures[0].state.action, ActionKind::Perch);
    }

    /// Leaving a moving platform needs somewhere steady to land and the nerve to go. A neighbor
    /// that is being dragged too, a cautious rider, and reduced motion all keep everyone aboard.
    #[test]
    fn a_moving_neighbor_a_cautious_rider_or_reduced_motion_stays_aboard() {
        for reason in 0..3 {
            let (mut world, mut desktop, now) = scene();
            let c = &mut world.save.creatures[0];
            c.personality.boldness = if reason == 1 { 0.5 } else { 1.0 };
            c.personality.window_tolerance = 1.0;
            c.state.surface.relative_x = 0.95;
            c.state.position.x = 770.0;
            world.save.settings.reduce_motion = reason == 2;
            let id = world.save.creatures[0].id;
            let mut target = desktop.windows[0].clone();
            target.key = 702;
            target.bounds.x = 1170.0;
            target.bounds.width = 220.0;
            desktop.windows.push(target);
            world.geometry_observer = Default::default();
            step(&mut world, &mut desktop, now, 50);
            step(&mut world, &mut desktop, now, 100);
            for millis in (150..=2250).step_by(50) {
                if millis == 250 {
                    desktop.windows[0].bounds.x += 180.0;
                }
                if millis == 500 {
                    desktop.windows[0].bounds.x += 50.0;
                }
                // The neighbor is being dragged as well: there is nothing steady to land on.
                if reason == 0 && millis >= 250 {
                    desktop.windows[1].bounds.x += 4.0;
                }
                desktop.window_sample.as_mut().unwrap().monotonic_millis = if millis < 250 {
                    100
                } else {
                    millis / 250 * 250
                };
                world.tick(now + Duration::milliseconds(millis as i64), 0.05, &desktop);
                assert!(
                    !world.window_journeys.get(&id).is_some_and(
                        |j| matches!(j, WindowJourney::Gap(g) if g.preparation == 0.0)
                    ),
                    "reason {reason} left a moving window"
                );
            }
        }
    }

    /// Carried toward the edge of the display, a rider walks back along its own window instead
    /// of riding it out of the habitat.
    #[test]
    fn a_rider_carried_toward_the_edge_of_the_display_scrambles_back_inside() {
        let (mut world, mut desktop, now) = scene();
        let rider = world.save.creatures[0].id;
        desktop.windows[0].bounds.x = 980.0;
        world.save.creatures[0].state.surface.relative_x = 0.7;
        world.save.creatures[0].state.position.x = 1_400.0;
        // Nobody else is bold enough to be thrown off this ride, which keeps the scene readable.
        for c in world.save.creatures.iter_mut().skip(1) {
            c.personality.boldness = 0.0;
        }
        let mut scrambled = false;
        let mut walked_against_the_drag = false;
        for i in 1u64..60 {
            if i.is_multiple_of(5) {
                desktop.windows[0].bounds.x += 120.0;
            }
            step(&mut world, &mut desktop, now, i * 50);
            scrambled |= world
                .attention
                .plans
                .get(&rider)
                .is_some_and(|p| p.walk.is_some() && p.emotion == AttentionEmotion::Startled);
            let c = &world.save.creatures[0];
            walked_against_the_drag |=
                c.state.action == ActionKind::ReactToWindow && c.state.velocity.x < 0.0;
            assert!(habitat_contains(
                &world.save.settings.habitat,
                &desktop.monitors[0],
                c.state.position
            ));
        }
        assert!(
            scrambled && walked_against_the_drag,
            "{scrambled} {walked_against_the_drag}"
        );
    }

    #[test]
    fn chaotic_rides_get_one_bounded_dizzy_recovery_after_stopping() {
        let (mut world, mut desktop, now) = scene();
        let id = world.save.creatures[0].id;
        let mut recoveries = 0;
        let mut was_dizzy = false;
        for i in 1..310 {
            if i <= 40 && i % 5 == 0 {
                desktop.windows[0].bounds.x += if i % 10 == 0 { -160.0 } else { 160.0 };
            }
            step(&mut world, &mut desktop, now, i * 50);
            let dizzy = world
                .attention
                .plans
                .get(&id)
                .is_some_and(|p| p.ride.is_some_and(|r| r.kind == RideKind::Dizzy));
            recoveries += usize::from(dizzy && !was_dizzy);
            was_dizzy = dizzy;
        }
        assert_eq!(recoveries, 1);
        assert!(!was_dizzy, "{:?}", world.attention.plans.get(&id));
    }

    /// Finding your feet again after a chaotic ride is worth a companion's concern. With reduced
    /// motion the creature still reorients, but it does it standing still.
    #[test]
    fn a_dizzy_creature_is_watched_and_reduced_motion_keeps_the_recovery_in_place() {
        for reduced in [false, true] {
            let (mut world, mut desktop, now) = scene();
            world.save.settings.reduce_motion = reduced;
            let id = world.save.creatures[0].id;
            let mut swayed = false;
            let mut stood_still = true;
            let mut watched = false;
            let mut place = world.save.creatures[0].state.position;
            for i in 1u64..160 {
                if i <= 40 && i.is_multiple_of(5) {
                    desktop.windows[0].bounds.x +=
                        if i.is_multiple_of(10) { -160.0 } else { 160.0 };
                }
                if i == 46 {
                    // The companions already reacted to the ride itself; let their own reaction
                    // intervals lapse so they are free to notice the wobble that follows it.
                    world.attention.cooldowns.clear();
                }
                step(&mut world, &mut desktop, now, i * 50);
                let dizzy = world
                    .attention
                    .plans
                    .get(&id)
                    .is_some_and(|p| p.ride.is_some_and(|r| r.kind == RideKind::Dizzy));
                if !dizzy {
                    place = world.save.creatures[0].state.position;
                    continue;
                }
                swayed = true;
                if reduced {
                    assert!(world.attention.plans[&id].walk.is_none());
                    stood_still &= world.save.creatures[0].state.position == place;
                }
                let head = head_point(&world.save.creatures[0], &world.save.settings, &desktop);
                watched |=
                    world.save.creatures.iter().any(|other| {
                        world.attention.plans.get(&other.id).is_some_and(
                            |p| matches!(p.role, Role::Observer { actor } if actor == id),
                        ) && other
                            .state
                            .attention
                            .is_some_and(|pose| (pose.target.x - head.x).abs() < 4.0)
                    });
            }
            assert!(swayed && watched, "reduced {reduced}: {swayed} {watched}");
            assert!(stood_still);
        }
    }

    /// Two riders showing off to each other are doing so because of the ride. When the window
    /// they share goes away the contest goes with it, and both of them land somewhere safe.
    #[test]
    fn a_shared_ride_contest_ends_with_the_window_that_carried_it() {
        let (mut world, mut desktop, now) = scene();
        for c in world.save.creatures.iter_mut().take(2) {
            c.personality.window_tolerance = 0.9;
            c.personality.boldness = 0.8;
        }
        let mut paired = false;
        let mut watched = false;
        for i in 1..40 {
            desktop.windows[0].bounds.x += 12.0;
            step(&mut world, &mut desktop, now, i * 50);
            paired |= world.save.creatures.iter().take(2).all(|c| {
                matches!(
                    world.attention.plans.get(&c.id).map(|p| p.role),
                    Some(Role::Actor {
                        companion: Some(_),
                        ..
                    })
                )
            });
            let head = head_point(&world.save.creatures[0], &world.save.settings, &desktop);
            watched |= world.save.creatures[2..].iter().any(|other| {
                other
                    .state
                    .attention
                    .is_some_and(|pose| (pose.target.x - head.x).abs() < 4.0)
            });
        }
        assert!(paired && watched, "{paired} {watched}");
        desktop.windows.clear();
        world.drain_events().for_each(drop);
        step(&mut world, &mut desktop, now, 2_000);
        assert!(!world.attention.plans.values().any(|p| matches!(
            p.role,
            Role::Actor {
                companion: Some(_),
                ..
            }
        )));
        for c in world.save.creatures.iter().take(2) {
            assert_eq!(c.state.surface.kind, SurfaceKind::ScreenFloor);
            assert!(habitat_contains(
                &world.save.settings.habitat,
                &desktop.monitors[0],
                c.state.position
            ));
        }
    }

    #[test]
    fn boundary_scramble_treats_a_connected_allowed_display_as_safe() {
        let (mut world, mut desktop, _) = scene();
        desktop.windows[0].bounds.x = 980.0;
        let c = &mut world.save.creatures[0];
        c.state.position.x = 1400.0;
        c.state.surface.relative_x = 0.7;
        let velocity = Point { x: 480.0, y: 0.0 };
        assert!(boundary_walk(&world, &world.save.creatures[0], velocity, &desktop).is_some());
        let mut second = desktop.monitors[0].clone();
        second.id = 2;
        second.display_key = DisplayKey([2; 16]);
        second.primary = false;
        second.bounds.x += 1440.0;
        second.usable_bounds.x += 1440.0;
        desktop.monitors.push(second);
        assert!(boundary_walk(&world, &world.save.creatures[0], velocity, &desktop).is_none());
        world.save.settings.reduce_motion = true;
        desktop.monitors.pop();
        assert!(boundary_walk(&world, &world.save.creatures[0], velocity, &desktop).is_none());
    }

    #[test]
    fn reduced_motion_rides_keep_a_stationary_gaze_and_no_hanging_pose() {
        let (mut world, mut desktop, now) = scene();
        world.save.settings.reduce_motion = true;
        desktop.windows[0].bounds.x += 180.0;
        for i in 1..50 {
            step(&mut world, &mut desktop, now, 250 + i * 50);
            assert!(
                world
                    .save
                    .creatures
                    .iter()
                    .all(|c| c.state.attention.is_none_or(|p| p.hanging == 0.0))
            );
            assert!(
                world
                    .save
                    .creatures
                    .iter()
                    .all(|c| c.state.action != ActionKind::Dangle)
            );
        }
    }

    /// Rides are felt by whoever is on the window, not by the first four in the colony: with only
    /// the last two of a full colony aboard, both are carried, feel the motion, and react to it.
    #[test]
    fn the_last_two_of_a_full_colony_feel_and_react_to_their_ride() {
        let (mut world, mut desktop, now) = scene();
        super::super::tests::fill_colony(&mut world, &desktop, now);
        for (index, c) in world.save.creatures.iter_mut().enumerate() {
            if index < 4 {
                c.state.surface = SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id: 1,
                    window_key: None,
                    relative_x: 0.5,
                };
                c.state.position = Point {
                    x: 900.0 + index as f32 * 90.0,
                    y: 846.0,
                };
            } else {
                let seat = (index - 4) as f32;
                c.state.surface = SurfaceAttachment {
                    kind: SurfaceKind::WindowLedge,
                    monitor_id: 1,
                    window_key: Some(701),
                    relative_x: 0.2 + seat * 0.4,
                };
                c.state.position = Point {
                    x: 320.0 + seat * 240.0,
                    y: 600.0,
                };
                c.personality.boldness = 1.0;
                c.personality.window_tolerance = 1.0;
                c.personality.curiosity = 1.0;
            }
            c.state.action = ActionKind::Idle;
            c.state.action_duration = 100.0;
        }
        let riders = [world.save.creatures[4].id, world.save.creatures[5].id];
        let (mut felt, mut reacted) = ([false; 2], [false; 2]);
        for i in 1..40 {
            desktop.windows[0].bounds.x += 12.0;
            step(&mut world, &mut desktop, now, i * 50);
            for (index, id) in riders.into_iter().enumerate() {
                felt[index] |= world.ride_memory.riding(id);
                reacted[index] |= world
                    .attention
                    .plans
                    .get(&id)
                    .is_some_and(|p| matches!(p.role, Role::Actor { .. }));
            }
            for c in &world.save.creatures[4..] {
                assert_eq!(c.state.surface.window_key, Some(701));
                assert_eq!(c.state.position.y, desktop.windows[0].bounds.y);
            }
        }
        assert_eq!((felt, reacted), ([true; 2], [true; 2]));
    }

    #[test]
    fn two_riders_on_one_window_watch_each_other_and_show_off_when_it_is_steady() {
        let (mut world, mut desktop, now) = scene();
        for c in world.save.creatures.iter_mut().take(2) {
            c.personality.window_tolerance = 0.9;
            c.personality.boldness = 0.8;
        }
        let mut watched_each_other = false;
        let mut showed_off = false;
        for i in 1..60 {
            // A long, steady drag: the kind of ride worth enjoying.
            desktop.windows[0].bounds.x += 12.0;
            step(&mut world, &mut desktop, now, i * 50);
            let (first, second) = (&world.save.creatures[0], &world.save.creatures[1]);
            if let (Some(a), Some(b)) = (first.state.attention, second.state.attention) {
                watched_each_other |= (a.target.x - second.state.position.x).abs() < 1.0
                    && (b.target.x - first.state.position.x).abs() < 1.0;
                showed_off |= a.emotion == AttentionEmotion::Enjoying
                    && b.emotion == AttentionEmotion::Enjoying;
            }
            // The ride still owns their contact with the window.
            for c in world.save.creatures.iter().take(2) {
                assert_eq!(c.state.surface.window_key, Some(701));
                assert_eq!(c.state.position.y, desktop.windows[0].bounds.y);
            }
        }
        assert!(
            watched_each_other && showed_off,
            "{watched_each_other} {showed_off}"
        );
    }

    #[test]
    fn a_confident_rider_can_dismount_onto_a_stationary_neighbor() {
        let (mut world, mut desktop, now) = scene();
        world.save.creatures[0].personality.boldness = 1.0;
        world.save.creatures[0].personality.window_tolerance = 1.0;
        world.save.creatures[0].state.surface.relative_x = 0.95;
        world.save.creatures[0].state.position.x = 770.0;
        let mut target = desktop.windows[0].clone();
        target.key = 702;
        target.bounds.x = 1170.0;
        target.bounds.width = 220.0;
        desktop.windows.push(target);
        world.geometry_observer = Default::default();
        step(&mut world, &mut desktop, now, 50);
        step(&mut world, &mut desktop, now, 100);
        let id = world.save.creatures[0].id;
        let mut departed = false;
        for millis in (150..=2250).step_by(50) {
            if millis == 250 {
                desktop.windows[0].bounds.x += 180.0;
            }
            if millis == 500 {
                desktop.windows[0].bounds.x += 50.0;
            }
            desktop.window_sample.as_mut().unwrap().monotonic_millis = if millis < 250 {
                100
            } else {
                millis / 250 * 250
            };
            world.tick(now + Duration::milliseconds(millis as i64), 0.05, &desktop);
            departed |= world
                .window_journeys
                .get(&id)
                .is_some_and(|j| matches!(j, WindowJourney::Gap(gap) if gap.preparation == 0.0));
        }
        assert!(departed);
        assert_eq!(world.save.creatures[0].state.surface.window_key, Some(702));
    }
}
