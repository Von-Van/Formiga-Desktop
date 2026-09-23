use super::*;

/// Nothing the simulation keeps about the moment may grow without bound. Under a desktop that
/// keeps shoving windows about, every runtime map stays inside the colony it belongs to, and a
/// scene that is cut short leaves no plan, no route, and no reserved landing behind it.
#[test]
fn every_runtime_map_stays_inside_the_colony_and_a_cut_short_scene_leaves_nothing() {
    let created = datetime!(2026-05-01 0:00 UTC);
    let mut desktop = desktop();
    desktop.window_sample = Some(WindowSample {
        monotonic_millis: 0,
        reliable: true,
    });
    for index in 0..4 {
        desktop.windows.push(DesktopWindow {
            key: 950 + index,
            bounds: DesktopRect {
                x: 120.0 + index as f32 * 300.0,
                y: 560.0 + (index % 2) as f32 * 60.0,
                width: 260.0,
                height: 240.0,
            },
            z_order: index as u32,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
    }
    let mut world = two_creature_world([88; 32], created);
    let now = created + Duration::hours(1);
    let colony = world.save.creatures.len();
    let mut peak = (0, 0, 0, 0);
    for step in 1..=4_000 {
        // A restless desktop: something moves every half second, and every so often a scene
        // is cut short under the colony's feet.
        if step % 10 == 0 {
            let index = (step / 10 % 4) as usize;
            desktop.windows[index].bounds.x += if step % 20 == 0 { 24.0 } else { -24.0 };
        }
        if step % 400 == 0 {
            world.save.settings.paused = true;
        }
        if step % 400 == 20 {
            world.save.settings.paused = false;
        }
        desktop.window_sample.as_mut().unwrap().monotonic_millis = step as u64 * 50;
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        let (plans, cooldowns) = world.attention.held();
        peak = (
            peak.0.max(plans),
            peak.1.max(cooldowns),
            peak.2.max(world.window_routes.len()),
            peak.3.max(world.window_journeys.len()),
        );
        for map in [
            plans,
            cooldowns,
            world.window_routes.len(),
            world.window_journeys.len(),
            world.tosses.len(),
        ] {
            assert!(
                map <= colony,
                "a runtime map outgrew the colony at step {step}"
            );
        }
        assert!(
            world.save.companion.journal.len() <= MAX_JOURNAL_ENTRIES,
            "the journal outgrew its cap at step {step}"
        );
    }
    assert!(peak.0 > 0, "the colony did have scenes to clean up after");
    // Cut everything short at once, the way hiding the colony does.
    world.save.settings.visible = false;
    for step in 4_001..=4_040 {
        desktop.window_sample.as_mut().unwrap().monotonic_millis = step as u64 * 50;
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
    }
    assert_eq!(world.attention.held().0, 0, "no plan survives the cut");
    assert!(
        world.window_journeys.is_empty(),
        "and no journey does either"
    );
    assert!(
        world
            .save
            .creatures
            .iter()
            .all(|c| c.state.attention.is_none()),
        "and nobody is left holding a pose"
    );
}

#[test]
fn passive_activity_actions_have_distinct_state_outcomes() {
    let desktop = desktop();
    let creature = World::new([61; 32], datetime!(2026-01-01 0:00 UTC), &desktop)
        .save
        .creatures
        .remove(0);
    let context = BehaviorContext {
        cursor_safe: true,
        ambience: DesktopAmbience::default(),
        nearest_creature_distance: None,
        nearest_creature_position: None,
        nearest_creature_id: None,
        bond: None,
        on_window_ledge: false,
        reachable_window_ledge: false,
        window_changed_nearby: false,
        objects: ObjectUtility::default(),
        hour_utc: 12,
        home_point: None,
    };

    let mut eating = creature.clone();
    eating.state.action = ActionKind::Eat;
    eating.state.drives.energy = 0.25;
    let energy_before = eating.state.drives.energy;
    update_drives(&mut eating, 1.0);
    execute_action(&mut eating, &desktop, context, 1.0, None, None);
    assert!(eating.state.drives.energy > energy_before);

    let mut drinking = creature.clone();
    drinking.state.action = ActionKind::Drink;
    drinking.state.drives.comfort = 0.2;
    drinking.state.drives.arousal = 0.8;
    execute_action(&mut drinking, &desktop, context, 1.0, None, None);
    assert!(drinking.state.drives.comfort > 0.2);
    assert!(drinking.state.drives.arousal < 0.8);

    let mut sprinting = creature;
    sprinting.state.action = ActionKind::Sprint;
    sprinting.state.facing_right = true;
    let start_x = sprinting.state.position.x;
    let walking_speed = 24.0 + sprinting.personality.activity * 34.0;
    execute_action(&mut sprinting, &desktop, context, 0.5, None, None);
    assert!(sprinting.state.position.x - start_x > walking_speed * 0.5 * 2.0);
}

/// A creature that walks up to something and then does it where it stands — eats, drinks, plays
/// by itself, hangs off a ledge, looks something over, or holds up a find — comes to a stop the
/// way one that simply stands about does, rather than carrying the walk's speed through the whole
/// action and gliding across the desktop mid-snack.
#[test]
fn doing_something_where_it_stands_brings_a_walk_to_a_stop() {
    let desktop = desktop();
    let creature = World::new([61; 32], datetime!(2026-01-01 0:00 UTC), &desktop)
        .save
        .creatures
        .remove(0);
    let context = BehaviorContext {
        cursor_safe: true,
        ambience: DesktopAmbience::default(),
        nearest_creature_distance: None,
        nearest_creature_position: None,
        nearest_creature_id: None,
        bond: None,
        on_window_ledge: false,
        reachable_window_ledge: false,
        window_changed_nearby: false,
        objects: ObjectUtility::default(),
        hour_utc: 12,
        home_point: None,
    };
    let walking = 24.0 + creature.personality.activity * 34.0;
    let mut sliding = Vec::new();
    for action in [
        ActionKind::Eat,
        ActionKind::Drink,
        ActionKind::SoloPlay,
        ActionKind::Dangle,
        ActionKind::InspectScreen,
        ActionKind::PresentDiscovery,
    ] {
        let mut settling = creature.clone();
        settling.state.action = action;
        settling.state.velocity = Point { x: walking, y: 0.0 };
        let start = settling.state.position.x;
        for _ in 0..40 {
            execute_action(&mut settling, &desktop, context, 0.05, None, None);
        }
        let travelled = settling.state.position.x - start;
        eprintln!("{action:?}: {travelled:.1} px in two seconds from {walking:.1} px/s");
        if travelled >= 10.0 || settling.state.velocity.x.abs() >= 0.5 {
            sliding.push((action, travelled));
        }
    }
    assert!(sliding.is_empty(), "still sliding: {sliding:?}");
}

#[test]
fn stale_monitor_ids_rebind_all_arrived_creatures_instead_of_hiding_them() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([97; 32], created);
    for creature in &mut world.save.creatures {
        creature.state.surface.monitor_id = u64::MAX;
    }
    keep_creatures_in_habitat(
        &mut world.save.creatures,
        &desktop,
        &world.save.settings.habitat,
        &[],
    );
    assert_eq!(world.save.creatures.len(), 2);
    assert!(
        world
            .save
            .creatures
            .iter()
            .all(|creature| creature.state.surface.monitor_id == desktop.monitors[0].id)
    );
}

/// Over half an hour on a desktop full of windows, the same colony spends more of its time up on
/// ledges when its companions are climbers and less when they are floor-dwellers, and a leaning
/// is kept in the save only once someone chooses one.
#[test]
fn a_colony_of_climbers_lives_higher_than_a_colony_of_floor_dwellers() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    for index in 0..6_u64 {
        desktop.windows.push(DesktopWindow {
            key: 900 + index,
            bounds: DesktopRect {
                x: 80.0 + (index % 3) as f32 * 440.0,
                y: 260.0 + (index / 3) as f32 * 260.0,
                width: 380.0,
                height: 200.0,
            },
            z_order: index as u32,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
    }
    let share_up_high = |leaning: RoamingLeaning| {
        let mut world = World::new([83; 32], created, &desktop);
        let start = created + Duration::days(40);
        world.tick(start, 0.05, &desktop);
        let_colony_wander(&mut world, start);
        world.save.ritual.next_at_utc = start + Duration::days(1);
        for creature in &mut world.save.creatures {
            creature.state.arrival_delay_secs = 0.0;
            creature.leaning = leaning;
        }
        // Everyone is on the floor at the houses whatever their leaning, so only time out on the
        // desktop says anything about where a companion likes to be.
        let (mut up, mut total) = (0_usize, 0_usize);
        for step in 1..=36_000_i64 {
            world.tick(start + Duration::milliseconds(step * 50), 0.05, &desktop);
            world.drain_events().for_each(drop);
            if world.save.home.is_active() {
                continue;
            }
            for creature in &world.save.creatures {
                total += 1;
                up += usize::from(creature.state.surface.kind == SurfaceKind::WindowLedge);
            }
        }
        up as f32 / total.max(1) as f32
    };
    let climbers = share_up_high(RoamingLeaning::Climber);
    let anywhere = share_up_high(RoamingLeaning::Anywhere);
    let homebodies = share_up_high(RoamingLeaning::Homebody);
    let floor = share_up_high(RoamingLeaning::FloorDweller);
    eprintln!(
        "time on ledges while out: climbers {:.1}%, anywhere {:.1}%, homebodies {:.1}%, \
         floor-dwellers {:.1}%",
        climbers * 100.0,
        anywhere * 100.0,
        homebodies * 100.0,
        floor * 100.0
    );
    assert!(
        climbers >= anywhere && anywhere > homebodies && homebodies > floor,
        "{climbers} {anywhere} {homebodies} {floor}"
    );
    assert!(floor < anywhere / 2.0, "a floor-dweller rarely climbs");

    let mut world = World::new([83; 32], created, &desktop);
    let id = world.save.creatures[0].id;
    let text = serde_json::to_string(&world.save).unwrap();
    assert!(
        !text.contains("leaning"),
        "an untouched colony writes no leaning"
    );
    assert!(world.set_roaming_leaning(id, RoamingLeaning::Homebody));
    assert!(!world.set_roaming_leaning(id, RoamingLeaning::Homebody));
    let reloaded: SaveFile =
        serde_json::from_str(&serde_json::to_string(&world.save).unwrap()).unwrap();
    assert_eq!(reloaded.creatures[0].leaning, RoamingLeaning::Homebody);
}

/// Measurement, not a check: how much of their time out on the desktop an ordinary colony spends
/// up on ledges, on a few everyday arrangements of windows. Run with `--ignored --nocapture`.
#[test]
#[ignore]
fn measure_time_on_ledges() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let window = |key: u64, x: f32, y: f32, width: f32, height: f32| DesktopWindow {
        key,
        bounds: DesktopRect {
            x,
            y,
            width,
            height,
        },
        z_order: key as u32,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    };
    let layouts: Vec<(&str, Vec<DesktopWindow>)> = vec![
        (
            "grid of six",
            (0..6_u64)
                .map(|index| {
                    window(
                        900 + index,
                        80.0 + (index % 3) as f32 * 440.0,
                        260.0 + (index / 3) as f32 * 260.0,
                        380.0,
                        200.0,
                    )
                })
                .collect(),
        ),
        (
            "laptop",
            vec![
                window(901, 60.0, 40.0, 1000.0, 780.0),
                window(902, 420.0, 90.0, 960.0, 700.0),
                window(903, 900.0, 380.0, 480.0, 420.0),
            ],
        ),
        (
            "everyday",
            vec![
                window(901, 100.0, 24.0, 1100.0, 826.0),
                window(902, 300.0, 170.0, 760.0, 480.0),
                window(903, 880.0, 90.0, 480.0, 600.0),
            ],
        ),
        ("maximised", vec![window(901, 0.0, 24.0, 1440.0, 826.0)]),
        (
            "side by side",
            vec![
                window(901, 0.0, 24.0, 720.0, 826.0),
                window(902, 720.0, 24.0, 720.0, 826.0),
            ],
        ),
    ];
    for (name, windows) in layouts {
        let mut desktop = desktop();
        desktop.windows = windows;
        let mut world = World::new([83; 32], created, &desktop);
        let start = created + Duration::days(40);
        world.tick(start, 0.05, &desktop);
        let_colony_wander(&mut world, start);
        world.save.ritual.next_at_utc = start + Duration::days(1);
        for creature in &mut world.save.creatures {
            creature.state.arrival_delay_secs = 0.0;
        }
        let (mut up, mut total, mut all_down, mut ticks) = (0_usize, 0_usize, 0_usize, 0_usize);
        // How long each stint up high and down on the floor lasts, in seconds.
        let mut stints: BTreeMap<CreatureId, (bool, f32)> = BTreeMap::new();
        let (mut ups, mut downs): (Vec<f32>, Vec<f32>) = (Vec::new(), Vec::new());
        for step in 1..=36_000_i64 {
            let now = start + Duration::milliseconds(step * 50);
            world.tick(now, 0.05, &desktop);
            world.drain_events().for_each(drop);
            let_colony_wander(&mut world, now);
            ticks += 1;
            let mut any_up = false;
            for creature in &world.save.creatures {
                total += 1;
                let high = creature.state.surface.kind == SurfaceKind::WindowLedge
                    || creature.state.position.y < 800.0;
                up += usize::from(high);
                any_up |= high;
                let stint = stints.entry(creature.id).or_insert((high, 0.0));
                if stint.0 == high {
                    stint.1 += 0.05;
                } else {
                    if stint.0 {
                        ups.push(stint.1)
                    } else {
                        downs.push(stint.1)
                    }
                    *stint = (high, 0.05);
                }
            }
            all_down += usize::from(!any_up);
        }
        let mean = |values: &[f32]| values.iter().sum::<f32>() / values.len().max(1) as f32;
        eprintln!(
            "{name:>13}: {} companions, {:.1}% of their time up off the floor, all on the floor \
             {:.1}% of the time; {} stints up of {:.0}s, {} down of {:.0}s on average",
            world.save.creatures.len(),
            up as f32 / total.max(1) as f32 * 100.0,
            all_down as f32 / ticks.max(1) as f32 * 100.0,
            ups.len(),
            mean(&ups),
            downs.len(),
            mean(&downs),
        );
    }
}
