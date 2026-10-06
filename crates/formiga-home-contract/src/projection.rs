//! From Desktop's colony to one household's snapshot. This is the only module that reads Desktop's
//! model; the documents themselves are plain data. It is Desktop's side of the contract, and Home's
//! rehearsals call it too, so they open a colony file exactly as Desktop would open the house.

use crate::document::HomeError;
use crate::inventory::{DisplayItem, DisplaySource, Ink, find_modes, souvenir_modes};
use crate::limits::*;
use crate::snapshot::{HomeCapability, HomeSnapshot, Household, Neighbour, header};
use crate::{DisplayId, HomeDocument};
use formiga_art::{TrinketAtlasRenderer, palette_for};
use formiga_core as core;
use formiga_expansion_rulebook::{SessionId, sanitize_text};
use formiga_travel::TravelerId;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

/// What a house that cannot be opened is told.
#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error("nobody keeps that house")]
    NoSuchHouse,
    #[error("the household could not be described for Home: {0}")]
    Travel(#[from] formiga_travel::ProjectionError),
    #[error("the household could not be described for Home: {0}")]
    Invalid(#[from] HomeError),
}

/// The colony's key in Home's records: a one-way digest of its seed, under a label of Home's own,
/// so it says nothing a trip's colony id says.
pub fn colony_key(colony_seed: &[u8; 32]) -> String {
    let digest: [u8; 32] = Sha256::new()
        .chain_update(b"formiga-home-colony-v1")
        .chain_update(colony_seed)
        .finalize()
        .into();
    hex(&digest[..8])
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// How many friends Desktop lends a visit at most.
pub const VISITORS_LENT: usize = 2;

/// Who might drop by when `keeper`'s house is opened: full-size companions who keep houses of
/// their own and are close friends of someone who lives in this one, closest first, and at most
/// [`VISITORS_LENT`] of them. Desktop lends whichever of them are free.
pub fn likely_visitors(save: &core::SaveFile, keeper: core::CreatureId) -> Vec<core::CreatureId> {
    let order = &save.home.cottage_order;
    let owners = core::house_owners(&save.creatures, order);
    let Some(slot) = owners.as_slice().iter().position(|id| *id == keeper) else {
        return Vec::new();
    };
    let living: Vec<core::CreatureId> = save
        .creatures
        .iter()
        .filter(|creature| core::house_slot_for(creature, &save.creatures, order) == slot)
        .map(|creature| creature.id)
        .collect();
    let mut friends: Vec<(u8, core::CreatureId)> = owners
        .as_slice()
        .iter()
        .filter(|id| !living.contains(id))
        .filter_map(|id| {
            save.relationships
                .iter()
                .filter(|bond| {
                    let (a, b) = (bond.a, bond.b);
                    (a == *id && living.contains(&b)) || (b == *id && living.contains(&a))
                })
                .filter(|bond| {
                    formiga_travel::Band::of(bond.affinity) == formiga_travel::Band::High
                        && formiga_travel::Band::of(bond.avoidance) <= formiga_travel::Band::Low
                })
                .map(|bond| bond.affinity)
                .max()
                .map(|affinity| (affinity, *id))
        })
        .collect();
    friends.sort_by_key(|(affinity, id)| (std::cmp::Reverse(*affinity), *id));
    friends
        .into_iter()
        .map(|(_, id)| id)
        .take(VISITORS_LENT)
        .collect()
}

/// The snapshot of the household that keeps `keeper`'s house: the keeper and every little one who
/// lives with it, drawn exactly as a trip would carry them; the `visitors` Desktop lends for the
/// visit, of those who keep houses of their own; who keeps every other house; everything the
/// colony has that a house can show; and what Desktop `offers` to take back from the visit, which
/// should be only what it applies. The same colony, house, visitors, offer, session and time
/// always give the same snapshot, byte for byte.
pub fn project_household(
    save: &core::SaveFile,
    keeper: core::CreatureId,
    visitors: &[core::CreatureId],
    offers: &[HomeCapability],
    session_id: SessionId,
    created_at_utc: OffsetDateTime,
    desktop_version: &str,
) -> Result<HomeSnapshot, ProjectionError> {
    let order = &save.home.cottage_order;
    let owners = core::house_owners(&save.creatures, order);
    let slot = owners
        .as_slice()
        .iter()
        .position(|id| *id == keeper)
        .ok_or(ProjectionError::NoSuchHouse)?;
    let travel =
        formiga_travel::project_colony(save, session_id.clone(), created_at_utc, desktop_version)?;

    let mut living: Vec<&core::Creature> = save
        .creatures
        .iter()
        .filter(|creature| core::house_slot_for(creature, &save.creatures, order) == slot)
        .collect();
    living.sort_by_key(|creature| (creature.id != keeper, creature.colony_order, creature.id));
    let residents: Vec<_> = living
        .iter()
        .filter_map(|creature| travel.traveler(TravelerId(creature.id)).cloned())
        .take(MAX_RESIDENTS)
        .collect();
    let mut lent: Vec<_> = Vec::new();
    for id in visitors {
        let keeps_another = owners.as_slice().contains(id) && *id != keeper;
        let living_here = residents.iter().any(|resident| resident.id.0 == *id);
        if let Some(visitor) = travel.traveler(TravelerId(*id))
            && keeps_another
            && !living_here
            && visitor.role == formiga_travel::TravelRole::Adult
            && !lent
                .iter()
                .any(|known: &formiga_travel::Traveler| known.id == visitor.id)
            && lent.len() < MAX_VISITORS
            && residents.len() + lent.len() < MAX_RESIDENTS
        {
            lent.push(visitor.clone());
        }
    }
    let visitors = lent;
    let home = |id: TravelerId| {
        residents.iter().any(|resident| resident.id == id)
            || visitors.iter().any(|visitor| visitor.id == id)
    };
    let relationships = travel
        .relationships
        .iter()
        .filter(|pair| home(pair.a) && home(pair.b))
        .copied()
        .collect();
    let village = owners
        .as_slice()
        .iter()
        .enumerate()
        .filter_map(|(slot, id)| {
            Some(Neighbour {
                keeper: TravelerId(*id),
                name: travel.traveler(TravelerId(*id))?.name.clone(),
                slot: slot as u8,
            })
        })
        .take(MAX_HOUSEHOLDS)
        .collect();
    let style = save.home.house_style_list(&save.creatures)[slot].into();

    let (format, version, min_reader_version) = header();
    let snapshot = HomeSnapshot {
        format,
        version,
        min_reader_version,
        session_id,
        colony_key: colony_key(&save.colony_seed),
        created_at_utc: travel.created_at_utc,
        desktop_version: travel.desktop_version.clone(),
        travel_version: travel.version,
        travel_min_reader_version: travel.min_reader_version,
        capabilities: HomeCapability::ALL
            .into_iter()
            .filter(|capability| offers.contains(capability))
            .collect(),
        household: Household {
            keeper: TravelerId(keeper),
            slot: slot as u8,
            style,
        },
        residents,
        visitors,
        relationships,
        village,
        inventory: inventory(save),
        days_lived: u32::try_from((created_at_utc - save.created_at_utc).whole_days().max(0))
            .unwrap_or(u32::MAX),
        presentation: travel.presentation,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

/// Everything the colony has that a house can show: each find in the scrapbook, in catalogue
/// order, then each souvenir in the order it came home.
fn inventory(save: &core::SaveFile) -> Vec<DisplayItem> {
    let members: Vec<_> = save
        .creatures
        .iter()
        .map(|creature| palette_for(&creature.appearance))
        .collect();
    let finds = save.companion.scrapbook.iter().map(|record| {
        let variant = record.variant;
        let name = core::trinket_info(variant).map_or("A find", |info| info.name);
        DisplayItem {
            id: DisplayId::find(variant),
            source: DisplaySource::DesktopFind { variant },
            name: item_name(name),
            ink: Some(Ink::of(TrinketAtlasRenderer::ink(
                save.colony_seed,
                &members,
                variant,
            ))),
            modes: find_modes(variant),
            found_at_utc: whole_seconds(record.first_at),
            finder_name: Some(sanitize_text(&record.finder_name, MAX_NAME_CHARS))
                .filter(|name| !name.is_empty()),
        }
    });
    let souvenirs = save.trips.souvenirs.iter().filter_map(|record| {
        let id = record.souvenir.id();
        Some(DisplayItem {
            id: DisplayId::souvenir(id)?,
            source: DisplaySource::HillSouvenir { id: id.to_owned() },
            name: item_name(record.souvenir.name()),
            ink: None,
            modes: souvenir_modes(record.souvenir),
            found_at_utc: whole_seconds(record.brought_home_at_utc),
            finder_name: None,
        })
    });
    finds.chain(souvenirs).take(MAX_INVENTORY).collect()
}

fn item_name(name: &str) -> String {
    Some(sanitize_text(name, MAX_ITEM_NAME_CHARS))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "A find".to_owned())
}

fn whole_seconds(at: OffsetDateTime) -> OffsetDateTime {
    at.replace_nanosecond(0).unwrap_or(at)
}
