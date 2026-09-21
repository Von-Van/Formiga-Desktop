use super::*;

#[test]
fn home_appears_for_fifteen_minutes_then_observes_its_cooldown() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([52; 32], created, &desktop);
    world.tick(created, 0.05, &desktop);
    assert!(world.save.home.is_active());
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Traverse);
    assert_eq!(world.save.home.display, Some(DisplayKey([1; 16])));

    world.tick(
        created + time::Duration::minutes(14) + time::Duration::seconds(59),
        0.05,
        &desktop,
    );
    assert!(world.save.home.is_active());
    world.tick(created + time::Duration::minutes(15), 0.05, &desktop);
    assert!(!world.save.home.is_active());
    assert_eq!(
        world.save.home.last_disappeared_utc,
        Some(created + time::Duration::minutes(15))
    );

    world.tick(
        created + time::Duration::minutes(29) + time::Duration::seconds(59),
        0.05,
        &desktop,
    );
    assert!(!world.save.home.is_active());
    world.tick(created + time::Duration::minutes(30), 0.05, &desktop);
    assert!(world.save.home.is_active());
}

#[test]
fn house_spawn_preserves_position_then_walks_and_settles_at_both_corners() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
        for reduce_motion in [false, true] {
            let mut world = World::new([52; 32], created, &desktop);
            world.save.home.corner = corner;
            world.save.settings.reduce_motion = reduce_motion;
            let start = Point { x: 720.0, y: 846.0 };
            world.save.creatures[0].state.position = start;
            // The founder shares the colony house and waits out the visit on the commons.
            let (_, target) = home_resting_position(
                &world.save.home,
                0,
                1,
                &[],
                &desktop.monitors,
                &world.save.settings.habitat,
                world.save.settings.display_scale,
            )
            .unwrap();
            let anchor = resolved_home_anchor(
                &world.save.home,
                &desktop.monitors[0],
                world.save.settings.display_scale,
                &world.save.settings.habitat,
            )
            .unwrap();
            // The founder settles on the ground in front of its own house, on the village's own
            // line. Which part of that ground it is standing on at any moment is roaming's
            // business, not the layout's.
            assert!(
                (target.x - anchor.x).abs()
                    <= DwellingKind::Main.width() / 2.0
                        * f32::from(world.save.settings.display_scale),
                "{corner:?}: a resident settled away from its own frontage",
            );
            assert_eq!(target.y, anchor.y);

            world.tick(created, 0.0, &desktop);
            assert!(world.save.home.is_active());
            assert_eq!(world.save.creatures[0].state.position, start);
            assert_eq!(world.save.creatures[0].state.action, ActionKind::Traverse);
            world.tick(created, 0.05, &desktop);
            let walker = &world.save.creatures[0];
            assert!(walker.state.position.distance(start) > 0.0);
            assert!(walker.state.position.distance(start) <= 58.0 * 0.05 + 0.001);
            assert!(walker.state.position.distance(target) < start.distance(target));
            assert_eq!(walker.state.facing_right, target.x > start.x);

            // Relaunching during the approach resumes from the saved position.
            let saved_position = walker.state.position;
            world = World::from_save(world.save.clone());
            world.tick(created, 0.0, &desktop);
            assert_eq!(world.save.creatures[0].state.position, saved_position);
            finish_home_approach_still(&mut world, created, &desktop);
            assert_eq!(world.save.creatures[0].state.position, target);
            assert_eq!(world.save.creatures[0].state.velocity, Point::default());
            world.save.settings.reduce_motion = true;
            world.tick(created, 0.05, &desktop);
            assert_eq!(world.save.creatures[0].state.position, target);
            world.save.settings.reduce_motion = reduce_motion;
        }
    }
}

#[test]
fn walking_home_pauses_for_settings_and_petting_then_resumes() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([52; 32], created, &desktop);
    let start = world.save.creatures[0].state.position;
    world.save.settings.paused = true;
    world.tick(created, 0.05, &desktop);
    assert!(world.save.home.is_active());
    assert_eq!(world.save.creatures[0].state.position, start);
    world.save.settings.paused = false;
    world.tick(created, 0.05, &desktop);
    let position = world.save.creatures[0].state.position;
    let id = world.save.creatures[0].id;
    world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id: id,
            cursor: position,
        },
        &desktop,
    );
    world.tick(created, 0.05, &desktop);
    assert_eq!(world.save.creatures[0].state.position, position);
    world.handle_command(
        WorldCommand::EndInteraction {
            cursor: position,
            velocity: Point::default(),
        },
        &desktop,
    );
    world.tick(created, 0.05, &desktop);
    assert_eq!(
        world.save.creatures[0].state.action,
        ActionKind::PetReaction
    );
    assert_eq!(world.save.creatures[0].state.position, position);
    for _ in 0..30 {
        world.tick(created, 0.05, &desktop);
    }
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Traverse);
    assert_ne!(world.save.creatures[0].state.position, position);
    world.dismiss_home(created, true);
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Idle);
    assert_eq!(world.save.creatures[0].state.velocity, Point::default());
}

#[test]
fn home_descent_is_continuous_and_finishes_if_house_disappears() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    for dismiss_during_descent in [false, true] {
        let mut world = World::new([52; 32], created, &desktop);
        let start = Point { x: 720.0, y: 400.0 };
        world.save.creatures[0].state.position = start;
        world.save.creatures[0].state.surface.kind = SurfaceKind::WindowLedge;
        world.save.creatures[0].state.surface.window_key = Some(77);
        world.tick(created, 0.05, &desktop);
        let creature = &world.save.creatures[0];
        assert_eq!(creature.state.action, ActionKind::Landing);
        assert!(creature.state.position.y > start.y);
        assert!(creature.state.position.distance(start) <= 9.01);
        assert_eq!(creature.state.position.x, start.x);
        if dismiss_during_descent {
            world.dismiss_home(created, true);
        }
        for _ in 0..55 {
            world.tick(created, 0.05, &desktop);
        }
        assert_eq!(world.save.creatures[0].state.position.y, 846.0);
        assert_eq!(
            world.save.creatures[0].state.surface.kind,
            SurfaceKind::ScreenFloor
        );
        assert!(world.window_journeys.is_empty());
        if !dismiss_during_descent {
            finish_home_approach(&mut world, created, &desktop);
        }
    }
}

#[test]
fn home_walk_changes_displays_only_when_crossing_the_seam() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    let mut secondary = desktop.monitors[0].clone();
    secondary.id = 2;
    secondary.display_key = DisplayKey([2; 16]);
    secondary.primary = false;
    secondary.bounds.x = -1440.0;
    secondary.usable_bounds.x = -1440.0;
    desktop.monitors.push(secondary);
    let mut world = World::new([52; 32], created, &desktop);
    world.save.home.corner = HomeCorner::BottomLeft;
    world.save.home.display = Some(DisplayKey([1; 16]));
    world.save.creatures[0].state.position = Point {
        x: -100.0,
        y: 846.0,
    };
    world.save.creatures[0].state.surface.monitor_id = 2;
    world.tick(created, 0.05, &desktop);
    assert_eq!(world.save.creatures[0].state.surface.monitor_id, 2);
    assert!(world.save.creatures[0].state.position.x < 0.0);
    finish_home_approach(&mut world, created, &desktop);
    assert_eq!(world.save.creatures[0].state.surface.monitor_id, 1);
}

#[test]
fn home_walk_waits_at_excluded_regions_and_resumes_when_reopened() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([52; 32], created, &desktop);
    world.save.home.corner = HomeCorner::BottomLeft;
    world.save.creatures[0].state.position = Point {
        x: 1000.0,
        y: 846.0,
    };
    world.save.settings.habitat.zones.push(HabitatZone {
        id: 1,
        display: DisplayKey([1; 16]),
        normalized_bounds: DesktopRect {
            x: 0.4,
            y: 0.0,
            width: 0.2,
            height: 1.0,
        },
        kind: HabitatZoneKind::Excluded,
        enabled: true,
    });
    for _ in 0..300 {
        let previous = world.save.creatures[0].state.position;
        world.tick(created, 0.05, &desktop);
        let position = world.save.creatures[0].state.position;
        assert!(position.x >= 864.0);
        assert!(position.distance(previous) <= 2.91);
    }
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Idle);
    assert_eq!(world.save.creatures[0].state.velocity, Point::default());
    world.save.settings.habitat.zones.clear();
    finish_home_approach(&mut world, created, &desktop);
}

/// A colony of `members`, already walked home and standing perfectly still at its own doors.
pub(super) fn settled_colony(
    seed: [u8; 32],
    members: usize,
    created: OffsetDateTime,
    desktop: &DesktopSnapshot,
) -> World {
    let mut world = World::new(seed, created, desktop);
    // Give the colony its generations without waiting out the real arrival schedule.
    while world.save.creatures.len() < members {
        let generation = world.save.creatures.len() as u8;
        let mut grown = world.save.creatures[0].clone();
        grown.generation = generation;
        grown.colony_order = generation;
        grown.id = u64::from(generation) + 100;
        world.save.creatures.push(grown);
    }
    world.tick(created, 0.05, desktop);
    assert!(world.save.home.is_active());
    finish_home_approach_still(&mut world, created, desktop);
    world
}

/// The stretch of ground the village occupies: every resting spot and every belonging on it.
/// Nothing a resident does while the home is out may take it outside this.
fn village_strip(world: &World, desktop: &DesktopSnapshot) -> (f32, f32) {
    let cottages = colony_cottages(&world.save.creatures);
    let policy = &world.save.settings.habitat;
    let scale = world.save.settings.display_scale;
    let anchor =
        resolved_home_anchor(&world.save.home, &desktop.monitors[0], scale, policy).unwrap();
    let mut span = (anchor.x, anchor.x);
    let mut widen = |x: f32| {
        span.0 = span.0.min(x);
        span.1 = span.1.max(x);
    };
    let residents = world.save.creatures.len().max(1);
    for slot in 0..residents {
        let (_, point) = home_resting_position(
            &world.save.home,
            slot,
            residents,
            &cottages,
            &desktop.monitors,
            policy,
            scale,
        )
        .unwrap();
        widen(point.x);
    }
    for slot in 0..world.save.objects.objects.len() {
        if let Some((_, point)) = home_object_position(
            &world.save.home,
            slot,
            &cottages,
            &desktop.monitors,
            policy,
            scale,
        ) {
            widen(point.x);
        }
    }
    (span.0 - 0.01, span.1 + 0.01)
}

/// Gives the colony a few belongings on the strip, so an errand has somewhere to go.
fn scatter_belongings(world: &mut World, desktop: &DesktopSnapshot) {
    let monitor = &desktop.monitors[0];
    for (slot, kind) in [
        ColonyObjectKind::Toy,
        ColonyObjectKind::Cup,
        ColonyObjectKind::Pebble,
    ]
    .into_iter()
    .enumerate()
    {
        world.save.objects.objects.push(ColonyObject {
            id: 500 + slot as u64,
            kind,
            display: monitor.display_key,
            normalized_position: Point { x: 0.1, y: 0.95 },
            role: kind.default_role(),
        });
    }
}

#[test]
fn every_member_rests_beside_its_own_door_with_a_clear_face() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let monitor = &desktop.monitors[0];

    for display_scale in [1_u8, 3, 5] {
        for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
            let mut world = World::new([53; 32], created, &desktop);
            world.save.settings.display_scale = display_scale;
            world.save.home.corner = corner;
            while world.save.creatures.len() < 4 {
                let generation = world.save.creatures.len() as u8;
                let mut grown = world.save.creatures[0].clone();
                grown.generation = generation;
                grown.colony_order = generation;
                grown.id = u64::from(generation) + 100;
                world.save.creatures.push(grown);
            }
            world.tick(created, 0.05, &desktop);
            assert!(world.save.home.is_active());
            finish_home_approach_still(&mut world, created, &desktop);

            // Everybody is on the resting place the layout spreads out for its place in colony
            // order.
            let cottages = colony_cottages(&world.save.creatures);
            let mut order: Vec<_> = world
                .save
                .creatures
                .iter()
                .map(|creature| (creature.colony_order, creature.id))
                .collect();
            order.sort_unstable();
            let residents = order.len().max(1);
            for (slot, (_, creature_id)) in order.iter().enumerate() {
                let (_, spot) = home_resting_position(
                    &world.save.home,
                    slot,
                    residents,
                    &cottages,
                    &desktop.monitors,
                    &world.save.settings.habitat,
                    display_scale,
                )
                .unwrap();
                let creature = world
                    .save
                    .creatures
                    .iter()
                    .find(|creature| creature.id == *creature_id)
                    .unwrap();
                assert_eq!(creature.state.position, spot);
            }

            let creature_width =
                CREATURE_FRAME_WIDTH * f32::from(display_scale) / monitor.scale_factor.max(1.0);
            let mut xs: Vec<_> = world
                .save
                .creatures
                .iter()
                .map(|creature| creature.state.position.x)
                .collect();
            xs.sort_by(f32::total_cmp);
            for pair in xs.windows(2) {
                let gap = pair[1] - pair[0];
                assert!(
                    gap >= creature_width * REST_CLEAR_RATIO - 0.01,
                    "scale {display_scale} {corner:?}: creatures {gap} apart but draw \
                     {creature_width} wide",
                );
            }
        }
    }
}

/// What the tightening is for. A grown colony with everything it owns, on the laptop display the
/// complaint came from: the whole village — trees, houses, belongings and keepsakes — has to leave
/// most of the screen alone at every scale it can be drawn at, and the tree has to be on the far
/// side of the colony house from the cottages in either corner.
#[test]
fn a_grown_village_leaves_most_of_a_laptop_display_alone() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let monitor = &desktop.monitors[0];
    for display_scale in [2_u8, 3, 4] {
        for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
            let mut world = World::new([61; 32], created, &desktop);
            world.save.settings.display_scale = display_scale;
            world.save.home.corner = corner;
            while world.save.creatures.len() < 4 {
                let generation = world.save.creatures.len() as u8;
                let mut grown = world.save.creatures[0].clone();
                grown.generation = generation;
                grown.colony_order = generation;
                grown.id = u64::from(generation) + 100;
                world.save.creatures.push(grown);
            }
            for slot in 0..MAX_COLONY_OBJECTS {
                world.save.objects.objects.push(ColonyObject {
                    id: 600 + slot as u64,
                    kind: ColonyObjectKind::ALL[slot % ColonyObjectKind::ALL.len()],
                    display: monitor.display_key,
                    normalized_position: Point { x: 0.1, y: 0.95 },
                    role: ColonyObjectRole::Curiosity,
                });
            }
            world.tick(created, 0.05, &desktop);
            assert!(world.save.home.is_active());
            let cottages = colony_cottages(&world.save.creatures);
            let policy = &world.save.settings.habitat;
            let unit = f32::from(display_scale) / monitor.scale_factor.max(1.0);
            let span = village_span(&cottages) * unit;
            assert!(
                span <= monitor.usable_bounds.width * 0.7,
                "scale {display_scale} {corner:?}: the village covers {span} of a \
                 {}-point display",
                monitor.usable_bounds.width
            );
            let tree = |end| {
                home_tree_position(
                    &world.save.home,
                    end,
                    &cottages,
                    &desktop.monitors,
                    policy,
                    4,
                )
                .expect("a laptop display has room for both trees")
                .1
            };
            let (outward, inward) = (tree(TreeEnd::Outward), tree(TreeEnd::Inward));
            let (_, house) = home_dwelling_position(
                &world.save.home,
                0,
                &cottages,
                &desktop.monitors,
                policy,
                4,
            )
            .unwrap();
            let (_, cottage) = home_dwelling_position(
                &world.save.home,
                1,
                &cottages,
                &desktop.monitors,
                policy,
                4,
            )
            .unwrap();
            // The trees bookend the houses: one past the colony house away from the cottages, one
            // past the last of them, and the last cottage between the two.
            assert_ne!(
                (outward.x - house.x).signum(),
                (cottage.x - house.x).signum(),
                "{corner:?}: the outward tree stood among the cottages"
            );
            assert!(
                (inward.x - house.x).abs() > (cottage.x - house.x).abs(),
                "{corner:?}: the inward tree stood short of the cottages"
            );
            assert_eq!(
                (inward.x - house.x).signum(),
                (cottage.x - house.x).signum(),
                "{corner:?}: the inward tree left the cottages' end"
            );
            assert_eq!(
                (outward.y, inward.y),
                (house.y, house.y),
                "both trees stand on the village ground line"
            );
            // Every belonging is in one yard or the other, and both yards are lived in.
            let places = crate::home_object_positions(
                &world.save.home,
                &cottages,
                &desktop.monitors,
                policy,
                4,
            );
            let far = places
                .iter()
                .flatten()
                .filter(|(_, point)| (point.x - outward.x).abs() > (point.x - inward.x).abs())
                .count();
            assert_eq!(
                (places.iter().flatten().count(), far),
                (MAX_COLONY_OBJECTS, MAX_COLONY_OBJECTS / 2),
                "{corner:?}: the colony's things did not split between the two yards"
            );
        }
    }
}

#[test]
fn dragging_a_homebound_creature_dismisses_the_shelter_and_starts_cooldown() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([53; 32], created, &desktop);
    world.tick(created, 0.05, &desktop);
    let creature_id = world.save.creatures[0].id;
    let position = world.save.creatures[0].state.position;
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id,
            cursor: position,
        },
        &desktop,
    ));
    assert!(world.save.home.is_active());
    assert!(world.handle_command(
        WorldCommand::UpdateInteraction {
            cursor: Point {
                x: position.x + 12.0,
                y: position.y,
            },
            velocity: Point::default(),
        },
        &desktop,
    ));
    assert!(!world.save.home.is_active());
    assert_eq!(world.save.home.last_disappeared_utc, Some(created));
    assert!(
        world
            .drain_events()
            .any(|event| matches!(event, WorldEvent::HomeDisappeared { interrupted: true }))
    );

    assert!(world.handle_command(
        WorldCommand::EndInteraction {
            cursor: Point { x: 720.0, y: 500.0 },
            velocity: Point::default(),
        },
        &desktop,
    ));
    world.tick(created + time::Duration::minutes(14), 0.05, &desktop);
    assert!(!world.save.home.is_active());
    world.tick(created + time::Duration::minutes(15), 0.05, &desktop);
    assert!(world.save.home.is_active());
}

/// Ten minutes of village life, recorded tick by tick: what everybody was doing and where.
fn watch_the_village(
    world: &mut World,
    now: OffsetDateTime,
    desktop: &DesktopSnapshot,
    ticks: usize,
) -> Vec<Vec<(ActionKind, i32)>> {
    (0..ticks)
        .map(|_| {
            world.tick(now, 0.05, desktop);
            world
                .save
                .creatures
                .iter()
                .map(|creature| (creature.state.action, creature.state.position.x as i32))
                .collect()
        })
        .collect()
}

#[test]
fn the_village_keeps_one_quiet_moment_going_at_a_time_and_always_settles_back() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([61; 32], 4, created, &desktop);
    scatter_belongings(&mut world, &desktop);
    let strip = village_strip(&world, &desktop);
    let ground = world.save.creatures[0].state.position.y;
    // The commons is what a settled companion may stand on, rather than one spot each.
    let walk = home_commons(
        &world.save.home,
        colony_cottages(&world.save.creatures).as_slice(),
        &desktop.monitors,
        &world.save.settings.habitat,
        world.save.settings.display_scale,
    )
    .unwrap();
    let spots: BTreeMap<CreatureId, (f32, f32)> = world
        .save
        .creatures
        .iter()
        .map(|creature| (creature.id, (walk.low_x, walk.high_x)))
        .collect();

    let mut seen: Vec<ActionKind> = Vec::new();
    let mut busiest = 0;
    let mut holding: BTreeMap<CreatureId, u32> = BTreeMap::new();
    for _ in 0..12_000 {
        world.tick(created, 0.05, &desktop);
        let busy = world.home_moments.len();
        busiest = busiest.max(busy);
        for creature in &world.save.creatures {
            if !seen.contains(&creature.state.action) {
                seen.push(creature.state.action);
            }
            assert!(
                creature.state.position.x >= strip.0 && creature.state.position.x <= strip.1,
                "{:?} wandered out of the village at {}",
                creature.state.action,
                creature.state.position.x
            );
            assert_eq!(
                creature.state.position.y, ground,
                "somebody left the ground"
            );
            let run = holding.entry(creature.id).or_default();
            if creature.state.action == ActionKind::Homebound {
                *run = 0;
                let commons = spots[&creature.id];
                assert!(
                    creature.state.position.x >= commons.0 - 0.01
                        && creature.state.position.x <= commons.1 + 0.01,
                    "a resident settled off the commons at {}",
                    creature.state.position.x
                );
            } else {
                *run += 1;
                assert!(*run < 3_600, "a quiet moment never ended");
            }
        }
    }
    assert!(
        seen.len() > 2,
        "the village never did anything but stand still: {seen:?}"
    );
    assert!(
        seen.iter().all(|action| matches!(
            action,
            ActionKind::Homebound
                | ActionKind::Idle
                | ActionKind::Traverse
                | ActionKind::Eat
                | ActionKind::Drink
                | ActionKind::SoloPlay
                | ActionKind::Sleep
                | ActionKind::Greet
                | ActionKind::InspectScreen
        )),
        "a resident did something that is not a quiet moment: {seen:?}"
    );
    assert!(
        busiest <= 2,
        "{busiest} residents were busy at once; one, plus a neighbour waving back, is the rule"
    );
}

#[test]
fn a_colony_always_fidgets_the_same_way_and_two_colonies_do_not() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let run = |seed: [u8; 32]| {
        let mut world = settled_colony(seed, 3, created, &desktop);
        scatter_belongings(&mut world, &desktop);
        watch_the_village(&mut world, created, &desktop, 6_000)
    };
    let once = run([61; 32]);
    assert_eq!(once, run([61; 32]), "the same colony drifted apart");
    assert_ne!(
        once,
        run([62; 32]),
        "two different colonies kept the same routine"
    );
}

#[test]
fn nothing_starts_while_the_colony_is_still_walking_home() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([61; 32], created, &desktop);
    world.save.creatures[0].state.position = Point {
        x: 1200.0,
        y: 400.0,
    };
    for _ in 0..400 {
        world.tick(created, 0.05, &desktop);
        let creature = &world.save.creatures[0];
        assert!(
            matches!(
                creature.state.action,
                ActionKind::Traverse | ActionKind::Landing | ActionKind::Homebound
            ),
            "{:?} happened on the way home",
            creature.state.action
        );
        if creature.state.action == ActionKind::Homebound {
            return;
        }
    }
    panic!("the colony never reached its doors");
}

#[test]
fn a_still_desktop_or_a_hidden_one_leaves_the_village_perfectly_still() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    for quiet in ["reduce motion", "hidden"] {
        let mut world = settled_colony([61; 32], 4, created, &desktop);
        scatter_belongings(&mut world, &desktop);
        let spots: BTreeMap<CreatureId, Point> = world
            .save
            .creatures
            .iter()
            .map(|creature| (creature.id, creature.state.position))
            .collect();
        if quiet == "reduce motion" {
            world.save.settings.reduce_motion = true;
        } else {
            world.save.settings.visible = false;
        }
        for _ in 0..12_000 {
            world.tick(created, 0.05, &desktop);
        }
        for creature in &world.save.creatures {
            assert_eq!(
                creature.state.action,
                ActionKind::Homebound,
                "{quiet}: a resident moved anyway"
            );
            assert_eq!(creature.state.position, spots[&creature.id]);
        }
    }
}

#[test]
fn every_interruption_ends_a_quiet_moment_and_leaves_a_resident_standing() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    for interruption in ["pet", "pick up", "dismiss", "pause", "hide"] {
        let mut world = settled_colony([61; 32], 3, created, &desktop);
        let creature_id = world.save.creatures[0].id;
        let spot = world.save.creatures[0].state.position;
        assert!(world.begin_home_moment(creature_id, ActionKind::Sleep, 90.0));
        world.tick(created, 0.05, &desktop);
        assert_eq!(world.save.creatures[0].state.action, ActionKind::Sleep);

        match interruption {
            "pet" | "pick up" => {
                assert!(world.handle_command(
                    WorldCommand::BeginInteraction {
                        creature_id,
                        cursor: spot,
                    },
                    &desktop,
                ));
                if interruption == "pick up" {
                    assert!(world.handle_command(
                        WorldCommand::UpdateInteraction {
                            cursor: Point {
                                x: spot.x + 40.0,
                                y: spot.y - 40.0,
                            },
                            velocity: Point::default(),
                        },
                        &desktop,
                    ));
                    assert!(
                        !world.save.home.is_active(),
                        "a pick-up sends the house away"
                    );
                }
                assert!(world.handle_command(
                    WorldCommand::EndInteraction {
                        cursor: spot,
                        velocity: Point::default(),
                    },
                    &desktop,
                ));
            }
            "dismiss" => world.dismiss_home(created, true),
            "pause" => world.save.settings.paused = true,
            _ => world.save.settings.visible = false,
        }
        world.tick(created, 0.05, &desktop);
        // Whatever happened, the colony is left somewhere it can carry on from.
        for creature in &world.save.creatures {
            assert!(
                creature.state.action != ActionKind::Sleep,
                "{interruption}: somebody dozed straight through it"
            );
            assert!(creature.state.position.x.is_finite());
        }
        if interruption == "pause" {
            world.save.settings.paused = false;
        }
        if interruption == "hide" {
            world.save.settings.visible = true;
        }
        world.tick(created, 0.05, &desktop);
        assert!(
            world
                .save
                .creatures
                .iter()
                .all(|creature| creature.state.action != ActionKind::Sleep),
            "{interruption}: the doze came back"
        );
    }
}

#[test]
fn quiet_moments_teach_the_colony_nothing_and_write_nothing_down() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    // The same colony twice: one village left to potter about, one held perfectly still. Anything
    // the pottering one learned that the still one did not would have come from a fidget.
    let learned = |lively: bool| {
        let mut world = settled_colony([61; 32], 4, created, &desktop);
        scatter_belongings(&mut world, &desktop);
        world.save.settings.reduce_motion = !lively;
        world.drain_events().for_each(drop);
        let mut busy = false;
        for _ in 0..12_000 {
            world.tick(created, 0.05, &desktop);
            for event in world.drain_events() {
                assert!(
                    !matches!(
                        event,
                        WorldEvent::ActionCompleted { .. }
                            | WorldEvent::CreatureRested { .. }
                            | WorldEvent::SleepInterrupted { .. }
                            | WorldEvent::BondInteraction { .. }
                    ),
                    "a quiet moment reported itself as something worth learning: {event:?}"
                );
            }
            busy |= world
                .save
                .creatures
                .iter()
                .any(|creature| creature.state.action != ActionKind::Homebound);
        }
        assert_eq!(
            busy, lively,
            "the village did not behave as the test set it up to"
        );
        (
            world
                .save
                .creatures
                .iter()
                .map(|creature| (creature.tendencies, creature.memory.clone()))
                .collect::<Vec<_>>(),
            world.save.relationships.clone(),
            world.save.companion.journal.clone(),
        )
    };
    let (lively_profiles, lively_bonds, lively_journal) = learned(true);
    let (still_profiles, still_bonds, still_journal) = learned(false);
    for ((lively, memory), (still, still_memory)) in lively_profiles.iter().zip(&still_profiles) {
        assert_eq!(lively, still, "a fidget moved a learned tendency");
        assert_eq!(memory.play_sessions, still_memory.play_sessions);
        assert_eq!(
            memory.longest_sleep_seconds,
            still_memory.longest_sleep_seconds
        );
        assert_eq!(memory.sleep_interruptions, still_memory.sleep_interruptions);
        assert_eq!(memory.home_visits, still_memory.home_visits);
        assert_eq!(memory.discoveries_found, still_memory.discoveries_found);
    }
    assert_eq!(lively_bonds, still_bonds, "a fidget moved a friendship");
    assert_eq!(
        lively_journal.len(),
        still_journal.len(),
        "a fidget was written down"
    );
}

#[test]
fn petting_a_dozing_resident_is_not_a_night_of_sleep_cut_short() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([61; 32], 2, created, &desktop);
    let creature_id = world.save.creatures[0].id;
    let spot = world.save.creatures[0].state.position;
    let before = world.save.creatures[0].tendencies.sleep_security;
    assert!(world.begin_home_moment(creature_id, ActionKind::Sleep, 90.0));
    world.tick(created, 0.05, &desktop);
    world.drain_events().for_each(drop);
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id,
            cursor: spot,
        },
        &desktop,
    ));
    assert!(world.handle_command(
        WorldCommand::EndInteraction {
            cursor: spot,
            velocity: Point::default(),
        },
        &desktop,
    ));
    assert!(
        !world
            .drain_events()
            .any(|event| matches!(event, WorldEvent::SleepInterrupted { .. })),
        "a doze at home counted as a broken sleep"
    );
    assert_eq!(
        world.save.creatures[0].tendencies.sleep_security, before,
        "the creature learned to distrust its own doorstep"
    );
}

#[test]
fn an_offered_moment_is_taken_where_the_resident_rests_and_ends_there() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();

    // Taken where a companion stands, even mid-stroll: the walk between two places on the
    // commons is most of an afternoon, and an offer is not refused for bad timing.
    let mut walking = World::new([61; 32], created, &desktop);
    walking.save.creatures[0].state.position = Point {
        x: 1200.0,
        y: 846.0,
    };
    walking.tick(created, 0.05, &desktop);
    let walker = walking.save.creatures[0].id;
    assert_eq!(walking.save.creatures[0].state.action, ActionKind::Traverse);
    assert!(walking.begin_home_moment(walker, ActionKind::Eat, 6.0));
    let caught = walking.save.creatures[0].state.position;
    walking.tick(created, 0.05, &desktop);
    assert_eq!(walking.save.creatures[0].state.action, ActionKind::Eat);
    assert_eq!(
        walking.save.creatures[0].state.position, caught,
        "a companion stops where it was asked rather than finishing its walk first"
    );

    // Refused while it is still climbing down to the village.
    let mut descending = World::new([61; 32], created, &desktop);
    descending.save.creatures[0].state.position = Point {
        x: 1200.0,
        y: 400.0,
    };
    descending.tick(created, 0.05, &desktop);
    let climber = descending.save.creatures[0].id;
    assert!(!descending.begin_home_moment(climber, ActionKind::Eat, 6.0));

    let mut world = settled_colony([61; 32], 2, created, &desktop);
    let creature_id = world.save.creatures[0].id;
    let spot = world.save.creatures[0].state.position;
    assert!(world.begin_home_moment(creature_id, ActionKind::SoloPlay, 5.0));
    world.tick(created, 0.05, &desktop);
    assert_eq!(world.save.creatures[0].state.action, ActionKind::SoloPlay);
    assert_eq!(world.save.creatures[0].state.position, spot);

    // A second offer replaces the first, and neither outlasts its welcome.
    assert!(world.begin_home_moment(creature_id, ActionKind::Eat, 3.0));
    world.tick(created, 0.05, &desktop);
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Eat);
    for _ in 0..120 {
        world.tick(created, 0.05, &desktop);
    }
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Homebound);
    assert!(
        (world.save.creatures[0].state.position.x - spot.x).abs() < 4.0,
        "a moment ended somewhere other than where it was taken"
    );

    // The house going away takes the moment with it, and afterwards there is nothing to offer.
    assert!(world.begin_home_moment(creature_id, ActionKind::Drink, 30.0));
    world.tick(created, 0.05, &desktop);
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Drink);
    world.dismiss_home(created, true);
    assert_ne!(world.save.creatures[0].state.action, ActionKind::Drink);
    assert!(!world.begin_home_moment(creature_id, ActionKind::Drink, 30.0));
}

/// Living next door is not the same as choosing someone's company: the village seats everyone a
/// step apart, so an afternoon at home must not quietly turn every pair into close friends.
#[test]
fn an_afternoon_at_home_builds_no_bonds_and_spends_none_of_the_calm_minutes_already_gathered() {
    let desktop = desktop();
    let created = datetime!(2026-09-20 11:00 UTC);
    let mut world = two_creature_world([21; 32], created);
    let pair = canonical_creature_pair(world.save.creatures[0].id, world.save.creatures[1].id)
        .expect("two companions");
    // Four calm minutes gathered out on the desktop, a minute short of a bond.
    world.calm_proximity_seconds.insert(pair, 4 * 60);

    assert!(world.handle_command(WorldCommand::SendHome, &desktop));
    finish_home_approach(&mut world, created, &desktop);
    world.drain_events().for_each(drop);
    for _ in 0..12_000 {
        world.tick(created, 0.05, &desktop);
        assert!(
            !world
                .drain_events()
                .any(|event| matches!(event, WorldEvent::BondInteraction { .. })),
            "a home visit grew a bond out of where the houses stand"
        );
    }
    assert_eq!(
        world.calm_proximity_seconds.get(&pair).copied(),
        Some(4 * 60),
        "the minutes a pair had already spent together were spent or forgotten at home"
    );
}
