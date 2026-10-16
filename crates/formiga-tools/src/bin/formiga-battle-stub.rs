//! A stand-in for Formiga Battle, for developing and testing a session in Battle from Desktop's
//! side.
//!
//! It does what Battle must: reads the roster it was handed in a directory named after its
//! session, says whether it can open it, prints who is on it, and plays one match against the
//! visitors as if the owner had won it: the result it writes wins the Backyard Cup, with a line
//! for the journal and the cup's trophy and medal. Then it waits a while and exits. It opens no
//! window. Desktop starts it like Battle when it is named by `FORMIGA_BATTLE_PATH`:
//!
//!   cargo build -p formiga-tools --bin formiga-battle-stub
//!   FORMIGA_BATTLE_PATH=target/debug/formiga-battle-stub cargo run -p formiga-desktop
//!
//! `FORMIGA_BATTLE_STUB` takes a comma-separated list of what to do instead of an ordinary
//! session, for exercising every way one can go:
//!
//! - `stay=SECONDS` stays that long after answering before leaving (default 8);
//! - `silent` plays no match, and writes no result;
//! - `refuse=version|busy|invalid` refuses the session and exits at once;
//! - `crash` exits abnormally right after answering.

use anyhow::{Context, Result, bail};
use formiga_battle_contract::{
    ACK_FILE, AckRefusal, BATTLE_FORMAT_VERSION, BattleAck, BattleResult, BattleSnapshot,
    LAUNCH_ARGUMENT, MatchResult, Mode, Opened, Rewards, SNAPSHOT_FILE, SessionId, open_session,
};
use formiga_travel::write_atomically;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-stub");

struct Options {
    stay: f32,
    refuse: Option<AckRefusal>,
    silent: bool,
    crash: bool,
}

fn options() -> Result<Options> {
    let mut options = Options {
        stay: 8.0,
        refuse: None,
        silent: false,
        crash: false,
    };
    let text = std::env::var("FORMIGA_BATTLE_STUB").unwrap_or_default();
    for item in text
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let (key, value) = item.split_once('=').unwrap_or((item, ""));
        match key {
            "stay" => options.stay = value.parse().context("stay=SECONDS")?,
            "refuse" => {
                options.refuse = Some(match value {
                    "version" => AckRefusal::UnsupportedVersion {
                        reads: BATTLE_FORMAT_VERSION - 1,
                    },
                    "busy" => AckRefusal::Busy,
                    _ => AckRefusal::Invalid,
                })
            }
            "silent" => options.silent = true,
            "crash" => options.crash = true,
            other => bail!("unknown FORMIGA_BATTLE_STUB option {other:?}"),
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
    bail!("usage: formiga-battle-stub {LAUNCH_ARGUMENT} SESSION_DIRECTORY")
}

fn main() -> Result<()> {
    let options = options()?;
    let dir = session_dir()?;
    if let Some(refusal) = options.refuse {
        // The session is named by its directory; nothing else in the path is trusted.
        let session = dir
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(SessionId::parse)
            .context("the session directory is not named for a session")?;
        eprintln!("formiga-battle-stub: refusing the session: {refusal:?}");
        let ack = BattleAck::new(session, VERSION, Some(refusal));
        write_atomically(&dir.join(ACK_FILE), &ack.encode()?)?;
        std::process::exit(2);
    }
    // Read and answered exactly as Battle does.
    match open_session(&dir, VERSION)? {
        Opened::Roster(roster) => {
            let names: Vec<_> = roster
                .contenders
                .iter()
                .map(|c| c.traveler.name.as_str())
                .collect();
            println!(
                "{} companions from Formiga Desktop {} (battle version {}): {}",
                names.len(),
                roster.desktop_version,
                roster.version,
                names.join(", ")
            );
            if !options.silent {
                win_the_cup(&dir, &roster)?;
            }
        }
        Opened::Refused(refusal, error) => {
            eprintln!("formiga-battle-stub: the session does not check out ({refusal:?}): {error}");
            std::process::exit(2);
        }
    }
    if options.crash {
        eprintln!("formiga-battle-stub: crashing on purpose");
        std::process::exit(101);
    }
    std::thread::sleep(Duration::from_secs_f32(options.stay.max(0.0)));
    println!("closing");
    Ok(())
}

/// One match against the visitors, won by the first three on the roster, and the third win that
/// takes the Backyard Cup: what Battle writes for it, whole, as it does after every match. The
/// match is named after the session, so the same session always earns the same match.
fn win_the_cup(dir: &Path, roster: &BattleSnapshot) -> Result<()> {
    let bytes = std::fs::read(dir.join(SNAPSHOT_FILE))?;
    let mut result = BattleResult::new(
        roster.session_id.clone(),
        formiga_expansion_rulebook::sha256_hex(&bytes),
        VERSION,
    );
    let team: Vec<&str> = roster
        .contenders
        .iter()
        .take(3)
        .map(|c| c.traveler.name.as_str())
        .collect();
    let team = match team.as_slice() {
        [one] => (*one).to_owned(),
        [first @ .., last] => format!("{} and {last}", first.join(", ")),
        [] => "The colony".to_owned(),
    };
    result.matches.push(MatchResult {
        id: roster.session_id.as_str().to_owned(),
        mode: Mode::Ai,
        rewards: Rewards {
            cup_win: Some("cup.backyard".to_owned()),
            unlocks: vec!["cup.woodland".to_owned()],
            journal: vec![formiga_expansion_rulebook::sanitize_text(
                &format!("{team} won the Backyard Cup against the visitors."),
                120,
            )],
            keepsakes: vec![
                "battle.trophy.backyard_cup".to_owned(),
                "battle.medal.backyard_cup".to_owned(),
            ],
            ..Rewards::default()
        },
    });
    result.write(dir)?;
    println!("won the Backyard Cup");
    Ok(())
}
