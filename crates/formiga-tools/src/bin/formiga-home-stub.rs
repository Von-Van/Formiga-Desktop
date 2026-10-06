//! A stand-in for Formiga Home, for developing and testing a visit to a house from Desktop's side.
//!
//! It does what Home must: reads the snapshot and the homes it was handed, checks them against
//! each other, says whether it can open the house, and when the visit is over writes back the
//! homes as it would have them and a receipt. It opens no window. Desktop starts it like Home when
//! it is named by `FORMIGA_HOME_PATH`:
//!
//!   cargo build -p formiga-tools --bin formiga-home-stub
//!   FORMIGA_HOME_PATH=target/debug/formiga-home-stub cargo run -p formiga-desktop
//!
//! `FORMIGA_HOME_STUB` takes a comma-separated list of what to do instead of an ordinary visit,
//! for exercising every way a visit can end:
//!
//! - `stay=SECONDS` stays that long before leaving (default 8);
//! - `arrange` sets a shelf in the house, with something of the colony's on it;
//! - `refuse=version|busy|invalid` refuses the house and exits at once;
//! - `crash` stays, and then exits abnormally without a receipt, after arranging if asked;
//! - `silent` stays and then exits without a receipt;
//! - `garbage` writes a receipt that is not a receipt;
//! - `stranger` writes a receipt for some other visit.
//!
//! A recall from Desktop, or the snapshot disappearing, ends the visit early with nothing more
//! written, as Home should.

use anyhow::{Context, Result, bail};
use formiga_home_contract::{
    ACK_FILE, AckRefusal, CatalogId, DisplayMode, HOME_FORMAT_VERSION, HomeAck, HomeCapability,
    HomeEffect, HomeError, HomeReceipt, HomeResult, HomeSnapshot, HomeState, HouseholdHome,
    LAUNCH_ARGUMENT, PlacedDisplay, PlacedPiece, RECALL_FILE, RECEIPT_FILE, RESULT_FILE,
    RoomLayout, SNAPSHOT_FILE, STATE_FILE, SessionId, SessionSeal, Spot, decode, limits,
    read_bounded, write_document,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use time::OffsetDateTime;

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-stub");

#[derive(Default)]
struct Options {
    stay: Option<f32>,
    arrange: bool,
    refuse: Option<AckRefusal>,
    crash: bool,
    silent: bool,
    garbage: bool,
    stranger: bool,
}

fn options() -> Result<Options> {
    let mut options = Options::default();
    let text = std::env::var("FORMIGA_HOME_STUB").unwrap_or_default();
    for item in text
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let (key, value) = item.split_once('=').unwrap_or((item, ""));
        match key {
            "stay" => options.stay = Some(value.parse().context("stay=SECONDS")?),
            "arrange" => options.arrange = true,
            "refuse" => {
                options.refuse = Some(match value {
                    "version" => AckRefusal::UnsupportedVersion { reads: 0 },
                    "busy" => AckRefusal::Busy,
                    _ => AckRefusal::Invalid,
                })
            }
            "crash" => options.crash = true,
            "silent" => options.silent = true,
            "garbage" => options.garbage = true,
            "stranger" => options.stranger = true,
            other => bail!("unknown FORMIGA_HOME_STUB option {other:?}"),
        }
    }
    Ok(options)
}

fn session_dir() -> Result<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == LAUNCH_ARGUMENT {
            return args
                .next()
                .map(PathBuf::from)
                .context("a session directory");
        }
    }
    bail!("usage: formiga-home-stub {LAUNCH_ARGUMENT} SESSION_DIRECTORY")
}

fn main() -> Result<()> {
    let options = options()?;
    let dir = session_dir()?;
    let opened = OffsetDateTime::now_utc();
    // The session is named by its directory; nothing else in the path is trusted.
    let session = dir
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(SessionId::parse)
        .context("the session directory is not named for a session")?;
    let snapshot_bytes = read_bounded(&dir.join(SNAPSHOT_FILE), limits::MAX_SNAPSHOT_BYTES)?;
    let state_bytes = read_bounded(&dir.join(STATE_FILE), limits::MAX_STATE_BYTES)?;
    let seal_of =
        |snapshot: &HomeSnapshot| SessionSeal::of(snapshot, &snapshot_bytes, &state_bytes);
    let read: Result<(HomeSnapshot, HomeState), HomeError> =
        decode::<HomeSnapshot>(&snapshot_bytes)
            .and_then(|snapshot| Ok((snapshot, decode::<HomeState>(&state_bytes)?)));
    let (snapshot, state) = match read {
        Ok((snapshot, state)) if snapshot.session_id == session => (snapshot, state),
        Ok((snapshot, _)) => return refuse(&dir, &seal_of(&snapshot), AckRefusal::Invalid),
        Err(HomeError::UnsupportedVersion { .. }) => {
            eprintln!("formiga-home-stub: the snapshot needs a newer Home");
            let reads = HOME_FORMAT_VERSION;
            let refusal = AckRefusal::UnsupportedVersion { reads };
            return refuse_unread(&dir, &session, &snapshot_bytes, &state_bytes, refusal);
        }
        Err(error) => {
            eprintln!("formiga-home-stub: the visit does not check out: {error}");
            let refusal = AckRefusal::Invalid;
            return refuse_unread(&dir, &session, &snapshot_bytes, &state_bytes, refusal);
        }
    };
    let seal = seal_of(&snapshot);
    if let Some(refusal) = options.refuse {
        return refuse(&dir, &seal, refusal);
    }
    describe(&snapshot);
    write_document(&dir.join(ACK_FILE), &HomeAck::accepted(&seal, VERSION))?;
    let mut homes = state.settled_for(&snapshot);
    if options.arrange {
        arrange(&snapshot, &mut homes);
        write_document(
            &dir.join(RESULT_FILE),
            &HomeResult::new(&seal, OffsetDateTime::now_utc(), VERSION, homes.clone()),
        )?;
        println!("set a shelf out in the house");
    }
    let stay = Duration::from_secs_f32(options.stay.unwrap_or(8.0).max(0.0));
    let since = Instant::now();
    while since.elapsed() < stay {
        // Called back out, or the visit cleared away: either way the house closes.
        if dir.join(RECALL_FILE).exists() || !dir.join(SNAPSHOT_FILE).exists() {
            println!("Desktop called the household back out; closing without a word");
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if options.crash {
        eprintln!("formiga-home-stub: crashing on purpose");
        std::process::exit(101);
    }
    if options.silent {
        println!("leaving without a receipt");
        return Ok(());
    }
    let path = dir.join(RECEIPT_FILE);
    if options.garbage {
        formiga_travel::write_atomically(
            &path,
            b"{\"format\": \"formiga.home.receipt\", \"version\": 1, \"eff",
        )?;
        return Ok(());
    }
    // The last word on the homes, once more on leaving, as Home writes it.
    write_document(
        &dir.join(RESULT_FILE),
        &HomeResult::new(&seal, OffsetDateTime::now_utc(), VERSION, homes),
    )?;
    let mut effects = Vec::new();
    if snapshot.offers(HomeCapability::VisitRecord) {
        effects.push(HomeEffect::HomeVisit {
            household: snapshot.household.keeper,
            arrived_at_utc: opened,
            left_at_utc: OffsetDateTime::now_utc(),
        });
    }
    let mut receipt = HomeReceipt::new(&seal, OffsetDateTime::now_utc(), VERSION, effects);
    if options.stranger {
        receipt.session_id = SessionId::generate()?;
    }
    write_document(&path, &receipt)?;
    println!("the household is on its way back out");
    Ok(())
}

/// A shelf against the far wall of the house's first room, with the first thing of the colony's
/// that stands on a shelf set on it.
fn arrange(snapshot: &HomeSnapshot, homes: &mut HomeState) {
    let keeper = snapshot.household.keeper;
    let on_the_shelf = snapshot
        .inventory
        .iter()
        .find(|item| item.allows(DisplayMode::SurfaceSmall))
        .map(|item| PlacedDisplay {
            item: item.id.clone(),
            spot: Spot::On { piece: 1, slot: 0 },
        });
    let home = HouseholdHome {
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
            displays: on_the_shelf.into_iter().collect(),
            plan: None,
            kind: None,
            doors: Vec::new(),
        }],
        likings: Vec::new(),
        mementos: Vec::new(),
        journal: Vec::new(),
    };
    homes.households.retain(|home| home.keeper != keeper);
    homes.households.push(home);
}

fn refuse(dir: &Path, seal: &SessionSeal, refusal: AckRefusal) -> Result<()> {
    eprintln!("formiga-home-stub: refusing the house: {refusal:?}");
    write_document(
        &dir.join(ACK_FILE),
        &HomeAck::refused(seal, VERSION, refusal),
    )?;
    std::process::exit(2);
}

/// A refusal for a visit that could not be read at all: it names the bytes it was given, which is
/// all a refusal has to answer.
fn refuse_unread(
    dir: &Path,
    session: &SessionId,
    snapshot_bytes: &[u8],
    state_bytes: &[u8],
    refusal: AckRefusal,
) -> Result<()> {
    let seal = SessionSeal {
        session_id: session.clone(),
        snapshot_sha256: formiga_home_contract::sha256_hex(snapshot_bytes),
        state_sha256: formiga_home_contract::sha256_hex(state_bytes),
        created_at_utc: OffsetDateTime::now_utc(),
    };
    refuse(dir, &seal, refusal)
}

/// Who came in, as Home would greet them.
fn describe(snapshot: &HomeSnapshot) {
    let keeper = snapshot
        .residents
        .first()
        .map_or("someone", |resident| resident.name.as_str());
    println!(
        "{keeper}'s house from Formiga Desktop {} (household version {}): {} at home, {} come \
         round, {} things to show",
        snapshot.desktop_version,
        snapshot.version,
        snapshot.residents.len(),
        snapshot.visitors.len(),
        snapshot.inventory.len()
    );
}
