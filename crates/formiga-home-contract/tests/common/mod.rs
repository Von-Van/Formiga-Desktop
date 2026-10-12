//! The sample household, and a home arranged in it, for the contract's tests.

#![allow(dead_code)]

use formiga_home_contract::*;
use time::macros::datetime;

pub fn session() -> SessionId {
    SessionId::parse(sample::SESSION).unwrap()
}

/// The sample household's home with a little in it: a shelf with the shell on it, the postcard on
/// the wall, and the toy sheep on the floor.
pub fn arranged(snapshot: &HomeSnapshot) -> HouseholdHome {
    HouseholdHome {
        keeper: snapshot.household.keeper,
        rooms: vec![RoomLayout {
            width: 8,
            depth: 8,
            floor: CatalogId::known("floor.boards"),
            wall: CatalogId::known("wall.plaster"),
            pieces: vec![
                PlacedPiece {
                    uid: 1,
                    piece: CatalogId::known("shelf"),
                    x: 0,
                    y: 2,
                    turn: 1,
                },
                PlacedPiece {
                    uid: 2,
                    piece: CatalogId::known("armchair"),
                    x: 3,
                    y: 3,
                    turn: 0,
                },
            ],
            displays: vec![
                PlacedDisplay {
                    item: DisplayId::find(3),
                    spot: Spot::On { piece: 1, slot: 0 },
                },
                PlacedDisplay {
                    item: DisplayId::find(76),
                    spot: Spot::Wall {
                        side: WallSide::North,
                        at: 4,
                    },
                },
                PlacedDisplay {
                    item: DisplayId::find(132),
                    spot: Spot::Floor { x: 6, y: 6 },
                },
            ],
            plan: None,
            kind: None,
            doors: Vec::new(),
        }],
        likings: Vec::new(),
        mementos: Vec::new(),
        journal: Vec::new(),
        stays_out: Vec::new(),
    }
}

/// A home for another house in the village, showing the pressed daisy and the ribbon.
pub fn neighbours_home(keeper: TravelerId) -> HouseholdHome {
    HouseholdHome {
        keeper,
        rooms: vec![RoomLayout {
            width: 6,
            depth: 6,
            floor: CatalogId::known("floor.checks"),
            wall: CatalogId::known("wall.stripes"),
            pieces: Vec::new(),
            displays: vec![
                PlacedDisplay {
                    item: DisplayId::find(26),
                    spot: Spot::Wall {
                        side: WallSide::West,
                        at: 1,
                    },
                },
                PlacedDisplay {
                    item: DisplayId::souvenir("picnic_ribbon").unwrap(),
                    spot: Spot::Wall {
                        side: WallSide::North,
                        at: 2,
                    },
                },
            ],
            plan: None,
            kind: None,
            doors: Vec::new(),
        }],
        likings: Vec::new(),
        mementos: Vec::new(),
        journal: Vec::new(),
        stays_out: Vec::new(),
    }
}

pub fn closed_at() -> time::OffsetDateTime {
    datetime!(2026-11-11 10:20 UTC)
}
