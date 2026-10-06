//! A sample household, for Home's previews and tests and for this crate's own: Desktop's own
//! sample colony (every recipe edition, a companion from before recipes, a little one, something
//! worn, something pinned, habits, bonds of every strength), given a scrapbook with something of
//! every way a find can be shown and a few of Formiga Hill's souvenirs. It is the same every time.

use crate::{
    HomeCapability, HomeSnapshot, HomeState, SessionId, likely_visitors, project_household,
};
use formiga_core::{SaveFile, Souvenir, SouvenirRecord, house_owners};
use time::{Duration, OffsetDateTime};

/// When the sample colony was made.
pub const MADE: OffsetDateTime = formiga_travel::sample::MADE;

/// How long the sample colony had lived when its house was opened.
pub const DAYS_LIVED: i64 = 40;

/// The sample visit's session.
pub const SESSION: &str = "0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e";

/// The finds in the sample scrapbook, and how many days into the colony's life each turned up:
/// things for tables and shelves, for cases, for walls and for the floor.
pub const FINDS: [(u8, i64); 18] = [
    (3, 1),    // Shell
    (1, 2),    // Key
    (16, 3),   // Acorn
    (18, 4),   // Marble
    (76, 5),   // Postcard
    (23, 6),   // Ribbon
    (132, 8),  // Toy sheep
    (0, 9),    // Gem
    (26, 11),  // Pressed daisy
    (9, 13),   // Firefly jar
    (57, 15),  // Owl feather
    (89, 17),  // Teacup
    (135, 19), // Pocket watch
    (62, 22),  // Glow mushroom
    (12, 25),  // Ticket stub
    (44, 28),  // Toy block
    (159, 31), // Rainbow marble
    (98, 35),  // Recipe card
];

/// The souvenirs the sample colony has brought home, in the order they came.
pub const SOUVENIRS: [Souvenir; 4] = [
    Souvenir::PicnicRibbon,
    Souvenir::PressedDaisy,
    Souvenir::ChestMarble,
    Souvenir::FairTicket,
];

/// The sample colony, with its scrapbook and souvenirs.
pub fn colony() -> SaveFile {
    let mut save = formiga_travel::sample::colony(3);
    let finders: Vec<_> = save
        .creatures
        .iter()
        .map(|creature| (creature.id, creature.name.clone()))
        .collect();
    for (index, (variant, day)) in FINDS.into_iter().enumerate() {
        let (finder, name) = finders[index % finders.len()].clone();
        save.companion
            .remember_discovery(variant, finder, name, MADE + Duration::days(day));
    }
    save.trips.souvenirs = SOUVENIRS
        .into_iter()
        .enumerate()
        .map(|(index, souvenir)| SouvenirRecord {
            souvenir,
            brought_home_at_utc: MADE + Duration::days(20 + 4 * index as i64),
        })
        .collect();
    save
}

/// The house the sample opens: the colony house, where the founder lives with its little one.
pub fn keeper(save: &SaveFile) -> formiga_core::CreatureId {
    house_owners(&save.creatures, &save.home.cottage_order).as_slice()[0]
}

/// The sample household's snapshot, as Desktop would write it, with its closest friends lent for
/// the visit and every capability offered, so a preview shows everything Home can do.
pub fn snapshot() -> HomeSnapshot {
    let save = colony();
    let keeper = keeper(&save);
    project_household(
        &save,
        keeper,
        &likely_visitors(&save, keeper),
        &HomeCapability::ALL,
        SessionId::parse(SESSION).expect("the sample session is a session id"),
        MADE + Duration::days(DAYS_LIVED),
        "sample colony",
    )
    .expect("the sample household can be opened")
}

/// The sample colony before anything has been arranged in any house.
pub fn state() -> HomeState {
    HomeState::new(&snapshot().colony_key)
}
