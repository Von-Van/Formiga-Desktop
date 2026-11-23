//! The blink-and-change: where a companion's face changes from one expression to another that
//! looks different both above and at the mouth, it shows one presentation tick of the plain face
//! with its lids half down on the way, so the new face comes in behind a blink, as a face changes
//! in life, rather than swapping whole in one frame. A change of only the eyes or only the mouth
//! still cuts straight across: there is nothing for a blink to hide.
//!
//! This is presentation only. The expression a creature wears stays a pure function of the
//! creature (`resolve_expression`); the memory of what it wore last is a [`FaceChange`] the app
//! keeps beside each companion and hands every overlay, outside the world and its save. The
//! in-between face is a cell every face texture already has, and none is added.
use super::face::{FaceDrawing, face_drawing, resolve_expression};
use super::{ExpressionKind, EyelidPose, FaceRenderState};
use formiga_core::{AppearanceGenome, Creature, MouthStyle};

/// The face every blink-and-change passes through: the plain face, lids half down.
pub const FACE_CHANGE_FACE: (ExpressionKind, EyelidPose) =
    (ExpressionKind::Neutral, EyelidPose::Half);

/// One companion's memory of the expression it wore last, advanced once a presentation tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FaceChange {
    /// The expression the creature resolved to on the last tick, before any blink.
    last: Option<ExpressionKind>,
    /// Whether this tick shows the in-between face in place of the expression.
    changing: bool,
}

impl FaceChange {
    /// Moves on one presentation tick to the expression `creature` wears now, and says whether
    /// this tick shows the in-between face in its place (see [`Self::advance_to`]).
    pub fn advance(&mut self, creature: &Creature, reduced: bool) -> bool {
        self.advance_to(&creature.appearance, resolve_expression(creature), reduced)
    }

    /// Moves on one presentation tick to `expression`, worn by a creature that looks like
    /// `genome`, and says whether this tick shows the in-between face. It lasts exactly one tick;
    /// a change on the tick straight after one never starts another, as the face has just been
    /// through a blink; and with motion reduced nothing passes through it, so a change is the one
    /// swap it always was and no third face flickers between the two.
    pub fn advance_to(
        &mut self,
        genome: &AppearanceGenome,
        expression: ExpressionKind,
        reduced: bool,
    ) -> bool {
        let just_changed = self.changing;
        let last = self.last.replace(expression);
        self.changing = match last {
            Some(last) if !reduced && !just_changed && last != expression => {
                blinks_between(face_drawing(genome), last, expression)
            }
            _ => false,
        };
        self.changing
    }

    /// Whether this tick shows the in-between face.
    pub fn changing(&self) -> bool {
        self.changing
    }

    /// What to draw for `face` this tick: the in-between face, looking the same way, while a
    /// blink-and-change is on, and `face` itself otherwise.
    pub fn shown(&self, face: FaceRenderState) -> FaceRenderState {
        if !self.changing {
            return face;
        }
        FaceRenderState {
            expression: FACE_CHANGE_FACE.0,
            eyelids: FACE_CHANGE_FACE.1,
            gaze: face.gaze,
        }
    }
}

/// Whether a face drawn as `drawing` passes through a blink going from `from` to `to`: where the
/// two differ both above the mouth and at it. Never into a fright, which has to land on its tick.
pub(super) fn blinks_between(
    drawing: FaceDrawing,
    from: ExpressionKind,
    to: ExpressionKind,
) -> bool {
    from != to
        && to != ExpressionKind::Startled
        && eyes(drawing, from) != eyes(drawing, to)
        && mouth(drawing, from) != mouth(drawing, to)
}

/// What a face drawn as `drawing` shows above the mouth wearing `expression` with its eyes open,
/// as a key: two expressions with the same key look the same there. It follows the drawing:
/// an authored layout puts the expression in the eyes' own shape and shine (`EyeLook::of`,
/// `draw_layout_eye`, where focused looks ahead), the classic face
/// in its brows and a few eyes moved or shining (`draw_brows`, `draw_eye`, `draw_face`), and the
/// fox in five kinds of eye.
pub(super) fn eyes(drawing: FaceDrawing, expression: ExpressionKind) -> u32 {
    use ExpressionKind as E;
    match drawing {
        FaceDrawing::Layout { .. } => match expression {
            E::Neutral | E::Content => 0,
            E::Joy | E::Affectionate => 1,
            _ => 2 + expression.index(),
        },
        FaceDrawing::Classic { browless, .. } => {
            let brows = match expression {
                E::Neutral | E::Content | E::Sleepy if browless => 0,
                E::Worried | E::Affectionate | E::Pleading => 1,
                E::Focused | E::Determined | E::Yawning | E::Grumpy => 2,
                E::Startled | E::Curious => 3,
                E::Bored | E::Sleepy => 4,
                _ => 5,
            };
            let eyes = match expression {
                E::Worried => 1,
                E::Curious => 2,
                E::Yawning => 3,
                E::Pleading => 4,
                _ => 0,
            };
            brows * 8 + eyes
        }
        FaceDrawing::Fox => match expression {
            E::Sleepy | E::Yawning => 0,
            E::Joy | E::Affectionate => 1,
            E::Bored | E::Grumpy | E::Smug => 2,
            E::Startled => 3,
            _ => 4,
        },
    }
}

/// What a face drawn as `drawing` shows at the mouth wearing `expression`, as a key: two
/// expressions with the same key draw the same mouth (`draw_layout_mouth`, `draw_mouth`).
pub(super) fn mouth(drawing: FaceDrawing, expression: ExpressionKind) -> u32 {
    use ExpressionKind as E;
    let own = 8 + expression.index();
    match drawing {
        FaceDrawing::Layout {
            mouth: MouthStyle::Beak,
        } => match expression {
            E::Yawning => 0,
            E::Joy | E::Startled => 1,
            E::Content | E::Affectionate | E::Smug => 2,
            _ => 3,
        },
        FaceDrawing::Layout { .. } => match expression {
            E::Focused | E::Determined => 0,
            _ => own,
        },
        FaceDrawing::Classic {
            mouth: MouthStyle::Tiny,
            ..
        } if expression == E::Pleading => 8 + E::Neutral.index(),
        FaceDrawing::Classic { .. } => own,
        FaceDrawing::Fox => u32::from(expression == E::Yawning),
    }
}
