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
    let journey = build_route_hop_journey(
        &world.save.creatures[0],
        route[0],
        &desktop,
        &world.save.settings.habitat,
        world.save.settings.display_scale,
    );
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
    let mut journey = build_window_journey(
        creature,
        target,
        surface,
        &desktop,
        &world.save.settings.habitat,
        world.save.settings.display_scale,
    );
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
        pose: None,
    };
    let mut mantled = false;
    for _ in 0..400 {
        last = journey.advance(0.05);
        if stages.last() != Some(&last.action) {
            stages.push(last.action);
        }
        mantled |= last
            .pose
            .is_some_and(|(gesture, _)| gesture == Gesture::Mantle);
        if last.complete {
            break;
        }
    }
    assert!(last.complete);
    // Walk to the track and climb it; the haul and the step in onto the ledge are the climb's own
    // mantle, never a landing.
    assert_eq!(stages, vec![ActionKind::Traverse, ActionKind::ClimbWindow]);
    assert!(mantled);
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
    let mut journey = build_window_journey(
        &creature,
        target,
        surface,
        &desktop,
        &HabitatPolicy::default(),
        3,
    );
    assert!(matches!(journey, WindowJourney::Hop(_)));
    // A hop down still lands as a hop does, with no mantle over it.
    for _ in 0..200 {
        let step = journey.advance(0.05);
        assert_eq!(step.action, ActionKind::Landing);
        assert!(step.pose.is_none());
        if step.complete {
            break;
        }
    }
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
        &HabitatPolicy::default(),
        3,
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

/// A climb used to go up a track five points inside the window's side, so half the body or more
/// was over the window the whole way up, and its hands never met the edge. It goes up the outside
/// now, hung beside the window by the paw gripping its edge — as far out as that paw reaches in
/// the art, at whatever size the companion is drawn — and only crosses the edge at the top: its
/// chest comes up to the edge straight up beside the window, the push heaves it up and over onto
/// the ledge, and it steps the rest of the way in level along the ledge, a short way, not a slide.
#[test]
fn a_climb_hangs_beside_the_window_by_its_edge_and_crosses_it_on_the_push() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let haul_secs = f32::from(MANTLE_HAUL_FRAMES) / f32::from(MANTLE_FRAMES_PER_SECOND);
    let bounds = DesktopRect {
        x: 400.0,
        y: 500.0,
        width: 360.0,
        height: 220.0,
    };
    let mut desktop = desktop();
    desktop.windows.push(DesktopWindow {
        key: 95,
        bounds,
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let world = World::new([77; 32], created, &desktop);
    // Both sides, from beside the window and from under it, and drawn at two sizes: an art pixel
    // a point and a half (the default) and two points.
    for (start_x, display_scale, left) in [
        (bounds.x - 30.0, 3, true),
        (bounds.x + 40.0, 3, true),
        (bounds.right() + 30.0, 3, false),
        (bounds.x - 30.0, 4, true),
    ] {
        let mut creature = world.save.creatures[0].clone();
        creature.state.position = Point {
            x: start_x,
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
            &world.save.settings.habitat,
            display_scale,
        );
        let WindowJourney::Climb(climb) = journey.clone() else {
            panic!("an upward transfer climbs");
        };
        let unit = f32::from(display_scale) / desktop.monitors[0].scale_factor;
        let (edge, inward) = if left {
            (bounds.x, 1.0)
        } else {
            (bounds.right(), -1.0)
        };
        let case = format!("from {start_x} at display scale {display_scale}");
        // Beside the window, its paw on the window's side two art pixels over the edge line.
        let paw = climb.approach.x + inward * climb_grip_reach(&creature.appearance) * unit;
        assert!(
            (inward * (paw - edge) - 2.0 * unit).abs() < 1e-3,
            "{case}: the paw is {} over the edge",
            inward * (paw - edge)
        );
        assert_eq!(climb.climb_end.x, climb.approach.x, "{case}");
        let mut climbing = Vec::new();
        let mut chest = Vec::new();
        let mut push = Vec::new();
        let mut stepping = Vec::new();
        let mut elapsed = 0.0;
        for _ in 0..400 {
            let step = journey.advance(0.05);
            elapsed += 0.05;
            assert_eq!(
                journey.facing_right().is_some(),
                elapsed >= climb.approach_duration,
                "{case}: a climb faces the wall from the moment it reaches it"
            );
            if let Some(facing_right) = journey.facing_right() {
                assert_eq!(facing_right, left, "{case}: it faces the window");
            }
            if step.pose.is_some() {
                // How far into the mantle this step is, as the art counts its frames.
                let into = journey.action_elapsed();
                if into < haul_secs / 2.0 {
                    chest.push(step.position);
                } else if into < haul_secs {
                    push.push(step.position);
                } else {
                    stepping.push(step.position);
                }
            } else if step.action == ActionKind::ClimbWindow {
                climbing.push(step.position);
            }
            if step.complete {
                break;
            }
        }
        assert!(
            climbing.len() > 4 && chest.len() >= 2 && push.len() >= 2,
            "{case}"
        );
        // Up the outside, and the chest up to the edge straight up beside the window.
        for at in climbing.iter().chain(&chest) {
            assert_eq!(at.x, climb.approach.x, "{case}");
            assert!(
                inward * (at.x - edge) < 0.0,
                "{case}: over the window at {at:?}"
            );
        }
        // The push takes it over the edge, onto the ledge's height at the end of the haul.
        let pushed = *push.last().unwrap();
        assert!(
            inward * (pushed.x - edge) > 0.0,
            "{case}: not over at {pushed:?}"
        );
        for pair in push.windows(2) {
            assert!(inward * (pair[1].x - pair[0].x) >= 0.0, "{case}: slid back");
            assert!(pair[1].y <= pair[0].y, "{case}: sank on the push");
        }
        // The step in is level along the ledge, short, and ends where it stands.
        for at in &stepping {
            assert_eq!(at.y, bounds.y, "{case}: the step rose");
        }
        let stood = *stepping.last().unwrap();
        assert_eq!(stood, climb.target, "{case}");
        assert_eq!(stood.x, edge + inward * 18.0, "{case}");
        assert!(
            (stood.x - pushed.x).abs() <= 18.0 - 10.0 * unit.min(1.8) + 2.0,
            "{case}: stepped in {}",
            (stood.x - pushed.x).abs()
        );
    }
}

/// A window's side with nothing beside it — the side of the display, or of the habitat — has
/// nowhere to hang from, so a climb up it goes just inside the edge line as every climb once
/// did; and one with only a little room hangs as far out as the whole companion is still drawn.
#[test]
fn a_window_side_with_no_room_beside_it_is_climbed_just_inside() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    let world = World::new([77; 32], created, &desktop);
    let settings = &world.save.settings;
    let unit = f32::from(settings.display_scale) / desktop.monitors[0].scale_factor;
    let half = body_half_width(&desktop.monitors[0], settings.display_scale);
    for (window_x, track) in [
        // Flush with the display's side: just inside.
        (0.0, 5.0),
        // Some room, not enough for the whole companion: as far out as it is still drawn whole.
        (half + 6.0, half),
        // Plenty of room: hung all the way out.
        (
            300.0,
            300.0 - (climb_grip_reach(&world.save.creatures[0].appearance) - 2.0) * unit,
        ),
    ] {
        desktop.windows = vec![DesktopWindow {
            key: 95,
            bounds: DesktopRect {
                x: window_x,
                y: 500.0,
                width: 360.0,
                height: 220.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        }];
        let mut creature = world.save.creatures[0].clone();
        creature.state.position = Point {
            x: window_x + 20.0,
            y: 846.0,
        };
        let journey = build_window_journey(
            &creature,
            Point {
                x: window_x + 18.0,
                y: 500.0,
            },
            SurfaceAttachment {
                kind: SurfaceKind::WindowLedge,
                monitor_id: 1,
                window_key: Some(95),
                relative_x: 0.1,
            },
            &desktop,
            &settings.habitat,
            settings.display_scale,
        );
        let WindowJourney::Climb(climb) = journey else {
            panic!("an upward transfer climbs");
        };
        assert!(
            (climb.approach.x - track).abs() < 1e-3,
            "a window at {window_x} is climbed at {}, not {track}",
            climb.approach.x
        );
    }
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
            &world.save.settings.habitat,
            world.save.settings.display_scale,
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

/// A world with one window 120 points above the floor and its one companion beside it, set off
/// on a climb onto it, for the mantle's tests.
fn climbing_world(reduce_motion: bool) -> (World, DesktopSnapshot, OffsetDateTime, f32) {
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
    world.save.settings.reduce_motion = reduce_motion;
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
    assert!(world.begin_window_climb(id, 96, &desktop));
    (world, desktop, now, floor)
}

/// Topping out is a mantle of its own rather than a hop's landing: from the tick the climb
/// reaches the top of the window's side until the companion stands on the ledge, it shows the
/// mantle and nothing else — never the landing a hop ends in — hauling itself up on the
/// mantle's first two frames, stepping in on its third, and letting go of it as it perches. It
/// faces the window it climbed onto the whole way, and with reduced motion it mantles the same.
#[test]
fn topping_out_shows_the_mantle_every_tick_and_never_a_landing() {
    for reduce_motion in [false, true] {
        let (mut world, desktop, mut now, floor) = climbing_world(reduce_motion);
        let haul_secs = f32::from(MANTLE_HAUL_FRAMES) / f32::from(MANTLE_FRAMES_PER_SECOND);
        let frame = |elapsed: f32| (elapsed * f32::from(MANTLE_FRAMES_PER_SECOND)) as u32;
        let mut topped_out = false;
        let mut frames = Vec::new();
        let mut perched = false;
        for _ in 0..300 {
            now += Duration::milliseconds(50);
            world.tick(now, 0.05, &desktop);
            let creature = &world.save.creatures[0];
            let mantling = world
                .window_journeys
                .get(&creature.id)
                .is_some_and(|journey| match journey {
                    WindowJourney::Climb(climb) => {
                        climb.elapsed >= climb.approach_duration + climb.climb_duration
                    }
                    _ => false,
                });
            let gesture = creature.state.attention.and_then(|pose| pose.gesture);
            assert_ne!(creature.state.action, ActionKind::Landing);
            if mantling {
                topped_out = true;
                assert_eq!(gesture, Some(Gesture::Mantle), "{reduce_motion}");
                assert_eq!(creature.state.action, ActionKind::ClimbWindow);
                assert!(
                    creature.state.facing_right,
                    "it faces the window it climbs onto"
                );
                assert_eq!(creature.state.attention.map(|pose| pose.hanging), Some(0.0));
                let into = creature.state.action_elapsed;
                // The haul is the first two frames, which bring it up to the ledge, and the step
                // in along the ledge the third.
                let hauling = creature.state.position.y > floor - 120.0;
                assert_eq!(hauling, into < haul_secs, "{into}");
                frames.push(frame(into).min(2));
            } else {
                assert_ne!(gesture, Some(Gesture::Mantle), "{reduce_motion}");
            }
            if creature.state.action == ActionKind::Perch {
                perched = true;
                // The mantle is let go of; a scene's own look (the hesitation a climb starts
                // with may still be running) is the scene's to end.
                assert_ne!(
                    creature.state.attention.and_then(|pose| pose.gesture),
                    Some(Gesture::Mantle)
                );
                if !world.attention.has_plan(creature.id) {
                    assert!(creature.state.attention.is_none());
                }
                assert_eq!(creature.state.position.y, floor - 120.0);
                break;
            }
        }
        assert!(topped_out && perched, "{reduce_motion}");
        frames.dedup();
        assert_eq!(frames, [0, 1, 2], "{reduce_motion}");
    }
}

/// A mantle cut short leaves nothing of itself behind: picked up partway through hauling itself
/// onto the ledge, a companion is just carried, and shows no part of the mantle.
#[test]
fn a_mantle_cut_short_lets_go_of_its_pose() {
    let (mut world, desktop, mut now, _) = climbing_world(false);
    let id = world.save.creatures[0].id;
    for _ in 0..300 {
        now += Duration::milliseconds(50);
        world.tick(now, 0.05, &desktop);
        if world.save.creatures[0]
            .state
            .attention
            .is_some_and(|pose| pose.gesture == Some(Gesture::Mantle))
        {
            break;
        }
    }
    assert_eq!(
        world.save.creatures[0]
            .state
            .attention
            .and_then(|pose| pose.gesture),
        Some(Gesture::Mantle)
    );
    let at = world.save.creatures[0].state.position;
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id: id,
            cursor: at,
        },
        &desktop,
    ));
    let creature = &world.save.creatures[0];
    assert!(!world.window_journeys.contains_key(&id));
    assert_ne!(
        creature.state.attention.and_then(|pose| pose.gesture),
        Some(Gesture::Mantle)
    );
}

/// A hop from standing is drawn in three parts, and the world tells the art which: it crouches
/// where it stands for a moment first, then is in the air for the whole flight, rising and then
/// coming down, and from the tick it touches down counts the time since, for the landing. The
/// action is the landing clip's from the crouch to the touchdown, as it always was.
#[test]
fn a_hop_crouches_then_flies_then_lands() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([32; 32], created, &desktop);
    let_colony_wander(&mut world, created);
    let start = world.save.creatures[0].state.position;
    let target = Point {
        x: start.x + if start.x < 700.0 { 220.0 } else { -220.0 },
        y: start.y,
    };
    let surface = world.save.creatures[0].state.surface.clone();
    let journey = build_window_journey(
        &world.save.creatures[0],
        target,
        surface,
        &desktop,
        &HabitatPolicy::default(),
        3,
    );
    assert!(
        matches!(journey, WindowJourney::Hop(_)),
        "a hop along the floor"
    );
    let id = world.save.creatures[0].id;
    world.save.creatures[0].state.action = journey.initial_action();
    world.window_journeys.insert(id, journey);

    let mut seen = Vec::new();
    for _ in 0..80 {
        world.tick(created, 0.05, &desktop);
        let state = &world.save.creatures[0].state;
        seen.push((state.position, state.action, state.presence.hop));
    }
    let crouched: Vec<_> = seen
        .iter()
        .take_while(|(_, _, hop)| matches!(hop, Some(HopMoment::Gather(_))))
        .collect();
    assert!(
        crouched.len() >= 3,
        "a crouch of {HOP_GATHER_SECS} s first: {seen:?}"
    );
    for (at, action, _) in &crouched[..3] {
        assert_eq!(*at, start, "it crouches where it stands");
        assert_eq!(*action, ActionKind::Landing);
    }
    let flight: Vec<f32> = seen
        .iter()
        .skip(crouched.len())
        .map_while(|(_, action, hop)| match hop {
            Some(HopMoment::Flight(across)) if *action == ActionKind::Landing => Some(*across),
            _ => None,
        })
        .collect();
    assert!(flight.len() >= 10, "in the air for the flight: {seen:?}");
    assert!(flight.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(flight[0] > 0.05 && flight[0] < 0.2);
    assert!(flight.iter().any(|across| *across < 0.5));
    assert!(flight.iter().any(|across| *across >= 0.5 && *across < 1.0));
    let down = crouched.len() + flight.len();
    let (at, action, hop) = seen[down];
    assert_eq!(at, target);
    assert_eq!(action, ActionKind::Idle);
    assert_eq!(hop, Some(HopMoment::Down(0.0)), "touched down");
    let Some(HopMoment::Down(since)) = seen[down + 4].2 else {
        panic!("still down: {seen:?}");
    };
    assert!((since - 0.2).abs() < 1e-4);
}

/// Picked up partway through a hop, it is in the hand, not in the air or landing.
#[test]
fn a_hop_let_go_of_by_a_pick_up_is_no_longer_a_hop() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([32; 32], created, &desktop);
    let_colony_wander(&mut world, created);
    let creature = &world.save.creatures[0];
    let target = Point {
        x: creature.state.position.x + 200.0,
        y: creature.state.position.y,
    };
    let journey = build_window_journey(
        creature,
        target,
        creature.state.surface.clone(),
        &desktop,
        &HabitatPolicy::default(),
        3,
    );
    let id = creature.id;
    world.save.creatures[0].state.action = journey.initial_action();
    world.window_journeys.insert(id, journey);
    for _ in 0..8 {
        world.tick(created, 0.05, &desktop);
    }
    assert!(matches!(
        world.save.creatures[0].state.presence.hop,
        Some(HopMoment::Flight(_))
    ));
    world.save.creatures[0].state.action = ActionKind::Dragged;
    world.window_journeys.remove(&id);
    world.tick(created, 0.05, &desktop);
    assert_eq!(world.save.creatures[0].state.presence.hop, None);
}
