//! What Desktop keeps of Home's answer: only the visited household's home, every find in one place
//! at a time, and nothing at all from an answer that is not this visit's.

mod common;

use common::{arranged, closed_at, neighbours_home};
use formiga_home_contract::*;

struct Visit {
    snapshot: HomeSnapshot,
    previous: HomeState,
    seal: SessionSeal,
}

/// A visit to the sample house, with the next house along already showing the daisy and the
/// ribbon.
fn visit() -> Visit {
    let snapshot = sample::snapshot();
    let mut previous = HomeState::new(&snapshot.colony_key);
    previous
        .households
        .push(neighbours_home(snapshot.village[1].keeper));
    let snapshot_bytes = encode(&snapshot).unwrap();
    let state_bytes = encode(&previous).unwrap();
    let seal = SessionSeal::of(&snapshot, &snapshot_bytes, &state_bytes);
    Visit {
        snapshot,
        previous,
        seal,
    }
}

fn result(visit: &Visit, state: HomeState) -> HomeResult {
    HomeResult::new(&visit.seal, closed_at(), "0.1.0", state)
}

/// What Home would write back after arranging the visited house as `home`.
fn proposing(visit: &Visit, home: HouseholdHome) -> HomeResult {
    let mut state = visit.previous.clone();
    for item in home.shown().cloned().collect::<Vec<_>>() {
        for other in &mut state.households {
            other.take_down(&item);
        }
    }
    state.set_household(home);
    result(visit, state)
}

#[test]
fn the_visited_household_keeps_its_home_as_it_was_left() {
    let visit = visit();
    let home = arranged(&visit.snapshot);
    let accepted = accept_result(
        &visit.seal,
        &visit.snapshot,
        &visit.previous,
        &proposing(&visit, home.clone()),
    );
    assert_eq!(accepted.set_aside, Vec::<&str>::new());
    assert_eq!(accepted.state.household(home.keeper), Some(&home));
    assert_eq!(
        accepted.state.household(visit.snapshot.village[1].keeper),
        visit.previous.household(visit.snapshot.village[1].keeper),
        "the house next door is as it was"
    );
}

#[test]
fn a_find_moved_here_from_next_door_is_moved_not_copied() {
    let visit = visit();
    let daisy = DisplayId::find(26);
    let mut home = arranged(&visit.snapshot);
    home.rooms[0].displays.push(PlacedDisplay {
        item: daisy.clone(),
        spot: Spot::Wall {
            side: WallSide::West,
            at: 3,
        },
    });
    let accepted = accept_result(
        &visit.seal,
        &visit.snapshot,
        &visit.previous,
        &proposing(&visit, home.clone()),
    );
    assert_eq!(accepted.state.shown_by(&daisy), Some(home.keeper));
    let next_door = accepted
        .state
        .household(visit.snapshot.village[1].keeper)
        .unwrap();
    assert!(next_door.shown().all(|item| item != &daisy));
    assert!(
        next_door
            .shown()
            .any(|item| item == &DisplayId::souvenir("picnic_ribbon").unwrap()),
        "only what moved left next door"
    );
    accepted.state.validate().unwrap();
}

#[test]
fn a_find_shown_twice_by_a_careless_home_ends_up_in_one_place() {
    let visit = visit();
    let daisy = DisplayId::find(26);
    let mut home = arranged(&visit.snapshot);
    home.rooms[0].displays.push(PlacedDisplay {
        item: daisy.clone(),
        spot: Spot::Wall {
            side: WallSide::West,
            at: 3,
        },
    });
    // Home forgot to take the daisy down next door, which a valid state could not even say: the
    // result is refused as it is read, so Desktop never sees it.
    let mut state = visit.previous.clone();
    state.set_household(home);
    assert!(encode(&result(&visit, state)).is_err());
}

#[test]
fn changes_to_another_household_are_set_aside() {
    let visit = visit();
    let mut state = visit.previous.clone();
    let next_door = visit.snapshot.village[1].keeper;
    state.household_mut(next_door).unwrap().rooms[0].width = 12;
    state.set_household(arranged(&visit.snapshot));
    let accepted = accept_result(
        &visit.seal,
        &visit.snapshot,
        &visit.previous,
        &result(&visit, state),
    );
    assert_eq!(accepted.set_aside, vec!["another_household_changed"]);
    assert_eq!(
        accepted.state.household(next_door).unwrap().rooms[0].width,
        6
    );
}

#[test]
fn something_the_colony_does_not_have_is_never_kept() {
    let visit = visit();
    let mut home = arranged(&visit.snapshot);
    home.rooms[0].displays.push(PlacedDisplay {
        item: DisplayId::find(150),
        spot: Spot::On { piece: 1, slot: 1 },
    });
    home.rooms[0].displays.push(PlacedDisplay {
        item: DisplayId::souvenir("sovereign_crown").unwrap(),
        spot: Spot::On { piece: 1, slot: 2 },
    });
    // A shell is not something to stand on the floor.
    home.rooms[0].displays.push(PlacedDisplay {
        item: DisplayId::find(3),
        spot: Spot::Floor { x: 1, y: 1 },
    });
    home.rooms[0].displays.remove(0);
    let accepted = accept_result(
        &visit.seal,
        &visit.snapshot,
        &visit.previous,
        &proposing(&visit, home),
    );
    assert_eq!(accepted.set_aside, vec!["display_not_kept"]);
    let kept: Vec<_> = accepted
        .state
        .household(visit.snapshot.household.keeper)
        .unwrap()
        .shown()
        .cloned()
        .collect();
    assert_eq!(kept, vec![DisplayId::find(76), DisplayId::find(132)]);
}

#[test]
fn an_answer_for_another_visit_or_colony_changes_nothing() {
    let visit = visit();
    let home = arranged(&visit.snapshot);
    let mut stale = proposing(&visit, home.clone());
    stale.state_sha256 = "ee".repeat(32);
    let mut foreign = proposing(&visit, home);
    foreign.state.colony_key = "ffffffffffffffff".to_owned();
    let mut empty = proposing(&visit, arranged(&visit.snapshot));
    empty
        .state
        .households
        .retain(|h| h.keeper != visit.snapshot.household.keeper);
    for (answer, why) in [
        (stale, "result_for_another_visit"),
        (foreign, "result_for_another_colony"),
        (empty, "result_without_this_household"),
    ] {
        let accepted = accept_result(&visit.seal, &visit.snapshot, &visit.previous, &answer);
        assert_eq!(accepted.set_aside, vec![why]);
        assert_eq!(accepted.state, visit.previous);
    }
}

#[test]
fn a_household_whose_keeper_has_gone_lets_its_finds_go() {
    let visit = visit();
    let mut previous = visit.previous.clone();
    let mut gone = neighbours_home(TravelerId(0xdead));
    gone.rooms[0].displays.clear();
    gone.rooms[0].displays.push(PlacedDisplay {
        item: DisplayId::find(0),
        spot: Spot::Wall {
            side: WallSide::North,
            at: 0,
        },
    });
    previous.households.push(gone);
    let settled = previous.settled_for(&visit.snapshot);
    assert!(settled.household(TravelerId(0xdead)).is_none());
    assert_eq!(settled.shown_by(&DisplayId::find(0)), None);
}

#[test]
fn homes_kept_for_another_colony_are_never_shown_to_this_one() {
    let visit = visit();
    let mut other = visit.previous.clone();
    other.colony_key = "ffffffffffffffff".to_owned();
    let settled = other.settled_for(&visit.snapshot);
    assert!(settled.households.is_empty());
    assert_eq!(settled.colony_key, visit.snapshot.colony_key);
}

#[test]
fn only_those_who_live_here_keep_what_they_have_come_to_like() {
    let visit = visit();
    let mut home = arranged(&visit.snapshot);
    let resident = visit.snapshot.residents[0].id;
    let visitor = visit.snapshot.visitors[0].id;
    home.note_use(resident, Liked::Piece { room: 0, uid: 2 });
    home.note_use(resident, Liked::Piece { room: 0, uid: 2 });
    home.note_use(
        resident,
        Liked::Shown {
            item: DisplayId::find(3),
        },
    );
    home.note_use(visitor, Liked::Piece { room: 0, uid: 2 });
    let accepted = accept_result(
        &visit.seal,
        &visit.snapshot,
        &visit.previous,
        &proposing(&visit, home),
    );
    assert_eq!(accepted.set_aside, vec!["liking_not_kept"]);
    let kept = &accepted
        .state
        .household(visit.snapshot.household.keeper)
        .unwrap()
        .likings;
    assert_eq!(kept.len(), 2);
    assert!(kept.iter().all(|liking| liking.resident == resident));
    assert_eq!(kept[0].uses, 2);
}

#[test]
fn a_liking_for_a_piece_taken_away_is_forgotten_with_it() {
    let visit = visit();
    let mut home = arranged(&visit.snapshot);
    let resident = visit.snapshot.residents[0].id;
    home.note_use(resident, Liked::Piece { room: 0, uid: 2 });
    home.rooms[0].pieces.retain(|piece| piece.uid != 2);
    assert!(
        HomeState {
            households: vec![home.clone()],
            ..visit.previous.clone()
        }
        .validate()
        .is_err(),
        "a liking for something not there does not add up"
    );
    home.forget_what_is_gone();
    assert!(home.likings.is_empty());
}

#[test]
fn a_household_keeps_its_own_keepsakes_on_show_but_none_can_move_house() {
    let visit = visit();
    let keeper = visit.snapshot.household.keeper;
    let mut home = arranged(&visit.snapshot);
    home.mementos.push(Memento {
        serial: 1,
        kind: MementoKind::Drawing,
        by: Some(visit.snapshot.residents[1].id),
        of: vec![keeper],
        inks: Vec::new(),
        made_at_utc: closed_at(),
    });
    home.rooms[0].displays.push(PlacedDisplay {
        item: DisplayId::memento(keeper, 1),
        spot: Spot::Wall {
            side: WallSide::West,
            at: 3,
        },
    });
    home.likings.push(Liking {
        resident: keeper,
        thing: Liked::Shown {
            item: DisplayId::memento(keeper, 1),
        },
        uses: 4,
    });
    let accepted = accept_result(
        &visit.seal,
        &visit.snapshot,
        &visit.previous,
        &proposing(&visit, home.clone()),
    );
    assert_eq!(accepted.set_aside, Vec::<&str>::new());
    assert_eq!(accepted.state.household(keeper), Some(&home));
    // Shown next door, a keepsake from this house does not check out.
    let mut stray = home.clone();
    stray.mementos.clear();
    let mut state = visit.previous.clone();
    state.set_household(stray);
    assert!(state.validate().is_err());
}

#[test]
fn only_those_who_live_here_can_be_kept_out_and_whoever_moves_out_is_let_go() {
    let visit = visit();
    let mut home = arranged(&visit.snapshot);
    let little = visit.snapshot.residents[1].id;
    let visitor = visit.snapshot.visitors[0].id;
    home.stays_out = vec![little, visitor];
    let accepted = accept_result(
        &visit.seal,
        &visit.snapshot,
        &visit.previous,
        &proposing(&visit, home),
    );
    assert_eq!(accepted.set_aside, vec!["staying_out_not_kept"]);
    let keeper = visit.snapshot.household.keeper;
    assert_eq!(
        accepted.state.household(keeper).unwrap().stays_out,
        vec![little],
        "a visitor comes or not with each visit"
    );
    let mut moved_out = visit.snapshot.clone();
    moved_out.residents.retain(|resident| resident.id != little);
    assert!(
        accepted
            .state
            .settled_for(&moved_out)
            .household(keeper)
            .unwrap()
            .stays_out
            .is_empty()
    );
}
