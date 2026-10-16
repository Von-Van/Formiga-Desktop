//! What the colony keeps of Formiga Battle.
//!
//! Battle keeps nothing of its own: Desktop keeps the colony's Battle record here and hands it to
//! Battle at the start of every session, and Battle answers with what each match earned, which
//! Desktop applies once per match. The record is written in exactly the shape the Battle
//! contract's `BattleRecord` has, so it crosses to Battle and back unchanged; what it means is
//! Battle's, and Desktop only keeps it inside its bounds. The keepsakes a match earns are
//! Desktop's own: it keeps the ones it knows, once each, for Formiga Home to show, and says what
//! they are in its own words.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use time::OffsetDateTime;

/// How much of Battle's record the colony keeps: the same bounds the Battle contract holds it to.
pub mod limits {
    pub const MAX_HISTORY: usize = 20;
    pub const MAX_PAGES: usize = 24;
    pub const MAX_SHOWINGS: usize = 10;
    pub const MAX_MOMENTS: usize = 8;
    pub const MAX_TEXT_CHARS: usize = 120;
    pub const MAX_ID_CHARS: usize = 48;
    pub const MAX_SIGNS: usize = 32;
    pub const MAX_APPLIED: usize = 256;
    /// Cups and unlocks.
    pub const MAX_PROGRESS: usize = 64;
    pub const MAX_TEAM: usize = 3;
    pub const MAX_HANDLER_CHARS: usize = 24;
    pub const MAX_CLUB_CHARS: usize = 32;
    pub const MAX_TRIO_CHARS: usize = 32;
    /// Trios kept, retired ones included.
    pub const MAX_TRIOS: usize = 12;
    pub const MAX_LINEUPS: usize = 8;
    pub const MAX_TRIO_CUPS: usize = 8;
    pub const MAX_STYLE_CHARS: usize = 24;
}
use limits::*;

/// Who a match was against.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleMode {
    Ai,
    Friend,
}

/// What a match showed of one companion, by Battle's own names for what it saw.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleShowing {
    pub steps: u16,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub signs: BTreeMap<String, u16>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub friends: BTreeMap<u64, u16>,
}

/// One companion's page of Battle's notebook.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleNotebookPage {
    pub creature_id: u64,
    pub matches: u32,
    pub wins: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent: Vec<BattleShowing>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moments: Vec<String>,
}

/// One line of the match history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleMatchSummary {
    #[serde(with = "time::serde::rfc3339")]
    pub finished_at_utc: OffsetDateTime,
    pub mode: BattleMode,
    pub won: Option<bool>,
    pub ended_by: String,
    pub turns: u8,
    pub team: Vec<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opponent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opponent_trio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opponent_pennant: Option<BattlePennant>,
}

/// Wins toward one of Battle's cups.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleCupWins {
    pub cup: String,
    pub wins: u8,
}

/// A Handler's pennant, as four of Battle's own names for its parts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattlePennant {
    pub shape: String,
    pub field: String,
    pub symbol: String,
    pub trim: String,
}

/// Who the player is in Battle: a name, an optional club and a pennant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleHandlerCard {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub club: Option<String>,
    pub pennant: BattlePennant,
}

/// Matches played, won and drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleTally {
    pub played: u32,
    pub won: u32,
    pub drawn: u32,
}

impl BattleTally {
    pub fn is_empty(&self) -> bool {
        self.played == 0
    }

    fn holds(&self) -> bool {
        u64::from(self.won) + u64::from(self.drawn) <= u64::from(self.played)
    }
}

/// The three a trio had before its members last changed, and when that ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleLineup {
    pub members: Vec<u64>,
    #[serde(with = "time::serde::rfc3339")]
    pub until_utc: OffsetDateTime,
}

/// Three companions the Handler named as a team, and what they have done as one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleTrio {
    pub id: String,
    pub name: String,
    pub members: Vec<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pennant: Option<BattlePennant>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retired: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub formed_at_utc: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub members_since_utc: OffsetDateTime,
    #[serde(default, skip_serializing_if = "BattleTally::is_empty")]
    pub friends: BattleTally,
    #[serde(default, skip_serializing_if = "BattleTally::is_empty")]
    pub visitors: BattleTally,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cups: Vec<BattleCupWins>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lineups: Vec<BattleLineup>,
}

/// The colony's Battle record, as the Battle contract writes it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleRecord {
    pub circuit_points: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unlocked: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cups: Vec<BattleCupWins>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<BattleMatchSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notebook: Vec<BattleNotebookPage>,
    /// The matches already applied, the newest last, so none is applied twice.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applied: Vec<String>,
    /// Who the player is in Battle, once they have said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handler: Option<BattleHandlerCard>,
    /// The trios the Handler has named, retired ones included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trios: Vec<BattleTrio>,
}

impl BattleRecord {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Whether the record is inside every bound the Battle contract holds it to.
    pub fn holds(&self) -> bool {
        self.applied.len() <= MAX_APPLIED
            && self.applied.iter().all(|id| is_match_id(id))
            && self.unlocked.len() <= MAX_PROGRESS
            && self.unlocked.iter().all(|id| is_battle_id(id))
            && self.cups.len() <= MAX_PROGRESS
            && self.cups.iter().all(|cup| is_battle_id(&cup.cup))
            && self.history.len() <= MAX_HISTORY
            && self.history.iter().all(BattleMatchSummary::holds)
            && self.notebook.len() <= MAX_PAGES
            && self.notebook.iter().all(BattleNotebookPage::holds)
            && self.handler.as_ref().is_none_or(BattleHandlerCard::holds)
            && self.trios.len() <= MAX_TRIOS
            && self.trios.iter().all(BattleTrio::holds)
            && self
                .trios
                .iter()
                .enumerate()
                .all(|(i, trio)| self.trios[..i].iter().all(|t| t.id != trio.id))
    }

    /// Bring the record inside its bounds, keeping the newest of whatever there is too much of,
    /// and leaving out anything that cannot be written as Battle writes it.
    pub fn normalize(&mut self) {
        keep_newest(&mut self.applied, MAX_APPLIED);
        self.applied.retain(|id| is_match_id(id));
        self.unlocked.retain(|id| is_battle_id(id));
        keep_newest(&mut self.unlocked, MAX_PROGRESS);
        self.cups.retain(|cup| is_battle_id(&cup.cup));
        keep_newest(&mut self.cups, MAX_PROGRESS);
        self.history.retain(BattleMatchSummary::holds);
        keep_newest(&mut self.history, MAX_HISTORY);
        keep_newest(&mut self.notebook, MAX_PAGES);
        for page in &mut self.notebook {
            page.wins = page.wins.min(page.matches);
            page.recent.retain(BattleShowing::holds);
            keep_newest(&mut page.recent, MAX_SHOWINGS);
            page.moments.retain(|moment| is_battle_line(moment));
            keep_newest(&mut page.moments, MAX_MOMENTS);
        }
        if self.handler.as_ref().is_some_and(|card| !card.holds()) {
            self.handler = None;
        }
        let mut seen = Vec::new();
        self.trios.retain(|trio| {
            let first = !seen.contains(&trio.id);
            seen.push(trio.id.clone());
            first
        });
        for trio in &mut self.trios {
            keep_newest(&mut trio.cups, MAX_TRIO_CUPS);
            keep_newest(&mut trio.lineups, MAX_LINEUPS);
            trio.lineups.retain(|lineup| is_lineup(&lineup.members));
            for tally in [&mut trio.friends, &mut trio.visitors] {
                tally.won = tally.won.min(tally.played);
                tally.drawn = tally.drawn.min(tally.played - tally.won);
            }
        }
        self.trios.retain(BattleTrio::holds);
        keep_newest(&mut self.trios, MAX_TRIOS);
    }
}

impl BattleShowing {
    fn holds(&self) -> bool {
        self.signs.len() <= MAX_SIGNS
            && self.signs.keys().all(|sign| is_battle_id(sign))
            && self.friends.len() <= MAX_SIGNS
    }
}

impl BattleNotebookPage {
    fn holds(&self) -> bool {
        self.wins <= self.matches
            && self.recent.len() <= MAX_SHOWINGS
            && self.recent.iter().all(BattleShowing::holds)
            && self.moments.len() <= MAX_MOMENTS
            && self.moments.iter().all(|moment| is_battle_line(moment))
    }
}

impl BattleMatchSummary {
    fn holds(&self) -> bool {
        is_battle_id(&self.ended_by)
            && self.team.len() <= MAX_TEAM
            && self.opponent.as_deref().is_none_or(is_battle_line)
            && self.trio.as_deref().is_none_or(is_trio_id)
            && self
                .opponent_trio
                .as_deref()
                .is_none_or(|name| is_battle_name(name, MAX_TRIO_CHARS))
            && self
                .opponent_pennant
                .as_ref()
                .is_none_or(BattlePennant::holds)
    }
}

impl BattlePennant {
    fn holds(&self) -> bool {
        [&self.shape, &self.field, &self.symbol, &self.trim]
            .iter()
            .all(|part| {
                !part.is_empty()
                    && part.len() <= MAX_STYLE_CHARS
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b))
            })
    }
}

impl BattleHandlerCard {
    fn holds(&self) -> bool {
        is_battle_name(&self.name, MAX_HANDLER_CHARS)
            && self
                .club
                .as_deref()
                .is_none_or(|club| is_battle_name(club, MAX_CLUB_CHARS))
            && self.pennant.holds()
    }
}

impl BattleTrio {
    fn holds(&self) -> bool {
        is_trio_id(&self.id)
            && is_battle_name(&self.name, MAX_TRIO_CHARS)
            && is_lineup(&self.members)
            && self.pennant.as_ref().is_none_or(BattlePennant::holds)
            && self.friends.holds()
            && self.visitors.holds()
            && self.cups.len() <= MAX_TRIO_CUPS
            && self.cups.iter().all(|cup| is_battle_id(&cup.cup))
            && self.lineups.len() <= MAX_LINEUPS
            && self.lineups.iter().all(|lineup| is_lineup(&lineup.members))
    }
}

/// A trio's id: 16 lowercase hex digits.
fn is_trio_id(id: &str) -> bool {
    id.len() == 16
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Three different companions, smallest id first.
fn is_lineup(members: &[u64]) -> bool {
    members.len() == 3 && members.windows(2).all(|w| w[0] < w[1])
}

/// A name Battle lets cross: a plain line of at most `max` characters.
fn is_battle_name(text: &str, max: usize) -> bool {
    is_battle_line(text) && text.chars().count() <= max
}

fn keep_newest<T>(items: &mut Vec<T>, max: usize) {
    if items.len() > max {
        items.drain(..items.len() - max);
    }
}

fn is_match_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// One of Battle's identifiers: `cup.backyard`, `took_the_edge`.
pub fn is_battle_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_CHARS
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

/// A line of Battle's text Desktop keeps: what the Battle contract lets cross, which is text that
/// is already plain. Nothing that changes how text reads without being seen (control characters,
/// bidirectional overrides and isolates, invisible separators, the byte-order mark), no run of
/// whitespace but a single space, no space at either end, and at most 120 characters, a space
/// counting as one.
pub fn is_battle_line(text: &str) -> bool {
    let invisible = |c: char| {
        matches!(
            c,
            '\u{200B}'
                | '\u{200E}'
                | '\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{206A}'..='\u{206F}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
        )
    };
    let mut previous_space = true;
    for c in text.chars() {
        if c.is_whitespace() {
            if c != ' ' || previous_space {
                return false;
            }
            previous_space = true;
        } else if c.is_control() || invisible(c) {
            return false;
        } else {
            previous_space = false;
        }
    }
    !text.is_empty() && !previous_space && text.chars().count() <= MAX_TEXT_CHARS
}

/// One of Battle's keepsakes that Desktop keeps: a trophy and a medal for each of Battle's four
/// cups (`rewards::CUPS` in Formiga Battle), by Battle's own identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BattleKeepsake {
    #[serde(rename = "battle.trophy.backyard_cup")]
    BackyardCupTrophy,
    #[serde(rename = "battle.medal.backyard_cup")]
    BackyardCupMedal,
    #[serde(rename = "battle.trophy.woodland_cup")]
    WoodlandCupTrophy,
    #[serde(rename = "battle.medal.woodland_cup")]
    WoodlandCupMedal,
    #[serde(rename = "battle.trophy.millpond_cup")]
    MillpondCupTrophy,
    #[serde(rename = "battle.medal.millpond_cup")]
    MillpondCupMedal,
    #[serde(rename = "battle.trophy.cove_cup")]
    CoveCupTrophy,
    #[serde(rename = "battle.medal.cove_cup")]
    CoveCupMedal,
}

impl BattleKeepsake {
    /// Each cup's trophy and medal, in the order Battle's cups open.
    pub const ALL: [Self; 8] = [
        Self::BackyardCupTrophy,
        Self::BackyardCupMedal,
        Self::WoodlandCupTrophy,
        Self::WoodlandCupMedal,
        Self::MillpondCupTrophy,
        Self::MillpondCupMedal,
        Self::CoveCupTrophy,
        Self::CoveCupMedal,
    ];

    /// Battle's identifier for it: what a match's rewards name, and how a save writes it.
    pub const fn id(self) -> &'static str {
        match self {
            Self::BackyardCupTrophy => "battle.trophy.backyard_cup",
            Self::BackyardCupMedal => "battle.medal.backyard_cup",
            Self::WoodlandCupTrophy => "battle.trophy.woodland_cup",
            Self::WoodlandCupMedal => "battle.medal.woodland_cup",
            Self::MillpondCupTrophy => "battle.trophy.millpond_cup",
            Self::MillpondCupMedal => "battle.medal.millpond_cup",
            Self::CoveCupTrophy => "battle.trophy.cove_cup",
            Self::CoveCupMedal => "battle.medal.cove_cup",
        }
    }

    /// The keepsake Battle calls `id`, if it is one Desktop keeps.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|keepsake| keepsake.id() == id)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::BackyardCupTrophy => "Backyard Cup trophy",
            Self::BackyardCupMedal => "Backyard Cup medal",
            Self::WoodlandCupTrophy => "Woodland Cup trophy",
            Self::WoodlandCupMedal => "Woodland Cup medal",
            Self::MillpondCupTrophy => "Millpond Cup trophy",
            Self::MillpondCupMedal => "Millpond Cup medal",
            Self::CoveCupTrophy => "Sandy Cove Cup trophy",
            Self::CoveCupMedal => "Sandy Cove Cup medal",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::BackyardCupTrophy => {
                "A little cup on a wooden plinth, for winning the Backyard Cup."
            }
            Self::BackyardCupMedal => "A medal on a green ribbon, from the Backyard Cup.",
            Self::WoodlandCupTrophy => {
                "A little cup on a mossy stump, for winning the Woodland Cup."
            }
            Self::WoodlandCupMedal => "A medal on a russet ribbon, from the Woodland Cup.",
            Self::MillpondCupTrophy => "A little cup on a millstone, for winning the Millpond Cup.",
            Self::MillpondCupMedal => "A medal on a blue ribbon, from the Millpond Cup.",
            Self::CoveCupTrophy => {
                "A little cup on a piece of driftwood, for winning the Sandy Cove Cup."
            }
            Self::CoveCupMedal => "A medal on a sandy yellow ribbon, from the Sandy Cove Cup.",
        }
    }
}

/// A keepsake the colony earned in Formiga Battle, and when.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleKeepsakeRecord {
    pub keepsake: BattleKeepsake,
    #[serde(with = "time::serde::rfc3339")]
    pub earned_at_utc: OffsetDateTime,
}

/// Everything the colony keeps of Formiga Battle. Absent from the file until the colony has played.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleLog {
    #[serde(default, skip_serializing_if = "BattleRecord::is_empty")]
    pub record: BattleRecord,
    /// Each keepsake once, in the order it was earned.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keepsakes: Vec<BattleKeepsakeRecord>,
}

impl BattleLog {
    pub fn is_empty(&self) -> bool {
        self.record.is_empty() && self.keepsakes.is_empty()
    }

    pub fn has_keepsake(&self, keepsake: BattleKeepsake) -> bool {
        self.keepsakes
            .iter()
            .any(|record| record.keepsake == keepsake)
    }

    /// Whether everything here is inside its bounds and each keepsake is kept once.
    pub fn holds(&self) -> bool {
        self.record.holds()
            && self.keepsakes.iter().enumerate().all(|(index, record)| {
                self.keepsakes[..index]
                    .iter()
                    .all(|earlier| earlier.keepsake != record.keepsake)
            })
    }

    /// Bring everything inside its bounds; a keepsake kept twice is the one earned first.
    pub fn normalize(&mut self) {
        self.record.normalize();
        let mut kept = Vec::new();
        self.keepsakes.retain(|record| {
            let first = !kept.contains(&record.keepsake);
            kept.push(record.keepsake);
            first
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn a_save_writes_each_keepsake_by_battles_own_identifier() {
        for keepsake in BattleKeepsake::ALL {
            assert_eq!(serde_json::to_value(keepsake).unwrap(), keepsake.id());
            assert_eq!(BattleKeepsake::from_id(keepsake.id()), Some(keepsake));
            assert!(is_battle_id(keepsake.id()));
            for text in [keepsake.name(), keepsake.description()] {
                assert!(is_battle_line(text) && text.len() <= 80, "{text}");
            }
        }
        assert_eq!(BattleKeepsake::from_id("battle.trophy.grand_cup"), None);
        assert_eq!(BattleKeepsake::from_id("battle.ribbon.someday"), None);
    }

    #[test]
    fn a_line_of_battles_text_is_one_plain_line() {
        assert!(is_battle_line(
            "Mochi, Biscuit and Fig won a Backyard Cup match."
        ));
        for bad in [
            "",
            " padded",
            "padded ",
            "two  spaces",
            "two\nlines",
            "a\u{202E}reversed",
            &"x".repeat(121),
        ] {
            assert!(!is_battle_line(bad), "{bad:?}");
        }
    }

    #[test]
    fn a_record_out_of_bounds_keeps_its_newest_and_what_can_be_written() {
        let summary = BattleMatchSummary {
            finished_at_utc: datetime!(2026-10-15 12:00 UTC),
            mode: BattleMode::Friend,
            won: Some(true),
            ended_by: "capture".to_owned(),
            turns: 8,
            team: vec![1, 2, 3],
            opponent: Some("Sam".to_owned()),
            trio: None,
            opponent_trio: None,
            opponent_pennant: None,
        };
        let mut log = BattleLog {
            record: BattleRecord {
                history: vec![summary; MAX_HISTORY + 5],
                applied: vec!["not a match".to_owned(), "0".repeat(32)],
                notebook: vec![BattleNotebookPage {
                    creature_id: 1,
                    matches: 1,
                    wins: 4,
                    moments: vec!["fine".to_owned(), "two\nlines".to_owned()],
                    ..BattleNotebookPage::default()
                }],
                ..BattleRecord::default()
            },
            keepsakes: vec![
                BattleKeepsakeRecord {
                    keepsake: BattleKeepsake::BackyardCupMedal,
                    earned_at_utc: datetime!(2026-10-15 12:00 UTC),
                };
                2
            ],
        };
        assert!(!log.holds());
        log.normalize();
        assert!(log.holds());
        assert_eq!(log.record.history.len(), MAX_HISTORY);
        assert_eq!(log.record.applied, vec!["0".repeat(32)]);
        assert_eq!(log.record.notebook[0].wins, 1);
        assert_eq!(log.record.notebook[0].moments, vec!["fine".to_owned()]);
        assert_eq!(log.keepsakes.len(), 1);
    }

    #[test]
    fn a_handler_or_trio_out_of_bounds_is_left_out_and_the_rest_kept() {
        let at = datetime!(2026-10-16 12:00 UTC);
        let pennant = BattlePennant {
            shape: "pennon".to_owned(),
            field: "forest".to_owned(),
            symbol: "acorn".to_owned(),
            trim: "plain".to_owned(),
        };
        let trio = |id: &str, name: &str, members: Vec<u64>| BattleTrio {
            id: id.to_owned(),
            name: name.to_owned(),
            members,
            pennant: None,
            retired: false,
            formed_at_utc: at,
            members_since_utc: at,
            friends: BattleTally {
                played: 2,
                won: 5,
                drawn: 0,
            },
            visitors: BattleTally::default(),
            cups: Vec::new(),
            lineups: Vec::new(),
        };
        let mut record = BattleRecord {
            handler: Some(BattleHandlerCard {
                name: "x".repeat(MAX_HANDLER_CHARS + 1),
                club: None,
                pennant,
            }),
            trios: vec![
                trio("7a1e5c3b9d2f4068", "Thistle", vec![1, 2, 3]),
                trio("7a1e5c3b9d2f4068", "Thistle again", vec![4, 5, 6]),
                trio("0000000000000001", "Twice", vec![1, 1, 2]),
                trio("not-a-trio-id!!!", "Odd", vec![1, 2, 3]),
            ],
            ..BattleRecord::default()
        };
        assert!(!record.holds());
        record.normalize();
        assert!(record.holds());
        assert!(record.handler.is_none());
        assert_eq!(record.trios.len(), 1);
        assert_eq!(record.trios[0].name, "Thistle");
        assert_eq!(record.trios[0].friends.won, 2);
    }
}
