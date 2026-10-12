//! `formiga-tools dev …`: the fixed colonies the developer toolkit (`devtools/`) is built on.
//!
//!   cargo run -p formiga-tools -- dev fixtures
//!   cargo run -p formiga-tools -- dev fixture NAME --out DIR [--now-unix SECONDS]
//!   cargo run -p formiga-tools -- dev check-fixtures
//!   cargo run -p formiga-tools -- dev capture NAME --out PNG
//!   cargo run -p formiga-tools -- dev catalog                            (see dev_catalog.rs)
//!   cargo run -p formiga-tools -- dev art-check --out DIR [--kind KIND]  (see dev_art.rs)
//!   cargo run -p formiga-tools -- dev fits NAME                          (see dev_catalog.rs)
//!   cargo run -p formiga-tools -- dev save FILE                          (see dev_save.rs)
//!
//! Each fixture is a colony grown from a fixed seed to a fixed age, the way a real one grows:
//! `World::new`, then the clock run forward. `fixture` writes one as `colony.json` through the
//! app's own `SaveStore`, so `FORMIGA_DATA_DIR=DIR` opens Formiga on it. Without `--now-unix` the
//! colony is grown to a fixed moment, so the same name always gives the same file; the toolkit
//! passes the current time when it opens the app, so the colony is the age it says and nothing is
//! caught up on launch.
//!
//! `check-fixtures` holds every fixture to what `soak` holds a colony to: nothing
//! `formiga_core::violations` names, a save that reads back as the colony written, and a few
//! seconds of running without breaking either. `capture` draws one fixture through the art crate's
//! own colony card and creature cards. Every command prints one JSON document on stdout.

use crate::{blit_canvas_scaled, fixture_desktop, soak, write_png};
use anyhow::{Context, Result, bail};
use formiga_art::{
    CARD_HEIGHT, CARD_WIDTH, COLONY_CARD_HEIGHT, COLONY_CARD_WIDTH, ColonyCardRenderer,
    CreatureCardRenderer,
};
use formiga_core::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use time::{Duration, OffsetDateTime};

/// The moment every fixture is grown to unless told otherwise: the settings review's.
const FIXED_NOW: OffsetDateTime = time::macros::datetime!(2026-09-14 12:00 UTC);

/// Names given in colony order, so captures and summaries read as people rather than numbers.
const NAMES: [&str; 8] = [
    "Mallow", "Juniper", "Pebble", "Tofu", "Clover", "Biscuit", "Fern", "Mochi",
];

/// One fixed colony.
struct Fixture {
    name: &'static str,
    about: &'static str,
    grow: fn(OffsetDateTime, &DesktopSnapshot) -> World,
}

const FIXTURES: [Fixture; 4] = [
    Fixture {
        name: "new-colony",
        about: "The founder alone, just met: welcome finished, nothing else yet.",
        grow: new_colony,
    },
    Fixture {
        name: "young-colony",
        about: "Ten days in: the first arrivals, a little history.",
        grow: young_colony,
    },
    Fixture {
        name: "mature-colony",
        about: "Half a year in: a full household, grown the way the demo colony grows.",
        grow: mature_colony,
    },
    Fixture {
        name: "full-village",
        about: "Forty days in with every decoration, as many objects as a colony keeps, a guest \
                at the door and a full guest book: the settings review's colony, kept valid.",
        grow: full_village,
    },
];

pub fn run(args: &[String]) -> Result<()> {
    let report = match args.first().map(String::as_str) {
        Some("fixtures") => fixtures(),
        Some("fixture") => {
            let fixture = named(args.get(1))?;
            let out = path_option(args, "--out")?.context("dev fixture needs --out DIR")?;
            write_fixture(fixture, &out, now_option(args)?)?
        }
        Some("check-fixtures") => check_fixtures(),
        Some("catalog") => crate::dev_catalog::catalog(),
        Some("art-check") => {
            let out = path_option(args, "--out")?.context("dev art-check needs --out DIR")?;
            let kind = args
                .iter()
                .position(|arg| arg == "--kind")
                .and_then(|at| args.get(at + 1));
            crate::dev_art::run(&out, kind.map(String::as_str))?
        }
        Some("fits") => crate::dev_catalog::fits(args.get(1).context("dev fits needs a name")?),
        Some("save") => {
            let file = args.get(1).context("dev save needs a colony file")?;
            crate::dev_save::inspect(Path::new(file))?
        }
        Some("capture") => {
            let fixture = named(args.get(1))?;
            let out = path_option(args, "--out")?.context("dev capture needs --out PNG")?;
            capture(fixture, &out, now_option(args)?)?
        }
        _ => bail!(
            "usage: formiga-tools dev fixtures | fixture NAME --out DIR [--now-unix SECONDS] | \
             check-fixtures | capture NAME --out PNG [--now-unix SECONDS] | catalog | \
             art-check --out DIR [--kind KIND] | fits NAME | save FILE"
        ),
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report.get("success") == Some(&Value::Bool(false)) {
        std::process::exit(1);
    }
    Ok(())
}

fn named(name: Option<&String>) -> Result<&'static Fixture> {
    let name = name.context("name a fixture; `dev fixtures` lists them")?;
    FIXTURES
        .iter()
        .find(|fixture| fixture.name == name)
        .with_context(|| format!("no fixture called {name:?}; `dev fixtures` lists them"))
}

fn path_option(args: &[String], flag: &str) -> Result<Option<PathBuf>> {
    match args.iter().position(|arg| arg == flag) {
        None => Ok(None),
        Some(index) => Ok(Some(PathBuf::from(
            args.get(index + 1)
                .with_context(|| format!("{flag} needs a value"))?,
        ))),
    }
}

fn now_option(args: &[String]) -> Result<OffsetDateTime> {
    match args.iter().position(|arg| arg == "--now-unix") {
        None => Ok(FIXED_NOW),
        Some(index) => {
            let seconds: i64 = args
                .get(index + 1)
                .context("--now-unix needs a number of seconds")?
                .parse()
                .context("--now-unix needs a whole number of seconds")?;
            Ok(OffsetDateTime::from_unix_timestamp(seconds)?)
        }
    }
}

fn seed(name: &str) -> [u8; 32] {
    let mut seed = [0_u8; 32];
    seed.copy_from_slice(&Sha256::digest(format!("formiga-dev-fixture-{name}")));
    seed
}

fn grown(name: &str, days: i64, now: OffsetDateTime, desktop: &DesktopSnapshot) -> World {
    let mut world = World::new(seed(name), now - Duration::days(days), desktop);
    world.tick(now, 0.05, desktop);
    world.save.companion.onboarding_complete = true;
    for (creature, name) in world.save.creatures.iter_mut().zip(NAMES) {
        creature.name = name.into();
    }
    world
}

fn new_colony(now: OffsetDateTime, desktop: &DesktopSnapshot) -> World {
    grown("new-colony", 0, now, desktop)
}

fn young_colony(now: OffsetDateTime, desktop: &DesktopSnapshot) -> World {
    grown("young-colony", 10, now, desktop)
}

fn mature_colony(now: OffsetDateTime, desktop: &DesktopSnapshot) -> World {
    grown("mature-colony", 181, now, desktop)
}

/// The settings review's colony (`formiga-desktop/src/settings_review.rs`, `fixture`), which is
/// test-only there: forty days in, a friend at the door, a full guest book with two favourites
/// kept, and every decoration there is. That fixture also places all twenty kinds of colony object,
/// more than a save may hold; here the colony keeps the first `MAX_COLONY_OBJECTS` kinds, so the
/// file it writes is one Formiga would itself have written.
fn full_village(now: OffsetDateTime, desktop: &DesktopSnapshot) -> World {
    let mut world = grown("full-village", 40, now, desktop);
    let founder = &mut world.save.creatures[0];
    founder.memory.times_petted = 42;
    founder.memory.window_climbs = 18;
    founder.memory.discoveries_found = 7;
    let friend = SharedCreatureSeed {
        source_colony_seed: [70; 32],
        source_generation: 1,
        design: Some(CreatureDesign::generated([70; 32], 1, None)),
    };
    world
        .invite_visitor(friend, now, desktop)
        .expect("a fresh colony has room for a guest");
    for index in 0..MAX_GUEST_BOOK_ENTRIES {
        world.save.visitors.guest_book.push(GuestBookEntry {
            visited_at_utc: now - Duration::days((MAX_GUEST_BOOK_ENTRIES - index) as i64),
            name: format!("Wanderer {index}"),
            origin: CreatureOrigin {
                design: None,
                source_colony_seed: [index as u8; 32],
                source_generation: index as u8 % 4,
            },
            source: if index % 2 == 0 {
                VisitorSource::Wanderer
            } else {
                VisitorSource::Invited
            },
        });
    }
    let wanderer = world.save.visitors.guest_book[2].clone();
    world
        .save
        .visitors
        .keep_favorite(&wanderer.name, wanderer.origin, now - Duration::days(9))
        .expect("the guest book has room for a favourite");
    world.save.home.unlocks.decorations = ShelterDecorationKind::ALL.to_vec();
    world.save.objects.objects = ColonyObjectKind::ALL
        .iter()
        .take(MAX_COLONY_OBJECTS)
        .enumerate()
        .map(|(index, kind)| ColonyObject {
            id: index as u64,
            kind: *kind,
            role: kind.default_role(),
            ..Default::default()
        })
        .collect();
    world
}

/// What a fixture holds, in the terms a developer asks about.
fn summary(fixture: &Fixture, save: &SaveFile) -> Value {
    json!({
        "name": fixture.name,
        "about": fixture.about,
        "save_version": save.save_version,
        "creatures": save
            .creatures
            .iter()
            .map(|creature| json!({
                "name": creature.name,
                "family": format!("{:?}", creature.appearance.family),
                "generation": creature.generation,
            }))
            .collect::<Vec<_>>(),
        "decorations_unlocked": save.home.unlocks.decorations.len(),
        "objects": save.objects.objects.len(),
        "guest_visiting": save.visitors.guest.is_some(),
        "guest_book_entries": save.visitors.guest_book.len(),
        "journal_entries": save.companion.journal.len(),
    })
}

fn fixtures() -> Value {
    let desktop = fixture_desktop();
    json!({
        "success": true,
        "fixtures": FIXTURES
            .iter()
            .map(|fixture| summary(fixture, &(fixture.grow)(FIXED_NOW, &desktop).save))
            .collect::<Vec<_>>(),
    })
}

fn write_fixture(fixture: &Fixture, out: &Path, now: OffsetDateTime) -> Result<Value> {
    let world = (fixture.grow)(now, &fixture_desktop());
    let store = SaveStore::new(out.join("colony.json"));
    store
        .save(&world.save)
        .with_context(|| format!("write {}", store.path().display()))?;
    let mut report = summary(fixture, &world.save);
    report["success"] = json!(true);
    report["path"] = json!(store.path());
    report["grown_to"] = json!(now.unix_timestamp());
    Ok(report)
}

/// Every fixture, held to what `soak` holds a colony to.
fn check_fixtures() -> Value {
    let desktop = fixture_desktop();
    let scratch = std::env::temp_dir().join(format!("formiga-dev-check-{}", std::process::id()));
    let results: Vec<Value> = FIXTURES
        .iter()
        .map(|fixture| {
            let problems = std::panic::catch_unwind(|| check(fixture, &desktop, &scratch))
                .unwrap_or_else(|panic| {
                    let message = panic
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                        .unwrap_or_else(|| "panicked".into());
                    vec![format!("panicked: {message}")]
                });
            json!({ "name": fixture.name, "ok": problems.is_empty(), "problems": problems })
        })
        .collect();
    let _ = std::fs::remove_dir_all(&scratch);
    json!({
        "success": results.iter().all(|result| result["ok"] == json!(true)),
        "fixtures": results,
    })
}

fn check(fixture: &Fixture, desktop: &DesktopSnapshot, scratch: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    let mut world = (fixture.grow)(FIXED_NOW, desktop);
    problems.extend(
        violations(&world.save)
            .into_iter()
            .map(|violation| format!("as grown: {violation}")),
    );
    let store = SaveStore::new(scratch.join(fixture.name).join("colony.json"));
    match soak::reload(&world, &store) {
        Ok(read) => world = read,
        Err(error) => problems.push(format!("save and reload: {error:#}")),
    }
    // Ten seconds of running, at the desktop's cadence.
    for tick in 1..=200 {
        world.tick(FIXED_NOW + Duration::milliseconds(50 * tick), 0.05, desktop);
    }
    problems.extend(
        violations(&world.save)
            .into_iter()
            .map(|violation| format!("after ten seconds of running: {violation}")),
    );
    problems
}

/// The colony card, then every resident's card, two to a row.
fn capture(fixture: &Fixture, out: &Path, now: OffsetDateTime) -> Result<Value> {
    let world = (fixture.grow)(now, &fixture_desktop());
    let save = &world.save;
    let cards = 1 + save.creatures.len() as u32;
    let width = COLONY_CARD_WIDTH.max(CARD_WIDTH) * 2;
    let row_height = COLONY_CARD_HEIGHT.max(CARD_HEIGHT);
    let height = row_height * cards.div_ceil(2);
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[38, 32, 44, 255]);
    }
    let mut drawn = vec![json!({ "what": "colony card" })];
    blit_canvas_scaled(
        &mut pixels,
        width,
        0,
        0,
        &ColonyCardRenderer::render(save),
        1,
    );
    for (index, creature) in save.creatures.iter().enumerate() {
        let slot = index as u32 + 1;
        blit_canvas_scaled(
            &mut pixels,
            width,
            (slot % 2) * width / 2,
            (slot / 2) * row_height,
            &CreatureCardRenderer::render(creature),
            1,
        );
        drawn.push(json!({ "what": "creature card", "name": creature.name }));
    }
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_png(out, width, height, &pixels)?;
    Ok(json!({
        "success": true,
        "fixture": fixture.name,
        "path": out,
        "width": width,
        "height": height,
        "drawn": drawn,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fixture_passes_its_own_check() {
        let report = check_fixtures();
        assert_eq!(report["success"], json!(true), "{report:#}");
    }

    #[test]
    fn the_same_fixture_is_the_same_colony_every_time() {
        let desktop = fixture_desktop();
        for fixture in &FIXTURES {
            let first = serde_json::to_value(&(fixture.grow)(FIXED_NOW, &desktop).save).unwrap();
            let second = serde_json::to_value(&(fixture.grow)(FIXED_NOW, &desktop).save).unwrap();
            assert_eq!(
                first, second,
                "{} changed between two growings",
                fixture.name
            );
        }
    }

    #[test]
    fn fixture_names_are_unique() {
        let mut names: Vec<_> = FIXTURES.iter().map(|fixture| fixture.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), FIXTURES.len());
    }
}
