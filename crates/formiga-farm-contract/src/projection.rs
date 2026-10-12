//! How Desktop writes a session's snapshot from its colony: the one creature being reshaped,
//! exactly as a trip would carry its look, or a stand-in for a new one. Nothing else about the
//! colony leaves it.

use crate::document::FarmError;
use crate::limits::*;
use crate::{
    Availability, FARM_FORMAT_VERSION, FarmCapability, FarmCreature, FarmDocument, FarmMode,
    FarmSnapshot, Renderer, SNAPSHOT_FORMAT,
};
use formiga_core as core;
use formiga_core::forms::Design;
use formiga_expansion_rulebook::{SessionId, sanitize_text};
use formiga_travel::{Presentation, TravelerId};
use time::OffsetDateTime;

#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error("there is no creature {0} in the colony")]
    NoSuchCreature(core::CreatureId),
    #[error("the colony could not be projected: {0}")]
    Travel(#[from] formiga_travel::ProjectionError),
    #[error("the snapshot would not be valid: {0}")]
    Invalid(#[from] FarmError),
}

/// What Desktop sets out for any session.
pub struct Opening<'a> {
    pub session_id: SessionId,
    pub created_at_utc: OffsetDateTime,
    pub desktop_version: &'a str,
    pub capabilities: Vec<FarmCapability>,
    pub renderer: Renderer,
}

fn snapshot(opening: Opening<'_>, mode: FarmMode, presentation: Presentation) -> FarmSnapshot {
    FarmSnapshot {
        format: SNAPSHOT_FORMAT.to_owned(),
        version: FARM_FORMAT_VERSION,
        min_reader_version: 1,
        session_id: opening.session_id,
        created_at_utc: opening
            .created_at_utc
            .replace_nanosecond(0)
            .unwrap_or(opening.created_at_utc),
        desktop_version: sanitize_text(opening.desktop_version, MAX_VERSION_CHARS),
        mode,
        capabilities: opening.capabilities,
        renderer: opening.renderer,
        presentation,
    }
}

fn presentation(save: &core::SaveFile) -> Presentation {
    Presentation {
        reduce_motion: save.settings.reduce_motion,
        theme: save.companion.appearance.theme.into(),
        text_scale_percent: save.companion.appearance.text_scale.clamp(100, 150),
    }
}

/// The snapshot for reshaping `creature`: its look as a trip carries it, its sculpt if it has
/// one (which a trip leaves behind), and whether it is home to be changed.
#[allow(clippy::too_many_arguments)]
pub fn project_edit(
    save: &core::SaveFile,
    creature: core::CreatureId,
    available: bool,
    session_id: SessionId,
    created_at_utc: OffsetDateTime,
    desktop_version: &str,
    capabilities: Vec<FarmCapability>,
    renderer: Renderer,
) -> Result<FarmSnapshot, ProjectionError> {
    let living = save
        .creatures
        .iter()
        .find(|c| c.id == creature)
        .ok_or(ProjectionError::NoSuchCreature(creature))?;
    // A trip's own projection, so the look Farm draws is exactly the look Hill and Home draw.
    let trip =
        formiga_travel::project_colony(save, session_id.clone(), created_at_utc, desktop_version)?;
    let traveler = trip
        .traveler(TravelerId(creature))
        .ok_or(ProjectionError::NoSuchCreature(creature))?;
    let design = Design::of(&living.appearance);
    let farm_creature = FarmCreature {
        id: traveler.id,
        name: traveler.name.clone(),
        role: traveler.role,
        appearance: traveler.appearance.clone(),
        sculpt: living.appearance.sculpt.clone(),
        revision: design.revision(),
        stature_percent: traveler.stature_percent,
        scale_percent: traveler.scale_percent,
        accessory: traveler.accessory,
        availability: if available {
            Availability::Available
        } else {
            Availability::Away
        },
    };
    let snapshot = snapshot(
        Opening {
            session_id,
            created_at_utc,
            desktop_version,
            capabilities,
            renderer,
        },
        FarmMode::EditExisting {
            creature: farm_creature,
        },
        presentation(save),
    );
    snapshot.validate()?;
    Ok(snapshot)
}

/// The snapshot for drawing a new companion over `stand_in`: the look Desktop would give a
/// newcomer before it is anyone, which a design is drawn over until Desktop welcomes it.
pub fn project_create(
    save: &core::SaveFile,
    stand_in: &core::AppearanceGenome,
    session_id: SessionId,
    created_at_utc: OffsetDateTime,
    desktop_version: &str,
    capabilities: Vec<FarmCapability>,
    renderer: Renderer,
) -> Result<FarmSnapshot, ProjectionError> {
    let snapshot = snapshot(
        Opening {
            session_id,
            created_at_utc,
            desktop_version,
            capabilities,
            renderer,
        },
        FarmMode::Create {
            stand_in: stand_in.into(),
        },
        presentation(save),
    );
    snapshot.validate()?;
    Ok(snapshot)
}
