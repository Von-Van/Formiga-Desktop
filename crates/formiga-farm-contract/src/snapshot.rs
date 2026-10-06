//! What Desktop tells Farm when it opens a session: whether a new creature is being drawn or an
//! existing one reshaped, what that one looks like now, what Desktop can draw and keep, and how
//! the owner likes things shown. Nothing about the creature's temperament, habits, friends,
//! family or history, and nothing about the desktop it lives on.

use crate::SNAPSHOT_FORMAT;
use crate::document::{FarmDocument, FarmError, header_ok, is_lower_hex};
use crate::limits::*;
use formiga_core as core;
use formiga_core::forms::{self, Design, Plan, Sculpt};
use formiga_expansion_rulebook::{SessionId, is_sanitized};
use formiga_travel::{Presentation, TravelAccessory, TravelAppearance, TravelRole, TravelerId};
use serde::{Deserialize, Deserializer, Serialize};
use time::OffsetDateTime;

/// What Desktop will do with Farm's proposals. Farm only proposes what is listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FarmCapability {
    /// Desktop keeps a new design for the creature the session opened on.
    EditExisting,
    /// Desktop takes a design into its own welcome for a new companion, which names it and
    /// gives it a life of its own.
    CreateNew,
    /// Desktop can keep and draw sculpted forms, on the plans [`Renderer::plans`] names. Without
    /// it, only companion recipes (and a look from before recipes) can be proposed.
    SculptedForms,
    /// Desktop notes a kept change in its journal, in its own words.
    JournalNote,
    /// Anything a newer Desktop offers that this build does not know.
    #[serde(other)]
    Unknown,
}

/// What Desktop's renderer can draw.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Renderer {
    /// The design model Desktop reads: [`forms::DESIGN_VERSION`] when Desktop is built with the
    /// same.
    pub design_version: u32,
    /// The sculpted plans it can draw. A plan a newer Desktop names that this build does not know
    /// is left out as it is read.
    #[serde(deserialize_with = "known_plans")]
    pub plans: Vec<Plan>,
}

impl Renderer {
    /// Everything this build can draw.
    pub fn this_build() -> Self {
        Self {
            design_version: forms::DESIGN_VERSION,
            plans: Plan::ALL.to_vec(),
        }
    }
}

fn known_plans<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Plan>, D::Error> {
    let values: Vec<serde_json::Value> = Vec::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect())
}

/// Whether Desktop could keep a change to the creature now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    /// Out with another companion app, on a trip or at home. Farm may go on drafting, but a
    /// change waits until it is back.
    Away,
    #[serde(other)]
    Unknown,
}

/// The creature a session opened on, as far as Farm needs it to draw it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FarmCreature {
    pub id: TravelerId,
    /// For the window's title, and nothing else.
    pub name: String,
    pub role: TravelRole,
    /// Its look now, exactly as a trip would carry it: for a sculpted form, the companion recipe
    /// kept beside it.
    pub appearance: TravelAppearance,
    /// Its sculpted form, if it has one, which a trip leaves behind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sculpt: Option<Sculpt>,
    /// [`Design::revision`] of its design now. A proposal names this, and Desktop keeps it only
    /// if the creature still has it.
    pub revision: String,
    /// How tall it stands against the average companion, in percent.
    pub stature_percent: u8,
    /// Its share of an adult's size, in percent.
    pub scale_percent: u8,
    /// What it is wearing, so Farm can show it on the new form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessory: Option<TravelAccessory>,
    pub availability: Availability,
}

impl FarmCreature {
    /// Its genome, sculpt and all, as Desktop's renderer draws it.
    pub fn genome(&self) -> Result<core::AppearanceGenome, FarmError> {
        let mut genome = self
            .appearance
            .to_genome()
            .map_err(|error| FarmError::invalid(format!("its appearance: {error}")))?;
        genome.sculpt = self.sculpt.clone();
        Ok(genome)
    }

    /// Its design now.
    pub fn design(&self) -> Result<Design, FarmError> {
        Ok(Design::of(&self.genome()?))
    }
}

/// What the session is for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FarmMode {
    /// Drawing a new companion. `stand_in` is the look Desktop would draw a design over until
    /// it is someone's: size, gait, and the seed small marks are laid out from.
    Create { stand_in: TravelAppearance },
    /// Reshaping one that lives in the colony.
    EditExisting { creature: FarmCreature },
}

/// Everything Farm is told about one session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FarmSnapshot {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at_utc: OffsetDateTime,
    pub desktop_version: String,
    pub mode: FarmMode,
    pub capabilities: Vec<FarmCapability>,
    pub renderer: Renderer,
    #[serde(default)]
    pub presentation: Presentation,
}

impl FarmSnapshot {
    pub fn offers(&self, capability: FarmCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// The creature being reshaped, in an edit session.
    pub fn creature(&self) -> Option<&FarmCreature> {
        match &self.mode {
            FarmMode::EditExisting { creature } => Some(creature),
            FarmMode::Create { .. } => None,
        }
    }

    /// The genome designs are drawn over in this session: the creature's own, or the stand-in.
    pub fn base_genome(&self) -> Result<core::AppearanceGenome, FarmError> {
        match &self.mode {
            FarmMode::EditExisting { creature } => creature.genome(),
            FarmMode::Create { stand_in } => stand_in
                .to_genome()
                .map_err(|error| FarmError::invalid(format!("the stand-in: {error}"))),
        }
    }

    /// Whether Desktop can draw and keep `design`: a sculpted form only on a plan it names, and
    /// only if it keeps sculpted forms at all.
    pub fn can_keep(&self, design: &Design) -> bool {
        match design.form.sculpt() {
            Some(sculpt) => {
                self.offers(FarmCapability::SculptedForms)
                    && self.renderer.design_version >= forms::DESIGN_VERSION
                    && self.renderer.plans.contains(&sculpt.plan)
            }
            None => true,
        }
    }
}

impl FarmDocument for FarmSnapshot {
    const FORMAT: &'static str = SNAPSHOT_FORMAT;
    const MAX_BYTES: u64 = MAX_SNAPSHOT_BYTES;

    fn validate(&self) -> Result<(), FarmError> {
        if !header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            SNAPSHOT_FORMAT,
        ) {
            return Err(FarmError::invalid("a snapshot header that does not add up"));
        }
        if !is_sanitized(&self.desktop_version, MAX_VERSION_CHARS)
            || self.capabilities.len() > MAX_CAPABILITIES
            || self.renderer.plans.len() > Plan::ALL.len() * 4
            || !(100..=150).contains(&self.presentation.text_scale_percent)
        {
            return Err(FarmError::invalid("a snapshot out of bounds"));
        }
        match &self.mode {
            FarmMode::Create { stand_in } => {
                stand_in
                    .to_genome()
                    .map_err(|error| FarmError::invalid(format!("the stand-in: {error}")))?;
            }
            FarmMode::EditExisting { creature } => {
                if !is_sanitized(&creature.name, MAX_NAME_CHARS)
                    || creature.name.is_empty()
                    || !is_lower_hex(&creature.revision, 32)
                    || !(1..=100).contains(&creature.scale_percent)
                    || creature.stature_percent == 0
                {
                    return Err(FarmError::invalid("the creature is out of bounds"));
                }
                if let Some(sculpt) = &creature.sculpt {
                    sculpt.validate().map_err(|error| {
                        FarmError::invalid(format!("the creature's sculpt: {error}"))
                    })?;
                }
                let design = creature.design()?;
                if design.revision() != creature.revision {
                    return Err(FarmError::invalid(
                        "the creature's revision is not the revision of its design",
                    ));
                }
            }
        }
        Ok(())
    }
}
