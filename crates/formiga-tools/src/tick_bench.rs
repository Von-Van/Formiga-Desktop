//! Per-tick cost of the simulation, measured without a GUI.
//!
//! `World::tick` is the work the desktop app repeats four to twenty times a second, so its cost in
//! microseconds is the floor under the application's CPU use. This drives `formiga_core::World`
//! exactly the way `formiga-desktop`'s `App::tick` does — tick, then drain events — over synthetic
//! desktops, and reports microseconds per tick. It is deterministic, so running it before and after
//! a change is a like-for-like regression check. Build it in release, or the numbers mean nothing:
//!
//!   cargo run --release -p formiga-tools -- tick-bench [--ticks N] [--warmup N] [filter]

use std::time::Instant;

use formiga_core::{
    ActionKind, CreatureDesign, CursorSnapshot, DesktopRect, DesktopSnapshot, DesktopWindow,
    DisplayKey, MonitorInfo, Point, SaveStore, SaveUrgency, SharedCreatureSeed, SurfaceAttachment,
    SurfaceKind, World, save_due,
};
use time::{Duration, OffsetDateTime, macros::datetime};

/// The simulated seconds one tick advances. `formiga-desktop` uses a variable dt clamped to 0.2s;
/// 0.05 is the 20 Hz cadence it runs at whenever anything is moving, and the value every test in
/// `crates/formiga-core/src/world.rs` uses.
const DT: f32 = 0.05;

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let mut ticks = 40_000usize;
    let mut warmup = 4_000usize;
    let mut filter: Option<String> = None;
    let mut saved = None;

    let mut args = args.iter().cloned();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ticks" => ticks = args.next().and_then(|v| v.parse().ok()).unwrap_or(ticks),
            "--warmup" => warmup = args.next().and_then(|v| v.parse().ok()).unwrap_or(warmup),
            "-h" | "--help" => {
                eprintln!("usage: formiga-tools tick-bench [--ticks N] [--warmup N] [filter]");
                return Ok(());
            }
            other => filter = Some(other.to_string()),
        }
    }

    println!("# Formiga simulation tick cost");
    println!("#");
    println!("# host          : {}", host_line());
    println!("# measured ticks: {ticks} per scenario (after {warmup} warm-up ticks)");
    println!("# dt            : {DT} simulated seconds per tick");
    println!("# method        : std::time::Instant around World::tick + World::drain_events,");
    println!("#                 the same pair formiga-desktop's App::tick runs each cadence.");
    println!();
    println!(
        "{:<36} {:>3} {:>9} {:>9} {:>9} {:>9} {:>8} {:>6} {:>8} {:>9}  notes",
        "scenario",
        "n",
        "mean us",
        "p50 us",
        "p95 us",
        "max us",
        "motion%",
        "home%",
        "asks/min",
        "saves/min"
    );
    println!("{}", "-".repeat(148));

    for scenario in scenarios() {
        if let Some(filter) = &filter
            && !scenario.name.contains(filter.as_str())
        {
            continue;
        }
        let (result, world) = measure(&scenario, warmup, ticks);
        println!(
            "{:<36} {:>3} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {:>8.1} {:>6.1} {:>8.1} {:>9.1}  {}",
            scenario.name,
            result.creatures,
            result.mean_us,
            result.p50_us,
            result.p95_us,
            result.max_us,
            result.motion_pct,
            result.home_pct,
            result.asks_per_minute,
            result.saves_per_minute,
            result.notes
        );
        if scenario.name == SAVE_SCENARIO {
            saved = Some(world);
        }
    }
    if let Some(world) = saved {
        save_cost(&world)?;
    }

    println!();
    println!("# Reading these numbers:");
    println!("#   formiga-desktop ticks at 20 Hz while anything moves, 10 Hz for expressive or");
    println!("#   cursor-responsive rest, 5 Hz for quiet rest, and 4 Hz while paused or hidden");
    println!("#   (crates/formiga-desktop/src/app.rs, `world_tick_interval`).");
    println!(
        "#   One core-percent at 20 Hz == 500 us per tick. So mean_us/500 is the share of one"
    );
    println!("#   core the SIMULATION costs at the worst cadence; the app's total CPU also");
    println!("#   includes the native desktop scan, egui, and the GPU overlay present, which this");
    println!("#   harness does not measure. This is a floor, not the application's CPU figure.");
    println!("#   asks/min counts the ticks whose events ask for the colony to be saved, per");
    println!("#   simulated minute: what formiga-desktop wrote before routine checkpoints.");
    println!("#   saves/min is how often it writes now, through formiga_core::save_due.");
    Ok(())
}

/// The scenario whose colony, as the run leaves it, is written out to measure what a save costs.
const SAVE_SCENARIO: &str = "6 creatures, busy desktop + cursor";

/// What one full save of a colony costs, through the same `SaveStore::save` the app calls:
/// serialize, write and flush a temporary file, read and validate the current one, copy it to the
/// backup, and replace it. Written to a fresh temporary directory, removed afterwards.
fn save_cost(world: &World) -> anyhow::Result<()> {
    const SAVES: usize = 200;
    let directory = std::env::temp_dir().join(format!("formiga-save-bench-{}", std::process::id()));
    std::fs::create_dir_all(&directory)?;
    let store = SaveStore::new(directory.join("colony.json"));
    store.save(&world.save)?;
    let mut samples = Vec::with_capacity(SAVES);
    for _ in 0..SAVES {
        let at = Instant::now();
        store.save(&world.save)?;
        samples.push(at.elapsed().as_secs_f64() * 1_000.0);
    }
    let bytes = std::fs::metadata(store.path())?.len();
    std::fs::remove_dir_all(&directory)?;
    samples.sort_by(f64::total_cmp);
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    println!();
    println!("# Save cost ({SAVE_SCENARIO}, as the run left it)");
    println!(
        "#   {SAVES} saves of a {bytes}-byte colony file: mean {mean:.2} ms, p50 {:.2} ms, \
         p95 {:.2} ms, max {:.2} ms",
        samples[samples.len() / 2],
        samples[samples.len() * 95 / 100],
        samples[samples.len() - 1]
    );
    Ok(())
}

struct Scenario {
    name: &'static str,
    creatures: usize,
    desktop: DesktopKind,
    paused: bool,
    /// Keep the colony at the houses for the whole run rather than letting it wander the desktop.
    homebound: bool,
    /// A friend invited for the day, touring the village while the colony is home.
    visitor: bool,
    notes: &'static str,
}

#[derive(Clone, Copy, PartialEq)]
enum DesktopKind {
    /// One display, no windows, a parked cursor: the cheapest thing the simulation can see.
    Quiet,
    /// Two displays, 24 windows, a cursor in motion, and window geometry that keeps changing, so
    /// the topology/attention observers rebuild instead of hitting their unchanged-hash fast path.
    Busy,
}

fn scenarios() -> Vec<Scenario> {
    let scenario = |name, creatures, desktop, paused, homebound, visitor, notes| Scenario {
        name,
        creatures,
        desktop,
        paused,
        homebound,
        visitor,
        notes,
    };
    use DesktopKind::{Busy, Quiet};
    vec![
        scenario(
            "1 creature, quiet desktop",
            1,
            Quiet,
            false,
            false,
            false,
            "cheapest realistic input",
        ),
        scenario(
            "4 creatures, quiet desktop",
            4,
            Quiet,
            false,
            false,
            false,
            "",
        ),
        scenario(
            "6 creatures, quiet desktop",
            6,
            Quiet,
            false,
            false,
            false,
            "a full colony",
        ),
        scenario(
            "1 creature, busy desktop + cursor",
            1,
            Busy,
            false,
            false,
            false,
            "",
        ),
        scenario(
            "4 creatures, busy desktop + cursor",
            4,
            Busy,
            false,
            false,
            false,
            "",
        ),
        scenario(
            "6 creatures, busy desktop + cursor",
            6,
            Busy,
            false,
            false,
            false,
            "most expensive realistic input",
        ),
        scenario(
            "4 creatures, homebound at shelter",
            4,
            Quiet,
            false,
            true,
            false,
            "",
        ),
        scenario(
            "6 creatures, homebound at shelter",
            6,
            Quiet,
            false,
            true,
            false,
            "",
        ),
        scenario(
            "6 creatures + visitor, homebound",
            6,
            Quiet,
            false,
            true,
            true,
            "an invited friend touring the village",
        ),
        scenario(
            "4 creatures, paused, busy desktop",
            4,
            Busy,
            true,
            false,
            false,
            "paused: budget forbids a busy loop",
        ),
        scenario(
            "6 creatures, paused, busy desktop",
            6,
            Busy,
            true,
            false,
            false,
            "paused: budget forbids a busy loop",
        ),
    ]
}

struct Outcome {
    creatures: usize,
    mean_us: f64,
    p50_us: f64,
    p95_us: f64,
    max_us: f64,
    /// Share of measured ticks in which at least one creature was actually moving across the
    /// desktop, using the same predicate `formiga-desktop` uses to pick its 20 Hz cadence. It is
    /// what makes a "resting" or "moving" scenario label checkable rather than asserted.
    motion_pct: f64,
    /// Share of measured ticks with the houses out, which is what makes "homebound" checkable.
    home_pct: f64,
    /// Ticks per simulated minute whose events ask for a save: before routine checkpoints, each
    /// of them wrote the whole colony to disk.
    asks_per_minute: f64,
    /// Whole-colony writes per simulated minute under the app's save policy.
    saves_per_minute: f64,
    notes: String,
}

/// Mirror of `world_has_spatial_motion` in crates/formiga-desktop/src/app.rs.
fn has_spatial_motion(world: &World) -> bool {
    if world.save.settings.paused {
        return false;
    }
    world.save.creatures.iter().any(|creature| {
        creature.state.velocity.x.abs() > 0.1
            || creature.state.velocity.y.abs() > 0.1
            || matches!(
                creature.state.action,
                ActionKind::Traverse
                    | ActionKind::SqueezeWindow
                    | ActionKind::Sprint
                    | ActionKind::InvestigateCursor
                    | ActionKind::AvoidCursor
                    | ActionKind::ReactToWindow
                    | ActionKind::Follow
                    | ActionKind::Dragged
                    | ActionKind::Landing
                    | ActionKind::ClimbWindow
                    | ActionKind::Tossed
            )
    })
}

fn measure(scenario: &Scenario, warmup: usize, ticks: usize) -> (Outcome, World) {
    let created = datetime!(2026-09-16 10:00 UTC);
    let mut world = build_world(scenario, created);
    let started_with = world.save.creatures.len();

    let mut clock = created + Duration::days(40);
    let mut step = 0usize;

    // Warm-up: let the colony settle into ordinary behaviour before anything is timed.
    for _ in 0..warmup {
        let desktop = desktop_for(scenario.desktop, step);
        hold_gathering(&mut world, scenario, clock);
        world.tick(clock, DT, &desktop);
        let _ = world.drain_events().count();
        clock += Duration::seconds_f64(f64::from(DT));
        step += 1;
    }

    let mut samples: Vec<u64> = Vec::with_capacity(ticks);
    let mut moving_ticks = 0usize;
    let mut home_ticks = 0usize;
    let (mut ask_ticks, mut saves) = (0usize, 0usize);
    let (mut waiting, mut since_save) = (SaveUrgency::None, std::time::Duration::ZERO);
    for _ in 0..ticks {
        let desktop = desktop_for(scenario.desktop, step);
        hold_gathering(&mut world, scenario, clock);
        let at = Instant::now();
        world.tick(clock, DT, &desktop);
        let urgency = world.drain_events().fold(SaveUrgency::None, |most, event| {
            most.max(event.save_urgency())
        });
        samples.push(at.elapsed().as_nanos() as u64);
        ask_ticks += usize::from(std::hint::black_box(urgency) > SaveUrgency::None);
        waiting = waiting.max(urgency);
        since_save += std::time::Duration::from_secs_f32(DT);
        if save_due(waiting, since_save) {
            saves += 1;
            waiting = SaveUrgency::None;
            since_save = std::time::Duration::ZERO;
        }
        if has_spatial_motion(&world) {
            moving_ticks += 1;
        }
        if world.save.home.is_active() {
            home_ticks += 1;
        }
        clock += Duration::seconds_f64(f64::from(DT));
        step += 1;
    }

    samples.sort_unstable();
    let total: u128 = samples.iter().map(|n| u128::from(*n)).sum();
    let mean_us = total as f64 / samples.len() as f64 / 1_000.0;
    let p50_us = samples[samples.len() / 2] as f64 / 1_000.0;
    let p95_us = samples[samples.len() * 95 / 100] as f64 / 1_000.0;
    let max_us = *samples.last().unwrap() as f64 / 1_000.0;

    let ended_with = world.save.creatures.len();
    let mut notes = scenario.notes.to_string();
    if ended_with != started_with {
        notes = format!("{notes} [colony changed {started_with}->{ended_with}]");
    }

    let outcome = Outcome {
        creatures: ended_with,
        mean_us,
        p50_us,
        p95_us,
        max_us,
        motion_pct: moving_ticks as f64 * 100.0 / ticks as f64,
        home_pct: home_ticks as f64 * 100.0 / ticks as f64,
        asks_per_minute: ask_ticks as f64 / (ticks as f64 * f64::from(DT) / 60.0),
        saves_per_minute: saves as f64 / (ticks as f64 * f64::from(DT) / 60.0),
        notes,
    };
    (outcome, world)
}

/// A gathering lasts fifteen simulated minutes, a third of a default run, so a homebound scenario
/// keeps its houses out by holding the gathering five minutes in. Outside the timed region, and
/// only for the scenarios that ask to stay home.
fn hold_gathering(world: &mut World, scenario: &Scenario, clock: OffsetDateTime) {
    if scenario.homebound {
        world.save.home.active_since_utc = Some(clock - Duration::minutes(5));
        world.save.home.last_disappeared_utc = None;
    }
}

fn build_world(scenario: &Scenario, created: OffsetDateTime) -> World {
    let desktop = desktop_for(scenario.desktop, 0);
    let mut world = World::new([0x5a; 32], created, &desktop);

    if scenario.creatures > 1 {
        // The colony grows on its own at 1 hour, 7 days and 1 calendar month. Ticking past the
        // last milestone lets the real arrival path build the colony, rather than hand-assembling
        // creatures the simulation would never produce.
        let mut at = created;
        for day in [1i64, 8, 32, 33, 34] {
            at = created + Duration::days(day);
            world.tick(at, DT, &desktop);
            let _ = world.drain_events().count();
            if world.save.creatures.len() >= scenario.creatures {
                break;
            }
        }
        // Arrivals stop short of a full colony; full-size companions added the way the studio
        // adds them make up the rest.
        let mut seed = 0x60_u8;
        while world.save.creatures.len() < scenario.creatures {
            seed += 1;
            world
                .add_designed_adult([seed; 32], None, at, &desktop)
                .expect("the colony has room for the companions a scenario asks for");
        }
        // Clear the staggered reveal delay so every creature is actually simulated.
        for _ in 0..200 {
            at += Duration::seconds(1);
            world.tick(at, 1.0, &desktop);
            let _ = world.drain_events().count();
        }
        world.save.creatures.truncate(scenario.creatures);
    }
    // Stop further scheduled arrivals, the colony's and every adult's minis, so the colony size
    // stays fixed for the whole run.
    world.save.arrival_state.arrived = [true; 3];
    for creature in &mut world.save.creatures {
        creature.mini_arrivals.arrived = [true; 2];
    }

    world.save.settings.paused = scenario.paused;

    if scenario.homebound {
        world.save.home.active_since_utc = Some(created + Duration::days(40));
        world.save.home.last_disappeared_utc = None;
        if scenario.visitor {
            let friend = SharedCreatureSeed {
                source_colony_seed: [0x77; 32],
                source_generation: 0,
                design: Some(CreatureDesign::generated([0x77; 32], 0, None)),
            };
            world
                .invite_visitor(friend, created + Duration::days(40), &desktop)
                .expect("nobody else is visiting");
        }
    } else {
        // Same trick `world.rs`'s own `let_colony_wander` test fixture uses: retire the shelter so
        // the colony is out on the desktop instead of tucked away at home.
        world.save.home.active_since_utc = None;
        world.save.home.last_disappeared_utc = Some(created + Duration::days(40));
        for (index, creature) in world.save.creatures.iter_mut().enumerate() {
            creature.state.position = Point {
                x: 300.0 + index as f32 * 90.0,
                y: 846.0,
            };
            creature.state.surface = SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id: 1,
                window_key: None,
                relative_x: 0.5,
            };
            creature.state.action = ActionKind::Idle;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = 2.0;
        }
    }

    world
}

fn monitor(id: u64, key: u8, x: f32, scale: f32, primary: bool) -> MonitorInfo {
    MonitorInfo {
        id,
        display_key: DisplayKey([key; 16]),
        bounds: DesktopRect {
            x,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        },
        usable_bounds: DesktopRect {
            x,
            y: 24.0,
            width: 1440.0,
            height: 826.0,
        },
        scale_factor: scale,
        primary,
    }
}

fn desktop_for(kind: DesktopKind, step: usize) -> DesktopSnapshot {
    match kind {
        DesktopKind::Quiet => DesktopSnapshot {
            monitors: vec![monitor(1, 1, 0.0, 2.0, true)],
            windows: Vec::new(),
            cursor: CursorSnapshot {
                position: Point { x: 720.0, y: 400.0 },
                velocity: Point { x: 0.0, y: 0.0 },
                available: true,
            },
            idle_duration: std::time::Duration::from_secs(120),
            ..DesktopSnapshot::default()
        },
        DesktopKind::Busy => {
            let phase = step as f32;
            // Two displays at different scale factors, the mixed-DPI case the release notes call
            // out, plus 24 windows. Three of them drift, so the geometry hash keeps changing and
            // the topology and attention observers do real work instead of short-circuiting.
            let mut windows = Vec::with_capacity(24);
            for index in 0..24u64 {
                let column = (index % 6) as f32;
                let row = (index / 6) as f32;
                let drift = if index < 3 {
                    ((phase * 0.05 + index as f32).sin()) * 40.0
                } else {
                    0.0
                };
                let display_x = if index >= 12 { 1440.0 } else { 0.0 };
                windows.push(DesktopWindow {
                    key: index + 1,
                    bounds: DesktopRect {
                        x: display_x + column * 210.0 + drift,
                        y: 60.0 + row * 190.0,
                        width: 360.0,
                        height: 240.0,
                    },
                    z_order: index as u32,
                    visible: true,
                    minimized: false,
                    application: None,
                    application_name: None,
                });
            }
            DesktopSnapshot {
                monitors: vec![
                    monitor(1, 1, 0.0, 2.0, true),
                    monitor(2, 2, 1440.0, 1.0, false),
                ],
                windows,
                cursor: CursorSnapshot {
                    position: Point {
                        x: 720.0 + (phase * 0.11).sin() * 600.0,
                        y: 450.0 + (phase * 0.07).cos() * 300.0,
                    },
                    velocity: Point {
                        x: (phase * 0.11).cos() * 260.0,
                        y: -(phase * 0.07).sin() * 130.0,
                    },
                    available: true,
                },
                idle_duration: std::time::Duration::ZERO,
                ..DesktopSnapshot::default()
            }
        }
    }
}

fn host_line() -> String {
    let read = |args: &[&str]| -> String {
        std::process::Command::new(args[0])
            .args(&args[1..])
            .output()
            .ok()
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
            .unwrap_or_default()
    };
    let cpu = read(&["sysctl", "-n", "machdep.cpu.brand_string"]);
    let os = read(&["sw_vers", "-productVersion"]);
    if cpu.is_empty() {
        std::env::consts::OS.to_string()
    } else {
        format!("{cpu}, macOS {os}")
    }
}
