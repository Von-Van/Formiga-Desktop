use super::*;
use crate::{MAX_PLACED_WONDERS, WonderKind, WonderRecord};

fn found(world: &mut World, kinds: &[WonderKind], now: OffsetDateTime) {
    for kind in kinds {
        world.save.companion.wonders.push(WonderRecord {
            kind: *kind,
            first_at: now,
            finder: None,
            finder_name: "Mallow".into(),
            goes: 1,
        });
    }
}

fn awake(world: &mut World) {
    world.wonders.hold_off();
    for creature in &mut world.save.creatures {
        creature.state.drives.energy = 0.9;
        creature.state.drives.sleep_pressure = 0.1;
    }
}

/// Only a wonder the colony has found can be put out, each once, and no more than three at a
/// time; each stands on the floor where it was put, clear of the others, and goes when put away.
#[test]
fn found_wonders_are_put_out_and_put_away() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([21; 32], created);
    let now = created + Duration::hours(1);
    assert!(!world.handle_command(
        WorldCommand::PutOutWonder {
            kind: WonderKind::Chair
        },
        &desktop
    ));
    let kinds = [
        WonderKind::Chair,
        WonderKind::Fountain,
        WonderKind::Seesaw,
        WonderKind::Hammock,
    ];
    found(&mut world, &kinds, now);
    for kind in &kinds[..MAX_PLACED_WONDERS] {
        assert!(world.handle_command(WorldCommand::PutOutWonder { kind: *kind }, &desktop));
    }
    assert!(
        !world.handle_command(
            WorldCommand::PutOutWonder {
                kind: WonderKind::Hammock
            },
            &desktop
        ),
        "three at a time"
    );
    assert!(!world.handle_command(
        WorldCommand::PutOutWonder {
            kind: WonderKind::Chair
        },
        &desktop
    ));
    let views = world.placed_wonders();
    assert_eq!(views.len(), MAX_PLACED_WONDERS);
    for view in &views {
        assert!((view.at.y - 846.0).abs() < 0.5, "on the floor: {view:?}");
        assert_eq!(view.presence, 1.0);
    }
    for (index, a) in views.iter().enumerate() {
        for b in &views[index + 1..] {
            assert!((a.at.x - b.at.x).abs() > 40.0, "{a:?} and {b:?} overlap");
        }
    }
    // Still there after a while, and after a relaunch.
    for _ in 0..100 {
        world.tick(now, 0.05, &desktop);
    }
    assert_eq!(world.placed_wonders().len(), MAX_PLACED_WONDERS);
    let mut relaunched = World::from_save(world.save.clone());
    relaunched.tick(now, 0.05, &desktop);
    assert_eq!(relaunched.placed_wonders().len(), MAX_PLACED_WONDERS);

    assert!(world.handle_command(
        WorldCommand::PutAwayWonder {
            kind: WonderKind::Fountain
        },
        &desktop
    ));
    assert!(!world.handle_command(
        WorldCommand::PutAwayWonder {
            kind: WonderKind::Fountain
        },
        &desktop
    ));
    assert_eq!(world.placed_wonders().len(), 2);
}

/// Asked to, the nearest free companion goes over to a wonder put out and has its go; when it is
/// over the wonder is still there, and nothing new was found.
#[test]
fn a_go_on_a_wonder_put_out_leaves_it_where_it_stands() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([21; 32], created);
    awake(&mut world);
    let now = created + Duration::hours(1);
    found(&mut world, &[WonderKind::Chair], now);
    assert!(world.handle_command(
        WorldCommand::PutOutWonder {
            kind: WonderKind::Chair
        },
        &desktop
    ));
    let at = world.placed_wonders()[0].at;
    assert!(world.handle_command(
        WorldCommand::PlayOnWonder {
            kind: WonderKind::Chair
        },
        &desktop
    ));
    assert!(world.placed_wonder_in_use(WonderKind::Chair));
    assert!(
        world.wonder().is_none(),
        "not drawn as a wonder that turned up"
    );
    let player = world.wonders.player_ids()[0];
    let mut sat = false;
    let mut elapsed = 0.0;
    while world.placed_wonder_in_use(WonderKind::Chair) && elapsed < 90.0 {
        world.tick(now, 0.05, &desktop);
        elapsed += 0.05;
        let creature = world
            .save
            .creatures
            .iter()
            .find(|c| c.id == player)
            .unwrap();
        sat |= creature
            .state
            .attention
            .is_some_and(|pose| pose.gesture == Some(Gesture::Sit));
    }
    assert!(sat, "it sat in the chair");
    assert!(!world.placed_wonder_in_use(WonderKind::Chair));
    assert!(!world.wonders.owns(player));
    let views = world.placed_wonders();
    assert_eq!(views.len(), 1, "the chair is still out");
    assert_eq!(views[0].at, at);
    let record = &world.save.companion.wonders[0];
    assert_eq!(record.goes, 2, "one more go, and no second find");
}

/// Let go of over a window's top edge, a wonder stands on it and rides it; when the window
/// closes it is on the floor below, and a relaunch finds it there.
#[test]
fn a_wonder_set_on_a_window_rides_it_and_comes_down_when_it_closes() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut desktop = desktop();
    let mut world = two_creature_world([21; 32], created);
    let now = created + Duration::hours(1);
    found(&mut world, &[WonderKind::Fountain], now);
    assert!(world.handle_command(
        WorldCommand::PutOutWonder {
            kind: WonderKind::Fountain
        },
        &desktop
    ));
    desktop.windows.push(DesktopWindow {
        key: 7,
        bounds: DesktopRect {
            x: 300.0,
            y: 300.0,
            width: 500.0,
            height: 300.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    world.tick(now, 0.05, &desktop);
    assert!(world.handle_command(
        WorldCommand::MoveWonder {
            kind: WonderKind::Fountain,
            to: Point { x: 520.0, y: 290.0 },
        },
        &desktop
    ));
    let view = world.placed_wonders()[0];
    assert_eq!(view.window_key, Some(7));
    assert!((view.at.x - 520.0).abs() < 0.5 && (view.at.y - 300.0).abs() < 0.5);

    desktop.windows[0].bounds.x += 100.0;
    desktop.windows[0].bounds.y += 50.0;
    world.tick(now, 0.05, &desktop);
    let view = world.placed_wonders()[0];
    assert!((view.at.x - 620.0).abs() < 0.5 && (view.at.y - 350.0).abs() < 0.5);

    desktop.windows.clear();
    world.tick(now, 0.05, &desktop);
    let view = world.placed_wonders()[0];
    assert_eq!(view.window_key, None);
    assert!((view.at.x - 620.0).abs() < 1.0, "on the floor under it");
    assert!((view.at.y - 846.0).abs() < 0.5);
    let mut relaunched = World::from_save(world.save.clone());
    relaunched.tick(now, 0.05, &desktop);
    assert!((relaunched.placed_wonders()[0].at.x - 620.0).abs() < 1.0);

    // Let go of out in the open, it is on the floor under the pointer.
    assert!(world.handle_command(
        WorldCommand::MoveWonder {
            kind: WonderKind::Fountain,
            to: Point { x: 900.0, y: 400.0 },
        },
        &desktop
    ));
    let view = world.placed_wonders()[0];
    assert!((view.at.x - 900.0).abs() < 0.5 && (view.at.y - 846.0).abs() < 0.5);
}

/// Set down in among the houses of a village that stays out, a wonder stands beside them.
#[test]
fn a_wonder_set_down_among_the_houses_stands_beside_them() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([21; 32], created);
    world.save.home.stays_out = true;
    world.save.home.display = Some(DisplayKey([1; 16]));
    let now = created + Duration::hours(1);
    world.tick(now, 0.05, &desktop);
    found(&mut world, &[WonderKind::Chair], now);
    assert!(world.handle_command(
        WorldCommand::PutOutWonder {
            kind: WonderKind::Chair
        },
        &desktop
    ));
    let commons = home_commons(
        &world.save.home,
        colony_cottage_list(&world.save.creatures).as_slice(),
        &desktop.monitors,
        &world.save.settings.habitat,
        world.save.settings.display_scale,
    )
    .unwrap();
    // Art pixels are a point and a half each on this display.
    let half = WonderKind::Chair.span() * 1.5 / 2.0;
    let clear = |x: f32| x + half <= commons.low_x + 0.5 || x - half >= commons.high_x - 0.5;
    assert!(
        clear(world.placed_wonders()[0].at.x),
        "put out beside the village"
    );
    assert!(world.handle_command(
        WorldCommand::MoveWonder {
            kind: WonderKind::Chair,
            to: Point {
                x: (commons.low_x + commons.high_x) / 2.0,
                y: 800.0,
            },
        },
        &desktop
    ));
    let x = world.placed_wonders()[0].at.x;
    assert!(clear(x), "{x} in {}..{}", commons.low_x, commons.high_x);
}
