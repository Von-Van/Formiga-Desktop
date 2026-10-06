//! Sample sessions, for Farm's rehearsals and tests and for this crate's own: Desktop's own
//! sample colony, opened to reshape its first companion, or to draw a new one. They are the same
//! every time.

use crate::{FarmCapability, FarmSnapshot, Renderer, SessionId, project_create, project_edit};
use formiga_core::{AppearanceGenome, SaveFile};
use time::{Duration, OffsetDateTime};

/// When the sample colony was made.
pub const MADE: OffsetDateTime = formiga_travel::sample::MADE;

/// When the sample sessions were opened: a few weeks into the colony's life.
pub const OPENED: OffsetDateTime = MADE.saturating_add(Duration::days(21));

/// The sample edit session.
pub const EDIT_SESSION: &str = "fa4fa4fa4fa4fa4fa4fa4fa4fa4fa4fa";

/// The sample create session.
pub const CREATE_SESSION: &str = "c4ea7ec4ea7ec4ea7ec4ea7ec4ea7ec4";

/// The Desktop version the samples are written as from.
pub const DESKTOP_VERSION: &str = "0.67.0";

/// The sample colony.
pub fn colony() -> SaveFile {
    formiga_travel::sample::colony(3)
}

/// Everything a Desktop that has adopted Farm offers.
pub fn capabilities() -> Vec<FarmCapability> {
    vec![
        FarmCapability::EditExisting,
        FarmCapability::CreateNew,
        FarmCapability::SculptedForms,
        FarmCapability::JournalNote,
    ]
}

/// The snapshot of a session reshaping `creature` (an index into the sample colony).
pub fn edit_snapshot(creature: usize) -> FarmSnapshot {
    let save = colony();
    project_edit(
        &save,
        save.creatures[creature].id,
        true,
        SessionId::parse(EDIT_SESSION).expect("a session id"),
        OPENED,
        DESKTOP_VERSION,
        capabilities(),
        Renderer::this_build(),
    )
    .expect("the sample colony projects")
}

/// The look a newcomer is drawn over before it is anyone: an ordinary grown companion of
/// average size.
pub fn stand_in() -> AppearanceGenome {
    let mut genome = colony().creatures[0].appearance.clone();
    genome.logical_size = 38;
    genome
}

/// The snapshot of a session drawing a new companion.
pub fn create_snapshot() -> FarmSnapshot {
    project_create(
        &colony(),
        &stand_in(),
        SessionId::parse(CREATE_SESSION).expect("a session id"),
        OPENED,
        DESKTOP_VERSION,
        capabilities(),
        Renderer::this_build(),
    )
    .expect("the sample stand-in projects")
}
