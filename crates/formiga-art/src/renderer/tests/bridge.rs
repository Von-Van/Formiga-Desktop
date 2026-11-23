//! The one-tick bridge between clips: a change of body whose outline jumps passes through the
//! settle for exactly one presentation tick, a loop going round never does, and with motion
//! reduced nothing is bridged.
use super::*;
use crate::renderer::bridge::{bridge_reason, stands};
use formiga_core::{
    AttentionEmotion, AttentionPose, DesktopSnapshot, Presence, PresenceMark, World,
};
use std::cell::RefCell;
use std::collections::HashMap;

/// A frame's outline as the overlay's atlas keeps it, read from the frame itself, and whether the
/// creature has anything in its paws in it just now, as the overlay asks.
fn shapes(appearance: AppearanceGenome) -> impl Fn(&Creature, BodyClip, u8) -> Option<FrameShape> {
    let seen = RefCell::new(HashMap::new());
    move |creature, clip, frame| {
        let (top, bottom, baked) = *seen.borrow_mut().entry((clip, frame)).or_insert_with(|| {
            let rendered = CreatureRenderer::render_body_frame(&appearance, clip, frame, false);
            let (_, top, _, bottom) = rendered.canvas.alpha_bounds().expect("a drawn frame");
            (top as u8, (bottom + 1) as u8, rendered.held)
        });
        Some(FrameShape {
            top,
            bottom,
            holding: held_now(creature, clip, baked).is_some(),
        })
    }
}

fn settle(facing_right: bool) -> BodyPresentation {
    BodyPresentation {
        clip: SETTLE_FRAME.0,
        frame: SETTLE_FRAME.1,
        facing_right,
    }
}

/// One presentation tick: what the creature's body says, its outline, and what is drawn for it.
struct Tick {
    body: BodyPresentation,
    shape: Option<FrameShape>,
    shown: BodyPresentation,
}

/// Two seconds of rest, a second's fright, a rest, four seconds' walk with a fright partway, and
/// a rest again, a tick every twentieth of a second as the overlay presents it, through one
/// `ClipBridge`. A fright from the plain settle has nothing to bridge; one in mid-stride does.
fn run(seed: u8, reduced: bool) -> Vec<Tick> {
    let mut creature = World::preview_adult(
        [seed; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    creature.state.action = ActionKind::Idle;
    creature.state.attention = None;
    creature.state.beat = None;
    creature.state.flourish = None;
    creature.state.facing_right = true;
    let mark = PresenceMark {
        x: creature.state.position.x,
        depth: 0.0,
        window: None,
    };
    let shape = shapes(creature.appearance.clone());
    let mut bridge = ClipBridge::default();
    let mut ticks = Vec::new();
    let mut since = 0.0;
    let mut stride = 0.0;
    for tick in 0..240 {
        let at = tick as f32 * 0.05;
        let walking = (100..180).contains(&tick);
        if tick == 100 || tick == 180 {
            since = 0.0;
        }
        let fright = (40..60).contains(&tick) || (130..145).contains(&tick);
        creature.state.attention = fright.then_some(AttentionPose {
            target: creature.state.position,
            emotion: AttentionEmotion::Startled,
            hanging: 0.0,
            gesture: Some(Gesture::Gasp),
        });
        creature.state.action = if walking {
            ActionKind::Traverse
        } else {
            ActionKind::Idle
        };
        creature.state.action_elapsed = if walking { since } else { at };
        creature.state.presence = Presence {
            clock: 40.0 + at,
            since,
            stepping: walking,
            stride,
            reduced,
            last: Some(mark),
            ..Presence::default()
        };
        let body = BodyPresentation::for_creature(&creature);
        let shape = shape(&creature, body.clip, body.frame);
        bridge.advance(body, shape, reduced);
        ticks.push(Tick {
            body,
            shape,
            shown: bridge.shown(body),
        });
        since += 0.05;
        if walking {
            stride += 0.4;
        }
    }
    ticks
}

#[test]
fn a_jump_in_outline_passes_through_the_settle_exactly_once() {
    for seed in [29_u8, 77, 201] {
        let ticks = run(seed, false);
        let mut bridges = 0;
        let mut wrapped = false;
        for (index, tick) in ticks.iter().enumerate().skip(1) {
            let before = &ticks[index - 1];
            if before.body.clip == tick.body.clip && tick.body.frame < before.body.frame {
                wrapped = true;
            }
            let wanted = before.shown == before.body
                && bridge_reason((before.body, before.shape), (tick.body, tick.shape)).is_some();
            if tick.shown == tick.body {
                assert!(
                    !wanted,
                    "seed {seed}: tick {index} should pass through the settle"
                );
                continue;
            }
            bridges += 1;
            assert!(wanted, "seed {seed}: tick {index} bridged {:?}", tick.body);
            assert_eq!(tick.shown, settle(tick.body.facing_right));
            assert_ne!(
                before.body.clip, tick.body.clip,
                "seed {seed}: never on a loop boundary (tick {index})"
            );
            let after = &ticks[index + 1];
            assert_eq!(after.shown, after.body, "seed {seed}: one tick, no more");
        }
        assert!(wrapped, "seed {seed}: the walk went round its loop");
        assert!(bridges > 0, "seed {seed}: the fright jumps the crown");
    }
}

#[test]
fn reduced_motion_never_bridges() {
    for seed in [29_u8, 77, 201] {
        for (index, tick) in run(seed, true).iter().enumerate() {
            assert_eq!(tick.shown, tick.body, "seed {seed}: tick {index}");
        }
    }
}

#[test]
fn only_a_jump_between_standing_clips_bridges() {
    let at = |clip: BodyClip, frame: u8| BodyPresentation {
        clip,
        frame,
        facing_right: false,
    };
    let outline = |top: u8| {
        Some(FrameShape {
            top,
            bottom: 44,
            holding: false,
        })
    };
    let rest = (at(BodyClip::Action(ActionKind::Idle), 2), outline(12));
    let gasp = (at(BodyClip::Gesture(Gesture::Gasp), 0), outline(4));
    assert_eq!(bridge_reason(rest, gasp), Some(BridgeReason::Silhouette));
    assert_eq!(bridge_reason(gasp, rest), Some(BridgeReason::Silhouette));
    // The feet count as the crown does.
    let stomp = (
        at(BodyClip::Gesture(Gesture::Stomp), 0),
        Some(FrameShape {
            top: 12,
            bottom: 40,
            holding: false,
        }),
    );
    assert_eq!(bridge_reason(rest, stomp), Some(BridgeReason::Silhouette));
    // Within a clip, however far the outline moves: a loop going round.
    assert_eq!(bridge_reason(rest, (at(rest.0.clip, 5), outline(2))), None);
    assert_eq!(
        bridge_reason(
            (at(rest.0.clip, 5), outline(2)),
            (at(rest.0.clip, 0), outline(12))
        ),
        None
    );
    // Only beyond the allowance.
    assert_eq!(
        bridge_reason(rest, (gasp.0, outline(12 - BRIDGE_ROWS))),
        None
    );
    // Not from or to the settle, not with something in the paws, not with an outline unknown.
    let settled = (settle(false), outline(12));
    assert_eq!(bridge_reason(settled, gasp), None);
    assert_eq!(bridge_reason(gasp, settled), None);
    let holding = Some(FrameShape {
        top: 4,
        bottom: 44,
        holding: true,
    });
    assert_eq!(bridge_reason(rest, (gasp.0, holding)), None);
    assert_eq!(bridge_reason(rest, (gasp.0, None)), None);
    assert_eq!(bridge_reason((rest.0, None), gasp), None);
    // Not off its feet or asleep.
    for clip in [
        BodyClip::Action(ActionKind::Dragged),
        BodyClip::Action(ActionKind::Landing),
        BodyClip::Action(ActionKind::Dangle),
        BodyClip::Action(ActionKind::Sleep),
        BodyClip::Gesture(Gesture::Sprawl),
        BodyClip::Gesture(Gesture::Sit),
    ] {
        assert!(!stands(clip), "{clip:?}");
        assert_eq!(bridge_reason(rest, (at(clip, 0), outline(4))), None);
        assert_eq!(bridge_reason((at(clip, 0), outline(4)), rest), None);
    }
    // A bridge lasts one tick, and a change straight after it does not start another.
    let mut bridge = ClipBridge::default();
    assert_eq!(bridge.advance(rest.0, rest.1, false), None);
    assert_eq!(
        bridge.advance(gasp.0, gasp.1, false),
        Some(BridgeReason::Silhouette)
    );
    assert_eq!(bridge.shown(gasp.0), settle(false));
    assert_eq!(bridge.advance(rest.0, rest.1, false), None);
    assert_eq!(bridge.shown(rest.0), rest.0);
    assert_eq!(
        bridge.advance(gasp.0, gasp.1, false),
        Some(BridgeReason::Silhouette)
    );
    assert_eq!(bridge.advance(gasp.0, gasp.1, false), None);
    assert_eq!(bridge.shown(gasp.0), gasp.0);
    // And with motion reduced, never.
    let mut bridge = ClipBridge::default();
    bridge.advance(rest.0, rest.1, true);
    assert_eq!(bridge.advance(gasp.0, gasp.1, true), None);
    assert_eq!(bridge.shown(gasp.0), gasp.0);
}
