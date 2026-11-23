use super::*;

#[test]
fn a_tall_staircase_step_needs_individual_traversal_ability() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    // A 240-point tier: reachable for some creatures and out of reach for others.
    for (key, y) in [(51, 800.0), (52, 560.0)] {
        desktop.windows.push(DesktopWindow {
            key,
            bounds: DesktopRect {
                x: 200.0,
                y,
                width: 400.0,
                height: 240.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
    }
    let mut world = World::new([12; 32], created, &desktop);
    let_colony_wander(&mut world, created);
    world
        .topology
        .rebuild_if_changed(&desktop, &BTreeMap::new());
    let c = &mut world.save.creatures[0];
    c.state.surface = SurfaceAttachment {
        kind: SurfaceKind::WindowLedge,
        monitor_id: 1,
        window_key: Some(51),
        relative_x: 0.5,
    };
    c.state.position = Point { x: 400.0, y: 800.0 };
    c.state.drives = Drives::default();
    let route = |world: &World| {
        planned_window_route(
            &world.save.creatures[0],
            &desktop,
            &world.save.settings.habitat,
            &world.topology,
            None,
        )
    };
    world.save.creatures[0].personality.boldness = 1.0;
    world.save.creatures[0].personality.activity = 1.0;
    assert_eq!(route(&world).len(), 1);
    world.save.creatures[0].personality.boldness = 0.0;
    world.save.creatures[0].personality.activity = 0.0;
    assert!(
        route(&world).is_empty(),
        "a timid creature refuses the tier"
    );
    // Learned climbing and liveliness bring the same creature back over that limit.
    world.save.creatures[0].tendencies.climbing = 100.0;
    world.save.creatures[0].personality.activity = 0.5;
    assert_eq!(route(&world).len(), 1);
    // Fatigue takes it away again, without touching the route caps themselves.
    world.save.creatures[0].state.drives.sleep_pressure = 1.0;
    assert!(route(&world).is_empty());
}

#[test]
fn personality_scores_stay_bounded_and_identity_does_not_enter_them() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([19; 32], created, &desktop);
    let base = world.save.creatures[0].clone();
    for bits in 0..64_u8 {
        let c = &mut world.save.creatures[0];
        *c = base.clone();
        let bit = |index: u8| f32::from(bits >> index & 1);
        c.personality.boldness = bit(0);
        c.personality.activity = bit(1);
        c.personality.curiosity = bit(2);
        c.personality.playfulness = bit(3);
        c.state.drives.energy = bit(4);
        c.state.drives.sleep_pressure = bit(5);
        c.tendencies.climbing = if bits % 2 == 0 { -100.0 } else { 100.0 };
        let (rise, drop) = traversal_ability(c);
        assert!((120.0..=400.0).contains(&rise), "rise {rise} for {bits}");
        assert!((180.0..=440.0).contains(&drop), "drop {drop} for {bits}");
        assert!(rise.is_finite() && drop.is_finite());
        // The same traits give the same reach whatever the creature is called.
        let mut renamed = c.clone();
        renamed.id = c.id ^ 0xfeed;
        renamed.name = "Someone else".to_owned();
        assert_eq!(traversal_ability(&renamed), (rise, drop));
    }
}

#[test]
fn narrow_gap_squeeze_repairs_once_then_cancels_on_another_geometry_change() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    desktop.windows = vec![
        DesktopWindow {
            key: 301,
            bounds: DesktopRect {
                x: 100.0,
                y: 300.0,
                width: 240.0,
                height: 220.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        },
        DesktopWindow {
            key: 302,
            bounds: DesktopRect {
                x: 360.0,
                y: 310.0,
                width: 240.0,
                height: 220.0,
            },
            z_order: 1,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        },
    ];
    let mut world = World::new([103; 32], created, &desktop);
    let_colony_wander(&mut world, created);
    world.save.creatures[0].state.position = Point { x: 320.0, y: 300.0 };
    world.save.creatures[0].state.surface = SurfaceAttachment {
        kind: SurfaceKind::WindowLedge,
        monitor_id: 1,
        window_key: Some(301),
        relative_x: 0.9,
    };
    world.tick(created, 0.05, &desktop);

    let route = world.topology.plan_route(301, RoutePreferences::default());
    assert_eq!(route.len(), 1);
    assert_eq!(route[0].kind, RouteHopKind::NarrowGap);
    let creature_id = world.save.creatures[0].id;
    let journey = build_route_hop_journey(&world.save.creatures[0], route[0], &desktop);
    world.save.creatures[0].state.action = journey.initial_action();
    world.save.creatures[0].state.action_duration = f32::MAX;
    world.window_journeys.insert(creature_id, journey);
    world.window_routes.insert(
        creature_id,
        WindowRoutePlan {
            repaired: false,
            geometry_hash: world.topology.geometry_hash(),
            remaining: VecDeque::new(),
        },
    );
    world.tick(created, 0.1, &desktop);
    assert_eq!(
        world.save.creatures[0].state.action,
        ActionKind::InspectScreen
    );
    assert!(world.window_routes.contains_key(&creature_id));

    desktop.windows[1].bounds.x += 1.0;
    world.tick(created, 0.05, &desktop);
    assert!(world.window_routes[&creature_id].repaired);
    assert!(world.window_journeys[&creature_id].valid(&desktop));
    desktop.windows[1].bounds.x += 1.0;
    world.tick(created, 0.05, &desktop);
    assert!(!world.window_routes.contains_key(&creature_id));
    assert!(!world.window_journeys.contains_key(&creature_id));
    assert!(matches!(
        world.save.creatures[0].state.action,
        ActionKind::ReactToWindow | ActionKind::RideWindow
    ));
}

#[test]
fn ledge_journey_visibly_moves_before_attaching() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    let mut world = World::new([32; 32], created, &desktop);
    let_colony_wander(&mut world, created);
    let start = world.save.creatures[0].state.position;
    desktop.windows.push(DesktopWindow {
        key: 92,
        bounds: DesktopRect {
            x: start.x - 100.0,
            y: start.y - 300.0,
            width: 300.0,
            height: 220.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let (target, surface) = find_nearby_ledge(
        &world.save.creatures[0],
        &desktop,
        &world.save.settings.habitat,
        &{
            let mut topology = DesktopTopology::default();
            topology.rebuild_if_changed(&desktop, &BTreeMap::new());
            topology
        },
        world.save.settings.display_scale,
    )
    .expect("test window should expose a reachable ledge");
    let creature_id = world.save.creatures[0].id;
    world.save.creatures[0].state.action = ActionKind::Landing;
    world.window_journeys.insert(
        creature_id,
        WindowJourney::Hop(HopJourney {
            start,
            target,
            surface,
            elapsed: 0.0,
            duration: 1.0,
            lift: 0.0,
            ballistic: false,
        }),
    );
    world.tick(created, 0.4, &desktop);
    let midway = world.save.creatures[0].state.position;
    assert_ne!(midway, start);
    assert_ne!(midway, target);
    world.tick(created, 0.7, &desktop);
    let creature = &world.save.creatures[0];
    assert_eq!(creature.state.position, target);
    assert_eq!(creature.state.surface.kind, SurfaceKind::WindowLedge);
    assert_eq!(creature.state.action, ActionKind::Perch);
}

#[test]
fn upward_window_routes_stage_traverse_climb_mantle_and_perch() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    let world = World::new([33; 32], created, &desktop);
    let creature = &world.save.creatures[0];
    let start = creature.state.position;
    let bounds = DesktopRect {
        x: start.x - 110.0,
        y: start.y - 360.0,
        width: 320.0,
        height: 260.0,
    };
    desktop.windows.push(DesktopWindow {
        key: 93,
        bounds,
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let surface = SurfaceAttachment {
        kind: SurfaceKind::WindowLedge,
        monitor_id: 1,
        window_key: Some(93),
        relative_x: 0.5,
    };
    let target = Point {
        x: start.x,
        y: bounds.y,
    };
    let mut journey = build_window_journey(creature, target, surface, &desktop);
    assert_eq!(journey.initial_action(), ActionKind::Traverse);
    let WindowJourney::Climb(climb) = &journey else {
        panic!("upward transfer should climb");
    };
    let climb_speed = climb.approach.distance(climb.climb_end) / climb.climb_duration;
    assert!((44.0..=62.0).contains(&climb_speed));
    assert_eq!(climb.climb_end.y, bounds.y + MANTLE_LIFT_POINTS);
    assert_eq!(climb.target.y, bounds.y);
    assert_eq!(climb.mantle_duration, 0.7);

    let mut stages = Vec::new();
    let mut last = JourneyStep {
        position: start,
        action: ActionKind::Traverse,
        complete: false,
    };
    for _ in 0..400 {
        last = journey.advance(0.05);
        if stages.last() != Some(&last.action) {
            stages.push(last.action);
        }
        if last.complete {
            break;
        }
    }
    assert!(last.complete);
    // Walk to the track, climb it, haul straight up, then step in onto the ledge.
    assert_eq!(
        stages,
        vec![
            ActionKind::Traverse,
            ActionKind::ClimbWindow,
            ActionKind::Landing
        ]
    );
    assert_eq!(last.position.y, bounds.y);
    assert!(last.position.x == bounds.x + 18.0 || last.position.x == bounds.right() - 18.0);
}

#[test]
fn downward_window_routes_keep_the_existing_hop() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    desktop.windows.push(DesktopWindow {
        key: 94,
        bounds: DesktopRect {
            x: 300.0,
            y: 620.0,
            width: 420.0,
            height: 200.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let mut creature = World::new([34; 32], created, &desktop)
        .save
        .creatures
        .remove(0);
    creature.state.position = Point { x: 500.0, y: 280.0 };
    let target = Point { x: 510.0, y: 620.0 };
    let surface = SurfaceAttachment {
        kind: SurfaceKind::WindowLedge,
        monitor_id: 1,
        window_key: Some(94),
        relative_x: 0.5,
    };
    assert!(matches!(
        build_window_journey(&creature, target, surface, &desktop),
        WindowJourney::Hop(_)
    ));
}

#[test]
fn climb_cancels_when_its_path_leaves_the_habitat() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    let window = DesktopWindow {
        key: 96,
        bounds: DesktopRect {
            x: 320.0,
            y: 360.0,
            width: 420.0,
            height: 360.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    };
    desktop.windows.push(window.clone());
    let mut world = World::new([42; 32], created, &desktop);
    let_colony_wander(&mut world, created);
    world.save.settings.habitat.zones.push(HabitatZone {
        id: 1,
        display: DisplayKey([1; 16]),
        normalized_bounds: DesktopRect {
            x: 0.14,
            y: 0.0,
            width: 0.06,
            height: 1.0,
        },
        kind: HabitatZoneKind::Excluded,
        enabled: true,
    });
    let creature = &mut world.save.creatures[0];
    creature.state.position = Point { x: 100.0, y: 846.0 };
    creature.state.surface = SurfaceAttachment {
        kind: SurfaceKind::ScreenFloor,
        monitor_id: 1,
        window_key: None,
        relative_x: 0.07,
    };
    let surface = SurfaceAttachment {
        kind: SurfaceKind::WindowLedge,
        monitor_id: 1,
        window_key: Some(96),
        relative_x: 0.5,
    };
    let journey = build_window_journey(
        creature,
        Point {
            x: 500.0,
            y: window.bounds.y,
        },
        surface,
        &desktop,
    );
    let creature_id = creature.id;
    creature.state.action = journey.initial_action();
    creature.state.action_duration = f32::MAX;
    world.window_journeys.insert(creature_id, journey);
    for _ in 0..120 {
        world.tick(created, 0.05, &desktop);
        if !world.window_journeys.contains_key(&creature_id) {
            break;
        }
    }
    assert!(!world.window_journeys.contains_key(&creature_id));
    assert_eq!(
        world.save.creatures[0].state.action,
        ActionKind::ReactToWindow
    );
    assert!(habitat_contains(
        &world.save.settings.habitat,
        &desktop.monitors[0],
        world.save.creatures[0].state.position,
    ));
}

/// A window narrower than the clearance a ledge keeps from its own corners leaves nowhere to
/// stand. The window scanners never report one today, so only the searches themselves can keep
/// the simulation from being handed an impossible landing.
#[test]
fn a_window_too_narrow_to_stand_on_is_no_landing_at_all() {
    let policy = HabitatPolicy::default();
    let mut desktop = desktop();
    desktop.windows.push(DesktopWindow {
        key: 77,
        bounds: DesktopRect {
            x: 400.0,
            y: 500.0,
            width: 18.0,
            height: 300.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let above = Point { x: 409.0, y: 100.0 };
    let (point, surface) = find_drop_support(above, &desktop, &policy, true, 3)
        .expect("the habitat floor still catches a drop");
    assert_eq!(surface.kind, SurfaceKind::ScreenFloor);
    assert!(point.y > 500.0, "the sliver of a window caught nobody");
    assert_eq!(
        find_swept_support(
            above,
            Point { x: 409.0, y: 900.0 },
            &desktop,
            &policy,
            true,
            3
        )
        .map(|(_, surface)| surface.kind),
        Some(SurfaceKind::ScreenFloor),
        "and a toss passes straight through it too"
    );

    // Exactly wide enough for the clearance and it holds a creature again — on the single point
    // it has room for. A drop slides sideways onto that point; a toss falling past it does not.
    desktop.windows[0].bounds.width = 24.0;
    let (point, surface) =
        find_drop_support(above, &desktop, &policy, true, 3).expect("the ledge is standable now");
    assert_eq!(surface.kind, SurfaceKind::WindowLedge);
    assert_eq!(point, Point { x: 412.0, y: 500.0 });
    assert_eq!(
        find_swept_support(
            above,
            Point { x: 409.0, y: 900.0 },
            &desktop,
            &policy,
            true,
            3
        )
        .map(|(_, surface)| surface.kind),
        Some(SurfaceKind::ScreenFloor),
        "a swept arc only catches where it actually crosses"
    );
}

/// The last stretch of a climb used to happen all at once — rising and sliding inward together —
/// so a creature crossed onto the ledge diagonally and read as clipping along its edge. It pulls
/// itself straight up where its hands are first, and only then steps in.
#[test]
fn topping_out_hauls_straight_up_before_stepping_onto_the_ledge() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    desktop.windows.push(DesktopWindow {
        key: 95,
        bounds: DesktopRect {
            x: 400.0,
            y: 500.0,
            width: 360.0,
            height: 220.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let bounds = desktop.windows[desktop.windows.len() - 1].bounds;
    let world = World::new([77; 32], created, &desktop);
    let mut creature = world.save.creatures[0].clone();
    creature.state.position = Point {
        x: bounds.x + 4.0,
        y: 846.0,
    };
    let mut journey = build_window_journey(
        &creature,
        Point {
            x: bounds.x + 18.0,
            y: bounds.y,
        },
        SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: 1,
            window_key: Some(95),
            relative_x: 0.1,
        },
        &desktop,
    );
    let mut rising = Vec::new();
    let mut stepping = Vec::new();
    for _ in 0..400 {
        let step = journey.advance(0.05);
        match step.action {
            ActionKind::ClimbWindow => rising.push(step.position),
            ActionKind::Landing => stepping.push(step.position),
            _ => {}
        }
        if step.complete {
            break;
        }
    }
    let haul = rising.split_off(rising.len().saturating_sub(6));
    assert!(haul.len() >= 2 && !stepping.is_empty());
    for pair in haul.windows(2) {
        assert!(
            (pair[1].x - pair[0].x).abs() < 0.01,
            "the haul drifted sideways by {}",
            (pair[1].x - pair[0].x).abs()
        );
    }
    for pair in stepping.windows(2) {
        assert!(
            (pair[1].y - pair[0].y).abs() < 0.01,
            "the step onto the ledge rose by {}",
            (pair[1].y - pair[0].y).abs()
        );
    }
    assert_eq!(stepping.last().unwrap().y, bounds.y);
}

/// The climb sheet's climb is the colony's own: a look, the walk to the window's side, the
/// climb, the step in, and a perch on the ledge.
#[test]
fn a_climb_begun_on_demand_runs_every_stage_and_perches() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    let world = World::new([41; 32], created, &desktop);
    let floor = world.save.creatures[0].state.position.y;
    desktop.windows.push(DesktopWindow {
        key: 96,
        bounds: DesktopRect {
            x: 600.0,
            y: floor - 120.0,
            width: 320.0,
            height: 120.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let mut world = World::new([41; 32], created, &desktop);
    let mut now = created;
    for _ in 0..60 {
        now += Duration::milliseconds(50);
        world.tick(now, 0.05, &desktop);
    }
    let_colony_wander(&mut world, now);
    world.clear_attention();
    let id = world.save.creatures[0].id;
    let creature = &mut world.save.creatures[0];
    creature.state.position = Point { x: 560.0, y: floor };
    creature.state.surface = SurfaceAttachment {
        kind: SurfaceKind::ScreenFloor,
        monitor_id: 1,
        window_key: None,
        relative_x: 0.5,
    };
    assert!(!world.begin_window_climb(id, 7, &desktop), "no such window");
    assert!(world.begin_window_climb(id, 96, &desktop));
    let mut stages = vec![world.save.creatures[0].state.action];
    for _ in 0..200 {
        now += Duration::milliseconds(50);
        world.tick(now, 0.05, &desktop);
        let action = world.save.creatures[0].state.action;
        if stages.last() != Some(&action) {
            stages.push(action);
        }
        if action == ActionKind::Perch {
            break;
        }
    }
    assert_eq!(
        stages,
        vec![
            ActionKind::InspectScreen,
            ActionKind::Traverse,
            ActionKind::ClimbWindow,
            ActionKind::Landing,
            ActionKind::Perch,
        ]
    );
    let creature = &world.save.creatures[0];
    assert_eq!(creature.state.position.y, floor - 120.0);
    assert_eq!(creature.state.surface.window_key, Some(96));
}

/// Which beat of a stroke a climb is on `into` seconds in: 0 the reach, 1 the pull, 2 the
/// settle, as the art draws them at the climb's own rate.
fn climb_beat(into: f32) -> u32 {
    (into * f32::from(CLIMB_FRAMES_PER_SECOND)) as u32 % u32::from(CLIMB_STROKE_FRAMES)
}

/// A climb goes up a paw at a time instead of sliding up the window's side at a steady speed: it
/// rises mostly on the pulls and hardly at all on the reaches, yet it starts where it did, gets
/// to the top of its climb at the same moment, and never slips back down.
#[test]
fn a_climb_rises_on_its_pulls_and_keeps_its_time_and_ends() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    let world = World::new([33; 32], created, &desktop);
    for (lift, offset) in [(120.0, -40.0), (260.0, 30.0), (60.0, -8.0)] {
        let mut creature = world.save.creatures[0].clone();
        let start = creature.state.position;
        let bounds = DesktopRect {
            x: start.x + offset,
            y: start.y - lift,
            width: 320.0,
            height: lift,
        };
        desktop.windows.retain(|window| window.key != 97);
        desktop.windows.push(DesktopWindow {
            key: 97,
            bounds,
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
        creature.state.position = start;
        let mut journey = build_window_journey(
            &creature,
            Point {
                x: bounds.x + 18.0,
                y: bounds.y,
            },
            SurfaceAttachment {
                kind: SurfaceKind::WindowLedge,
                monitor_id: 1,
                window_key: Some(97),
                relative_x: 0.1,
            },
            &desktop,
        );
        let WindowJourney::Climb(climb) = journey.clone() else {
            panic!("an upward transfer climbs");
        };
        let climb_end_at = climb.approach_duration + climb.climb_duration;
        let speed = climb.approach.distance(climb.climb_end) / climb.climb_duration;
        // One tick of the steady climb this one replaces.
        let one_tick = speed * 0.05;
        let mut elapsed = 0.0;
        let mut climbing: Vec<(f32, Point)> = Vec::new();
        let mut after = None;
        for _ in 0..400 {
            let step = journey.advance(0.05);
            elapsed += 0.05;
            if elapsed >= climb.approach_duration && elapsed < climb_end_at {
                assert_eq!(step.action, ActionKind::ClimbWindow);
                climbing.push((elapsed - climb.approach_duration, step.position));
            } else if elapsed >= climb_end_at && after.is_none() {
                after = Some(step.position);
            }
            if step.complete {
                break;
            }
        }
        let (first_into, first) = climbing[0];
        let (_, last) = *climbing.last().expect("a climb takes ticks");
        // Starts where the steady climb did, a tick's rise at most from the bottom of the wall.
        assert_eq!(first.x, climb.approach.x);
        assert!(climb.approach.y - first.y <= one_tick, "{lift}: {first:?}");
        assert!(first_into < 0.05);
        // Gets to the top of the climb on the same tick as the steady climb, and not before.
        assert!(
            last.y > climb.climb_end.y,
            "{lift}: up before the climb was over"
        );
        assert!(
            after.expect("the haul follows").y <= climb.climb_end.y + 0.01,
            "{lift}: not up by the end of the climb"
        );
        // Never slips back, and never stands still on the wall.
        let mut previous = climb.approach.y;
        let mut rise = [0.0_f32; 3];
        let mut ticks = [0_u32; 3];
        let mut was = 0.0;
        for (into, at) in &climbing {
            assert!(at.y < previous, "{lift}: stood still or slipped at {into}");
            assert_eq!(at.x, climb.approach.x);
            // Each tick's rise counted to the beat in the middle of it.
            let beat = climb_beat((*into + was) / 2.0) as usize;
            rise[beat] += previous - at.y;
            ticks[beat] += 1;
            previous = at.y;
            was = *into;
        }
        let per_tick = |beat: usize| rise[beat] / ticks[beat].max(1) as f32;
        assert!(
            per_tick(1) > 2.0 * per_tick(2) && per_tick(2) > per_tick(0),
            "{lift}: the rise per tick on reach, pull, settle was {:?}",
            [per_tick(0), per_tick(1), per_tick(2)]
        );
    }
}

/// The hands drawn and the rise agree in a running colony: the climb's clip counts from the
/// action's own elapsed time, which starts the moment the journey reached the wall, so the body
/// goes up on the ticks the clip shows a pull.
#[test]
fn a_climbing_companion_rises_on_the_frames_its_clip_pulls() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    let world = World::new([41; 32], created, &desktop);
    let floor = world.save.creatures[0].state.position.y;
    desktop.windows.push(DesktopWindow {
        key: 98,
        bounds: DesktopRect {
            x: 600.0,
            y: floor - 220.0,
            width: 320.0,
            height: 220.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let mut world = World::new([41; 32], created, &desktop);
    let mut now = created;
    for _ in 0..60 {
        now += Duration::milliseconds(50);
        world.tick(now, 0.05, &desktop);
    }
    let_colony_wander(&mut world, now);
    world.clear_attention();
    let id = world.save.creatures[0].id;
    let creature = &mut world.save.creatures[0];
    creature.state.position = Point { x: 570.0, y: floor };
    creature.state.surface = SurfaceAttachment {
        kind: SurfaceKind::ScreenFloor,
        monitor_id: 1,
        window_key: None,
        relative_x: 0.5,
    };
    assert!(world.begin_window_climb(id, 98, &desktop));
    let mut rise = [0.0_f32; 3];
    let mut ticks = [0_u32; 3];
    let mut last: Option<(f32, f32)> = None;
    for _ in 0..300 {
        now += Duration::milliseconds(50);
        world.tick(now, 0.05, &desktop);
        let creature = world
            .save
            .creatures
            .iter()
            .find(|creature| creature.id == id)
            .expect("the climber");
        let climbing = creature.state.action == ActionKind::ClimbWindow
            && creature.state.position.y > floor - 220.0 + MANTLE_LIFT_POINTS + 0.01;
        if !climbing {
            if last.is_some() {
                break;
            }
            continue;
        }
        let (elapsed, y) = (creature.state.action_elapsed, creature.state.position.y);
        if let Some((was_elapsed, was_y)) = last {
            assert!((elapsed - was_elapsed - 0.05).abs() < 1e-4);
            let beat = climb_beat(elapsed - 0.025) as usize;
            rise[beat] += was_y - y;
            ticks[beat] += 1;
        } else {
            assert!(
                elapsed < 0.05,
                "the clip starts with the climb, {elapsed} in"
            );
        }
        last = Some((elapsed, y));
    }
    assert!(ticks.iter().all(|count| *count > 0), "{ticks:?}");
    let per_tick = |beat: usize| rise[beat] / ticks[beat] as f32;
    assert!(
        per_tick(1) > 2.0 * per_tick(2) && per_tick(2) > per_tick(0),
        "the rise per tick on reach, pull, settle was {:?}",
        [per_tick(0), per_tick(1), per_tick(2)]
    );
}
