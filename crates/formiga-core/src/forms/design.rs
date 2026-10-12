//! A creature's whole design: its form, and the features of its face that every form shares.

use super::{FormError, Sculpt};
use crate::{
    AppearanceGenome, BrowStyle, CheekStyle, CreatureDesign, EyeShape, FaceGenome, HighlightStyle,
    MouthStyle, PupilStyle,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The body a design is drawn with.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Form {
    /// The look a companion made before recipes existed, drawn from its genes exactly as Desktop
    /// draws it. Only a creature that already looks this way can keep it; nothing can be made
    /// into one.
    Original,
    /// One of Desktop's five companion plans, exactly as its recipe says, drawn by Desktop's own
    /// renderer. Every creature with a recipe arrives as one of these and, unless the owner
    /// changes its body, leaves as one, so nothing about how it was drawn can drift.
    Companion { recipe: CreatureDesign },
    /// A sculpted form on one of the newer body plans.
    Sculpted { sculpt: Sculpt },
}

impl Form {
    pub fn sculpt(&self) -> Option<&Sculpt> {
        match self {
            Self::Sculpted { sculpt } => Some(sculpt),
            _ => None,
        }
    }

    pub fn recipe(&self) -> Option<CreatureDesign> {
        match self {
            Self::Companion { recipe } => Some(*recipe),
            _ => None,
        }
    }
}

/// Everything about how a creature looks that Formiga Farm can change, and nothing about who it
/// is. A creature's name, temperament, family, habits, relationships and history are kept apart
/// from its design, so changing one can never touch the other.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Design {
    pub form: Form,
    /// The features of its face: eye shape and size, brows, mouth and cheeks. Its expressions
    /// are drawn from these by Formiga's own face renderer, whatever body they are on.
    pub face: FaceGenome,
}

impl Design {
    /// The design a creature has now, from its genome and the sculpt kept in it.
    pub fn of(genome: &AppearanceGenome) -> Self {
        let form = match (&genome.sculpt, genome.design) {
            (Some(sculpt), _) => Form::Sculpted {
                sculpt: sculpt.clone(),
            },
            (None, Some(recipe)) => Form::Companion { recipe },
            (None, None) => Form::Original,
        };
        Self {
            form,
            face: genome.face,
        }
    }

    /// A short, stable name for exactly this design, so that a proposal can say which design it
    /// was made from, and Desktop can tell whether the creature has changed since.
    pub fn revision(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("a design always serializes");
        let digest = Sha256::digest(&bytes);
        digest.iter().take(16).map(|b| format!("{b:02x}")).collect()
    }

    /// The same design with every value in range: what the renderer draws, whatever it is given.
    pub fn normalized(&self) -> Self {
        let form = match &self.form {
            Form::Original => Form::Original,
            Form::Companion { recipe } => Form::Companion {
                recipe: recipe.bounded(),
            },
            Form::Sculpted { sculpt } => Form::Sculpted {
                sculpt: sculpt.normalized(),
            },
        };
        Self {
            form,
            face: bounded_face(self.face),
        }
    }

    /// Whether the design is exactly as [`Self::normalized`] leaves it. A design that comes from
    /// anywhere else is checked with this and refused, never repaired.
    pub fn validate(&self) -> Result<(), FormError> {
        if let Form::Sculpted { sculpt } = &self.form {
            sculpt.validate()?;
        }
        if let Form::Companion { recipe } = &self.form
            && *recipe != recipe.bounded()
        {
            return Err(FormError::Invalid(
                "the companion recipe is out of range".into(),
            ));
        }
        if self.face != bounded_face(self.face) {
            return Err(FormError::Invalid(
                "the face's features are out of range".into(),
            ));
        }
        Ok(())
    }

    /// The companion recipe a creature with this design keeps for every reader that cannot draw
    /// it: the recipe itself for a companion; for a sculpted form, the companion plan nearest its
    /// own in its colours, starting from the recipe it had before (`previous`), so an older Hill or
    /// Home still shows a creature that looks something like it rather than nothing at all. A look
    /// from before recipes keeps having none.
    pub fn fallback_recipe(&self, previous: Option<CreatureDesign>) -> Option<CreatureDesign> {
        use crate::BodyPlan;
        match &self.form {
            Form::Original => None,
            Form::Companion { recipe } => Some(recipe.bounded()),
            Form::Sculpted { sculpt } => {
                let mut recipe = previous.unwrap_or_else(|| face_carrier(sculpt));
                recipe.body = match sculpt.plan {
                    super::Plan::CompactQuadruped
                    | super::Plan::LargeQuadruped
                    | super::Plan::TallQuadruped => BodyPlan::Long,
                    super::Plan::Upright => BodyPlan::Upright,
                    super::Plan::Percher => BodyPlan::Winged,
                    super::Plan::Floater | super::Plan::Crawler => BodyPlan::Round,
                };
                recipe.coat = sculpt.coat.primary;
                recipe.accent = sculpt.coat.secondary;
                if recipe.archetype == 0 {
                    recipe.archetype = crate::BodyArchetype::for_body(recipe.body, false).number();
                }
                recipe.face_template = sculpt.face_template;
                Some(recipe.bounded())
            }
        }
    }

    /// `base` with this design put on it: the genome a creature with this design keeps, and
    /// Formiga's renderer draws. Everything the design does not say (size, gait, the seed its
    /// small marks are laid out from) stays as `base` has it. A sculpted form is kept whole
    /// beside the companion recipe nearest it ([`Self::fallback_recipe`]), which every reader
    /// that cannot draw sculpts draws instead.
    pub fn genome(&self, base: &AppearanceGenome) -> AppearanceGenome {
        let mut genome = base.clone();
        genome.face = bounded_face(self.face);
        genome.design = self.fallback_recipe(base.design);
        genome.sculpt = self.form.sculpt().map(Sculpt::normalized);
        genome
    }
}

/// The recipe a sculpted form's face is drawn from: its face layout, and soft colours from its
/// coat so the face sits in the same light. Formiga's face renderer reads a recipe for its
/// layout and palette and nothing else, so a sculpted form wears exactly the faces a companion
/// does and keeps every one of its expressions.
pub fn face_carrier(sculpt: &Sculpt) -> CreatureDesign {
    let mut recipe = CreatureDesign::generated([7; 32], 0, None);
    recipe.coat = sculpt.coat.primary;
    recipe.accent = sculpt.coat.accent;
    recipe.classic = Default::default();
    recipe.details = Default::default();
    recipe.archetype = recipe.archetype.max(1);
    recipe.face_template = sculpt.face_template.clamp(1, crate::FACE_TEMPLATES);
    // Not a blob, so the layout is worn as chosen rather than moved down a row.
    recipe.body = crate::BodyPlan::Round;
    recipe.bounded()
}

/// A face's features inside the ranges Desktop generates them in, so every face drawn is one
/// the face renderer was made for.
pub fn bounded_face(face: FaceGenome) -> FaceGenome {
    FaceGenome {
        eye_size: face.eye_size.clamp(1, 2),
        eye_spacing: face.eye_spacing.clamp(4, 7),
        vertical_offset: face.vertical_offset.clamp(-1, 1),
        ..face
    }
}

/// A plain face, for a design made from nothing.
pub fn plain_face() -> FaceGenome {
    FaceGenome {
        eye_shape: EyeShape::Round,
        eye_size: 2,
        eye_spacing: 5,
        vertical_offset: 0,
        pupil_style: PupilStyle::Dot,
        highlight_style: HighlightStyle::Single,
        brow_style: BrowStyle::None,
        mouth_style: MouthStyle::Smile,
        cheek_style: CheekStyle::Blush,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forms::Plan;

    #[test]
    fn a_revision_changes_with_anything_about_the_design_and_nothing_else() {
        let design = Design {
            form: Form::Sculpted {
                sculpt: Sculpt::starter(Plan::Floater),
            },
            face: plain_face(),
        };
        assert_eq!(design.revision(), design.clone().revision());
        assert_eq!(design.revision().len(), 32);
        let mut changed = design.clone();
        if let Form::Sculpted { sculpt } = &mut changed.form {
            sculpt.coat.primary[0] ^= 1;
        }
        assert_ne!(design.revision(), changed.revision());
        let mut face = design.clone();
        face.face.eye_spacing = 6;
        assert_ne!(design.revision(), face.revision());
    }

    #[test]
    fn a_face_out_of_range_is_refused() {
        let mut design = Design {
            form: Form::Original,
            face: plain_face(),
        };
        assert_eq!(design.validate(), Ok(()));
        design.face.eye_spacing = 40;
        assert!(design.validate().is_err());
        assert_eq!(design.normalized().validate(), Ok(()));
    }

    #[test]
    fn a_sculpted_face_carrier_is_a_valid_recipe_wearing_the_chosen_layout() {
        for template in 1..=crate::FACE_TEMPLATES {
            let mut sculpt = Sculpt::starter(Plan::Upright);
            sculpt.face_template = template;
            let carrier = face_carrier(&sculpt);
            assert_eq!(carrier, carrier.bounded());
            assert_eq!(carrier.face_template, template);
        }
    }
}
