use super::home::settled_colony;
use super::*;
use crate::WonderKind;

/// Ticks until the wonder out now has gone, or `seconds` have passed. Gives the seconds taken.
fn play_out(
    world: &mut World,
    now: OffsetDateTime,
    desktop: &DesktopSnapshot,
    seconds: f32,
) -> f32 {
    let mut elapsed = 0.0;
    while elapsed < seconds {
        world.tick(now, 0.05, desktop);
        elapsed += 0.05;
        if world.wonder().is_none() {
            return elapsed;
        }
    }
    elapsed
}

/// Ticks until a wonder appears, giving up after a minute.
fn bring_one_out(
    world: &mut World,
    now: OffsetDateTime,
    desktop: &DesktopSnapshot,
    kind: Option<WonderKind>,
) -> bool {
    world.wonders.due_now(kind);
    for _ in 0..1_200 {
        world.tick(now, 0.05, desktop);
        if world.wonder().is_some() {
            return true;
        }
    }
    false
}

fn awake(world: &mut World) {
    for creature in &mut world.save.creatures {
        creature.state.drives.energy = 0.9;
        creature.state.drives.sleep_pressure = 0.1;
        creature.personality.playfulness = 0.9;
    }
}

/// A wonder turns up near somebody, who goes straight over, has a go, and is back on the floor on
/// its own feet once it has gone; the notebook writes the first of its kind down.
#[test]
fn a_wonder_turns_up_is_played_on_and_goes() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([21; 32], created);
    awake(&mut world);
    let now = created + Duration::hours(1);
    assert!(bring_one_out(
        &mut world,
        now,
        &desktop,
        Some(WonderKind::Chair)
    ));
    let view = world.wonder().unwrap();
    assert_eq!(view.kind, WonderKind::Chair);
    let players = world.wonders.player_ids();
    assert_eq!(players.len(), 1, "a chair is for one");
    let player = players[0];
    assert!(world.wonders.owns(player));
    // On the floor of the only display, inside it.
    assert!(
        (view.at.y - 846.0).abs() < 0.5,
        "stands on the floor: {view:?}"
    );
    let mut sat = false;
    let mut elapsed = 0.0;
    while world.wonder().is_some() && elapsed < 60.0 {
        world.tick(now, 0.05, &desktop);
        elapsed += 0.05;
        let creature = world
            .save
            .creatures
            .iter()
            .find(|c| c.id == player)
            .unwrap();
        if creature
            .state
            .attention
            .is_some_and(|pose| pose.gesture == Some(Gesture::Sit))
        {
            sat = true;
            assert!(creature.state.position.y < 846.0, "sitting up on the seat");
        }
    }
    assert!(world.wonder().is_none(), "gone again within a minute");
    assert!(sat, "it sat in the chair");
    let creature = world
        .save
        .creatures
        .iter()
        .find(|c| c.id == player)
        .unwrap();
    assert!(!world.wonders.owns(player));
    assert!(
        (creature.state.position.y - 846.0).abs() < 0.5,
        "back on the floor"
    );
    assert!(creature.state.attention.is_none());
    let record = &world.save.companion.wonders[0];
    assert_eq!(record.kind, WonderKind::Chair);
    assert_eq!(record.finder, Some(player));
    assert_eq!(record.goes, 1);
    assert!(
        world
            .save
            .companion
            .journal
            .iter()
            .any(|entry| entry.moment == JournalMoment::Wonder(WonderKind::Chair))
    );
    // A second chair is another go, not another find.
    assert!(bring_one_out(
        &mut world,
        now,
        &desktop,
        Some(WonderKind::Chair)
    ));
    play_out(&mut world, now, &desktop, 60.0);
    assert_eq!(world.save.companion.wonders.len(), 1);
    assert_eq!(world.save.companion.wonders[0].goes, 2);
}

/// A wonder for two sends both off at once, and they play it together.
#[test]
fn a_wonder_for_two_sends_both_at_once() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([22; 32], created);
    awake(&mut world);
    let now = created + Duration::hours(1);
    assert!(bring_one_out(
        &mut world,
        now,
        &desktop,
        Some(WonderKind::Seesaw)
    ));
    let players = world.wonders.player_ids();
    assert_eq!(players.len(), 2);
    // Both are on their way on the very tick it appeared, rather than one waiting for the other.
    for id in &players {
        let creature = world.save.creatures.iter().find(|c| c.id == *id).unwrap();
        assert!(matches!(
            creature.state.action,
            ActionKind::Traverse | ActionKind::Sprint | ActionKind::Idle
        ));
    }
    let mut both_up = false;
    let mut elapsed = 0.0;
    while world.wonder().is_some() && elapsed < 60.0 {
        world.tick(now, 0.05, &desktop);
        elapsed += 0.05;
        both_up |= players.iter().all(|id| {
            world
                .save
                .creatures
                .iter()
                .find(|c| c.id == *id)
                .and_then(|c| c.state.attention)
                .is_some_and(|pose| pose.gesture == Some(Gesture::Sit))
        });
    }
    assert!(both_up, "both sat on the seesaw at once");
    assert!(world.wonder().is_none());
}

/// Picking a player up, on the way or in the middle of it, sends the wonder away and lets the
/// other player go.
#[test]
fn picking_a_player_up_sends_the_wonder_away() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    for wait in [0.5_f32, 12.0] {
        let mut world = two_creature_world([23; 32], created);
        awake(&mut world);
        let now = created + Duration::hours(1);
        assert!(bring_one_out(
            &mut world,
            now,
            &desktop,
            Some(WonderKind::Seesaw)
        ));
        let players = world.wonders.player_ids();
        let mut elapsed = 0.0;
        while elapsed < wait {
            world.tick(now, 0.05, &desktop);
            elapsed += 0.05;
        }
        let held = players[0];
        let at = world
            .save
            .creatures
            .iter()
            .find(|c| c.id == held)
            .unwrap()
            .state
            .position;
        assert!(world.handle_command(
            WorldCommand::BeginInteraction {
                creature_id: held,
                cursor: at,
            },
            &desktop,
        ));
        world.tick(now, 0.05, &desktop);
        assert!(
            world.wonder().is_none_or(|view| view.presence < 1.0) && world.wonders.leaving(),
            "going as soon as somebody is picked up"
        );
        for id in &players {
            assert!(!world.wonders.owns(*id));
        }
        let other = world
            .save
            .creatures
            .iter()
            .find(|c| c.id == players[1])
            .unwrap();
        assert!(other.state.attention.is_none());
        assert!(
            (other.state.position.y - 846.0).abs() < 0.5,
            "the other is back on the floor"
        );
        let held = world.save.creatures.iter().find(|c| c.id == held).unwrap();
        assert!(
            (held.state.position.y - 846.0).abs() < 0.5,
            "the one picked up off a seat is on its feet in the hand"
        );
        play_out(&mut world, now, &desktop, 2.0);
        assert!(world.wonder().is_none());
    }
}

/// While the houses are out a wonder only ever turns up on the village's own ground, and its
/// players come back to that ground afterwards.
#[test]
fn at_home_a_wonder_stays_on_the_village_ground() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    for kind in [
        WonderKind::LeafSled,
        WonderKind::Fountain,
        WonderKind::Tightrope,
    ] {
        let mut world = settled_colony([24; 32], 3, created, &desktop);
        awake(&mut world);
        let commons = home_commons(
            &world.save.home,
            colony_cottage_list(&world.save.creatures).as_slice(),
            &desktop.monitors,
            &world.save.settings.habitat,
            world.save.settings.display_scale,
        )
        .unwrap();
        assert!(
            bring_one_out(&mut world, created, &desktop, Some(kind)),
            "{kind:?} turned up at home"
        );
        let view = world.wonder().unwrap();
        assert!((view.at.y - commons.ground_y).abs() < 0.5);
        assert!(view.at.x >= commons.low_x && view.at.x <= commons.high_x);
        let players = world.wonders.player_ids();
        play_out(&mut world, created, &desktop, 60.0);
        assert!(world.wonder().is_none());
        assert!(world.save.home.is_active());
        for id in players {
            let creature = world.save.creatures.iter().find(|c| c.id == id).unwrap();
            assert!((creature.state.position.y - commons.ground_y).abs() < 0.5);
        }
    }
}

/// A wonder can turn up on top of a window, and whoever it is for climbs up to it and ends its go
/// standing on that window.
#[test]
fn a_wonder_can_turn_up_on_a_window() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    desktop.windows.push(DesktopWindow {
        key: 61,
        bounds: DesktopRect {
            x: 300.0,
            y: 560.0,
            width: 700.0,
            height: 300.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let mut on_window = 0;
    for seed in 30..50_u8 {
        let mut world = two_creature_world([seed; 32], created);
        awake(&mut world);
        world
            .topology
            .rebuild_if_changed(&desktop, &BTreeMap::new());
        let now = created + Duration::hours(1);
        if !bring_one_out(&mut world, now, &desktop, Some(WonderKind::ArrowSign)) {
            continue;
        }
        let view = world.wonder().unwrap();
        let players = world.wonders.player_ids();
        play_out(&mut world, now, &desktop, 60.0);
        assert!(world.wonder().is_none(), "seed {seed}");
        if view.window_key == Some(61) {
            on_window += 1;
            assert!((view.at.y - 560.0).abs() < 0.5);
            for id in players {
                let creature = world.save.creatures.iter().find(|c| c.id == id).unwrap();
                assert_eq!(creature.state.surface.window_key, Some(61), "seed {seed}");
                assert!(
                    (creature.state.position.y - 560.0).abs() < 0.5,
                    "seed {seed}"
                );
            }
        }
    }
    assert!(on_window > 0, "some turn up on the window");
}

/// Reduced motion, a hidden colony and a paused one have no wonders.
#[test]
fn no_wonders_while_still_hidden_or_paused() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    for setting in 0..3 {
        let mut world = two_creature_world([25; 32], created);
        awake(&mut world);
        match setting {
            0 => world.save.settings.reduce_motion = true,
            1 => world.save.settings.visible = false,
            _ => world.save.settings.paused = true,
        }
        let now = created + Duration::hours(1);
        assert!(!bring_one_out(&mut world, now, &desktop, None));
    }
}

/// Every script starts with its players where they gathered and ends with them back on the
/// ground, for one and for two, and never strikes a pose mid-air without a hop to get there.
#[test]
fn every_script_starts_and_ends_on_the_ground() {
    use super::super::wonders::{Role, script_length, stance, start_offset};
    for kind in WonderKind::ALL {
        let length = script_length(kind);
        for pair in [false, true] {
            for role in [Role::Lead, Role::Partner] {
                if role == Role::Partner && !pair {
                    continue;
                }
                for toss in [false, true] {
                    let first = stance(kind, role, pair, 0.0, length, toss);
                    assert!(
                        first.dy.abs() < 0.01,
                        "{kind:?} {role:?} begins on the ground"
                    );
                    assert_eq!(first.dx, start_offset(kind, role), "{kind:?} {role:?}");
                    let last = stance(kind, role, pair, length, length, toss);
                    assert!(last.dy.abs() < 0.01, "{kind:?} {role:?} ends on the ground");
                    // Nothing moves further in one tick than the quickest hop up does.
                    let mut previous = first;
                    let mut t = 0.0;
                    while t <= length {
                        let next = stance(kind, role, pair, t, length, toss);
                        let jump = ((next.dx - previous.dx).powi(2)
                            + (next.dy - previous.dy).powi(2))
                        .sqrt();
                        assert!(jump < 10.0, "{kind:?} {role:?} jumps {jump} at {t}");
                        previous = next;
                        t += 0.05;
                    }
                }
            }
        }
    }
}

/// The leaf sled took the bike's place in 0.66.1 and keeps its name in a save, so a colony that
/// found the bike in 0.66.0 has found the sled, and a 0.66.0 build still reads a newer file.
#[test]
fn the_leaf_sled_is_saved_under_the_bikes_name() {
    assert_eq!(
        serde_json::to_string(&WonderKind::LeafSled).unwrap(),
        "\"Bike\""
    );
    assert_eq!(
        serde_json::from_str::<WonderKind>("\"Bike\"").unwrap(),
        WonderKind::LeafSled
    );
    let record: crate::WonderRecord = serde_json::from_str(
        r#"{"kind":"Bike","first_at":"2026-10-01T18:00:00Z","finder":7,"finder_name":"Poppy","goes":3}"#,
    )
    .unwrap();
    assert_eq!(record.kind, WonderKind::LeafSled);
    assert_eq!(record.goes, 3);
}
