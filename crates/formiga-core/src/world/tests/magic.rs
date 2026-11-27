use super::*;
use crate::magic::MagicKind;
use crate::meals::Food;
use crate::tuning::MAGIC;

const FLOOR_Y: f32 = 846.0;

/// A colony of two out on the desktop, both standing about on the floor.
fn colony(seed: u8) -> World {
    two_creature_world([seed; 32], datetime!(2026-09-29 12:00 UTC))
}

fn run(world: &mut World, desktop: &DesktopSnapshot, seconds: f32) {
    let now = world.save.maximum_seen_utc;
    for _ in 0..(seconds / 0.05).round() as usize {
        world.tick(now, 0.05, desktop);
    }
}

fn spell(world: &World, index: usize) -> Option<crate::magic::Spell> {
    world.save.creatures[index].state.magic
}

/// Wherever it is, it is somewhere a companion may be: on the floor, or on the window it says it
/// is on, along its top, and drawn on its own feet.
fn on_its_feet(world: &World, desktop: &DesktopSnapshot, index: usize) {
    let state = &world.save.creatures[index].state;
    match state.surface.window_key {
        None => assert!(
            (state.position.y - FLOOR_Y).abs() < 0.5 || state.action == ActionKind::Tossed,
            "off the floor at {:?}",
            state.position
        ),
        Some(key) => {
            let window = desktop.windows.iter().find(|w| w.key == key).unwrap();
            assert!((state.position.y - window.bounds.y).abs() < 0.5);
        }
    }
}

fn cast(world: &mut World, index: usize, kind: MagicKind) {
    let id = world.save.creatures[index].id;
    world.cast_spell(id, kind, 30.0, 0.0);
}

#[test]
fn a_ripe_magic_fruit_turns_up_in_the_next_meal_and_casts_a_spell_once_eaten() {
    let desktop = desktop();
    let mut cast_any = 0;
    for seed in [3_u8, 17, 40] {
        let mut world = colony(seed);
        world.magic.ripen_now();
        let eater = world.save.creatures[0].id;
        {
            let state = &mut world.save.creatures[0].state;
            state.action = ActionKind::Eat;
            state.action_elapsed = 0.0;
            state.action_duration = 6.0;
        }
        run(&mut world, &desktop, 0.1);
        assert_eq!(world.magic.eating(), Some(eater));
        assert_eq!(
            world.save.creatures[0].state.meal.map(|meal| meal.food),
            Some(Food::Starfruit)
        );
        assert!(
            world
                .thought_bubbles()
                .iter()
                .any(|b| b.creature_id == eater && b.icon == BubbleIcon::Surprise)
        );
        // Nothing comes over it until it has eaten it.
        assert!(spell(&world, 0).is_none());
        for _ in 0..200 {
            run(&mut world, &desktop, 0.05);
            if spell(&world, 0).is_some() {
                break;
            }
        }
        let cast = spell(&world, 0).expect("the spell comes once the fruit is eaten");
        cast_any += 1;
        assert!(world.magic.eating().is_none());
        assert!(cast.length >= MAGIC.length_secs.start);
        // The next one is a few days of desktop time away.
        assert!(world.magic.ripe_in >= MAGIC.ripe_after_secs.start);
    }
    assert_eq!(cast_any, 3);
}

#[test]
fn a_magic_fruit_dropped_half_eaten_does_nothing_and_another_ripens_sooner() {
    let desktop = desktop();
    let mut world = colony(5);
    world.magic.ripen_now();
    let eater = world.save.creatures[0].id;
    {
        let state = &mut world.save.creatures[0].state;
        state.action = ActionKind::Eat;
        state.action_elapsed = 0.0;
        state.action_duration = 8.0;
    }
    run(&mut world, &desktop, 1.0);
    assert_eq!(world.magic.eating(), Some(eater));
    // Picked up a moment in.
    let at = world.save.creatures[0].state.position;
    world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id: eater,
            cursor: at,
        },
        &desktop,
    );
    run(&mut world, &desktop, 0.2);
    world.handle_command(WorldCommand::CancelInteraction, &desktop);
    run(&mut world, &desktop, 3.0);
    assert!(spell(&world, 0).is_none());
    assert!(world.magic.eating().is_none());
    assert!(world.magic.ripe_in <= MAGIC.retry_secs.end);
}

#[test]
fn every_spell_shows_and_then_wears_off_by_itself_leaving_the_companion_as_it_was() {
    let desktop = desktop();
    for kind in MagicKind::ALL {
        let mut world = colony(9);
        cast(&mut world, 0, kind);
        let mut highest: f32 = 0.0;
        let mut lights = 0;
        for _ in 0..((30.0 + MAGIC.wear_off_secs + 1.0) / 0.05) as usize {
            run(&mut world, &desktop, 0.05);
            let Some(spell) = spell(&world, 0) else {
                break;
            };
            highest = highest.max(spell.lift);
            let presence = &world.save.creatures[0].state.presence;
            lights += spell
                .glints(presence.stepping, presence.stride, true)
                .iter()
                .flatten()
                .count();
            on_its_feet(&world, &desktop, 0);
        }
        assert!(spell(&world, 0).is_none(), "{kind:?} wore off");
        assert_eq!(world.save.creatures[0].state.drawn_lift(), 0.0);
        assert!(lights > 20, "{kind:?} showed its lights");
        match kind {
            MagicKind::Float => assert!(highest >= MAGIC.float_height - MAGIC.float_bob - 0.5),
            MagicKind::Bounce => assert!(highest >= MAGIC.bounce_height * 0.8),
            MagicKind::Zoomies | MagicKind::Shimmer => assert_eq!(highest, 0.0),
        }
        on_its_feet(&world, &desktop, 0);
    }
}

#[test]
fn zoomies_hurry_a_walk_and_nothing_else_does() {
    let desktop = desktop();
    let walked = |zoomies: bool| {
        let mut world = colony(12);
        if zoomies {
            cast(&mut world, 0, MagicKind::Zoomies);
        }
        // Let it come on before the walk.
        run(&mut world, &desktop, 1.5);
        let state = &mut world.save.creatures[0].state;
        state.beat = None;
        state.position.x = 300.0;
        state.facing_right = true;
        state.action = ActionKind::Traverse;
        state.action_elapsed = 0.0;
        state.action_duration = 100.0;
        world.save.creatures[1].state.position.x = 1200.0;
        run(&mut world, &desktop, 1.5);
        (world.save.creatures[0].state.position.x - 300.0).abs()
    };
    let (plain, hurried) = (walked(false), walked(true));
    assert!(hurried > plain * 1.4, "{plain} then {hurried}");
}

/// Each way a spell can be cut short, as something to do to the colony and whether the spell is
/// gone after it (rather than only held off for the moment).
#[test]
fn every_interruption_ends_a_spell_or_holds_it_off_and_leaves_the_companion_on_its_feet() {
    type Interruption = fn(&mut World, &mut DesktopSnapshot);
    let interruptions: [(&str, Interruption); 11] = [
        ("picked up", |world, desktop| {
            let id = world.save.creatures[0].id;
            let at = world.save.creatures[0].state.position;
            world.handle_command(
                WorldCommand::BeginInteraction {
                    creature_id: id,
                    cursor: at,
                },
                desktop,
            );
        }),
        ("thrown", |world, desktop| {
            let id = world.save.creatures[0].id;
            let at = world.save.creatures[0].state.position;
            world.handle_command(
                WorldCommand::BeginInteraction {
                    creature_id: id,
                    cursor: at,
                },
                desktop,
            );
            world.handle_command(
                WorldCommand::UpdateInteraction {
                    cursor: Point { x: at.x, y: 500.0 },
                    velocity: Point {
                        x: 300.0,
                        y: -300.0,
                    },
                },
                desktop,
            );
            world.handle_command(
                WorldCommand::EndInteraction {
                    cursor: Point { x: at.x, y: 500.0 },
                    velocity: Point {
                        x: 300.0,
                        y: -300.0,
                    },
                },
                desktop,
            );
        }),
        ("paused", |world, _| world.save.settings.paused = true),
        ("hidden", |world, _| world.save.settings.visible = false),
        ("quiet", |world, _| {
            world.save.companion.quiet_until =
                Some(world.save.maximum_seen_utc + Duration::hours(1));
        }),
        ("sent home", |world, desktop| {
            world.handle_command(WorldCommand::SendHome, desktop);
        }),
        ("asleep", |world, _| {
            let state = &mut world.save.creatures[0].state;
            state.beat = None;
            state.action = ActionKind::Sleep;
            state.action_elapsed = 0.0;
            state.action_duration = 30.0;
        }),
        ("off on a trip", |world, _| {
            let now = world.save.maximum_seen_utc;
            world.prepare_for_trip(now);
        }),
        ("into a house", |world, desktop| {
            let id = world.save.creatures[0].id;
            world.begin_house_visit(&[id], desktop);
        }),
        ("the colony settled", |world, desktop| {
            world.handle_command(WorldCommand::GatherCreatures, desktop);
        }),
        ("its window closing", |_world, desktop| {
            desktop.windows.clear();
        }),
    ];
    for (name, interrupt) in interruptions {
        for kind in [MagicKind::Float, MagicKind::Bounce, MagicKind::Shimmer] {
            let mut desktop = desktop();
            let mut world = colony(21);
            if name == "its window closing" {
                desktop.windows.push(DesktopWindow {
                    key: 44,
                    bounds: DesktopRect {
                        x: 200.0,
                        y: 600.0,
                        width: 600.0,
                        height: 200.0,
                    },
                    z_order: 0,
                    visible: true,
                    minimized: false,
                    application: None,
                    application_name: None,
                });
                world.save.creatures[0].state.surface = SurfaceAttachment {
                    kind: SurfaceKind::WindowLedge,
                    monitor_id: 1,
                    window_key: Some(44),
                    relative_x: 0.5,
                };
                world.save.creatures[0].state.position = Point { x: 500.0, y: 600.0 };
                run(&mut world, &desktop, 0.1);
            }
            cast(&mut world, 0, kind);
            run(&mut world, &desktop, 2.0);
            assert!(spell(&world, 0).is_some(), "{name}: the spell took");
            interrupt(&mut world, &mut desktop);
            run(&mut world, &desktop, 0.1);
            let state = &world.save.creatures[0].state;
            assert_eq!(
                state.magic.map_or(0.0, |s| s.lift),
                0.0,
                "{name}: {kind:?} is drawn on its feet at once"
            );
            let ends = !matches!(name, "its window closing" | "sent home");
            if ends {
                assert!(state.magic.is_none(), "{name}: {kind:?} ended");
            }
            // Let it all play out: let go of anything held, unpause, show, and run on.
            world.handle_command(WorldCommand::CancelInteraction, &desktop);
            world.save.settings.paused = false;
            world.save.settings.visible = true;
            world.save.companion.quiet_until = None;
            world.end_house_visit();
            for _ in 0..200 {
                run(&mut world, &desktop, 0.05);
                let state = &world.save.creatures[0].state;
                if state.action != ActionKind::Tossed {
                    assert!(state.magic.is_none_or(|s| s.lift >= 0.0));
                }
            }
            if ends {
                assert!(spell(&world, 0).is_none(), "{name}: it does not come back");
            }
            let state = &world.save.creatures[0].state;
            if state.action != ActionKind::Tossed && !world.save.home.is_active() {
                on_its_feet(&world, &desktop, 0);
            }
        }
    }
}

#[test]
fn a_climb_takes_the_body_and_the_spell_comes_back_once_it_is_up() {
    let mut desktop = desktop();
    desktop.windows.push(DesktopWindow {
        key: 7,
        bounds: DesktopRect {
            x: 600.0,
            y: 700.0,
            width: 400.0,
            height: 120.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let mut world = colony(30);
    // A new window starts a scene of its own: let it play out first.
    run(&mut world, &desktop, 4.0);
    world.clear_attention();
    for (index, x) in [(0, 400.0), (1, 1300.0)] {
        let state = &mut world.save.creatures[index].state;
        state.position = Point { x, y: FLOOR_Y };
        state.surface.kind = SurfaceKind::ScreenFloor;
        state.surface.window_key = None;
        state.action = ActionKind::Idle;
        state.action_elapsed = 0.0;
        state.action_duration = 100.0;
        state.beat = None;
    }
    world.window_journeys.clear();
    cast(&mut world, 0, MagicKind::Float);
    world.save.creatures[0].state.magic.as_mut().unwrap().length = 120.0;
    run(&mut world, &desktop, 2.0);
    assert!(spell(&world, 0).unwrap().lift > 0.0);
    let id = world.save.creatures[0].id;
    world.save.creatures[0].state.beat = None;
    assert!(world.begin_window_climb(id, 7, &desktop));
    let mut climbing_ticks = 0;
    for _ in 0..400 {
        run(&mut world, &desktop, 0.05);
        if world.window_journeys.contains_key(&id) {
            climbing_ticks += 1;
            assert_eq!(
                spell(&world, 0).unwrap().lift,
                0.0,
                "nothing floats up a window"
            );
        }
    }
    assert!(climbing_ticks > 10);
    let state = &world.save.creatures[0].state;
    assert_eq!(state.surface.window_key, Some(7));
    // Up, and floating again over the window's top.
    let mut floated = false;
    for _ in 0..100 {
        run(&mut world, &desktop, 0.05);
        floated |= spell(&world, 0).is_some_and(|s| s.lift > 0.0);
    }
    assert!(floated);
    on_its_feet(&world, &desktop, 0);
}

#[test]
fn reduced_motion_has_the_still_kind_and_nothing_hurries_or_lifts() {
    let desktop = desktop();
    let mut world = colony(14);
    world.save.settings.reduce_motion = true;
    cast(&mut world, 0, MagicKind::Float);
    let spell_now = spell(&world, 0).unwrap();
    assert!(spell_now.still);
    assert_eq!(spell_now.kind, MagicKind::Shimmer);
    assert!(!spell_now.moving());
    // No pose is struck for it either.
    assert!(world.save.creatures[0].state.beat.is_none());
    for _ in 0..200 {
        run(&mut world, &desktop, 0.05);
        let s = spell(&world, 0).unwrap();
        assert_eq!(s.lift, 0.0);
        assert_eq!(s.pace(), 1.0);
    }
    // Turned on in the middle of one, a spell goes still at once.
    let mut world = colony(14);
    cast(&mut world, 0, MagicKind::Bounce);
    run(&mut world, &desktop, 2.0);
    world.save.settings.reduce_motion = true;
    run(&mut world, &desktop, 0.05);
    let s = spell(&world, 0).unwrap();
    assert!(s.still);
    assert_eq!(s.lift, 0.0);
}

#[test]
fn a_spell_is_never_saved_and_a_relaunch_finds_everyone_as_they_were() {
    let desktop = desktop();
    let mut world = colony(18);
    cast(&mut world, 0, MagicKind::Float);
    run(&mut world, &desktop, 2.0);
    assert!(spell(&world, 0).is_some());
    let text = serde_json::to_string(&world.save).unwrap();
    assert!(!text.contains("magic") && !text.contains("lift"));
    let back: SaveFile = serde_json::from_str(&text).unwrap();
    assert!(back.creatures.iter().all(|c| c.state.magic.is_none()));
    let reopened = World::from_save(world.save.clone());
    assert!(
        reopened
            .save
            .creatures
            .iter()
            .all(|c| c.state.magic.is_none())
    );
    let mut state = world.save.creatures[0].state.clone();
    state.forget_runtime();
    assert!(state.magic.is_none());
}

#[test]
fn magic_fruit_waits_for_the_desktop_and_is_rare() {
    let desktop = desktop();
    let mut world = colony(22);
    let first = world.magic.ripe_in;
    assert!(MAGIC.ripe_after_secs.contains(&first));
    // A visit home, a pause, or a hidden colony do not ripen it.
    world.save.settings.paused = true;
    run(&mut world, &desktop, 10.0);
    world.save.settings.paused = false;
    world.save.settings.visible = false;
    run(&mut world, &desktop, 10.0);
    assert_eq!(world.magic.ripe_in, first);
    world.save.settings.visible = true;
    run(&mut world, &desktop, 10.0);
    assert!(world.magic.ripe_in < first);
}

#[test]
fn the_overlay_and_the_click_proxy_draw_a_floating_companion_where_it_floats() {
    let desktop = desktop();
    let mut world = colony(26);
    cast(&mut world, 0, MagicKind::Float);
    run(&mut world, &desktop, 2.0);
    let state = &world.save.creatures[0].state;
    let lift = state.magic.unwrap().lift;
    assert!(lift > 0.0);
    // Two points to the art pixel on this display, as the proxy measures it.
    let drawn = state.drawn_position(2.0);
    assert_eq!(drawn.x, state.position.x);
    assert!((state.position.y - drawn.y - lift * 2.0).abs() < 1e-3);
    assert!(
        (state.position.y - FLOOR_Y).abs() < 0.5,
        "its feet stay on the floor"
    );
}
