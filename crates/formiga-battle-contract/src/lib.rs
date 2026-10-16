//! The roster contract between Formiga Desktop and Formiga Battle: a read-only copy of the
//! colony's companions, and Battle's answer to it.
//!
//! It is built from Desktop's own pieces so the two cannot disagree: each
//! contender travels as the very [`formiga_travel::Traveler`] a trip to Formiga Hill carries
//! (look, temperament kind, axes, tension, traits), with the learned leanings a trip leaves at
//! home added beside it, and bonds travel in Desktop's own bands. Documents are bounded, checked
//! and written the way every companion app's are, through `formiga-expansion-rulebook`.
//!
//! Battle never changes a colony. Whatever it sends back (results, medals, souvenirs) is a later
//! version of this contract; nothing here is written by Battle but its acknowledgement. How Battle
//! turns a contender into a match profile is Battle's own, so this crate knows nothing of its
//! rules.
//!
//! # A session, file by file
//!
//! Everything for one session lives in one directory, named after its [`SessionId`]:
//!
//! | File | Written by | When |
//! |---|---|---|
//! | [`SNAPSHOT_FILE`] | Desktop | Before Battle is started. Never changed afterwards |
//! | [`ACK_FILE`] | Battle | Once it has read the snapshot, saying whether it can open it |
//! | [`RESULT_FILE`] | Battle | After every match, whole: every match the session played and what each earned. Desktop applies it when Battle closes |
//!
//! Battle is started with two arguments, [`LAUNCH_ARGUMENT`] and the session directory's
//! absolute path, and with nothing else. Every file is written whole to a temporary name and
//! renamed into place.
//!
//! | Version | What it added |
//! |---|---|
//! | 1 | The roster snapshot and the acknowledgement |
//! | 2 | The Battle record in the snapshot, and the result Battle answers with ([`result`]) |

use formiga_expansion_rulebook::{self as rulebook, Kind, sanitize_text};
pub use formiga_expansion_rulebook::{AckRefusal, SessionId};
use formiga_travel::{Presentation, TravelRelationship, Traveler};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;
use time::OffsetDateTime;

pub mod result;
pub mod sample;

pub use result::{
    BattleRecord, BattleResult, CupWins, MatchResult, MatchSummary, Mode, NotebookAddition,
    NotebookPage, RESULT_FILE, RESULT_FORMAT, Rewards, Showing,
};

/// The version of every Battle document this build writes, and the newest it reads.
pub const BATTLE_FORMAT_VERSION: u32 = 2;
pub const SNAPSHOT_FORMAT: &str = "formiga.battle.snapshot";
pub const SNAPSHOT_FILE: &str = "snapshot.json";
pub const ACK_FORMAT: &str = "formiga.battle.ack";
pub const ACK_FILE: &str = "ack.json";
/// The argument Battle is started with, followed by the session directory's absolute path.
pub const LAUNCH_ARGUMENT: &str = "--formiga-battle";

/// How Desktop would find an installed Battle without starting it: the same arrangement as Hill,
/// Home and Farm, under Battle's own names.
pub mod discovery {
    pub const MACOS_BUNDLE_ID: &str = "com.formiga.battle";
    /// An integer in Battle's `Info.plist`: the newest Battle version it reads.
    pub const MACOS_BATTLE_VERSION_KEY: &str = "FormigaBattleVersion";
    /// The per-user registry key Battle's Windows installer would write, under
    /// `HKEY_CURRENT_USER`, with the same key under `HKEY_LOCAL_MACHINE` for a machine-wide
    /// install.
    pub const WINDOWS_REGISTRY_KEY: &str = r"Software\Formiga\Battle";
    /// `REG_SZ`: the full path of Battle's executable.
    pub const WINDOWS_PATH_VALUE: &str = "Path";
    /// `REG_SZ`: Battle's version, for messages.
    pub const WINDOWS_VERSION_VALUE: &str = "Version";
    /// `REG_DWORD`: the newest Battle version it reads.
    pub const WINDOWS_BATTLE_VERSION_VALUE: &str = "BattleVersion";
    /// For development: the path of a Battle executable (or, on macOS, an `.app`) to use instead
    /// of an installed one.
    pub const PATH_OVERRIDE_ENV: &str = "FORMIGA_BATTLE_PATH";
}

pub mod limits {
    pub const MAX_SNAPSHOT_BYTES: u64 = 256 * 1024;
    pub const MAX_ACK_BYTES: u64 = 4 * 1024;
    pub const MAX_CONTENDERS: usize = formiga_travel::limits::MAX_TRAVELERS;
    pub const MAX_RELATIONSHIPS: usize = formiga_travel::limits::MAX_RELATIONSHIPS;
    pub const MAX_VERSION_CHARS: usize = 32;
}

rulebook::contract_error!(pub enum BattleError, "Battle");

/// Desktop's learned leanings, whole numbers from -100 to 100. A trip to Hill leaves them at
/// home; Battle reads them, because how a companion has come to live is part of how it walks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContenderLeanings {
    pub cursor_trust: i8,
    pub sociability: i8,
    pub climbing: i8,
    pub sleep_security: i8,
    pub exploration: i8,
    pub play: i8,
    pub home_affinity: i8,
    pub routine: i8,
}

impl From<&formiga_core::LearnedTendencies> for ContenderLeanings {
    /// Each tendency to the nearest whole number on its scale; one that is not a number at all
    /// rests at zero.
    fn from(t: &formiga_core::LearnedTendencies) -> Self {
        let q = |v: f32| {
            if v.is_finite() {
                v.round().clamp(-100.0, 100.0) as i8
            } else {
                0
            }
        };
        Self {
            cursor_trust: q(t.cursor_trust),
            sociability: q(t.sociability),
            climbing: q(t.climbing),
            sleep_security: q(t.sleep_security),
            exploration: q(t.exploration),
            play: q(t.play),
            home_affinity: q(t.home_affinity),
            routine: q(t.routine),
        }
    }
}

impl ContenderLeanings {
    fn values(&self) -> [i8; 8] {
        [
            self.cursor_trust,
            self.sociability,
            self.climbing,
            self.sleep_security,
            self.exploration,
            self.play,
            self.home_affinity,
            self.routine,
        ]
    }
}

/// One companion who could be picked for a team.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Contender {
    #[serde(flatten)]
    pub traveler: Traveler,
    pub leanings: ContenderLeanings,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BattleSnapshot {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    /// The same one-way colony digest a trip to Hill carries.
    pub colony_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at_utc: OffsetDateTime,
    pub desktop_version: String,
    pub contenders: Vec<Contender>,
    #[serde(default)]
    pub relationships: Vec<TravelRelationship>,
    #[serde(default)]
    pub presentation: Presentation,
    /// What Desktop keeps of Battle for this colony. Since version 2; absent before, and for a
    /// colony that has never played.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<BattleRecord>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error(transparent)]
    Travel(#[from] formiga_travel::ProjectionError),
    #[error(transparent)]
    Battle(#[from] BattleError),
}

fn kind() -> Kind {
    Kind {
        format: SNAPSHOT_FORMAT,
        max_bytes: limits::MAX_SNAPSHOT_BYTES,
        reads: BATTLE_FORMAT_VERSION,
    }
}

/// The roster Desktop would hand Battle for a colony: every companion as a trip carries it, with
/// its leanings. The same colony, session and time give the same snapshot, byte for byte.
pub fn project_roster(
    save: &formiga_core::SaveFile,
    session_id: SessionId,
    created_at_utc: OffsetDateTime,
    desktop_version: &str,
) -> Result<BattleSnapshot, ProjectionError> {
    let travel = formiga_travel::project_colony(save, session_id, created_at_utc, desktop_version)?;
    let contenders = travel
        .travelers
        .into_iter()
        .filter_map(|traveler| {
            let creature = save.creatures.iter().find(|c| c.id == traveler.id.0)?;
            Some(Contender {
                traveler,
                leanings: ContenderLeanings::from(&creature.tendencies),
            })
        })
        .collect();
    let snapshot = BattleSnapshot {
        format: SNAPSHOT_FORMAT.to_owned(),
        version: BATTLE_FORMAT_VERSION,
        min_reader_version: 1,
        session_id: travel.session_id,
        colony_id: travel.colony_id,
        created_at_utc: travel.created_at_utc,
        desktop_version: travel.desktop_version,
        contenders,
        relationships: travel.relationships,
        presentation: travel.presentation,
        record: None,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

impl BattleSnapshot {
    pub fn validate(&self) -> Result<(), BattleError> {
        let invalid = |why: &str| BattleError::Invalid(why.to_owned());
        if !rulebook::header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            SNAPSHOT_FORMAT,
        ) {
            return Err(invalid(
                "the snapshot's header is not one this build writes",
            ));
        }
        if sanitize_text(&self.desktop_version, limits::MAX_VERSION_CHARS) != self.desktop_version {
            return Err(invalid("the Desktop version is not plain text"));
        }
        if self.contenders.is_empty() || self.contenders.len() > limits::MAX_CONTENDERS {
            return Err(invalid("a roster has between one and twelve companions"));
        }
        let ids: BTreeSet<u64> = self.contenders.iter().map(|c| c.traveler.id.0).collect();
        if ids.len() != self.contenders.len() {
            return Err(invalid("two contenders are the same companion"));
        }
        for c in &self.contenders {
            let t = &c.traveler;
            if !rulebook::is_sanitized(&t.name, formiga_travel::limits::MAX_NAME_CHARS)
                || t.name.is_empty()
            {
                return Err(invalid("a contender's name is not plain text"));
            }
            let a = t.character.axes;
            let unit = |v: f32| v.is_finite() && (0.0..=1.0).contains(&v);
            if ![
                a.social,
                a.energy,
                a.boldness,
                a.playfulness,
                a.curiosity,
                a.feistiness,
                a.impulsiveness,
                a.suspicion,
                a.affection,
            ]
            .into_iter()
            .all(unit)
            {
                return Err(invalid("a temperament axis is off its scale"));
            }
            if !c.leanings.values().iter().all(|v| (-100..=100).contains(v)) {
                return Err(invalid("a leaning is off its scale"));
            }
        }
        if self.relationships.len() > limits::MAX_RELATIONSHIPS
            || !self
                .relationships
                .iter()
                .all(|r| r.a.0 < r.b.0 && ids.contains(&r.a.0) && ids.contains(&r.b.0))
        {
            return Err(invalid("a bond names someone not on the roster"));
        }
        if let Some(record) = &self.record {
            record.validate()?;
        }
        Ok(())
    }

    /// The same snapshot, carrying what Desktop keeps of Battle for the colony.
    pub fn with_record(mut self, record: BattleRecord) -> Self {
        self.record = Some(record);
        self
    }

    /// The snapshot's bytes, once validated. The same snapshot always gives the same bytes.
    pub fn encode(&self) -> Result<Vec<u8>, BattleError> {
        self.validate()?;
        Ok(rulebook::encode(self, kind())?)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, BattleError> {
        let snapshot: Self = rulebook::decode(bytes, kind())?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub fn read(path: &std::path::Path) -> Result<Self, BattleError> {
        Self::decode(&rulebook::read_bounded(path, limits::MAX_SNAPSHOT_BYTES)?)
    }
}

/// Battle's answer to a session: it can open the roster, or why not.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleAck {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    /// The version of Battle that answered, for messages only.
    pub battle_version: String,
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<AckRefusal>,
}

fn ack_kind() -> Kind {
    Kind {
        format: ACK_FORMAT,
        max_bytes: limits::MAX_ACK_BYTES,
        reads: BATTLE_FORMAT_VERSION,
    }
}

impl BattleAck {
    pub fn new(session_id: SessionId, battle_version: &str, refusal: Option<AckRefusal>) -> Self {
        Self {
            format: ACK_FORMAT.to_owned(),
            version: BATTLE_FORMAT_VERSION,
            min_reader_version: 1,
            session_id,
            battle_version: sanitize_text(battle_version, limits::MAX_VERSION_CHARS),
            accepted: refusal.is_none(),
            refusal,
        }
    }

    pub fn validate(&self) -> Result<(), BattleError> {
        if !rulebook::header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            ACK_FORMAT,
        ) || self.accepted != self.refusal.is_none()
        {
            return Err(BattleError::Invalid(
                "an acknowledgement either accepts or says why not".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, BattleError> {
        self.validate()?;
        Ok(rulebook::encode(self, ack_kind())?)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, BattleError> {
        let ack: Self = rulebook::decode(bytes, ack_kind())?;
        ack.validate()?;
        Ok(ack)
    }
}

/// What opening a session directory gave: the roster, or the refusal Battle has already written
/// into the directory for Desktop to read.
#[derive(Debug)]
pub enum Opened {
    Roster(Box<BattleSnapshot>),
    Refused(AckRefusal, BattleError),
}

/// Reads a session directory as Battle does when Desktop starts it, and answers in it. The
/// directory must be named after the snapshot's own session, so a snapshot copied anywhere else
/// is not mistaken for a session.
pub fn open_session(directory: &Path, battle_version: &str) -> Result<Opened, BattleError> {
    let read = BattleSnapshot::read(&directory.join(SNAPSHOT_FILE)).and_then(|snapshot| {
        let named =
            directory.file_name().and_then(|n| n.to_str()) == Some(snapshot.session_id.as_str());
        if named {
            Ok(snapshot)
        } else {
            Err(BattleError::Invalid(
                "the session directory is not named after its snapshot's session".to_owned(),
            ))
        }
    });
    let session = directory
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(SessionId::parse);
    let (ack, opened) = match read {
        Ok(snapshot) => (
            BattleAck::new(snapshot.session_id.clone(), battle_version, None),
            Opened::Roster(Box::new(snapshot)),
        ),
        Err(error) => {
            let refusal = match error {
                BattleError::UnsupportedVersion { .. } => AckRefusal::UnsupportedVersion {
                    reads: BATTLE_FORMAT_VERSION,
                },
                _ => AckRefusal::Invalid,
            };
            let Some(session) = session else {
                // Nothing can be answered in a directory that is not a session's.
                return Err(error);
            };
            (
                BattleAck::new(session, battle_version, Some(refusal)),
                Opened::Refused(refusal, error),
            )
        }
    };
    rulebook::write_atomically(&directory.join(ACK_FILE), &ack.encode()?)?;
    Ok(opened)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_roster_round_trips_byte_for_byte() {
        let snapshot = sample::snapshot();
        let bytes = snapshot.encode().unwrap();
        let read = BattleSnapshot::decode(&bytes).unwrap();
        assert_eq!(read, snapshot);
        assert_eq!(read.encode().unwrap(), bytes);
    }

    #[test]
    fn a_snapshot_from_a_newer_battle_reader_is_refused_for_its_version() {
        let mut snapshot = sample::snapshot();
        snapshot.version = BATTLE_FORMAT_VERSION + 1;
        snapshot.min_reader_version = BATTLE_FORMAT_VERSION + 1;
        let bytes = serde_json::to_vec(&snapshot).unwrap();
        assert!(matches!(
            BattleSnapshot::decode(&bytes),
            Err(BattleError::UnsupportedVersion { .. })
        ));
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "formiga-battle-contract-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_session_is_opened_and_answered_in_its_own_directory() {
        let snapshot = sample::snapshot();
        let dir = scratch("open").join(snapshot.session_id.as_str());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(SNAPSHOT_FILE), snapshot.encode().unwrap()).unwrap();
        let opened = open_session(&dir, "0.1.0").unwrap();
        assert!(matches!(opened, Opened::Roster(ref read) if **read == snapshot));
        let ack = BattleAck::decode(&std::fs::read(dir.join(ACK_FILE)).unwrap()).unwrap();
        assert!(ack.accepted);
        assert_eq!(ack.session_id, snapshot.session_id);
    }

    #[test]
    fn a_snapshot_that_does_not_check_out_is_answered_with_a_refusal() {
        let snapshot = sample::snapshot();
        let dir = scratch("refuse").join(snapshot.session_id.as_str());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(SNAPSHOT_FILE),
            b"{\"format\":\"formiga.battle.snapshot\"}",
        )
        .unwrap();
        let opened = open_session(&dir, "0.1.0").unwrap();
        assert!(matches!(opened, Opened::Refused(AckRefusal::Invalid, _)));
        let ack = BattleAck::decode(&std::fs::read(dir.join(ACK_FILE)).unwrap()).unwrap();
        assert!(!ack.accepted);
    }

    #[test]
    fn every_companion_is_on_the_roster_with_its_leanings_rounded() {
        let save = sample::colony();
        let snapshot = sample::snapshot();
        assert_eq!(snapshot.contenders.len(), save.creatures.len());
        for (contender, creature) in snapshot.contenders.iter().zip(&save.creatures) {
            assert_eq!(contender.traveler.id.0, creature.id);
            let t = &creature.tendencies;
            assert_eq!(
                i32::from(contender.leanings.exploration),
                t.exploration.round() as i32
            );
            assert_eq!(i32::from(contender.leanings.play), t.play.round() as i32);
        }
        assert!(
            snapshot
                .contenders
                .iter()
                .any(|c| c.leanings != ContenderLeanings::default()),
            "the sample's companions have lived a little"
        );
    }

    #[test]
    fn a_tendency_that_is_not_a_number_rests_at_zero() {
        let tendencies = formiga_core::LearnedTendencies {
            play: f32::NAN,
            routine: 250.0,
            ..Default::default()
        };
        let leanings = ContenderLeanings::from(&tendencies);
        assert_eq!((leanings.play, leanings.routine), (0, 100));
    }

    #[test]
    fn a_result_applies_to_the_record_once_and_only_once() {
        let mut record = sample::record();
        let result = sample::result();
        let points = record.circuit_points;
        for m in &result.matches {
            assert!(record.apply(m));
        }
        assert_eq!(record.circuit_points, points + 15);
        assert_eq!(record.cup_wins("cup.backyard"), 2);
        assert_eq!(record.history.len(), 2);
        let again = record.clone();
        for m in &result.matches {
            assert!(!record.apply(m), "a match already applied changes nothing");
        }
        assert_eq!(record, again);
        assert!(record.validate().is_ok());
    }

    #[test]
    fn a_session_of_many_unlocks_keeps_the_record_within_its_bounds() {
        let mut record = sample::record();
        for n in 0..result::result_limits::MAX_MATCHES {
            let m = MatchResult {
                id: format!("{:032x}", 0x100 + n),
                mode: Mode::Ai,
                rewards: Rewards {
                    cup_win: Some(format!("cup.c{n}")),
                    unlocks: (0..result::result_limits::MAX_LINES)
                        .map(|u| format!("unlock.u{n}_{u}"))
                        .collect(),
                    ..Rewards::default()
                },
            };
            assert!(record.apply(&m));
        }
        assert!(record.validate().is_ok());
        assert_eq!(record.unlocked.last().unwrap(), "unlock.u63_7");
        assert_eq!(record.cups.last().unwrap().cup, "cup.c63");
    }

    #[test]
    fn a_friend_match_cannot_earn_keepsakes_nor_an_ai_match_points() {
        let mut result = sample::result();
        result.matches[0].rewards.keepsakes = vec!["battle.trophy".to_owned()];
        assert!(result.validate().is_err());
        let mut result = sample::result();
        result.matches[1].rewards.circuit_points = 5;
        assert!(result.validate().is_err());
    }

    #[test]
    fn a_leaning_off_its_scale_is_refused() {
        let mut snapshot = sample::snapshot();
        snapshot.contenders[0].leanings.play = 127;
        assert!(snapshot.validate().is_err());
    }
}
