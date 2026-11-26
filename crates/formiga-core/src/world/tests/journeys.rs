use super::*;

/// Gives `creature` a count of climbs behind it with which it makes its next one onto `window`
/// the way asked: with its back to the screen up the window's face, or side-on up the outside of
/// its side (see `climbs_away`). A test of either way's geometry asks for it, rather than leave
/// it to the roll.
fn set_to_climb(creature: &mut Creature, window: WindowKey, away: bool) {
    creature.memory.window_climbs = (0..10_000)
        .find(|climbs| {
            creature.memory.window_climbs = *climbs;
            climbs_away(creature, window) == away
        })
        .expect("a count of climbs that picks this way up");
}

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
    let mut creature = world.save.creatures[0].clone();
    set_to_climb(&mut creature, 93, false);
    let creature = &creature;
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
    // A stroke of the climb is twelve art pixels and three frames at ten a second, whoever
    // climbs: the paws are drawn that far apart, so the climb cannot go faster and keep them
    // where they grip.
    let climb_speed = climb.approach.distance(climb.climb_end) / climb.climb_duration;
    let stroke: f32 = CLIMB_FRAME_RISE.iter().map(|rise| f32::from(*rise)).sum();
    let paced =
        stroke * climb.pixel * f32::from(CLIMB_FRAMES_PER_SECOND) / f32::from(CLIMB_STROKE_FRAMES);
    assert!(
        (climb_speed - paced).abs() < paced * 0.05,
        "{climb_speed} against {paced}"
    );
    // It stops hung from the ledge at arms' length, and the mantle takes it from there: the
    // pull-up, the haul and the step in.
    let drops = mantle_frame_drop(&creature.appearance).map(f32::from);
    assert_eq!(climb.climb_end.y, bounds.y + drops[0] * climb.pixel);
    assert_eq!(climb.chest.y, bounds.y + drops[2] * climb.pixel);
    assert_eq!(climb.mantle_skip, 0);
    assert_eq!(climb.target.y, bounds.y);
    assert!((climb.mantle_duration - 1.1).abs() < 1e-5);

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
    set_to_climb(creature, 96, false);
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
/// the art, at whatever size the companion is drawn — and only crosses the edge at the top: it
/// pulls itself up from the ledge and brings its chest up to the edge straight up beside the
/// window, the push heaves it up and over onto the ledge, and it steps the rest of the way in
/// level along the ledge, a short way, not a slide.
#[test]
fn a_climb_hangs_beside_the_window_by_its_edge_and_crosses_it_on_the_push() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let pull_secs = f32::from(MANTLE_PULL_FRAMES) / f32::from(MANTLE_FRAMES_PER_SECOND);
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
        set_to_climb(&mut creature, 95, false);
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
                if into < pull_secs + haul_secs / 2.0 {
                    chest.push(step.position);
                } else if into < pull_secs + haul_secs {
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
        set_to_climb(&mut creature, 95, false);
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
    set_to_climb(&mut world.save.creatures[0], 96, false);
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

/// How far a climb has risen by the time the frame its clip shows `into` seconds in has come, in
/// art pixels: the sum of what each frame it has come to takes, counted here apart from the code
/// that paces the climb.
fn climb_frame_rise(into: f32) -> f32 {
    let frame = (into * f32::from(CLIMB_FRAMES_PER_SECOND)) as usize;
    let per_stroke = usize::from(CLIMB_STROKE_FRAMES);
    (1..=frame)
        .map(|coming| f32::from(CLIMB_FRAME_RISE[coming % per_stroke]))
        .sum()
}

/// A climb goes up a paw at a time instead of sliding up the window's side at a steady speed.
/// The art draws the paw that holds the wall lower against the body by a set number of pixels
/// from each frame to the next, so the body has to be up by exactly that many when the frame
/// comes: it is, to within a pixel, on every tick, at any size, and it is never still on the wall
/// and never slips. It stops hung under the ledge, where the mantle takes over.
#[test]
fn a_climb_goes_up_as_its_frames_come_so_its_paws_hold_where_they_grip() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    let world = World::new([33; 32], created, &desktop);
    for (lift, offset, display_scale) in [
        (120.0, -40.0, 3),
        (260.0, 30.0, 3),
        (60.0, -8.0, 3),
        (200.0, -40.0, 4),
        (200.0, -40.0, 2),
    ] {
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
        set_to_climb(&mut creature, 97, false);
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
            display_scale,
        );
        let WindowJourney::Climb(climb) = journey.clone() else {
            panic!("an upward transfer climbs");
        };
        let case = format!("{lift} up at display scale {display_scale}");
        assert_eq!(
            climb.pixel,
            f32::from(display_scale) / desktop.monitors[0].scale_factor,
            "{case}"
        );
        let height = (climb.approach.y - climb.climb_end.y) / climb.pixel;
        let climb_end_at = climb.approach_duration + climb.climb_duration;
        let mut elapsed = 0.0;
        let mut climbing: Vec<(f32, Point)> = Vec::new();
        let mut after = None;
        for _ in 0..600 {
            let step = journey.advance(0.05);
            elapsed += 0.05;
            if elapsed >= climb.approach_duration && elapsed < climb_end_at {
                assert_eq!(step.action, ActionKind::ClimbWindow, "{case}");
                assert!(step.pose.is_none(), "{case}: a mantle before the top");
                climbing.push((elapsed - climb.approach_duration, step.position));
            } else if elapsed >= climb_end_at && after.is_none() {
                after = Some(step);
            }
            if step.complete {
                break;
            }
        }
        // A ledge only a little higher than a companion hangs from is a stroke's climb.
        assert!(
            climbing.len() >= if height > 24.0 { 8 } else { 2 },
            "{case}"
        );
        let mut previous = climb.approach.y;
        for (into, at) in &climbing {
            assert!(at.y < previous, "{case}: stood still or slipped at {into}");
            assert_eq!(at.x, climb.approach.x, "{case}");
            previous = at.y;
            let risen = (climb.approach.y - at.y) / climb.pixel;
            let drawn_for = climb_frame_rise(*into).min(height);
            assert!(
                (0.0..1.0).contains(&(risen - drawn_for)),
                "{case}: {risen} pixels up at {into}, on a frame drawn for {drawn_for}"
            );
        }
        // The mantle begins hung under the ledge, where the climb left it.
        let after = after.expect("the mantle follows");
        assert!(after.pose.is_some(), "{case}");
        assert!(
            (0.0..climb.pixel).contains(&(climb.climb_end.y - after.position.y)),
            "{case}: the mantle began at {:?}, not {:?}",
            after.position,
            climb.climb_end
        );
    }
}

/// The paws drawn and the rise agree in a running colony: the climb's clip is drawn from the
/// action's own elapsed time, which the world keeps the same number as its journey's count of the
/// climb, so on every tick the body is up by what the frame showing was drawn for.
#[test]
fn a_climbing_companion_is_as_high_as_the_frame_its_clip_shows() {
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
    set_to_climb(&mut world.save.creatures[0], 98, false);
    assert!(world.begin_window_climb(id, 98, &desktop));
    let pixel = f32::from(world.save.settings.display_scale) / desktop.monitors[0].scale_factor;
    let mut ticks = 0;
    let mut last: Option<f32> = None;
    for _ in 0..400 {
        now += Duration::milliseconds(50);
        world.tick(now, 0.05, &desktop);
        let creature = world
            .save
            .creatures
            .iter()
            .find(|creature| creature.id == id)
            .expect("the climber");
        let mantling = creature
            .state
            .attention
            .is_some_and(|pose| pose.gesture == Some(Gesture::Mantle));
        if creature.state.action != ActionKind::ClimbWindow || mantling {
            if ticks > 0 {
                break;
            }
            continue;
        }
        let (elapsed, y) = (creature.state.action_elapsed, creature.state.position.y);
        if let Some(was) = last {
            assert!((elapsed - was - 0.05_f32).abs() < 1e-4);
        } else {
            assert!(
                elapsed < 0.05,
                "the clip starts with the climb, {elapsed} in"
            );
        }
        last = Some(elapsed);
        let risen = (floor - y) / pixel;
        let drawn_for = climb_frame_rise(elapsed);
        assert!(
            (0.0..1.0).contains(&(risen - drawn_for)),
            "{risen} pixels up at {elapsed}, on a frame drawn for {drawn_for}"
        );
        ticks += 1;
    }
    assert!(ticks > 20, "climbed for {ticks} ticks");
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
    set_to_climb(&mut world.save.creatures[0], 96, false);
    assert!(world.begin_window_climb(id, 96, &desktop));
    (world, desktop, now, floor)
}

/// Topping out is a mantle of its own rather than a hop's landing: from the tick the climb
/// reaches the top of the window's side until the companion stands on the ledge, it shows the
/// mantle and nothing else — never the landing a hop ends in — pulling itself up on the mantle's
/// first two frames, hauling itself over on the next two, stepping in on its last, and letting
/// go of it as it perches. It
/// faces the window it climbed onto the whole way, and with reduced motion it mantles the same.
#[test]
fn topping_out_shows_the_mantle_every_tick_and_never_a_landing() {
    for reduce_motion in [false, true] {
        let (mut world, desktop, mut now, floor) = climbing_world(reduce_motion);
        let haul_secs = f32::from(MANTLE_PULL_FRAMES + MANTLE_HAUL_FRAMES)
            / f32::from(MANTLE_FRAMES_PER_SECOND);
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
                // The pull-up and the haul are the first four frames, which bring it up to the
                // ledge, and the step in along the ledge the fifth.
                let hauling = creature.state.position.y > floor - 120.0;
                assert_eq!(hauling, into < haul_secs, "{into}");
                frames.push(frame(into).min(4));
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
        assert_eq!(frames, [0, 1, 2, 3, 4], "{reduce_motion}");
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

/// A companion that can be drawn turned picks its way up for each climb: with its back to the
/// screen a little more often than side-on, the same answer however often it is asked before the
/// climb is made, and another roll once it has been. One that cannot be drawn turned — a
/// four-pawed body, an original hopper, the fox — always climbs as it did.
#[test]
fn a_companion_picks_its_way_up_for_each_climb_and_keeps_to_it() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let (mut away, mut asked) = (0, 0);
    let mut both_ways = 0;
    for seed in 1..=12_u8 {
        let world = World::new([seed; 32], created, &desktop);
        for creature in &world.save.creatures {
            let mut creature = creature.clone();
            let mut ways = [false; 2];
            for climbs in 0..40 {
                creature.memory.window_climbs = climbs;
                for window in [7, 96, 4_021] {
                    let way = climbs_away(&creature, window);
                    assert_eq!(way, climbs_away(&creature, window), "the same climb");
                    if climbs_turned(&creature.appearance) {
                        away += u32::from(way);
                        asked += 1;
                        ways[usize::from(way)] = true;
                    } else {
                        assert!(!way, "a body nothing draws turned climbs as it did");
                    }
                }
            }
            both_ways += u32::from(ways == [true; 2]);
            if climbs_turned(&creature.appearance) {
                assert_eq!(
                    ways, [true; 2],
                    "every companion that turns climbs both ways"
                );
            }
            let mut fox = creature.clone();
            fox.appearance.echo_fox = true;
            assert!((0..40).all(|climbs| {
                fox.memory.window_climbs = climbs;
                !climbs_away(&fox, 96)
            }));
        }
    }
    assert!(
        both_ways > 0 && asked > 1_000,
        "{both_ways} companions, {asked} climbs"
    );
    let share = away as f32 / asked as f32;
    assert!(
        (0.55..0.65).contains(&share),
        "{away} of {asked} climbs were made back to the screen"
    );
}

/// A climb made with its back to the screen goes up the window's face: the companion walks in
/// under the window, goes straight up with all of it on the window, and stands up where it
/// climbed, never crossing the edge. It is turned away the whole time it is on the wall, which is
/// what the art draws it by, and side-on for a climb up the outside of the window's side.
#[test]
fn a_climb_made_back_to_the_screen_goes_up_the_windows_face() {
    for away in [true, false] {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut desktop = desktop();
        let world = World::new([41; 32], created, &desktop);
        let floor = world.save.creatures[0].state.position.y;
        let bounds = DesktopRect {
            x: 600.0,
            y: floor - 160.0,
            width: 320.0,
            height: 160.0,
        };
        desktop.windows.push(DesktopWindow {
            key: 96,
            bounds,
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
        let turner = 0;
        let id = world.save.creatures[turner].id;
        let creature = &mut world.save.creatures[turner];
        // An original blob, which is drawn turned whatever the founder of this colony is.
        creature.appearance.design = None;
        creature.appearance.sculpt = None;
        creature.appearance.echo_fox = false;
        creature.appearance.family = BodyFamily::Blob;
        assert!(climbs_turned(&creature.appearance));
        creature.state.position = Point { x: 560.0, y: floor };
        creature.state.surface = SurfaceAttachment {
            kind: SurfaceKind::ScreenFloor,
            monitor_id: 1,
            window_key: None,
            relative_x: 0.5,
        };
        set_to_climb(creature, 96, away);
        assert!(world.begin_window_climb(id, 96, &desktop));
        let pixel = f32::from(world.save.settings.display_scale) / desktop.monitors[0].scale_factor;
        let mut on_the_wall = 0;
        let mut perched = false;
        for _ in 0..400 {
            now += Duration::milliseconds(50);
            world.tick(now, 0.05, &desktop);
            let creature = &world.save.creatures[turner];
            if creature.state.action == ActionKind::ClimbWindow {
                on_the_wall += 1;
                let turn = creature.state.presence.turn;
                let x = creature.state.position.x;
                if away {
                    assert_eq!(turn, Turn::Back);
                    assert!(
                        (x - (bounds.x + CLIMB_BACK_INSET * pixel)).abs() < 0.01,
                        "it goes straight up the face, {x} against a window from {}",
                        bounds.x
                    );
                } else {
                    assert_eq!(turn, Turn::Side);
                    assert!(x < bounds.x + 18.0 + 0.01, "{x}");
                }
            }
            if creature.state.action == ActionKind::Perch {
                perched = true;
                break;
            }
        }
        assert!(
            perched && on_the_wall > 20,
            "away {away}: {on_the_wall} ticks on the wall"
        );
        let stood = world.save.creatures[turner].state.position;
        assert_eq!(stood.y, bounds.y, "away {away}");
        let inside = if away { CLIMB_BACK_INSET * pixel } else { 18.0 };
        assert!(
            (stood.x - (bounds.x + inside)).abs() < 0.01,
            "away {away}: {}",
            stood.x
        );
    }
}
