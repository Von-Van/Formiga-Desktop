//! Wonders: something to play on that turns up for a little while and is gone again.
//!
//! A wonder appears somewhere a companion can get to — on the floor, up on a window, or along the
//! village's ground while the houses are out — and whoever it turned up for goes straight over to
//! it: a little chair to sit in, a leaf to sled on, a fountain to splash in, a tightrope to cross. Once
//! they have had their go it is gone, and if one of them is picked up on the way, or in the middle
//! of it, it is gone sooner.
//!
//! Nothing about a wonder is kept but the notebook's page of them: the first time each kind turned
//! up, who found it, and how many goes the colony has had on it since. There is no inventory and
//! nothing to place. The code calls them props where it means the thing on the screen; the person
//! at the desk only ever reads "wonders".

use crate::CreatureId;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// One kind of wonder. The variant is what a save keeps, so a kind always means what it meant
/// when it was first found: new ones are only ever added at the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WonderKind {
    /// A little wooden chair to climb into, sit back in and swing its feet.
    Chair,
    /// A curled-up leaf to sit in and scoot along on, and back. It took the place of a bike in
    /// 0.66.1, and a save still calls it by the bike's name, so a colony that found the bike has
    /// found the sled, and either build reads the other's file.
    #[serde(rename = "Bike")]
    LeafSled,
    /// A stone fountain with a spout, to splash about in.
    Fountain,
    /// A rope strung between two posts, to walk across with its arms out.
    Tightrope,
    /// A tree stump for a table with a stool either side, to sit at and play cards.
    StumpTable,
    /// A big signpost with an arrow, to look up at, give a spin and point along.
    ArrowSign,
    /// A hammock slung between two posts, for a swaying doze.
    Hammock,
    /// A tall stack of books to climb up, sit on top of and read.
    BookStack,
    /// A plank on a log, for two to go up and down on.
    Seesaw,
}

/// How many can play on a wonder at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WonderSeats {
    /// Always one.
    One,
    /// One, or two when a friend is free to come along.
    OneOrTwo,
    /// Never without a friend.
    Two,
}

impl WonderKind {
    pub const ALL: [Self; 9] = [
        Self::Chair,
        Self::LeafSled,
        Self::Fountain,
        Self::Tightrope,
        Self::StumpTable,
        Self::ArrowSign,
        Self::Hammock,
        Self::BookStack,
        Self::Seesaw,
    ];

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Chair => "Little chair",
            Self::LeafSled => "Leaf sled",
            Self::Fountain => "Fountain",
            Self::Tightrope => "Tightrope",
            Self::StumpTable => "Stump table",
            Self::ArrowSign => "Arrow sign",
            Self::Hammock => "Hammock",
            Self::BookStack => "Stack of books",
            Self::Seesaw => "Seesaw",
        }
    }

    /// What the notebook says about one the colony has found.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Chair => "Just the right size. Best sat in with both feet swinging.",
            Self::LeafSled => "Sat in, curled up at the front, and scooted along and back by paw.",
            Self::Fountain => "A spout of water and a stone rim, for splashing.",
            Self::Tightrope => "Crossed slowly, arms out, with a wobble in the middle.",
            Self::StumpTable => "A stump for a table and a stool each side. Cards are dealt.",
            Self::ArrowSign => "Spun round, and then pointed along wherever it stops.",
            Self::Hammock => "Slung between two posts, for a slow, swaying doze.",
            Self::BookStack => "Climbed up and sat on top of, for a little read.",
            Self::Seesaw => "Up, down, up, down. It takes two.",
        }
    }

    /// What an empty place in the notebook says before this one has turned up. It describes the
    /// wonder's own world, and never asks the reader to go and do anything.
    pub const fn hint(self) -> &'static str {
        match self {
            Self::Chair => "Something with four legs, just the right height.",
            Self::LeafSled => "Something green to go for a ride in.",
            Self::Fountain => "Somewhere wet that is not a puddle.",
            Self::Tightrope => "A long way across, and not much to stand on.",
            Self::StumpTable => "A table that grew where it stands.",
            Self::ArrowSign => "It says go that way.",
            Self::Hammock => "Somewhere to sway.",
            Self::BookStack => "Something tall to read.",
            Self::Seesaw => "Only works with a friend.",
        }
    }

    pub const fn seats(self) -> WonderSeats {
        match self {
            Self::Chair | Self::LeafSled | Self::Hammock => WonderSeats::One,
            Self::Fountain
            | Self::Tightrope
            | Self::StumpTable
            | Self::ArrowSign
            | Self::BookStack => WonderSeats::OneOrTwo,
            Self::Seesaw => WonderSeats::Two,
        }
    }

    /// How much ground it stands on, in art pixels (a companion's frame is 48 across). Where a
    /// wonder can go is measured with this and the room its players stand in beside it.
    pub const fn span(self) -> f32 {
        match self {
            Self::Chair => 22.0,
            Self::LeafSled => 30.0,
            Self::Fountain => 40.0,
            Self::Tightrope => 96.0,
            Self::StumpTable => 76.0,
            Self::ArrowSign => 28.0,
            Self::Hammock => 80.0,
            Self::BookStack => 22.0,
            Self::Seesaw => 84.0,
        }
    }

    /// The whole run of ground a turn on it covers, players and all, in art pixels: the span,
    /// a frame of room either side for whoever stands beside it, and the ride for a sled.
    pub const fn room(self) -> f32 {
        match self {
            Self::LeafSled => 180.0,
            Self::Tightrope => 150.0,
            Self::Fountain | Self::ArrowSign | Self::BookStack => self.span() + 96.0,
            _ => self.span() + 48.0,
        }
    }
}

/// The first time one kind of wonder turned up, who it turned up for, and how many goes the
/// colony has had on it since. One record per kind, so the notebook's page can never grow past
/// the catalogue.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WonderRecord {
    pub kind: WonderKind,
    #[serde(with = "time::serde::rfc3339")]
    pub first_at: OffsetDateTime,
    pub finder: Option<CreatureId>,
    /// Kept so a departed finder still has a name, without keeping a copy of the creature.
    pub finder_name: String,
    /// Every go anybody has had on it, counting the first.
    #[serde(default)]
    pub goes: u32,
}

/// A wonder on the screen just now, for the overlay to draw. Runtime only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WonderView {
    pub kind: WonderKind,
    /// The middle of where it stands on the ground, in desktop points.
    pub at: crate::Point,
    pub monitor_id: crate::MonitorId,
    /// The window it stands on, if it is up on one, so it is hidden with whatever covers it.
    pub window_key: Option<crate::WindowKey>,
    /// Drawn turned the other way round.
    pub mirrored: bool,
    /// How far it has appeared, from 0 to 1, and back down as it goes.
    pub presence: f32,
    /// Seconds since it appeared, for its own little loops: a spout, a turning wheel.
    pub elapsed: f32,
    /// What its moving part is doing, from -1 to 1, as if it were not turned round: a seesaw's
    /// tilt, a hammock's sway, the arrow's spin, a fountain's splash, the rope's wobble, the
    /// cards down on the table, a sled sliding along. Zero for one standing still.
    pub motion: f32,
    /// Just found for the first time, and glinting for it.
    pub glint: bool,
}
