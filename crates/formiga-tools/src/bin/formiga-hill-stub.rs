//! A stand-in for Formiga Hill, for developing and testing the trip from Desktop's side.
//!
//! It does what Hill's host must: reads the snapshot it was handed, checks it, says whether it
//! can host the colony, draws every traveler with the same art Desktop uses, and when the visit is
//! over writes a receipt. It opens no window. Desktop starts it like Hill when it is named by
//! `FORMIGA_HILL_PATH`:
//!
//!   cargo build -p formiga-tools --bin formiga-hill-stub
//!   FORMIGA_HILL_PATH=target/debug/formiga-hill-stub cargo run -p formiga-desktop
//!
//! `FORMIGA_HILL_STUB` takes a comma-separated list of what to do instead of an ordinary visit,
//! for exercising every way a trip can end:
//!
//! - `stay=SECONDS` stays that long before coming home (default 8);
//! - `refuse=version|busy|invalid` refuses the colony and exits at once;
//! - `crash` stays, and then exits abnormally without a receipt;
//! - `silent` stays and then exits without a receipt;
//! - `garbage` writes a receipt that is not a receipt;
//! - `stranger` writes a receipt for some other trip;
//! - `souvenir` brings home every souvenir the snapshot says Desktop keeps, as Hill does for a
//!   colony that has kept them all, and `souvenir=ID` brings home that one, listed or not;
//! - `sheet=PATH` draws every traveler into a PNG at PATH.
//!
//! A recall from Desktop, or the session directory disappearing, ends the visit early without a
//! receipt, as Hill should.

use anyhow::{Context, Result, bail};
use formiga_art::{BodyClip, Canvas, CreatureRenderer, FRAME_SIZE};
use formiga_core::ActionKind;
use formiga_travel::{
    ACK_FILE, AckRefusal, Acknowledgement, Capability, LAUNCH_ARGUMENT, RECALL_FILE, RECEIPT_FILE,
    ReturnEffect, ReturnReceipt, SNAPSHOT_FILE, SessionId, SnapshotSeal, TRAVEL_FORMAT_VERSION,
    TravelError, TravelRole, TravelSnapshot, decode, limits, read_bounded, write_atomically,
    write_document,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use time::OffsetDateTime;

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-stub");

#[derive(Default)]
struct Options {
    stay: Option<f32>,
    refuse: Option<AckRefusal>,
    crash: bool,
    silent: bool,
    garbage: bool,
    stranger: bool,
    /// Which souvenirs to bring home: every one the snapshot lists when empty.
    souvenir: Option<String>,
    sheet: Option<PathBuf>,
}

fn options() -> Result<Options> {
    let mut options = Options::default();
    let text = std::env::var("FORMIGA_HILL_STUB").unwrap_or_default();
    for item in text
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let (key, value) = item.split_once('=').unwrap_or((item, ""));
        match key {
            "stay" => options.stay = Some(value.parse().context("stay=SECONDS")?),
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
            "souvenir" => options.souvenir = Some(value.to_owned()),
            "sheet" => options.sheet = Some(PathBuf::from(value)),
            other => bail!("unknown FORMIGA_HILL_STUB option {other:?}"),
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
    bail!("usage: formiga-hill-stub {LAUNCH_ARGUMENT} SESSION_DIRECTORY")
}

fn main() -> Result<()> {
    let options = options()?;
    let dir = session_dir()?;
    let started = OffsetDateTime::now_utc();
    // The session is named by its directory; nothing else in the path is trusted.
    let session = dir
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(SessionId::parse)
        .context("the session directory is not named for a session")?;
    let bytes = read_bounded(&dir.join(SNAPSHOT_FILE), limits::MAX_SNAPSHOT_BYTES)?;
    let snapshot: Result<TravelSnapshot, TravelError> = decode(&bytes);
    let seal = |created_at_utc| SnapshotSeal {
        session_id: session.clone(),
        snapshot_sha256: formiga_travel::sha256_hex(&bytes),
        created_at_utc,
    };
    let snapshot = match snapshot {
        Ok(snapshot) if snapshot.session_id == session => snapshot,
        Ok(_) => return refuse(&dir, &seal(started), AckRefusal::Invalid),
        Err(TravelError::UnsupportedVersion { .. }) => {
            return refuse(
                &dir,
                &seal(started),
                AckRefusal::UnsupportedVersion {
                    reads: TRAVEL_FORMAT_VERSION,
                },
            );
        }
        Err(error) => {
            eprintln!("formiga-hill-stub: the snapshot does not check out: {error}");
            return refuse(&dir, &seal(started), AckRefusal::Invalid);
        }
    };
    let seal = SnapshotSeal::of(&snapshot, &bytes);
    if let Some(refusal) = options.refuse {
        return refuse(&dir, &seal, refusal);
    }
    describe(&snapshot)?;
    if let Some(path) = &options.sheet {
        sheet(&snapshot, path)?;
        println!("drew the travelers into {}", path.display());
    }
    write_document(
        &dir.join(ACK_FILE),
        &Acknowledgement::accepted(&seal, VERSION),
    )?;
    let stay = Duration::from_secs_f32(options.stay.unwrap_or(8.0).max(0.0));
    let since = Instant::now();
    while since.elapsed() < stay {
        // Called home, or the trip cleared away: either way the visit is over.
        if dir.join(RECALL_FILE).exists() || !dir.join(SNAPSHOT_FILE).exists() {
            println!("Desktop called the colony home; closing without a receipt");
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if options.crash {
        eprintln!("formiga-hill-stub: crashing on purpose");
        std::process::exit(101);
    }
    if options.silent {
        println!("leaving without a receipt");
        return Ok(());
    }
    let path = dir.join(RECEIPT_FILE);
    if options.garbage {
        write_atomically(
            &path,
            b"{\"format\": \"formiga.travel.receipt\", \"version\": 1, \"eff",
        )?;
        return Ok(());
    }
    let mut effects = vec![ReturnEffect::Visit {
        arrived_at_utc: started,
        left_at_utc: OffsetDateTime::now_utc(),
    }];
    match options.souvenir.as_deref() {
        // Every one Desktop keeps, and only while it offers to keep them, as Hill does.
        Some("") if snapshot.offers(Capability::Souvenirs) => {
            effects.extend(
                snapshot
                    .accepts_souvenirs
                    .iter()
                    .map(|id| ReturnEffect::Souvenir { id: id.clone() }),
            );
        }
        Some("") | None => {}
        Some(id) => effects.push(ReturnEffect::Souvenir { id: id.to_owned() }),
    }
    let mut receipt = ReturnReceipt::new(&seal, OffsetDateTime::now_utc(), VERSION, effects);
    if options.stranger {
        receipt.session_id = SessionId::generate()?;
    }
    write_document(&path, &receipt)?;
    println!("the colony is on its way home");
    Ok(())
}

fn refuse(dir: &Path, seal: &SnapshotSeal, refusal: AckRefusal) -> Result<()> {
    eprintln!("formiga-hill-stub: refusing the colony: {refusal:?}");
    write_document(
        &dir.join(ACK_FILE),
        &Acknowledgement::refused(seal, VERSION, refusal),
    )?;
    std::process::exit(2);
}

/// Who arrived, as Hill would greet them.
fn describe(snapshot: &TravelSnapshot) -> Result<()> {
    println!(
        "{} travelers from Formiga Desktop {} (travel version {})",
        snapshot.travelers.len(),
        snapshot.desktop_version,
        snapshot.version
    );
    for traveler in &snapshot.travelers {
        let creature = traveler.to_creature()?;
        let role = match traveler.role {
            TravelRole::Adult => "adult".to_owned(),
            TravelRole::Mini { parent_id } => format!(
                "little one of {}",
                snapshot
                    .traveler(parent_id)
                    .map_or("someone", |parent| parent.name.as_str())
            ),
        };
        println!(
            "  {:<24} {:<22} {} · {}% stature · {} habit(s){}",
            traveler.name,
            role,
            traveler.character.phrase,
            traveler.stature_percent,
            traveler.habits.len(),
            traveler
                .accessory
                .map(|accessory| format!(" · wearing {}", accessory.to_accessory().label()))
                .unwrap_or_default()
        );
        let frame = CreatureRenderer::render_body_frame(
            &creature.appearance,
            BodyClip::Action(ActionKind::Idle),
            0,
            false,
        );
        if frame.canvas.alpha_bounds().is_none() {
            bail!("{} could not be drawn", traveler.name);
        }
    }
    println!("{} friendships", snapshot.relationships.len());
    Ok(())
}

/// Every traveler standing in a row, dressed, at 4x.
fn sheet(snapshot: &TravelSnapshot, path: &Path) -> Result<()> {
    const SCALE: u32 = 4;
    let width = FRAME_SIZE * SCALE * snapshot.travelers.len() as u32;
    let height = FRAME_SIZE * SCALE;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[234, 230, 222, 255]);
    }
    for (index, traveler) in snapshot.travelers.iter().enumerate() {
        let creature = traveler.to_creature()?;
        let dress = traveler.accessory.map(|accessory| accessory.to_art());
        let canvas: Canvas = CreatureRenderer::render_dressed_frame(
            &creature.appearance,
            dress,
            ActionKind::Idle,
            0,
            true,
        );
        for y in 0..FRAME_SIZE {
            for x in 0..FRAME_SIZE {
                let pixel = canvas.get(x as i32, y as i32);
                if pixel.a == 0 {
                    continue;
                }
                for oy in 0..SCALE {
                    for ox in 0..SCALE {
                        let px = index as u32 * FRAME_SIZE * SCALE + x * SCALE + ox;
                        let py = y * SCALE + oy;
                        let at = ((py * width + px) * 4) as usize;
                        let alpha = f32::from(pixel.a) / 255.0;
                        for (channel, value) in [pixel.r, pixel.g, pixel.b].into_iter().enumerate()
                        {
                            pixels[at + channel] = (f32::from(value) * alpha
                                + f32::from(pixels[at + channel]) * (1.0 - alpha))
                                as u8;
                        }
                    }
                }
            }
        }
    }
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    Ok(())
}
