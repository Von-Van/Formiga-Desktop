//! A numbered contact sheet of freshly generated companions, for judging what the generator makes
//! by eye: one idle frame each, numbered in reading order, with a table of what each one is made
//! of. Rate them by number and hand the ratings back with `--ratings`, and the report shows which
//! archetypes, faces and parts turn up among the ones rated bad more than they turn up overall.
//!
//! Everything is drawn from fixed seeds, so the same arguments always make the same sheet.

use crate::{blit_scaled_square_alpha, fill_gradient, fixture_desktop, write_png};
use anyhow::{Context, Result, bail};
use formiga_art::{
    BodyClip, CreatureRenderer, ExpressionKind, EyelidPose, FRAME_SIZE, FaceRenderState,
    GazeDirection,
};
use formiga_core::{
    ActionKind, CreatureDesign, Edition, SeedStream, Strangeness, World, apply_creature_design,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

const SCALE: u32 = 3;
const COLUMNS: u32 = 15;
/// Room under each drawing for its number.
const LABEL: u32 = 22;

pub(crate) struct Options {
    pub(crate) count: u32,
    pub(crate) seed: u64,
    pub(crate) edition: Edition,
    pub(crate) ratings: Option<PathBuf>,
}

pub(crate) fn options(args: &[String]) -> Result<Options> {
    let value = |name: &str| {
        args.windows(2)
            .find(|window| window[0] == name)
            .map(|window| window[1].clone())
    };
    let count = match value("--count") {
        Some(count) => count.parse().context("--count takes a number")?,
        None => 150,
    };
    if !(1..=400).contains(&count) {
        bail!("--count takes 1 to 400");
    }
    let edition = match value("--edition").as_deref() {
        None | Some("details") => Edition::Details,
        Some("archetypes") => Edition::Archetypes,
        Some("original") => Edition::Original,
        Some(other) => bail!("--edition is details, archetypes or original, not {other}"),
    };
    Ok(Options {
        count,
        seed: match value("--seed") {
            Some(seed) => seed.parse().context("--seed takes a number")?,
            None => 1,
        },
        edition,
        ratings: value("--ratings").map(PathBuf::from),
    })
}

/// One companion on the sheet: its number, its recipe, and how the generator came to pick it.
struct Entry {
    number: u32,
    design: CreatureDesign,
    strangeness: Option<Strangeness>,
}

fn entries(options: &Options) -> Vec<Entry> {
    let streams = SeedStream::new(options.seed.to_le_bytes().repeat(4).try_into().unwrap());
    (0..options.count)
        .map(|index| {
            let seed = streams.bytes("cuteness-sheet", u64::from(index));
            let (design, strangeness) = match options.edition {
                Edition::Original => (
                    CreatureDesign::generated_by(Edition::Original, seed, 0, None),
                    None,
                ),
                Edition::Archetypes => {
                    let (design, strangeness) = CreatureDesign::drawn(seed, 0);
                    (design, Some(strangeness))
                }
                Edition::Details => {
                    let (design, strangeness) = CreatureDesign::detailed(seed, 0);
                    (design, Some(strangeness))
                }
            };
            Entry {
                number: index + 1,
                design,
                strangeness,
            }
        })
        .collect()
}

pub(crate) fn run(path: PathBuf, options: Options) -> Result<()> {
    let entries = entries(&options);
    let rows = options.count.div_ceil(COLUMNS);
    let cell_width = FRAME_SIZE * SCALE;
    let cell_height = cell_width + LABEL;
    let (width, height) = (cell_width * COLUMNS, cell_height * rows);
    let mut pixels = vec![0; (width * height * 4) as usize];
    fill_gradient(
        &mut pixels,
        width,
        height,
        [240, 236, 226, 255],
        [214, 228, 221, 255],
    );
    let mut creature =
        World::preview_adult([13; 32], OffsetDateTime::UNIX_EPOCH, &fixture_desktop());
    let state = FaceRenderState {
        expression: ExpressionKind::Neutral,
        eyelids: EyelidPose::Open,
        gaze: GazeDirection::default(),
    };
    for entry in &entries {
        apply_creature_design(&mut creature, Some(entry.design));
        let canvas = CreatureRenderer::render_composited_frame(
            &creature.appearance,
            BodyClip::Action(ActionKind::Idle),
            0,
            true,
            false,
            state,
        );
        let index = entry.number - 1;
        let (x, y) = (index % COLUMNS * cell_width, index / COLUMNS * cell_height);
        blit_scaled_square_alpha(
            &mut pixels,
            width,
            x,
            y,
            &canvas.rgba_bytes(),
            FRAME_SIZE,
            SCALE,
        );
        crate::social_preview::draw_text(
            &mut pixels,
            width as i32,
            height as i32,
            x as i32 + 6,
            (y + cell_width + 4) as i32,
            &entry.number.to_string(),
            2,
            [52, 64, 58, 255],
        );
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {} companions to {}", entries.len(), path.display());
    println!("#\tclass\tarchetype\tcoherence\tbody\tears\tear size\ttail\tface\tmarking\tclassic");
    for entry in &entries {
        let d = entry.design;
        println!(
            "{}\t{}\t{}\t{:.2}\t{:?}\t{:?}\t{}\t{}\t{}\t{}\t{:?}",
            entry.number,
            entry.strangeness.map_or("original", Strangeness::label),
            d.body_archetype()
                .map_or("-", |archetype| archetype.label()),
            d.coherence(),
            d.body,
            d.ears,
            d.ear_size,
            d.tail,
            d.face_template,
            d.marking,
            d.classic,
        );
    }
    if let Some(ratings) = &options.ratings {
        report(&entries, ratings)?;
    }
    Ok(())
}

/// Ratings from a plain text file: lines that start `good:`, `ok:` or `bad:`, each followed by
/// numbers from the sheet, separated by spaces or commas.
fn read_ratings(path: &Path) -> Result<BTreeMap<u32, &'static str>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read ratings from {}", path.display()))?;
    let mut ratings = BTreeMap::new();
    for line in text.lines() {
        let Some((label, numbers)) = line.split_once(':') else {
            continue;
        };
        let rating = match label.trim().to_ascii_lowercase().as_str() {
            "good" => "good",
            "ok" | "acceptable" => "ok",
            "bad" => "bad",
            _ => continue,
        };
        for number in numbers.split(|c: char| c == ',' || c.is_whitespace()) {
            if number.is_empty() {
                continue;
            }
            ratings.insert(number.parse().context("a rating names a number")?, rating);
        }
    }
    Ok(ratings)
}

/// Which parts turn up among the companions rated bad, against how often they turn up at all.
fn report(entries: &[Entry], path: &Path) -> Result<()> {
    let ratings = read_ratings(path)?;
    let rated = |rating: &str| {
        entries
            .iter()
            .filter(|entry| ratings.get(&entry.number) == Some(&rating))
            .count()
    };
    println!(
        "\nratings: {} good, {} ok, {} bad, {} unrated",
        rated("good"),
        rated("ok"),
        rated("bad"),
        entries.len() - ratings.len().min(entries.len())
    );
    let traits = |entry: &Entry| {
        let d = entry.design;
        let mut traits = vec![
            format!(
                "class {}",
                entry.strangeness.map_or("original", Strangeness::label)
            ),
            format!(
                "archetype {}",
                d.body_archetype()
                    .map_or("-", |archetype| archetype.label())
            ),
            format!("body {:?}", d.body),
            format!("ears {:?}", d.ears),
            format!("tail {}", d.tail),
            format!("face {}", d.face_template),
            format!("marking {}", d.marking),
        ];
        let k = d.classic;
        for (name, value) in [
            ("candy coat", k.coat),
            ("classic face", k.face),
            ("classic limbs", k.limbs),
            ("crown", k.crown),
            ("pattern", k.pattern),
            ("classic tail", k.tail),
        ] {
            if value > 0 {
                traits.push(format!("{name} {value}"));
            }
        }
        traits
    };
    let mut seen: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    for entry in entries {
        for name in traits(entry) {
            let counts = seen.entry(name).or_default();
            counts.0 += 1;
            match ratings.get(&entry.number) {
                Some(&"bad") => counts.1 += 1,
                Some(&"good") => counts.2 += 1,
                _ => {}
            }
        }
    }
    let bad_total = rated("bad").max(1) as f32;
    let all = entries.len() as f32;
    let mut lines: Vec<(f32, String)> = seen
        .into_iter()
        .filter(|(_, (count, _, _))| *count >= 3)
        .map(|(name, (count, bad, good))| {
            // How much more often the part turns up among the bad ones than overall.
            let lift = (bad as f32 / bad_total) / (count as f32 / all);
            (
                lift,
                format!("{name:<22}{count:>5} seen{bad:>5} bad{good:>5} good   x{lift:.2}"),
            )
        })
        .collect();
    lines.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("\nparts by how much more often they are rated bad than they appear:");
    for (_, line) in lines {
        println!("  {line}");
    }
    Ok(())
}

/// The poses a companion's temperament strikes — a huff, a swoon, begging, a strut, a peek and a
/// stomp — one row each, every frame of each, on the three original bodies and one new companion
/// of every body plan, each wearing the face its moment wears.
pub(crate) fn temperament_sheet(path: PathBuf) -> Result<()> {
    use formiga_core::{BodyPlan, Gesture};
    const GESTURES: [Gesture; 6] = [
        Gesture::Huff,
        Gesture::Swoon,
        Gesture::Beg,
        Gesture::Strut,
        Gesture::Peek,
        Gesture::Stomp,
    ];
    let mut bodies: Vec<_> = crate::reference_creatures()
        .into_iter()
        .map(|mut creature| {
            apply_creature_design(&mut creature, None);
            creature.appearance
        })
        .collect();
    let streams = SeedStream::new([61; 32]);
    for plan in BodyPlan::ALL {
        let (design, _) = (0_u64..)
            .map(|index| CreatureDesign::drawn(streams.bytes("temperament-sheet", index), 0))
            .find(|(design, strangeness)| design.body == plan && *strangeness == Strangeness::Cute)
            .expect("every body plan is drawn");
        let mut creature =
            World::preview_adult([13; 32], OffsetDateTime::UNIX_EPOCH, &fixture_desktop());
        apply_creature_design(&mut creature, Some(design));
        bodies.push(creature.appearance);
    }
    let most_frames = GESTURES
        .iter()
        .map(|gesture| u32::from(formiga_art::AnimationSpec::for_clip(*gesture).frames))
        .max()
        .unwrap_or(1);
    let cell = FRAME_SIZE * SCALE;
    let gap = cell / 3;
    let width = bodies.len() as u32 * (most_frames * cell + gap);
    let height = GESTURES.len() as u32 * cell;
    let mut pixels = vec![0; (width * height * 4) as usize];
    fill_gradient(
        &mut pixels,
        width,
        height,
        [240, 236, 226, 255],
        [214, 228, 221, 255],
    );
    for (row, gesture) in GESTURES.into_iter().enumerate() {
        let frames = formiga_art::AnimationSpec::for_clip(gesture).frames;
        for (column, genome) in bodies.iter().enumerate() {
            for frame in 0..frames {
                let canvas = CreatureRenderer::render_composited_frame(
                    genome,
                    BodyClip::Gesture(gesture),
                    frame,
                    true,
                    false,
                    crate::gesture_face(gesture),
                );
                blit_scaled_square_alpha(
                    &mut pixels,
                    width,
                    column as u32 * (most_frames * cell + gap) + u32::from(frame) * cell,
                    row as u32 * cell,
                    &canvas.rgba_bytes(),
                    FRAME_SIZE,
                    SCALE,
                );
            }
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// The authored faces a new companion can wear: one column per layout, and a row for each
/// expression and a blink, on one plain round body so only the face changes from column to column.
pub(crate) fn face_sheet(path: PathBuf) -> Result<()> {
    use formiga_core::{BodyPlan, FACE_TEMPLATES};
    // Every expression with the eyes open, then a blink.
    let rows: Vec<(ExpressionKind, EyelidPose)> = ExpressionKind::ALL
        .into_iter()
        .map(|expression| (expression, EyelidPose::Open))
        .chain([(ExpressionKind::Neutral, EyelidPose::Closed)])
        .collect();
    let mut creature =
        World::preview_adult([13; 32], OffsetDateTime::UNIX_EPOCH, &fixture_desktop());
    let (base, _) = CreatureDesign::drawn([13; 32], 0);
    let cell = FRAME_SIZE * SCALE;
    let (width, height) = (u32::from(FACE_TEMPLATES) * cell, rows.len() as u32 * cell);
    let mut pixels = vec![0; (width * height * 4) as usize];
    fill_gradient(
        &mut pixels,
        width,
        height,
        [240, 236, 226, 255],
        [214, 228, 221, 255],
    );
    for template in 1..=FACE_TEMPLATES {
        apply_creature_design(
            &mut creature,
            Some(CreatureDesign {
                body: BodyPlan::Round,
                face_template: template,
                classic: formiga_core::ClassicParts::default(),
                marking: 1,
                head: 9,
                width: 10,
                coat: [238, 198, 184],
                accent: [236, 128, 146],
                ..base
            }),
        );
        for (row, &(expression, eyelids)) in rows.iter().enumerate() {
            let canvas = CreatureRenderer::render_composited_frame(
                &creature.appearance,
                BodyClip::Action(ActionKind::Idle),
                0,
                true,
                false,
                FaceRenderState {
                    expression,
                    eyelids,
                    gaze: GazeDirection::default(),
                },
            );
            blit_scaled_square_alpha(
                &mut pixels,
                width,
                u32::from(template - 1) * cell,
                row as u32 * cell,
                &canvas.rgba_bytes(),
                FRAME_SIZE,
                SCALE,
            );
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}
