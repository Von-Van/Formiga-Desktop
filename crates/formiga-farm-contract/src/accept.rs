//! What Desktop keeps of a proposal, and how. This is the whole rule: Desktop runs it on every
//! proposal, however carefully Farm already checked it, and changes a creature only through
//! [`apply_design`], which touches its look and nothing else.

use crate::{
    FarmCapability, FarmDocument, FarmMode, FarmProposal, FarmSnapshot, Lineage, ProposalKind,
    SessionSeal, Verdict,
};
use formiga_core as core;
use formiga_core::forms::{Design, Form};

/// What the colony holds now, for the proposal to be checked against.
#[derive(Clone, Copy, Debug)]
pub enum Current<'a> {
    /// The creature an edit session opened on, as it is now: its record, and whether it is home
    /// to be changed.
    Creature {
        creature: &'a core::Creature,
        available: bool,
    },
    /// The creature an edit session opened on is no longer in the colony.
    Gone,
    /// A create session: there is no creature yet.
    Nobody,
}

/// A proposal Desktop can keep.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Accepted {
    /// Give `target` this design, with [`apply_design`].
    Edit {
        target: core::CreatureId,
        design: Design,
    },
    /// Take this design into Desktop's welcome for a new companion, which gives it an id, a name
    /// and a life of its own, and then [`apply_design`] to the companion it makes.
    Create { design: Design, lineage: Lineage },
}

/// Why Desktop keeps nothing of a proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// It belongs to another session, or to another snapshot.
    WrongSession,
    /// It does not check out.
    Invalid(String),
    /// It asks for something Desktop did not offer, or a design Desktop cannot draw.
    Unsupported,
    /// The creature's look changed after the session opened.
    Stale { current: Design, revision: String },
    /// The creature is out with another companion app.
    Unavailable,
    /// The creature is no longer in the colony.
    Gone,
}

impl Rejection {
    /// What Farm is told.
    pub fn verdict(&self) -> Verdict {
        match self {
            Self::WrongSession | Self::Invalid(_) => Verdict::Invalid,
            Self::Unsupported => Verdict::Unsupported,
            Self::Stale { current, revision } => Verdict::Stale {
                current: current.clone(),
                revision: revision.clone(),
            },
            Self::Unavailable => Verdict::Unavailable,
            Self::Gone => Verdict::Gone,
        }
    }
}

/// Whether Desktop keeps `proposal`, made in the session `snapshot` opened (sealed as `seal`),
/// with the colony as `current` says it is now.
///
/// It keeps nothing from another session; nothing that does not validate; nothing the snapshot
/// did not offer, or that it cannot draw; nothing made from a design the creature no longer has;
/// and nothing for a creature that is away or gone. A look from before recipes can only be kept
/// by a creature that already has it, since nothing can be made into one.
pub fn accept_proposal(
    seal: &SessionSeal,
    snapshot: &FarmSnapshot,
    proposal: &FarmProposal,
    current: Current<'_>,
) -> Result<Accepted, Rejection> {
    if !proposal.answers(seal) || snapshot.session_id != seal.session_id {
        return Err(Rejection::WrongSession);
    }
    proposal
        .validate()
        .map_err(|error| Rejection::Invalid(error.to_string()))?;
    if !snapshot.can_keep(&proposal.design) {
        return Err(Rejection::Unsupported);
    }
    match (&proposal.kind, &snapshot.mode) {
        (
            ProposalKind::EditExisting {
                target,
                expected_revision,
            },
            FarmMode::EditExisting { creature: opened },
        ) => {
            if !snapshot.offers(FarmCapability::EditExisting) {
                return Err(Rejection::Unsupported);
            }
            if *target != opened.id {
                return Err(Rejection::Invalid(
                    "the proposal is for another creature".into(),
                ));
            }
            let (creature, available) = match current {
                Current::Creature {
                    creature,
                    available,
                } => (creature, available),
                Current::Gone => return Err(Rejection::Gone),
                Current::Nobody => {
                    return Err(Rejection::Invalid("an edit with no creature".into()));
                }
            };
            if creature.id != target.0 {
                return Err(Rejection::Invalid(
                    "the colony's creature is not the one proposed for".into(),
                ));
            }
            let now = Design::of(&creature.appearance);
            let revision = now.revision();
            if revision != *expected_revision {
                return Err(Rejection::Stale {
                    current: now,
                    revision,
                });
            }
            if !available {
                return Err(Rejection::Unavailable);
            }
            if matches!(proposal.design.form, Form::Original)
                && creature.appearance.design.is_some()
            {
                return Err(Rejection::Invalid(
                    "only a creature that already has its original look can keep it".into(),
                ));
            }
            Ok(Accepted::Edit {
                target: creature.id,
                design: proposal.design.clone(),
            })
        }
        (ProposalKind::CreateNew, FarmMode::Create { .. }) => {
            if !snapshot.offers(FarmCapability::CreateNew) {
                return Err(Rejection::Unsupported);
            }
            if matches!(proposal.design.form, Form::Original) {
                return Err(Rejection::Invalid(
                    "a new companion cannot be given an original look".into(),
                ));
            }
            Ok(Accepted::Create {
                design: proposal.design.clone(),
                lineage: proposal.lineage.clone(),
            })
        }
        _ => Err(Rejection::Unsupported),
    }
}

/// Give `creature` `design`, changing its look and nothing else: its recipe (kept in its origin
/// too, as Desktop always keeps it), its sculpt if it has one, and its face's features. Any
/// sculpt it had before is let go.
///
/// A sculpted creature keeps a companion recipe as well ([`Design::fallback_recipe`]), so any
/// reader that cannot draw sculpts (an older Hill or Home, a share code) still draws something
/// in its colours rather than nothing.
///
/// Its id, name, temperament, family, habits, memories, relationships and history are never
/// touched: they are not part of a design.
pub fn apply_design(creature: &mut core::Creature, design: &Design) {
    let design = design.normalized();
    // A look from before recipes is only ever kept by a creature that has it already
    // (`accept_proposal` refuses anything else), so its recipe, none, is left as it is.
    if !matches!(design.form, Form::Original) {
        let recipe = design.fallback_recipe(creature.appearance.design);
        core::apply_creature_design(creature, recipe);
    }
    creature.appearance.face = design.face;
    creature.appearance.sculpt = design.form.sculpt().cloned();
}
