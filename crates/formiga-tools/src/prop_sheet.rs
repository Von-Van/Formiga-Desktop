//! Everything a companion can be holding, on the backgrounds it will be held against.
//!
//! Three bands. The colony's trinket atlas at 4x and 2x, resting and glinting, on pale, dark, and
//! busy wallpaper. The same sixteen held up by a companion in the presentation pose, arranged the
//! way the overlay arranges them — body frame, face anchor, prop quad. Then every toy, snack, and
//! cup the generator can produce, across the frames of its own clip, for each body plan.
//!
//! This is a review sheet, not a test: it exists so a person can see at a glance whether a held
//! thing reads as that thing at the size it is actually shown.
//!
//!   cargo run -p formiga-tools -- prop-sheet [--output PATH]

use anyhow::Result;
use formiga_art::{
    Canvas, CreatureRenderer, FACE_FRAME_SIZE, FRAME_SIZE, FramePlacement, PropAnchor,
    TRINKET_ATLAS_COLUMNS, TRINKET_CELL, TRINKET_FRAME_GLINT, TRINKET_FRAME_REST,
    TrinketAtlasRenderer,
};
use formiga_core::{ActionKind, AppearanceGenome, Creature, TrinketCondition, all_trinkets};
use std::path::PathBuf;

const COLUMNS: u32 = 16;
const COLUMN: u32 = 108;
const WIDTH: u32 = COLUMNS * COLUMN;
/// Twelve creature cells: four frames of a clip for each of three body plans.
const CLIP_CELL: u32 = WIDTH / 12;

/// The three desktops a keepsake has to survive.
#[derive(Clone, Copy, PartialEq)]
enum Ground {
    Pale,
    Dark,
    Busy,
}

impl Ground {
    const ALL: [Self; 3] = [Self::Pale, Self::Dark, Self::Busy];

    fn label(self) -> &'static str {
        match self {
            Self::Pale => "PALE WALLPAPER",
            Self::Dark => "DARK WALLPAPER",
            Self::Busy => "BUSY WALLPAPER",
        }
    }

    fn ink(self) -> [u8; 4] {
        match self {
            Self::Pale => [60, 70, 66, 255],
            _ => [226, 232, 224, 255],
        }
    }
}

struct Sheet {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
}

impl Sheet {
    fn new(width: u32, height: u32) -> Self {
        Self {
            pixels: vec![0; (width * height * 4) as usize],
            width,
            height,
        }
    }

    fn ground(&mut self, ground: Ground, y: u32, height: u32) {
        for row in y..(y + height).min(self.height) {
            for x in 0..self.width {
                let color = match ground {
                    Ground::Pale => [238, 234, 226, 255],
                    Ground::Dark => [24, 27, 32, 255],
                    Ground::Busy => {
                        // Diagonal bands with a hard edge: the case where a thin outline and a
                        // pale highlight both have somewhere to hide.
                        let band = ((x / 9 + row / 9) % 3) as usize;
                        [[40, 88, 104, 255], [206, 176, 104, 255], [122, 70, 96, 255]][band]
                    }
                };
                self.set(x, row, color);
            }
        }
    }

    fn set(&mut self, x: u32, y: u32, color: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let index = ((y * self.width + x) * 4) as usize;
        self.pixels[index..index + 4].copy_from_slice(&color);
    }

    fn rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: [u8; 4]) {
        for py in y..y + height {
            for px in x..x + width {
                crate::blend_pixel(&mut self.pixels, self.width, px, py, color);
            }
        }
    }

    /// Nearest-neighbour, exactly how the overlay magnifies a sprite.
    fn stamp(&mut self, canvas: &Canvas, origin_x: i32, origin_y: i32, scale: i32) {
        for sy in 0..canvas.height() as i32 {
            for sx in 0..canvas.width() as i32 {
                let pixel = canvas.get(sx, sy);
                if pixel.a == 0 {
                    continue;
                }
                for oy in 0..scale {
                    for ox in 0..scale {
                        let (x, y) = (origin_x + sx * scale + ox, origin_y + sy * scale + oy);
                        if x < 0 || y < 0 {
                            continue;
                        }
                        crate::blend_pixel(
                            &mut self.pixels,
                            self.width,
                            x as u32,
                            y as u32,
                            [pixel.r, pixel.g, pixel.b, pixel.a],
                        );
                    }
                }
            }
        }
    }

    /// One cell of the trinket atlas, cut out and magnified.
    fn stamp_cell(
        &mut self,
        atlas: &Canvas,
        variant: u8,
        frame: u8,
        origin_x: i32,
        origin_y: i32,
        scale: i32,
    ) {
        let (x0, y0, width, height) = TrinketAtlasRenderer::cell_rect(variant, frame);
        let mut cell = Canvas::new(width, height);
        for y in 0..height {
            for x in 0..width {
                cell.set(
                    x as i32,
                    y as i32,
                    atlas.get((x0 + x) as i32, (y0 + y) as i32),
                );
            }
        }
        self.stamp(&cell, origin_x, origin_y, scale);
    }

    /// A five-pixel-tall caption, drawn from the same little alphabet the other sheets use.
    fn text(&mut self, text: &str, x: u32, y: u32, scale: u32, color: [u8; 4]) {
        let mut cursor = x;
        for character in text.chars() {
            for (row, bits) in glyph(character).into_iter().enumerate() {
                for column in 0..4 {
                    if bits & (1 << (3 - column)) == 0 {
                        continue;
                    }
                    self.rect(
                        cursor + column * scale,
                        y + row as u32 * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
            cursor += 5 * scale;
        }
    }
}

pub fn run(path: PathBuf) -> Result<()> {
    let creatures = crate::reference_creatures();
    let colony_seed = [61_u8; 32];
    let members: Vec<formiga_art::Palette> = creatures
        .iter()
        .map(|creature| formiga_art::palette_for(&creature.appearance))
        .collect();
    let atlas = TrinketAtlasRenderer::render(colony_seed, &members);

    // Header, a 4x row for each frame, a 2x pair, and a caption.
    let swatch_band = 22 + 4 * TRINKET_CELL + 4 * TRINKET_CELL + 2 * TRINKET_CELL + 26;
    let held_band = 22 + 3 * 132;
    let clip_rows =
        u32::from(formiga_art::TOY_KINDS + formiga_art::SNACK_KINDS + formiga_art::DRINK_KINDS);
    let clip_band = 22 + clip_rows * CLIP_CELL;
    let height = swatch_band * 3 + held_band + clip_band;
    let mut sheet = Sheet::new(WIDTH, height);

    let mut y = 0;
    for ground in Ground::ALL {
        sheet.ground(ground, y, swatch_band);
        sheet.text(
            &format!("FOUND THINGS 4X AND 2X {}", ground.label()),
            10,
            y + 8,
            2,
            ground.ink(),
        );
        for variant in 0..TRINKET_ATLAS_COLUMNS as u8 {
            let column = variant as u32 * COLUMN;
            let big = TRINKET_CELL as i32 * 4;
            sheet.stamp_cell(
                &atlas,
                variant,
                TRINKET_FRAME_REST,
                (column + (COLUMN - big as u32) / 2) as i32,
                (y + 22) as i32,
                4,
            );
            sheet.stamp_cell(
                &atlas,
                variant,
                TRINKET_FRAME_GLINT,
                (column + (COLUMN - big as u32) / 2) as i32,
                (y + 22 + 4 * TRINKET_CELL) as i32,
                4,
            );
            let small = TRINKET_CELL as i32 * 2;
            let pair = (COLUMN - (small as u32 * 2 + 8)) / 2;
            for (index, frame) in [TRINKET_FRAME_REST, TRINKET_FRAME_GLINT]
                .into_iter()
                .enumerate()
            {
                sheet.stamp_cell(
                    &atlas,
                    variant,
                    frame,
                    (column + pair) as i32 + index as i32 * (small + 8),
                    (y + 22 + 8 * TRINKET_CELL + 4) as i32,
                    2,
                );
            }
            if let Some(info) = formiga_core::trinket_info(variant) {
                sheet.text(
                    &short(info.name),
                    column + 6,
                    y + swatch_band - 12,
                    1,
                    ground.ink(),
                );
                if info.condition != TrinketCondition::Anywhere {
                    sheet.text(
                        &condition_tag(info.condition),
                        column + 6,
                        y + swatch_band - 20,
                        1,
                        ground.ink(),
                    );
                }
            }
        }
        y += swatch_band;
    }

    // Held up, exactly as the overlay stacks it: body frame, layered face, then the prop quad
    // placed from `PropAnchor` against the face anchor.
    sheet.ground(Ground::Pale, y, held_band);
    sheet.text(
        "HELD UP IN THE PRESENTATION POSE",
        10,
        y + 8,
        2,
        [60, 70, 66, 255],
    );
    for (row, creature) in creatures.iter().enumerate() {
        let ground = Ground::ALL[row % Ground::ALL.len()];
        let top = y + 22 + row as u32 * 132;
        sheet.ground(ground, top, 132);
        for variant in 0..TRINKET_ATLAS_COLUMNS as u8 {
            let column = variant as u32 * COLUMN;
            held(&mut sheet, &atlas, creature, variant, column, top);
        }
    }
    y += held_band;

    // Playing, eating, drinking: four frames of the clip, for each body plan.
    sheet.ground(Ground::Pale, y, clip_band);
    sheet.text(
        "PLAYING EATING DRINKING EVERY KIND EVERY BODY",
        10,
        y + 8,
        2,
        [60, 70, 66, 255],
    );
    let mut row = 0;
    for (action, kinds) in [
        (ActionKind::SoloPlay, formiga_art::TOY_KINDS),
        (ActionKind::Eat, formiga_art::SNACK_KINDS),
        (ActionKind::Drink, formiga_art::DRINK_KINDS),
    ] {
        for kind in 0..kinds {
            let top = y + 22 + row * CLIP_CELL;
            sheet.ground(
                if row % 2 == 0 {
                    Ground::Pale
                } else {
                    Ground::Dark
                },
                top,
                CLIP_CELL,
            );
            for (plan, creature) in creatures.iter().enumerate() {
                let genome = genome_with(&creature.appearance, action, kind);
                for frame in 0..4_u8 {
                    let cell = plan as u32 * 4 + u32::from(frame);
                    let rendered = CreatureRenderer::render_frame(&genome, action, frame, true);
                    sheet.stamp(
                        &rendered,
                        (cell * CLIP_CELL + (CLIP_CELL - FRAME_SIZE * 3) / 2) as i32,
                        (top + 2) as i32,
                        3,
                    );
                }
            }
            sheet.text(
                &format!("{} {kind}", clip_tag(action)),
                4,
                top + CLIP_CELL - 10,
                1,
                if row % 2 == 0 {
                    [60, 70, 66, 255]
                } else {
                    [226, 232, 224, 255]
                },
            );
            row += 1;
        }
    }

    crate::write_png(&path, WIDTH, height, &sheet.pixels)?;
    println!("wrote {}", path.display());
    println!("{}", catalogue());
    Ok(())
}

/// One companion presenting one keepsake, stacked the way `gpu.rs` stacks its quads.
fn held(sheet: &mut Sheet, atlas: &Canvas, creature: &Creature, variant: u8, x: u32, y: u32) {
    const ZOOM: i32 = 2;
    let mut creature = creature.clone();
    creature.state.action = ActionKind::PresentDiscovery;
    creature.state.activity_variant = variant;
    creature.state.facing_right = true;
    let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
    let origin = FramePlacement::for_creature(&creature, baseline).origin_y;
    let body = CreatureRenderer::render_body_frame(
        &creature.appearance,
        ActionKind::PresentDiscovery,
        2,
        false,
    );
    let _ = origin;
    let left = x as i32 + (COLUMN as i32 - FRAME_SIZE as i32 * ZOOM) / 2;
    let top = y as i32 + 16;
    sheet.stamp(&body.canvas, left, top, ZOOM);
    let face = CreatureRenderer::render_face_frame(
        &creature.appearance,
        CreatureRenderer::resolve_face_state(
            &creature,
            formiga_core::CursorSnapshot::default(),
            false,
        ),
    );
    let anchor = body.face_anchor;
    let half = FACE_FRAME_SIZE as i32 * ZOOM / 2;
    sheet.stamp(
        &face,
        left + anchor.x * ZOOM - half,
        top + anchor.y * ZOOM - half,
        ZOOM,
    );
    let prop = PropAnchor::for_creature(&creature);
    sheet.stamp_cell(
        atlas,
        variant,
        TRINKET_FRAME_GLINT,
        left + anchor.x * ZOOM + (prop.dx * ZOOM as f32) as i32 - half,
        top + anchor.y * ZOOM + (prop.dy * ZOOM as f32) as i32 - half,
        ZOOM,
    );
}

/// The same appearance, nudged onto one particular belonging. Only a review sheet does this: a
/// real companion keeps whichever one its own genes resolve to.
fn genome_with(base: &AppearanceGenome, action: ActionKind, kind: u8) -> AppearanceGenome {
    let wanted = |genome: &AppearanceGenome| {
        let (toy, snack, drink) = formiga_art::prop_variants(genome);
        match action {
            ActionKind::Eat => snack,
            ActionKind::Drink => drink,
            _ => toy,
        }
    };
    let mut genome = base.clone();
    for nudge in 0..4096_u64 {
        // Spread the nudge across the whole word: each belonging reads a different slice of it.
        genome.marking_seed = base.marking_seed ^ nudge.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        if wanted(&genome) == kind {
            return genome;
        }
    }
    genome
}

fn clip_tag(action: ActionKind) -> &'static str {
    match action {
        ActionKind::Eat => "SNACK",
        ActionKind::Drink => "CUP",
        _ => "TOY",
    }
}

fn condition_tag(condition: TrinketCondition) -> String {
    match condition {
        TrinketCondition::Night => "NIGHT",
        TrinketCondition::HighTier => "HIGH",
        TrinketCondition::MidRide => "RIDE",
        TrinketCondition::BesideCloseFriend => "FRIEND",
        TrinketCondition::Anywhere => "ANY",
    }
    .into()
}

/// The catalogue in one line, so a reviewer can check the sheet against the table.
pub fn catalogue() -> String {
    all_trinkets()
        .map(|info| {
            format!(
                "{:>2} {:<18} {:<18} {}",
                info.variant,
                info.name,
                condition_tag(info.condition),
                info.hint
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn short(name: &str) -> String {
    name.to_ascii_uppercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ')
        .take(16)
        .collect()
}

/// A 4x5 alphabet, enough for the captions on this sheet.
fn glyph(character: char) -> [u8; 5] {
    match character.to_ascii_uppercase() {
        'A' => [0b0110, 0b1001, 0b1111, 0b1001, 0b1001],
        'B' => [0b1110, 0b1001, 0b1110, 0b1001, 0b1110],
        'C' => [0b0111, 0b1000, 0b1000, 0b1000, 0b0111],
        'D' => [0b1110, 0b1001, 0b1001, 0b1001, 0b1110],
        'E' => [0b1111, 0b1000, 0b1110, 0b1000, 0b1111],
        'F' => [0b1111, 0b1000, 0b1110, 0b1000, 0b1000],
        'G' => [0b0111, 0b1000, 0b1011, 0b1001, 0b0111],
        'H' => [0b1001, 0b1001, 0b1111, 0b1001, 0b1001],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b0011, 0b0001, 0b0001, 0b1001, 0b0110],
        'K' => [0b1001, 0b1010, 0b1100, 0b1010, 0b1001],
        'L' => [0b1000, 0b1000, 0b1000, 0b1000, 0b1111],
        'M' => [0b1001, 0b1111, 0b1111, 0b1001, 0b1001],
        'N' => [0b1001, 0b1101, 0b1111, 0b1011, 0b1001],
        'O' => [0b0110, 0b1001, 0b1001, 0b1001, 0b0110],
        'P' => [0b1110, 0b1001, 0b1110, 0b1000, 0b1000],
        'Q' => [0b0110, 0b1001, 0b1001, 0b1011, 0b0111],
        'R' => [0b1110, 0b1001, 0b1110, 0b1010, 0b1001],
        'S' => [0b0111, 0b1000, 0b0110, 0b0001, 0b1110],
        'T' => [0b1111, 0b0110, 0b0110, 0b0110, 0b0110],
        'U' => [0b1001, 0b1001, 0b1001, 0b1001, 0b0110],
        'V' => [0b1001, 0b1001, 0b1001, 0b0110, 0b0110],
        'W' => [0b1001, 0b1001, 0b1111, 0b1111, 0b1001],
        'X' => [0b1001, 0b0110, 0b0110, 0b0110, 0b1001],
        'Y' => [0b1001, 0b0110, 0b0110, 0b0110, 0b0110],
        'Z' => [0b1111, 0b0010, 0b0100, 0b1000, 0b1111],
        '0' => [0b0110, 0b1011, 0b1101, 0b1001, 0b0110],
        '1' => [0b0010, 0b0110, 0b0010, 0b0010, 0b0111],
        '2' => [0b1110, 0b0001, 0b0110, 0b1000, 0b1111],
        '3' => [0b1110, 0b0001, 0b0110, 0b0001, 0b1110],
        '4' => [0b1010, 0b1010, 0b1111, 0b0010, 0b0010],
        '5' => [0b1111, 0b1000, 0b1110, 0b0001, 0b1110],
        '6' => [0b0110, 0b1000, 0b1110, 0b1001, 0b0110],
        '7' => [0b1111, 0b0001, 0b0010, 0b0100, 0b0100],
        '8' => [0b0110, 0b1001, 0b0110, 0b1001, 0b0110],
        '9' => [0b0110, 0b1001, 0b0111, 0b0001, 0b0110],
        _ => [0; 5],
    }
}
