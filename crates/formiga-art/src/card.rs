use crate::{Canvas, CreatureRenderer, PALETTES, Rgba};
use epaint::{Color32, FontFamily, FontId, Fonts, TextOptions, text::FontDefinitions};
use formiga_core::{
    ActionKind, BodyFamily, Creature, EffectMotif, encode_creature_seed, profile_descriptors,
};
use time::Month;

pub const CARD_WIDTH: u32 = 960;
pub const CARD_HEIGHT: u32 = 600;

const INK: Rgba = Rgba::new(34, 47, 43, 255);
const MUTED_INK: Rgba = Rgba::new(79, 91, 81, 255);
const PAPER: Rgba = Rgba::new(249, 239, 211, 255);
const PAPER_SHADOW: Rgba = Rgba::new(213, 185, 143, 255);
const NIGHT: Rgba = Rgba::new(18, 39, 38, 255);
const NIGHT_LIGHT: Rgba = Rgba::new(27, 61, 56, 255);
const CREAM: Rgba = Rgba::new(255, 246, 221, 255);

/// Stateless by design: all card-only allocations are dropped when an export finishes.
pub struct CreatureCardRenderer;

impl CreatureCardRenderer {
    pub fn render(creature: &Creature) -> Canvas {
        let mut canvas = Canvas::new(CARD_WIDTH, CARD_HEIGHT);
        let palette = PALETTES[usize::from(creature.appearance.palette_index) % PALETTES.len()];
        let mut rng = CardRng::new(creature.appearance.marking_seed);

        draw_background(&mut canvas, &mut rng, palette.accent);
        stepped_panel(&mut canvas, 24, 22, 912, 556, PAPER_SHADOW);
        stepped_panel(&mut canvas, 31, 29, 898, 542, PAPER);

        draw_portrait_panel(&mut canvas, creature, &mut rng);
        draw_divider(&mut canvas, palette.accent);

        let mut text = CardText::new();
        text.draw(&mut canvas, 488, 55, "FORMIGA", 22.0, palette.shadow);
        text.draw(
            &mut canvas,
            488,
            82,
            "DESKTOP CREATURE CARD",
            14.0,
            MUTED_INK,
        );

        let name_size = text.fit_size(&creature.name, 415.0, 54.0, 18.0);
        text.draw(&mut canvas, 486, 119, &creature.name, name_size, INK);

        let family = family_label(creature.appearance.family);
        text.draw(&mut canvas, 488, 184, family, 20.0, MUTED_INK);
        canvas.fill_rect(488, 216, 398, 2, PAPER_SHADOW);

        text.draw(&mut canvas, 488, 242, "KNOWN FOR", 14.0, MUTED_INK);
        let descriptors = profile_descriptors(creature);
        if descriptors.is_empty() {
            draw_chip(
                &mut canvas,
                &mut text,
                488,
                271,
                "Still becoming themself",
                palette.highlight,
            );
        } else {
            let mut x = 488;
            let mut y = 271;
            for descriptor in descriptors {
                let label = descriptor.label();
                let chip_width = (text.measure(label, 17.0).ceil() as i32 + 26).clamp(80, 394);
                if x + chip_width > 894 {
                    x = 488;
                    y += 43;
                }
                draw_chip(&mut canvas, &mut text, x, y, label, palette.highlight);
                x += chip_width + 9;
            }
        }

        draw_field(
            &mut canvas,
            &mut text,
            488,
            392,
            "ARRIVED",
            &birth_month_year(creature),
            palette.accent,
        );
        draw_field(
            &mut canvas,
            &mut text,
            691,
            392,
            "SEED GLIMPSE",
            &abbreviated_seed_code(creature),
            palette.accent,
        );

        canvas.fill_rect(488, 486, 398, 2, PAPER_SHADOW);
        text.draw(
            &mut canvas,
            488,
            506,
            "A TINY LIFE FROM A QUIET DESKTOP COLONY",
            14.0,
            MUTED_INK,
        );
        draw_motif(
            &mut canvas,
            873,
            527,
            creature.appearance.effect_motif,
            2,
            palette.accent,
        );
        canvas
    }
}

pub fn abbreviated_seed_code(creature: &Creature) -> String {
    let full = encode_creature_seed(creature.origin);
    let body: String = full
        .strip_prefix("FORMIGA-")
        .unwrap_or(&full)
        .chars()
        .filter(|character| *character != '-')
        .collect();
    let first = &body[..4.min(body.len())];
    let last_start = body.len().saturating_sub(4);
    format!("{first}…{}", &body[last_start..])
}

fn draw_background(canvas: &mut Canvas, rng: &mut CardRng, accent: Rgba) {
    canvas.fill_rect(0, 0, CARD_WIDTH as i32, CARD_HEIGHT as i32, NIGHT);
    canvas.fill_rect(0, 0, CARD_WIDTH as i32, 16, NIGHT_LIGHT);
    canvas.fill_rect(
        0,
        CARD_HEIGHT as i32 - 16,
        CARD_WIDTH as i32,
        16,
        NIGHT_LIGHT,
    );
    for _ in 0..150 {
        let x = rng.range(CARD_WIDTH as i32);
        let y = rng.range(CARD_HEIGHT as i32);
        let color = if rng.range(5) == 0 {
            accent
        } else {
            NIGHT_LIGHT
        };
        canvas.fill_rect(x, y, if rng.range(4) == 0 { 3 } else { 2 }, 2, color);
    }
}

fn draw_portrait_panel(canvas: &mut Canvas, creature: &Creature, rng: &mut CardRng) {
    let palette = PALETTES[usize::from(creature.appearance.palette_index) % PALETTES.len()];
    stepped_panel(canvas, 57, 57, 382, 486, palette.outline);
    stepped_panel(canvas, 64, 64, 368, 472, NIGHT_LIGHT);
    canvas.fill_rect(72, 72, 352, 328, Rgba::new(35, 75, 69, 255));
    canvas.fill_rect(72, 72, 352, 114, Rgba::new(42, 87, 79, 255));
    canvas.fill_rect(72, 186, 352, 106, Rgba::new(31, 67, 63, 255));
    canvas.fill_rect(72, 292, 352, 108, Rgba::new(24, 54, 52, 255));

    canvas.fill_circle(347, 126, 34, CREAM);
    canvas.fill_circle(337, 117, 28, Rgba::new(255, 226, 145, 255));
    for _ in 0..24 {
        let x = 86 + rng.range(312);
        let y = 86 + rng.range(245);
        let size = if rng.range(5) == 0 { 3 } else { 2 };
        canvas.fill_rect(x, y, size, size, Rgba::new(174, 222, 176, 255));
    }

    draw_vine(canvas, 83, 84, true, palette.accent);
    draw_vine(canvas, 410, 83, false, palette.accent);
    canvas.fill_rect(72, 391, 352, 9, palette.shadow);
    canvas.fill_rect(82, 400, 332, 7, palette.outline);

    let sprite = CreatureRenderer::render_frame(&creature.appearance, ActionKind::Greet, 2, true);
    blit_scaled(canvas, &sprite, 101, 119, 6);

    canvas.fill_rect(81, 433, 334, 70, Rgba::new(241, 220, 179, 255));
    canvas.fill_rect(81, 433, 334, 5, palette.accent);
    let mut text = CardText::new();
    text.draw(canvas, 103, 448, "COLONY KEEPSAKE", 15.0, MUTED_INK);
    text.draw(
        canvas,
        103,
        472,
        &format!("NO. {:02}", creature.colony_order.saturating_add(1)),
        18.0,
        INK,
    );
    draw_motif(
        canvas,
        376,
        467,
        creature.appearance.effect_motif,
        3,
        palette.accent,
    );
}

fn draw_divider(canvas: &mut Canvas, accent: Rgba) {
    canvas.fill_rect(463, 58, 2, 480, PAPER_SHADOW);
    for y in (70..530).step_by(28) {
        canvas.fill_rect(459, y, 10, 3, accent);
    }
}

fn draw_chip(canvas: &mut Canvas, text: &mut CardText, x: i32, y: i32, label: &str, tint: Rgba) {
    let width = (text.measure(label, 17.0).ceil() as i32 + 26).clamp(80, 394);
    stepped_panel(canvas, x, y, width, 34, tint);
    canvas.fill_rect(x + 5, y + 5, width - 10, 24, CREAM);
    text.draw(canvas, x + 13, y + 7, label, 17.0, INK);
}

fn draw_field(
    canvas: &mut Canvas,
    text: &mut CardText,
    x: i32,
    y: i32,
    label: &str,
    value: &str,
    accent: Rgba,
) {
    canvas.fill_rect(x, y, 6, 58, accent);
    text.draw(canvas, x + 16, y, label, 13.0, MUTED_INK);
    text.draw(canvas, x + 16, y + 23, value, 19.0, INK);
}

fn birth_month_year(creature: &Creature) -> String {
    format!(
        "{} {}",
        month_label(creature.born_at_utc.month()),
        creature.born_at_utc.year()
    )
}

const fn month_label(month: Month) -> &'static str {
    match month {
        Month::January => "January",
        Month::February => "February",
        Month::March => "March",
        Month::April => "April",
        Month::May => "May",
        Month::June => "June",
        Month::July => "July",
        Month::August => "August",
        Month::September => "September",
        Month::October => "October",
        Month::November => "November",
        Month::December => "December",
    }
}

const fn family_label(family: BodyFamily) -> &'static str {
    match family {
        BodyFamily::Blob => "Blob",
        BodyFamily::Hopper => "Hopper",
        BodyFamily::SoftQuadruped => "Soft Quadruped",
    }
}

fn stepped_panel(canvas: &mut Canvas, x: i32, y: i32, width: i32, height: i32, color: Rgba) {
    canvas.fill_rect(x + 7, y, width - 14, height, color);
    canvas.fill_rect(x, y + 7, width, height - 14, color);
}

fn draw_vine(canvas: &mut Canvas, x: i32, y: i32, right: bool, color: Rgba) {
    let direction = if right { 1 } else { -1 };
    canvas.line(x, y, x + direction * 18, y + 90, 2, color);
    for offset in [18, 42, 67] {
        let stem_x = x + direction * offset / 5;
        let stem_y = y + offset;
        canvas.fill_ellipse(stem_x + direction * 7, stem_y, 8, 4, color);
    }
}

fn draw_motif(canvas: &mut Canvas, x: i32, y: i32, motif: EffectMotif, scale: i32, color: Rgba) {
    match motif {
        EffectMotif::Heart => {
            canvas.fill_circle(x - 2 * scale, y - scale, 2 * scale, color);
            canvas.fill_circle(x + 2 * scale, y - scale, 2 * scale, color);
            canvas.line(x - 4 * scale, y, x, y + 5 * scale, scale, color);
            canvas.line(x + 4 * scale, y, x, y + 5 * scale, scale, color);
        }
        EffectMotif::Leaf => {
            canvas.fill_ellipse(x, y, 5 * scale, 3 * scale, color);
            canvas.line(
                x - 4 * scale,
                y + 3 * scale,
                x + 5 * scale,
                y - 3 * scale,
                scale,
                INK,
            );
        }
        EffectMotif::Star | EffectMotif::Spark => {
            canvas.line(x - 5 * scale, y, x + 5 * scale, y, scale, color);
            canvas.line(x, y - 5 * scale, x, y + 5 * scale, scale, color);
            canvas.line(
                x - 3 * scale,
                y - 3 * scale,
                x + 3 * scale,
                y + 3 * scale,
                scale,
                color,
            );
            canvas.line(
                x + 3 * scale,
                y - 3 * scale,
                x - 3 * scale,
                y + 3 * scale,
                scale,
                color,
            );
        }
        EffectMotif::Dot | EffectMotif::None => canvas.fill_circle(x, y, 5 * scale, color),
    }
}

fn blit_scaled(destination: &mut Canvas, source: &Canvas, x: i32, y: i32, scale: i32) {
    for source_y in 0..source.height() as i32 {
        for source_x in 0..source.width() as i32 {
            let pixel = source.get(source_x, source_y);
            if pixel.a == 0 {
                continue;
            }
            let target_x = x + source_x * scale;
            let target_y = y + source_y * scale;
            for dy in 0..scale {
                for dx in 0..scale {
                    let px = target_x + dx;
                    let py = target_y + dy;
                    let foreground = Rgba::new(pixel.r, pixel.g, pixel.b, 255);
                    destination.set(px, py, blend(destination.get(px, py), foreground, pixel.a));
                }
            }
        }
    }
}

fn blend(destination: Rgba, foreground: Rgba, coverage: u8) -> Rgba {
    let alpha = u32::from(coverage) * u32::from(foreground.a) / 255;
    let inverse = 255 - alpha;
    Rgba::new(
        ((u32::from(foreground.r) * alpha + u32::from(destination.r) * inverse) / 255) as u8,
        ((u32::from(foreground.g) * alpha + u32::from(destination.g) * inverse) / 255) as u8,
        ((u32::from(foreground.b) * alpha + u32::from(destination.b) * inverse) / 255) as u8,
        255,
    )
}

struct CardText {
    fonts: Fonts,
}

impl CardText {
    fn new() -> Self {
        Self {
            fonts: Fonts::new(TextOptions::default(), FontDefinitions::default()),
        }
    }

    fn measure(&mut self, value: &str, size: f32) -> f32 {
        self.layout(value, size).rect.width()
    }

    fn fit_size(&mut self, value: &str, maximum_width: f32, start: f32, minimum: f32) -> f32 {
        let mut size = start;
        while size > minimum && self.measure(value, size) > maximum_width {
            size -= 2.0;
        }
        size
    }

    fn layout(&mut self, value: &str, size: f32) -> std::sync::Arc<epaint::Galley> {
        self.fonts.with_pixels_per_point(1.0).layout_no_wrap(
            value.to_owned(),
            FontId::new(size, FontFamily::Proportional),
            Color32::WHITE,
        )
    }

    fn draw(&mut self, canvas: &mut Canvas, x: i32, y: i32, value: &str, size: f32, color: Rgba) {
        let galley = self.layout(value, size);
        let atlas = self.fonts.image();
        for placed_row in &galley.rows {
            for glyph in &placed_row.glyphs {
                if glyph.uv_rect.is_nothing() {
                    continue;
                }
                let target_x =
                    x + (placed_row.pos.x + glyph.pos.x + glyph.uv_rect.offset.x).round() as i32;
                let target_y =
                    y + (placed_row.pos.y + glyph.pos.y + glyph.uv_rect.offset.y).round() as i32;
                for source_y in glyph.uv_rect.min[1]..glyph.uv_rect.max[1] {
                    for source_x in glyph.uv_rect.min[0]..glyph.uv_rect.max[0] {
                        let source = atlas.pixels
                            [usize::from(source_y) * atlas.size[0] + usize::from(source_x)];
                        let coverage = source.a();
                        if coverage == 0 {
                            continue;
                        }
                        let px = target_x + i32::from(source_x - glyph.uv_rect.min[0]);
                        let py = target_y + i32::from(source_y - glyph.uv_rect.min[1]);
                        canvas.set(px, py, blend(canvas.get(px, py), color, coverage));
                    }
                }
            }
        }
    }
}

struct CardRng(u64);

impl CardRng {
    fn new(seed: u64) -> Self {
        Self(seed ^ 0x464F_524D_4947_4121)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn range(&mut self, upper: i32) -> i32 {
        (self.next() % upper.max(1) as u64) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::{DesktopRect, DesktopSnapshot, DisplayKey, MonitorInfo, World};
    use sha2::{Digest as _, Sha256};
    use time::macros::datetime;

    fn creature() -> Creature {
        let desktop = DesktopSnapshot {
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
                    height: 876.0,
                },
                scale_factor: 1.0,
                primary: true,
            }],
            ..DesktopSnapshot::default()
        };
        World::new([7; 32], datetime!(2026-08-14 12:30 UTC), &desktop)
            .save
            .creatures
            .remove(0)
    }

    #[test]
    fn card_is_fixed_size_opaque_and_pixel_deterministic() {
        let creature = creature();
        let first = CreatureCardRenderer::render(&creature);
        let second = CreatureCardRenderer::render(&creature);
        assert_eq!((first.width(), first.height()), (CARD_WIDTH, CARD_HEIGHT));
        assert_eq!(first, second);
        assert!(first.pixels().iter().all(|pixel| pixel.a == 255));
        let digest = Sha256::digest(first.rgba_bytes());
        assert_ne!(digest.as_slice(), &[0; 32]);
    }

    #[test]
    fn unicode_names_render_without_leaving_the_card() {
        let mut creature = creature();
        creature.name = "Mochi 雪 ✨ très doux".into();
        let card = CreatureCardRenderer::render(&creature);
        assert_eq!((card.width(), card.height()), (960, 600));
        assert!(card.pixels().iter().all(|pixel| pixel.a == 255));
    }

    #[test]
    fn maximum_length_wide_name_fits_its_readable_column() {
        let mut text = CardText::new();
        let name = "WWWWWWWWWWWWWWWWWWWWWWWW";
        let size = text.fit_size(name, 415.0, 54.0, 18.0);
        assert!(size >= 18.0);
        assert!(text.measure(name, size) <= 415.0);
    }

    #[test]
    fn abbreviated_code_does_not_expose_full_seed() {
        let creature = creature();
        let abbreviated = abbreviated_seed_code(&creature);
        let full = encode_creature_seed(creature.origin);
        assert_eq!(abbreviated.chars().count(), 9);
        assert!(!full.contains(&abbreviated));
        assert!(!abbreviated.contains("FORMIGA"));
    }

    #[test]
    fn renderer_holds_no_persistent_export_state() {
        assert_eq!(std::mem::size_of::<CreatureCardRenderer>(), 0);
    }
}
