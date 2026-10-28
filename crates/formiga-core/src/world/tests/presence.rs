use super::super::presence::carry;
use super::*;

/// A companion on the floor of the test display, doing `action`, at `x`.
fn walker(action: ActionKind, x: f32) -> Creature {
    let mut creature = World::preview_adult([57; 32], datetime!(2026-01-01 0:00 UTC), &desktop());
    creature.state.action = action;
    creature.state.position = Point { x, y: 846.0 };
    creature.state.surface = SurfaceAttachment {
        kind: SurfaceKind::ScreenFloor,
        monitor_id: 1,
        window_key: None,
        relative_x: 0.5,
    };
    creature
}

/// Ticks of a moving colony, one after another.
const TICK: f32 = 0.05;

/// Walking shows from its first step and stops showing once it has stopped; a walk's action
/// alone decides neither.
#[test]
fn stepping_is_what_the_feet_are_doing() {
    let desktop = desktop();
    let mut creature = walker(ActionKind::Traverse, 400.0);
    // Stood still long enough to set off at once.
    for _ in 0..10 {
        carry(&mut creature, TICK, &desktop, 1.0);
    }
    assert!(creature.state.presence.measured());
    assert!(
        !creature.state.presence.stepping,
        "a walk going nowhere is standing"
    );
    creature.state.position.x += 2.0;
    carry(&mut creature, TICK, &desktop, 1.0);
    assert!(
        creature.state.presence.stepping,
        "a walk shows from its first step"
    );
    for _ in 0..20 {
        creature.state.position.x += 2.0;
        carry(&mut creature, TICK, &desktop, 1.0);
    }
    assert!(creature.state.presence.stepping);
    // Arrived: a tick of standing is not yet a stop, two are.
    carry(&mut creature, TICK, &desktop, 1.0);
    assert!(creature.state.presence.stepping);
    carry(&mut creature, TICK, &desktop, 1.0);
    carry(&mut creature, TICK, &desktop, 1.0);
    assert!(!creature.state.presence.stepping, "stopped on its mark");
    assert!(creature.state.presence.since < 0.1);
}

/// A greeting with a few steps to go is stepping once the steps have gone on a moment, and a
/// single tick's nudge never is.
#[test]
fn a_nudge_is_not_a_step() {
    let desktop = desktop();
    let mut creature = walker(ActionKind::Idle, 400.0);
    for _ in 0..10 {
        carry(&mut creature, TICK, &desktop, 1.0);
    }
    creature.state.position.x += 3.0;
    carry(&mut creature, TICK, &desktop, 1.0);
    for _ in 0..10 {
        carry(&mut creature, TICK, &desktop, 1.0);
        assert!(!creature.state.presence.stepping, "one nudge is not a walk");
    }
    creature.state.action = ActionKind::Greet;
    for _ in 0..3 {
        creature.state.position.x -= 1.5;
        carry(&mut creature, TICK, &desktop, 1.0);
    }
    assert!(creature.state.presence.stepping, "a greeting walking over");
}

/// Standing on a window that is being moved is not walking, and walking along one is.
#[test]
fn a_window_carrying_a_creature_is_not_its_feet() {
    let mut desktop = desktop();
    desktop.windows.push(DesktopWindow {
        key: 7,
        bounds: DesktopRect {
            x: 300.0,
            y: 400.0,
            width: 500.0,
            height: 300.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let mut creature = walker(ActionKind::Idle, 550.0);
    creature.state.position.y = 400.0;
    creature.state.surface = SurfaceAttachment {
        kind: SurfaceKind::WindowLedge,
        monitor_id: 1,
        window_key: Some(7),
        relative_x: 0.5,
    };
    for _ in 0..10 {
        carry(&mut creature, TICK, &desktop, 1.0);
    }
    for _ in 0..20 {
        desktop.windows[0].bounds.x += 10.0;
        creature.state.position.x += 10.0;
        carry(&mut creature, TICK, &desktop, 1.0);
        assert!(
            !creature.state.presence.stepping,
            "carried along on the window"
        );
    }
    for _ in 0..4 {
        creature.state.surface.relative_x += 0.004;
        creature.state.position.x += 2.0;
        carry(&mut creature, TICK, &desktop, 1.0);
    }
    assert!(creature.state.presence.stepping, "walking along the ledge");
}

/// The feet go through a walking frame for each little stretch of ground covered, a smaller
/// body taking more frames for the same ground, within the range a step can be drawn at; a jump
/// is not walked at all.
#[test]
fn a_stride_keeps_pace_with_the_ground() {
    let desktop = desktop();
    let strides = |logical_size: u8, pace: f32| {
        let mut creature = walker(ActionKind::Traverse, 400.0);
        creature.appearance.logical_size = logical_size;
        for _ in 0..10 {
            carry(&mut creature, TICK, &desktop, 1.0);
        }
        for _ in 0..20 {
            creature.state.position.x += pace * TICK;
            carry(&mut creature, TICK, &desktop, 1.0);
        }
        creature.state.presence.stride
    };
    // Frames a second over the last second of walking.
    let adult = strides(38, 30.0);
    let mini = strides(26, 30.0);
    assert!(mini > adult, "a mini patters: {mini} against {adult}");
    let slow = strides(38, 16.0);
    let quick = strides(38, 50.0);
    assert!(slow < adult && adult < quick);
    assert!(
        slow >= 7.0 * 0.95 - 0.5,
        "never slower than seven frames a second: {slow}"
    );
    assert!(
        strides(20, 300.0) <= 15.0 * 1.0 + 0.5,
        "never faster than fifteen"
    );
    let mut creature = walker(ActionKind::Traverse, 400.0);
    for _ in 0..10 {
        carry(&mut creature, TICK, &desktop, 1.0);
    }
    creature.state.position.x += 200.0;
    carry(&mut creature, TICK, &desktop, 1.0);
    assert_eq!(creature.state.presence.stride, 0.0, "a jump is not walked");
    assert!(!creature.state.presence.stepping);
}

/// Nothing a colony does depends on how its companions are drawn: the same seed ticked the same
/// way does the same things whatever the presence says.
#[test]
fn presence_changes_nothing_the_colony_does() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let run = |wipe: bool| {
        let mut world = World::new([61; 32], created, &desktop);
        let now = created + Duration::hours(1);
        let_colony_wander(&mut world, now);
        for tick in 0..2_400 {
            if wipe {
                for creature in &mut world.save.creatures {
                    creature.state.presence = Presence::default();
                }
            }
            world.tick(now + Duration::milliseconds(50 * tick), TICK, &desktop);
        }
        world
            .save
            .creatures
            .iter()
            .map(|creature| (creature.state.position, creature.state.action))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(false), run(true));
}
