//! How a companion looks through one of its small moments — a yawn, a leaf on its face, a turn at
//! the garden or its own door: which pose its body strikes, what its face does, and where it looks,
//! at each point from the moment's start to its end.
//!
//! Everything here reuses poses and expressions the atlas already bakes, apart from the yawn,
//! which has a gesture and a face of its own. A beat is shown in place of the action's clip only
//! while it asks for a pose; for the rest of it, the action — a walk after a snack that rolled
//! away, a shuffle across to the cushion — shows as it always does.

use super::*;
use formiga_core::{Beat, BeatKind};

/// The pose a beat strikes at this point, if it strikes one, and how far into that pose it is.
pub(super) fn beat_pose(beat: Beat) -> Option<(Gesture, f32)> {
    let p = beat.progress();
    let at = |from: f32| (beat.elapsed - from * beat.length).max(0.0);
    let gesture = match beat.kind {
        BeatKind::Yawn => return Some((Gesture::Yawn, beat.elapsed)),
        BeatKind::ResistYawn => Gesture::Worry,
        BeatKind::Notice => Gesture::Watch,
        BeatKind::LeafOnFace if p < 0.3 => Gesture::Gasp,
        // Paws up to the face, eyes shut, brushing it off.
        BeatKind::LeafOnFace if p < 0.8 => return Some((Gesture::Cover, at(0.3))),
        BeatKind::LeafOnFace => return None,
        BeatKind::DroppedSnack if p < 0.35 => Gesture::Gasp,
        BeatKind::DroppedSnack => return None,
        BeatKind::Retrieve => Gesture::Crouch,
        BeatKind::MissedCushion if p < 0.3 => Gesture::Crouch,
        BeatKind::MissedCushion if p < 0.55 => return Some((Gesture::Gasp, at(0.3))),
        BeatKind::MissedCushion => return None,
        BeatKind::Watering
        | BeatKind::Picking
        | BeatKind::AdjustFlap
        | BeatKind::TidyLeaves
        | BeatKind::FluffCushion => Gesture::Reach,
        BeatKind::InspectSprout => Gesture::Crouch,
        BeatKind::ShowingOff => Gesture::Reach,
        BeatKind::Admiring | BeatKind::InspectCap => Gesture::Watch,
        BeatKind::RoofSit | BeatKind::Carrying => return None,
    };
    Some((gesture, beat.elapsed))
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
        _ => {}
    }
    beat.look.map(|target| {
        GazeDirection::new(
            face::axis_direction(target.x - creature.state.position.x, 10.0),
            face::axis_direction(target.y - (creature.state.position.y - 28.0), 10.0),
        )
    })
}
