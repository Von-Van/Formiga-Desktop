//! From Desktop's colony to the travel format, and from a traveler back to something the art
//! crate can draw. This is the only module that reads Desktop's model; the documents themselves
//! are plain data.

use crate::document::{Document, TravelError};
use crate::ids::{SessionId, TravelerId};
use crate::limits::*;
use crate::snapshot::*;
use crate::text::sanitize_text;
use formiga_art::{AccessoryArt, Rgba, palette_for};
use formiga_core as core;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

/// What a colony that cannot be sent is told. Kept apart from reading errors so Desktop can say
/// which side of the trip went wrong.
#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error("the colony has nobody in it to send")]
    Empty,
    #[error("the colony could not be described for the trip: {0}")]
    Invalid(#[from] TravelError),
}

/// The oldest travel reader that can draw a companion of each edition. Every edition so far can
/// be drawn by version 1. A new edition in the core fails to build here until it is given the
/// version that first carries it, and a snapshot with such a companion then asks for that version,
/// so an older Hill says it is too old for the colony rather than that the colony is damaged.
const fn reader_for(edition: core::Edition) -> u32 {
    match edition {
        core::Edition::Original | core::Edition::Archetypes | core::Edition::Details => 1,
    }
}

/// The oldest travel reader a snapshot of this colony can be read by: the newest any companion's
/// look needs, and never older than version 1. Desktop asks this before it starts Hill, so a Hill
/// that says which version it reads is turned away with a reason before the colony leaves.
pub fn reader_for_colony(save: &core::SaveFile) -> u32 {
    save.creatures
        .iter()
        .filter_map(|creature| creature.appearance.design)
        .map(|design| reader_for(design.edition()))
        .max()
        .unwrap_or(1)
        .max(1)
}

/// The travel snapshot of a colony: every companion, how each looks and carries itself, how they
/// get on, and the owner's shared preferences. Nothing about the desktop they live on, their
/// plans, their memories of it, or the colony's own seed. The same colony, session and time always
/// give the same snapshot, byte for byte.
pub fn project_colony(
    save: &core::SaveFile,
    session_id: SessionId,
    created_at_utc: OffsetDateTime,
    desktop_version: &str,
) -> Result<TravelSnapshot, ProjectionError> {
    if save.creatures.is_empty() {
        return Err(ProjectionError::Empty);
    }
    let colony: [u8; 32] = Sha256::new()
        .chain_update(b"formiga-travel-colony-v1")
        .chain_update(save.colony_seed)
        .finalize()
        .into();
    let mut snapshot = TravelSnapshot::new(
        session_id,
        reader_for_colony(save),
        crate::document::hex(&colony[..8]),
        created_at_utc
            .replace_nanosecond(0)
            .unwrap_or(created_at_utc),
        sanitize_text(desktop_version, MAX_VERSION_CHARS),
    );
    let members: Vec<_> = save
        .creatures
        .iter()
        .map(|creature| palette_for(&creature.appearance))
        .collect();
    for creature in save.creatures.iter().take(MAX_TRAVELERS) {
        snapshot.travelers.push(traveler(creature, save, &members));
    }
    let travelling = |id: core::CreatureId| snapshot.traveler(TravelerId(id)).is_some();
    let mut relationships: Vec<_> = save
        .relationships
        .iter()
        .filter_map(|bond| {
            let (a, b) = core::canonical_creature_pair(bond.a, bond.b)?;
            (travelling(a) && travelling(b)).then_some(TravelRelationship {
                a: TravelerId(a),
                b: TravelerId(b),
                affinity: Band::of(bond.affinity),
                familiarity: Band::of(bond.familiarity),
                playfulness: Band::of(bond.playfulness),
                avoidance: Band::of(bond.avoidance),
            })
        })
        .collect();
    relationships.sort_by_key(|pair| (pair.a, pair.b));
    relationships.dedup_by_key(|pair| (pair.a, pair.b));
    relationships.truncate(MAX_RELATIONSHIPS);
    snapshot.relationships = relationships;
    snapshot.presentation = Presentation {
        reduce_motion: save.settings.reduce_motion,
        theme: save.companion.appearance.theme.into(),
        text_scale_percent: save.companion.appearance.text_scale.clamp(100, 150),
    };
    snapshot.validate()?;
    Ok(snapshot)
}

fn traveler(
    creature: &core::Creature,
    save: &core::SaveFile,
    members: &[formiga_art::Palette],
) -> Traveler {
    let name = Some(sanitize_text(&creature.name, MAX_NAME_CHARS))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Companion".to_owned());
    // A little one whose adult is somehow not in the colony travels as an adult rather than
    // keeping the colony home.
    let role = match creature.role {
        core::CreatureRole::Mini { parent_id }
            if save
                .creatures
                .iter()
                .any(|parent| parent.id == parent_id && parent.role.is_adult()) =>
        {
            TravelRole::Mini {
                parent_id: TravelerId(parent_id),
            }
        }
        _ => TravelRole::Adult,
    };
    let temperament = creature.temperament();
    let unit = |value: f32| {
        if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.5
        }
    };
    let axes = temperament.axes.bounded();
    Traveler {
        id: TravelerId(creature.id),
        name,
        role,
        born_at_utc: creature
            .born_at_utc
            .replace_nanosecond(0)
            .unwrap_or(creature.born_at_utc),
        appearance: (&creature.appearance).into(),
        stature_percent: core::stature_percent(creature.origin.source_colony_seed),
        scale_percent: creature.display_scale_percent.clamp(1, 100),
        character: TravelCharacter {
            temperament: temperament.kind.into(),
            axes: axes.into(),
            tension: temperament.tension.map(Into::into),
            traits: creature
                .traits()
                .iter()
                .map(|label| sanitize_text(label.label(), MAX_TRAIT_CHARS))
                .take(MAX_TRAITS)
                .collect(),
            trait_ids: creature
                .traits()
                .into_iter()
                .map(Trait::from)
                .take(MAX_TRAITS)
                .collect(),
            phrase: sanitize_text(&creature.temperament_phrase(), MAX_PHRASE_CHARS),
        },
        motion: TravelMotion {
            activity: unit(creature.personality.activity),
            playfulness: unit(creature.personality.playfulness),
            boldness: unit(creature.personality.boldness),
            celebration: Some(core::Celebration::for_creature(creature).into()),
        },
        habits: creature
            .memory
            .habits
            .iter()
            .map(|habit| (*habit).into())
            .take(MAX_HABITS)
            .collect(),
        accessory: creature.accessory.map(|accessory| {
            let ink = AccessoryArt::resolve(accessory, save.colony_seed, members).ink;
            TravelAccessory {
                item: match accessory {
                    core::Accessory::Worn(kind) => AccessoryItem::Worn { item: kind.into() },
                    core::Accessory::Pin(variant) => AccessoryItem::Pin { trinket: variant },
                },
                colors: AccessoryColors {
                    outline: rgb(ink.outline),
                    deep: rgb(ink.deep),
                    body: rgb(ink.body),
                    light: rgb(ink.light),
                    accent: rgb(ink.accent),
                },
            }
        }),
    }
}

fn rgb(color: Rgba) -> [u8; 3] {
    [color.r, color.g, color.b]
}

fn rgba([r, g, b]: [u8; 3]) -> Rgba {
    Rgba::new(r, g, b, 255)
}

impl TravelAccessory {
    pub fn to_accessory(&self) -> core::Accessory {
        match self.item {
            AccessoryItem::Worn { item } => core::Accessory::Worn(item.into()),
            AccessoryItem::Pin { trinket } => core::Accessory::Pin(trinket),
        }
    }

    /// The accessory as the art crate draws it, in the colours Desktop resolved for it.
    pub fn to_art(&self) -> AccessoryArt {
        AccessoryArt {
            accessory: self.to_accessory(),
            ink: formiga_art::TrinketInk {
                outline: rgba(self.colors.outline),
                deep: rgba(self.colors.deep),
                body: rgba(self.colors.body),
                light: rgba(self.colors.light),
                accent: rgba(self.colors.accent),
            },
        }
    }
}

impl Traveler {
    /// A stand-in companion for drawing: its look, its temperament, its pace, its habits and what
    /// it wears, standing still on a floor of its own. Its memories of Desktop, its plans and the
    /// seed it makes choices from are not part of the trip, so these are fresh.
    pub fn to_creature(&self) -> Result<core::Creature, TravelError> {
        let appearance = self.appearance.to_genome()?;
        let temperament = core::Temperament {
            kind: self.character.temperament.into(),
            axes: core::Axes::from(self.character.axes).bounded(),
            tension: self.character.tension.map(Into::into),
        };
        let mut personality = temperament.genome(0.5);
        personality.activity = self.motion.activity;
        personality.playfulness = self.motion.playfulness;
        personality.boldness = self.motion.boldness;
        let seed = |label: &[u8]| -> [u8; 32] {
            Sha256::new()
                .chain_update(label)
                .chain_update(self.id.0.to_be_bytes())
                .finalize()
                .into()
        };
        Ok(core::Creature {
            id: self.id.0,
            generation: 0,
            origin: core::CreatureOrigin {
                design: appearance.design,
                source_colony_seed: seed(b"formiga-travel-source-v1"),
                source_generation: 0,
            },
            colony_order: 0,
            role: match self.role {
                TravelRole::Adult => core::CreatureRole::Adult,
                TravelRole::Mini { parent_id } => core::CreatureRole::Mini {
                    parent_id: parent_id.0,
                },
            },
            kept: true,
            mini_arrivals: core::MiniArrivalState::default(),
            name: self.name.clone(),
            born_at_utc: self.born_at_utc,
            display_scale_percent: self.scale_percent,
            appearance,
            personality,
            temperament: Some(temperament),
            behavior_seed: seed(b"formiga-travel-behavior-v1"),
            memory: core::CreatureMemory {
                habits: self.habits.iter().map(|habit| (*habit).into()).collect(),
                ..core::CreatureMemory::default()
            },
            tendencies: core::LearnedTendencies::default(),
            routines: core::RoutineTable::default(),
            state: core::CreatureState {
                attention: None,
                position: core::Point::default(),
                velocity: core::Point::default(),
                facing_right: true,
                action: core::ActionKind::Idle,
                action_elapsed: 0.0,
                action_duration: 2.5,
                drives: core::Drives::default(),
                surface: core::SurfaceAttachment {
                    kind: core::SurfaceKind::ScreenFloor,
                    monitor_id: 0,
                    window_key: None,
                    relative_x: 0.5,
                },
                cursor_cooldown: 0.0,
                activity_variant: 0,
                arrival_delay_secs: 0.0,
                flourish: None,
                nudge: None,
                beat: None,
                indoors: false,
            },
            leaning: core::RoamingLeaning::default(),
            accessory: self.accessory.map(|accessory| accessory.to_accessory()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_build_reads_every_edition_it_can_send() {
        for edition in [
            core::Edition::Original,
            core::Edition::Archetypes,
            core::Edition::Details,
        ] {
            assert!(reader_for(edition) <= crate::TRAVEL_FORMAT_VERSION);
        }
    }
}
