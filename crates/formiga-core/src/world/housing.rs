//! Who lives in which house, as the person at the desk arranges it.
//!
//! Every full-size companion keeps a house of its own unless it lodges with somebody
//! ([`Creature::lodges_with`]), and a mini lives in its big version's unless it lodges somewhere
//! else. A house belongs to its keeper — its type, how far back it stands, its place in the row
//! and its decorations are all kept by the keeper's id — so moving a household moves people, never
//! a house, and a house taken down and put up again comes back as it was.
//!
//! Nothing here touches a companion's memories, bonds, belongings, wear or progress: only the two
//! saved housing fields on [`Creature`] and the keeper-keyed house choices on [`ColonyHome`]
//! change. Every change ends what the village was in the middle of (a call next door, a walk to a
//! doorstep, a guest's walk round), so nobody walks on to a door that has moved, and none of it
//! is allowed while a house is open in Formiga Home.
//!
//! Where a displaced household goes: the house with the fewest residents that has room for the
//! whole household ([`HOUSE_ROOM`]), the nearest along the row to the one taken down on a tie;
//! when none has room, the house with the fewest residents, nearest first, shared. The household
//! always moves together, and its keeper lodges with the keeper of the house it moves into.

use super::*;

/// How many companions a house is a comfortable home for: a keeper and as many minis as a
/// full-size companion can have. A house can hold more — six companions always fit in one village
/// — but a household being rehoused looks for one with this much room first.
pub const HOUSE_ROOM: usize = 1 + MAX_MINIS_PER_ADULT;

/// Why a change to who lives where was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HousingError {
    #[error("that companion does not live here")]
    NotFound,
    #[error("that companion keeps no house")]
    NoHouse,
    #[error("the village keeps at least one house")]
    LastHouse,
    #[error("they already live there")]
    AlreadyThere,
    #[error("a mini lives with somebody rather than keeping a house")]
    NotFullSize,
    #[error("a house is open in Formiga Home; bring its household back first")]
    HouseOpen,
}

/// The two saved housing fields of one companion, as an undo point keeps them.
pub(super) type Housing = (CreatureId, Option<CreatureId>, bool);

/// Every companion's housing fields, in colony order.
pub(super) fn housing_of(creatures: &[Creature]) -> Vec<Housing> {
    creatures
        .iter()
        .map(|creature| {
            (
                creature.id,
                creature.lodges_with,
                creature.keeps_colony_house,
            )
        })
        .collect()
}

/// Who lives in `keeper`'s house: the keeper first, then everyone else in the order they
/// arrived. Empty when `keeper` keeps no house.
pub fn house_residents(creatures: &[Creature], keeper: CreatureId) -> Vec<CreatureId> {
    let mut living: Vec<&Creature> = creatures
        .iter()
        .filter(|creature| house_keeper_of(creature, creatures) == Some(keeper))
        .collect();
    if !living.iter().any(|creature| creature.id == keeper) {
        return Vec::new();
    }
    living.sort_by_key(|creature| (creature.id != keeper, creature.colony_order, creature.id));
    living.into_iter().map(|creature| creature.id).collect()
}

/// Where a household of `moving` companions leaving `leaving`'s house would go, by the rule in
/// this module's notes. `None` when there is no other house.
pub fn rehoming_host(
    creatures: &[Creature],
    cottage_order: &[CreatureId],
    leaving: CreatureId,
    moving: usize,
) -> Option<CreatureId> {
    let owners = house_owners(creatures, cottage_order);
    let owners = owners.as_slice();
    let from = owners.iter().position(|id| *id == leaving).unwrap_or(0);
    owners
        .iter()
        .enumerate()
        .filter(|(_, id)| **id != leaving)
        .min_by_key(|(slot, id)| {
            let living = house_residents(creatures, **id).len();
            let fits = living + moving <= HOUSE_ROOM;
            (!fits, living, slot.abs_diff(from), *slot)
        })
        .map(|(_, id)| *id)
}

/// Bring the housing fields inside what they can say: nobody lodges with themselves or with
/// somebody who is not here, every lodging names a house's keeper directly where it can, only a
/// full-size keeper keeps the colony house and only one does, and a mark that says no more than
/// the order of arrival already does is not written down. Run on every colony that is opened and
/// after every change here.
pub(crate) fn normalize_housing(creatures: &mut [Creature]) {
    let ids: Vec<CreatureId> = creatures.iter().map(|creature| creature.id).collect();
    for creature in creatures.iter_mut() {
        if creature
            .lodges_with
            .is_some_and(|host| host == creature.id || !ids.contains(&host))
        {
            creature.lodges_with = None;
        }
        if !creature.role.is_adult() {
            creature.keeps_colony_house = false;
        }
    }
    // A lodging names the keeper of the house it means: a chain through other lodgers, or a
    // mini's lodging that is just where its big version lives anyway, is said the short way.
    let snapshot = creatures.to_vec();
    for creature in creatures.iter_mut() {
        if creature.lodges_with.is_none() {
            continue;
        }
        let keeper = house_keeper_of(
            snapshot
                .iter()
                .find(|c| c.id == creature.id)
                .expect("from the same list"),
            &snapshot,
        );
        let natural = match creature.role {
            CreatureRole::Mini { parent_id } => snapshot
                .iter()
                .find(|c| c.id == parent_id)
                .and_then(|parent| house_keeper_of(parent, &snapshot)),
            CreatureRole::Adult => None,
        };
        creature.lodges_with = match keeper {
            Some(keeper) if keeper == creature.id => None,
            Some(keeper) if Some(keeper) == natural => None,
            Some(keeper) => Some(keeper),
            // A circle of lodgers: the first of them keeps a house again.
            None => None,
        };
    }
    // The colony house is kept by one full-size keeper at most, and the mark is dropped when it
    // names the companion who would keep it anyway.
    let snapshot = creatures.to_vec();
    let flagged = snapshot
        .iter()
        .filter(|creature| creature.keeps_colony_house && keeps_house(creature, &snapshot))
        .min_by_key(|creature| (creature.colony_order, creature.id))
        .map(|creature| creature.id);
    let natural = snapshot
        .iter()
        .filter(|creature| keeps_house(creature, &snapshot))
        .min_by_key(|creature| (creature.colony_order, creature.id))
        .map(|creature| creature.id);
    for creature in creatures.iter_mut() {
        creature.keeps_colony_house = Some(creature.id) == flagged && flagged != natural;
    }
}

impl World {
    /// Who lives in the house `keeper` keeps, keeper first.
    pub fn house_residents(&self, keeper: CreatureId) -> Vec<CreatureId> {
        house_residents(&self.save.creatures, keeper)
    }

    /// Take down the house `keeper` keeps. Its whole household — the keeper, its minis, and
    /// anyone lodging with it — moves together into another house by the rule in this module's
    /// notes, and nobody leaves the colony or forgets anything. When it was the colony house, the
    /// next house along becomes the colony house. Returns the keeper of the house they moved into.
    pub fn take_down_house(&mut self, keeper: CreatureId) -> Result<CreatureId, HousingError> {
        self.check_housing(keeper)?;
        if !self.keeps(keeper) {
            return Err(HousingError::NoHouse);
        }
        let moving = self.house_residents(keeper).len();
        let host = rehoming_host(
            &self.save.creatures,
            &self.save.home.cottage_order,
            keeper,
            moving,
        )
        .ok_or(HousingError::LastHouse)?;
        self.move_household(keeper, host);
        Ok(host)
    }

    /// Move `resident` into the house `keeper` keeps. A mini or a lodger simply moves; a
    /// companion who keeps a house of its own brings its household with it and its house comes
    /// down, exactly as [`World::take_down_house`] would, but into this house.
    pub fn move_into_house(
        &mut self,
        resident: CreatureId,
        keeper: CreatureId,
    ) -> Result<(), HousingError> {
        self.check_housing(resident)?;
        if !self.keeps(keeper) {
            return Err(HousingError::NoHouse);
        }
        let creatures = &self.save.creatures;
        let mover = creatures
            .iter()
            .find(|creature| creature.id == resident)
            .ok_or(HousingError::NotFound)?;
        if house_keeper_of(mover, creatures) == Some(keeper) {
            return Err(HousingError::AlreadyThere);
        }
        if keeps_house(mover, creatures) {
            self.move_household(resident, keeper);
            return Ok(());
        }
        if let Some(creature) = creature_mut(&mut self.save.creatures, resident) {
            creature.lodges_with = Some(keeper);
        }
        self.housing_changed();
        Ok(())
    }

    /// Give a full-size companion who lodges with somebody a house of its own again: in the
    /// place in the row it last had, or else at the end, built as it was, as far back as it
    /// stood and wearing what it wore. Its minis who live with it come too.
    pub fn give_own_house(&mut self, resident: CreatureId) -> Result<(), HousingError> {
        self.check_housing(resident)?;
        let creatures = &self.save.creatures;
        let mover = creatures
            .iter()
            .find(|creature| creature.id == resident)
            .ok_or(HousingError::NotFound)?;
        if !mover.role.is_adult() {
            return Err(HousingError::NotFullSize);
        }
        if keeps_house(mover, creatures) {
            return Err(HousingError::AlreadyThere);
        }
        // Its minis who were living with it, wherever that was, come along.
        let minis: Vec<CreatureId> = creatures
            .iter()
            .filter(|creature| {
                creature.role.parent_id() == Some(resident)
                    && house_keeper_of(creature, creatures) == house_keeper_of(mover, creatures)
            })
            .map(|creature| creature.id)
            .collect();
        for creature in &mut self.save.creatures {
            if creature.id == resident || minis.contains(&creature.id) {
                creature.lodges_with = None;
            }
        }
        self.housing_changed();
        Ok(())
    }

    /// Swap the households of two houses: everyone living in `a`'s house moves into `b`'s and
    /// everyone in `b`'s into `a`'s. The houses stay exactly where and as they are — type, depth,
    /// decorations — and only who lives in them changes.
    pub fn swap_households(&mut self, a: CreatureId, b: CreatureId) -> Result<(), HousingError> {
        self.check_housing(a)?;
        self.check_housing(b)?;
        if a == b {
            return Err(HousingError::AlreadyThere);
        }
        if !self.keeps(a) || !self.keeps(b) {
            return Err(HousingError::NoHouse);
        }
        let creatures = self.save.creatures.clone();
        let home = &mut self.save.home;
        let owners = house_owners(&creatures, &home.cottage_order);
        let owners = owners.as_slice().to_vec();
        let styles = home.house_style_list(&creatures);
        let slot_of = |id: CreatureId| owners.iter().position(|o| *o == id).unwrap_or(0);
        let (slot_a, slot_b) = (slot_of(a), slot_of(b));
        let other = |id: CreatureId| if id == a { b } else { a };
        // Each house keeps the type it is drawn as now, written down for its new keeper unless
        // that is what the new keeper's house would be anyway.
        let style_for = |keeper: CreatureId, slot: usize| -> Option<ShelterStyle> {
            let default = if slot == 0 {
                home.shelter.style
            } else {
                creatures
                    .iter()
                    .find(|creature| creature.id == keeper)
                    .map_or(home.shelter.style, ShelterStyle::for_keeper)
            };
            (styles[slot] != default).then_some(styles[slot])
        };
        let (style_a, style_b) = (style_for(b, slot_a), style_for(a, slot_b));
        home.house_styles
            .retain(|choice| choice.keeper != a && choice.keeper != b);
        if let Some(style) = style_a {
            home.house_styles
                .push(HouseStyleChoice { keeper: b, style });
        }
        if let Some(style) = style_b {
            home.house_styles
                .push(HouseStyleChoice { keeper: a, style });
        }
        for choice in &mut home.house_depths {
            if choice.keeper == a || choice.keeper == b {
                choice.keeper = other(choice.keeper);
            }
        }
        for dressing in &mut home.dressing {
            if dressing.keeper == a || dressing.keeper == b {
                dressing.keeper = other(dressing.keeper);
            }
        }
        // The row, written out in full with the two swapped, the colony house's keeper first.
        let mut row = owners.clone();
        row.swap(slot_a, slot_b);
        let colony_keeper = row[0];
        for creature in &mut self.save.creatures {
            creature.keeps_colony_house = creature.id == colony_keeper;
        }
        let creatures = self.save.creatures.clone();
        self.save
            .home
            .arrange_cottages(row[1..].to_vec(), &creatures);
        self.housing_changed();
        Ok(())
    }

    /// Whether this companion keeps a house.
    fn keeps(&self, id: CreatureId) -> bool {
        self.save
            .creatures
            .iter()
            .find(|creature| creature.id == id)
            .is_some_and(|creature| keeps_house(creature, &self.save.creatures))
    }

    fn check_housing(&self, id: CreatureId) -> Result<(), HousingError> {
        if !self.house_visit.is_empty() {
            return Err(HousingError::HouseOpen);
        }
        if !self.save.creatures.iter().any(|creature| creature.id == id) {
            return Err(HousingError::NotFound);
        }
        Ok(())
    }

    /// `keeper`'s whole household lodges with `host` from now on, and `keeper`'s house comes
    /// down. When it was the colony house, the next house along is the colony house now.
    fn move_household(&mut self, keeper: CreatureId, host: CreatureId) {
        let creatures = &self.save.creatures;
        let owners = house_owners(creatures, &self.save.home.cottage_order);
        let owners = owners.as_slice().to_vec();
        let household = house_residents(creatures, keeper);
        // A mini of the keeper's who lives with it by right follows it without a word; everyone
        // else living there is written down as lodging with the new house's keeper.
        let follows: Vec<CreatureId> = creatures
            .iter()
            .filter(|creature| {
                creature.role.parent_id() == Some(keeper) && creature.lodges_with.is_none()
            })
            .map(|creature| creature.id)
            .collect();
        let next_along = (owners.first() == Some(&keeper))
            .then(|| owners.iter().copied().find(|id| *id != keeper))
            .flatten();
        for creature in &mut self.save.creatures {
            if creature.id == keeper {
                creature.lodges_with = Some(host);
                creature.keeps_colony_house = false;
            } else if household.contains(&creature.id) && !follows.contains(&creature.id) {
                creature.lodges_with = Some(host);
            }
            if let Some(next) = next_along {
                creature.keeps_colony_house = creature.id == next;
            }
        }
        self.housing_changed();
    }

    /// After any change to who lives where: tidy the fields, and let go of everything the village
    /// was in the middle of that was headed for one house or another — a call next door, a walk
    /// up to a doorstep, somebody indoors — so everyone carries on from where they stand towards
    /// the house they live in now. A guest on its walk round works its tour out again.
    fn housing_changed(&mut self) {
        normalize_housing(&mut self.save.creatures);
        let creatures = self.save.creatures.clone();
        let order = self.save.home.cottage_order.clone();
        self.save.home.arrange_cottages(order, &creatures);
        self.save.home.normalize_village();
        self.end_village_life();
        self.pending_home_greetings.clear();
        if let Some(guest) = &mut self.save.visitors.guest {
            guest.visit.planned = None;
        }
    }
}
