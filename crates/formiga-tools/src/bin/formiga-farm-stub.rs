//! A stand-in for Formiga Farm, for developing and testing a session in Farm from Desktop's side.
//!
//! It does what Farm must: reads the snapshot it was handed, says whether it can open it, and
//! proposes one design as the owner would by pressing Apply: a sculpted form on one of the newer
//! plans, in the face the companion (or the stand-in for a new one) already wears. It prints
//! Desktop's verdict, waits a while, and exits. It opens no window. Desktop starts it like Farm
//! when it is named by `FORMIGA_FARM_PATH`:
//!
//!   cargo build -p formiga-tools --bin formiga-farm-stub
//!   FORMIGA_FARM_PATH=target/debug/formiga-farm-stub cargo run -p formiga-desktop
//!
//! `FORMIGA_FARM_STUB` takes a comma-separated list of what to do instead of an ordinary session,
//! for exercising every way one can go:
//!
//! - `stay=SECONDS` stays that long after its verdict before leaving (default 8);
//! - `after=SECONDS` waits that long before proposing (default 2);
//! - `plan=PLAN` proposes that plan, by its name in a design: `floater`, `percher`,
//!   `compact_quadruped` and the rest (default `floater`);
//! - `refuse=version|busy|invalid` refuses the session and exits at once;
//! - `silent` proposes nothing;
//! - `stale` proposes from a look the companion no longer has;
//! - `garbage` writes a proposal that is not a proposal;
//! - `stranger` writes a proposal for some other session;
//! - `crash` exits abnormally right after proposing, before any verdict.
//!
//! A recall from Desktop, or the snapshot disappearing, ends the session with nothing more
//! written, as Farm should.

use anyhow::{Context, Result, bail};
use formiga_core::forms::{Design, Form, Plan, Sculpt};
use formiga_farm_contract::{
    ACK_FILE, AckRefusal, FARM_FORMAT_VERSION, FarmAck, FarmError, FarmProposal, FarmSnapshot,
    FarmVerdict, LAUNCH_ARGUMENT, Lineage, PROPOSAL_FILE, ProposalKind, RECALL_FILE, SNAPSHOT_FILE,
    SessionId, SessionSeal, VERDICT_FILE, decode, limits, read_bounded, read_document,
    write_document,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use time::OffsetDateTime;

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-stub");

struct Options {
    stay: f32,
    after: f32,
    plan: Plan,
    refuse: Option<AckRefusal>,
    silent: bool,
    stale: bool,
    garbage: bool,
    stranger: bool,
    crash: bool,
}

fn options() -> Result<Options> {
    let mut options = Options {
        stay: 8.0,
        after: 2.0,
        plan: Plan::Floater,
        refuse: None,
        silent: false,
        stale: false,
        garbage: false,
        stranger: false,
        crash: false,
    };
    let text = std::env::var("FORMIGA_FARM_STUB").unwrap_or_default();
    for item in text
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let (key, value) = item.split_once('=').unwrap_or((item, ""));
        match key {
            "stay" => options.stay = value.parse().context("stay=SECONDS")?,
            "after" => options.after = value.parse().context("after=SECONDS")?,
            "plan" => {
                options.plan = serde_json::from_value(serde_json::Value::String(value.into()))
                    .with_context(|| format!("no plan is called {value:?}"))?;
            }
            "refuse" => {
                options.refuse = Some(match value {
                    "version" => AckRefusal::UnsupportedVersion { reads: 0 },
                    "busy" => AckRefusal::Busy,
                    _ => AckRefusal::Invalid,
                })
            }
            "silent" => options.silent = true,
            "stale" => options.stale = true,
            "garbage" => options.garbage = true,
            "stranger" => options.stranger = true,
            "crash" => options.crash = true,
            other => bail!("unknown FORMIGA_FARM_STUB option {other:?}"),
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
    bail!("usage: formiga-farm-stub {LAUNCH_ARGUMENT} SESSION_DIRECTORY")
}

fn main() -> Result<()> {
    let options = options()?;
    let dir = session_dir()?;
    // The session is named by its directory; nothing else in the path is trusted.
    let session = dir
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(SessionId::parse)
        .context("the session directory is not named for a session")?;
    let bytes = read_bounded(&dir.join(SNAPSHOT_FILE), limits::MAX_SNAPSHOT_BYTES)?;
    let snapshot = match decode::<FarmSnapshot>(&bytes) {
        Ok(snapshot) if snapshot.session_id == session => snapshot,
        Ok(snapshot) => {
            return refuse(
                &dir,
                &SessionSeal::of(&snapshot, &bytes),
                AckRefusal::Invalid,
            );
        }
        Err(error) => {
            eprintln!("formiga-farm-stub: the session does not check out: {error}");
            let refusal = match error {
                FarmError::UnsupportedVersion { .. } => AckRefusal::UnsupportedVersion {
                    reads: FARM_FORMAT_VERSION,
                },
                _ => AckRefusal::Invalid,
            };
            let seal = SessionSeal {
                session_id: session,
                snapshot_sha256: formiga_farm_contract::sha256_hex(&bytes),
            };
            return refuse(&dir, &seal, refusal);
        }
    };
    let seal = SessionSeal::of(&snapshot, &bytes);
    if let Some(refusal) = options.refuse {
        return refuse(&dir, &seal, refusal);
    }
    describe(&snapshot);
    write_document(&dir.join(ACK_FILE), &FarmAck::accepted(&seal, VERSION))?;
    if !wait(&dir, options.after) {
        return Ok(());
    }
    // Only a proposal for this session can be answered: anything else waits for nothing.
    if !options.silent && propose(&dir, &snapshot, &seal, &options)? {
        if options.crash {
            eprintln!("formiga-farm-stub: crashing on purpose");
            std::process::exit(101);
        }
        let since = Instant::now();
        loop {
            if let Ok((verdict, _)) = read_document::<FarmVerdict>(&dir.join(VERDICT_FILE))
                && verdict.answers(&seal, 1)
            {
                println!("Desktop answered: {:?}", verdict.verdict);
                break;
            }
            if since.elapsed() > Duration::from_secs(10) {
                println!("Desktop has not answered");
                break;
            }
            if !wait(&dir, 0.2) {
                return Ok(());
            }
        }
    }
    wait(&dir, options.stay);
    println!("closing");
    Ok(())
}

/// Wait `seconds`, unless the session is called off first. Returns whether it is still on.
fn wait(dir: &Path, seconds: f32) -> bool {
    let until = Instant::now() + Duration::from_secs_f32(seconds.max(0.0));
    loop {
        if dir.join(RECALL_FILE).exists() || !dir.join(SNAPSHOT_FILE).exists() {
            println!("Desktop ended the session; closing without a word");
            return false;
        }
        if Instant::now() >= until {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// One design, as the owner would apply it: the plan asked for, in the face already worn.
/// Returns whether it was written for this session, to be answered.
fn propose(
    dir: &Path,
    snapshot: &FarmSnapshot,
    seal: &SessionSeal,
    options: &Options,
) -> Result<bool> {
    let path = dir.join(PROPOSAL_FILE);
    if options.garbage {
        formiga_travel::write_atomically(
            &path,
            b"{\"format\": \"formiga.farm.proposal\", \"version\": 1, \"des",
        )?;
        println!("proposed something that is not a proposal");
        return Ok(false);
    }
    let base = snapshot.base_genome()?;
    let design = Design {
        form: Form::Sculpted {
            sculpt: Sculpt::starter(options.plan),
        },
        face: base.face,
    };
    let kind = match snapshot.creature() {
        Some(creature) => ProposalKind::EditExisting {
            target: creature.id,
            expected_revision: if options.stale {
                "0".repeat(32)
            } else {
                creature.revision.clone()
            },
        },
        None => ProposalKind::CreateNew,
    };
    let mut seal = seal.clone();
    if options.stranger {
        seal.session_id = SessionId::generate()?;
    }
    let lineage = Lineage::new(None, Some("Stub"));
    let proposal = FarmProposal::new(
        &seal,
        1,
        OffsetDateTime::now_utc(),
        VERSION,
        kind,
        design,
        lineage,
    );
    write_document(&path, &proposal)?;
    println!("proposed a {}", options.plan.label().to_lowercase());
    Ok(!options.stranger)
}

fn refuse(dir: &Path, seal: &SessionSeal, refusal: AckRefusal) -> Result<()> {
    eprintln!("formiga-farm-stub: refusing the session: {refusal:?}");
    write_document(
        &dir.join(ACK_FILE),
        &FarmAck::refused(seal, VERSION, refusal),
    )?;
    std::process::exit(2);
}

/// What was opened, as Farm would title its window.
fn describe(snapshot: &FarmSnapshot) {
    match snapshot.creature() {
        Some(creature) => println!(
            "reshaping {} from Formiga Desktop {} (design version {})",
            creature.name, snapshot.desktop_version, snapshot.version
        ),
        None => println!(
            "drawing a new companion from Formiga Desktop {} (design version {})",
            snapshot.desktop_version, snapshot.version
        ),
    }
}
