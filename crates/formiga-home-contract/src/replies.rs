//! What Home says back: that it has the household, how it would have the houses arranged, and
//! what the visit brings home. And what Desktop says if it ends the visit first.
//!
//! Every answer is sealed to the visit it answers: its session, and the exact bytes of the
//! snapshot and the state Desktop wrote for it, so an answer can only ever be about one visit and
//! one starting point.

use crate::document::{HomeDocument, HomeError, header_ok, is_sha256_hex};
use crate::limits::*;
use crate::state::HomeMoment;
use crate::state::HomeState;
use crate::{
    ACK_FORMAT, HOME_FORMAT_VERSION, HomeSnapshot, RECALL_FORMAT, RECEIPT_FORMAT, RESULT_FORMAT,
};
use formiga_travel::{SessionId, TravelerId, is_sanitized, sanitize_text};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// What every answer must match: the visit, and the exact bytes of the snapshot and the state
/// Desktop wrote for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSeal {
    pub session_id: SessionId,
    pub snapshot_sha256: String,
    pub state_sha256: String,
    pub created_at_utc: OffsetDateTime,
}

impl SessionSeal {
    /// The seal of a visit, from the bytes its snapshot and state were written as.
    pub fn of(snapshot: &HomeSnapshot, snapshot_bytes: &[u8], state_bytes: &[u8]) -> Self {
        Self {
            session_id: snapshot.session_id.clone(),
            snapshot_sha256: crate::sha256_hex(snapshot_bytes),
            state_sha256: crate::sha256_hex(state_bytes),
            created_at_utc: snapshot.created_at_utc,
        }
    }

    fn matches(&self, session_id: &SessionId, snapshot: &str, state: &str) -> bool {
        &self.session_id == session_id
            && self.snapshot_sha256 == snapshot
            && self.state_sha256 == state
    }
}

/// Why Home could not take the household.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AckRefusal {
    /// The snapshot or state needs a newer reader than this Home has; `reads` is the newest Home
    /// version it has.
    UnsupportedVersion { reads: u32 },
    /// The snapshot or state did not check out.
    Invalid,
    /// Home already has a house open.
    Busy,
    /// Anything a newer Home says that this build does not know.
    #[serde(other)]
    Other,
}

/// Home's answer once it has read the snapshot and the state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomeAck {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    pub snapshot_sha256: String,
    pub state_sha256: String,
    pub home_version: String,
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<AckRefusal>,
}

impl HomeAck {
    pub fn accepted(seal: &SessionSeal, home_version: &str) -> Self {
        Self::new(seal, home_version, None)
    }

    pub fn refused(seal: &SessionSeal, home_version: &str, refusal: AckRefusal) -> Self {
        Self::new(seal, home_version, Some(refusal))
    }

    fn new(seal: &SessionSeal, home_version: &str, refusal: Option<AckRefusal>) -> Self {
        Self {
            format: ACK_FORMAT.to_owned(),
            version: HOME_FORMAT_VERSION,
            min_reader_version: 1,
            session_id: seal.session_id.clone(),
            snapshot_sha256: seal.snapshot_sha256.clone(),
            state_sha256: seal.state_sha256.clone(),
            home_version: version_text(home_version),
            accepted: refusal.is_none(),
            refusal,
        }
    }

    /// Whether this acknowledgement is about exactly the visit `seal` names.
    pub fn answers(&self, seal: &SessionSeal) -> bool {
        seal.matches(&self.session_id, &self.snapshot_sha256, &self.state_sha256)
    }
}

impl HomeDocument for HomeAck {
    const FORMAT: &'static str = ACK_FORMAT;
    const MAX_BYTES: u64 = MAX_ACK_BYTES;

    fn validate(&self) -> Result<(), HomeError> {
        if !header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            ACK_FORMAT,
        ) || !is_sha256_hex(&self.snapshot_sha256)
            || !is_sha256_hex(&self.state_sha256)
            || !is_sanitized(&self.home_version, MAX_VERSION_CHARS)
            || self.accepted == self.refusal.is_some()
        {
            return Err(HomeError::invalid(
                "an acknowledgement that does not add up",
            ));
        }
        Ok(())
    }
}

/// How Home would have every house arranged, whole: the state Desktop sent, with the visited
/// household's home as the owner left it. Desktop keeps only what [`crate::accept_result`] lets
/// it keep.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomeResult {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    pub snapshot_sha256: String,
    pub state_sha256: String,
    #[serde(with = "time::serde::rfc3339")]
    pub written_at_utc: OffsetDateTime,
    pub home_version: String,
    pub state: HomeState,
}

impl HomeResult {
    pub fn new(
        seal: &SessionSeal,
        written_at_utc: OffsetDateTime,
        home_version: &str,
        state: HomeState,
    ) -> Self {
        Self {
            format: RESULT_FORMAT.to_owned(),
            version: HOME_FORMAT_VERSION,
            min_reader_version: 1,
            session_id: seal.session_id.clone(),
            snapshot_sha256: seal.snapshot_sha256.clone(),
            state_sha256: seal.state_sha256.clone(),
            written_at_utc,
            home_version: version_text(home_version),
            state,
        }
    }

    pub fn answers(&self, seal: &SessionSeal) -> bool {
        seal.matches(&self.session_id, &self.snapshot_sha256, &self.state_sha256)
    }
}

impl HomeDocument for HomeResult {
    const FORMAT: &'static str = RESULT_FORMAT;
    const MAX_BYTES: u64 = MAX_RESULT_BYTES;

    fn validate(&self) -> Result<(), HomeError> {
        if !header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            RESULT_FORMAT,
        ) || !is_sha256_hex(&self.snapshot_sha256)
            || !is_sha256_hex(&self.state_sha256)
            || !is_sanitized(&self.home_version, MAX_VERSION_CHARS)
        {
            return Err(HomeError::invalid("a result that does not add up"));
        }
        self.state.validate()
    }
}

/// Something a visit brings home.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HomeEffect {
    /// The owner spent time in the household's house, from when it opened to when they left.
    /// Applied by a Desktop that offers [`crate::HomeCapability::VisitRecord`], at most once a
    /// visit.
    HomeVisit {
        household: TravelerId,
        #[serde(with = "time::serde::rfc3339")]
        arrived_at_utc: OffsetDateTime,
        #[serde(with = "time::serde::rfc3339")]
        left_at_utc: OffsetDateTime,
    },
    /// Two companions spent time together at home, of one kind, `times` times and at most
    /// [`crate::limits::MAX_TOGETHER`] a visit. Applied by a Desktop that offers
    /// [`crate::HomeCapability::BondNudges`], by its own rules, and only as far as they let it.
    Together {
        a: TravelerId,
        b: TravelerId,
        together: Together,
        times: u8,
    },
    /// Something that happened at home worth a line in Desktop's journal, in Desktop's own
    /// words. Offered by [`crate::HomeCapability::JournalMoments`]; at most
    /// [`crate::limits::MAX_MOMENTS`] a visit.
    Moment { moment: HomeMoment },
    /// The owner went over to another house in the village, its keeper's: Desktop opens it
    /// next, if it offers [`crate::HomeCapability::NextDoor`] and can, by its own rules. At most
    /// once a visit. Since version 5.
    NextDoor { household: TravelerId },
    /// The owner asked `resident`, a friend visiting, to come and live in the house `household`
    /// keeps, the one visited, and it would like to. A request only: Desktop grants it or not by
    /// its own rules, if it offers [`crate::HomeCapability::Roommates`]. At most once a visit.
    /// Since version 6.
    MoveIn {
        resident: TravelerId,
        household: TravelerId,
    },
    /// Anything a newer Home sends that this build does not know.
    #[serde(other)]
    Unsupported,
}

/// A kind of time spent together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Together {
    /// Played together, or with a toy between them.
    Play,
    /// Sat together, shared a snack, turned in together.
    Cozy,
    /// One comforted or hugged the other.
    Care,
    /// One teased or grumbled at the other.
    Squabble,
    #[serde(other)]
    Unknown,
}

impl HomeEffect {
    /// A short, fixed name for logs.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::HomeVisit { .. } => "home_visit",
            Self::Together { .. } => "together",
            Self::Moment { .. } => "moment",
            Self::NextDoor { .. } => "next_door",
            Self::MoveIn { .. } => "move_in",
            Self::Unsupported => "unsupported",
        }
    }
}

/// What Home writes when the owner leaves the house.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomeReceipt {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    pub snapshot_sha256: String,
    pub state_sha256: String,
    #[serde(with = "time::serde::rfc3339")]
    pub closed_at_utc: OffsetDateTime,
    pub home_version: String,
    #[serde(default)]
    pub effects: Vec<HomeEffect>,
}

impl HomeReceipt {
    pub fn new(
        seal: &SessionSeal,
        closed_at_utc: OffsetDateTime,
        home_version: &str,
        effects: Vec<HomeEffect>,
    ) -> Self {
        Self {
            format: RECEIPT_FORMAT.to_owned(),
            version: HOME_FORMAT_VERSION,
            min_reader_version: 1,
            session_id: seal.session_id.clone(),
            snapshot_sha256: seal.snapshot_sha256.clone(),
            state_sha256: seal.state_sha256.clone(),
            closed_at_utc,
            home_version: version_text(home_version),
            effects,
        }
    }

    pub fn answers(&self, seal: &SessionSeal) -> bool {
        seal.matches(&self.session_id, &self.snapshot_sha256, &self.state_sha256)
    }

    /// The house to open next, if the owner went over to one Desktop offered to open: another
    /// house of the village the snapshot names, never the one just visited.
    pub fn next_door(&self, snapshot: &HomeSnapshot) -> Option<TravelerId> {
        if !snapshot.offers(crate::HomeCapability::NextDoor) {
            return None;
        }
        self.effects.iter().find_map(|effect| match effect {
            HomeEffect::NextDoor { household }
                if *household != snapshot.household.keeper
                    && snapshot.neighbour(*household).is_some() =>
            {
                Some(*household)
            }
            _ => None,
        })
    }
}

impl HomeReceipt {
    /// Who the owner asked to move in, and into whose house, if Desktop offered to consider it:
    /// a friend lent for the visit, into the house visited.
    pub fn move_in(&self, snapshot: &HomeSnapshot) -> Option<(TravelerId, TravelerId)> {
        if !snapshot.offers(crate::HomeCapability::Roommates) {
            return None;
        }
        self.effects.iter().find_map(|effect| match effect {
            HomeEffect::MoveIn {
                resident,
                household,
            } if *household == snapshot.household.keeper
                && snapshot.visitor(*resident).is_some() =>
            {
                Some((*resident, *household))
            }
            _ => None,
        })
    }
}

impl HomeDocument for HomeReceipt {
    const FORMAT: &'static str = RECEIPT_FORMAT;
    const MAX_BYTES: u64 = MAX_RECEIPT_BYTES;

    fn validate(&self) -> Result<(), HomeError> {
        let invalid = HomeError::invalid;
        if !header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            RECEIPT_FORMAT,
        ) || !is_sha256_hex(&self.snapshot_sha256)
            || !is_sha256_hex(&self.state_sha256)
            || !is_sanitized(&self.home_version, MAX_VERSION_CHARS)
        {
            return Err(invalid("a receipt that does not add up"));
        }
        if self.effects.len() > MAX_EFFECTS {
            return Err(invalid("a receipt with too many effects"));
        }
        let backwards = self.effects.iter().any(|effect| {
            matches!(effect, HomeEffect::HomeVisit { arrived_at_utc, left_at_utc, .. }
                if arrived_at_utc > left_at_utc)
        });
        if backwards {
            return Err(invalid("a visit that ended before it began"));
        }
        // Each pair counted once for each kind of time, within bounds, and never with itself.
        let mut pairs = std::collections::BTreeSet::new();
        for effect in &self.effects {
            if let HomeEffect::Together {
                a,
                b,
                together,
                times,
            } = effect
            {
                let pair = (a.min(b), a.max(b), *together);
                if a == b || !(1..=MAX_TOGETHER).contains(times) || !pairs.insert(pair) {
                    return Err(invalid("time together that does not add up"));
                }
            }
        }
        let moments = self
            .effects
            .iter()
            .filter(|effect| matches!(effect, HomeEffect::Moment { .. }))
            .count();
        if moments > MAX_MOMENTS {
            return Err(invalid("a receipt with too many moments"));
        }
        let next = self
            .effects
            .iter()
            .filter(|effect| matches!(effect, HomeEffect::NextDoor { .. }))
            .count();
        if next > 1 {
            return Err(invalid("a receipt going to two houses at once"));
        }
        let mut moves = 0;
        for effect in &self.effects {
            if let HomeEffect::MoveIn {
                resident,
                household,
            } = effect
            {
                moves += 1;
                if resident == household || moves > 1 {
                    return Err(invalid("a move that does not add up"));
                }
            }
        }
        Ok(())
    }
}

/// Why Desktop ended the visit without waiting for Home.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallReason {
    /// The owner asked for the household back on the desktop.
    OwnerAsked,
    /// Desktop started again and found the visit still open.
    DesktopRestarted,
    #[serde(other)]
    Other,
}

/// Desktop's word that the visit is over. Home should close the house without writing anything
/// more; anything it writes afterwards is never read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomeRecall {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    #[serde(with = "time::serde::rfc3339")]
    pub at_utc: OffsetDateTime,
    pub reason: RecallReason,
}

impl HomeRecall {
    pub fn new(session_id: SessionId, at_utc: OffsetDateTime, reason: RecallReason) -> Self {
        Self {
            format: RECALL_FORMAT.to_owned(),
            version: HOME_FORMAT_VERSION,
            min_reader_version: 1,
            session_id,
            at_utc,
            reason,
        }
    }
}

impl HomeDocument for HomeRecall {
    const FORMAT: &'static str = RECALL_FORMAT;
    const MAX_BYTES: u64 = MAX_RECALL_BYTES;

    fn validate(&self) -> Result<(), HomeError> {
        if !header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            RECALL_FORMAT,
        ) {
            return Err(HomeError::invalid("a recall that does not add up"));
        }
        Ok(())
    }
}

/// A version as it may be written: plain text, never empty.
fn version_text(version: &str) -> String {
    Some(sanitize_text(version, MAX_VERSION_CHARS))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode, encode};
    use time::macros::datetime;

    fn seal() -> SessionSeal {
        SessionSeal {
            session_id: SessionId::parse("00112233445566778899aabbccddeeff").unwrap(),
            snapshot_sha256: "ab".repeat(32),
            state_sha256: "cd".repeat(32),
            created_at_utc: datetime!(2026-10-05 12:00 UTC),
        }
    }

    #[test]
    fn effects_a_newer_home_sends_are_read_as_unsupported() {
        let effects: Vec<HomeEffect> = serde_json::from_str(
            r#"[
                {"kind": "home_visit", "household": "0000000000000007",
                 "arrived_at_utc": "2026-10-05T12:00:00Z", "left_at_utc": "2026-10-05T12:20:00Z"},
                {"kind": "bond_nudge", "a": "0000000000000007", "b": "0000000000000008"}
            ]"#,
        )
        .unwrap();
        assert_eq!(effects[0].kind(), "home_visit");
        assert_eq!(effects[1], HomeEffect::Unsupported);
    }

    #[test]
    fn every_answer_names_only_its_own_visit() {
        let receipt = HomeReceipt::new(&seal(), datetime!(2026-10-05 12:20 UTC), "0.1.0", vec![]);
        let read: HomeReceipt = decode(&encode(&receipt).unwrap()).unwrap();
        assert!(read.answers(&seal()));
        let mut moved_on = seal();
        moved_on.state_sha256 = "ef".repeat(32);
        assert!(
            !read.answers(&moved_on),
            "a receipt for an older state is not this one's"
        );
        let ack = HomeAck::refused(&seal(), "0.1.0", AckRefusal::Busy);
        let read: HomeAck = decode(&encode(&ack).unwrap()).unwrap();
        assert!(read.answers(&seal()) && !read.accepted);
    }

    #[test]
    fn answers_that_do_not_add_up_are_refused() {
        let base = HomeReceipt::new(&seal(), datetime!(2026-10-05 12:20 UTC), "0.1.0", vec![]);
        let mut backwards = base.clone();
        backwards.effects = vec![HomeEffect::HomeVisit {
            household: TravelerId(7),
            arrived_at_utc: datetime!(2026-10-05 13:00 UTC),
            left_at_utc: datetime!(2026-10-05 12:00 UTC),
        }];
        let mut flood = base.clone();
        flood.effects = vec![HomeEffect::Unsupported; MAX_EFFECTS + 1];
        let mut prose = base.clone();
        prose.home_version = "0.1.0\u{202E}evil".to_owned();
        let mut unsealed = base;
        unsealed.state_sha256 = "not a digest".to_owned();
        for receipt in [backwards, flood, prose, unsealed] {
            assert!(receipt.validate().is_err(), "{receipt:?} was accepted");
        }
        let mut muddled = HomeAck::accepted(&seal(), "0.1.0");
        muddled.refusal = Some(AckRefusal::Invalid);
        assert!(muddled.validate().is_err());
    }

    #[test]
    fn time_together_is_counted_once_a_pair_and_kind_within_bounds_and_moments_are_few() {
        let base = HomeReceipt::new(&seal(), datetime!(2026-10-05 12:20 UTC), "0.1.0", vec![]);
        let together = |a: u64, b: u64, together: Together, times: u8| HomeEffect::Together {
            a: TravelerId(a),
            b: TravelerId(b),
            together,
            times,
        };
        let mut fine = base.clone();
        fine.effects = vec![
            together(7, 8, Together::Play, 3),
            together(7, 8, Together::Care, 1),
            together(9, 7, Together::Play, 1),
        ];
        assert!(fine.validate().is_ok());
        let mut twice = base.clone();
        twice.effects = vec![
            together(7, 8, Together::Play, 1),
            together(8, 7, Together::Play, 2),
        ];
        let mut alone = base.clone();
        alone.effects = vec![together(7, 7, Together::Cozy, 1)];
        let mut too_often = base.clone();
        too_often.effects = vec![together(7, 8, Together::Squabble, MAX_TOGETHER + 1)];
        let mut never = base.clone();
        never.effects = vec![together(7, 8, Together::Squabble, 0)];
        let mut chatty = base;
        chatty.effects = vec![
            HomeEffect::Moment {
                moment: HomeMoment::Visit {
                    visitor: TravelerId(8)
                }
            };
            MAX_MOMENTS + 1
        ];
        for receipt in [twice, alone, too_often, never, chatty] {
            assert!(receipt.validate().is_err(), "{receipt:?} was accepted");
        }
    }

    #[test]
    fn only_a_visiting_friend_can_be_asked_to_move_in_and_only_once() {
        let snapshot = crate::sample::snapshot();
        let (keeper, friend) = (snapshot.household.keeper, snapshot.visitors[0].id);
        let moving = |resident, household| HomeEffect::MoveIn {
            resident,
            household,
        };
        let mut receipt = HomeReceipt::new(
            &seal(),
            datetime!(2026-10-05 12:20 UTC),
            "0.1.0",
            vec![moving(friend, keeper)],
        );
        assert!(receipt.validate().is_ok());
        assert_eq!(receipt.move_in(&snapshot), Some((friend, keeper)));
        let mut declined = snapshot.clone();
        declined
            .capabilities
            .retain(|capability| *capability != crate::HomeCapability::Roommates);
        assert_eq!(receipt.move_in(&declined), None, "not offered");
        receipt.effects = vec![moving(snapshot.residents[1].id, keeper)];
        assert_eq!(receipt.move_in(&snapshot), None, "lives here already");
        receipt.effects = vec![moving(friend, friend)];
        assert!(receipt.validate().is_err(), "into its own house");
        receipt.effects = vec![moving(friend, keeper), moving(friend, keeper)];
        assert!(receipt.validate().is_err(), "twice");
    }

    #[test]
    fn the_owner_goes_next_door_once_and_only_to_a_house_desktop_offered() {
        let snapshot = crate::sample::snapshot();
        let neighbour = snapshot
            .village
            .iter()
            .find(|house| house.keeper != snapshot.household.keeper)
            .unwrap()
            .keeper;
        let going = |household| HomeEffect::NextDoor { household };
        let mut receipt = HomeReceipt::new(
            &seal(),
            datetime!(2026-10-05 12:20 UTC),
            "0.1.0",
            vec![going(neighbour)],
        );
        assert!(receipt.validate().is_ok());
        assert_eq!(receipt.next_door(&snapshot), Some(neighbour));
        let mut declined = snapshot.clone();
        declined
            .capabilities
            .retain(|capability| *capability != crate::HomeCapability::NextDoor);
        assert_eq!(receipt.next_door(&declined), None, "not offered");
        receipt.effects = vec![going(snapshot.household.keeper)];
        assert_eq!(receipt.next_door(&snapshot), None, "the house just visited");
        receipt.effects = vec![going(TravelerId(4040))];
        assert_eq!(receipt.next_door(&snapshot), None, "no such house");
        receipt.effects = vec![going(neighbour), going(neighbour)];
        assert!(receipt.validate().is_err(), "two houses at once");
    }
}
