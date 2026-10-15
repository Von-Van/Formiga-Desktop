//! A stand-in for Formiga Battle, for developing and testing a session in Battle from Desktop's
//! side.
//!
//! It does what Battle must: reads the roster it was handed in a directory named after its
//! session, says whether it can open it, prints who is on it, waits a while, and exits. It opens
//! no window. Desktop starts it like Battle when it is named by `FORMIGA_BATTLE_PATH`:
//!
//!   cargo build -p formiga-tools --bin formiga-battle-stub
//!   FORMIGA_BATTLE_PATH=target/debug/formiga-battle-stub cargo run -p formiga-desktop
//!
//! `FORMIGA_BATTLE_STUB` takes a comma-separated list of what to do instead of an ordinary
//! session, for exercising every way one can go:
//!
//! - `stay=SECONDS` stays that long after answering before leaving (default 8);
//! - `refuse=version|busy|invalid` refuses the session and exits at once;
//! - `crash` exits abnormally right after answering.

use anyhow::{Context, Result, bail};
use formiga_battle_contract::{
    ACK_FILE, AckRefusal, BATTLE_FORMAT_VERSION, BattleAck, LAUNCH_ARGUMENT, Opened, SessionId,
    open_session,
};
use formiga_travel::write_atomically;
use std::path::PathBuf;
use std::time::Duration;

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-stub");

struct Options {
    stay: f32,
    refuse: Option<AckRefusal>,
    crash: bool,
}

fn options() -> Result<Options> {
    let mut options = Options {
        stay: 8.0,
        refuse: None,
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
