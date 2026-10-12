//! A creature's form: the semantic design Formiga Farm edits, and Desktop keeps.
//!
//! A [`Design`] is everything about how a creature looks and nothing about who it is. Its
//! [`Form`] is one of three:
//!
//! - **Original**: the look of a companion made before recipes, drawn from its genes;
//! - **Companion**: one of the five companion plans, exactly as its recipe
//!   ([`crate::CreatureDesign`]) says;
//! - **Sculpted**: a [`Sculpt`] on one of the newer [`Plan`]s, quadrupeds large and small, an
//!   upright body, a floater, a crawler and a percher, with parts fitted to named slots, a coat
//!   and its markings.
//!
//! A creature keeps its design in its [`crate::AppearanceGenome`]: the recipe as always, and a
//! sculpt beside it when it has one. A sculpted creature's recipe is the companion plan nearest
//! its form ([`Design::fallback_recipe`]), so anything that cannot draw sculpts (an older Formiga
//! Hill or Home, a share code) shows something in its colours rather than nothing. `formiga-art`
//! draws the sculpt itself, at the same 48-pixel size and wearing the same faces as a companion.
//!
//! Every value in a design is small, bounded and named for what it does. A design read from
//! anywhere else is checked with [`Design::validate`] and refused rather than repaired, and
//! [`Design::revision`] names it exactly, so a change proposed against one revision can be told
//! apart from a creature that has changed since.

mod design;
mod sculpt;

pub use design::{Design, Form, bounded_face, face_carrier, plain_face};
pub use sculpt::{
    Coat, Dimension, Ink, Locomotion, MAX_MARKINGS, MIDDLE, Marking, MarkingKind, NUDGE, Part,
    PartKind, Plan, STEPS, Sculpt, Shape, Slot, Treatment,
};

/// The version of the design model this build reads and writes: which plans, parts, markings
/// and treatments exist, and what each value means. A design from a newer model may name
/// something this build does not know, and is refused rather than guessed at.
pub const DESIGN_VERSION: u32 = 1;

/// Why a design cannot be used.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FormError {
    #[error("the design is not usable: {0}")]
    Invalid(String),
}
