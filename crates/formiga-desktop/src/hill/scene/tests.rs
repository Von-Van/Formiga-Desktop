use super::*;
use formiga_core::{DesktopSnapshot, DisplayKey, World};
use time::macros::datetime;

const STEP: f32 = 0.05;

fn monitor(id: MonitorId, x: f32, primary: bool, scale_factor: f32) -> MonitorInfo {
    MonitorInfo {
        id,
        display_key: DisplayKey([id as u8; 16]),
        bounds: DesktopRect {
            x,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        },
        usable_bounds: DesktopRect {
            x,
            y: 24.0,
            width: 1440.0,
            height: 826.0,
        },
        scale_factor,
        primary,
    }
}

/// Two displays, the primary on the right; a colony with somebody at each end of the primary's
/// floor, somebody up on a ledge, and somebody on the other display.
fn colony(display_scale: u8, scale_factor: f32) -> (SaveFile, Vec<MonitorInfo>) {
    let monitors = vec![
        monitor(1, 0.0, true, scale_factor),
        monitor(2, -1440.0, false, scale_factor),
    ];
    let desktop = DesktopSnapshot {
        monitors: monitors.clone(),
        ..DesktopSnapshot::default()
    };
    let now = datetime!(2026-10-02 9:00 UTC);
    let mut world = World::new([8; 32], now, &desktop);
    for seed in 9..12 {
        world
            .add_designed_adult([seed; 32], None, now, &desktop)
            .unwrap();
    }
    let mut save = world.save.clone();
    save.settings.display_scale = display_scale;
    save.home.display = Some(monitors[0].display_key);
    let ground = monitors[0].usable_bounds.bottom() - 4.0;
    let place = |creature: &mut formiga_core::Creature, monitor_id, kind, x: f32, y: f32| {
        creature.state.position = Point { x, y };
        creature.state.surface = SurfaceAttachment {
            kind,
            monitor_id,
            window_key: None,
            relative_x: 0.5,
        };
        creature.state.action = ActionKind::Idle;
    };
    place(
        &mut save.creatures[0],
        1,
        SurfaceKind::ScreenFloor,
        30.0,
        ground,
    );
    place(
        &mut save.creatures[1],
        1,
        SurfaceKind::ScreenFloor,
        1400.0,
        ground,
    );
    place(
        &mut save.creatures[2],
        1,
        SurfaceKind::WindowLedge,
        700.0,
        300.0,
    );
    place(
        &mut save.creatures[3],
        2,
        SurfaceKind::ScreenFloor,
        -800.0,
        ground,
    );
    (save, monitors)
}

/// Every frame of a scene, in order: the elapsed time, the train and who is on the platform.
fn play(mut scene: TrainScene, save: &SaveFile) -> Vec<(f32, Option<TrainPose>, SaveFile)> {
    let mut frames = Vec::new();
    let mut t = 0.0;
    while !scene.finished() {
        frames.push((t, scene.train(), scene.stage(save)));
        scene.advance(STEP);
        t += STEP;
        assert!(t < 60.0, "the scene never ends");
    }
    frames.push((t, scene.train(), scene.stage(save)));
    frames
}

#[test]
fn leaving_everyone_gets_on_and_the_train_pulls_away() {
    let (save, monitors) = colony(3, 2.0);
    let scene = TrainScene::departure(&save, &monitors).unwrap();
    assert_eq!(scene.leg(), Leg::Departure);
    assert_eq!(scene.monitor_id(), 1);
    let frames = play(scene, &save);
    let (_, first_train, first) = &frames[0];
    // The train comes in from off the display, and the companions already here are standing where
    // they were; the one on the other display is not here yet.
    assert!(first_train.unwrap().left >= monitors[0].bounds.right());
    assert!(!first_train.unwrap().facing_right);
    assert!(
        first
            .creatures
            .iter()
            .all(|c| c.state.surface.monitor_id == 1)
    );
    assert!(first.creatures.iter().all(|c| c.id != save.creatures[3].id));
    // Nobody steps aboard a moving train.
    for pair in frames.windows(2) {
        let ((_, train, before), (_, _, after)) = (&pair[0], &pair[1]);
        if after.creatures.len() < before.creatures.len() {
            let train = train.expect("a train to get on");
            assert!(
                train.frame >= formiga_art::RUNNING_FRAMES,
                "boarding while it moves"
            );
        }
    }
    let (_, last_train, last) = frames.last().unwrap();
    assert!(last.creatures.is_empty(), "everyone is aboard");
    assert!(last_train.is_none(), "and the train has gone");
    let seen: std::collections::BTreeSet<_> = frames
        .iter()
        .flat_map(|(_, _, staged)| staged.creatures.iter().map(|c| c.id))
        .collect();
    assert_eq!(
        seen.len(),
        save.creatures.len(),
        "everyone is seen getting on"
    );
}

#[test]
fn companions_from_another_display_come_in_from_its_side() {
    let (save, monitors) = colony(3, 2.0);
    let traveller = save.creatures[3].id;
    let frames = play(TrainScene::departure(&save, &monitors).unwrap(), &save);
    let first_seen = frames
        .iter()
        .find_map(|(_, _, staged)| staged.creatures.iter().find(|c| c.id == traveller))
        .unwrap();
    assert!(
        first_seen.state.position.x < 40.0,
        "it comes in at the left, nearest its own display: {}",
        first_seen.state.position.x
    );
    assert_eq!(first_seen.state.surface.monitor_id, 1);
}

#[test]
fn coming_home_everyone_ends_exactly_where_they_were() {
    let (save, monitors) = colony(3, 2.0);
    let frames = play(TrainScene::arrival(&save, &monitors).unwrap(), &save);
    let (_, first_train, first) = &frames[0];
    assert!(first.creatures.is_empty(), "everyone starts aboard");
    assert!(first_train.unwrap().facing_right);
    assert!(first_train.unwrap().left < monitors[0].bounds.x);
    let (_, last_train, last) = frames.last().unwrap();
    assert!(last_train.is_none());
    // Those who live on this floor are standing exactly as they were before they left; the others
    // have gone on to their ledge and their display, where the colony itself puts them.
    let floor: Vec<_> = save.creatures[..2].to_vec();
    assert_eq!(last.creatures, floor);
}

#[test]
fn with_reduced_motion_the_train_does_not_slide() {
    let (mut save, monitors) = colony(3, 2.0);
    save.settings.reduce_motion = true;
    for scene in [
        TrainScene::departure(&save, &monitors).unwrap(),
        TrainScene::arrival(&save, &monitors).unwrap(),
    ] {
        let frames = play(scene, &save);
        let lefts: std::collections::BTreeSet<_> = frames
            .iter()
            .filter_map(|(_, train, _)| train.map(|train| train.left.to_bits()))
            .collect();
        assert_eq!(lefts.len(), 1, "the train stands still where it is");
        assert!(
            frames
                .iter()
                .filter_map(|(_, train, _)| *train)
                .all(|train| train.frame == formiga_art::RUNNING_FRAMES),
            "and neither puffs nor turns its wheels"
        );
    }
}

#[test]
fn nobody_waits_long_for_the_train_at_any_size() {
    for display_scale in [2, 3, 4] {
        for scale_factor in [1.0, 1.5, 2.0] {
            let (save, monitors) = colony(display_scale, scale_factor);
            for scene in [
                TrainScene::departure(&save, &monitors).unwrap(),
                TrainScene::arrival(&save, &monitors).unwrap(),
            ] {
                assert!(
                    scene.length() < 16.0,
                    "{:?} at {display_scale}x on a {scale_factor}x display takes {}s",
                    scene.leg(),
                    scene.length()
                );
            }
        }
    }
}

#[test]
fn turned_back_the_train_lets_everyone_off_where_it_stands() {
    let (save, monitors) = colony(3, 2.0);
    let mut leaving = TrainScene::departure(&save, &monitors).unwrap();
    leaving.advance(4.0);
    let standing = leaving.train().unwrap();
    let back = leaving.turn_back(&save, &monitors).unwrap();
    let train = back.train().unwrap();
    assert_eq!(train.left, standing.left, "the same train at the same stop");
    assert_eq!(train.facing_right, standing.facing_right);
    let frames = play(back, &save);
    assert_eq!(
        frames.last().unwrap().2.creatures,
        save.creatures[..2].to_vec()
    );
}

#[test]
fn a_hidden_colony_or_one_with_no_ground_has_no_scene() {
    let (mut save, monitors) = colony(3, 2.0);
    save.settings.visible = false;
    assert!(TrainScene::departure(&save, &monitors).is_none());
    let (save, _) = colony(3, 2.0);
    assert!(TrainScene::arrival(&save, &[]).is_none());
}
