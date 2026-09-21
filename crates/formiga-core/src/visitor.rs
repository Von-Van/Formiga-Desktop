use crate::{Creature, CreatureId, CreatureOrigin, Gesture, Point};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// How many past visitors the guest book remembers.
pub const MAX_GUEST_BOOK_ENTRIES: usize = 24;

/// How many places a guest's walk around the village stops at, counting the spot it walked in
/// to. Three to five is a tour; more would be pacing, and a village laid out for four rarely
/// has the spare ground for more anyway.
pub const MAX_TOUR_STOPS: usize = 5;

/// Why an invitation was turned down. A code that will not decode at all is refused before it
/// ever reaches the colony, by the seed code itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum VisitorError {
    #[error("this companion already lives here")]
    AlreadyHome,
    #[error("someone is already visiting")]
    GuestPresent,
}

/// Why a visitor is here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VisitorSource {
    /// Someone passing through who stopped at the houses.
    #[default]
    Wanderer,
    /// A friend's creature, invited with its seed code.
    Invited,
}

/// How far through its visit a guest is. Runtime only, like a journey or an attention pose: a
/// visit that is interrupted by a relaunch begins again at the next gathering rather than
/// resuming halfway along the floor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VisitPhase {
    /// Off stage: between gatherings, or waiting for the residents to settle.
    #[default]
    Waiting,
    /// Walking in along the floor from the far side of the village.
    ArrivingWalk,
    /// Saying hello.
    Greeting,
    /// A few calm shared moments beside the houses.
    Visiting,
    /// Waving goodbye.
    Farewell,
    /// Walking back out the way it came.
    LeavingWalk,
    /// Done for this gathering.
    Gone,
}

/// What a guest has walked over to look at, once it has stopped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TourInterest {
    /// A house: the guest stands beside it, as close as the wall sliver allows, and looks up.
    House,
    /// Something the colony keeps: a belonging in the tree's yard, or a keepsake above it.
    Keepsake,
    /// Nothing in particular. The spot the guest walked in to, and comes back to.
    #[default]
    Ground,
}

/// One place on a guest's walk around the village: somewhere a creature may legitimately stand,
/// and what it turns to look at when it gets there. Worked out from the village's own layout, so
/// a tour follows wherever the houses, the shared ground and the trees' yards have ended up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TourStop {
    pub at: Point,
    pub look: Point,
    pub interest: TourInterest,
}

/// What the guest is doing this moment of its tour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TourMoment {
    /// On its way to the next stop.
    #[default]
    Walking,
    /// Saying hello to one resident it has not been over to yet.
    Greeting(CreatureId),
    /// Looking at whatever this stop came over for.
    Looking,
    /// A moment's rest, and then on.
    Resting,
}

/// One resident's answer to the hello: when it turns, how long it holds, and what its
/// temperament makes of the moment. A timid companion only looks; a playful one bounces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResidentAnswer {
    pub creature_id: CreatureId,
    /// Seconds after the hello before this one turns, so the colony answers in its own time.
    pub after: f32,
    /// Seconds it holds the answer once it has turned.
    pub hold: f32,
    pub gesture: Option<Gesture>,
}

/// Where an authored visit has got to. Never saved: see [`VisitPhase`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VisitProgress {
    pub phase: VisitPhase,
    /// Seconds spent in the current phase.
    pub elapsed: f32,
    /// Seconds since the home appeared, so arrival waits for the residents to settle.
    pub since_home: f32,
    /// Whether the hello has been said yet, and how long ago. The gap is what staggers the
    /// colony's answers; the flag is what makes a visit worth writing down.
    pub greeted: bool,
    pub since_hello: f32,
    /// Where the visitor walked in from, and where it walks back out to. A village with no room
    /// to walk in has none, and the guest simply steps out from beside the last house.
    pub doorway: Option<Point>,
    /// Which calm moment is being shown, and how much of it is left.
    pub beat: u8,
    pub beat_remaining: f32,
    /// How the colony answered the hello, and how it answers a hello said at a stop since. One
    /// entry per resident for the hello itself and at most one more each for the walk round, so
    /// this is never longer than twice a colony.
    pub answers: Vec<ResidentAnswer>,
    /// The guest's walk around the village: everywhere it stops, which stop it is standing at or
    /// heading for, what it is doing there, and how long that has left. The first stop is always
    /// the spot the guest walked in to, so the ring runs out along the houses and back again.
    /// Never more than [`MAX_TOUR_STOPS`] of them.
    pub stops: Vec<TourStop>,
    pub stop: u8,
    pub moment: TourMoment,
    pub stay: f32,
    /// The spot the stops were worked out from. A village that moves underneath the guest — a
    /// display going away, a habitat redrawn around it — is toured again from where it has
    /// ended up rather than walked as it used to be.
    pub planned: Option<Point>,
    /// The residents this guest has already been over to greet, so it meets somebody new each
    /// time it stops. At most one entry per colony member.
    pub met: Vec<CreatureId>,
}

/// A guest at the colony houses. A visitor is a whole creature so it draws, animates, and reads
/// like anyone else, but it is never a colony member: it holds no bond record, no cottage, and
/// no place in the colony's order, and it only ever appears while the home is out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Visitor {
    pub creature: Creature,
    #[serde(default)]
    pub source: VisitorSource,
    /// An invited guest keeps coming back to the houses until this time. A wanderer has none and
    /// leaves with the gathering it arrived at.
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub stays_until_utc: Option<OffsetDateTime>,
    /// Whether the guest is out on the desktop right now, rather than between gatherings.
    #[serde(default)]
    pub on_stage: bool,
    /// Whether this visit is already in the guest book. A day-long stay signs once, not once a
    /// gathering, so the book reads as a list of visitors rather than a list of appearances.
    #[serde(default)]
    pub signed: bool,
    #[serde(skip)]
    pub visit: VisitProgress,
}

impl Visitor {
    /// A guest who has not been on stage yet this gathering.
    pub fn new(
        creature: Creature,
        source: VisitorSource,
        stays_until_utc: Option<OffsetDateTime>,
    ) -> Self {
        Self {
            creature,
            source,
            stays_until_utc,
            on_stage: false,
            signed: false,
            visit: VisitProgress::default(),
        }
    }

    /// Whether an invited guest's day is up. A wanderer's stay is the gathering it arrived at, so
    /// it is never outstayed by a clock.
    pub fn outstayed(&self, now: OffsetDateTime) -> bool {
        self.stays_until_utc.is_some_and(|until| now >= until)
    }
}

/// One line of the guest book: who came by, when, and the origin that recreates them exactly.
/// Like a shared seed code, the origin carries appearance and temperament and nothing else.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GuestBookEntry {
    #[serde(with = "time::serde::rfc3339")]
    pub visited_at_utc: OffsetDateTime,
    pub name: String,
    pub origin: CreatureOrigin,
    #[serde(default)]
    pub source: VisitorSource,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VisitorState {
    /// Whoever is visiting, if anyone.
    pub guest: Option<Visitor>,
    /// How many home gatherings this colony has held. Whether a wanderer turns up at the next one
    /// is decided from the colony seed and this count, never from a clock.
    pub gatherings: u32,
    pub guest_book: Vec<GuestBookEntry>,
}

impl VisitorState {
    /// The guest as the desktop should draw it: only while it is actually out.
    pub fn on_stage(&self) -> Option<&Creature> {
        self.guest
            .as_ref()
            .filter(|guest| guest.on_stage)
            .map(|guest| &guest.creature)
    }

    /// Whether the colony has room to keep whoever is visiting. The settings window asks a saved
    /// colony this directly, so the rule that decides it is only written down once.
    pub fn can_stay(&self, creatures: &[Creature]) -> bool {
        let Some(guest) = &self.guest else {
            return false;
        };
        creatures.len() < crate::MAX_COLONY_CREATURES
            && creatures
                .iter()
                .filter(|member| member.role.is_adult())
                .count()
                < crate::MAX_ADULT_CREATURES
            && !creatures
                .iter()
                .any(|member| member.id == guest.creature.id)
    }

    /// Remember a visitor, oldest dropped once the book is full. This is a line in a book, not a
    /// change to the visit itself: whoever is here carries on exactly as they were.
    pub(crate) fn sign(&mut self, entry: GuestBookEntry) {
        self.guest_book.push(entry);
        self.trim();
    }

    fn trim(&mut self) {
        if self.guest_book.len() > MAX_GUEST_BOOK_ENTRIES {
            self.guest_book
                .drain(..self.guest_book.len() - MAX_GUEST_BOOK_ENTRIES);
        }
    }

    /// Settle a colony that has just been opened. A visit is a scene, not a saved position: any
    /// guest is found waiting between gatherings, and walks in again when the houses next appear.
    pub fn normalize(&mut self) {
        self.trim();
        if let Some(guest) = &mut self.guest {
            guest.on_stage = false;
            guest.visit = VisitProgress::default();
            guest.creature.state.attention = None;
            guest.creature.state.velocity = Point::default();
        }
    }
}
