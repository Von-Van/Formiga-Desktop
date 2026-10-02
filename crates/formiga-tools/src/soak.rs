//! Long simulated lives for many randomized colonies, checking what must always hold.
//!
//! Each colony gets its own seed, size, preferences and desktop, and lives for some days of
//! sessions and gaps: twenty ticks a second while Formiga would be running, and the clock jumping
//! ahead while it would not — across midnight, across a weekend. Between sessions displays are
//! plugged in, unplugged and rescaled, preferences are toggled, the village is rearranged and the
//! changes taken back, companions come and go, and the colony is written to disk and read back,
//! the way a relaunch reads it. During a session windows open, move and close, the cursor
//! wanders, and companions are picked up, carried, tossed, petted and offered things.
//!
//! At every checkpoint the colony must hold nothing `formiga_core::violations` names, every
//! companion must stand somewhere a display reaches, and a colony read back from disk must be the
//! colony that was written. A panic, a broken invariant or a reload that comes back different
//! stops that colony; its number, the run's seed and its last good save are written out, so
//! `--seed S --only N` replays exactly that life.
//!
//! With `--damage N`, each colony's last save is also damaged N ways — a value of the wrong kind,
//! a list repeated, a field removed — and each damaged file must be refused or come back valid and
//! open and run.
//!
//!   cargo run --release -p formiga-tools -- soak [--colonies N] [--days N] [--seed N]
//!       [--threads N] [--damage N] [--only N] [--out DIR]

use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, bail};
use formiga_core::{
    BehaviorPreset, ColonyEdit, CursorSnapshot, DesktopRect, DesktopSnapshot, DesktopWindow,
    DisplayKey, GardenKind, HabitatPreset, HangoutKind, HomeCorner, MonitorInfo, Point,
    RoamingLeaning, RoutineSchedule, SaveFile, SaveStore, ScheduledTransition, ValidatedSave,
    VillagePalette, World, WorldCommand, violations,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use time::{Duration, OffsetDateTime, macros::datetime};

/// One tick of simulated time, the cadence the desktop runs at whenever anything moves.
const DT: f32 = 0.05;
/// Ticks between the light checkpoints inside a session: once a simulated minute.
const CHECK_EVERY: u32 = 1_200;
/// How far outside every display a companion may be and still count as somewhere: a frame and
/// a half, for one halfway through a hop off the edge of a window.
const REACH: f32 = 96.0;
/// How long a colony has to settle after its displays change before anybody still out of reach
/// counts as lost: ten seconds.
const SETTLE_TICKS: u32 = 200;

struct Options {
    colonies: usize,
    days: u32,
    seed: u64,
    threads: usize,
    damage: u32,
    only: Option<usize>,
    out: PathBuf,
}

fn options(args: &[String]) -> anyhow::Result<Options> {
    let mut options = Options {
        colonies: 24,
        days: 2,
        seed: 0x0663,
        // Half the machine unless asked for more: a soak run left to take every core makes the
        // rest of a working computer crawl. The scheduled workflow asks for the whole runner.
        threads: std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).max(1)),
        damage: 0,
        only: None,
        out: std::env::temp_dir().join("formiga-soak"),
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .with_context(|| format!("{arg} needs a value"))
                .cloned()
        };
        match arg.as_str() {
            "--colonies" => options.colonies = value()?.parse()?,
            "--days" => options.days = value()?.parse()?,
            "--seed" => options.seed = value()?.parse()?,
            "--threads" => options.threads = value()?.parse::<usize>()?.max(1),
            "--damage" => options.damage = value()?.parse()?,
            "--only" => options.only = Some(value()?.parse()?),
            "--out" => options.out = PathBuf::from(value()?),
            "-h" | "--help" => bail!(
                "usage: formiga-tools soak [--colonies N] [--days N] [--seed N] [--threads N] \
                 [--damage N] [--only N] [--out DIR]"
            ),
            other => bail!("unknown soak option {other}"),
        }
    }
    Ok(options)
}

/// What one colony's life came to.
struct Lived {
    ticks: u64,
    reloads: u32,
    edits: u32,
    undos: u32,
    damaged: u32,
    refused: u32,
    /// Weekly routines that came round while the colony was away.
    transitions: u32,
    /// The most ticks a colony took to bring everybody back within reach after a change.
    slowest_settle: u32,
}

/// Why a colony's life was stopped, and the last save it got to.
struct Failure {
    index: usize,
    what: String,
    save: Option<SaveFile>,
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let options = options(args)?;
    std::fs::create_dir_all(&options.out)?;
    let indices: Vec<usize> = match options.only {
        Some(index) => vec![index],
        None => (0..options.colonies).collect(),
    };
    println!(
        "# soak: {} colonies x {} days, seed {}, {} threads",
        indices.len(),
        options.days,
        options.seed,
        options.threads
    );
    let started = std::time::Instant::now();
    let next = AtomicUsize::new(0);
    let failures = Mutex::new(Vec::new());
    let totals = Mutex::new((0u64, 0u32, 0u32, 0u32, 0u32, 0u32, 0u32, 0u32));
    // A panic is reported as a failure of the colony it happened in, not printed as it happens.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    std::thread::scope(|scope| {
        for _ in 0..options.threads.min(indices.len()) {
            scope.spawn(|| {
                loop {
                    let position = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&index) = indices.get(position) else {
                        break;
                    };
                    let scratch = options.out.join(format!("colony-{index}"));
                    let last_save = Mutex::new(None);
                    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                        live(index, &options, &scratch, &last_save)
                    }));
                    let _ = std::fs::remove_dir_all(&scratch);
                    let failure = match result {
                        Ok(Ok(lived)) => {
                            let mut totals = totals.lock().unwrap();
                            totals.0 += lived.ticks;
                            totals.1 += lived.reloads;
                            totals.2 += lived.edits;
                            totals.3 += lived.undos;
                            totals.4 += lived.damaged;
                            totals.5 += lived.refused;
                            totals.6 = totals.6.max(lived.slowest_settle);
                            totals.7 += lived.transitions;
                            None
                        }
                        Ok(Err(error)) => Some(format!("{error:#}")),
                        Err(panic) => Some(format!(
                            "panicked: {}",
                            panic
                                .downcast_ref::<String>()
                                .cloned()
                                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                                .unwrap_or_default()
                        )),
                    };
                    if let Some(what) = failure {
                        eprintln!("colony {index}: {what}");
                        failures.lock().unwrap().push(Failure {
                            index,
                            what,
                            save: last_save.into_inner().unwrap(),
                        });
                    }
                }
            });
        }
    });
    std::panic::set_hook(default_hook);
    let failures = failures.into_inner().unwrap();
    let (ticks, reloads, edits, undos, damaged, refused, slowest, transitions) =
        totals.into_inner().unwrap();
    println!(
        "# lived {ticks} ticks ({:.1} simulated hours) with {reloads} reloads, {edits} edits, \
         {undos} undos and {transitions} routine changes come round while away; {damaged} damaged files, {refused} refused, the rest repaired; the \
         slowest return within reach after a change took {:.2} s; {:.0} s",
        ticks as f64 * f64::from(DT) / 3600.0,
        f64::from(slowest) * f64::from(DT),
        started.elapsed().as_secs_f64()
    );
    for failure in &failures {
        let path = options.out.join(format!(
            "soak-failure-{}-{}.json",
            options.seed, failure.index
        ));
        let report = serde_json_report(options.seed, failure);
        std::fs::write(&path, report)?;
        println!(
            "# colony {} failed: {} (replay: soak --seed {} --only {} --days {}; save in {})",
            failure.index,
            failure.what,
            options.seed,
            failure.index,
            options.days,
            path.display()
        );
    }
    if failures.is_empty() {
        println!("# every colony held");
        Ok(())
    } else {
        bail!("{} of {} colonies failed", failures.len(), indices.len())
    }
}

/// A failure, written out as JSON, with the colony's own file inside it as it would be saved.
fn serde_json_report(seed: u64, failure: &Failure) -> String {
    let save = failure
        .save
        .as_ref()
        .and_then(|save| serde_json::to_string_pretty(save).ok())
        .unwrap_or_else(|| "null".to_owned());
    format!(
        "{{\n  \"seed\": {seed},\n  \"colony\": {},\n  \"what\": {:?},\n  \"last_save\": {save}\n}}\n",
        failure.index, failure.what
    )
}

/// One display, `width` by `height` points at `x`, `y`.
fn display(
    id: u64,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    scale: f32,
    primary: bool,
) -> MonitorInfo {
    let bounds = DesktopRect {
        x,
        y,
        width,
        height,
    };
    MonitorInfo {
        id,
        display_key: DisplayKey([id as u8; 16]),
        bounds,
        usable_bounds: DesktopRect {
            x,
            y: y + 24.0,
            width,
            height: height - 24.0 - 60.0,
        },
        scale_factor: scale,
        primary,
    }
}

/// A random arrangement of one to three displays: side by side, stacked, or to the left of the
/// primary at negative coordinates, at scales from 1x to 2x.
fn displays(rng: &mut ChaCha8Rng) -> Vec<MonitorInfo> {
    const SIZES: [(f32, f32); 5] = [
        (1280.0, 800.0),
        (1366.0, 768.0),
        (1440.0, 900.0),
        (1920.0, 1080.0),
        (2560.0, 1440.0),
    ];
    const SCALES: [f32; 4] = [1.0, 1.25, 1.5, 2.0];
    let count = rng.random_range(1..=3);
    let mut monitors = Vec::with_capacity(count);
    let (width, height) = SIZES[rng.random_range(0..SIZES.len())];
    monitors.push(display(
        1,
        0.0,
        0.0,
        width,
        height,
        SCALES[rng.random_range(0..SCALES.len())],
        true,
    ));
    for id in 2..=count as u64 {
        let (w, h) = SIZES[rng.random_range(0..SIZES.len())];
        // Beside, above or to the left of everything placed so far, so no two overlap.
        let right = monitors
            .iter()
            .map(|m| m.bounds.right())
            .fold(f32::MIN, f32::max);
        let left = monitors.iter().map(|m| m.bounds.x).fold(f32::MAX, f32::min);
        let top = monitors.iter().map(|m| m.bounds.y).fold(f32::MAX, f32::min);
        let (x, y) = match rng.random_range(0..3) {
            0 => (right, rng.random_range(-200.0..200.0)),
            1 => (
                left + rng.random_range(0.0..(right - left - w).max(1.0)),
                top - h,
            ),
            _ => (left - w, rng.random_range(-200.0..200.0)),
        };
        monitors.push(display(
            id,
            x,
            y,
            w,
            h,
            SCALES[rng.random_range(0..SCALES.len())],
            false,
        ));
    }
    monitors
}

/// A few windows scattered over the displays.
fn windows(
    rng: &mut ChaCha8Rng,
    monitors: &[MonitorInfo],
    next_key: &mut u64,
) -> Vec<DesktopWindow> {
    let count = rng.random_range(0..=10);
    (0..count)
        .map(|index| {
            let monitor = &monitors[rng.random_range(0..monitors.len())].bounds;
            let width = rng.random_range(240.0..monitor.width.max(260.0) * 0.8);
            let height = rng.random_range(160.0..monitor.height.max(180.0) * 0.8);
            *next_key += 1;
            DesktopWindow {
                key: *next_key,
                bounds: DesktopRect {
                    x: monitor.x + rng.random_range(0.0..(monitor.width - width).max(1.0)),
                    y: monitor.y + rng.random_range(30.0..(monitor.height - height).max(31.0)),
                    width,
                    height,
                },
                z_order: index,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            }
        })
        .collect()
}

/// Whether a point is somewhere a display reaches.
fn reached(point: Point, monitors: &[MonitorInfo]) -> bool {
    monitors.iter().any(|monitor| {
        let b = monitor.bounds;
        point.x >= b.x - REACH
            && point.x <= b.x + b.width + REACH
            && point.y >= b.y - REACH
            && point.y <= b.y + b.height + REACH
    })
}

/// Everything a colony must hold at a checkpoint.
fn check(world: &World, desktop: &DesktopSnapshot, when: &str) -> anyhow::Result<()> {
    let broken = violations(&world.save);
    if !broken.is_empty() {
        bail!("{when}: {}", broken.join("; "));
    }
    if !desktop.monitors.is_empty() {
        for creature in &world.save.creatures {
            if !reached(creature.state.position, &desktop.monitors) {
                bail!(
                    "{when}: {} is at ({:.0}, {:.0}), which no display reaches ({:?} on {:?}; \
                     houses {}; paused {}, visible {}, reduced motion {}, quiet {}, interacting \
                     {}; wonder {:?}; village moment {:?}; displays {:?})",
                    creature.name,
                    creature.state.position.x,
                    creature.state.position.y,
                    creature.state.action,
                    creature.state.surface.kind,
                    if world.save.home.is_active() {
                        "out"
                    } else {
                        "away"
                    },
                    world.save.settings.paused,
                    world.save.settings.visible,
                    world.save.settings.reduce_motion,
                    world.save.companion.quiet_until.is_some(),
                    world.is_interacting(),
                    world.wonder().map(|w| (w.kind, w.at, w.monitor_id)),
                    world.village_moment(),
                    desktop
                        .monitors
                        .iter()
                        .map(|m| (m.bounds.x, m.bounds.y, m.bounds.width, m.bounds.height))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
    Ok(())
}

/// Write the colony and read it back, the way a relaunch does. What comes back must be the colony
/// that was written, validated.
fn reload(world: &World, store: &SaveStore) -> anyhow::Result<World> {
    let written = world.save.clone();
    store.save(&written)?;
    let read = store
        .load()?
        .context("the colony was written but nothing came back")?;
    // Compared as written, since what a colony holds only while it runs is never written.
    let written_json = serde_json::to_value(&written)?;
    let expected_json = serde_json::to_value(&*ValidatedSave::from(written))?;
    let read_json = serde_json::to_value(&*read)?;
    for (before, after, what) in [
        (
            &expected_json,
            &read_json,
            "the colony read back is not the colony written",
        ),
        (
            &written_json,
            &expected_json,
            "the running colony held what validation changes",
        ),
    ] {
        if before != after {
            let mut differences = Vec::new();
            differ(before, after, String::new(), &mut differences);
            bail!("{what}: {}", differences.join("; "));
        }
    }
    let bytes = std::fs::metadata(store.path())?.len();
    if bytes > formiga_core::MAX_SAVE_BYTES / 4 {
        bail!("the colony file has grown to {bytes} bytes");
    }
    Ok(World::from_save(read))
}

/// Where two JSON documents differ, as pointers with both values, the first few of them.
fn differ(a: &serde_json::Value, b: &serde_json::Value, at: String, out: &mut Vec<String>) {
    use serde_json::Value;
    if out.len() >= 6 || a == b {
        return;
    }
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for key in x.keys().chain(y.keys().filter(|key| !x.contains_key(*key))) {
                differ(
                    x.get(key).unwrap_or(&Value::Null),
                    y.get(key).unwrap_or(&Value::Null),
                    format!("{at}/{key}"),
                    out,
                );
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (index, (x, y)) in x.iter().zip(y).enumerate() {
                differ(x, y, format!("{at}/{index}"), out);
            }
        }
        _ => {
            let short = |value: &Value| {
                let text = value.to_string();
                if text.len() > 120 {
                    format!("{}…", &text[..120])
                } else {
                    text
                }
            };
            out.push(format!("{at}: {} became {}", short(a), short(b)));
        }
    }
}

/// One colony's whole life.
fn live(
    index: usize,
    options: &Options,
    scratch: &Path,
    last_save: &Mutex<Option<SaveFile>>,
) -> anyhow::Result<Lived> {
    let mut rng = ChaCha8Rng::seed_from_u64(
        options.seed ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
    );
    let mut lived = Lived {
        ticks: 0,
        reloads: 0,
        edits: 0,
        undos: 0,
        damaged: 0,
        refused: 0,
        transitions: 0,
        slowest_settle: 0,
    };
    std::fs::create_dir_all(scratch)?;
    let store = SaveStore::new(scratch.join("colony.json"));
    let mut next_key = 0;
    let mut monitors = displays(&mut rng);
    let mut desktop = DesktopSnapshot {
        windows: windows(&mut rng, &monitors, &mut next_key),
        monitors: monitors.clone(),
        cursor: CursorSnapshot {
            position: Point { x: 300.0, y: 300.0 },
            velocity: Point::default(),
            available: true,
        },
        ..DesktopSnapshot::default()
    };
    // Somewhere in the year, at some hour of the morning.
    let mut now = datetime!(2026-01-01 7:00 UTC)
        + Duration::days(rng.random_range(0..365))
        + Duration::minutes(rng.random_range(0..240));
    let mut world = World::new(rng.random(), now, &desktop);
    for _ in 0..rng.random_range(0..=4) {
        let _ = world.add_designed_adult(rng.random(), None, now, &desktop);
    }
    randomize_preferences(&mut world, &mut rng);
    if rng.random_bool(0.35) {
        weekly_routine(&mut world, &mut rng);
    }
    check(&world, &desktop, "founded")?;

    for day in 0..options.days {
        let sessions = rng.random_range(2..=5);
        for session in 0..sessions {
            let minutes = rng.random_range(4..=30);
            let ticks = minutes * 60 * 20;
            let mut carrying: Option<u32> = None;
            let applied_before = world.save.companion.schedule.applied;
            for tick in 0..ticks {
                now += Duration::milliseconds(50);
                // The desktop goes on around the colony.
                if rng.random_bool(0.01) && !desktop.windows.is_empty() {
                    let which = rng.random_range(0..desktop.windows.len());
                    let window = &mut desktop.windows[which];
                    window.bounds.x += rng.random_range(-40.0..40.0);
                    window.bounds.y += rng.random_range(-20.0..20.0);
                }
                if rng.random_bool(0.0005) {
                    desktop.windows = windows(&mut rng, &monitors, &mut next_key);
                }
                if rng.random_bool(0.02) {
                    let monitor = monitors[rng.random_range(0..monitors.len())].bounds;
                    let to = Point {
                        x: monitor.x + rng.random_range(0.0..monitor.width),
                        y: monitor.y + rng.random_range(0.0..monitor.height),
                    };
                    desktop.cursor.velocity = Point {
                        x: (to.x - desktop.cursor.position.x) * 2.0,
                        y: (to.y - desktop.cursor.position.y) * 2.0,
                    };
                    desktop.cursor.position = to;
                }
                // Somebody at the desk now and then.
                if let Some(left) = carrying.as_mut() {
                    *left = left.saturating_sub(1);
                    desktop.cursor.position.x += rng.random_range(-6.0..6.0);
                    desktop.cursor.position.y += rng.random_range(-6.0..6.0);
                    let cursor = desktop.cursor.position;
                    if *left == 0 {
                        let velocity = Point {
                            x: rng.random_range(-900.0..900.0),
                            y: rng.random_range(-900.0..300.0),
                        };
                        world.handle_command(
                            WorldCommand::EndInteraction { cursor, velocity },
                            &desktop,
                        );
                        carrying = None;
                    } else {
                        world.handle_command(
                            WorldCommand::UpdateInteraction {
                                cursor,
                                velocity: Point { x: 12.0, y: -4.0 },
                            },
                            &desktop,
                        );
                    }
                } else if rng.random_bool(0.0008) && !world.save.creatures.is_empty() {
                    let creature =
                        &world.save.creatures[rng.random_range(0..world.save.creatures.len())];
                    let id = creature.id;
                    let cursor = creature.state.position;
                    desktop.cursor.position = cursor;
                    let command = match rng.random_range(0..7) {
                        0..=2 => WorldCommand::BeginInteraction {
                            creature_id: id,
                            cursor,
                        },
                        3 => WorldCommand::OfferSnack { creature_id: id },
                        4 => WorldCommand::OfferToy { creature_id: id },
                        5 => WorldCommand::GatherCreatures,
                        _ => WorldCommand::SendHome,
                    };
                    let begins = matches!(command, WorldCommand::BeginInteraction { .. });
                    if world.handle_command(command, &desktop) && begins {
                        carrying = Some(rng.random_range(1..80));
                    }
                }
                world.tick(now, DT, &desktop);
                world.drain_events().for_each(drop);
                lived.ticks += 1;
                if tick == 0 {
                    // Back from being away: the routine is the one the week says it is now,
                    // whatever was missed, and a change that came round says so in the settings.
                    routine_holds(&world, now, applied_before)
                        .with_context(|| format!("day {day}, start of session {session}"))?;
                    if world.save.companion.schedule.applied != applied_before {
                        lived.transitions += 1;
                    }
                }
                if tick % CHECK_EVERY == CHECK_EVERY - 1 {
                    check(
                        &world,
                        &desktop,
                        &format!("day {day}, session {session}, minute {}", tick / 1200),
                    )?;
                    *last_save.lock().unwrap() = Some(world.save.clone());
                }
            }
            if carrying.is_some() {
                world.handle_command(WorldCommand::CancelInteraction, &desktop);
            }
            check(
                &world,
                &desktop,
                &format!("day {day}, end of session {session}"),
            )?;

            // Between sessions: written to disk, and half the time read back as a relaunch would.
            if rng.random_bool(0.5) {
                world = reload(&world, &store)?;
                lived.reloads += 1;
            } else {
                store.save(&world.save)?;
            }
            *last_save.lock().unwrap() = Some(world.save.clone());
            // Changes made in the notebook while it was away, and some of them taken back.
            for _ in 0..rng.random_range(0..4) {
                if edit(&mut world, &mut rng, now, &desktop) {
                    lived.edits += 1;
                }
            }
            for _ in 0..rng.random_range(0..3) {
                if world.undo_last_edit().is_ok() {
                    lived.undos += 1;
                }
            }
            if rng.random_bool(0.3) {
                randomize_preferences(&mut world, &mut rng);
                // Preferences changed by hand hold until the routine's next change.
                world.override_routine();
            }
            if rng.random_bool(0.1) {
                world.resume_routine(now);
                routine_holds(&world, now, world.save.companion.schedule.applied)
                    .context("just after the routine was resumed")?;
            }
            if rng.random_bool(0.2) {
                world.set_quiet_mode(rng.random_range(0..=60), now);
            }
            // A display plugged in or out, or the whole arrangement changed.
            if rng.random_bool(0.25) {
                monitors = displays(&mut rng);
                desktop.monitors = monitors.clone();
                desktop.windows = windows(&mut rng, &monitors, &mut next_key);
            }
            // A changed arrangement of displays is settled as the colony carries on: anybody
            // left where no display reaches is brought back within a few seconds.
            let mut settled_in = None;
            for tick in 0..SETTLE_TICKS {
                if check(&world, &desktop, "").is_ok() {
                    settled_in = Some(tick);
                    break;
                }
                now += Duration::milliseconds(50);
                world.tick(now, DT, &desktop);
                world.drain_events().for_each(drop);
            }
            if settled_in.is_none() {
                check(
                    &world,
                    &desktop,
                    &format!(
                        "day {day}, {} s after session {session}'s changes",
                        SETTLE_TICKS / 20
                    ),
                )?;
            }
            lived.slowest_settle = lived.slowest_settle.max(settled_in.unwrap_or(0));
            // Away for a while: a coffee, a meeting, an afternoon.
            now += Duration::minutes(rng.random_range(10..=240));
        }
        // Overnight, and sometimes a weekend.
        let nights = if rng.random_bool(0.15) { 3 } else { 1 };
        now += Duration::hours(rng.random_range(8..=14)) + Duration::days(nights - 1);
    }
    let final_save = world.save.clone();
    *last_save.lock().unwrap() = Some(final_save.clone());
    for round in 0..options.damage {
        let refused = damage_round(&final_save, &mut rng, &desktop)
            .with_context(|| format!("damage round {round}"))?;
        lived.damaged += 1;
        lived.refused += u32::from(refused);
    }
    Ok(lived)
}

/// A weekly routine between two saved sets of preferences, changing one to four times a week at
/// random minutes on random days.
fn weekly_routine(world: &mut World, rng: &mut ChaCha8Rng) {
    let mut work = BehaviorPreset::capture(&world.save.settings);
    work.window_ledges = true;
    work.reduce_motion = false;
    let mut relax = work.clone();
    relax.window_ledges = false;
    relax.cursor_reactions = !work.cursor_reactions;
    relax.habitat.preset = HabitatPreset::BottomEdge;
    world.save.companion.modes = [Some(work), Some(relax)];
    world.save.companion.schedule = RoutineSchedule {
        enabled: true,
        transitions: (0..rng.random_range(1..=4))
            .map(|_| ScheduledTransition {
                days: rng.random_range(1..=0b111_1111),
                minute: rng.random_range(0..1440),
                preset: rng.random_range(0..2),
            })
            .collect(),
        ..RoutineSchedule::default()
    };
}

/// The weekly routine is the one the week says it is at `now`, in local time, and if it changed
/// since `before`, the preferences are that routine's.
fn routine_holds(world: &World, now: OffsetDateTime, before: Option<u8>) -> anyhow::Result<()> {
    let companion = &world.save.companion;
    let schedule = &companion.schedule;
    if !schedule.enabled {
        return Ok(());
    }
    let local =
        now.to_offset(time::UtcOffset::local_offset_at(now).unwrap_or(time::UtcOffset::UTC));
    let Some(intended) = schedule.intended(local) else {
        return Ok(());
    };
    let Some(preset) = companion.modes[usize::from(intended)].as_ref() else {
        return Ok(());
    };
    if schedule.applied != Some(intended) {
        bail!(
            "the routine at {local} should be {intended} but {:?} is in place",
            schedule.applied
        );
    }
    let settings = &world.save.settings;
    if schedule.applied != before
        && (settings.window_ledges != preset.window_ledges
            || settings.cursor_reactions != preset.cursor_reactions
            || settings.reduce_motion != preset.reduce_motion
            || settings.habitat.preset != preset.habitat.preset)
    {
        bail!("routine {intended} came round at {local} but the preferences are not its own");
    }
    Ok(())
}

/// Random preferences, as the person at the desk might set them.
fn randomize_preferences(world: &mut World, rng: &mut ChaCha8Rng) {
    let settings = &mut world.save.settings;
    settings.visible = rng.random_bool(0.9);
    settings.paused = rng.random_bool(0.08);
    settings.window_ledges = rng.random_bool(0.85);
    settings.cursor_reactions = rng.random_bool(0.85);
    settings.reduce_motion = rng.random_bool(0.1);
    settings.direct_manipulation = rng.random_bool(0.95);
    settings.display_scale = [2, 3, 4][rng.random_range(0..3)];
    settings.habitat.preset = [
        HabitatPreset::EntireDesktop,
        HabitatPreset::PrimaryDisplay,
        HabitatPreset::BottomEdge,
        HabitatPreset::BottomCorners,
        HabitatPreset::LowerHalf,
    ][rng.random_range(0..5)];
    settings.habitat.zones.clear();
    if let Some(creature) = world.save.creatures.first() {
        let id = creature.id;
        let _ = world.set_roaming_leaning(id, RoamingLeaning::ALL[rng.random_range(0..4)]);
    }
}

/// One change of the kind the notebook makes, through `World::edit` as the notebook does.
/// Returns whether it changed anything.
fn edit(
    world: &mut World,
    rng: &mut ChaCha8Rng,
    now: OffsetDateTime,
    desktop: &DesktopSnapshot,
) -> bool {
    let before = world.undoable_edits();
    match rng.random_range(0..7) {
        0 => {
            let kind = HangoutKind::STARTING[rng.random_range(0..3)];
            let along = rng.random_bool(0.8).then(|| rng.random_range(0.0..1.0));
            world.edit(ColonyEdit::Hangout(kind), |world| {
                world.save.home.set_hangout(kind, along)
            });
        }
        1 => {
            let kind = GardenKind::STARTING[rng.random_range(0..3)];
            let along = rng.random_bool(0.8).then(|| rng.random_range(0.0..1.0));
            world.edit(ColonyEdit::Garden(kind), |world| {
                world.save.home.set_garden(kind, along, now)
            });
        }
        2 => {
            let mut order: Vec<_> = world.save.creatures.iter().map(|c| c.id).collect();
            for i in (1..order.len()).rev() {
                order.swap(i, rng.random_range(0..=i));
            }
            world.edit(ColonyEdit::MovedCottages, |world| {
                let creatures = world.save.creatures.clone();
                world.save.home.arrange_cottages(order, &creatures);
            });
        }
        3 => {
            let palette = rng
                .random_bool(0.8)
                .then(|| VillagePalette::ALL[rng.random_range(0..VillagePalette::ALL.len())]);
            world.edit(ColonyEdit::PaintedVillage, |world| {
                world.save.home.palette = palette;
            });
        }
        4 => {
            world.edit(ColonyEdit::MovedHome, |world| {
                world.save.home.corner = match world.save.home.corner {
                    HomeCorner::BottomLeft => HomeCorner::BottomRight,
                    HomeCorner::BottomRight => HomeCorner::BottomLeft,
                };
            });
        }
        5 if world.save.creatures.len() > 1 => {
            let creature = &world.save.creatures[rng.random_range(0..world.save.creatures.len())];
            let (id, name) = (creature.id, creature.name.clone());
            let _ = world.edit(ColonyEdit::Removed { name }, |world| {
                world.remove_colony_creature(id)
            });
        }
        _ => {
            let seed = rng.random();
            let _ = world.edit(ColonyEdit::Welcomed, |world| {
                world.add_designed_adult(seed, None, now, desktop)
            });
        }
    }
    world.undoable_edits() != before || before == formiga_core::tuning::UNDO.depth
}

/// Damage the colony's file one way and read it: it must be refused, or come back holding
/// nothing a validated colony never does, and open and run. Returns whether it was refused.
fn damage_round(
    save: &SaveFile,
    rng: &mut ChaCha8Rng,
    desktop: &DesktopSnapshot,
) -> anyhow::Result<bool> {
    let mut value = serde_json::to_value(save)?;
    for _ in 0..rng.random_range(1..=3) {
        formiga_core::damage(&mut value, rng);
    }
    let bytes = serde_json::to_vec(&value)?;
    match formiga_core::decode(&bytes) {
        Err(_) => Ok(true),
        Ok(read) => {
            let broken = violations(&read);
            if !broken.is_empty() {
                bail!("a damaged file was half read: {}", broken.join("; "));
            }
            let mut opened = World::from_save(read);
            let now = opened.save.maximum_seen_utc;
            for step in 1..=20 {
                opened.tick(now + Duration::milliseconds(50 * step), DT, desktop);
            }
            Ok(false)
        }
    }
}
