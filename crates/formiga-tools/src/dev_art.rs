//! `formiga-tools dev art-check --out DIR`: every item drawn in every pose, and looked over.
//!
//!   cargo run -p formiga-tools -- dev art-check --out .dev/art [--kind KIND]
//!
//! Each kind is drawn the way the app draws it, through the art crate's own renderers, each item
//! into a frame of its own: village pieces and colony objects into their sheet cells, every
//! decoration on every type of house by day and lit after dark, every find resting and glinting,
//! every souvenir, every frame of every wonder, every accessory on six bodies in every clip, and
//! every kind of body the generator makes in every frame of every clip. Each drawing is then
//! checked for three things:
//!
//!   blank       nothing drawn at all (an error), or an accessory that never shows (an error)
//!   edge        the drawing reaches the edge of its frame, where anything further is cut off
//!   look-alike  two items of a kind drawn the same, pixel for pixel (an error), or nearly so
//!
//! Where art already sits on its frame's edge by design (a house on its ground line), only the
//! other edges count. One sheet per kind is written to DIR, with what was flagged outlined: red
//! for an error, amber for a warning. The JSON lists every problem with the item and pose.

use crate::accessory_sheet::{posed, subjects};
use anyhow::Result;
use formiga_art::*;
use formiga_core::*;
use serde_json::{Value, json};
use std::path::Path;
use time::OffsetDateTime;

/// The colony every drawing is coloured for, so a run is the same every time.
const SEED: [u8; 32] = [42; 32];
/// Two drawings of a kind that differ in fewer than this share of the pixels either covers are
/// reported as looking nearly the same.
const NEARLY_THE_SAME: f32 = 0.04;
/// How many recipes the generator is asked for when looking for every kind of body.
const BODY_SAMPLES: u32 = 4_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Warning,
    Error,
}

impl Level {
    fn name(self) -> &'static str {
        match self {
            Level::Warning => "warning",
            Level::Error => "error",
        }
    }
}

/// Which edges of its frame a drawing may sit on by design.
#[derive(Clone, Copy)]
struct Edges {
    top: bool,
    bottom: bool,
    left: bool,
    right: bool,
}

const NO_EDGES: Edges = Edges {
    top: false,
    bottom: false,
    left: false,
    right: false,
};
const GROUND: Edges = Edges {
    bottom: true,
    ..NO_EDGES
};

/// One drawing of one item in one pose.
struct Drawing {
    item: String,
    pose: String,
    canvas: Canvas,
    /// Where this drawing is only part of the frame (an accessory on a body), the pixels it put
    /// there; otherwise everything drawn counts.
    part: Option<Vec<bool>>,
    /// Whether the sheet shows it when nothing was flagged.
    shown: bool,
}

struct Problem {
    item: String,
    pose: Option<String>,
    level: Level,
    check: &'static str,
    message: String,
}

struct KindReport {
    kind: &'static str,
    frame: (u32, u32),
    drawings: Vec<Drawing>,
    problems: Vec<Problem>,
}

/// Draws one kind and checks every drawing.
type KindCheck = fn() -> KindReport;

pub fn run(out: &Path, only: Option<&str>) -> Result<Value> {
    std::fs::create_dir_all(out)?;
    let kinds: [(&str, KindCheck); 13] = [
        ("object", objects),
        ("hangout", hangouts),
        ("garden", gardens),
        ("ornament", ornaments),
        ("decoration", decorations),
        ("house-style", house_styles),
        ("palette", palettes),
        ("trinket", trinkets),
        ("souvenir", souvenirs),
        ("wonder", wonders),
        ("accessory", accessories),
        ("body", bodies),
        ("face", faces),
    ];
    if let Some(only) = only
        && !kinds.iter().any(|(key, _)| *key == only)
    {
        anyhow::bail!(
            "no kind called {only}; kinds: {}",
            kinds.map(|(key, _)| key).join(", ")
        );
    }
    let mut reports = Vec::new();
    for (key, check) in kinds {
        if only.is_some_and(|only| only != key) {
            continue;
        }
        let report = check();
        let sheet = out.join(format!("{key}.png"));
        write_sheet(&report, &sheet)?;
        reports.push(json!({
            "kind": report.kind,
            "frame": [report.frame.0, report.frame.1],
            "drawings": report.drawings.len(),
            "items": items(&report),
            "sheet": sheet,
            "errors": report.problems.iter().filter(|p| p.level == Level::Error).count(),
            "warnings": report.problems.iter().filter(|p| p.level == Level::Warning).count(),
            "problems": report.problems.iter().map(|p| json!({
                "item": p.item,
                "pose": p.pose,
                "level": p.level.name(),
                "check": p.check,
                "message": p.message,
            })).collect::<Vec<_>>(),
        }));
    }
    let errors: u64 = reports
        .iter()
        .map(|r| r["errors"].as_u64().unwrap_or(0))
        .sum();
    Ok(json!({
        "success": errors == 0,
        "seed": hex(&SEED),
        "kinds": reports,
    }))
}

fn items(report: &KindReport) -> usize {
    let mut items: Vec<&str> = report.drawings.iter().map(|d| d.item.as_str()).collect();
    items.dedup();
    items.len()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The checks every kind gets: blank, edge, look-alike within each pose.
fn checked(
    kind: &'static str,
    frame: (u32, u32),
    edges: Edges,
    drawings: Vec<Drawing>,
) -> KindReport {
    let mut problems = Vec::new();
    for drawing in &drawings {
        let mask = covered(drawing);
        if !mask.iter().any(|on| *on) {
            problems.push(Problem {
                item: drawing.item.clone(),
                pose: Some(drawing.pose.clone()),
                level: Level::Error,
                check: "blank",
                message: "nothing is drawn".into(),
            });
            continue;
        }
        let touched = touching(
            &mask,
            drawing.canvas.width(),
            drawing.canvas.height(),
            edges,
        );
        if !touched.is_empty() {
            problems.push(Problem {
                item: drawing.item.clone(),
                pose: Some(drawing.pose.clone()),
                level: Level::Warning,
                check: "edge",
                message: format!(
                    "reaches the {} edge of its {}×{} frame; anything further is cut off",
                    touched.join(" and "),
                    drawing.canvas.width(),
                    drawing.canvas.height()
                ),
            });
        }
    }
    problems.extend(look_alikes(&drawings));
    KindReport {
        kind,
        frame,
        drawings,
        problems,
    }
}

fn covered(drawing: &Drawing) -> Vec<bool> {
    drawing
        .part
        .clone()
        .unwrap_or_else(|| drawing.canvas.pixels().iter().map(|p| p.a > 0).collect())
}

fn touching(mask: &[bool], width: u32, height: u32, allowed: Edges) -> Vec<&'static str> {
    let at = |x: u32, y: u32| mask[(y * width + x) as usize];
    let mut sides = Vec::new();
    if !allowed.top && (0..width).any(|x| at(x, 0)) {
        sides.push("top");
    }
    if !allowed.bottom && (0..width).any(|x| at(x, height - 1)) {
        sides.push("bottom");
    }
    if !allowed.left && (0..height).any(|y| at(0, y)) {
        sides.push("left");
    }
    if !allowed.right && (0..height).any(|y| at(width - 1, y)) {
        sides.push("right");
    }
    sides
}

/// Items drawn the same in the same pose. Only different items are compared, and only once.
fn look_alikes(drawings: &[Drawing]) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut reported: Vec<(String, String)> = Vec::new();
    for (index, a) in drawings.iter().enumerate() {
        for b in &drawings[index + 1..] {
            if a.item == b.item || a.pose != b.pose {
                continue;
            }
            let pair = (a.item.clone(), b.item.clone());
            if reported.contains(&pair) {
                continue;
            }
            let (differing, covered) = difference(a, b);
            if covered == 0 {
                continue;
            }
            let share = differing as f32 / covered as f32;
            let level = if differing == 0 {
                Level::Error
            } else if share < NEARLY_THE_SAME {
                Level::Warning
            } else {
                continue;
            };
            reported.push(pair);
            problems.push(Problem {
                item: a.item.clone(),
                pose: Some(a.pose.clone()),
                level,
                check: "look-alike",
                message: if differing == 0 {
                    format!("drawn exactly the same as {}", b.item)
                } else {
                    format!(
                        "drawn nearly the same as {}: {differing} of {covered} pixels differ",
                        b.item
                    )
                },
            });
        }
    }
    problems
}

/// Items drawn the same (an error) or nearly so (a warning) taken over every pose together,
/// for kinds where each item is drawn in the same poses in the same order.
fn alike_in_every_pose(drawings: &[Drawing]) -> Vec<Problem> {
    let mut items: Vec<(&str, Vec<&Drawing>)> = Vec::new();
    for drawing in drawings {
        match items.last_mut() {
            Some((item, poses)) if *item == drawing.item => poses.push(drawing),
            _ => items.push((&drawing.item, vec![drawing])),
        }
    }
    let mut problems = Vec::new();
    for (index, (a, poses_a)) in items.iter().enumerate() {
        for (b, poses_b) in &items[index + 1..] {
            let (mut differing, mut covered) = (0, 0);
            for (pose_a, pose_b) in poses_a.iter().zip(poses_b) {
                debug_assert_eq!(pose_a.pose, pose_b.pose);
                let (d, c) = difference(pose_a, pose_b);
                differing += d;
                covered += c;
            }
            if covered == 0 {
                continue;
            }
            let level = if differing == 0 {
                Level::Error
            } else if (differing as f32 / covered as f32) < NEARLY_THE_SAME {
                Level::Warning
            } else {
                continue;
            };
            problems.push(Problem {
                item: a.to_string(),
                pose: None,
                level,
                check: "look-alike",
                message: if differing == 0 {
                    format!("drawn exactly the same as {b} in every pose")
                } else {
                    format!(
                        "drawn nearly the same as {b} across every pose: {differing} of \
                         {covered} pixels differ"
                    )
                },
            });
        }
    }
    problems
}

fn difference(a: &Drawing, b: &Drawing) -> (usize, usize) {
    let (mask_a, mask_b) = (covered(a), covered(b));
    let mut differing = 0;
    let mut union = 0;
    for (index, (pa, pb)) in a.canvas.pixels().iter().zip(b.canvas.pixels()).enumerate() {
        let (in_a, in_b) = (mask_a[index], mask_b[index]);
        if !in_a && !in_b {
            continue;
        }
        union += 1;
        if in_a != in_b || pa != pb {
            differing += 1;
        }
    }
    (differing, union)
}

fn drawing(item: impl Into<String>, pose: impl Into<String>, canvas: Canvas) -> Drawing {
    Drawing {
        item: item.into(),
        pose: pose.into(),
        canvas,
        part: None,
        shown: true,
    }
}

fn cut(source: &Canvas, x: u32, y: u32, width: u32, height: u32) -> Canvas {
    let mut tile = Canvas::new(width, height);
    for ty in 0..height as i32 {
        for tx in 0..width as i32 {
            tile.set(tx, ty, source.get(x as i32 + tx, y as i32 + ty));
        }
    }
    tile
}

fn atlas_cell(atlas: &Canvas, cell: u32) -> Canvas {
    let (x, y) = ColonyObjectRenderer::cell_origin(cell);
    cut(atlas, x, y, COLONY_OBJECT_SIZE, COLONY_OBJECT_SIZE)
}

const CELL: (u32, u32) = (COLONY_OBJECT_SIZE, COLONY_OBJECT_SIZE);
/// Objects and village ground pieces are each drawn into a 16×16 tile of their own and fill it
/// edge to edge by design, with nothing past it to lose, so only blank and look-alike count.
const FILLED: Edges = Edges {
    top: true,
    bottom: true,
    left: true,
    right: true,
};

fn objects() -> KindReport {
    let atlas = ColonyObjectRenderer::render_atlas(SEED);
    let drawings = ColonyObjectKind::ALL
        .iter()
        .map(|&kind| {
            drawing(
                format!("{kind:?}"),
                "sheet cell",
                atlas_cell(&atlas, ColonyObjectRenderer::object_cell(kind)),
            )
        })
        .collect();
    checked("object", CELL, FILLED, drawings)
}

fn hangouts() -> KindReport {
    let atlas = ColonyObjectRenderer::render_atlas(SEED);
    let drawings = HangoutKind::ALL
        .iter()
        .map(|&kind| {
            drawing(
                format!("{kind:?}"),
                "sheet cell",
                atlas_cell(&atlas, ColonyObjectRenderer::hangout_cell(kind)),
            )
        })
        .collect();
    checked("hangout", CELL, FILLED, drawings)
}

fn gardens() -> KindReport {
    let atlas = ColonyObjectRenderer::render_atlas(SEED);
    let mut drawings = Vec::new();
    for kind in GardenKind::ALL {
        for stage in GardenStage::ALL {
            drawings.push(drawing(
                format!("{kind:?}"),
                format!("{stage:?}"),
                atlas_cell(&atlas, ColonyObjectRenderer::garden_cell(kind, stage)),
            ));
        }
    }
    checked("garden", CELL, FILLED, drawings)
}

fn ornaments() -> KindReport {
    let atlas = ColonyObjectRenderer::render_atlas(SEED);
    let drawings = OrnamentKind::ALL
        .iter()
        .map(|&kind| {
            drawing(
                format!("{kind:?}"),
                "sheet cell",
                atlas_cell(&atlas, ColonyObjectRenderer::ornament_cell(kind)),
            )
        })
        .collect();
    checked("ornament", CELL, FILLED, drawings)
}

/// One cottage of `style` wearing `decorations`, by day or lit after dark: the narrowest house,
/// where a decoration has least room, cut from the village the way the app cuts it.
fn cottage(
    home: &ColonyHome,
    style: ShelterStyle,
    decorations: Vec<ShelterDecorationKind>,
    lit: bool,
) -> Canvas {
    let village = ShelterRenderer::render_village(
        &home.shelter,
        &[Vec::new(), decorations],
        &[],
        &[home.shelter.style, style],
        lit,
    );
    let (x, y) = ShelterRenderer::village_cell(VillageCell::House {
        slot: 1,
        lit,
        occupied: false,
    });
    cut(&village, x, y, SHELTER_SIZE, SHELTER_SIZE)
}

/// Each decoration is what it adds to a bare cottage: every house type, by day and after dark.
fn decorations() -> KindReport {
    let home = ColonyHome::from_seed(SEED, None, None, None);
    let bare: Vec<(ShelterStyle, bool, Canvas)> = ShelterStyle::ALL
        .into_iter()
        .flat_map(|style| {
            [false, true].map(|lit| (style, lit, cottage(&home, style, Vec::new(), lit)))
        })
        .collect();
    let mut drawings = Vec::new();
    for kind in ShelterDecorationKind::ALL {
        for (style, lit, bare) in &bare {
            let (style, lit) = (*style, *lit);
            {
                let dressed = cottage(&home, style, vec![kind], lit);
                let part = dressed
                    .pixels()
                    .iter()
                    .zip(bare.pixels())
                    .map(|(d, b)| d != b)
                    .collect();
                drawings.push(Drawing {
                    item: format!("{kind:?}"),
                    pose: format!("{style:?}{}", if lit { ", lit" } else { "" }),
                    canvas: dressed,
                    part: Some(part),
                    shown: true,
                });
            }
        }
    }
    let mut report = checked("decoration", (SHELTER_SIZE, SHELTER_SIZE), GROUND, drawings);
    // A decoration that adds nothing to one type of house but shows on another is hidden
    // there, not missing: a warning for each type. One that never shows at all is one error.
    let poses = ShelterStyle::ALL.len() * 2;
    let (blank, mut rest): (Vec<Problem>, Vec<Problem>) =
        report.problems.drain(..).partition(|p| p.check == "blank");
    for kind in ShelterDecorationKind::ALL {
        let item = format!("{kind:?}");
        let mine: Vec<Problem> = blank
            .iter()
            .filter(|p| p.item == item)
            .map(|p| Problem {
                item: p.item.clone(),
                pose: p.pose.clone(),
                level: Level::Warning,
                check: "blank",
                message: "adds nothing to this type of house".into(),
            })
            .collect();
        if mine.len() == poses {
            rest.push(Problem {
                item,
                pose: None,
                level: Level::Error,
                check: "blank",
                message: "adds nothing to any type of house, by day or lit".into(),
            });
        } else {
            rest.extend(mine);
        }
    }
    report.problems = rest;
    report
}

fn house_styles() -> KindReport {
    let home = ColonyHome::from_seed(SEED, None, None, None);
    let mut drawings = Vec::new();
    for style in ShelterStyle::ALL {
        for lit in [false, true] {
            drawings.push(drawing(
                format!("{style:?}"),
                if lit { "lit" } else { "by day" },
                cottage(&home, style, Vec::new(), lit),
            ));
        }
    }
    checked(
        "house-style",
        (SHELTER_SIZE, SHELTER_SIZE),
        GROUND,
        drawings,
    )
}

fn palettes() -> KindReport {
    let mut drawings = Vec::new();
    for palette in VillagePalette::ALL {
        let mut home = ColonyHome::from_seed(SEED, None, None, None);
        home.palette = Some(palette);
        let genome = home.drawn_shelter();
        drawings.push(drawing(
            format!("{palette:?}"),
            "colony house",
            ShelterRenderer::render(&genome),
        ));
    }
    checked("palette", (SHELTER_SIZE, SHELTER_SIZE), GROUND, drawings)
}

fn trinkets() -> KindReport {
    let mut drawings = Vec::new();
    for variant in 0..TRINKET_VARIANTS {
        let ink = TrinketAtlasRenderer::ink(SEED, &[], variant);
        for (frame, pose) in [
            (TRINKET_FRAME_REST, "resting"),
            (TRINKET_FRAME_GLINT, "glinting"),
        ] {
            let mut canvas = Canvas::new(TRINKET_CELL, TRINKET_CELL);
            draw_trinket(&mut canvas, ink, variant, frame, 0, 0);
            drawings.push(drawing(variant.to_string(), pose, canvas));
        }
    }
    checked("trinket", (TRINKET_CELL, TRINKET_CELL), NO_EDGES, drawings)
}

/// Souvenirs are drawn on a frame one icon wider on every side, so a picture that runs past its
/// icon is seen doing so rather than being cut.
fn souvenirs() -> KindReport {
    let mut drawings = Vec::new();
    let mut problems = Vec::new();
    let size = SOUVENIR_ICON * 3;
    for souvenir in Souvenir::ALL {
        let mut canvas = Canvas::new(size, size);
        draw_souvenir(
            &mut canvas,
            souvenir,
            SOUVENIR_ICON as i32,
            SOUVENIR_ICON as i32,
        );
        let outside = canvas
            .pixels()
            .iter()
            .enumerate()
            .filter(|(index, pixel)| {
                let (x, y) = (*index as u32 % size, *index as u32 / size);
                let inside = (SOUVENIR_ICON..2 * SOUVENIR_ICON).contains(&x)
                    && (SOUVENIR_ICON..2 * SOUVENIR_ICON).contains(&y);
                pixel.a > 0 && !inside
            })
            .count();
        if outside > 0 {
            problems.push(Problem {
                item: souvenir.id().into(),
                pose: None,
                level: Level::Error,
                check: "edge",
                message: format!(
                    "{outside} pixels fall outside its {SOUVENIR_ICON}×{SOUVENIR_ICON} icon"
                ),
            });
        }
        drawings.push(drawing(
            souvenir.id(),
            "icon",
            cut(
                &canvas,
                SOUVENIR_ICON,
                SOUVENIR_ICON,
                SOUVENIR_ICON,
                SOUVENIR_ICON,
            ),
        ));
    }
    // A picture filling its icon edge to edge is how souvenirs are drawn; only spilling counts.
    let mut report = checked("souvenir", (SOUVENIR_ICON, SOUVENIR_ICON), FILLED, drawings);
    report.problems.extend(problems);
    report
}

fn wonders() -> KindReport {
    let mut drawings = Vec::new();
    for kind in WonderKind::ALL {
        let strip = WonderRenderer::render(kind, SEED);
        for frame in 0..wonder_frames(kind) {
            drawings.push(drawing(
                format!("{kind:?}"),
                format!("frame {frame}"),
                cut(
                    &strip,
                    frame * WONDER_CELL_WIDTH,
                    0,
                    WONDER_CELL_WIDTH,
                    WONDER_CELL_HEIGHT,
                ),
            ));
        }
    }
    // Different wonders have different numbers of frames, so only first frames are compared.
    let mut report = checked(
        "wonder",
        (WONDER_CELL_WIDTH, WONDER_CELL_HEIGHT),
        GROUND,
        drawings,
    );
    report
        .problems
        .retain(|p| p.check != "look-alike" || p.pose.as_deref() == Some("frame 0"));
    report
}

/// Every clip a body has frames of, and its first frame: what the accessory check dresses.
fn clips() -> Vec<BodyClip> {
    BodyClip::baked().collect()
}

fn clip_name(clip: BodyClip) -> String {
    match clip {
        BodyClip::Action(action) => format!("{action:?}"),
        BodyClip::Gesture(gesture) => format!("{gesture:?}"),
    }
}

/// The clips an accessory is shown in on its sheet: the poses it is seen in most.
const SHEET_CLIPS: [BodyClip; 4] = [
    BodyClip::Action(ActionKind::Idle),
    BodyClip::Action(ActionKind::Traverse),
    BodyClip::Action(ActionKind::Sleep),
    BodyClip::Gesture(Gesture::Cheer),
];

/// Every accessory on six bodies, in the first frame of every clip, facing either way. What an
/// accessory adds is the pixels the dressed figure differs from the same figure undressed.
fn accessories() -> KindReport {
    let subjects = subjects();
    let members: Vec<Palette> = subjects
        .iter()
        .map(|creature| palette_for(&creature.appearance))
        .collect();
    let mut drawings = Vec::new();
    let mut shown: Vec<(String, usize)> = Vec::new();
    for kind in AccessoryKind::ALL {
        let art = AccessoryArt::resolve(Accessory::Worn(kind), SEED, &members);
        let mut poses_shown = 0;
        for (index, subject) in subjects.iter().enumerate() {
            let face = CreatureRenderer::resolve_face_state(
                &posed(subject, (ActionKind::Idle, 0.0, None)),
                CursorSnapshot::default(),
                false,
            );
            for clip in clips() {
                for facing_right in [true, false] {
                    let render = |dress| {
                        CreatureRenderer::render_dressed_composited_frame(
                            &subject.appearance,
                            dress,
                            clip,
                            0,
                            facing_right,
                            false,
                            face,
                        )
                    };
                    let bare = render(None);
                    let dressed = render(Some(art));
                    let part: Vec<bool> = dressed
                        .pixels()
                        .iter()
                        .zip(bare.pixels())
                        .map(|(d, b)| d != b)
                        .collect();
                    if part.iter().any(|p| *p) {
                        poses_shown += 1;
                    }
                    drawings.push(Drawing {
                        item: format!("{kind:?}"),
                        pose: format!(
                            "body {index}, {}, facing {}",
                            clip_name(clip),
                            if facing_right { "right" } else { "left" }
                        ),
                        canvas: dressed,
                        part: Some(part),
                        shown: facing_right && SHEET_CLIPS.contains(&clip),
                    });
                }
            }
        }
        shown.push((format!("{kind:?}"), poses_shown));
    }
    let mut report = checked("accessory", (FRAME_SIZE, FRAME_SIZE), NO_EDGES, drawings);
    // Hidden in a pose (behind the body, say) is fine; never showing at all is not. Two
    // accessories both tucked out of sight in a pose look alike there and say nothing by it, so
    // look-alikes are judged across every pose at once.
    report
        .problems
        .retain(|p| p.check != "blank" && p.check != "look-alike");
    report
        .problems
        .extend(alike_in_every_pose(&report.drawings));
    for (item, poses) in shown {
        if poses == 0 {
            report.problems.push(Problem {
                item,
                pose: None,
                level: Level::Error,
                check: "blank",
                message: "never shows on any body in any clip".into(),
            });
        }
    }
    report
}

/// Every kind of body the generator makes, as (archetype, body plan, ears), from the first of
/// `BODY_SAMPLES` recipes that has it.
fn body_kinds() -> Vec<(String, Creature)> {
    let desktop = crate::fixture_desktop();
    let mut kinds: Vec<(String, Creature)> = Vec::new();
    for sample in 0..BODY_SAMPLES {
        let mut seed = [0_u8; 32];
        seed[..4].copy_from_slice(&sample.to_le_bytes());
        seed[31] = 0xB0;
        let creature = World::preview_adult(seed, OffsetDateTime::UNIX_EPOCH, &desktop);
        let Some(design) = creature.appearance.design else {
            continue;
        };
        let archetype = design
            .body_archetype()
            .map_or_else(|| "None".to_owned(), |a| format!("{a:?}"));
        let key = format!("{archetype} {:?} {:?} ears", design.body, design.ears);
        if !kinds.iter().any(|(existing, _)| *existing == key) {
            kinds.push((key, creature));
        }
    }
    kinds.sort_by(|a, b| a.0.cmp(&b.0));
    kinds
}

/// Every frame of every clip, for every kind of body the generator makes.
fn bodies() -> KindReport {
    let mut drawings = Vec::new();
    for (key, creature) in body_kinds() {
        let face =
            CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
        for clip in clips() {
            for frame in 0..AnimationSpec::for_clip(clip).frames {
                drawings.push(Drawing {
                    shown: frame == 0,
                    ..drawing(
                        key.clone(),
                        format!("{} {frame}", clip_name(clip)),
                        CreatureRenderer::render_composited_frame(
                            &creature.appearance,
                            clip,
                            frame,
                            true,
                            false,
                            face,
                        ),
                    )
                });
            }
        }
    }
    let mut report = checked("body", (FRAME_SIZE, FRAME_SIZE), NO_EDGES, drawings);
    // Bodies are many recipes of the same parts; two looking alike is the generator at work.
    report.problems.retain(|p| p.check != "look-alike");
    report
}

/// Every expression with every eyelid, on one face, so a face that loses its eyes is seen and
/// two expressions that read the same are caught.
fn faces() -> KindReport {
    let creature =
        World::preview_adult(SEED, OffsetDateTime::UNIX_EPOCH, &crate::fixture_desktop());
    let base = CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
    let mut drawings = Vec::new();
    for expression in ExpressionKind::ALL {
        for eyelids in EyelidPose::ALL {
            let face = FaceRenderState {
                expression,
                eyelids,
                ..base
            };
            drawings.push(drawing(
                format!("{expression:?}"),
                format!("{eyelids:?} eyelids"),
                CreatureRenderer::render_face_frame(&creature.appearance, face),
            ));
        }
    }
    let mut report = checked(
        "face",
        (FACE_FRAME_SIZE, FACE_FRAME_SIZE),
        NO_EDGES,
        drawings,
    );
    // Lids lowered over the eyes leave little of an expression to tell apart: only faces with
    // their eyes open are compared.
    let open = format!("{:?} eyelids", EyelidPose::ALL[0]);
    report
        .problems
        .retain(|p| p.check != "look-alike" || p.pose.as_deref() == Some(open.as_str()));
    report
}

/// One sheet per kind, for looking the check over by eye: each item's drawings in a block with
/// its name, the blocks flowing in columns, and anything flagged outlined. Kinds with hundreds of
/// drawings an item show a few typical poses, plus every pose that was flagged.
/// One item's drawings on a sheet, under its name, each with what was found in it.
type Block<'a> = (&'a str, Vec<(&'a Drawing, Option<Level>)>);

fn write_sheet(report: &KindReport, path: &Path) -> Result<()> {
    const GAP: u32 = 4;
    const LABEL: u32 = 150;
    /// The widest a sheet grows by adding columns of blocks.
    const SHEET_WIDTH: u32 = 2400;
    let (frame_w, frame_h) = report.frame;
    let scale = if frame_w <= 16 {
        3
    } else if frame_w <= 48 && report.drawings.len() < 1_000 {
        2
    } else {
        1
    };
    let level_of = |drawing: &Drawing| {
        report
            .problems
            .iter()
            .filter(|p| {
                p.item == drawing.item && p.pose.as_deref().is_none_or(|pose| pose == drawing.pose)
            })
            .map(|p| p.level)
            .max()
    };
    let mut blocks: Vec<Block> = Vec::new();
    for drawing in &report.drawings {
        let level = level_of(drawing);
        if !drawing.shown && level.is_none() {
            continue;
        }
        match blocks.last_mut() {
            Some((item, cells)) if *item == drawing.item => cells.push((drawing, level)),
            _ => blocks.push((&drawing.item, vec![(drawing, level)])),
        }
    }
    let (cell_w, cell_h) = (frame_w * scale + GAP, frame_h * scale + GAP);
    let widest = blocks
        .iter()
        .map(|(_, cells)| cells.len())
        .max()
        .unwrap_or(1) as u32;
    let block_w = LABEL + widest * cell_w + GAP;
    // Roughly as wide as it is tall, and never much wider than SHEET_WIDTH.
    let square = ((blocks.len() as f32 * cell_h as f32 / block_w as f32).sqrt()).round() as u32;
    let columns = square.clamp(1, (SHEET_WIDTH / block_w).max(1));
    let rows = (blocks.len() as u32).div_ceil(columns).max(1);
    let width = columns * block_w + GAP;
    let height = rows * cell_h + GAP;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    crate::fill_rect(
        &mut pixels,
        width,
        0,
        0,
        width,
        height,
        [232, 228, 218, 255],
    );
    for (index, (item, cells)) in blocks.iter().enumerate() {
        let (column, row) = (index as u32 / rows, index as u32 % rows);
        let (left, y) = (GAP + column * block_w, GAP + row * cell_h);
        crate::social_preview::draw_text(
            &mut pixels,
            width as i32,
            height as i32,
            left as i32,
            (y + cell_h / 2) as i32 - 3,
            item,
            1,
            [60, 52, 50, 255],
        );
        for (position, (drawing, level)) in cells.iter().enumerate() {
            let x = left + LABEL + position as u32 * cell_w;
            let ground = match level {
                Some(Level::Error) => [214, 60, 60, 255],
                Some(Level::Warning) => [236, 170, 40, 255],
                None => [208, 204, 194, 255],
            };
            crate::fill_rect(
                &mut pixels,
                width,
                x - 2,
                y - 2,
                frame_w * scale + 4,
                frame_h * scale + 4,
                ground,
            );
            crate::fill_rect(
                &mut pixels,
                width,
                x,
                y,
                frame_w * scale,
                frame_h * scale,
                [246, 243, 236, 255],
            );
            crate::blit_canvas_scaled(&mut pixels, width, x, y, &drawing.canvas, scale);
        }
    }
    crate::write_png(path, width, height, &pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(size: u32, from: (i32, i32), to: (i32, i32), color: Rgba) -> Canvas {
        let mut canvas = Canvas::new(size, size);
        for y in from.1..to.1 {
            for x in from.0..to.0 {
                canvas.set(x, y, color);
            }
        }
        canvas
    }

    const INK: Rgba = Rgba::new(40, 40, 40, 255);

    fn found(report: &KindReport) -> Vec<(&str, &'static str, Level)> {
        report
            .problems
            .iter()
            .map(|p| (p.item.as_str(), p.check, p.level))
            .collect()
    }

    #[test]
    fn a_blank_drawing_is_an_error() {
        let report = checked(
            "test",
            (8, 8),
            NO_EDGES,
            vec![drawing("empty", "rest", Canvas::new(8, 8))],
        );
        assert_eq!(found(&report), vec![("empty", "blank", Level::Error)]);
    }

    #[test]
    fn reaching_an_edge_is_a_warning_unless_the_art_sits_there_by_design() {
        let touching = || drawing("tall", "rest", square(8, (2, 2), (6, 8), INK));
        let report = checked("test", (8, 8), NO_EDGES, vec![touching()]);
        assert_eq!(found(&report), vec![("tall", "edge", Level::Warning)]);
        assert!(report.problems[0].message.contains("bottom"));
        assert!(
            checked("test", (8, 8), GROUND, vec![touching()])
                .problems
                .is_empty()
        );
    }

    #[test]
    fn items_drawn_alike_are_caught_in_the_same_pose_only() {
        let same = |item: &str, pose: &str| drawing(item, pose, square(8, (2, 2), (6, 6), INK));
        let report = checked(
            "test",
            (8, 8),
            NO_EDGES,
            vec![same("a", "rest"), same("b", "rest")],
        );
        assert_eq!(found(&report), vec![("a", "look-alike", Level::Error)]);
        let report = checked(
            "test",
            (8, 8),
            NO_EDGES,
            vec![same("a", "rest"), same("b", "run")],
        );
        assert!(report.problems.is_empty());
        let mut nearly = square(8, (1, 1), (7, 7), INK);
        nearly.set(1, 1, Rgba::new(200, 0, 0, 255));
        let report = checked(
            "test",
            (8, 8),
            NO_EDGES,
            vec![
                drawing("a", "rest", square(8, (1, 1), (7, 7), INK)),
                drawing("b", "rest", nearly),
            ],
        );
        assert_eq!(found(&report), vec![("a", "look-alike", Level::Warning)]);
    }

    #[test]
    fn accessories_alike_in_one_pose_only_are_not_look_alikes() {
        let a = |item: &str, pose: &str| drawing(item, pose, square(8, (2, 2), (6, 6), INK));
        let b = |item: &str, pose: &str| drawing(item, pose, square(8, (1, 1), (7, 7), INK));
        let differ_when_running = [a("x", "rest"), a("x", "run"), a("y", "rest"), b("y", "run")];
        assert!(alike_in_every_pose(&differ_when_running).is_empty());
        let alike_always = [a("x", "rest"), b("x", "run"), a("y", "rest"), b("y", "run")];
        let found: Vec<_> = alike_in_every_pose(&alike_always)
            .into_iter()
            .map(|p| (p.item, p.level))
            .collect();
        assert_eq!(found, vec![("x".to_string(), Level::Error)]);
    }

    #[test]
    fn the_shipped_art_passes() {
        let out = std::env::temp_dir().join(format!("formiga-art-check-{}", std::process::id()));
        for kind in ["trinket", "souvenir", "decoration", "face"] {
            let report = run(&out, Some(kind)).unwrap();
            assert_eq!(report["success"], json!(true), "{kind}: {report:#}");
        }
        let _ = std::fs::remove_dir_all(&out);
    }
}
