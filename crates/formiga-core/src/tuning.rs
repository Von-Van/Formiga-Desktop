//! The colony's design values, grouped by feature.
//!
//! Everything here is a choice about how the colony feels — how often something happens, how long
//! it lasts, how far a companion will go for it, how likely it is — rather than about how the code
//! works. Each feature reads its values from one struct here, so retuning a feature is a change to
//! this file alone, and its diff reads as the retune it is.
//!
//! What does not belong here: limits the save format or memory depend on (`MAX_COLONY_CREATURES`
//! and the other caps live beside the data they bound), sizes the art decides (frame widths,
//! anchors), and the shape of an animation, which lives with the motion it draws.

use std::ops::{Range, RangeInclusive};

/// Things to play on that turn up now and then (`world/wonders.rs`).
pub struct Wonders {
    /// How long between one wonder and the next, in visible seconds.
    pub interval_secs: Range<f32>,
    /// How soon to look again when nobody was free for one, or nowhere would take it.
    pub retry_secs: Range<f32>,
    /// How long it takes to appear, and to go.
    pub appear_secs: f32,
    pub vanish_secs: f32,
    /// How long its players have to get there before it gives up on them.
    pub patience_secs: f32,
    /// How long the finder of a new kind stands marvelling at it before playing.
    pub marvel_secs: f32,
    /// How near counts as there, in points.
    pub arrived: f32,
    /// Further than this, in points, and the walk over turns into an eager run.
    pub run_from: f32,
    /// How far from its player a wonder on the player's own ground turns up, in frames.
    pub near_frames: Range<f32>,
    /// How far to either side a companion on the floor will go to climb up to one, in points.
    pub climb_reach: f32,
    /// How much of its own height a player needs clear above a ledge, below the top of the screen.
    pub headroom_frames: f32,
}

pub const WONDERS: Wonders = Wonders {
    interval_secs: 600.0..1200.0,
    retry_secs: 40.0..90.0,
    appear_secs: 0.45,
    vanish_secs: 0.6,
    patience_secs: 30.0,
    marvel_secs: 2.2,
    arrived: 1.5,
    run_from: 160.0,
    near_frames: 1.6..5.0,
    climb_reach: 420.0,
    headroom_frames: 1.4,
};

/// Trinkets turning up, on the desktop and at home (`world/discovery.rs`, `model.rs`).
pub struct Finds {
    /// How many trinkets a colony finds on one local day, drawn evenly from this range.
    pub per_day: RangeInclusive<u8>,
    /// Visible time between one find out on the desktop and the next: long enough that a day's
    /// few finds are spread across it rather than found in its first hour.
    pub desktop_interval_secs: Range<f32>,
    /// One in this many chores, garden visits and roof sits at home turns something up.
    pub at_home_in: u32,
    /// About half of the finds that could be conditional are, so a circumstance makes a keepsake
    /// likely without ever promising one.
    pub conditional_in: u32,
    /// Three times in four the pick prefers something the scrapbook has not seen yet, so an empty
    /// slot fills before a duplicate turns up again.
    pub prefer_undiscovered_in: u32,
    /// One find in this many is one of the rare ones, wherever and whenever it happens.
    pub rare_in: u32,
    /// "After dark", for the night finds: wider than the late-night ritual's 22..05, because it
    /// is about it being dark outside rather than the person still being up, and wide enough
    /// that someone who keeps ordinary evening hours can find them.
    pub evening_hour: u8,
    pub morning_hour: u8,
    /// The early part of the day, when the morning finds turn up: from first light until then.
    pub late_morning_hour: u8,
    /// How far down the nearest thing that would catch a fall has to be, in points, before a
    /// ledge counts as high: about a window's worth of clear air.
    pub high_drop: f32,
    /// How near to exactly full the moon has to be to count as full, in days either side.
    pub full_moon_days: f64,
    /// How many days either side of the colony's own birthday its birthday finds turn up.
    pub birthday_days: i64,
}

pub const FINDS: Finds = Finds {
    per_day: 1..=5,
    desktop_interval_secs: 3_600.0..9_000.0,
    at_home_in: 7,
    conditional_in: 2,
    prefer_undiscovered_in: 4,
    rare_in: 60,
    evening_hour: 20,
    morning_hour: 6,
    late_morning_hour: 10,
    high_drop: 140.0,
    full_moon_days: 1.5,
    birthday_days: 3,
};

/// How often a companion out on the desktop looks around, dangles, and comes down from the
/// windows (`world.rs`).
pub struct Roaming {
    /// Visible time between one look at something nearby and the next.
    pub inspect_interval_secs: Range<f32>,
    /// Visible time between one dangle from a ledge and the next.
    pub dangle_interval_secs: Range<f32>,
    /// The chance, each time it chooses something to do up on a ledge, that a companion who
    /// likes it anywhere comes down to do it on the floor instead.
    pub anywhere_comes_down: f64,
    /// The same for a climber, who mostly stays up.
    pub climber_comes_down: f64,
    /// How long a companion stays down on the floor before it thinks of climbing again, for each
    /// leaning: a climber is soon back up, a floor dweller hardly ever.
    pub climber_rest_secs: Range<f32>,
    pub anywhere_rest_secs: Range<f32>,
    pub homebody_rest_secs: Range<f32>,
    pub floor_dweller_rest_secs: Range<f32>,
}

pub const ROAMING: Roaming = Roaming {
    inspect_interval_secs: 120.0..240.0,
    dangle_interval_secs: 240.0..480.0,
    anywhere_comes_down: 0.35,
    climber_comes_down: 0.05,
    climber_rest_secs: 15.0..40.0,
    anywhere_rest_secs: 35.0..90.0,
    homebody_rest_secs: 60.0..150.0,
    floor_dweller_rest_secs: 90.0..200.0,
};

/// What residents get up to at home between everything else (`world/village_life.rs`).
pub struct VillageLife {
    /// The longest any one walk in a plan may take before the resident gives up on it and does
    /// its thing where it has got to.
    pub walk_limit_secs: f32,
    /// How long a find at home is held up before it goes in the scrapbook.
    pub find_secs: f32,
    /// How long a leaf takes to drift down onto a face, how long the start and the shake take,
    /// and how long it takes to reach the ground once shaken off.
    pub leaf_fall_secs: f32,
    pub leaf_beat_secs: f32,
    pub leaf_drop_secs: f32,
    /// How the choices weigh against one another: an ordinary quiet moment is still the
    /// commonest thing, the garden and the house come next, and a mishap is rare enough to be
    /// worth noticing.
    pub moment_weight: u32,
    pub garden_weight: u32,
    pub chore_weight: u32,
    pub indoors_weight: u32,
    pub roof_weight: u32,
    pub leaf_weight: u32,
    pub snack_weight: u32,
    pub cushion_weight: u32,
    /// How long a resident stays indoors: a nap, or a while pottering about in there.
    pub indoors_nap_secs: Range<f32>,
    pub indoors_potter_secs: Range<f32>,
    /// How long a resident sits up on its roof.
    pub roof_secs: Range<f32>,
}

pub const VILLAGE_LIFE: VillageLife = VillageLife {
    walk_limit_secs: 20.0,
    find_secs: 2.6,
    leaf_fall_secs: 1.1,
    leaf_beat_secs: 3.0,
    leaf_drop_secs: 0.8,
    moment_weight: 10,
    garden_weight: 6,
    chore_weight: 4,
    indoors_weight: 3,
    roof_weight: 2,
    leaf_weight: 1,
    snack_weight: 1,
    cushion_weight: 1,
    indoors_nap_secs: 40.0..110.0,
    indoors_potter_secs: 12.0..30.0,
    roof_secs: 18.0..45.0,
};

/// A guest's visit to the village (`world/visitors.rs`).
pub struct Visits {
    /// How long after the houses appear the visitor turns up. The residents walk home first; a
    /// guest arriving with them would read as one of them.
    pub arrival_delay_secs: f32,
    /// How much of the gathering is left when the visitor starts saying goodbye. Long enough for
    /// the wave and the walk back out, so nobody is ever cut off mid-farewell.
    pub departure_lead_secs: f32,
    /// The hello, and the wave goodbye.
    pub greeting_secs: f32,
    pub farewell_secs: f32,
    /// One shared moment beside the houses, and the still stretch between two of them.
    pub beat_secs: f32,
    pub still_secs: f32,
    /// A walk shorter than this is not a walk: the guest steps out from beside the last house.
    pub min_walk: f32,
    /// And no walk is longer than this, so an arrival reads the same on a laptop and on a wall of
    /// displays rather than growing with the desktop.
    pub max_walk: f32,
    /// A walk that has not finished by now was never going to; the guest simply arrives or is
    /// gone.
    pub max_walk_secs: f32,
    /// How long a resident holds its answer to the hello, before its temperament lengthens it.
    pub answer_hold_secs: f32,
    /// A resident still on its way home only notices a guest it is already this close to.
    /// Anyone resting at the village is part of the welcome, however long the strip has grown.
    pub notice_distance: f32,
    /// How long an invited friend stays.
    pub invited_stay: time::Duration,
    /// How long a guest stays at one stop on its walk around the village, and how much of that
    /// is the one small thing it came over to do. The rest of the stay is the same calm moments
    /// a visit has always had, only somewhere new each time.
    pub tour_stay_secs: f32,
    pub tour_beat_secs: f32,
    /// How much of a gathering is kept back for the walk back to the spot the guest came in at,
    /// so the goodbye is said there and the walk out is the walk in run backwards.
    pub tour_return_secs: f32,
}

pub const VISITS: Visits = Visits {
    arrival_delay_secs: 12.0,
    departure_lead_secs: 90.0,
    greeting_secs: 3.2,
    farewell_secs: 2.6,
    beat_secs: 6.5,
    still_secs: 15.0,
    min_walk: 24.0,
    max_walk: 520.0,
    max_walk_secs: 40.0,
    answer_hold_secs: 2.6,
    notice_distance: 420.0,
    invited_stay: time::Duration::hours(24),
    tour_stay_secs: 72.0,
    tour_beat_secs: 8.0,
    tour_return_secs: 45.0,
};

/// A snack or a toy held out from the creature menu (`world/offers.rs`).
pub struct Offers {
    /// A second offer inside this window is a nudge rather than a new question, and is turned
    /// down. It is what keeps a double click from feeding anyone twice.
    pub pester_secs: i64,
    /// How long a creature stays full after accepting a snack.
    pub fed_secs: i64,
    /// How long a creature has had its fill of the toy after accepting one.
    pub played_secs: i64,
    /// Sleep pressure past which nothing held out is interesting.
    pub too_sleepy: f32,
    /// Sleep pressure below which a sleeping creature has had its rest and can be woken gently.
    pub rested_enough: f32,
    /// Boldness below which a creature takes a beat before it answers, and how long that beat
    /// lasts.
    pub timid_boldness: f32,
    pub thinking_secs: f32,
    /// A cursor this close is the hand holding the offer out.
    pub cursor_reach: f32,
    /// How many kind and unkind handlings still colour how an offer is received.
    pub remembered_handling: u32,
    /// Playfulness below which a toy is simply baffling rather than unwanted.
    pub baffled_by_toys: f32,
}

pub const OFFERS: Offers = Offers {
    pester_secs: 6,
    fed_secs: 90,
    played_secs: 45,
    too_sleepy: 0.82,
    rested_enough: 0.35,
    timid_boldness: 0.38,
    thinking_secs: 0.55,
    cursor_reach: 240.0,
    remembered_handling: 50,
    baffled_by_toys: 0.35,
};

/// How much the colony's records have to show before the notebook remarks on a pattern
/// (`observations.rs`). Below these there is simply nothing to say.
pub struct Observations {
    /// Seconds on ledges before keeping to high places is worth remarking on, with at least this
    /// many climbs of its own to get there.
    pub high_places_seconds: u32,
    pub high_places_climbs: u32,
    /// Seconds riding windows.
    pub window_rider_seconds: u32,
    /// First to find at least this many things in the scrapbook.
    pub finder_firsts: u32,
    /// Seconds asleep without being disturbed.
    pub sound_sleeper_seconds: u32,
    /// How sure the colony has to be of a companion's usual spot before saying so.
    pub place_confidence: u8,
    /// Games played.
    pub playful_sessions: u32,
    /// Each sought the other out at least `seek_each` times, `seek_total` between them.
    pub seek_each: u16,
    pub seek_total: u16,
    /// Sought out this many times, at least three times as often as it went the other way.
    pub follow_times: u16,
    /// Naps side by side, games together, gifts, and squabbles between one pair.
    pub naps: u16,
    pub games: u16,
    pub gifts: u16,
    pub squabbles: u16,
    /// A visitor seen this many times is a regular.
    pub regular_visits: usize,
}

pub const OBSERVATIONS: Observations = Observations {
    high_places_seconds: 60 * 60,
    high_places_climbs: 10,
    window_rider_seconds: 30 * 60,
    finder_firsts: 5,
    sound_sleeper_seconds: 2 * 60 * 60,
    place_confidence: 24,
    playful_sessions: 20,
    seek_each: 5,
    seek_total: 16,
    follow_times: 12,
    naps: 10,
    games: 30,
    gifts: 4,
    squabbles: 8,
    regular_visits: 3,
};

/// Taking back changes made from the settings window (`world/undo.rs`).
pub struct Undo {
    /// How many changes to who lives here and how the village is laid out can be taken back,
    /// newest first. They are kept only while the app runs, never in the colony file.
    pub depth: usize,
}

pub const UNDO: Undo = Undo { depth: 8 };
