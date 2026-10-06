//! What Desktop keeps of Home's answer. Home proposes every house whole; Desktop keeps only the
//! visited household's home, and from every other house only the taking down of what the visited
//! household now shows, since moving a find between houses moves it rather than copying it.
//! Everything else Home proposes is set aside, and anything that does not check out leaves the
//! homes exactly as they were.

use crate::inventory::DisplayMode;
use crate::replies::{HomeResult, SessionSeal};
use crate::snapshot::HomeSnapshot;
use crate::state::{HomeState, HouseholdHome, Liked, Spot};
use crate::{HomeDocument, TravelerId};
use std::collections::BTreeSet;

/// The homes as Desktop will keep them, and the kinds of what it set aside, for its log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Accepted {
    pub state: HomeState,
    pub set_aside: Vec<&'static str>,
}

impl HomeState {
    /// These homes as they stand for the colony `snapshot` describes: none at all if they were
    /// kept for another colony; only the households whose keepers still keep a house; and
    /// nothing shown that the colony does not have, or that cannot be shown where it is.
    pub fn settled_for(&self, snapshot: &HomeSnapshot) -> Self {
        if self.colony_key != snapshot.colony_key {
            return Self::new(&snapshot.colony_key);
        }
        let mut settled = self.clone();
        // Written again by this build, it is this build's version.
        settled.version = settled.version.max(crate::HOME_FORMAT_VERSION);
        settled
            .households
            .retain(|home| snapshot.neighbour(home.keeper).is_some());
        let mut shown = BTreeSet::new();
        for home in &mut settled.households {
            let own = own_mementos(home);
            for room in &mut home.rooms {
                room.displays.retain(|placed| {
                    let fits = own.contains(&placed.item)
                        || snapshot
                            .item(&placed.item)
                            .is_some_and(|item| match placed.spot {
                                Spot::Floor { .. } => item.allows(DisplayMode::Floor),
                                Spot::On { .. } | Spot::Wall { .. } => true,
                            });
                    fits && shown.insert(placed.item.clone())
                });
            }
            home.forget_what_is_gone();
            home.likings.retain(|liking| match &liking.thing {
                Liked::Shown { item } => snapshot.item(item).is_some() || own.contains(item),
                Liked::Piece { .. } => true,
            });
        }
        settled
    }
}

/// What Desktop keeps of `result`, given the homes it sent (`previous`) for the visit `seal`
/// names. Home may change only the visited household's home. A result for another visit or
/// another colony changes nothing.
pub fn accept_result(
    seal: &SessionSeal,
    snapshot: &HomeSnapshot,
    previous: &HomeState,
    result: &HomeResult,
) -> Accepted {
    let unchanged = |why: &'static str| Accepted {
        state: previous.settled_for(snapshot),
        set_aside: vec![why],
    };
    if !result.answers(seal) || snapshot.session_id != seal.session_id {
        return unchanged("result_for_another_visit");
    }
    if result.state.colony_key != snapshot.colony_key {
        return unchanged("result_for_another_colony");
    }
    let keeper = snapshot.household.keeper;
    let Some(proposed) = result.state.household(keeper) else {
        return unchanged("result_without_this_household");
    };
    let mut set_aside = Vec::new();
    // Another house as Home left it should be that house as it was, less whatever moved here.
    let moved: Vec<_> = proposed.shown().collect();
    let other_changed = result.state.households.iter().any(|home| {
        let mut before = previous.household(home.keeper).cloned();
        if let Some(before) = &mut before {
            moved.iter().for_each(|item| {
                before.take_down(item);
            });
        }
        home.keeper != keeper && before.as_ref() != Some(home)
    });
    if other_changed {
        set_aside.push("another_household_changed");
    }

    let mut state = previous.settled_for(snapshot);
    let home = proposed_home(snapshot, keeper, proposed, &mut set_aside);
    for item in home.shown() {
        for other in &mut state.households {
            if other.keeper != keeper {
                other.take_down(item);
            }
        }
    }
    state.set_household(home);
    let state = state.settled_for(snapshot);
    match state.validate() {
        Ok(()) => Accepted { state, set_aside },
        Err(_) => unchanged("result_did_not_check_out"),
    }
}

/// The visited household's home as Home proposed it, showing only what the colony has, each
/// thing once, and only where it can be shown.
fn proposed_home(
    snapshot: &HomeSnapshot,
    keeper: TravelerId,
    proposed: &HouseholdHome,
    set_aside: &mut Vec<&'static str>,
) -> HouseholdHome {
    let mut home = proposed.clone();
    home.keeper = keeper;
    let own = own_mementos(&home);
    let mut shown = BTreeSet::new();
    for room in &mut home.rooms {
        let before = room.displays.len();
        room.displays.retain(|placed| {
            let fits = own.contains(&placed.item)
                || snapshot
                    .item(&placed.item)
                    .is_some_and(|item| match placed.spot {
                        Spot::Floor { .. } => item.allows(DisplayMode::Floor),
                        Spot::On { .. } | Spot::Wall { .. } => true,
                    });
            fits && shown.insert(placed.item.clone())
        });
        if room.displays.len() != before {
            set_aside.push("display_not_kept");
        }
    }
    // Only those who live here come to like things here: a visitor's likings are its own house's.
    let before = home.likings.len();
    home.likings
        .retain(|liking| snapshot.resident(liking.resident).is_some());
    home.forget_what_is_gone();
    if home.likings.len() != before {
        set_aside.push("liking_not_kept");
    }
    home
}

/// How the household's own keepsakes are shown: things it has that the colony's snapshot never
/// lists.
fn own_mementos(home: &HouseholdHome) -> Vec<crate::DisplayId> {
    home.mementos
        .iter()
        .map(|memento| memento.id(home.keeper))
        .collect()
}
