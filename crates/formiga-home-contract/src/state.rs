//! The houses as Home has arranged them: kept by Desktop beside the colony and never inside it,
//! so a colony without Home, or with Home removed, is exactly the colony it was.
//!
//! Only what Home owns is here: each household's rooms, what stands in them and what is shown
//! where. Pieces and surfaces are named by Home's catalogue identifiers and placed on whole floor
//! tiles; nothing here is a path, a script, a picture, or prose.

use crate::document::{HomeDocument, HomeError, header_ok, is_lower_hex};
use crate::ids::{CatalogId, DisplayId};
use crate::inventory::Ink;
use crate::limits::*;
use crate::{HOME_FORMAT_VERSION, STATE_FORMAT};
use formiga_travel::TravelerId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use time::OffsetDateTime;

/// One of the two walls a room shows: the far ones, which a cutaway leaves standing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallSide {
    /// Along the room's width, at the back on the right.
    North,
    /// Along the room's depth, at the back on the left.
    West,
}

/// Where something is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Spot {
    /// On one of a piece's surfaces: a table top, a shelf, a case. `slot` counts that piece's
    /// places from 0.
    On { piece: u16, slot: u8 },
    /// Hung on a wall, `at` tiles along it.
    Wall { side: WallSide, at: u8 },
    /// Standing on a floor tile of its own.
    Floor { x: u8, y: u8 },
}

/// Where a room stands on its house's plan: the far corner of its floor, in tiles from the first
/// room's. The plan's `x` runs along the rooms' width and its `y` along their depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlanPoint {
    pub x: i8,
    pub y: i8,
}

/// A doorway in one of a room's two far walls, `at` tiles along it. It opens onto whatever is on
/// the wall's other side: another room of the house, or outside, which is the way visitors come in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Door {
    pub side: WallSide,
    pub at: u8,
}

/// A piece of furniture where it stands. `uid` names this piece in its room, so what is shown on
/// it moves with it; `turn` is quarter turns from the piece's own front.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacedPiece {
    pub uid: u16,
    pub piece: CatalogId,
    pub x: u8,
    pub y: u8,
    #[serde(default)]
    pub turn: u8,
}

/// Something the colony has, shown somewhere in a room.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacedDisplay {
    pub item: DisplayId,
    pub spot: Spot,
}

/// One room: its size in floor tiles, what its floor and walls are, and what is in it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomLayout {
    pub width: u8,
    pub depth: u8,
    pub floor: CatalogId,
    pub wall: CatalogId,
    #[serde(default)]
    pub pieces: Vec<PlacedPiece>,
    #[serde(default)]
    pub displays: Vec<PlacedDisplay>,
    /// Where the room stands on the house's plan, since version 3. The first room stands at the
    /// plan's origin; a room with no place given is set out by Home beside the others.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<PlanPoint>,
    /// What kind of room it is, by Home's catalogue: a nook, a gallery. Since version 3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<CatalogId>,
    /// Its doorways, since version 3.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doors: Vec<Door>,
}

impl RoomLayout {
    pub fn piece(&self, uid: u16) -> Option<&PlacedPiece> {
        self.pieces.iter().find(|piece| piece.uid == uid)
    }

    /// How long one of its far walls is, in tiles.
    pub fn wall_length(&self, side: WallSide) -> u8 {
        match side {
            WallSide::North => self.width,
            WallSide::West => self.depth,
        }
    }

    /// Whether there is a doorway `at` tiles along the wall on `side`.
    pub fn has_door(&self, side: WallSide, at: u8) -> bool {
        self.doors.contains(&Door { side, at })
    }

    fn validate(&self) -> Result<(), HomeError> {
        let invalid = HomeError::invalid;
        let sides = MIN_ROOM_TILES..=MAX_ROOM_TILES;
        if !sides.contains(&self.width) || !sides.contains(&self.depth) {
            return Err(invalid("a room of a size no room can be"));
        }
        if self.doors.len() > MAX_DOORS {
            return Err(invalid("a room with too many doors"));
        }
        let mut doors = BTreeSet::new();
        for door in &self.doors {
            if door.at >= self.wall_length(door.side) || !doors.insert(*door) {
                return Err(invalid("a door that is not in a wall"));
            }
        }
        let mut uids = BTreeSet::new();
        for piece in &self.pieces {
            if piece.x >= self.width || piece.y >= self.depth || piece.turn >= TURNS {
                return Err(invalid("a piece stands outside its room"));
            }
            if !uids.insert(piece.uid) {
                return Err(invalid("two pieces in a room share a name"));
            }
        }
        let mut spots = BTreeSet::new();
        for shown in &self.displays {
            let inside = match shown.spot {
                Spot::On { piece, .. } => uids.contains(&piece),
                Spot::Wall {
                    side: WallSide::North,
                    at,
                } => at < self.width,
                Spot::Wall {
                    side: WallSide::West,
                    at,
                } => at < self.depth,
                Spot::Floor { x, y } => x < self.width && y < self.depth,
            };
            if !inside {
                return Err(invalid(
                    "something is shown somewhere its room does not have",
                ));
            }
            if !spots.insert(shown.spot) {
                return Err(invalid("two things are shown in one place"));
            }
            if let Spot::Wall { side, at } = shown.spot
                && self.has_door(side, at)
            {
                return Err(invalid("something hangs in a doorway"));
            }
        }
        Ok(())
    }

    /// The tiles it covers on the house's plan, as `(left, top, right, bottom)`, ends excluded,
    /// if it has been given a place.
    pub fn plan_extent(&self) -> Option<(i32, i32, i32, i32)> {
        let at = self.plan?;
        let (x, y) = (i32::from(at.x), i32::from(at.y));
        Some((x, y, x + i32::from(self.width), y + i32::from(self.depth)))
    }
}

/// One household's home.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HouseholdHome {
    /// The companion who keeps the house, as Desktop names it.
    pub keeper: TravelerId,
    /// The first room is the one the house opens into.
    pub rooms: Vec<RoomLayout>,
    /// What the household's residents have come to like in it, since version 2.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub likings: Vec<Liking>,
    /// What the household has been given or made at home, since version 4: Home's own
    /// keepsakes, shown like anything the colony has, in this house only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mementos: Vec<Memento>,
    /// What has happened at home worth remembering, oldest first, since version 4. Every entry
    /// is structured; Home words it, and it is read by nothing else.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub journal: Vec<JournalEntry>,
}

/// What a keepsake made at home is: a fixed catalogue, so whoever reads one can name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MementoKind {
    /// Left by a visitor.
    Postcard,
    JamJar,
    PressedFlower,
    Rosette,
    Pebble,
    /// Drawn by a little one.
    Drawing,
    /// A photo the owner took and framed, of whoever was in it.
    Photo,
    /// A kind a newer Home makes. Shown on its card.
    #[serde(other)]
    Unknown,
}

/// A keepsake the household came by at home.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memento {
    /// Its number in the household: it is shown as [`DisplayId::memento`] of the keeper and
    /// this.
    pub serial: u16,
    pub kind: MementoKind,
    /// Who it came from: the visitor who left it, the little one who drew it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<TravelerId>,
    /// Who is in it: whom a drawing is of, who was in a photo.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub of: Vec<TravelerId>,
    /// The colours of each of `of`, in turn, as they were when it was made: a photo keeps its
    /// likeness when whoever is in it is not at home.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inks: Vec<Ink>,
    #[serde(with = "time::serde::rfc3339")]
    pub made_at_utc: OffsetDateTime,
}

/// Something that happened at home worth a line: in the household's journal, and, a few at a
/// time, in Desktop's, each in its reader's own words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HomeMoment {
    /// A friend from another house came over.
    Visit { visitor: TravelerId },
    /// A keepsake came to the house: left by someone, drawn by someone, or a photo framed.
    Memento {
        memento: MementoKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<TravelerId>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        of: Vec<TravelerId>,
    },
    /// A resident took to something of a kind: a seat, a bed, a toy or a find.
    Favourite {
        resident: TravelerId,
        thing: FavouriteKind,
    },
    /// The house grew a room, of a kind in Home's catalogue.
    Room { room: CatalogId },
    /// A friend stayed over, the whole visit, and slept here. Since version 5.
    StayedOver { visitor: TravelerId },
    /// The owner asked a friend to come and live here, and it would like to. Since version 6.
    AskedToMoveIn { visitor: TravelerId },
    /// A moment a newer Home records.
    #[serde(other)]
    Unknown,
}

/// What a favourite is a favourite of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FavouriteKind {
    Seat,
    Bed,
    Toy,
    Find,
    #[serde(other)]
    Unknown,
}

/// A moment in the household's journal, and when.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    #[serde(with = "time::serde::rfc3339")]
    pub at_utc: OffsetDateTime,
    pub moment: HomeMoment,
}

impl HomeMoment {
    /// Everyone it names, for checking it is bounded.
    fn names(&self) -> usize {
        match self {
            Self::Memento { of, .. } => of.len(),
            _ => 0,
        }
    }
}

impl Memento {
    /// How it is shown, in the house of `keeper`.
    pub fn id(&self, keeper: TravelerId) -> DisplayId {
        DisplayId::memento(keeper, self.serial)
    }
}

/// Something in a home a resident can come to like.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Liked {
    /// A piece of furniture, by its room and its name there: a seat, a bed, a toy.
    Piece { room: u8, uid: u16 },
    /// Something the colony has, wherever in the house it is shown.
    Shown { item: DisplayId },
}

/// How often a resident has chosen something in its home: the seat it keeps going back to, its
/// toy, the find it keeps looking at. What a resident likes most of a kind is its favourite.
/// Flavour only: kept by Home, and read by nothing else.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Liking {
    pub resident: TravelerId,
    pub thing: Liked,
    pub uses: u16,
}

impl HouseholdHome {
    /// Furniture and displays together, across every room.
    pub fn placed(&self) -> usize {
        self.rooms
            .iter()
            .map(|room| room.pieces.len() + room.displays.len())
            .sum()
    }

    /// Every thing this household shows, in every room.
    pub fn shown(&self) -> impl Iterator<Item = &DisplayId> {
        self.rooms
            .iter()
            .flat_map(|room| room.displays.iter().map(|shown| &shown.item))
    }

    /// The keepsake shown as `id`, if it is one of this household's.
    pub fn memento(&self, id: &DisplayId) -> Option<&Memento> {
        self.mementos
            .iter()
            .find(|memento| &memento.id(self.keeper) == id)
    }

    /// The next number free for a keepsake.
    pub fn next_memento(&self) -> u16 {
        self.mementos
            .iter()
            .map(|memento| memento.serial)
            .max()
            .map_or(1, |serial| serial.saturating_add(1))
    }

    /// Note a moment in the journal, the oldest let go when it is full.
    pub fn note(&mut self, at_utc: OffsetDateTime, moment: HomeMoment) {
        self.journal.push(JournalEntry { at_utc, moment });
        while self.journal.len() > MAX_JOURNAL {
            self.journal.remove(0);
        }
    }

    /// Forget likings for anything no longer in the house: a piece taken away, a room gone.
    pub fn forget_what_is_gone(&mut self) {
        let rooms = &self.rooms;
        self.likings.retain(|liking| match &liking.thing {
            Liked::Piece { room, uid } => rooms
                .get(usize::from(*room))
                .is_some_and(|layout| layout.piece(*uid).is_some()),
            Liked::Shown { .. } => true,
        });
    }

    /// One more use of `thing` by `resident`. Kept within bounds by forgetting the least used.
    pub fn note_use(&mut self, resident: TravelerId, thing: Liked) {
        match self
            .likings
            .iter_mut()
            .find(|liking| liking.resident == resident && liking.thing == thing)
        {
            Some(liking) => liking.uses = liking.uses.saturating_add(1),
            None => self.likings.push(Liking {
                resident,
                thing,
                uses: 1,
            }),
        }
        if self.likings.len() > MAX_LIKINGS {
            let least = self
                .likings
                .iter()
                .enumerate()
                .min_by_key(|(_, liking)| liking.uses)
                .map(|(index, _)| index);
            if let Some(index) = least {
                self.likings.remove(index);
            }
        }
    }

    /// Stop showing `item` anywhere in this household. Whether it was shown.
    pub fn take_down(&mut self, item: &DisplayId) -> bool {
        let mut found = false;
        for room in &mut self.rooms {
            let before = room.displays.len();
            room.displays.retain(|shown| &shown.item != item);
            found |= room.displays.len() != before;
        }
        found
    }

    fn validate(&self) -> Result<(), HomeError> {
        if self.rooms.is_empty() || self.rooms.len() > MAX_ROOMS {
            return Err(HomeError::invalid("a home of one to three rooms"));
        }
        if self.placed() > MAX_PLACED_PER_HOUSEHOLD {
            return Err(HomeError::invalid("a home with too much in it"));
        }
        self.rooms.iter().try_for_each(RoomLayout::validate)?;
        // The first room is where the plan starts; no two rooms stand in one place, and the
        // house stays within reach of it.
        if self.rooms[0]
            .plan
            .is_some_and(|at| at != PlanPoint { x: 0, y: 0 })
        {
            return Err(HomeError::invalid(
                "the first room is where the plan starts",
            ));
        }
        let origin = PlanPoint { x: 0, y: 0 };
        let extents: Vec<_> = self
            .rooms
            .iter()
            .enumerate()
            .filter_map(|(index, room)| match index {
                0 => RoomLayout {
                    plan: Some(origin),
                    ..room.clone()
                }
                .plan_extent(),
                _ => room.plan_extent(),
            })
            .collect();
        let reach = i32::from(MAX_PLAN_REACH);
        for (index, a) in extents.iter().enumerate() {
            if a.0 < -reach || a.1 < -reach || a.2 > reach || a.3 > reach {
                return Err(HomeError::invalid(
                    "a room too far from the rest of its house",
                ));
            }
            let overlaps = extents[index + 1..]
                .iter()
                .any(|b| a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3);
            if overlaps {
                return Err(HomeError::invalid("two rooms stand in one place"));
            }
        }
        if self.likings.len() > MAX_LIKINGS {
            return Err(HomeError::invalid("a home with too many likings"));
        }
        if self.mementos.len() > MAX_MEMENTOS || self.journal.len() > MAX_JOURNAL {
            return Err(HomeError::invalid("a home with too much remembered in it"));
        }
        let mut serials = BTreeSet::new();
        for memento in &self.mementos {
            if !serials.insert(memento.serial)
                || memento.of.len() > MAX_IN_A_MEMENTO
                || memento.inks.len() > memento.of.len()
            {
                return Err(HomeError::invalid("a keepsake that does not add up"));
            }
        }
        if self
            .journal
            .iter()
            .any(|entry| entry.moment.names() > MAX_IN_A_MEMENTO)
        {
            return Err(HomeError::invalid("a journal entry that does not add up"));
        }
        // A keepsake is shown only in its own house.
        for room in &self.rooms {
            for shown in &room.displays {
                if shown.item.source() == DisplayId::MEMENTO && self.memento(&shown.item).is_none()
                {
                    return Err(HomeError::invalid(
                        "a keepsake shown in a house it is not from",
                    ));
                }
            }
        }
        let mut seen = BTreeSet::new();
        for liking in &self.likings {
            let there = match &liking.thing {
                Liked::Piece { room, uid } => self
                    .rooms
                    .get(usize::from(*room))
                    .is_some_and(|layout| layout.piece(*uid).is_some()),
                Liked::Shown { .. } => true,
            };
            if !there || liking.uses == 0 || !seen.insert((liking.resident, &liking.thing)) {
                return Err(HomeError::invalid("a liking that does not add up"));
            }
        }
        Ok(())
    }
}

/// Every household's home, for one colony.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomeState {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    /// The colony these homes belong to: a snapshot's `colony_key`. Homes kept for any other
    /// colony are never shown to this one.
    pub colony_key: String,
    #[serde(default)]
    pub households: Vec<HouseholdHome>,
}

impl HomeState {
    /// No homes arranged yet: every house opens as it first would.
    pub fn new(colony_key: &str) -> Self {
        Self {
            format: STATE_FORMAT.to_owned(),
            version: HOME_FORMAT_VERSION,
            min_reader_version: 1,
            colony_key: colony_key.to_owned(),
            households: Vec::new(),
        }
    }

    pub fn household(&self, keeper: TravelerId) -> Option<&HouseholdHome> {
        self.households.iter().find(|home| home.keeper == keeper)
    }

    pub fn household_mut(&mut self, keeper: TravelerId) -> Option<&mut HouseholdHome> {
        self.households
            .iter_mut()
            .find(|home| home.keeper == keeper)
    }

    /// The household that shows `item`, if any does.
    pub fn shown_by(&self, item: &DisplayId) -> Option<TravelerId> {
        self.households
            .iter()
            .find(|home| home.shown().any(|shown| shown == item))
            .map(|home| home.keeper)
    }

    /// Put `home` in place of the household's own, or add it.
    pub fn set_household(&mut self, home: HouseholdHome) {
        match self.household_mut(home.keeper) {
            Some(kept) => *kept = home,
            None => self.households.push(home),
        }
    }
}

impl HomeDocument for HomeState {
    const FORMAT: &'static str = STATE_FORMAT;
    const MAX_BYTES: u64 = MAX_STATE_BYTES;

    fn validate(&self) -> Result<(), HomeError> {
        let invalid = HomeError::invalid;
        if !header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            STATE_FORMAT,
        ) {
            return Err(invalid("the state's header is not one this build writes"));
        }
        if !is_lower_hex(&self.colony_key, 16) {
            return Err(invalid("a colony key is 16 lowercase hex digits"));
        }
        if self.households.len() > MAX_HOUSEHOLDS {
            return Err(invalid("too many households"));
        }
        let mut keepers = BTreeSet::new();
        let mut shown = BTreeSet::new();
        for home in &self.households {
            if !keepers.insert(home.keeper) {
                return Err(invalid("a household is listed twice"));
            }
            home.validate()?;
            for item in home.shown() {
                // One thing, one place: moving a find to another house moves it, never copies it.
                if !shown.insert(item) {
                    return Err(invalid("one thing is shown in two places"));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode, encode};

    pub(crate) fn room() -> RoomLayout {
        RoomLayout {
            width: 8,
            depth: 8,
            floor: CatalogId::known("floor.boards"),
            wall: CatalogId::known("wall.plaster"),
            pieces: vec![PlacedPiece {
                uid: 1,
                piece: CatalogId::known("shelf"),
                x: 0,
                y: 2,
                turn: 1,
            }],
            displays: vec![
                PlacedDisplay {
                    item: DisplayId::find(3),
                    spot: Spot::On { piece: 1, slot: 0 },
                },
                PlacedDisplay {
                    item: DisplayId::find(76),
                    spot: Spot::Wall {
                        side: WallSide::North,
                        at: 5,
                    },
                },
            ],
            plan: None,
            kind: None,
            doors: Vec::new(),
        }
    }

    fn state() -> HomeState {
        let mut state = HomeState::new("0123456789abcdef");
        state.households.push(HouseholdHome {
            keeper: TravelerId(7),
            rooms: vec![room()],
            likings: Vec::new(),
            mementos: Vec::new(),
            journal: Vec::new(),
        });
        state
    }

    #[test]
    fn a_state_round_trips_byte_for_byte() {
        let bytes = encode(&state()).unwrap();
        let read: HomeState = decode(&bytes).unwrap();
        assert_eq!(read, state());
        assert_eq!(encode(&read).unwrap(), bytes);
    }

    #[test]
    fn one_find_cannot_be_shown_in_two_houses() {
        let mut twice = state();
        twice.households.push(HouseholdHome {
            keeper: TravelerId(8),
            rooms: vec![room()],
            likings: Vec::new(),
            mementos: Vec::new(),
            journal: Vec::new(),
        });
        assert!(twice.validate().is_err());
        twice.households[1].take_down(&DisplayId::find(3));
        twice.households[1].take_down(&DisplayId::find(76));
        assert!(twice.validate().is_ok());
    }

    #[test]
    fn layouts_that_do_not_add_up_are_refused() {
        let mut outside = state();
        outside.households[0].rooms[0].pieces[0].x = 8;
        let mut turned = state();
        turned.households[0].rooms[0].pieces[0].turn = TURNS;
        let mut floating = state();
        floating.households[0].rooms[0].displays[0].spot = Spot::On { piece: 9, slot: 0 };
        let mut high = state();
        high.households[0].rooms[0].displays[1].spot = Spot::Wall {
            side: WallSide::West,
            at: 8,
        };
        let mut crowded = state();
        crowded.households[0].rooms[0].displays[1].spot = Spot::On { piece: 1, slot: 0 };
        let mut huge = state();
        huge.households[0].rooms[0].width = MAX_ROOM_TILES + 1;
        let mut rambling = state();
        rambling.households[0].rooms = vec![room(); MAX_ROOMS + 1];
        let mut roofless = state();
        roofless.households[0].rooms.clear();
        let mut cluttered = state();
        cluttered.households[0].rooms[0].pieces = (0..=MAX_PLACED_PER_HOUSEHOLD as u16)
            .map(|uid| PlacedPiece {
                uid,
                piece: CatalogId::known("cushion"),
                x: 0,
                y: 0,
                turn: 0,
            })
            .collect();
        let mut stranger = state();
        stranger.colony_key = "not a key".to_owned();
        for state in [
            outside, turned, floating, high, crowded, huge, rambling, roofless, cluttered, stranger,
        ] {
            assert!(state.validate().is_err(), "{state:?} was accepted");
        }
    }

    /// The front room and a nook behind it, through a door in the front room's far wall.
    fn two_rooms() -> HomeState {
        let mut state = state();
        let home = &mut state.households[0];
        home.rooms[0].doors.push(Door {
            side: WallSide::West,
            at: 6,
        });
        home.rooms.push(RoomLayout {
            width: 4,
            depth: 4,
            floor: CatalogId::known("floor.rose"),
            wall: CatalogId::known("wall.stripes"),
            pieces: Vec::new(),
            displays: Vec::new(),
            plan: Some(PlanPoint { x: 2, y: -4 }),
            kind: Some(CatalogId::known("room.nook")),
            doors: Vec::new(),
        });
        // The nook's way in is the doorway in the front room's far wall it backs onto.
        home.rooms[0].doors.push(Door {
            side: WallSide::North,
            at: 3,
        });
        state
    }

    #[test]
    fn a_house_of_rooms_round_trips_and_older_rooms_are_written_as_they_were() {
        let house = two_rooms();
        house.validate().unwrap();
        let bytes = encode(&house).unwrap();
        assert_eq!(decode::<HomeState>(&bytes).unwrap(), house);
        // A room with nothing new about it is written exactly as version 2 wrote it.
        let one_room = serde_json::to_value(room()).unwrap();
        for field in ["plan", "kind", "doors"] {
            assert!(one_room.get(field).is_none(), "{field}");
        }
    }

    #[test]
    fn doors_and_rooms_that_do_not_add_up_are_refused() {
        let mut off_the_wall = two_rooms();
        off_the_wall.households[0].rooms[1].doors.push(Door {
            side: WallSide::North,
            at: 4,
        });
        let mut many_doors = two_rooms();
        many_doors.households[0].rooms[0].doors = (0..=MAX_DOORS as u8)
            .map(|at| Door {
                side: WallSide::West,
                at,
            })
            .collect();
        let mut hung_in_a_doorway = two_rooms();
        hung_in_a_doorway.households[0].rooms[0].doors.push(Door {
            side: WallSide::North,
            at: 5,
        });
        let mut on_top_of_each_other = two_rooms();
        on_top_of_each_other.households[0].rooms[1].plan = Some(PlanPoint { x: 5, y: 5 });
        let mut moved_the_start = two_rooms();
        moved_the_start.households[0].rooms[0].plan = Some(PlanPoint { x: 1, y: 0 });
        let mut far_away = two_rooms();
        far_away.households[0].rooms[1].plan = Some(PlanPoint {
            x: MAX_PLAN_REACH,
            y: 0,
        });
        for state in [
            off_the_wall,
            many_doors,
            hung_in_a_doorway,
            on_top_of_each_other,
            moved_the_start,
            far_away,
        ] {
            assert!(state.validate().is_err(), "{state:?} was accepted");
        }
    }

    #[test]
    fn identifiers_that_are_not_identifiers_are_refused_when_read() {
        let mut value = serde_json::to_value(state()).unwrap();
        value["households"][0]["rooms"][0]["pieces"][0]["piece"] = "../../colony.json".into();
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(decode::<HomeState>(&bytes).is_err());
    }

    fn ink() -> Ink {
        Ink {
            outline: [60, 40, 30],
            deep: [120, 90, 70],
            body: [200, 170, 140],
            light: [230, 210, 190],
            accent: [180, 60, 80],
        }
    }

    #[test]
    fn a_household_s_keepsakes_and_journal_are_its_own_and_bounded() {
        let mut house = state();
        let keeper = house.households[0].keeper;
        let home = &mut house.households[0];
        home.mementos.push(Memento {
            serial: 1,
            kind: MementoKind::Photo,
            by: None,
            of: vec![keeper, TravelerId(8)],
            inks: Vec::new(),
            made_at_utc: time::macros::datetime!(2026-10-05 12:00 UTC),
        });
        home.rooms[0].displays.push(PlacedDisplay {
            item: DisplayId::memento(keeper, 1),
            spot: Spot::Floor { x: 4, y: 4 },
        });
        for day in 0..(MAX_JOURNAL + 5) {
            home.note(
                time::macros::datetime!(2026-10-05 12:00 UTC) + time::Duration::days(day as i64),
                HomeMoment::Visit {
                    visitor: TravelerId(8),
                },
            );
        }
        assert_eq!(home.journal.len(), MAX_JOURNAL, "the oldest are let go");
        assert_eq!(home.next_memento(), 2);
        house.validate().unwrap();
        let bytes = encode(&house).unwrap();
        assert_eq!(decode::<HomeState>(&bytes).unwrap(), house);

        let mut from_elsewhere = house.clone();
        from_elsewhere.households[0].rooms[0]
            .displays
            .push(PlacedDisplay {
                item: DisplayId::memento(TravelerId(99), 1),
                spot: Spot::Floor { x: 5, y: 5 },
            });
        let mut doubled = house.clone();
        let copy = doubled.households[0].mementos[0].clone();
        doubled.households[0].mementos.push(copy);
        let mut crowded = house.clone();
        crowded.households[0].mementos[0].of = vec![keeper; MAX_IN_A_MEMENTO + 1];
        let mut overdrawn = house.clone();
        overdrawn.households[0].mementos[0].inks = vec![ink(); 3];
        for state in [from_elsewhere, doubled, crowded, overdrawn] {
            assert!(state.validate().is_err(), "{state:?} was accepted");
        }
    }
}
