use super::{
    colony_bounds, habitat_editor_claims, resolve_press_target, world_redraw_interval,
    world_tick_interval,
};
use formiga_core::*;
use std::time::Duration;
use winit::event::WindowEvent;

/// A press on the desktop belongs to the open editor, which draws a region with it instead of
/// letting it reach the colony. A redraw never does: the editor claimed one for a while, and
/// the overlay it was addressed to then stopped painting for as long as the editor was open,
/// so the colony sat frozen on its last frame and not one of the regions being dragged out
/// was ever drawn.
#[test]
fn the_habitat_editor_takes_the_pointer_and_leaves_the_overlay_its_redraw() {
    let device_id = winit::event::DeviceId::dummy();
    assert!(habitat_editor_claims(&WindowEvent::MouseInput {
        device_id,
        state: winit::event::ElementState::Pressed,
        button: winit::event::MouseButton::Left,
    }));
    assert!(habitat_editor_claims(&WindowEvent::CursorMoved {
        device_id,
        position: winit::dpi::PhysicalPosition::new(4.0, 4.0),
    }));
    for event in [
        WindowEvent::RedrawRequested,
        WindowEvent::Resized(winit::dpi::PhysicalSize::new(800, 600)),
        WindowEvent::CloseRequested,
    ] {
        assert!(
            !habitat_editor_claims(&event),
            "the editor swallowed {event:?}, which the overlay needs"
        );
    }
}

/// How tall the strip along the bottom of the screen really is when the owner has never
/// touched it: the Dock with its factory forty-eight point tiles inside a panel of its own,
/// or the Windows 11 taskbar.
#[cfg(target_os = "macos")]
const FACTORY_SYSTEM_STRIP: f32 = 75.0;
#[cfg(target_os = "windows")]
const FACTORY_SYSTEM_STRIP: f32 = 48.0;

/// The colony stands on the floor of the ground it is given, so that floor has to clear the
/// strip the system keeps for itself along the bottom of the display. It stood forty points
/// up, well inside a Dock at its factory size, and the village lived behind one.
#[test]
fn the_village_stands_on_top_of_the_dock_rather_than_behind_it() {
    for screen in [
        DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 1512.0,
            height: 982.0,
        },
        DesktopRect {
            x: -1920.0,
            y: 120.0,
            width: 1920.0,
            height: 1080.0,
        },
    ] {
        // With no work area to read, the fixed strips stand in.
        let usable = colony_bounds(screen, None);
        // Where the houses are founded and where everybody's feet meet the ground.
        let ground = usable.bottom() - 4.0;
        assert!(
            screen.bottom() - ground >= FACTORY_SYSTEM_STRIP,
            "the village stands {} points up, inside a strip {FACTORY_SYSTEM_STRIP} tall",
            screen.bottom() - ground
        );
        assert_eq!(usable.x, screen.x);
        assert_eq!(usable.width, screen.width);
    }
}

/// The ground follows the work area the system reports: a Dock on the side narrows it
/// instead of lifting it, a hidden Dock gives the floor back, a notched display's taller menu
/// bar pushes the top down, and a taskbar along the top leaves the bottom free. Nonsense
/// insets never leave the colony less than 100 points to stand in.
#[test]
fn the_ground_follows_where_the_system_bars_really_are() {
    let screen = DesktopRect {
        x: 0.0,
        y: 0.0,
        width: 1512.0,
        height: 982.0,
    };
    let bars = |left, top, right, bottom| {
        Some(crate::platform::Insets {
            left,
            top,
            right,
            bottom,
        })
    };
    let dock_on_the_left = colony_bounds(screen, bars(64.0, 37.0, 0.0, 0.0));
    assert_eq!((dock_on_the_left.x, dock_on_the_left.width), (64.0, 1448.0));
    assert_eq!(
        (dock_on_the_left.y, dock_on_the_left.bottom()),
        (37.0, 982.0)
    );
    let hidden_dock = colony_bounds(screen, bars(0.0, 24.0, 0.0, 4.0));
    assert_eq!(hidden_dock.bottom(), 978.0);
    let taskbar_on_top = colony_bounds(screen, bars(0.0, 48.0, 0.0, 0.0));
    assert_eq!((taskbar_on_top.y, taskbar_on_top.bottom()), (48.0, 982.0));
    let tall_dock = colony_bounds(screen, bars(0.0, 24.0, 0.0, 128.0));
    assert_eq!(tall_dock.bottom(), 854.0);
    for nonsense in [
        bars(-40.0, f32::NAN, 5_000.0, f32::INFINITY),
        bars(2_000.0, 2_000.0, 2_000.0, 2_000.0),
    ] {
        let usable = colony_bounds(screen, nonsense);
        assert!(
            usable.width >= 100.0 && usable.height >= 100.0,
            "{usable:?}"
        );
        assert!(usable.x >= screen.x && usable.right() <= screen.right() + 0.01);
        assert!(usable.y >= screen.y && usable.bottom() <= screen.bottom() + 0.01);
    }
}

#[test]
fn a_gesture_is_presented_at_its_own_frame_rate_rather_than_the_action_beneath_it() {
    let mut world = World::new(
        [5; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    for creature in &mut world.save.creatures {
        creature.state.action = ActionKind::InspectScreen;
        creature.state.arrival_delay_secs = 0.0;
        creature.state.velocity = Point::default();
        creature.state.attention = None;
    }
    // Inspecting animates at four frames a second; a gasp over it animates at six.
    assert_eq!(world_redraw_interval(&world), Duration::from_secs_f32(0.25));
    let mut pose = AttentionPose {
        target: Point::default(),
        emotion: AttentionEmotion::Startled,
        hanging: 0.0,
        gesture: Some(Gesture::Gasp),
    };
    world.save.creatures[0].state.attention = Some(pose);
    assert_eq!(
        world_redraw_interval(&world),
        Duration::from_secs_f32(1.0 / 6.0)
    );
    // A cheer is done the creature's own way, at the rate of its own celebration.
    pose.gesture = Some(Gesture::Cheer);
    world.save.creatures[0].state.attention = Some(pose);
    let celebration = Celebration::for_creature(&world.save.creatures[0]).gesture();
    assert_eq!(
        world_redraw_interval(&world),
        Duration::from_secs_f32(
            1.0 / f32::from(formiga_art::AnimationSpec::for_clip(celebration).fps)
        )
    );
}

/// A colony that is only resting is the commonest thing on the screen, so what it costs to
/// draw is close to what Formiga costs. The rest loop is presented at its own three frames a
/// second, which is one redraw a second fewer than the four-frame rest it grew out of: the
/// longer loop is also the cheaper one, and only sleeping and walking home are quieter.
#[test]
fn a_resting_colony_is_drawn_three_times_a_second() {
    let mut world = World::new(
        [5; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    for creature in &mut world.save.creatures {
        creature.state.action = ActionKind::Idle;
        creature.state.arrival_delay_secs = 0.0;
        creature.state.velocity = Point::default();
        creature.state.attention = None;
        creature.state.flourish = None;
    }
    assert_eq!(
        world_redraw_interval(&world),
        Duration::from_secs_f32(1.0 / 3.0)
    );
    let resting = formiga_art::AnimationSpec::for_action(ActionKind::Idle);
    assert!(
        resting.fps < 4,
        "a colony at rest is redrawn as often as it was before the rest loop grew"
    );
    for action in [ActionKind::Sleep, ActionKind::Homebound] {
        assert!(
            formiga_art::AnimationSpec::for_action(action).fps < resting.fps,
            "{action:?} should be quieter still than resting"
        );
    }
}

/// A stroll round the village at home is ticked and drawn at 10 Hz; the walk home, anything
/// brisker, and any movement out on the desktop keep the full 20.
#[test]
fn a_stroll_at_home_ticks_at_ten_and_a_walk_at_twenty() {
    let mut world = World::new(
        [5; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    world.save.home.active_since_utc = Some(time::OffsetDateTime::UNIX_EPOCH);
    for creature in &mut world.save.creatures {
        creature.state.action = ActionKind::Traverse;
        creature.state.arrival_delay_secs = 0.0;
        creature.state.velocity = Point { x: 16.0, y: 0.0 };
    }
    assert_eq!(world_tick_interval(&world), Duration::from_millis(100));
    assert_eq!(world_redraw_interval(&world), Duration::from_millis(100));
    // The briskest stroll a colony can have counts as a stroll. A companion of high spirits
    // strolls at nearly twenty-six points a second, and one of them used to hold the whole
    // village at twenty frames a second.
    world.save.creatures[0].state.velocity.x = MAX_STROLL_SPEED;
    assert_eq!(world_tick_interval(&world), Duration::from_millis(100));
    assert_eq!(world_redraw_interval(&world), Duration::from_millis(100));
    // A walk home is a different thing: the slowest walk any companion has is over thirty
    // points a second, and the village is drawn every tick while one is crossing it.
    world.save.creatures[0].state.velocity.x = 30.8;
    assert_eq!(world_tick_interval(&world), Duration::from_millis(50));
    world.save.creatures[0].state.velocity.x = 36.0;
    assert_eq!(world_tick_interval(&world), Duration::from_millis(50));
    world.save.creatures[0].state.velocity.x = 16.0;
    world.save.home.active_since_utc = None;
    assert_eq!(world_tick_interval(&world), Duration::from_millis(50));
    assert_eq!(world_redraw_interval(&world), Duration::from_millis(50));
}

#[test]
fn press_stays_on_the_window_whose_own_mask_covers_the_cursor() {
    let hits = [(10u32, 1u64), (20u32, 2u64)];
    let order = [1u64, 2u64];
    assert_eq!(
        resolve_press_target(20u32, &hits, &order),
        Some((20u32, 2u64))
    );
}

#[test]
fn press_on_a_transparent_overlap_reaches_the_creature_underneath() {
    // The colony shares one corner at the shelter, so the press lands on proxy 30 even though
    // only creatures 1 and 2 have opaque pixels under the cursor.
    let hits = [(10u32, 1u64), (20u32, 2u64)];
    let order = [1u64, 2u64, 3u64];
    assert_eq!(
        resolve_press_target(30u32, &hits, &order),
        Some((20u32, 2u64)),
        "should pick the creature drawn last, which is the visible one on top"
    );
}

#[test]
fn press_over_no_creature_starts_no_drag() {
    assert_eq!(resolve_press_target(30u32, &[], &[1u64]), None);
}
