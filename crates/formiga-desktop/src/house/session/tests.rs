use super::*;
use formiga_core::SaveFile;
use formiga_home_contract::{
    AckRefusal, CatalogId, DisplayId, HouseholdHome, PlacedDisplay, PlacedPiece, RoomLayout,
    SessionId, Spot, TravelerId, project_household, sample,
};
use time::macros::datetime;

const OPENED: OffsetDateTime = datetime!(2026-11-12 10:00 UTC);

/// A directory of its own under the system's temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "formiga-house-session-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn colony() -> SaveFile {
    sample::colony()
}

/// The sample household's snapshot for a visit of its own, taking back only the visit.
fn snapshot(save: &SaveFile) -> HomeSnapshot {
    project_household(
        save,
        sample::keeper(save),
        &[],
        &[HomeCapability::VisitRecord],
        SessionId::generate().unwrap(),
        OPENED,
        "0.67.0",
    )
    .unwrap()
}

/// The visited household's home with the shell set out on a shelf.
fn arranged(keeper: TravelerId) -> HouseholdHome {
    HouseholdHome {
        keeper,
        rooms: vec![RoomLayout {
            width: 6,
            depth: 6,
            floor: CatalogId::known("floor.boards"),
            wall: CatalogId::known("wall.plaster"),
            pieces: vec![PlacedPiece {
                uid: 1,
                piece: CatalogId::known("shelf"),
                x: 0,
                y: 2,
                turn: 1,
            }],
            displays: vec![PlacedDisplay {
                item: DisplayId::find(3),
                spot: Spot::On { piece: 1, slot: 0 },
            }],
            plan: None,
            kind: None,
            doors: Vec::new(),
        }],
        likings: Vec::new(),
        mementos: Vec::new(),
        journal: Vec::new(),
    }
}

/// What Home would leave as its result: the homes it was sent, with the visited one arranged.
fn result(visit: &OpenVisit, snapshot: &HomeSnapshot) -> HomeResult {
    let mut state = HomeState::new(&snapshot.colony_key);
    state.households.push(arranged(snapshot.household.keeper));
    HomeResult::new(&visit.seal, OPENED + Duration::minutes(20), "0.1.0", state)
}

fn visit_effect(keeper: TravelerId, arrived: OffsetDateTime, left: OffsetDateTime) -> HomeEffect {
    HomeEffect::HomeVisit {
        household: keeper,
        arrived_at_utc: arrived,
        left_at_utc: left,
    }
}

#[test]
fn a_visit_is_written_whole_and_found_again_after_a_restart() {
    let scratch = Scratch::new("written");
    let files = HouseFiles::new(&scratch.0);
    let save = colony();
    let snapshot = snapshot(&save);
    let away: Vec<CreatureId> = snapshot.residents.iter().map(|r| r.id.0).collect();
    let visit = files.open(&snapshot, &away).unwrap();
    assert!(visit.dir.join(SNAPSHOT_FILE).is_file());
    assert!(visit.dir.join(STATE_FILE).is_file());
    assert_eq!(visit.keeper, sample::keeper(&save));
    assert_eq!(visit.away, away);
    assert_eq!(visit.capabilities, [HomeCapability::VisitRecord]);
    // The same visit, from nothing but the marker.
    assert_eq!(files.open_visit(), Some(visit.clone()));
    // Nothing answered yet.
    assert_eq!(files.answer(&visit), Answer::default());
}

#[test]
fn only_answers_for_exactly_this_visit_count() {
    let scratch = Scratch::new("answers");
    let files = HouseFiles::new(&scratch.0);
    let save = colony();
    let snapshot = snapshot(&save);
    let visit = files.open(&snapshot, &[]).unwrap();
    let mut stranger = visit.seal.clone();
    stranger.state_sha256 = "0".repeat(64);
    let theirs = HomeResult::new(
        &stranger,
        OPENED,
        "0.1.0",
        HomeState::new(&snapshot.colony_key),
    );
    write_document(&visit.dir.join(RESULT_FILE), &theirs).unwrap();
    fs::write(
        visit.dir.join(RECEIPT_FILE),
        b"{\"format\": \"formiga.home.rec",
    )
    .unwrap();
    let answer = files.answer(&visit);
    assert!(answer.result.is_none() && answer.receipt.is_none());
    assert_eq!(answer.problems.len(), 2, "{:?}", answer.problems);
    // The real answers are taken.
    write_document(&visit.dir.join(RESULT_FILE), &result(&visit, &snapshot)).unwrap();
    let receipt = HomeReceipt::new(&visit.seal, OPENED + Duration::minutes(21), "0.1.0", vec![]);
    write_document(&visit.dir.join(RECEIPT_FILE), &receipt).unwrap();
    let answer = files.answer(&visit);
    assert!(answer.result.is_some() && answer.receipt == Some(receipt));
    assert!(answer.problems.is_empty());
}

#[test]
fn home_saying_no_is_heard() {
    let scratch = Scratch::new("refused");
    let files = HouseFiles::new(&scratch.0);
    let snapshot = snapshot(&colony());
    let visit = files.open(&snapshot, &[]).unwrap();
    for (reason, said) in [
        (
            AckRefusal::Busy,
            "Formiga Home 0.1.0 already has a house open.",
        ),
        (
            AckRefusal::UnsupportedVersion { reads: 3 },
            "Formiga Home 0.1.0 reads household version 3, which is too old for this colony. \
             Update Formiga Home to open its houses.",
        ),
        (
            AckRefusal::Invalid,
            "Formiga Home 0.1.0 could not read the house it was given.",
        ),
    ] {
        let ack = HomeAck::refused(&visit.seal, "0.1.0", reason);
        write_document(&visit.dir.join(ACK_FILE), &ack).unwrap();
        let refusal = files.answer(&visit).refusal.expect("a refusal");
        assert_eq!(refusal.reason, reason);
        assert_eq!(refusal_text(&refusal), said);
    }
    let accepted = HomeAck::accepted(&visit.seal, "0.1.0");
    write_document(&visit.dir.join(ACK_FILE), &accepted).unwrap();
    assert_eq!(files.answer(&visit).refusal, None);
}

#[test]
fn a_result_is_kept_only_as_the_contract_allows_and_only_for_this_colony() {
    let scratch = Scratch::new("kept");
    let files = HouseFiles::new(&scratch.0);
    let save = colony();
    let snapshot = snapshot(&save);
    let visit = files.open(&snapshot, &[]).unwrap();
    // Home also proposes a home for a house nobody visited: that is set aside.
    let mut proposed = result(&visit, &snapshot);
    proposed
        .state
        .households
        .push(arranged(snapshot.village[1].keeper));
    let set_aside = files.keep_result(&visit, &proposed).unwrap();
    assert_eq!(set_aside, ["another_household_changed"]);
    let kept = files.kept(&snapshot.colony_key);
    assert_eq!(kept.households.len(), 1);
    assert_eq!(kept.households[0], arranged(snapshot.household.keeper));
    // The next visit is sent the homes as kept; another colony is sent none of them.
    let next = files.open(&self::snapshot(&save), &[]).unwrap();
    let (sent, _) = read_document::<HomeState>(&next.dir.join(STATE_FILE)).unwrap();
    assert_eq!(sent.households, kept.households);
    assert!(files.kept("0123456789abcdef").households.is_empty());
}

#[test]
fn a_result_is_weighed_only_against_the_files_desktop_wrote() {
    let scratch = Scratch::new("tampered");
    let files = HouseFiles::new(&scratch.0);
    let snapshot = snapshot(&colony());
    let visit = files.open(&snapshot, &[]).unwrap();
    let proposed = result(&visit, &snapshot);
    let mut altered = HomeState::new(&snapshot.colony_key);
    altered
        .households
        .push(arranged(snapshot.village[1].keeper));
    write_document(&visit.dir.join(STATE_FILE), &altered).unwrap();
    assert!(files.keep_result(&visit, &proposed).is_err());
    assert!(
        files.kept(&snapshot.colony_key).households.is_empty(),
        "the homes stay as they were"
    );
}

#[test]
fn a_receipt_adds_one_visit_line_and_nothing_else() {
    let scratch = Scratch::new("welcome");
    let files = HouseFiles::new(&scratch.0);
    let snapshot = snapshot(&colony());
    let mut visit = files.open(&snapshot, &[]).unwrap();
    let keeper = snapshot.household.keeper;
    let now = OPENED + Duration::minutes(30);
    let (arrived, left) = (
        OPENED + Duration::minutes(1),
        OPENED + Duration::minutes(29),
    );
    let receipt = |effects| HomeReceipt::new(&visit.seal, now, "0.1.0", effects);
    let welcomed = welcome(
        &visit,
        &receipt(vec![
            visit_effect(keeper, arrived, left),
            visit_effect(keeper, arrived, left),
            HomeEffect::NextDoor {
                household: snapshot.village[1].keeper,
            },
            HomeEffect::Unsupported,
        ]),
        now,
    );
    assert!(welcomed.visited);
    assert_eq!(
        welcomed.set_aside,
        ["home_visit", "next_door", "unsupported"]
    );
    // Another house, a visit outside the visit's own time, or a visit never offered: no line.
    for effect in [
        visit_effect(snapshot.village[1].keeper, arrived, left),
        visit_effect(keeper, OPENED - Duration::days(2), left),
        visit_effect(keeper, arrived, now + Duration::days(1)),
        visit_effect(keeper, left, arrived),
    ] {
        let welcomed = welcome(&visit, &receipt(vec![effect]), now);
        assert!(!welcomed.visited);
        assert_eq!(welcomed.set_aside, ["home_visit"]);
    }
    visit.capabilities.clear();
    let welcomed = welcome(
        &visit,
        &receipt(vec![visit_effect(keeper, arrived, left)]),
        now,
    );
    assert!(!welcomed.visited, "only taken when it was offered");
}

#[test]
fn closing_leaves_only_the_kept_homes_and_calling_home_leaves_home_its_recall() {
    let scratch = Scratch::new("closing");
    let files = HouseFiles::new(&scratch.0);
    let snapshot = snapshot(&colony());
    files.keep(&HomeState::new(&snapshot.colony_key)).unwrap();
    let visit = files.open(&snapshot, &[]).unwrap();
    files.call_home(&visit, RecallReason::OwnerAsked, OPENED);
    assert!(visit.dir.join(RECALL_FILE).is_file(), "Home can see it");
    assert_eq!(files.open_visit(), None);
    // The next visit sweeps the called-home one away, and keeps the homes.
    let next = files.open(&self::snapshot(&colony()), &[]).unwrap();
    assert!(!visit.dir.exists());
    files.close(&next);
    let left: Vec<_> = fs::read_dir(scratch.0.join(HOME_DIRECTORY))
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left, ["state.json"]);
}

#[test]
fn a_visit_is_marked_exactly_as_earlier_desktops_marked_it() {
    /// The marker as Desktop 0.67.1 wrote it, so a visit it left open is finished by this build.
    #[derive(Serialize)]
    struct Earlier {
        format: String,
        session_id: SessionId,
        snapshot_sha256: String,
        state_sha256: String,
        #[serde(with = "time::serde::rfc3339")]
        created_at_utc: OffsetDateTime,
        keeper: CreatureId,
        away: Vec<CreatureId>,
        capabilities: Vec<HomeCapability>,
    }
    let scratch = Scratch::new("earlier");
    let files = HouseFiles::new(&scratch.0);
    let snapshot = snapshot(&colony());
    let away = [snapshot.household.keeper.0];
    let visit = files.open(&snapshot, &away).unwrap();
    let mut earlier = serde_json::to_vec_pretty(&Earlier {
        format: "formiga.desktop.house-visit".to_owned(),
        session_id: visit.seal.session_id.clone(),
        snapshot_sha256: visit.seal.snapshot_sha256.clone(),
        state_sha256: visit.seal.state_sha256.clone(),
        created_at_utc: visit.seal.created_at_utc,
        keeper: visit.keeper,
        away: visit.away.clone(),
        capabilities: visit.capabilities.clone(),
    })
    .unwrap();
    earlier.push(b'\n');
    assert_eq!(
        fs::read(scratch.0.join(HOME_DIRECTORY).join(MARKER_FILE)).unwrap(),
        earlier
    );
}
