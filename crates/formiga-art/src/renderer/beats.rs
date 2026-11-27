//! How a companion looks through one of its small moments — a yawn, a leaf on its face, a turn at
//! the garden or its own door: which pose its body strikes, what its face does, and where it looks,
//! at each point from the moment's start to its end.
//!
//! Everything here reuses poses and expressions the atlas already bakes, apart from the yawn,
//! which has a gesture and a face of its own. A beat is shown in place of the action's clip only
//! while it asks for a pose; for the rest of it, the action — a walk after a snack that rolled
//! away, a shuffle across to the cushion — shows as it always does.

use super::*;
use crate::{COLONY_OBJECT_SIZE, PropSprite};
use formiga_core::{Beat, BeatKind, VillageProp};

/// The pose a beat strikes at this point, if it strikes one, and how far into that pose it is.
/// Each chore has a pose of its own, so a companion watering, picking, tidying or holding up what
/// it grew can be told apart at a glance rather than all reaching for something.
pub(super) fn beat_pose(beat: Beat) -> Option<(BodyClip, f32)> {
    let p = beat.progress();
    let at = |from: f32| (beat.elapsed - from * beat.length).max(0.0);
    let gesture = |gesture: Gesture, into: f32| Some((BodyClip::Gesture(gesture), into));
    let pose = match beat.kind {
        BeatKind::Yawn => return gesture(Gesture::Yawn, beat.elapsed),
        BeatKind::ResistYawn => Gesture::Worry,
        BeatKind::Notice => Gesture::Watch,
        BeatKind::LeafOnFace if p < 0.3 => Gesture::Gasp,
        // Paws up to the face, eyes shut, brushing it off.
        BeatKind::LeafOnFace if p < 0.8 => return gesture(Gesture::Cover, at(0.3)),
        BeatKind::LeafOnFace => return None,
        BeatKind::DroppedSnack if p < 0.35 => Gesture::Gasp,
        BeatKind::DroppedSnack => return None,
        // Bending down for what rolled away and coming up with it.
        BeatKind::Retrieve => Gesture::Pluck,
        BeatKind::MissedCushion if p < 0.3 => Gesture::Crouch,
        BeatKind::MissedCushion if p < 0.55 => return gesture(Gesture::Gasp, at(0.3)),
        BeatKind::MissedCushion => return None,
        BeatKind::Watering => Gesture::Pour,
        BeatKind::Picking => Gesture::Pluck,
        BeatKind::AdjustFlap | BeatKind::TidyLeaves | BeatKind::FluffCushion => Gesture::Tend,
        BeatKind::InspectSprout => Gesture::Crouch,
        // Holding it up in both paws, as a find is held up.
        BeatKind::ShowingOff => {
            return Some((BodyClip::Action(ActionKind::PresentDiscovery), beat.elapsed));
        }
        BeatKind::Admiring | BeatKind::InspectCap => Gesture::Watch,
        BeatKind::RoofSit | BeatKind::Carrying => return None,
        BeatKind::Huff => Gesture::Huff,
        // A start, and then over it goes.
        BeatKind::Swoon if p < 0.18 => Gesture::Gasp,
        BeatKind::Swoon => return gesture(Gesture::Swoon, at(0.18)),
        BeatKind::Beg => Gesture::Beg,
        BeatKind::Strut => Gesture::Strut,
        BeatKind::Peek => Gesture::Peek,
        BeatKind::Stomp => Gesture::Stomp,
        // Wound up low, then up and over.
        BeatKind::Pounce if p < 0.55 => Gesture::Crouch,
        BeatKind::Pounce => return gesture(Gesture::Cheer, at(0.55)),
        BeatKind::Startle => Gesture::Gasp,
        BeatKind::Delight => Gesture::Cheer,
        // A paw out toward the window, then a long look at it.
        BeatKind::PointAt if p < 0.4 => Gesture::Reach,
        BeatKind::PointAt => return gesture(Gesture::Watch, at(0.4)),
        // Down low first, then leaning out to look round the end of the ledge.
        BeatKind::PeekRound if p < 0.25 => Gesture::Crouch,
        BeatKind::PeekRound => return gesture(Gesture::Watch, at(0.25)),
        // Sat as on any window, its legs hanging over the front (`CreatureState::drawn_lift`).
        BeatKind::LedgeSit => {
            return Some((BodyClip::Action(ActionKind::Perch), beat.elapsed));
        }
        // Winding up, a paw out to push, and a bounce of glee.
        BeatKind::Nudge if p < 0.25 => Gesture::Crouch,
        BeatKind::Nudge if p < 0.6 => return gesture(Gesture::Reach, at(0.25)),
        BeatKind::Nudge => return gesture(Gesture::Cheer, at(0.6)),
        // Paws together for a moment, then a long look over: it is all right.
        BeatKind::Relieved if p < 0.35 => Gesture::Worry,
        BeatKind::Relieved => return gesture(Gesture::Watch, at(0.35)),
        // A look over, and a little bounce at the funny side of it.
        BeatKind::Amused if p < 0.4 => Gesture::Watch,
        BeatKind::Amused => return gesture(Gesture::Bop, at(0.4)),
    };
    gesture(pose, beat.elapsed)
}

/// The face a beat wears at this point.
pub(super) fn beat_expression(beat: Beat) -> ExpressionKind {
    let p = beat.progress();
    match beat.kind {
        BeatKind::Yawn if p < 0.2 => ExpressionKind::Content,
        BeatKind::Yawn if p < 0.75 => ExpressionKind::Yawning,
        BeatKind::Yawn => ExpressionKind::Sleepy,
        BeatKind::ResistYawn => ExpressionKind::Determined,
        BeatKind::Notice | BeatKind::InspectSprout | BeatKind::Picking | BeatKind::InspectCap => {
            ExpressionKind::Curious
        }
        BeatKind::LeafOnFace if p < 0.3 => ExpressionKind::Startled,
        BeatKind::LeafOnFace if p < 0.8 => ExpressionKind::Worried,
        BeatKind::DroppedSnack if p < 0.35 => ExpressionKind::Startled,
        BeatKind::DroppedSnack => ExpressionKind::Focused,
        BeatKind::MissedCushion if (0.3..0.55).contains(&p) => ExpressionKind::Startled,
        BeatKind::ShowingOff | BeatKind::Admiring | BeatKind::Carrying => ExpressionKind::Joy,
        BeatKind::AdjustFlap | BeatKind::FluffCushion | BeatKind::TidyLeaves => {
            ExpressionKind::Focused
        }
        // Hmph.
        BeatKind::Huff | BeatKind::Stomp => ExpressionKind::Grumpy,
        BeatKind::Swoon if p < 0.18 => ExpressionKind::Startled,
        // Eyes shut on a smile and blushing, all the way down.
        BeatKind::Swoon => ExpressionKind::Affectionate,
        BeatKind::Peek => ExpressionKind::Worried,
        BeatKind::Beg => ExpressionKind::Pleading,
        BeatKind::Strut => ExpressionKind::Smug,
        BeatKind::Pounce if p < 0.55 => ExpressionKind::Focused,
        BeatKind::Pounce => ExpressionKind::Joy,
        BeatKind::Startle => ExpressionKind::Startled,
        BeatKind::Delight => ExpressionKind::Joy,
        BeatKind::PointAt | BeatKind::PeekRound => ExpressionKind::Curious,
        BeatKind::Nudge if p < 0.6 => ExpressionKind::Smug,
        BeatKind::Nudge => ExpressionKind::Joy,
        BeatKind::Relieved if p < 0.35 => ExpressionKind::Worried,
        BeatKind::Relieved => ExpressionKind::Affectionate,
        BeatKind::Amused if p < 0.4 => ExpressionKind::Curious,
        BeatKind::Amused => ExpressionKind::Joy,
        _ => ExpressionKind::Content,
    }
}

/// The eyelids a beat asks for at this point, where it asks for any in particular.
pub(super) fn beat_eyelids(beat: Beat) -> Option<EyelidPose> {
    let p = beat.progress();
    match beat.kind {
        BeatKind::Yawn if (0.2..0.75).contains(&p) => Some(EyelidPose::Closed),
        BeatKind::Yawn if p >= 0.75 => Some(EyelidPose::Half),
        BeatKind::ResistYawn => Some(EyelidPose::Half),
        BeatKind::LeafOnFace if (0.3..0.8).contains(&p) => Some(EyelidPose::Closed),
        BeatKind::LeafOnFace | BeatKind::DroppedSnack | BeatKind::MissedCushion => {
            Some(EyelidPose::Open)
        }
        BeatKind::Swoon if p >= 0.18 => Some(EyelidPose::Closed),
        // Pleased with itself.
        BeatKind::Strut => Some(EyelidPose::Half),
        // Eyes shut behind the paws, then open for the peek.
        BeatKind::Peek => Some(if p < 0.5 {
            EyelidPose::Closed
        } else {
            EyelidPose::Half
        }),
        BeatKind::Startle | BeatKind::Swoon => Some(EyelidPose::Open),
        _ => None,
    }
}

/// Where a beat looks at this point: at whatever it is about if it names a place, down at the
/// ground for what is done low down, and up for the mushroom cap overhead.
pub(super) fn beat_gaze(creature: &Creature, beat: Beat) -> Option<GazeDirection> {
    let forward = if creature.state.facing_right { 1 } else { -1 };
    match beat.kind {
        BeatKind::InspectCap => return Some(GazeDirection::new(forward, -1)),
        BeatKind::InspectSprout | BeatKind::Retrieve | BeatKind::Picking | BeatKind::Watering => {
            return Some(GazeDirection::new(forward, 1));
        }
        BeatKind::LeafOnFace if beat.progress() < 0.3 => {
            return Some(GazeDirection::new(0, -1));
        }
        // Sat on a window's top, looking down at the desk below its feet.
        BeatKind::LedgeSit => return Some(GazeDirection::new(0, 1)),
        // Nose in the air, looking away from whatever it is put out about.
        BeatKind::Huff => {
            return Some(GazeDirection::new(
                beat.look.map_or(-forward, |target| {
                    -face::axis_direction(target.x - creature.state.position.x, 10.0)
                }),
                -1,
            ));
        }
        // Looking up at whoever has the snack, or whoever is watching.
        BeatKind::Beg | BeatKind::Strut if beat.look.is_none() => {
            return Some(GazeDirection::new(forward, -1));
        }
        _ => {}
    }
    beat.look.map(|target| {
        GazeDirection::new(
            face::axis_direction(target.x - creature.state.position.x, 10.0),
            face::axis_direction(target.y - (creature.state.position.y - 28.0), 10.0),
        )
    })
}

/// What a companion holding `prop` through a village beat has in its paw in `frame` of `clip`:
/// the cell drawn centred on the paw, and, while a pour has the can tipped over, where the water's
/// cell is centred, as an offset in art pixels from the can's middle with the frame facing right.
/// The can tips on the first frame the pour leans into and comes level when the pour does, and
/// the water runs from its spout for just those frames; nothing else held pours.
pub fn village_held(
    prop: VillageProp,
    clip: BodyClip,
    frame: u8,
) -> (PropSprite, Option<(i32, i32)>) {
    let pouring = prop == VillageProp::WateringCan
        && clip == BodyClip::Gesture(Gesture::Pour)
        && pour_tip(frame) > 0;
    if !pouring {
        return (PropSprite::of(prop), None);
    }
    let can = PropSprite::WateringCanTipped;
    let half = COLONY_OBJECT_SIZE as i32 / 2;
    let water = can.spout().map(|(x, y)| (x - half, y - half));
    (can, water)
}
