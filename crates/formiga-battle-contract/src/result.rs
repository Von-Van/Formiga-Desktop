//! What Battle hands back, and what Desktop keeps of it (version 2).
//!
//! Desktop owns everything Battle earns: it keeps a companion's Battle record in the colony and
//! hands it to Battle in each session's snapshot ([`BattleRecord`]), and Battle answers with
//! [`BattleResult`]: every match the session played, each with its rewards, which Desktop applies
//! once each, by match id, when Battle closes. Battle keeps nothing of its own.
//!
//! What a match earns depends on who it was against:
//!
//! | Against | Earns |
//! |---|---|
//! | A friend | Open Circuit points, a line in the match history, notebook entries for the three who played, and a match played for each of them |
//! | The AI | Championship progress (cup wins, unlocks), journal entries, and keepsakes for Home to show |

use crate::BattleError;
use formiga_expansion_rulebook::{self as rulebook, Kind, SessionId, is_sanitized};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use time::OffsetDateTime;

pub const RESULT_FORMAT: &str = "formiga.battle.result";
pub const RESULT_FILE: &str = "result.json";

pub mod result_limits {
    pub const MAX_RESULT_BYTES: u64 = 512 * 1024;
    pub const MAX_MATCHES: usize = 64;
    pub const MAX_HISTORY: usize = 20;
    pub const MAX_PAGES: usize = 24;
    /// The matches a notebook page keeps what it saw of.
    pub const MAX_SHOWINGS: usize = 10;
    pub const MAX_MOMENTS: usize = 8;
    pub const MAX_LINES: usize = 8;
    pub const MAX_TEXT_CHARS: usize = 120;
    pub const MAX_ID_CHARS: usize = 48;
    pub const MAX_SIGNS: usize = 32;
    /// Match ids remembered as applied.
    pub const MAX_APPLIED: usize = 256;
}
use result_limits::*;

/// Who a match was against.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Ai,
    Friend,
}

/// What a match showed of one companion: how often each kind of choice was seen, by Battle's
/// own names for them, and which teammates it kept near.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Showing {
    pub steps: u16,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub signs: BTreeMap<String, u16>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub friends: BTreeMap<u64, u16>,
}

/// One companion's page of the Handler's Notebook, as Desktop keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotebookPage {
    pub creature_id: u64,
    /// Friend matches played.
    pub matches: u32,
    pub wins: u32,
    /// What its last few friend matches showed, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent: Vec<Showing>,
    /// Moments worth remembering, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moments: Vec<String>,
}

/// One line of the match history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchSummary {
    #[serde(with = "time::serde::rfc3339")]
    pub finished_at_utc: OffsetDateTime,
    pub mode: Mode,
    /// `None` for a draw.
    pub won: Option<bool>,
    /// How it ended: `capture`, `knockout`, `turn_cap` or `forfeit`.
    pub ended_by: String,
    pub turns: u8,
    /// The three who played, by creature id.
    pub team: Vec<u64>,
    /// The other Handler's name, as they typed it, for a friend match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opponent: Option<String>,
}

/// Wins toward a cup of the Championship Circuit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CupWins {
    pub cup: String,
    pub wins: u8,
}

/// Everything Desktop keeps of Battle, handed to Battle in each snapshot (version 2).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleRecord {
    pub circuit_points: u32,
    /// Championship unlocks, by id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unlocked: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cups: Vec<CupWins>,
    /// The newest last, at most 20.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<MatchSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notebook: Vec<NotebookPage>,
    /// The ids of the matches already applied, the newest last, so none is applied twice.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applied: Vec<String>,
}

/// What a match earned. A friend match earns only the first four; an AI match only the last
/// three.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rewards {
    #[serde(default)]
    pub circuit_points: u32,
    /// The companions credited with a friend match played.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participation: Vec<u64>,
    /// What the match showed of each, for the notebook, with any moments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notebook: Vec<NotebookAddition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<MatchSummary>,
    /// A cup win, by cup id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cup_win: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unlocks: Vec<String>,
    /// Lines for Desktop's journal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub journal: Vec<String>,
    /// Keepsakes for Desktop to keep and Home to show, by id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keepsakes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotebookAddition {
    pub creature_id: u64,
    pub won: bool,
    pub showing: Showing,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moments: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchResult {
    /// 32 lowercase hex digits, unique to the match: Desktop applies each id once.
    pub id: String,
    pub mode: Mode,
    pub rewards: Rewards,
}

/// Battle's answer at the end of a session: every match it played, oldest first. Written whole
/// after each match, so a crash loses nothing already won.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleResult {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    /// The SHA-256 of the snapshot's bytes, so a result is never applied to another session.
    pub snapshot_sha256: String,
    pub battle_version: String,
    pub matches: Vec<MatchResult>,
}

fn result_kind() -> Kind {
    Kind {
        format: RESULT_FORMAT,
        max_bytes: MAX_RESULT_BYTES,
        reads: crate::BATTLE_FORMAT_VERSION,
    }
}

fn is_id(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_ID_CHARS
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

fn plain(text: &str) -> bool {
    !text.is_empty() && is_sanitized(text, MAX_TEXT_CHARS)
}

impl Showing {
    fn validate(&self) -> bool {
        self.signs.len() <= MAX_SIGNS
            && self.signs.keys().all(|k| is_id(k))
            && self.friends.len() <= MAX_SIGNS
    }
}

impl BattleRecord {
    /// Applies one match's rewards to the record, once: a match already applied changes nothing
    /// and gives `false`. Journal lines and keepsakes are Desktop's own to keep; this keeps the
    /// rest.
    pub fn apply(&mut self, m: &MatchResult) -> bool {
        if self.applied.contains(&m.id) {
            return false;
        }
        self.applied.push(m.id.clone());
        if self.applied.len() > MAX_APPLIED {
            self.applied.remove(0);
        }
        let r = &m.rewards;
        self.circuit_points = self.circuit_points.saturating_add(r.circuit_points);
        if let Some(summary) = &r.history {
            self.history.push(summary.clone());
            if self.history.len() > MAX_HISTORY {
                self.history.remove(0);
            }
        }
        for addition in &r.notebook {
            let page = match self
                .notebook
                .iter()
                .position(|p| p.creature_id == addition.creature_id)
            {
                Some(i) => &mut self.notebook[i],
                None => {
                    if self.notebook.len() >= MAX_PAGES {
                        self.notebook.remove(0);
                    }
                    self.notebook.push(NotebookPage {
                        creature_id: addition.creature_id,
                        ..NotebookPage::default()
                    });
                    self.notebook.last_mut().expect("just pushed")
                }
            };
            page.matches += 1;
            page.wins += u32::from(addition.won);
            page.recent.push(addition.showing.clone());
            if page.recent.len() > MAX_SHOWINGS {
                page.recent.remove(0);
            }
            for moment in &addition.moments {
                page.moments.retain(|m| m != moment);
                page.moments.push(moment.clone());
            }
            while page.moments.len() > MAX_MOMENTS {
                page.moments.remove(0);
            }
        }
        if let Some(cup) = &r.cup_win {
            match self.cups.iter_mut().find(|c| &c.cup == cup) {
                Some(c) => c.wins = c.wins.saturating_add(1),
                None => self.cups.push(CupWins {
                    cup: cup.clone(),
                    wins: 1,
                }),
            }
        }
        for unlock in &r.unlocks {
            if !self.unlocked.contains(unlock) {
                self.unlocked.push(unlock.clone());
            }
        }
        // Kept within the record's bounds as the colony keeps them: the newest of each.
        let over = self.cups.len().saturating_sub(MAX_MATCHES);
        self.cups.drain(..over);
        let over = self.unlocked.len().saturating_sub(MAX_MATCHES);
        self.unlocked.drain(..over);
        true
    }

    /// Wins toward a cup so far.
    pub fn cup_wins(&self, cup: &str) -> u8 {
        self.cups
            .iter()
            .find(|c| c.cup == cup)
            .map_or(0, |c| c.wins)
    }

    pub fn validate(&self) -> Result<(), BattleError> {
        let ok = self.applied.len() <= MAX_APPLIED
            && self.unlocked.iter().all(|u| is_id(u))
            && self.unlocked.len() <= MAX_MATCHES
            && self.cups.len() <= MAX_MATCHES
            && self.cups.iter().all(|c| is_id(&c.cup))
            && self.history.len() <= MAX_HISTORY
            && self.history.iter().all(MatchSummary::valid)
            && self.notebook.len() <= MAX_PAGES
            && self.notebook.iter().all(|p| {
                p.recent.len() <= MAX_SHOWINGS
                    && p.recent.iter().all(Showing::validate)
                    && p.moments.len() <= MAX_MOMENTS
                    && p.moments.iter().all(|m| plain(m))
                    && p.wins <= p.matches
            });
        if ok {
            Ok(())
        } else {
            Err(BattleError::Invalid(
                "the Battle record is out of bounds".to_owned(),
            ))
        }
    }
}

impl MatchSummary {
    fn valid(&self) -> bool {
        is_id(&self.ended_by)
            && self.team.len() <= 3
            && self.opponent.as_ref().is_none_or(|o| plain(o))
    }
}

impl BattleResult {
    pub fn new(session_id: SessionId, snapshot_sha256: String, battle_version: &str) -> Self {
        Self {
            format: RESULT_FORMAT.to_owned(),
            version: crate::BATTLE_FORMAT_VERSION,
            min_reader_version: 2,
            session_id,
            snapshot_sha256,
            battle_version: rulebook::sanitize_text(battle_version, 32),
            matches: Vec::new(),
        }
    }

    /// Every bound, and the rule that a friend match earns only friend rewards and an AI match
    /// only AI rewards.
    pub fn validate(&self) -> Result<(), BattleError> {
        let invalid = |why: &str| Err(BattleError::Invalid(why.to_owned()));
        if !rulebook::header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            RESULT_FORMAT,
        ) || !rulebook::is_sha256_hex(&self.snapshot_sha256)
        {
            return invalid("the result's header is not one this build writes");
        }
        if self.matches.len() > MAX_MATCHES {
            return invalid("too many matches in one session");
        }
        let ids: BTreeSet<&str> = self.matches.iter().map(|m| m.id.as_str()).collect();
        if ids.len() != self.matches.len()
            || !self.matches.iter().all(|m| {
                m.id.len() == 32
                    && m.id
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            })
        {
            return invalid("a match id is not 32 lowercase hex digits, or is used twice");
        }
        for m in &self.matches {
            let r = &m.rewards;
            let friendly = r.circuit_points > 0
                || !r.participation.is_empty()
                || !r.notebook.is_empty()
                || r.history.is_some();
            let solo = r.cup_win.is_some()
                || !r.unlocks.is_empty()
                || !r.journal.is_empty()
                || !r.keepsakes.is_empty();
            let fits = match m.mode {
                Mode::Friend => !solo,
                Mode::Ai => !friendly,
            };
            let bounded = r.circuit_points <= 100
                && r.participation.len() <= 3
                && r.notebook.len() <= 3
                && r.notebook.iter().all(|n| {
                    n.showing.validate()
                        && n.moments.len() <= MAX_MOMENTS
                        && n.moments.iter().all(|t| plain(t))
                })
                && r.history.as_ref().is_none_or(MatchSummary::valid)
                && r.cup_win.as_deref().is_none_or(is_id)
                && r.unlocks.len() <= MAX_LINES
                && r.unlocks.iter().all(|u| is_id(u))
                && r.journal.len() <= MAX_LINES
                && r.journal.iter().all(|t| plain(t))
                && r.keepsakes.len() <= MAX_LINES
                && r.keepsakes.iter().all(|k| is_id(k));
            if !fits {
                return invalid("a match earned rewards its kind of match never earns");
            }
            if !bounded {
                return invalid("a match's rewards are out of bounds");
            }
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, BattleError> {
        self.validate()?;
        Ok(rulebook::encode(self, result_kind())?)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, BattleError> {
        let result: Self = rulebook::decode(bytes, result_kind())?;
        result.validate()?;
        Ok(result)
    }

    /// Writes the result into its session directory, whole.
    pub fn write(&self, directory: &std::path::Path) -> Result<(), BattleError> {
        rulebook::write_atomically(&directory.join(RESULT_FILE), &self.encode()?)?;
        Ok(())
    }
}
