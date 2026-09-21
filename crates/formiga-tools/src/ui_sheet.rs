//! Review sheet for the on-desktop UI art: thought bubbles and the right-click creature menu.
//!
//! Everything is drawn at 4x nearest neighbour over three backgrounds — light, dark, and a busy
//! checker — because the overlay has no say in what is behind it. The last section repeats the
//! full bubbles and the menus at 2x, which is the smallest scale anyone runs the companion at
//! and therefore the only scale that really decides whether an icon reads.

use anyhow::{Context, Result};
use formiga_art::{
    BUBBLE_ANCHOR, BUBBLE_CELL, Canvas, CreatureRenderer, LABEL_TAB_GAP, LABEL_TAB_HEIGHT,
    MENU_STRIP_HEIGHT, MenuIcon, MenuLayout, SpriteRect, UiAtlasRenderer,
};
use formiga_core::{
    ActionKind, BubbleGrowth, BubbleIcon, Creature, DesktopRect, DesktopSnapshot, DisplayKey,
    MonitorInfo, World,
};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

const SCALE: u32 = 4;
const SMALL_SCALE: u32 = 2;

/// One bubble scene, in art pixels: a creature's head with a bubble over it.
const BUBBLE_SCENE: (u32, u32) = (40, 40);
/// One menu scene, in art pixels: a strip, a label tab, and the head it points at. Wide enough
/// that four of them span the same width as the fourteen bubble columns above.
const MENU_SCENE: (u32, u32) = (140, 56);
/// One moments scene: a colony menu with the strip of moments open beside it.
const MOMENT_SCENE: (u32, u32) = (180, 56);
/// Art pixels between a menu and the strip beside it, as the desktop places it.
const SIDE_GAP: i32 = 2;

/// The strips of moments shown open beside a menu, and which of their cells is hovered.
const MOMENT_STRIPS: [(&[MenuIcon], usize); 3] = [
    (&[MenuIcon::Picnic, MenuIcon::Dance, MenuIcon::Nap], 0),
    (&[MenuIcon::Picnic, MenuIcon::Dance, MenuIcon::Nap], 2),
    (&[MenuIcon::Stop, MenuIcon::Dance, MenuIcon::Nap], 0),
];

const GUTTER: u32 = 152;
const HEADER: u32 = 30;
const NAME_ROW: u32 = 22;
const BAND_GAP: u32 = 14;

const SHEET_WIDTH: u32 = GUTTER + BubbleIcon::ALL.len() as u32 * BUBBLE_SCENE.0 * SCALE;

/// The menus a creature can be offered. The first two are a colony creature's, out on the desktop
/// and at home with the houses out; the other two are the two shapes a visitor's menu takes.
const MENUS: [(&str, [MenuIcon; 4]); 4] = [
    (
        "colony",
        [
            MenuIcon::Snack,
            MenuIcon::Toy,
            MenuIcon::Home,
            MenuIcon::Profile,
        ],
    ),
    (
        "at home",
        [
            MenuIcon::Snack,
            MenuIcon::Toy,
            MenuIcon::Moment,
            MenuIcon::Profile,
        ],
    ),
    (
        "visitor stay",
        [
            MenuIcon::Snack,
            MenuIcon::Toy,
            MenuIcon::Stay,
            MenuIcon::Profile,
        ],
    ),
    (
        "visitor code",
        [
            MenuIcon::Snack,
            MenuIcon::Toy,
            MenuIcon::CopyCode,
            MenuIcon::Profile,
        ],
    ),
];

const GROWTHS: [(&str, BubbleGrowth); 3] = [
    ("full", BubbleGrowth::Full),
    ("medium", BubbleGrowth::Medium),
    ("small", BubbleGrowth::Small),
];

const BACKGROUNDS: [Background; 3] = [Background::Light, Background::Dark, Background::Busy];

pub fn run(path: PathBuf) -> Result<()> {
    let atlas = UiAtlasRenderer::render();
    let creature = sample_creature();
    let frame = CreatureRenderer::render_frame(&creature.appearance, ActionKind::Idle, 0, true);

    let bubble_band = HEADER + NAME_ROW + GROWTHS.len() as u32 * BUBBLE_SCENE.1 * SCALE + BAND_GAP;
    let menu_band = HEADER + MENUS.len() as u32 * MENU_SCENE.1 * SCALE + BAND_GAP;
    let moment_band = HEADER + MOMENT_SCENE.1 * SCALE + BAND_GAP;
    let small_band = HEADER + 116 + BAND_GAP;
    let height = 8 + 3 * (bubble_band + menu_band + moment_band + small_band) + 8;

    let mut painter = Painter {
        sheet: Sheet::new(SHEET_WIDTH, height),
        atlas: &atlas,
        frame: &frame,
    };
    let mut y = 8;
    for background in BACKGROUNDS {
        y = painter.bubble_band(background, y);
    }
    for background in BACKGROUNDS {
        y = painter.menu_band(background, y);
    }
    for background in BACKGROUNDS {
        y = painter.moment_band(background, y);
    }
    for background in BACKGROUNDS {
        y = painter.small_band(background, y);
    }

    let sheet = painter.sheet;
    write_png(&path, sheet.width, sheet.height, &sheet.pixels)?;
    println!(
        "wrote {} ({}x{})",
        path.display(),
        sheet.width,
        sheet.height
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Bands and scenes
// ---------------------------------------------------------------------------------------------

/// The sheet under construction, plus the two textures every scene draws from.
struct Painter<'a> {
    sheet: Sheet,
    atlas: &'a Canvas,
    frame: &'a Canvas,
}

impl Painter<'_> {
    fn bubble_band(&mut self, background: Background, top: u32) -> u32 {
        self.sheet
            .header(top, &format!("bubbles 4x on {}", background.name()));
        let names_y = top + HEADER;
        for (column, icon) in BubbleIcon::ALL.into_iter().enumerate() {
            let x = GUTTER + column as u32 * BUBBLE_SCENE.0 * SCALE;
            self.sheet
                .text(x + 6, names_y + 5, &icon_name(icon), 2, INK);
        }
        let grid_y = names_y + NAME_ROW;
        for (row, (label, growth)) in GROWTHS.into_iter().enumerate() {
            let y = grid_y + row as u32 * BUBBLE_SCENE.1 * SCALE;
            self.sheet.text(12, y + 14, label, 3, INK);
            for (column, icon) in BubbleIcon::ALL.into_iter().enumerate() {
                let x = GUTTER + column as u32 * BUBBLE_SCENE.0 * SCALE;
                self.bubble_scene(background, x, y, icon, growth);
            }
        }
        grid_y + GROWTHS.len() as u32 * BUBBLE_SCENE.1 * SCALE + BAND_GAP
    }

    fn menu_band(&mut self, background: Background, top: u32) -> u32 {
        self.sheet
            .header(top, &format!("menus 4x on {}", background.name()));
        let grid_y = top + HEADER;
        for (row, (label, items)) in MENUS.into_iter().enumerate() {
            let y = grid_y + row as u32 * MENU_SCENE.1 * SCALE;
            self.sheet.text(12, y + 16, label, 3, INK);
            for hovered in 0..items.len() {
                let x = GUTTER + hovered as u32 * MENU_SCENE.0 * SCALE;
                self.menu_scene(background, x, y, hovered, &items);
            }
        }
        grid_y + MENUS.len() as u32 * MENU_SCENE.1 * SCALE + BAND_GAP
    }

    /// A colony menu at home with its strip of moments open beside it, a different cell of the
    /// strip hovered in each scene: what the owner sees after choosing Moment.
    fn moment_band(&mut self, background: Background, top: u32) -> u32 {
        self.sheet
            .header(top, &format!("moments 4x on {}", background.name()));
        let y = top + HEADER;
        for (index, (strip, hovered)) in MOMENT_STRIPS.into_iter().enumerate() {
            let x = GUTTER + index as u32 * MOMENT_SCENE.0 * SCALE;
            self.moment_scene(background, x, y, strip, hovered);
        }
        y + MOMENT_SCENE.1 * SCALE + BAND_GAP
    }

    fn moment_scene(
        &mut self,
        background: Background,
        x: u32,
        y: u32,
        strip: &[MenuIcon],
        hovered: usize,
    ) {
        let (width, height) = (MOMENT_SCENE.0 * SCALE, MOMENT_SCENE.1 * SCALE);
        self.sheet.fill(x, y, width, height, background.base());
        background.decorate(&mut self.sheet, x, y, width, height);

        let items = MENUS[1].1;
        let layout = MenuLayout::new(&items);
        // The creature a third of the way in, so the strip beside its menu has room.
        let centre = MOMENT_SCENE.0 as i32 / 3;
        let strip_x = centre - layout.notch_x();
        let head_top =
            MENU_STRIP_HEIGHT as i32 + LABEL_TAB_GAP as i32 + LABEL_TAB_HEIGHT as i32 + 5;
        let bounds = self.frame.alpha_bounds().unwrap_or((0, 0, 0, 0));
        let frame_x = centre - (bounds.0 + bounds.2) as i32 / 2;
        self.sheet.blit_canvas(
            self.frame,
            x as i32 + frame_x * SCALE as i32,
            y as i32 + (head_top - bounds.1 as i32) * SCALE as i32,
            SCALE,
        );
        let origin = (x as i32 + strip_x * SCALE as i32, y as i32 + SCALE as i32);
        // Nothing on the menu itself is hovered: the cursor has moved on to the strip.
        self.menu(&layout, &items, origin, usize::MAX, SCALE);
        let side = MenuLayout::new(strip);
        let side_origin = (
            origin.0 + (layout.size().0 as i32 + SIDE_GAP) * SCALE as i32,
            origin.1,
        );
        self.side_strip(&side, strip, side_origin, hovered, SCALE);
    }

    /// A strip beside a menu: its plain tray, its cells, and the hovered item's label tab hung
    /// level with the menu's own.
    fn side_strip(
        &mut self,
        layout: &MenuLayout,
        items: &[MenuIcon],
        origin: (i32, i32),
        hovered: usize,
        scale: u32,
    ) {
        let (x, y) = origin;
        let tray =
            UiAtlasRenderer::menu_frame_plain(items.len() as u8).expect("2..=4 cells have a tray");
        self.sheet.blit(self.atlas, tray, x, y, scale);
        for (index, icon) in items.iter().enumerate() {
            let Some(cell) = layout.cell(index) else {
                continue;
            };
            let rect = UiAtlasRenderer::menu_icon(*icon, index == hovered);
            self.sheet.blit(
                self.atlas,
                rect,
                x + cell.x * scale as i32,
                y + cell.y * scale as i32,
                scale,
            );
        }
        if let (Some(tab), Some(icon)) = (layout.label_tab(hovered), layout.item(hovered)) {
            self.sheet.blit(
                self.atlas,
                UiAtlasRenderer::menu_label(icon),
                x + tab.x * scale as i32,
                y + tab.y * scale as i32,
                scale,
            );
        }
    }

    /// The scale that decides everything: 2x, with the bubbles in a clear row and one creature
    /// beside them so their size against a head stays honest.
    fn small_band(&mut self, background: Background, top: u32) -> u32 {
        self.sheet.header(
            top,
            &format!("2x, the smallest scale run, on {}", background.name()),
        );
        let y = top + HEADER;
        self.sheet.fill(0, y, SHEET_WIDTH, 116, background.base());
        background.decorate(&mut self.sheet, 0, y, SHEET_WIDTH, 116);

        for (index, icon) in BubbleIcon::ALL.into_iter().enumerate() {
            let x = 12 + index as u32 * (BUBBLE_CELL.0 + 2) * SMALL_SCALE;
            let rect = UiAtlasRenderer::bubble(icon, BubbleGrowth::Full);
            self.sheet
                .blit(self.atlas, rect, x as i32, y as i32 + 8, SMALL_SCALE);
        }

        // Each menu, first cell hovered, with its label, and the moments open beside the one a
        // companion is offered at home.
        let menus_y = y as i32 + 8 + (BUBBLE_CELL.1 as i32 + 6) * SMALL_SCALE as i32;
        let mut x = 12;
        for (name, items) in MENUS {
            let layout = MenuLayout::new(&items);
            self.menu(&layout, &items, (x as i32, menus_y), 0, SMALL_SCALE);
            x += (layout.size().0 + 12) * SMALL_SCALE;
            if name == "at home" {
                let (strip, _) = MOMENT_STRIPS[0];
                let side = MenuLayout::new(strip);
                let side_x = x as i32 - (12 - SIDE_GAP) * SMALL_SCALE as i32;
                self.side_strip(&side, strip, (side_x, menus_y), 1, SMALL_SCALE);
                x = side_x as u32 + (side.size().0 + 12) * SMALL_SCALE;
            }
        }

        // Past whichever runs further, the row of bubbles or the row of menus.
        let creature_x =
            (24 + BubbleIcon::ALL.len() as u32 * (BUBBLE_CELL.0 + 2) * SMALL_SCALE).max(x + 12);
        let bounds = self.frame.alpha_bounds().unwrap_or((0, 0, 0, 0));
        self.sheet.blit_canvas(
            self.frame,
            creature_x as i32,
            menus_y - bounds.1 as i32 * SMALL_SCALE as i32,
            SMALL_SCALE,
        );
        let head_centre = creature_x as i32 + (bounds.0 + bounds.2) as i32 / 2 * SMALL_SCALE as i32;
        let rect = UiAtlasRenderer::bubble(BubbleIcon::Heart, BubbleGrowth::Full);
        self.sheet.blit(
            self.atlas,
            rect,
            head_centre - BUBBLE_ANCHOR.0 as i32 * SMALL_SCALE as i32,
            menus_y - (BUBBLE_ANCHOR.1 as i32 + 1) * SMALL_SCALE as i32,
            SMALL_SCALE,
        );

        y + 116 + BAND_GAP
    }

    fn bubble_scene(
        &mut self,
        background: Background,
        x: u32,
        y: u32,
        icon: BubbleIcon,
        growth: BubbleGrowth,
    ) {
        let (width, height) = (BUBBLE_SCENE.0 * SCALE, BUBBLE_SCENE.1 * SCALE);
        self.sheet.fill(x, y, width, height, background.base());
        background.decorate(&mut self.sheet, x, y, width, height);

        // The head crown sits this far down the scene; the bubble hangs one pixel above it.
        const HEAD_TOP: i32 = 18;
        let centre = BUBBLE_SCENE.0 as i32 / 2;
        let bounds = self.frame.alpha_bounds().unwrap_or((0, 0, 0, 0));
        let frame_x = centre - (bounds.0 + bounds.2) as i32 / 2;
        self.sheet.blit_canvas(
            self.frame,
            x as i32 + frame_x * SCALE as i32,
            y as i32 + (HEAD_TOP - bounds.1 as i32) * SCALE as i32,
            SCALE,
        );

        let rect = UiAtlasRenderer::bubble(icon, growth);
        self.sheet.blit(
            self.atlas,
            rect,
            x as i32 + (centre - BUBBLE_ANCHOR.0 as i32) * SCALE as i32,
            y as i32 + (HEAD_TOP - 1 - BUBBLE_ANCHOR.1 as i32) * SCALE as i32,
            SCALE,
        );
    }

    fn menu_scene(
        &mut self,
        background: Background,
        x: u32,
        y: u32,
        hovered: usize,
        items: &[MenuIcon; 4],
    ) {
        let (width, height) = (MENU_SCENE.0 * SCALE, MENU_SCENE.1 * SCALE);
        self.sheet.fill(x, y, width, height, background.base());
        background.decorate(&mut self.sheet, x, y, width, height);

        let layout = MenuLayout::new(items);
        let centre = MENU_SCENE.0 as i32 / 2;
        let strip_x = centre - layout.notch_x();
        let head_top =
            MENU_STRIP_HEIGHT as i32 + LABEL_TAB_GAP as i32 + LABEL_TAB_HEIGHT as i32 + 5;

        let bounds = self.frame.alpha_bounds().unwrap_or((0, 0, 0, 0));
        let frame_x = centre - (bounds.0 + bounds.2) as i32 / 2;
        self.sheet.blit_canvas(
            self.frame,
            x as i32 + frame_x * SCALE as i32,
            y as i32 + (head_top - bounds.1 as i32) * SCALE as i32,
            SCALE,
        );

        let origin = (x as i32 + strip_x * SCALE as i32, y as i32 + SCALE as i32);
        self.menu(&layout, items, origin, hovered, SCALE);
    }

    /// The frame, its cells, and the hovered item's label tab, at `scale`.
    fn menu(
        &mut self,
        layout: &MenuLayout,
        items: &[MenuIcon],
        origin: (i32, i32),
        hovered: usize,
        scale: u32,
    ) {
        let (x, y) = origin;
        let frame =
            UiAtlasRenderer::menu_frame(items.len() as u8).expect("2..=4 cells have a frame");
        self.sheet.blit(self.atlas, frame, x, y, scale);
        for (index, icon) in items.iter().enumerate() {
            let Some(cell) = layout.cell(index) else {
                continue;
            };
            let rect = UiAtlasRenderer::menu_icon(*icon, index == hovered);
            self.sheet.blit(
                self.atlas,
                rect,
                x + cell.x * scale as i32,
                y + cell.y * scale as i32,
                scale,
            );
        }
        if let (Some(tab), Some(icon)) = (layout.label_tab(hovered), layout.item(hovered)) {
            self.sheet.blit(
                self.atlas,
                UiAtlasRenderer::menu_label(icon),
                x + tab.x * scale as i32,
                y + tab.y * scale as i32,
                scale,
            );
        }
    }
}

fn icon_name(icon: BubbleIcon) -> String {
    format!("{icon:?}").to_lowercase()
}

fn sample_creature() -> Creature {
    let mut seed = [0_u8; 32];
    seed.copy_from_slice(&Sha256::digest(b"formiga-ui-sheet"));
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
                height: 826.0,
            },
            scale_factor: 2.0,
            primary: true,
        }],
        ..DesktopSnapshot::default()
    };
    World::new(seed, OffsetDateTime::UNIX_EPOCH, &desktop)
        .save
        .creatures
        .remove(0)
}

// ---------------------------------------------------------------------------------------------
// Backgrounds
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Background {
    Light,
    Dark,
    Busy,
}

impl Background {
    fn name(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::Busy => "busy",
        }
    }

    fn base(self) -> [u8; 4] {
        match self {
            Self::Light => [233, 231, 226, 255],
            Self::Dark => [26, 30, 38, 255],
            // Light tiles under dark ones: the hardest case is cream paper on a pale wallpaper.
            Self::Busy => [186, 190, 176, 255],
        }
    }

    /// The busy background is the one that matters: a checker plus scattered speckle, roughly a
    /// photo wallpaper's worth of contrast in both directions.
    fn decorate(self, sheet: &mut Sheet, x: u32, y: u32, width: u32, height: u32) {
        if self != Self::Busy {
            return;
        }
        for row in 0..height.div_ceil(16) {
            for column in 0..width.div_ceil(16) {
                if (row + column) % 2 == 0 {
                    continue;
                }
                sheet.fill(
                    x + column * 16,
                    y + row * 16,
                    16.min(width.saturating_sub(column * 16)),
                    16.min(height.saturating_sub(row * 16)),
                    [58, 66, 74, 255],
                );
            }
        }
        // A fixed, seed-free speckle so the sheet stays byte-identical between runs.
        let mut noise = 0x9e37_79b9_u32;
        for _ in 0..(width * height / 90) {
            noise = noise.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let dx = (noise >> 8) % width;
            let dy = (noise >> 20) % height;
            let tone = if noise.is_multiple_of(2) {
                [212, 206, 190, 255]
            } else {
                [30, 34, 40, 255]
            };
            sheet.fill(x + dx, y + dy, 3, 3, tone);
        }
    }
}

const INK: [u8; 4] = [224, 226, 232, 255];

// ---------------------------------------------------------------------------------------------
// A plain RGBA sheet
// ---------------------------------------------------------------------------------------------

struct Sheet {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Sheet {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0x14; (width * height * 4) as usize],
        }
    }

    fn blend(&mut self, x: i32, y: i32, color: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 || color[3] == 0 {
            return;
        }
        let index = ((y as u32 * self.width + x as u32) * 4) as usize;
        let alpha = u32::from(color[3]);
        for (channel, value) in color[..3].iter().enumerate() {
            let source = u32::from(*value) * alpha;
            let target = u32::from(self.pixels[index + channel]) * (255 - alpha);
            self.pixels[index + channel] = ((source + target) / 255) as u8;
        }
        self.pixels[index + 3] = 255;
    }

    fn fill(&mut self, x: u32, y: u32, width: u32, height: u32, color: [u8; 4]) {
        for py in y..y + height {
            for px in x..x + width {
                self.blend(px as i32, py as i32, color);
            }
        }
    }

    fn blit(&mut self, source: &Canvas, rect: SpriteRect, x: i32, y: i32, scale: u32) {
        for sy in 0..rect.height {
            for sx in 0..rect.width {
                let pixel = source.get((rect.x + sx) as i32, (rect.y + sy) as i32);
                if pixel.a == 0 {
                    continue;
                }
                let color = [pixel.r, pixel.g, pixel.b, pixel.a];
                for oy in 0..scale {
                    for ox in 0..scale {
                        self.blend(
                            x + (sx * scale + ox) as i32,
                            y + (sy * scale + oy) as i32,
                            color,
                        );
                    }
                }
            }
        }
    }

    fn blit_canvas(&mut self, source: &Canvas, x: i32, y: i32, scale: u32) {
        let rect = SpriteRect {
            x: 0,
            y: 0,
            width: source.width(),
            height: source.height(),
        };
        self.blit(source, rect, x, y, scale);
    }

    fn header(&mut self, y: u32, text: &str) {
        self.fill(0, y, self.width, HEADER - 6, [38, 41, 48, 255]);
        self.text(12, y + 6, text, 3, INK);
    }

    /// A 3x5 pixel face, only for this review sheet. The shipped labels live in the art crate.
    fn text(&mut self, x: u32, y: u32, text: &str, scale: u32, color: [u8; 4]) {
        for (index, letter) in text.chars().enumerate() {
            let glyph = match letter {
                'a'..='z' => FONT[letter as usize - 'a' as usize],
                'A'..='Z' => FONT[letter as usize - 'A' as usize],
                _ => [0; 5],
            };
            let origin = x + index as u32 * 4 * scale;
            for (row, bits) in glyph.into_iter().enumerate() {
                for column in 0..3 {
                    if bits & (0b100 >> column) == 0 {
                        continue;
                    }
                    self.fill(
                        origin + column * scale,
                        y + row as u32 * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
    }
}

/// `a` through `z`, three pixels wide and five tall, the leftmost column in bit 2.
#[rustfmt::skip]
const FONT: [[u8; 5]; 26] = [
    [0b010, 0b101, 0b111, 0b101, 0b101], // a
    [0b110, 0b101, 0b110, 0b101, 0b110], // b
    [0b011, 0b100, 0b100, 0b100, 0b011], // c
    [0b110, 0b101, 0b101, 0b101, 0b110], // d
    [0b111, 0b100, 0b110, 0b100, 0b111], // e
    [0b111, 0b100, 0b110, 0b100, 0b100], // f
    [0b011, 0b100, 0b101, 0b101, 0b011], // g
    [0b101, 0b101, 0b111, 0b101, 0b101], // h
    [0b111, 0b010, 0b010, 0b010, 0b111], // i
    [0b001, 0b001, 0b001, 0b101, 0b010], // j
    [0b101, 0b101, 0b110, 0b101, 0b101], // k
    [0b100, 0b100, 0b100, 0b100, 0b111], // l
    [0b101, 0b111, 0b111, 0b101, 0b101], // m
    [0b110, 0b101, 0b101, 0b101, 0b101], // n
    [0b010, 0b101, 0b101, 0b101, 0b010], // o
    [0b110, 0b101, 0b110, 0b100, 0b100], // p
    [0b010, 0b101, 0b101, 0b110, 0b011], // q
    [0b110, 0b101, 0b110, 0b101, 0b101], // r
    [0b011, 0b100, 0b010, 0b001, 0b110], // s
    [0b111, 0b010, 0b010, 0b010, 0b010], // t
    [0b101, 0b101, 0b101, 0b101, 0b111], // u
    [0b101, 0b101, 0b101, 0b101, 0b010], // v
    [0b101, 0b101, 0b111, 0b111, 0b101], // w
    [0b101, 0b101, 0b010, 0b101, 0b101], // x
    [0b101, 0b101, 0b010, 0b010, 0b010], // y
    [0b111, 0b001, 0b010, 0b100, 0b111], // z
];

fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
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
    fn every_menu_on_the_sheet_lays_out_and_labels_itself() {
        for (name, items) in MENUS {
            let layout = MenuLayout::new(&items);
            assert_eq!(layout.len(), items.len(), "{name} should lay out");
            let (width, _) = layout.size();
            for index in 0..items.len() {
                let tab = layout.label_tab(index).expect("every item has a label");
                assert!(
                    tab.x >= 0 && tab.x + tab.width as i32 <= width as i32,
                    "{name} label {index} hangs off the strip"
                );
                let cell = layout.cell(index).expect("every item has a cell");
                assert_eq!(
                    layout.hit_test(
                        cell.x as f32 + cell.width as f32 / 2.0,
                        cell.y as f32 + cell.height as f32 / 2.0,
                    ),
                    Some(index),
                    "{name} cell {index} should be hittable"
                );
            }
        }
    }

    #[test]
    fn the_sheet_is_deterministic_and_the_scenes_fit_their_cells() {
        // The busy background's speckle must not drift between runs.
        let mut first = Sheet::new(64, 64);
        Background::Busy.decorate(&mut first, 0, 0, 64, 64);
        let mut second = Sheet::new(64, 64);
        Background::Busy.decorate(&mut second, 0, 0, 64, 64);
        assert_eq!(first.pixels, second.pixels);

        // A four cell strip and its widest label both fit the menu scene.
        let items = MENUS[2].1;
        let layout = MenuLayout::new(&items);
        assert!(
            layout.size().0 <= MENU_SCENE.0,
            "the strip must fit its scene"
        );
        let head_top = MENU_STRIP_HEIGHT + LABEL_TAB_GAP + LABEL_TAB_HEIGHT + 5;
        assert!(head_top < MENU_SCENE.1, "the head must fit under the strip");
        // And a bubble cell fits above a head in a bubble scene.
        assert!(BUBBLE_CELL.0 <= BUBBLE_SCENE.0 && BUBBLE_CELL.1 < BUBBLE_SCENE.1);
        // A menu and the strip beside it both fit a moments scene, with the creature a third in.
        for (strip, hovered) in MOMENT_STRIPS {
            let menu = MenuLayout::new(&MENUS[1].1);
            let side = MenuLayout::new(strip);
            assert!(hovered < side.len());
            let right = MOMENT_SCENE.0 as i32 / 3 - menu.notch_x()
                + menu.size().0 as i32
                + SIDE_GAP
                + side.size().0 as i32;
            assert!(
                right <= MOMENT_SCENE.0 as i32,
                "the strip must fit its scene"
            );
        }
        assert!(
            GUTTER + MOMENT_STRIPS.len() as u32 * MOMENT_SCENE.0 * SCALE <= SHEET_WIDTH,
            "the moments scenes fit the sheet"
        );
    }
}
