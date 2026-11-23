//! The one-tick bridge between clips: where the body changes from one clip to another whose
//! outline sits markedly higher or lower — a walk stopping on a high stride, a rest breaking into
//! a cheer — where one of its own moments at rest starts or ends away from the settle, or where
//! it turns round while resting or walking, it passes through the plain settle for one
//! presentation tick on the way, so the change reads as a movement rather than a cut.
//!
//! This is presentation only. [`BodyPresentation::for_creature`] stays a pure function of the
//! creature; the memory of what was shown last is a [`ClipBridge`] the overlay keeps beside each
//! companion, outside the world and its save. The bridge is drawn from frames every atlas already
//! has, and none is added. What a frame's outline is comes from the caller, which reads it from
//! the baked atlas ([`FrameShape`]), so the measure is the picture actually on the desktop.
//!
//! Every reason a change passes through the settle is one arm of [`bridge_reason`], and each
//! names itself in [`BridgeReason`], so a new reason is a new arm and a new variant.
use super::{BodyClip, BodyPresentation};
use formiga_core::{ActionKind, Gesture};

/// The frame every bridge passes through: the first frame of the rest, the plain square settle.
pub const SETTLE_FRAME: (BodyClip, u8) = (BodyClip::Action(ActionKind::Idle), 0);

/// How many rows a frame's crown or feet may move, from the frame shown last to the first frame
/// of the next clip, and still cut straight across. A walk's own bob moves the crown by as much
/// as four rows from one stride to the next without a bridge, so a change of clip has to move it
/// further than that before a cut shows as one.
pub const BRIDGE_ROWS: u8 = 3;

/// What the bridge needs to know of one frame as it is shown: the topmost and just-past-the-lowest
/// rows it draws, in art pixels from the frame's top edge, as the baked atlas keeps them, and
/// whether something is in its paws or mouth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameShape {
    pub top: u8,
    pub bottom: u8,
    /// Whether the companion has something in its paws or mouth in this frame just now. A frame
    /// holding something never bridges: the settle holds nothing, so the snack or the toy would
    /// wink out for the tick.
    pub holding: bool,
}

/// Why a change of body passes through the settle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BridgeReason {
    /// The next clip's outline is more than [`BRIDGE_ROWS`] higher or lower at the crown or the
    /// feet than the frame shown last.
    Silhouette,
    /// One of a companion's own moments at rest — a wash, a shake, a sniff of the air — starts
    /// from, or goes back to, a resting body that is not the settle: sat on its doorstep, or on
    /// a beat of the rest. However little the outline moves, the moment begins and ends on the
    /// settle, as it does where it is taken from the settle itself.
    Fidget,
    /// The body turns round, resting or walking: it faces the other way on this tick from the
    /// last. It passes through the screen-facing settle on the way, so a turn is a turn rather
    /// than a mirror.
    Turn,
}

/// One companion's memory of the body it was last shown in, advanced once a presentation tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClipBridge {
    /// What [`BodyPresentation::for_creature`] said on the last tick, before any bridge, and the
    /// outline of that frame as it was then.
    last: Option<(BodyPresentation, Option<FrameShape>)>,
    /// Why this tick shows the settle in place of the body, while it does.
    bridging: Option<BridgeReason>,
}

impl ClipBridge {
    /// Moves on one presentation tick to `body`, what the creature's body shows now, whose frame
    /// has the outline `shape` (`None` where it is not known yet, and nothing is bridged), and
    /// says whether this tick shows the settle in its place. With motion reduced nothing is
    /// bridged. A bridge lasts exactly one tick, and a change on the tick straight after one
    /// never starts another: the body has just been through the settle.
    pub fn advance(
        &mut self,
        body: BodyPresentation,
        shape: Option<FrameShape>,
        reduced: bool,
    ) -> Option<BridgeReason> {
        let just_bridged = self.bridging.is_some();
        let last = self.last.replace((body, shape));
        self.bridging = match last {
            Some(last) if !reduced && !just_bridged => bridge_reason(last, (body, shape)),
            _ => None,
        };
        self.bridging
    }

    /// Why this tick shows the settle, if it does.
    pub fn bridging(&self) -> Option<BridgeReason> {
        self.bridging
    }

    /// What to draw for `body` this tick: the settle, facing the same way, while a bridge is
    /// on, and `body` itself otherwise. The overlay, the bubbles and the grab mask all draw
    /// this, so the settle is seen and grabbed alike.
    pub fn shown(&self, body: BodyPresentation) -> BodyPresentation {
        if self.bridging.is_none() {
            return body;
        }
        BodyPresentation {
            clip: SETTLE_FRAME.0,
            frame: SETTLE_FRAME.1,
            facing_right: body.facing_right,
        }
    }
}

/// Why going from `from`, the body shown last, to `to` passes through the settle, if it does,
/// each with its frame's outline.
///
/// Never from or to the settle itself, which would only hold it a tick longer; never from or to
/// a body that is off its feet or asleep (see [`stands`]); never from or to a frame holding
/// something; and never where either frame's outline is not known. Within one clip only a turn
/// does (see [`turns`]), so a loop going round or a clip holding its last frame cuts as it always
/// has. Between two clips, a jump in the outline does, and so does one of its own moments at
/// rest starting or ending (see [`fidget`]) or a turn.
pub fn bridge_reason(
    (from, before): (BodyPresentation, Option<FrameShape>),
    (to, after): (BodyPresentation, Option<FrameShape>),
) -> Option<BridgeReason> {
    if (from.clip, from.frame) == SETTLE_FRAME
        || (to.clip, to.frame) == SETTLE_FRAME
        || !stands(from.clip)
        || !stands(to.clip)
    {
        return None;
    }
    let (before, after) = (before?, after?);
    if before.holding || after.holding {
        return None;
    }
    let turned = from.facing_right != to.facing_right && turns(from.clip) && turns(to.clip);
    if from.clip == to.clip {
        return turned.then_some(BridgeReason::Turn);
    }
    let moved = before
        .top
        .abs_diff(after.top)
        .max(before.bottom.abs_diff(after.bottom));
    if moved > BRIDGE_ROWS {
        Some(BridgeReason::Silhouette)
    } else if (fidget(from.clip) && rests(to.clip)) || (rests(from.clip) && fidget(to.clip)) {
        Some(BridgeReason::Fidget)
    } else {
        turned.then_some(BridgeReason::Turn)
    }
}

/// Whether a clip is one of the things a companion does with itself at rest: a wash, a shake or
/// a sniff of the air (see [`super::MotionSignature::rest_moment`]).
pub fn fidget(clip: BodyClip) -> bool {
    matches!(
        clip,
        BodyClip::Gesture(Gesture::Groom | Gesture::Shake | Gesture::Sniff)
    )
}

/// Whether a clip is a body at rest, where one of its own moments starts and ends: the rest and
/// its beats, sat on its doorstep, or up on a window.
fn rests(clip: BodyClip) -> bool {
    matches!(
        clip,
        BodyClip::Action(ActionKind::Idle | ActionKind::Homebound | ActionKind::Perch)
    )
}

/// Whether a clip is a body resting or walking, whose turning round passes through the settle:
/// the rest, sat on its doorstep or up on a window, and every walk and run, moods and all. A
/// twirl is a turn done on purpose, in a cheer, and stays as quick as it is.
pub fn turns(clip: BodyClip) -> bool {
    rests(clip)
        || matches!(
            clip,
            BodyClip::Action(ActionKind::Traverse | ActionKind::Follow | ActionKind::Sprint)
                | BodyClip::Gesture(Gesture::Skip | Gesture::Trudge)
        )
}

/// Whether a clip has the body up on its feet, where the standing settle can go between it and
/// another. Held in the hand, thrown, climbing or hanging it is off the ground; a landing is an
/// impact, which has to land on its tick; asleep, a sit up in the settle would read as waking;
/// sat on a wonder, the settle would stand in the seat.
pub fn stands(clip: BodyClip) -> bool {
    match clip {
        BodyClip::Action(action) => !matches!(
            action,
            ActionKind::Dragged
                | ActionKind::Tossed
                | ActionKind::Landing
                | ActionKind::ClimbWindow
                | ActionKind::Dangle
                | ActionKind::Sleep
        ),
        BodyClip::Gesture(gesture) => !matches!(
            gesture,
            Gesture::Sprawl | Gesture::Loaf | Gesture::Sit | Gesture::Scoot
        ),
    }
}
