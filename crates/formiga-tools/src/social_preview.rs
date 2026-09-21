//! Storefront art: a GitHub social preview image and an itch.io cover variant.
//!
//! Both are charming scenes of a few varied creatures among stylised windows, with a small
//! village tucked in a corner, plus a crisp pixel-lettered "Formiga" title and tagline.
//!
//! This module is deliberately self-contained. `hero_image` in `main.rs` builds a similar scene
//! with private helper functions, but keeping the distribution tooling additive (rather than
//! exporting those helpers) means this file owns its own small raster toolkit: alpha blending,
//! rectangle/gradient fills, sprite blitting, and a hand-drawn bitmap font. Everything here is
//! deterministic and reads only public `formiga_art` / `formiga_core` items, matching the
//! approach the other `formiga-tools` subcommands already use.

use anyhow::{Context, Result};
use formiga_art::{AnimationSpec, CreatureRenderer, FRAME_SIZE, SHELTER_SIZE, ShelterRenderer};
use formiga_core::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

const SOCIAL_WIDTH: i32 = 1280;
const SOCIAL_HEIGHT: i32 = 640;
const COVER_WIDTH: i32 = 630;
const COVER_HEIGHT: i32 = 500;

const SKY_TOP: [u8; 4] = [12, 20, 26, 255];
const SKY_BOTTOM: [u8; 4] = [27, 47, 46, 255];
const TITLE_INK: [u8; 4] = [250, 244, 224, 255];
const TITLE_SHADOW: [u8; 4] = [14, 24, 22, 235];
const TAGLINE_INK: [u8; 4] = [205, 221, 209, 255];
const TAGLINE_SHADOW: [u8; 4] = [14, 24, 22, 200];
const MOON: [u8; 4] = [255, 226, 145, 255];
const MOON_HALO: [u8; 4] = [255, 246, 221, 60];

pub fn social_preview(path: PathBuf) -> Result<()> {
    let pixels = render_social_scene();
    write_png(&path, SOCIAL_WIDTH, SOCIAL_HEIGHT, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

pub fn itch_cover(path: PathBuf) -> Result<()> {
    let pixels = render_cover_scene();
    write_png(&path, COVER_WIDTH, COVER_HEIGHT, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// GitHub recommends 1280x640 and keeps the central ~1200x560 free of chrome, so every
/// important element (title, creatures, village) stays inside that safe area.
fn render_social_scene() -> Vec<u8> {
    let mut pixels = vec![0_u8; (SOCIAL_WIDTH * SOCIAL_HEIGHT * 4) as usize];
    fill_gradient(
        &mut pixels,
        SOCIAL_WIDTH,
        SOCIAL_HEIGHT,
        SKY_TOP,
        SKY_BOTTOM,
    );
    fill_circle_alpha(
        &mut pixels,
        SOCIAL_WIDTH,
        SOCIAL_HEIGHT,
        1168,
        78,
        40,
        MOON_HALO,
    );
    fill_circle_alpha(&mut pixels, SOCIAL_WIDTH, SOCIAL_HEIGHT, 1168, 78, 26, MOON);
    scatter_fireflies(&mut pixels, SOCIAL_WIDTH, SOCIAL_HEIGHT, 130, 0x5EED_5EED);

    // Two overlapping windows on the right, like two desktop apps side by side. The ledge on
    // the taller one carries a perched creature well clear of the tagline's text block below.
    draw_window(
        &mut pixels,
        SOCIAL_WIDTH,
        SOCIAL_HEIGHT,
        760,
        330,
        300,
        270,
        [45, 66, 74, 255],
    );
    draw_window(
        &mut pixels,
        SOCIAL_WIDTH,
        SOCIAL_HEIGHT,
        1000,
        230,
        200,
        250,
        [53, 57, 79, 255],
    );
    fill_rect_alpha(
        &mut pixels,
        SOCIAL_WIDTH,
        SOCIAL_HEIGHT,
        790,
        430,
        190,
        140,
        [61, 185, 125, 46],
    );

    draw_village(&mut pixels, SOCIAL_WIDTH, SOCIAL_HEIGHT, 44, 600, 2);

    let creatures = varied_creatures();
    let placements = [
        (400, 600, ActionKind::Sleep, 3),
        (700, 600, ActionKind::Greet, 5),
        (900, 600, ActionKind::SoloPlay, 3),
        (1090, 230, ActionKind::Perch, 3),
        (1100, 600, ActionKind::Idle, 3),
    ];
    for (creature, (x, y, action, scale)) in creatures.iter().zip(placements) {
        stamp_creature(
            &mut pixels,
            SOCIAL_WIDTH,
            SOCIAL_HEIGHT,
            creature,
            action,
            x,
            y,
            scale,
        );
    }

    // The title sits in the plain sky above the scene, and the tagline's own measured width
    // keeps it clear of the perched creature to the right (see the placements above).
    let title_y = 62;
    draw_text_centered_with_shadow(
        &mut pixels,
        SOCIAL_WIDTH,
        SOCIAL_HEIGHT,
        SOCIAL_WIDTH / 2,
        title_y,
        "FORMIGA",
        9,
        TITLE_INK,
        TITLE_SHADOW,
    );
    draw_text_centered_with_shadow(
        &mut pixels,
        SOCIAL_WIDTH,
        SOCIAL_HEIGHT,
        SOCIAL_WIDTH / 2,
        title_y + FONT_HEIGHT * 9 + 20,
        "A TINY COLONY THAT LIVES ON YOUR DESKTOP.",
        3,
        TAGLINE_INK,
        TAGLINE_SHADOW,
    );

    pixels
}

/// itch.io's cover slot is 630x500: a squarer, shorter canvas than the social preview, so this
/// is its own composition rather than a crop - one window, a smaller village, and four
/// creatures instead of five.
fn render_cover_scene() -> Vec<u8> {
    let mut pixels = vec![0_u8; (COVER_WIDTH * COVER_HEIGHT * 4) as usize];
    fill_gradient(&mut pixels, COVER_WIDTH, COVER_HEIGHT, SKY_TOP, SKY_BOTTOM);
    fill_circle_alpha(
        &mut pixels,
        COVER_WIDTH,
        COVER_HEIGHT,
        540,
        78,
        34,
        MOON_HALO,
    );
    fill_circle_alpha(&mut pixels, COVER_WIDTH, COVER_HEIGHT, 540, 78, 22, MOON);
    scatter_fireflies(&mut pixels, COVER_WIDTH, COVER_HEIGHT, 70, 0xC0FF_EE01);

    draw_window(
        &mut pixels,
        COVER_WIDTH,
        COVER_HEIGHT,
        440,
        240,
        170,
        200,
        [45, 66, 74, 255],
    );
    fill_rect_alpha(
        &mut pixels,
        COVER_WIDTH,
        COVER_HEIGHT,
        460,
        320,
        130,
        90,
        [61, 185, 125, 46],
    );

    draw_village(&mut pixels, COVER_WIDTH, COVER_HEIGHT, 24, 470, 1);

    let creatures = varied_creatures();
    let placements = [
        (300, 470, ActionKind::Greet, 4),
        (430, 470, ActionKind::SoloPlay, 3),
        (525, 240, ActionKind::Perch, 3),
        (566, 470, ActionKind::Sleep, 2),
    ];
    for (creature, (x, y, action, scale)) in creatures.iter().zip(placements) {
        stamp_creature(
            &mut pixels,
            COVER_WIDTH,
            COVER_HEIGHT,
            creature,
            action,
            x,
            y,
            scale,
        );
    }

    let title_y = 34;
    draw_text_centered_with_shadow(
        &mut pixels,
        COVER_WIDTH,
        COVER_HEIGHT,
        COVER_WIDTH / 2,
        title_y,
        "FORMIGA",
        6,
        TITLE_INK,
        TITLE_SHADOW,
    );
    draw_text_centered_with_shadow(
        &mut pixels,
        COVER_WIDTH,
        COVER_HEIGHT,
        COVER_WIDTH / 2,
        title_y + FONT_HEIGHT * 6 + 14,
        "A DESKTOP COLONY",
        2,
        TAGLINE_INK,
        TAGLINE_SHADOW,
    );

    pixels
}

/// Five creatures, one per body plan, so the scene reads as varied at a glance. `preview_adult`
/// skips the days of simulated growth `hero_image`'s colony needs, which keeps this tool quick
/// to run while still producing real, deterministic, fully-rendered creatures.
fn varied_creatures() -> Vec<Creature> {
    let desktop = fixture_desktop();
    let stream = SeedStream::new([214; 32]);
    BodyPlan::ALL
        .into_iter()
        .enumerate()
        .map(|(index, body)| {
            let seed = stream.bytes("social-preview-creature", index as u64);
            let mut creature = World::preview_adult(seed, OffsetDateTime::UNIX_EPOCH, &desktop);
            let mut design = creature
                .appearance
                .design
                .expect("preview_adult always generates a modular design");
            design.body = body;
            design.ears = EarStyle::ALL[(index + 1) % EarStyle::ALL.len()];
            apply_creature_design(&mut creature, Some(design));
            creature
        })
        .collect()
}

/// Draws the colony house and two cottages as one small village row, all standing on the same
/// `baseline_y` ground line at `x`. `ShelterRenderer::render_village` lays every house out in its
/// own cell of an atlas meant for per-cell placement (see `home_yard_sheet` in `main.rs`), so each
/// is extracted and cropped to its own drawn pixels - trimming the transparent padding every cell
/// carries so a smaller cottage does not leave an awkward gap - to keep the three snug together.
/// Returns the x just past the rightmost house, in case a caller wants to place something next
/// to it.
fn draw_village(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    baseline_y: i32,
    scale: i32,
) -> i32 {
    let genome = ColonyHome::from_seed([166; 32], None, None, None).shelter;
    let atlas = ShelterRenderer::render_village(&genome, &ShelterDecorationKind::ALL, &[], false);
    let size = SHELTER_SIZE as i32;
    let gap = 4 * scale;
    let mut cursor = x;
    for slot in 0..3 {
        let (cell_x, cell_y) =
            ShelterRenderer::village_cell(formiga_art::VillageCell::House { slot, lit: false });
        let (cell_x, cell_y) = (cell_x as i32, cell_y as i32);
        let (cell_w, cell_h, sprite) = extract_cell_cropped(&atlas, cell_x, cell_y, size);
        blit_rect_alpha(
            pixels,
            width,
            height,
            cursor,
            baseline_y - cell_h * scale,
            &sprite,
            cell_w,
            cell_h,
            scale,
        );
        cursor += cell_w * scale + gap;
    }
    cursor - gap
}

/// Reads one `size`x`size` cell out of a village atlas and trims it to the bounding box of its
/// non-transparent pixels, so a cottage does not carry the same wide empty margin a full-size
/// colony house does.
fn extract_cell_cropped(
    atlas: &formiga_art::Canvas,
    cell_x: i32,
    cell_y: i32,
    size: i32,
) -> (i32, i32, Vec<u8>) {
    let mut min_x = size;
    let mut min_y = size;
    let mut max_x = -1;
    let mut max_y = -1;
    for local_y in 0..size {
        for local_x in 0..size {
            if atlas.get(cell_x + local_x, cell_y + local_y).a > 0 {
                min_x = min_x.min(local_x);
                min_y = min_y.min(local_y);
                max_x = max_x.max(local_x);
                max_y = max_y.max(local_y);
            }
        }
    }
    if max_x < min_x || max_y < min_y {
        return (1, 1, vec![0_u8; 4]);
    }
    let width = max_x - min_x + 1;
    let height = max_y - min_y + 1;
    let mut bytes = vec![0_u8; (width * height * 4) as usize];
    for local_y in 0..height {
        for local_x in 0..width {
            let pixel = atlas.get(cell_x + min_x + local_x, cell_y + min_y + local_y);
            let index = ((local_y * width + local_x) * 4) as usize;
            bytes[index] = pixel.r;
            bytes[index + 1] = pixel.g;
            bytes[index + 2] = pixel.b;
            bytes[index + 3] = pixel.a;
        }
    }
    (width, height, bytes)
}

fn fixture_desktop() -> DesktopSnapshot {
    DesktopSnapshot {
        monitors: vec![MonitorInfo {
            id: 1,
            display_key: DisplayKey([1; 16]),
            bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 1440.0,
                height: 900.0,
            },
            usable_bounds: DesktopRect {
                x: 0.0,
                y: 24.0,
                width: 1440.0,
                height: 826.0,
            },
            scale_factor: 2.0,
            primary: true,
        }],
        cursor: CursorSnapshot {
            position: Point { x: 720.0, y: 420.0 },
            velocity: Point::default(),
            available: true,
        },
        ..DesktopSnapshot::default()
    }
}

/// Stamps a creature with its feet at `(anchor_x, anchor_y)`, matching the anchor convention
/// `hero_image` uses in `main.rs`. The frame is chosen from the creature's own marking seed so a
/// handful of creatures never all share one identical pose phase.
#[allow(clippy::too_many_arguments)]
fn stamp_creature(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    creature: &Creature,
    action: ActionKind,
    anchor_x: i32,
    anchor_y: i32,
    scale: i32,
) {
    let spec = AnimationSpec::for_action(action);
    let frame_count = u64::from(spec.frames.max(1));
    let frame = (creature.appearance.marking_seed % frame_count) as u8;
    let rendered = CreatureRenderer::render_frame(&creature.appearance, action, frame, true);
    let size = FRAME_SIZE as i32 * scale;
    let origin_x = anchor_x - size / 2;
    let origin_y = anchor_y - size;
    blit_rect_alpha(
        pixels,
        width,
        height,
        origin_x,
        origin_y,
        &rendered.rgba_bytes(),
        FRAME_SIZE as i32,
        FRAME_SIZE as i32,
        scale,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_window(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    body: [u8; 4],
) {
    fill_rect_alpha(pixels, width, height, x + 10, y + 12, w, h, [0, 0, 0, 70]);
    fill_rect_alpha(pixels, width, height, x, y, w, h, body);
    fill_rect_alpha(pixels, width, height, x, y, w, 28.min(h), [22, 33, 39, 255]);
    for (offset, dot) in [
        (14, [233, 108, 101, 255]),
        (34, [235, 190, 91, 255]),
        (54, [102, 194, 128, 255]),
    ] {
        fill_rect_alpha(pixels, width, height, x + offset, y + 9, 10, 10, dot);
    }
}

fn scatter_fireflies(pixels: &mut [u8], width: i32, height: i32, count: i32, seed: u64) {
    let mut rng = SceneRng::new(seed);
    let sky_height = height * 7 / 10;
    for _ in 0..count {
        let x = rng.range(width);
        let y = rng.range(sky_height.max(1));
        let warm = rng.range(6) == 0;
        let color = if warm {
            [214, 196, 140, 190]
        } else {
            [255, 255, 255, 120]
        };
        let size = if rng.range(5) == 0 { 2 } else { 1 };
        fill_rect_alpha(pixels, width, height, x, y, size, size, color);
    }
}

struct SceneRng(u64);

impl SceneRng {
    fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn range(&mut self, upper: i32) -> i32 {
        (self.next() % u64::from(upper.max(1) as u32)) as i32
    }
}

fn fill_circle_alpha(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    center_x: i32,
    center_y: i32,
    radius: i32,
    color: [u8; 4],
) {
    if radius <= 0 {
        return;
    }
    let r2 = radius * radius;
    for y in -radius..=radius {
        for x in -radius..=radius {
            if x * x + y * y <= r2 {
                blend_pixel(pixels, width, height, center_x + x, center_y + y, color);
            }
        }
    }
}

fn fill_gradient(pixels: &mut [u8], width: i32, height: i32, top: [u8; 4], bottom: [u8; 4]) {
    for y in 0..height {
        let mix = y as f32 / height.max(1) as f32;
        let color = [
            lerp_u8(top[0], bottom[0], mix),
            lerp_u8(top[1], bottom[1], mix),
            lerp_u8(top[2], bottom[2], mix),
            255,
        ];
        for x in 0..width {
            let index = ((y * width + x) * 4) as usize;
            pixels[index..index + 4].copy_from_slice(&color);
        }
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

#[allow(clippy::too_many_arguments)]
fn fill_rect_alpha(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: [u8; 4],
) {
    for py in y..y + h {
        for px in x..x + w {
            blend_pixel(pixels, width, height, px, py, color);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_rect_alpha(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    source: &[u8],
    source_w: i32,
    source_h: i32,
    scale: i32,
) {
    for sy in 0..source_h {
        for sx in 0..source_w {
            let index = ((sy * source_w + sx) * 4) as usize;
            let color = [
                source[index],
                source[index + 1],
                source[index + 2],
                source[index + 3],
            ];
            if color[3] == 0 {
                continue;
            }
            for oy in 0..scale {
                for ox in 0..scale {
                    blend_pixel(
                        pixels,
                        width,
                        height,
                        x + sx * scale + ox,
                        y + sy * scale + oy,
                        color,
                    );
                }
            }
        }
    }
}

fn blend_pixel(pixels: &mut [u8], width: i32, height: i32, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= width || y >= height {
        return;
    }
    let index = ((y * width + x) * 4) as usize;
    let alpha = color[3] as f32 / 255.0;
    for channel in 0..3 {
        pixels[index + channel] =
            (color[channel] as f32 * alpha + pixels[index + channel] as f32 * (1.0 - alpha)) as u8;
    }
    pixels[index + 3] = 255;
}

// --- A small crisp pixel font, hand-drawn as 5x7 glyphs. --------------------------------------
//
// `crates/formiga-art/src/card.rs` draws its text with `epaint`'s proportional font, but that
// crate does not export its font machinery and `formiga-tools` cannot add a new dependency just
// for this one tool. A blocky bitmap font keeps the title deterministic, dependency-free, and -
// arguably even more fitting for a game about 48x48 pixel-art creatures - genuinely "pixel
// lettering". Only uppercase letters, digits, and a few punctuation marks are defined; anything
// else (beyond plain spaces, which advance the cursor without drawing) is skipped.

const FONT_WIDTH: i32 = 5;
const FONT_HEIGHT: i32 = 7;

#[allow(clippy::too_many_arguments)]
fn draw_text_centered_with_shadow(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    center_x: i32,
    y: i32,
    text: &str,
    scale: i32,
    color: [u8; 4],
    shadow: [u8; 4],
) {
    let text_w = text_width(text, scale);
    let x = center_x - text_w / 2;
    let offset = (scale / 3).max(1);
    draw_text(
        pixels,
        width,
        height,
        x + offset,
        y + offset,
        text,
        scale,
        shadow,
    );
    draw_text(pixels, width, height, x, y, text, scale, color);
}

#[allow(clippy::too_many_arguments)]
fn draw_text(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    text: &str,
    scale: i32,
    color: [u8; 4],
) {
    let mut cursor = x;
    for ch in text.chars() {
        if let Some(rows) = glyph(ch) {
            for (row_index, row) in rows.iter().enumerate() {
                for (col_index, cell) in row.chars().enumerate() {
                    if cell == '#' {
                        fill_rect_alpha(
                            pixels,
                            width,
                            height,
                            cursor + col_index as i32 * scale,
                            y + row_index as i32 * scale,
                            scale,
                            scale,
                            color,
                        );
                    }
                }
            }
        }
        cursor += glyph_advance(ch, scale);
    }
}

fn text_width(text: &str, scale: i32) -> i32 {
    let advance: i32 = text.chars().map(|ch| glyph_advance(ch, scale)).sum();
    advance - scale
}

fn glyph_advance(ch: char, scale: i32) -> i32 {
    if ch == ' ' {
        4 * scale
    } else {
        (FONT_WIDTH + 1) * scale
    }
}

/// Each row is `FONT_WIDTH` characters of `#` (ink) or `.` (empty), top row first.
fn glyph(ch: char) -> Option<[&'static str; 7]> {
    Some(match ch.to_ascii_uppercase() {
        'A' => [
            ".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
        ],
        'B' => [
            "####.", "#...#", "#...#", "####.", "#...#", "#...#", "####.",
        ],
        'C' => [
            ".####", "#....", "#....", "#....", "#....", "#....", ".####",
        ],
        'D' => [
            "####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####.",
        ],
        'E' => [
            "#####", "#....", "#....", "####.", "#....", "#....", "#####",
        ],
        'F' => [
            "#####", "#....", "#....", "####.", "#....", "#....", "#....",
        ],
        'G' => [
            ".####", "#....", "#....", "#.###", "#...#", "#...#", ".####",
        ],
        'H' => [
            "#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
        ],
        'I' => [
            "#####", "..#..", "..#..", "..#..", "..#..", "..#..", "#####",
        ],
        'J' => [
            "..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##..",
        ],
        'K' => [
            "#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#",
        ],
        'L' => [
            "#....", "#....", "#....", "#....", "#....", "#....", "#####",
        ],
        'M' => [
            "#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#",
        ],
        'N' => [
            "#...#", "##..#", "#.#.#", "#.#.#", "#..##", "#...#", "#...#",
        ],
        'O' => [
            ".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
        ],
        'P' => [
            "####.", "#...#", "#...#", "####.", "#....", "#....", "#....",
        ],
        'Q' => [
            ".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#",
        ],
        'R' => [
            "####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#",
        ],
        'S' => [
            ".####", "#....", "#....", ".###.", "....#", "....#", "####.",
        ],
        'T' => [
            "#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#..",
        ],
        'U' => [
            "#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
        ],
        'V' => [
            "#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#..",
        ],
        'W' => [
            "#...#", "#...#", "#...#", "#.#.#", "#.#.#", "##.##", "#...#",
        ],
        'X' => [
            "#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#",
        ],
        'Y' => [
            "#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#..",
        ],
        'Z' => [
            "#####", "....#", "...#.", "..#..", ".#...", "#....", "#####",
        ],
        '0' => [
            ".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###.",
        ],
        '1' => [
            "..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###.",
        ],
        '2' => [
            ".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####",
        ],
        '3' => [
            "####.", "....#", "....#", "..##.", "....#", "....#", "####.",
        ],
        '4' => [
            "...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#.",
        ],
        '5' => [
            "#####", "#....", "#....", "####.", "....#", "....#", "####.",
        ],
        '6' => [
            ".###.", "#....", "#....", "####.", "#...#", "#...#", ".###.",
        ],
        '7' => [
            "#####", "....#", "...#.", "..#..", "..#..", "..#..", "..#..",
        ],
        '8' => [
            ".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###.",
        ],
        '9' => [
            ".###.", "#...#", "#...#", ".####", "....#", "....#", ".###.",
        ],
        '.' => [
            ".....", ".....", ".....", ".....", ".....", ".##..", ".##..",
        ],
        ',' => [
            ".....", ".....", ".....", ".....", ".....", ".##..", "..#..",
        ],
        '\'' => [
            ".##..", ".##..", "..#..", ".....", ".....", ".....", ".....",
        ],
        '-' => [
            ".....", ".....", ".....", "#####", ".....", ".....", ".....",
        ],
        ':' => [
            ".....", ".##..", ".##..", ".....", ".##..", ".##..", ".....",
        ],
        '!' => [
            "..#..", "..#..", "..#..", "..#..", "..#..", ".....", "..#..",
        ],
        _ => return None,
    })
}

fn write_png(path: &Path, width: i32, height: i32, pixels: &[u8]) -> Result<()> {
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn social_scene_is_deterministic_fully_opaque_and_correctly_sized() {
        let first = render_social_scene();
        let second = render_social_scene();
        assert_eq!(first, second);
        assert_eq!(first.len(), (SOCIAL_WIDTH * SOCIAL_HEIGHT * 4) as usize);
        assert!(first.chunks_exact(4).all(|pixel| pixel[3] == 255));
    }

    #[test]
    fn cover_scene_is_deterministic_fully_opaque_and_correctly_sized() {
        let first = render_cover_scene();
        let second = render_cover_scene();
        assert_eq!(first, second);
        assert_eq!(first.len(), (COVER_WIDTH * COVER_HEIGHT * 4) as usize);
        assert!(first.chunks_exact(4).all(|pixel| pixel[3] == 255));
    }

    #[test]
    fn five_body_plans_are_all_represented_and_distinct() {
        let creatures = varied_creatures();
        assert_eq!(creatures.len(), BodyPlan::ALL.len());
        let bodies: Vec<_> = creatures
            .iter()
            .map(|creature| creature.appearance.design.unwrap().body)
            .collect();
        for body in BodyPlan::ALL {
            assert!(bodies.contains(&body));
        }
    }

    #[test]
    fn glyphs_are_five_columns_wide() {
        for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.,'-:!".chars() {
            let rows = glyph(ch).unwrap_or_else(|| panic!("missing glyph for {ch:?}"));
            for row in rows {
                assert_eq!(row.len(), FONT_WIDTH as usize, "row width for {ch:?}");
            }
        }
    }
}
