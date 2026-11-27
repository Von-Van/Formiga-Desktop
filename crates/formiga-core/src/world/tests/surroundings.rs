use super::*;

const FLOOR_Y: f32 = 846.0;

fn window(key: WindowKey, x: f32, y: f32, width: f32, height: f32) -> DesktopWindow {
    DesktopWindow {
        key,
        bounds: DesktopRect {
            x,
            y,
            width,
            height,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    }
}

fn run(world: &mut World, desktop: &DesktopSnapshot, seconds: f32) {
    let now = world.save.maximum_seen_utc;
    for _ in 0..(seconds / 0.05).round() as usize {
        world.tick(now, 0.05, desktop);
    }
}

/// Puts a companion standing still at `x`, on the floor or on top of `window`.
fn stand(world: &mut World, index: usize, x: f32, on: Option<&DesktopWindow>) {
    let state = &mut world.save.creatures[index].state;
    state.position = Point {
        x,
        y: on.map_or(FLOOR_Y, |w| w.bounds.y),
    };
    state.surface = SurfaceAttachment {
        kind: if on.is_some() {
            SurfaceKind::WindowLedge
        } else {
            SurfaceKind::ScreenFloor
        },
        monitor_id: 1,
        window_key: on.map(|w| w.key),
        relative_x: on.map_or(0.5, |w| (x - w.bounds.x) / w.bounds.width),
    };
    state.action = if on.is_some() {
        ActionKind::Perch
    } else {
        ActionKind::Idle
    };
    state.action_elapsed = 0.0;
    state.action_duration = 100.0;
    state.velocity = Point::default();
    state.beat = None;
}

/// A colony of two on `desktop`, settled after the scene its windows start, with the antics and
/// yawns that could take them first put off.
fn settled(seed: u8, desktop: &DesktopSnapshot) -> World {
    let mut world = two_creature_world([seed; 32], datetime!(2026-09-29 12:00 UTC));
    run(&mut world, desktop, 4.0);
    world.clear_attention();
    world.window_journeys.clear();
    world.window_routes.clear();
    world.tosses.clear();
    world.clear_beats();
    world.antics = crate::world::antics::Antics::new(&SeedStream::new([seed; 32]));
    world
}

fn beat_of(world: &World, index: usize) -> Option<BeatKind> {
    world.save.creatures[index].state.beat.map(|beat| beat.kind)
}

fn befriend(world: &mut World, affinity: u8) {
    let (a, b) = (world.save.creatures[0].id, world.save.creatures[1].id);
    let (a, b) = canonical_creature_pair(a, b).unwrap();
    world.save.relationships = vec![CreatureRelationship {
        a,
        b,
        affinity,
        familiarity: 160,
        playfulness: 160,
        avoidance: 0,
    }];
}

#[test]
fn a_companion_beside_a_window_points_at_it_and_looks() {
    let mut desktop = desktop();
    desktop.windows.push(window(5, 600.0, 500.0, 500.0, 330.0));
    let mut pointed = 0;
    for seed in 0..6_u8 {
        let mut world = settled(40 + seed, &desktop);
        stand(&mut world, 0, 540.0, None);
        stand(&mut world, 1, 200.0, None);
        world.surroundings.due_now();
        run(&mut world, &desktop, 0.1);
        if beat_of(&world, 0) == Some(BeatKind::PointAt) {
            pointed += 1;
            let state = &world.save.creatures[0].state;
            assert!(state.facing_right, "turned to the window");
            let look = state.beat.unwrap().look.unwrap();
            assert!(look.x > 600.0);
            // It stands where it was for the whole of it, and then it is over.
            run(&mut world, &desktop, 3.2);
            assert!((world.save.creatures[0].state.position.x - 540.0).abs() < 0.5);
            assert_ne!(beat_of(&world, 0), Some(BeatKind::PointAt));
        }
    }
    assert!(pointed >= 5, "{pointed} of 6");
}

#[test]
fn a_calm_companion_sits_on_a_tall_window_with_its_legs_over_the_front() {
    let mut desktop = desktop();
    let tall = window(9, 300.0, 400.0, 600.0, 400.0);
    desktop.windows.push(tall.clone());
    let mut sat = 0;
    for seed in 0..6_u8 {
        let mut world = settled(60 + seed, &desktop);
        world.save.creatures[0].personality.activity = 0.0;
        stand(&mut world, 0, 600.0, Some(&tall));
        stand(&mut world, 1, 100.0, None);
        world.surroundings.due_now();
        run(&mut world, &desktop, 0.1);
        if beat_of(&world, 0) != Some(BeatKind::LedgeSit) {
            continue;
        }
        sat += 1;
        run(&mut world, &desktop, 1.0);
        let state = &world.save.creatures[0].state;
        assert!(state.drawn_lift() < -1.0, "legs over the edge");
        assert_eq!(
            state.position.y, 400.0,
            "but sat on the window, where it was"
        );
        // The window goes: the sit goes with it, and it is drawn on its own feet.
        let mut gone = desktop.clone();
        gone.windows.clear();
        run(&mut world, &gone, 0.2);
        assert_ne!(beat_of(&world, 0), Some(BeatKind::LedgeSit));
        assert!(world.save.creatures[0].state.drawn_lift() >= 0.0);
    }
    assert!(sat >= 4, "{sat} of 6");
}

/// The friend at the end of a window's top, the nudger a step along from it, fond of each other.
fn nudge_scene(seed: u8, desktop: &DesktopSnapshot, ledge: &DesktopWindow) -> World {
    let mut world = settled(seed, desktop);
    befriend(&mut world, 220);
    world.save.creatures[0].personality.playfulness = 1.0;
    world.save.creatures[1].personality.playfulness = 0.0;
    let end = ledge.bounds.right();
    stand(&mut world, 0, end - 80.0, Some(ledge));
    stand(&mut world, 1, end - 20.0, Some(ledge));
    world
}

#[test]
fn a_playful_friend_nudges_another_off_a_low_ledge_and_it_lands_and_has_its_say() {
    let mut desktop = desktop();
    let low = window(3, 400.0, 720.0, 500.0, 120.0);
    desktop.windows.push(low.clone());
    let mut nudged = 0;
    for seed in 0..8_u8 {
        let mut world = nudge_scene(80 + seed, &desktop, &low);
        world.surroundings.due_now();
        run(&mut world, &desktop, 0.1);
        let Some((nudger, friend)) = world.surroundings.nudging() else {
            continue;
        };
        nudged += 1;
        assert_eq!(nudger, world.save.creatures[0].id);
        assert_eq!(friend, world.save.creatures[1].id);
        assert_eq!(beat_of(&world, 0), Some(BeatKind::Nudge));
        let mut in_the_air = false;
        let mut landed: Option<(Point, SurfaceKind)> = None;
        let mut said = false;
        for _ in 0..160 {
            run(&mut world, &desktop, 0.05);
            let state = &world.save.creatures[1].state;
            if state.action == ActionKind::Tossed {
                in_the_air = true;
            } else if in_the_air && landed.is_none() {
                landed = Some((state.position, state.surface.kind));
            }
            said |= matches!(
                beat_of(&world, 1),
                Some(BeatKind::Delight | BeatKind::Huff | BeatKind::Peek)
            );
            // Never anywhere outside the display it was on.
            assert!(state.position.x >= 0.0 && state.position.x <= 1440.0);
            assert!(state.position.y <= FLOOR_Y + 0.5);
        }
        assert!(in_the_air, "tipped off");
        assert!(said, "and had its say once it was down");
        assert!(world.surroundings.nudging().is_none());
        // It came down on the floor past the end of the ledge, standing.
        let (at, surface) = landed.expect("it came down");
        assert_eq!(surface, SurfaceKind::ScreenFloor);
        assert!((at.y - FLOOR_Y).abs() < 0.5);
        assert!(at.x > low.bounds.right() - 4.0);
        // And wherever it has got to since, it is on its feet somewhere it can be.
        let state = &world.save.creatures[1].state;
        if state.action != ActionKind::ClimbWindow {
            let y = state.surface.window_key.map_or(FLOOR_Y, |_| low.bounds.y);
            assert!((state.position.y - y).abs() < 0.5, "{:?}", state.position);
        }
    }
    assert!(nudged >= 5, "{nudged} of 8");
}

#[test]
fn nobody_is_nudged_off_a_high_ledge_out_of_the_habitat_or_by_a_stranger() {
    let mut nudges = 0;
    // Too high: a long way down is no joke.
    let mut desktop = desktop();
    let high = window(3, 400.0, 300.0, 500.0, 300.0);
    desktop.windows.push(high.clone());
    for seed in 0..6_u8 {
        let mut world = nudge_scene(100 + seed, &desktop, &high);
        world.surroundings.due_now();
        run(&mut world, &desktop, 0.1);
        nudges += usize::from(world.surroundings.nudging().is_some());
    }
    // The end of the ledge over the edge of the display.
    let mut desktop = super::desktop();
    let edge = window(4, 1000.0, 720.0, 440.0, 120.0);
    desktop.windows.push(edge.clone());
    for seed in 0..6_u8 {
        let mut world = nudge_scene(110 + seed, &desktop, &edge);
        world.surroundings.due_now();
        run(&mut world, &desktop, 0.1);
        nudges += usize::from(world.surroundings.nudging().is_some());
    }
    // Two who are not fond of each other.
    let mut desktop = super::desktop();
    let low = window(3, 400.0, 720.0, 500.0, 120.0);
    desktop.windows.push(low.clone());
    for seed in 0..6_u8 {
        let mut world = nudge_scene(120 + seed, &desktop, &low);
        befriend(&mut world, 40);
        world.surroundings.due_now();
        run(&mut world, &desktop, 0.1);
        nudges += usize::from(world.surroundings.nudging().is_some());
    }
    assert_eq!(nudges, 0);
}

#[test]
fn a_nudge_is_let_go_if_the_friend_moves_off_or_is_picked_up_first() {
    let mut desktop = desktop();
    let low = window(3, 400.0, 720.0, 500.0, 120.0);
    desktop.windows.push(low.clone());
    let mut checked = 0;
    for seed in 0..8_u8 {
        let mut world = nudge_scene(80 + seed, &desktop, &low);
        world.surroundings.due_now();
        run(&mut world, &desktop, 0.1);
        if world.surroundings.nudging().is_none() {
            continue;
        }
        checked += 1;
        let friend = world.save.creatures[1].id;
        let at = world.save.creatures[1].state.position;
        world.handle_command(
            WorldCommand::BeginInteraction {
                creature_id: friend,
                cursor: at,
            },
            &desktop,
        );
        run(&mut world, &desktop, 1.0);
        assert!(world.surroundings.nudging().is_none());
        world.handle_command(WorldCommand::CancelInteraction, &desktop);
        run(&mut world, &desktop, 2.0);
        assert_ne!(world.save.creatures[1].state.action, ActionKind::Tossed);
    }
    assert!(checked >= 4);
}

#[test]
fn nothing_begins_with_motion_reduced_during_a_scene_or_twice_running() {
    let mut desktop = desktop();
    let tall = window(9, 300.0, 400.0, 600.0, 400.0);
    desktop.windows.push(tall.clone());
    // Reduced motion: none of it.
    let mut world = settled(140, &desktop);
    world.save.settings.reduce_motion = true;
    stand(&mut world, 0, 600.0, Some(&tall));
    for _ in 0..20 {
        world.surroundings.due_now();
        run(&mut world, &desktop, 0.1);
        assert!(world.surroundings.begun.is_empty());
    }
    // While the colony is away from the desktop, paused or hidden: none of it either.
    let mut world = settled(141, &desktop);
    world.save.settings.paused = true;
    world.surroundings.due_now();
    run(&mut world, &desktop, 0.2);
    world.save.settings.paused = false;
    world.save.settings.visible = false;
    world.surroundings.due_now();
    run(&mut world, &desktop, 0.2);
    assert!(world.surroundings.begun.is_empty());
    // A companion has a while to itself after one, and the same kind never comes twice running.
    let mut world = settled(142, &desktop);
    world.save.creatures[0].personality.activity = 0.0;
    stand(&mut world, 0, 600.0, Some(&tall));
    stand(&mut world, 1, 100.0, None);
    world.surroundings.due_now();
    run(&mut world, &desktop, 0.1);
    let first = beat_of(&world, 0);
    assert!(first.is_some());
    run(&mut world, &desktop, 15.0);
    stand(&mut world, 0, 600.0, Some(&tall));
    world.surroundings.due_now();
    run(&mut world, &desktop, 0.1);
    assert!(
        beat_of(&world, 0).is_none(),
        "still having a while to itself"
    );
}

#[test]
fn surroundings_over_a_simulated_hour_vary_and_stay_occasional() {
    let mut desktop = desktop();
    desktop.windows.push(window(9, 300.0, 520.0, 600.0, 300.0));
    desktop.windows.push(window(10, 980.0, 700.0, 300.0, 140.0));
    let mut counts: BTreeMap<&'static str, u32> = BTreeMap::new();
    for seed in 0..4_u8 {
        let mut world = two_creature_world([150 + seed; 32], datetime!(2026-09-29 12:00 UTC));
        befriend(&mut world, 200);
        let start = world.save.maximum_seen_utc;
        // Out on the desktop for the whole hour.
        for step in 0..72_000_u32 {
            if step % 1_200 == 0 {
                let_colony_wander(&mut world, start);
            }
            world.tick(start, 0.05, &desktop);
        }
        for (kind, count) in &world.surroundings.begun {
            *counts.entry(kind).or_default() += count;
        }
    }
    let total: u32 = counts.values().sum();
    // Four colonies for an hour each: a handful an hour, never one every minute.
    assert!((4..=160).contains(&total), "{counts:?}");
    assert!(counts.len() >= 2, "more than one kind: {counts:?}");
}

/// How often each of these, and a magic fruit, comes round in colonies of four living on a desktop
/// with a few windows, the village coming and going as usual. Run by hand, in release:
/// `cargo test --release -p formiga-core week_of_surroundings -- --ignored --nocapture`.
#[test]
#[ignore = "a long measurement, run by hand"]
fn week_of_surroundings() {
    let mut desktop = desktop();
    desktop.window_sample = Some(WindowSample {
        monotonic_millis: 0,
        reliable: true,
    });
    desktop.windows.push(window(9, 260.0, 520.0, 560.0, 300.0));
    desktop.windows.push(window(10, 900.0, 690.0, 340.0, 150.0));
    desktop
        .windows
        .push(window(11, 1000.0, 300.0, 380.0, 240.0));
    let hours = 6;
    let mut counts: BTreeMap<&'static str, u32> = BTreeMap::new();
    let mut desktop_secs = 0.0_f64;
    let (mut spells, mut finds, mut waits) = (0_u32, 0_u32, Vec::new());
    for seed in 0..4_u8 {
        let start = datetime!(2026-10-05 12:00 UTC);
        let mut world = World::new([200 + seed; 32], start, &desktop);
        for extra in 0..3_u8 {
            let _ = world.add_designed_adult([seed * 7 + extra + 1; 32], None, start, &desktop);
        }
        let mut now = start;
        let mut ripe_since: Option<f64> = None;
        let mut had_spell = [false; 8];
        for step in 0..(hours * 72_000_u32) {
            now += Duration::milliseconds(50);
            desktop.window_sample.as_mut().unwrap().monotonic_millis = u64::from(step) * 50;
            world.tick(now, 0.05, &desktop);
            world.drain_events().for_each(drop);
            if !world.save.home.is_active() {
                desktop_secs += 0.05;
            }
            // A fresh fruit every hour, to see how soon somebody finds it.
            if step % 72_000 == 0 {
                world.magic.ripen_now();
                ripe_since = Some(f64::from(step) * 0.05);
            }
            if world.magic.eating().is_some()
                && let Some(since) = ripe_since.take()
            {
                finds += 1;
                waits.push(f64::from(step) * 0.05 - since);
            }
            for (index, creature) in world.save.creatures.iter().enumerate() {
                let has = creature.state.magic.is_some();
                if has && !had_spell[index] {
                    spells += 1;
                }
                had_spell[index] = has;
            }
        }
        for (kind, count) in &world.surroundings.begun {
            *counts.entry(kind).or_default() += count;
        }
    }
    let desktop_hours = desktop_secs / 3_600.0;
    println!("desktop hours across colonies: {desktop_hours:.1}");
    for (kind, count) in &counts {
        println!(
            "{kind}: {count} ({:.2} per desktop hour)",
            f64::from(*count) / desktop_hours
        );
    }
    let mean_wait = waits.iter().sum::<f64>() / waits.len().max(1) as f64;
    println!(
        "magic fruit: {finds} found of {} ripened, {spells} spells, mean wait {mean_wait:.0} s",
        hours * 4
    );
}
