//! The one-tick bridge between clips: a change of body whose outline jumps passes through the
//! settle for exactly one presentation tick, a loop going round never does, and with motion
//! reduced nothing is bridged.
use super::*;
use crate::renderer::bridge::{bridge_reason, fidget, stands};
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

/// A companion settled and measured, as the overlay sees one, and the presence it rests with.
fn settled_companion(seed: u8) -> Creature {
    let mut creature = World::preview_adult(
        [seed; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    creature.state.attention = None;
    creature.state.beat = None;
    creature.state.flourish = None;
    creature.state.facing_right = true;
    creature
}

/// `count` presentation ticks, a twentieth of a second each, of `creature` as `step` sets it
/// up for each tick, through one `ClipBridge`.
fn drive(
    mut creature: Creature,
    count: u32,
    reduced: bool,
    mut step: impl FnMut(u32, &mut Creature),
) -> Vec<Tick> {
    let shape = shapes(creature.appearance.clone());
    let mut bridge = ClipBridge::default();
    (0..count)
        .map(|tick| {
            step(tick, &mut creature);
            creature.state.presence.reduced = reduced;
            let body = BodyPresentation::for_creature(&creature);
            let shape = shape(&creature, body.clip, body.frame);
            bridge.advance(body, shape, reduced);
            Tick {
                body,
                shape,
                shown: bridge.shown(body),
            }
        })
        .collect()
}

/// Every tick that shows the settle in place of the body, with why.
fn bridged(ticks: &[Tick]) -> Vec<(usize, Option<BridgeReason>)> {
    ticks
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, tick)| tick.shown != tick.body)
        .map(|(index, tick)| {
            let before = &ticks[index - 1];
            assert_eq!(tick.shown, settle(tick.body.facing_right));
            if let Some(after) = ticks.get(index + 1) {
                assert_eq!(after.shown, after.body, "tick {index}: one tick, no more");
            }
            (
                index,
                bridge_reason((before.body, before.shape), (tick.body, tick.shape)),
            )
        })
        .collect()
}

/// Two minutes resting at `action`, measured and still, its own moments allowed.
fn resting(seed: u8, action: ActionKind, reduced: bool) -> Vec<Tick> {
    let creature = settled_companion(seed);
    let mark = PresenceMark {
        x: creature.state.position.x,
        depth: 0.0,
        window: None,
    };
    drive(creature, 2400, reduced, move |tick, creature| {
        let at = tick as f32 * 0.05;
        creature.state.action = action;
        creature.state.action_elapsed = at;
        creature.state.presence = Presence {
            clock: 40.0 + at,
            since: at,
            last: Some(mark),
            ..Presence::default()
        };
    })
}

#[test]
fn a_moment_of_its_own_begins_and_ends_on_the_settle() {
    let fidgets = |ticks: &[Tick]| {
        ticks
            .windows(2)
            .filter(|pair| {
                pair[0].body.clip != pair[1].body.clip
                    && (fidget(pair[0].body.clip) || fidget(pair[1].body.clip))
            })
            .count()
    };
    let mut at_home = 0;
    for seed in [29_u8, 77, 201, 140] {
        // Resting where it stands, the rest already holds the settle before and after each one,
        // so what is shown either side of a wash, a shake or a sniff is the settle.
        let ticks = resting(seed, ActionKind::Idle, false);
        for (index, pair) in ticks.windows(2).enumerate() {
            let (from, to) = (pair[0].shown, pair[1].shown);
            if from.clip != to.clip && (fidget(from.clip) || fidget(to.clip)) {
                let other = if fidget(from.clip) { to } else { from };
                assert_eq!(
                    (other.clip, other.frame),
                    SETTLE_FRAME,
                    "seed {seed}: tick {index} cuts a moment of its own from {other:?}"
                );
            }
        }
        // Sat on its doorstep, each one starts and ends with a tick on the settle, exactly once.
        let ticks = resting(seed, ActionKind::Homebound, false);
        let changes = fidgets(&ticks);
        let bridges = bridged(&ticks);
        assert_eq!(
            bridges.len(),
            changes,
            "seed {seed}: every start and end of its own moments at home, and nothing else"
        );
        for (index, reason) in bridges {
            // Where the outline jumps as well, that is the reason given; either way it bridges.
            assert!(
                matches!(
                    reason,
                    Some(BridgeReason::Fidget | BridgeReason::Silhouette)
                ),
                "seed {seed}: tick {index} {reason:?}"
            );
            assert_ne!(
                ticks[index - 1].body.clip,
                ticks[index].body.clip,
                "seed {seed}: never on a loop boundary"
            );
        }
        at_home += changes;
        // With motion reduced it has no moments of its own, and nothing is bridged.
        for tick in resting(seed, ActionKind::Homebound, true) {
            assert_eq!(tick.shown, tick.body, "seed {seed}");
        }
    }
    assert!(
        at_home > 0,
        "some companion washed, shook or sniffed at home"
    );
}

#[test]
fn a_turn_passes_through_the_settle_once() {
    // A walk that turns round twice, a flutter of two turns on consecutive ticks, and a turn
    // standing at rest on a beat of the rest.
    for seed in [29_u8, 77, 201] {
        for reduced in [false, true] {
            let creature = settled_companion(seed);
            let mark = PresenceMark {
                x: creature.state.position.x,
                depth: 0.0,
                window: None,
            };
            let ticks = drive(creature, 160, reduced, move |tick, creature| {
                let walking = tick < 120;
                let at = tick as f32 * 0.05;
                creature.state.facing_right = match tick {
                    0..40 => true,
                    40..80 => false,
                    80 => true,
                    81..120 => false,
                    // A rest turned round partway, on a beat.
                    _ => tick < 150,
                };
                creature.state.action = if walking {
                    ActionKind::Traverse
                } else {
                    ActionKind::Idle
                };
                let since = if walking { at } else { at - 6.0 };
                creature.state.action_elapsed = since;
                creature.state.presence = Presence {
                    clock: 40.0 + at,
                    since,
                    stepping: walking,
                    stride: at * 8.0,
                    last: Some(mark),
                    ..Presence::default()
                };
            });
            let bridges = bridged(&ticks);
            if reduced {
                assert!(bridges.is_empty(), "seed {seed}: never with motion reduced");
                continue;
            }
            let turns_at: Vec<usize> = bridges
                .iter()
                .filter(|(_, reason)| *reason == Some(BridgeReason::Turn))
                .map(|(index, _)| *index)
                .collect();
            // The flutter back on tick 81 comes straight after a bridge, and cuts.
            assert_eq!(turns_at[..2], [40, 80], "seed {seed}: {bridges:?}");
            // The rest's turn is bridged unless it falls on the settle itself.
            let rest_turn = &ticks[150];
            let on_settle = (rest_turn.body.clip, rest_turn.body.frame) == SETTLE_FRAME
                || (ticks[149].body.clip, ticks[149].body.frame) == SETTLE_FRAME;
            assert_eq!(
                turns_at.contains(&150),
                !on_settle,
                "seed {seed}: {bridges:?}"
            );
            // A walk going round its loop, facing the same way, never bridges.
            for (index, _) in &bridges {
                let (before, tick) = (&ticks[index - 1], &ticks[*index]);
                assert!(
                    before.body.clip != tick.body.clip
                        || before.body.facing_right != tick.body.facing_right,
                    "seed {seed}: tick {index} on a loop boundary"
                );
            }
        }
    }
}

#[test]
fn fidgets_and_turns_bridge_only_where_they_should() {
    let at = |clip: BodyClip, frame: u8, facing_right: bool| BodyPresentation {
        clip,
        frame,
        facing_right,
    };
    let outline = Some(FrameShape {
        top: 12,
        bottom: 44,
        holding: false,
    });
    let seat = (
        at(BodyClip::Action(ActionKind::Homebound), 1, true),
        outline,
    );
    let wash = (at(BodyClip::Gesture(Gesture::Groom), 0, true), outline);
    let beat = (at(BodyClip::Action(ActionKind::Idle), 3, true), outline);
    // However little the outline moves, a moment of its own starts and ends on the settle.
    assert_eq!(bridge_reason(seat, wash), Some(BridgeReason::Fidget));
    assert_eq!(bridge_reason(wash, seat), Some(BridgeReason::Fidget));
    assert_eq!(bridge_reason(beat, wash), Some(BridgeReason::Fidget));
    // Not from the settle, where it already starts, and not between other clips.
    assert_eq!(bridge_reason((settle(true), outline), wash), None);
    let gasp = (at(BodyClip::Gesture(Gesture::Gasp), 0, true), outline);
    assert_eq!(bridge_reason(seat, gasp), None);
    // A turn, within a clip or across two, resting or walking.
    let walk = |frame, facing| {
        (
            at(BodyClip::Action(ActionKind::Traverse), frame, facing),
            outline,
        )
    };
    assert_eq!(
        bridge_reason(walk(2, true), walk(3, false)),
        Some(BridgeReason::Turn)
    );
    assert_eq!(bridge_reason(walk(5, true), walk(0, true)), None);
    let skip = (at(BodyClip::Gesture(Gesture::Skip), 1, false), outline);
    assert_eq!(bridge_reason(beat, skip), Some(BridgeReason::Turn));
    let flipped_beat = (at(BodyClip::Action(ActionKind::Idle), 4, false), outline);
    assert_eq!(bridge_reason(beat, flipped_beat), Some(BridgeReason::Turn));
    // Not where it turns on the settle, not a twirl, and not with something in its paws.
    assert_eq!(bridge_reason((settle(true), outline), walk(0, false)), None);
    let twirl = |facing| (at(BodyClip::Gesture(Gesture::Cheer), 1, facing), outline);
    assert_eq!(bridge_reason(twirl(true), twirl(false)), None);
    let holding = Some(FrameShape {
        top: 12,
        bottom: 44,
        holding: true,
    });
    assert_eq!(
        bridge_reason(walk(2, true), (walk(3, false).0, holding)),
        None
    );
    // With motion reduced, neither.
    let mut bridge = ClipBridge::default();
    bridge.advance(walk(2, true).0, outline, true);
    assert_eq!(bridge.advance(walk(3, false).0, outline, true), None);
    bridge.advance(seat.0, outline, true);
    assert_eq!(bridge.advance(wash.0, outline, true), None);
}
