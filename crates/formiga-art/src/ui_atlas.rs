//! On-desktop UI art: icon thought bubbles and the right-click creature menu.
//!
//! Everything here is one deterministic RGBA atlas, rendered once by
//! [`UiAtlasRenderer::render`] and then sampled by rect. The UI never takes a creature palette:
//! a bubble over a mint creature and a bubble over a rose one are the same pixels, so the
//! overlay can draw every bubble on the screen from a single texture with one nearest-filtered
//! quad each.
//!
//! # Anchoring a bubble
//!
//! Every bubble sprite shares [`BUBBLE_CELL`] (17x16 art px), so the GPU side anchors all of
//! them identically and a growth step never shifts the bubble sideways. [`BUBBLE_ANCHOR`] names
//! the cell pixel that lands on the art pixel directly above the crown of the creature's head:
//! the bottom-centre pixel of the cell, one row under the tail's tip. Draw the cell at
//! `head_top - BUBBLE_ANCHOR` (in art px, before the 2x/3x/4x desktop scale) and the tail points
//! down at the head from a 1 px gap. The tail sits on the cell's exact centre column, so a
//! mirrored (UV-flipped) bubble lands on the same pixels.
//!
//! # Anchoring the menu
//!
//! [`MenuLayout`] is pure arithmetic in art pixels with its origin at the strip's top-left.
//! The frame's pointer notch hangs below the strip body; place the strip so the notch tip sits a
//! little above the creature's head, with `MenuLayout::notch_x` over the creature's centre.
//! A label tab hangs under the notch, centred on the hovered cell.
//!
//! # Style
//!
//! Dark plum outline, soft cream paper, flat shading, whole pixels only. Accents are chosen so
//! every icon is recognisable by SHAPE first: the colour is confirmation, never the message.

use crate::{Canvas, Rgba};
use formiga_core::{BubbleGrowth, BubbleIcon};

// ---------------------------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------------------------

/// The shared dark plum, as in `bubble.rs`.
const OUTLINE: Rgba = Rgba::new(66, 53, 72, 255);
/// Soft cream paper. Slightly translucent so a bubble sits *on* the desktop rather than punching
/// a hole in it, exactly as the milestone bubble does.
const PAPER: Rgba = Rgba::new(255, 246, 224, 244);
/// The menu is a control rather than a mood, so its paper is a touch more solid.
const PAPER_SOLID: Rgba = Rgba::new(255, 246, 224, 252);
/// A quiet warm grey for inner shading and cell separators.
const SHADE: Rgba = Rgba::new(226, 211, 186, 244);
/// Hovered cell fill. Clearly darker than the paper as well as warmer, so the highlight survives
/// a colour-blind viewer or a greyscale screenshot; the cell also gains a full outline ring.
const HOVER: Rgba = Rgba::new(244, 206, 140, 255);

const ROSE: Rgba = Rgba::new(214, 92, 108, 255);
const ROSE_DEEP: Rgba = Rgba::new(168, 62, 82, 255);
const BERRY: Rgba = Rgba::new(216, 106, 82, 255);
const BERRY_DEEP: Rgba = Rgba::new(166, 72, 56, 255);
const LEAF: Rgba = Rgba::new(104, 156, 96, 255);
const TEAL: Rgba = Rgba::new(86, 170, 174, 255);
const TEAL_DEEP: Rgba = Rgba::new(54, 122, 132, 255);
const TEAL_LIGHT: Rgba = Rgba::new(174, 226, 222, 255);
const AMBER: Rgba = Rgba::new(236, 172, 68, 255);
const AMBER_DEEP: Rgba = Rgba::new(190, 128, 44, 255);
const PERIWINKLE: Rgba = Rgba::new(134, 148, 196, 255);
const PERIWINKLE_DEEP: Rgba = Rgba::new(98, 112, 160, 255);
const VIOLET: Rgba = Rgba::new(146, 120, 188, 255);
const VIOLET_DEEP: Rgba = Rgba::new(106, 84, 148, 255);
const PLUM: Rgba = Rgba::new(170, 118, 164, 255);
const PLUM_DEEP: Rgba = Rgba::new(128, 84, 126, 255);
const MUTED: Rgba = Rgba::new(150, 138, 156, 255);
const SLATE: Rgba = Rgba::new(132, 142, 164, 255);
const SLATE_DEEP: Rgba = Rgba::new(96, 106, 130, 255);
const DOVE: Rgba = Rgba::new(138, 128, 150, 255);
const TAN: Rgba = Rgba::new(226, 172, 118, 255);
const TAN_DEEP: Rgba = Rgba::new(178, 128, 82, 255);
const GOLD: Rgba = Rgba::new(246, 202, 86, 255);
const PINE: Rgba = Rgba::new(92, 150, 126, 255);
const PINE_DEEP: Rgba = Rgba::new(62, 112, 96, 255);
const SEA: Rgba = Rgba::new(76, 158, 140, 255);
const SEA_DEEP: Rgba = Rgba::new(50, 118, 106, 255);
const STORM: Rgba = Rgba::new(118, 124, 146, 255);
const STORM_DEEP: Rgba = Rgba::new(86, 92, 114, 255);
const ENVY: Rgba = Rgba::new(122, 176, 92, 255);
const ENVY_DEEP: Rgba = Rgba::new(84, 132, 64, 255);
const BLUSH: Rgba = Rgba::new(232, 120, 142, 255);

// ---------------------------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------------------------

/// Every bubble sprite, at every growth step, is this many art pixels.
pub const BUBBLE_CELL: (u32, u32) = (17, 16);
/// The cell pixel that sits on the art pixel just above the top-centre of the creature's head.
/// It is the bottom-centre pixel of the cell: one row below the tail tip, on the mirror axis.
pub const BUBBLE_ANCHOR: (u32, u32) = (8, 15);

/// One menu cell, icon plus its padding.
pub const MENU_CELL: u32 = 14;
/// Paper plus outline between a cell and the strip edge.
const MENU_BORDER: u32 = 2;
/// Paper between two neighbouring cells.
const MENU_GAP: u32 = 2;
/// The strip without its pointer notch.
pub const MENU_BODY_HEIGHT: u32 = MENU_BORDER * 2 + MENU_CELL;
/// The notch hanging under the strip body.
pub const MENU_NOTCH_HEIGHT: u32 = 3;
/// Body plus notch: the full height of a `menu_frame` sprite.
pub const MENU_STRIP_HEIGHT: u32 = MENU_BODY_HEIGHT + MENU_NOTCH_HEIGHT;
/// A label tab, paper and outline included.
pub const LABEL_TAB_HEIGHT: u32 = 9;
/// Breathing room between the notch tip and the label tab under it.
pub const LABEL_TAB_GAP: u32 = 1;
/// Art pixels between neighbouring tabs when every item's label shows at once.
pub const LABEL_ROW_GAP: u32 = 2;

/// Fewest items a menu may hold.
pub const MENU_MIN_ITEMS: usize = 2;
/// Most items a menu may hold.
pub const MENU_MAX_ITEMS: usize = 4;

const GLYPH_WIDTH: u32 = 3;
const GLYPH_HEIGHT: u32 = 5;
/// One blank column between letters.
const GLYPH_ADVANCE: u32 = GLYPH_WIDTH + 1;

/// The atlas. Small enough to live beside the creature atlas without a second bind group.
pub const UI_ATLAS_WIDTH: u32 = 256;
/// See [`UI_ATLAS_WIDTH`].
pub const UI_ATLAS_HEIGHT: u32 = 120;

/// As many bubbles as fit across the atlas, in the two rows above the menu.
const BUBBLE_COLUMNS: u32 = UI_ATLAS_WIDTH / BUBBLE_CELL.0;
/// Two shared no-icon steps (`Small`, `Medium`) come first, then one `Full` cell per icon.
const SHARED_BUBBLE_SLOTS: u32 = 2;
/// Menu cells, normal and hovered side by side, fill two rows of cells across the atlas.
const MENU_ICON_ROW_Y: u32 = 32;
const MENU_ICON_ROWS: u32 = 2;
const MENU_CELLS_PER_ROW: u32 = UI_ATLAS_WIDTH / MENU_CELL;
/// Strips that point at a creature, then strips that sit beside one and point at nothing.
const MENU_FRAME_ROW_Y: u32 = MENU_ICON_ROW_Y + MENU_ICON_ROWS * MENU_CELL;
const MENU_PLAIN_ROW_Y: u32 = MENU_FRAME_ROW_Y + MENU_STRIP_HEIGHT;
/// Label tabs, packed along as many rows as they need.
const MENU_LABEL_ROW_Y: u32 = MENU_PLAIN_ROW_Y + MENU_BODY_HEIGHT;

// ---------------------------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------------------------

/// A sprite's place in the atlas, in atlas pixels, origin top-left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpriteRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl SpriteRect {
    /// True when the two rects share at least one atlas pixel.
    pub fn overlaps(&self, other: &Self) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

/// What a cell of the right-click menu offers.
///
/// A colony creature is offered `Snack, Toy, Home, Profile`, with `Moment` in place of `Home`
/// while the houses are out and there is something the village could share. A visitor is offered
/// `Snack, Toy, Stay, Profile` while it may still be invited to stay, and
/// `Snack, Toy, CopyCode, Profile` once it cannot. `Moment` opens a second strip beside the first,
/// holding the moments on offer — `Picnic`, `Dance`, `Nap` — and `Stop` while one is under way.
/// A house, while Formiga Home is installed, offers `Inside, Profile`, with `Stop` in place of
/// `Inside` while it is the house open. A wonder put out offers `Play, Move, PutAway, Profile`.
/// During a visit home to a village that stays out, `Out` sends everyone back out: in `Home`'s
/// cell, or at the end of the strip of moments when there are some.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuIcon {
    Snack,
    Toy,
    Home,
    Profile,
    Stay,
    CopyCode,
    Moment,
    Picnic,
    Dance,
    Nap,
    Stop,
    Inside,
    Play,
    Move,
    PutAway,
    Out,
}

impl MenuIcon {
    pub const ALL: [Self; 16] = [
        Self::Snack,
        Self::Toy,
        Self::Home,
        Self::Profile,
        Self::Stay,
        Self::CopyCode,
        Self::Moment,
        Self::Picnic,
        Self::Dance,
        Self::Nap,
        Self::Stop,
        Self::Inside,
        Self::Play,
        Self::Move,
        Self::PutAway,
        Self::Out,
    ];

    fn index(self) -> u32 {
        Self::ALL
            .iter()
            .position(|candidate| *candidate == self)
            .expect("every MenuIcon is in ALL") as u32
    }
}

/// The tab hung under a cell whose item was chosen but cannot be done just now: the companion has
/// started a climb, set off on a journey, or is otherwise somewhere an offer cannot reach it. A
/// companion that simply turns an offer down answers in its own bubble instead.
pub const MENU_UNAVAILABLE_TEXT: &str = "not now";

/// The label under a hovered cell. Always lowercase, always a word the owner would say.
pub fn menu_label_text(icon: MenuIcon) -> &'static str {
    match icon {
        MenuIcon::Snack => "snack",
        MenuIcon::Toy => "toy",
        MenuIcon::Home => "home",
        MenuIcon::Profile => "profile",
        MenuIcon::Stay => "stay",
        MenuIcon::CopyCode => "copy code",
        MenuIcon::Moment => "moment",
        MenuIcon::Picnic => "picnic",
        MenuIcon::Dance => "dance",
        MenuIcon::Nap => "nap",
        MenuIcon::Stop => "stop",
        MenuIcon::Inside => "look inside",
        MenuIcon::Play => "play",
        MenuIcon::Move => "move",
        MenuIcon::PutAway => "put away",
        MenuIcon::Out => "out",
    }
}

/// A rect in ART pixels, with the origin at the menu strip's top-left. `y` may exceed the strip
/// because the label tab hangs below it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl MenuRect {
    /// Half-open on the right and bottom, so neighbouring rects never both claim a pixel.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x as f32
            && y >= self.y as f32
            && x < (self.x + self.width as i32) as f32
            && y < (self.y + self.height as i32) as f32
    }
}

/// Where a menu's frame, cells, notch and label tab sit, in art pixels.
///
/// Pure arithmetic: no atlas, no canvas, no allocation. `MenuLayout::new` accepts 2 to 4 items;
/// anything else yields an empty layout whose `size` is `(0, 0)` and whose `hit_test` is always
/// `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuLayout {
    items: [MenuIcon; MENU_MAX_ITEMS],
    count: usize,
    size: (u32, u32),
    notch_x: i32,
}

impl MenuLayout {
    pub fn new(items: &[MenuIcon]) -> Self {
        let empty = Self {
            items: [MenuIcon::Snack; MENU_MAX_ITEMS],
            count: 0,
            size: (0, 0),
            notch_x: 0,
        };
        if items.len() < MENU_MIN_ITEMS || items.len() > MENU_MAX_ITEMS {
            return empty;
        }
        let mut stored = [MenuIcon::Snack; MENU_MAX_ITEMS];
        stored[..items.len()].copy_from_slice(items);
        let width = menu_frame_width(items.len() as u32);
        Self {
            items: stored,
            count: items.len(),
            size: (width, MENU_STRIP_HEIGHT),
            notch_x: (width / 2) as i32,
        }
    }

    /// The strip's overall size in art pixels, notch included. `(0, 0)` when empty.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// How many cells this layout holds.
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The item at `index`, if there is one.
    pub fn item(&self, index: usize) -> Option<MenuIcon> {
        (index < self.count).then(|| self.items[index])
    }

    /// The `MENU_CELL`-square rect an icon sprite is drawn into.
    pub fn cell(&self, index: usize) -> Option<MenuRect> {
        (index < self.count).then(|| MenuRect {
            x: (MENU_BORDER + index as u32 * (MENU_CELL + MENU_GAP)) as i32,
            y: MENU_BORDER as i32,
            width: MENU_CELL,
            height: MENU_CELL,
        })
    }

    /// The column the pointer notch is centred on. Put this over the creature's centre.
    pub fn notch_x(&self) -> i32 {
        self.notch_x
    }

    /// Where the label tab for `index` goes: under the notch, centred on that cell, and nudged
    /// so it never hangs off the end of the strip while it still fits within it.
    pub fn label_tab(&self, index: usize) -> Option<MenuRect> {
        self.tab_under(
            index,
            UiAtlasRenderer::menu_label(*self.items.get(index)?).width,
        )
    }

    /// Every item's label tab at once, in one row where the single tab hangs: in the strip's
    /// order, [`LABEL_ROW_GAP`] apart, and the row centred under the strip. Four labels run wider
    /// than their strip, so the row hangs past both its ends; whoever places it keeps it on the
    /// display. Slots past the last item are `None`.
    pub fn label_row(&self) -> [Option<MenuRect>; MENU_MAX_ITEMS] {
        let mut row = [None; MENU_MAX_ITEMS];
        let width = |index: usize| UiAtlasRenderer::menu_label(self.items[index]).width;
        let total = (0..self.count).map(width).sum::<u32>()
            + self.count.saturating_sub(1) as u32 * LABEL_ROW_GAP;
        let mut x = (self.size.0 as i32 - total as i32).div_euclid(2);
        for (index, slot) in row.iter_mut().enumerate().take(self.count) {
            let width = width(index);
            *slot = Some(MenuRect {
                x,
                y: (self.size.1 + LABEL_TAB_GAP) as i32,
                width,
                height: LABEL_TAB_HEIGHT,
            });
            x += (width + LABEL_ROW_GAP) as i32;
        }
        row
    }

    /// Where the "not now" tab hangs under a cell (see [`MENU_UNAVAILABLE_TEXT`]): where its
    /// label would, in its place. Draw [`UiAtlasRenderer::menu_unavailable_label`] into it.
    pub fn unavailable_tab(&self, index: usize) -> Option<MenuRect> {
        self.tab_under(index, UiAtlasRenderer::menu_unavailable_label().width)
    }

    /// A tab `width` art pixels wide, centred under a cell and kept within the strip.
    fn tab_under(&self, index: usize, width: u32) -> Option<MenuRect> {
        let cell = self.cell(index)?;
        let centre = cell.x + cell.width as i32 / 2;
        let limit = self.size.0 as i32 - width as i32;
        let x = if limit >= 0 {
            (centre - width as i32 / 2).clamp(0, limit)
        } else {
            // A tab wider than its own strip is centred on the strip instead.
            limit / 2
        };
        Some(MenuRect {
            x,
            y: (self.size.1 + LABEL_TAB_GAP) as i32,
            width,
            height: LABEL_TAB_HEIGHT,
        })
    }

    /// Which item is under a point, in art pixels relative to the strip's top-left. The frame
    /// border, the gaps between cells and the notch all return `None`.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<usize> {
        (0..self.count).find(|index| self.cell(*index).is_some_and(|cell| cell.contains(x, y)))
    }
}

fn menu_frame_width(cells: u32) -> u32 {
    MENU_BORDER * 2 + cells * MENU_CELL + cells.saturating_sub(1) * MENU_GAP
}

// ---------------------------------------------------------------------------------------------
// The renderer
// ---------------------------------------------------------------------------------------------

/// Stateless: `render` once at startup, then look sprites up by rect. Every function here is
/// deterministic and takes no seed, because the UI is the same for every creature.
pub struct UiAtlasRenderer;

impl UiAtlasRenderer {
    pub fn render() -> Canvas {
        let mut canvas = Canvas::new(UI_ATLAS_WIDTH, UI_ATLAS_HEIGHT);

        // Bubbles: the two shared no-icon steps, then one full bubble per icon.
        let small = bubble_slot(0);
        draw_bubble_frame(
            &mut canvas,
            small.x as i32,
            small.y as i32,
            BubbleGrowth::Small,
        );
        let medium = bubble_slot(1);
        draw_bubble_frame(
            &mut canvas,
            medium.x as i32,
            medium.y as i32,
            BubbleGrowth::Medium,
        );
        for (index, icon) in BubbleIcon::ALL.into_iter().enumerate() {
            let slot = bubble_slot(SHARED_BUBBLE_SLOTS + index as u32);
            let (x, y) = (slot.x as i32, slot.y as i32);
            draw_bubble_frame(&mut canvas, x, y, BubbleGrowth::Full);
            // The 9x9 icon box sits in the middle of the bubble's paper.
            draw_bubble_icon(&mut canvas, x + 4, y + 2, icon);
        }

        // Menu icon cells, normal then hovered.
        for icon in MenuIcon::ALL {
            for hovered in [false, true] {
                let rect = Self::menu_icon(icon, hovered);
                draw_menu_cell(&mut canvas, rect.x as i32, rect.y as i32, icon, hovered);
            }
        }

        // Menu frames: pointing at a creature, and plain for a strip beside another.
        for cells in MENU_MIN_ITEMS..=MENU_MAX_ITEMS {
            let rect = Self::menu_frame(cells as u8).expect("2..=4 cells have a frame");
            draw_menu_frame(&mut canvas, rect.x as i32, rect.y as i32, cells as u32);
            let plain = Self::menu_frame_plain(cells as u8).expect("2..=4 cells have a frame");
            draw_menu_body(&mut canvas, plain.x as i32, plain.y as i32, cells as u32);
        }

        // Label tabs, and the one that says an item cannot be done just now.
        for icon in MenuIcon::ALL {
            let rect = Self::menu_label(icon);
            draw_label_tab(
                &mut canvas,
                rect.x as i32,
                rect.y as i32,
                menu_label_text(icon),
            );
        }
        let rect = Self::menu_unavailable_label();
        draw_label_tab(
            &mut canvas,
            rect.x as i32,
            rect.y as i32,
            MENU_UNAVAILABLE_TEXT,
        );

        canvas
    }

    /// The bubble sprite for an icon at a growth step. `Small` and `Medium` carry no icon, so
    /// every icon shares those two cells; callers need not know that.
    pub fn bubble(icon: BubbleIcon, growth: BubbleGrowth) -> SpriteRect {
        let slot = match growth {
            BubbleGrowth::Small => 0,
            BubbleGrowth::Medium => 1,
            BubbleGrowth::Full => SHARED_BUBBLE_SLOTS + bubble_icon_index(icon),
        };
        bubble_slot(slot)
    }

    /// The pre-rendered frame for a strip of `cells` items, notch included. `None` outside 2..=4.
    pub fn menu_frame(cells: u8) -> Option<SpriteRect> {
        let cells = u32::from(cells);
        if cells < MENU_MIN_ITEMS as u32 || cells > MENU_MAX_ITEMS as u32 {
            return None;
        }
        let x: u32 = (MENU_MIN_ITEMS as u32..cells).map(menu_frame_width).sum();
        Some(SpriteRect {
            x,
            y: MENU_FRAME_ROW_Y,
            width: menu_frame_width(cells),
            height: MENU_STRIP_HEIGHT,
        })
    }

    /// The frame for a strip of `cells` items with no pointer notch: the one drawn beside a menu
    /// rather than over a creature, exactly the height of a strip's body. `None` outside 2..=4.
    pub fn menu_frame_plain(cells: u8) -> Option<SpriteRect> {
        let frame = Self::menu_frame(cells)?;
        Some(SpriteRect {
            y: MENU_PLAIN_ROW_Y,
            height: MENU_BODY_HEIGHT,
            ..frame
        })
    }

    /// One `MENU_CELL`-square cell: the icon, plus the hover highlight when `hovered`.
    /// Draw it into the matching `MenuLayout::cell` rect.
    pub fn menu_icon(icon: MenuIcon, hovered: bool) -> SpriteRect {
        let slot = icon.index() * 2 + u32::from(hovered);
        SpriteRect {
            x: slot % MENU_CELLS_PER_ROW * MENU_CELL,
            y: MENU_ICON_ROW_Y + slot / MENU_CELLS_PER_ROW * MENU_CELL,
            width: MENU_CELL,
            height: MENU_CELL,
        }
    }

    /// The paper tab carrying an item's label. Draw it into `MenuLayout::label_tab`. Tabs are
    /// laid along a row until the next would not fit, then along the next row down.
    pub fn menu_label(icon: MenuIcon) -> SpriteRect {
        label_slot(icon.index() as usize)
    }

    /// The paper tab saying "not now", packed after every item's label.
    pub fn menu_unavailable_label() -> SpriteRect {
        label_slot(MenuIcon::ALL.len())
    }
}

fn bubble_icon_index(icon: BubbleIcon) -> u32 {
    BubbleIcon::ALL
        .iter()
        .position(|candidate| *candidate == icon)
        .expect("every BubbleIcon is in ALL") as u32
}

fn bubble_slot(slot: u32) -> SpriteRect {
    SpriteRect {
        x: (slot % BUBBLE_COLUMNS) * BUBBLE_CELL.0,
        y: (slot / BUBBLE_COLUMNS) * BUBBLE_CELL.1,
        width: BUBBLE_CELL.0,
        height: BUBBLE_CELL.1,
    }
}

fn text_width(text: &str) -> u32 {
    let letters = text.chars().count() as u32;
    if letters == 0 {
        0
    } else {
        letters * GLYPH_ADVANCE - 1
    }
}

/// The `index`th tab along the label rows: each item's label in [`MenuIcon::ALL`] order, then
/// [`MENU_UNAVAILABLE_TEXT`].
fn label_slot(index: usize) -> SpriteRect {
    let texts = MenuIcon::ALL
        .iter()
        .map(|icon| menu_label_text(*icon))
        .chain([MENU_UNAVAILABLE_TEXT]);
    let (mut x, mut y) = (0, MENU_LABEL_ROW_Y);
    for (slot, text) in texts.enumerate() {
        let width = label_tab_width(text);
        if x + width > UI_ATLAS_WIDTH {
            x = 0;
            y += LABEL_TAB_HEIGHT;
        }
        if slot == index {
            return SpriteRect {
                x,
                y,
                width,
                height: LABEL_TAB_HEIGHT,
            };
        }
        x += width;
    }
    unreachable!("a label slot past the last tab")
}

fn label_tab_width(text: &str) -> u32 {
    // One outline column and one paper column on each side of the text.
    text_width(text) + 4
}

// ---------------------------------------------------------------------------------------------
// Contact shadow
// ---------------------------------------------------------------------------------------------

/// The widest contact shadow, in art pixels: a body frame across.
pub const SHADOW_MAX_WIDTH: u32 = crate::FRAME_SIZE;
/// The contact shadows' own sheet: every width a shadow can be, one under the other, the
/// desktop's two-row shadows first and then the village ground's three-row ones. It is a
/// texture of its own rather than cells of [`UiAtlasRenderer::render`], because a shadow is
/// drawn under every companion all the time and the UI atlas is only held while a bubble or a
/// menu is up.
pub const SHADOW_SHEET_WIDTH: u32 = SHADOW_MAX_WIDTH;
pub const SHADOW_SHEET_HEIGHT: u32 = SHADOW_MAX_WIDTH * (2 + 3);
/// How high above the ground, in art pixels, a companion's shadow has shrunk to nothing.
pub const SHADOW_FADE_HEIGHT: f32 = 48.0;
/// How far its feet can come up off the ground, in art pixels, before its shadow starts to
/// shrink: a stride's bob is not a hop.
const SHADOW_STILL_LIFT: f32 = 1.0;
/// Every shadow on the desktop: dark, at about a third, so it darkens a light wallpaper and
/// still reads on a dark one without ever hiding what is under it.
const SHADOW_INK: Rgba = Rgba::new(20, 24, 26, 90);
/// The shading the village ground stands its houses, trees and belongings on, in the ink and
/// strength every dwelling's own ground shadow is drawn in.
const GROUND_SHADE: Rgba = Rgba::new(20, 24, 26, 95);

/// What a shadow is laid on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadowGround {
    /// A window's ledge or the habitat floor, seen edge on: two rows.
    Desktop,
    /// The village's grass, seen a little from above as its houses' own shadows are: three rows,
    /// the outer two drawn in.
    Village,
}

impl ShadowGround {
    /// How many rows tall the shadow is.
    pub const fn rows(self) -> u32 {
        match self {
            Self::Desktop => 2,
            Self::Village => 3,
        }
    }

    /// How many of those rows lie above the line its feet stand on. On the desktop, the top
    /// one, behind the feet's own bottom row, so the shadow peeks out round them rather than
    /// hanging under them. On the village's grass, all three: the front of the grass is the
    /// ground line, so a shadow there lies back across the grass behind the feet, as the houses'
    /// own do, and never hangs off its front edge.
    pub const fn above_contact(self) -> u32 {
        match self {
            Self::Desktop => 1,
            Self::Village => 3,
        }
    }

    fn first_row(self) -> u32 {
        match self {
            Self::Desktop => 0,
            Self::Village => SHADOW_MAX_WIDTH * Self::Desktop.rows(),
        }
    }
}

/// A companion's shadow on whatever it stands on, across the frame it is drawn from: where its
/// middle falls, in art pixels from the frame's left edge, and how wide it is. Measured from the
/// frame's own silhouette alone, never from what is on the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactShadow {
    pub centre: f32,
    pub width: u32,
}

impl ContactShadow {
    /// The feet of a frame: the first and just-past-the-last columns its bottom two drawn rows
    /// reach, or `None` for a frame that draws nothing.
    pub fn feet(frame: &Canvas) -> Option<(u8, u8)> {
        let (_, _, _, bottom) = frame.alpha_bounds()?;
        let rows = bottom.saturating_sub(1)..=bottom;
        let mut reach: Option<(u32, u32)> = None;
        for y in rows {
            for x in 0..frame.width() {
                if frame.get(x as i32, y as i32).a > 0 {
                    reach = Some(match reach {
                        Some((left, right)) => (left.min(x), right.max(x + 1)),
                        None => (x, x + 1),
                    });
                }
            }
        }
        reach.map(|(left, right)| (left as u8, right as u8))
    }

    /// The shadow under feet reaching across `feet`, with the feet `lift` art pixels off the
    /// ground: as wide as they are while they are on it, shrinking evenly from both ends the
    /// higher they go, and gone by [`SHADOW_FADE_HEIGHT`].
    pub fn under((left, right): (u8, u8), lift: f32) -> Option<Self> {
        let full = u32::from(right.saturating_sub(left)).min(SHADOW_MAX_WIDTH);
        let share = if lift <= SHADOW_STILL_LIFT {
            1.0
        } else {
            (1.0 - (lift - SHADOW_STILL_LIFT) / SHADOW_FADE_HEIGHT).max(0.0)
        };
        // Two pixels at a time, one off each end, so it stays centred on whole pixels.
        let trimmed = ((full as f32 * (1.0 - share)) / 2.0).round() as u32 * 2;
        let width = full.saturating_sub(trimmed);
        (width >= 2).then(|| Self {
            centre: (f32::from(left) + f32::from(right)) / 2.0,
            width,
        })
    }

    /// The same shadow under the frame drawn the other way round.
    pub fn mirrored(self) -> Self {
        Self {
            centre: crate::FRAME_SIZE as f32 - self.centre,
            ..self
        }
    }

    /// Its left edge, in art pixels from the frame's left edge.
    pub fn left(self) -> f32 {
        self.centre - self.width as f32 / 2.0
    }

    /// Where a shadow this wide lies on [`ContactShadow::sheet`].
    pub fn cell(self, ground: ShadowGround) -> SpriteRect {
        let width = self.width.clamp(1, SHADOW_MAX_WIDTH);
        SpriteRect {
            x: 0,
            y: ground.first_row() + (width - 1) * ground.rows(),
            width,
            height: ground.rows(),
        }
    }

    /// Every shadow, at every width, on the desktop and on the village ground.
    pub fn sheet() -> Canvas {
        let mut canvas = Canvas::new(SHADOW_SHEET_WIDTH, SHADOW_SHEET_HEIGHT);
        for ground in [ShadowGround::Desktop, ShadowGround::Village] {
            for width in 1..=SHADOW_MAX_WIDTH {
                let cell = Self { centre: 0.0, width }.cell(ground);
                draw_shadow(&mut canvas, cell, ground);
            }
        }
        canvas
    }
}

/// One shadow filling `cell`: a flat dark band with its ends half as dark, so it reads as soft
/// and round at two pixels tall. On the village ground the rows above and below the middle one
/// are drawn in, as an ellipse seen from a little above.
fn draw_shadow(canvas: &mut Canvas, cell: SpriteRect, ground: ShadowGround) {
    let ink = match ground {
        ShadowGround::Desktop => SHADOW_INK,
        ShadowGround::Village => GROUND_SHADE,
    };
    let soft = Rgba::new(ink.r, ink.g, ink.b, ink.a / 2);
    let width = cell.width as i32;
    for row in 0..cell.height as i32 {
        let inset = match ground {
            ShadowGround::Village if row != 1 => (width as f32 * 0.13).round().max(1.0) as i32,
            _ => 0,
        };
        let (from, to) = (inset, width - inset);
        for x in from..to {
            let end = x == from || x == to - 1;
            canvas.set(
                cell.x as i32 + x,
                cell.y as i32 + row,
                if end { soft } else { ink },
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

/// Paint a small piece of pixel art written out as rows of characters. `'.'` leaves the pixel
/// alone; every other character is looked up in `ink`. Authoring icons this way keeps them
/// readable and reviewable in the source, which matters more here than speed: the atlas is
/// rendered once.
fn stamp(canvas: &mut Canvas, x: i32, y: i32, rows: &[&str], ink: &[(char, Rgba)]) {
    for (row, line) in rows.iter().enumerate() {
        for (column, mark) in line.chars().enumerate() {
            if mark == '.' {
                continue;
            }
            if let Some((_, color)) = ink.iter().find(|(key, _)| *key == mark) {
                canvas.set(x + column as i32, y + row as i32, *color);
            }
        }
    }
}

/// The bubble body and its downward tail. The tail always tapers to the same tip, so a bubble
/// grows upward out of one point above the head rather than sliding around.
fn draw_bubble_frame(canvas: &mut Canvas, x: i32, y: i32, growth: BubbleGrowth) {
    match growth {
        BubbleGrowth::Small => {
            canvas.fill_rect(x + 6, y + 9, 5, 4, OUTLINE);
            canvas.fill_rect(x + 7, y + 10, 3, 2, PAPER);
            // A one pixel wide tail, already reaching the shared tip.
            canvas.set(x + 8, y + 12, PAPER);
            canvas.set(x + 7, y + 13, OUTLINE);
            canvas.set(x + 9, y + 13, OUTLINE);
            canvas.set(x + 8, y + 13, PAPER);
            canvas.set(x + 8, y + 14, OUTLINE);
        }
        BubbleGrowth::Medium => {
            canvas.fill_rect(x + 4, y + 5, 9, 8, OUTLINE);
            canvas.fill_rect(x + 3, y + 6, 11, 6, OUTLINE);
            canvas.fill_rect(x + 5, y + 6, 7, 6, PAPER);
            canvas.fill_rect(x + 4, y + 7, 9, 4, PAPER);
            draw_bubble_tail(canvas, x, y);
        }
        BubbleGrowth::Full => {
            canvas.fill_rect(x + 1, y, 15, 13, OUTLINE);
            canvas.fill_rect(x, y + 1, 17, 11, OUTLINE);
            canvas.fill_rect(x + 2, y + 1, 13, 11, PAPER);
            canvas.fill_rect(x + 1, y + 2, 15, 9, PAPER);
            draw_bubble_tail(canvas, x, y);
        }
    }
}

/// The three-two-one taper shared by the medium and full bubbles, centred on the mirror axis.
fn draw_bubble_tail(canvas: &mut Canvas, x: i32, y: i32) {
    canvas.fill_rect(x + 7, y + 12, 3, 1, PAPER);
    canvas.set(x + 7, y + 13, OUTLINE);
    canvas.set(x + 9, y + 13, OUTLINE);
    canvas.set(x + 8, y + 13, PAPER);
    canvas.set(x + 8, y + 14, OUTLINE);
}

/// A 9x9 icon, drawn with its top-left at `x, y`. A few are eleven wide and start a column to
/// the left, so they stay centred on the bubble's mirror axis.
fn draw_bubble_icon(canvas: &mut Canvas, x: i32, y: i32, icon: BubbleIcon) {
    match icon {
        BubbleIcon::Heart => stamp(
            canvas,
            x,
            y,
            &[
                ".........",
                ".##...##.",
                "#########",
                "########+",
                "#######++",
                ".#####++.",
                "..####+..",
                "...##+...",
                "....#....",
            ],
            &[('#', ROSE), ('+', ROSE_DEEP)],
        ),
        BubbleIcon::Snack => stamp(
            canvas,
            x,
            y,
            &[
                ".....ggg.",
                "....ogg..",
                "...###...",
                "..#####..",
                ".#######.",
                ".######+.",
                ".#####++.",
                "..####+..",
                "...##+...",
            ],
            &[('#', BERRY), ('+', BERRY_DEEP), ('g', LEAF), ('o', OUTLINE)],
        ),
        // A spinning top, not a ball: the snack is already round, and at 2x two circles would
        // be the same icon in two colours.
        BubbleIcon::Toy => stamp(
            canvas,
            x,
            y,
            &[
                "....#....",
                "....#....",
                ".#######.",
                "#########",
                "#lllllll#",
                ".######+.",
                "..####+..",
                "...##+...",
                "....#....",
            ],
            &[('#', TEAL), ('+', TEAL_DEEP), ('l', TEAL_LIGHT)],
        ),
        BubbleIcon::Home => stamp(
            canvas,
            x,
            y,
            &[
                "....o....",
                "...o#o...",
                "..o###o..",
                ".o#####o.",
                "o#######o",
                ".opppppo.",
                ".opdddpo.",
                ".opdddpo.",
                ".ooooooo.",
            ],
            &[('#', LEAF), ('o', OUTLINE), ('p', PAPER), ('d', AMBER)],
        ),
        // A moon with one small star: a bare crescent at this size just reads as the letter c.
        BubbleIcon::Sleepy => stamp(
            canvas,
            x,
            y,
            &[
                "....###..",
                "...###...",
                "..###..+.",
                ".####.+++",
                ".####..+.",
                ".####....",
                "..###....",
                "...###...",
                "....###..",
            ],
            &[('#', PERIWINKLE), ('+', PERIWINKLE_DEEP)],
        ),
        BubbleIcon::Surprise => stamp(
            canvas,
            x,
            y,
            &[
                "...###...",
                "...###...",
                "...###...",
                "...###...",
                "...##+...",
                "....#....",
                ".........",
                "...###...",
                "...###...",
            ],
            &[('#', AMBER), ('+', AMBER_DEEP)],
        ),
        BubbleIcon::Question => stamp(
            canvas,
            x,
            y,
            &[
                "..####...",
                ".##..##..",
                ".....##..",
                "....##...",
                "...##....",
                "...##....",
                ".........",
                "...##....",
                "...##....",
            ],
            &[('#', VIOLET), ('+', VIOLET_DEEP)],
        ),
        BubbleIcon::Ellipsis => stamp(
            canvas,
            x,
            y,
            &[
                ".........",
                ".........",
                ".........",
                "##.##.##.",
                "##.##.##.",
                "++.++.++.",
                ".........",
                ".........",
                ".........",
            ],
            &[('#', MUTED), ('+', SLATE_DEEP)],
        ),
        // Small, thin and dove grey: a polite "no thanks", not a road sign.
        BubbleIcon::Decline => stamp(
            canvas,
            x,
            y,
            &[
                ".........",
                "..#####..",
                ".##...##.",
                ".#.....#.",
                ".#######.",
                ".#.....#.",
                ".##...##.",
                "..#####..",
                ".........",
            ],
            &[('#', DOVE), ('+', SLATE_DEEP)],
        ),
        BubbleIcon::Music => stamp(
            canvas,
            x,
            y,
            &[
                "....#####",
                "....##.##",
                "....##.##",
                "....####.",
                "....##...",
                ".#####...",
                "######...",
                "####+....",
                ".##+.....",
            ],
            &[('#', PINE), ('+', PINE_DEEP)],
        ),
        // Rounded corners and an open end, so the swirl reads as a curl rather than a maze.
        BubbleIcon::Dizzy => stamp(
            canvas,
            x,
            y,
            &[
                ".........",
                "..#####..",
                ".#.....#.",
                ".#.###.#.",
                ".#.#.#.#.",
                ".#.#.#.#.",
                ".#.#...#.",
                "..#.###..",
                ".........",
            ],
            &[('#', PLUM), ('+', PLUM_DEEP)],
        ),
        BubbleIcon::Hello => stamp(
            canvas,
            x,
            y,
            &[
                "##..##...",
                "##..##...",
                "######.#.",
                "######..#",
                "######..#",
                "#####..#.",
                ".####+...",
                "..###+...",
                ".........",
            ],
            &[('#', TAN), ('+', TAN_DEEP)],
        ),
        BubbleIcon::Sparkle => stamp(
            canvas,
            x,
            y,
            &[
                "....#..#.",
                "....#.###",
                "....#..#.",
                "..#####..",
                "#########",
                "..#####..",
                "....#....",
                "....#....",
                "....#....",
            ],
            &[('#', GOLD), ('+', AMBER_DEEP)],
        ),
        BubbleIcon::Stay => stamp(
            canvas,
            x,
            y,
            &[
                ".oo#####.",
                ".oo####..",
                ".oo###...",
                ".oo##....",
                ".oo......",
                ".oo......",
                ".oo......",
                ".oo......",
                "oooo.....",
            ],
            &[('#', SEA), ('+', SEA_DEEP), ('o', OUTLINE)],
        ),
        // A storm cloud with a bolt under it. Like the four after it, it is one shape with a
        // 1 px outline and one mark of its own, eleven wide: one column further out each side
        // than the 9x9 box, still two clear of the frame. Loose strokes and dithered cracks
        // blur into a smudge at 2x; a closed outline does not.
        BubbleIcon::Grumble => stamp(
            canvas,
            x - 1,
            y,
            &[
                "...ooo.....",
                "..oSSSoooo.",
                ".oSSSSSSSSo",
                "oSSSSSSSSSo",
                "oDDDDDDDDDo",
                ".oooAAoooo.",
                "....AA.....",
                "...AA......",
                "...A.......",
            ],
            &[
                ('o', OUTLINE),
                ('S', STORM),
                ('D', STORM_DEEP),
                ('A', AMBER),
            ],
        ),
        // A green heart broken clean in two.
        BubbleIcon::Jealous => stamp(
            canvas,
            x - 1,
            y,
            &[
                ".ooo...ooo.",
                "oGGGo.oGGGo",
                "oGGGGoGGGDo",
                "oGGGGoGGGDo",
                ".oGGoGGGDo.",
                "..oGGoGDo..",
                "...oGoDo...",
                "....ooo....",
                ".....o.....",
            ],
            &[('o', OUTLINE), ('G', ENVY), ('D', ENVY_DEEP)],
        ),
        // Seeing stars: one big five-pointed one, outlined, where Sparkle's are bare and
        // four-pointed.
        BubbleIcon::Swoon => stamp(
            canvas,
            x - 1,
            y,
            &[
                ".....o.....",
                "....oGo....",
                ".ooooGoooo.",
                "oGGGGGGGGGo",
                ".oGGGGGGGo.",
                "..oGGGGGo..",
                "..oGGoGGo..",
                ".oGGo.oGGo.",
                ".ooo...ooo.",
            ],
            &[('o', OUTLINE), ('G', GOLD)],
        ),
        // A shy little face, eyes squeezed shut over pink cheeks.
        BubbleIcon::Blush => stamp(
            canvas,
            x - 1,
            y,
            &[
                "..ooooooo..",
                ".o.......o.",
                "o..o...o..o",
                "o.o.o.o.o.o",
                "o.........o",
                "oBB.....BBo",
                "oBB.....BBo",
                ".o.......o.",
                "..ooooooo..",
            ],
            &[('o', OUTLINE), ('B', BLUSH)],
        ),
        // One wide-open eye with a big round iris.
        BubbleIcon::Watching => stamp(
            canvas,
            x - 1,
            y,
            &[
                "...........",
                "...ooooo...",
                ".oo.sss.oo.",
                "o..sssss..o",
                "o..ssOss..o",
                "o..sssss..o",
                ".oo.sss.oo.",
                "...ooooo...",
                "...........",
            ],
            &[('o', OUTLINE), ('s', SLATE_DEEP), ('O', OUTLINE)],
        ),
    }
}

/// One menu cell: the hover highlight when asked for, then the 12x12 icon.
fn draw_menu_cell(canvas: &mut Canvas, x: i32, y: i32, icon: MenuIcon, hovered: bool) {
    if hovered {
        canvas.fill_rect(x + 1, y, 12, 14, OUTLINE);
        canvas.fill_rect(x, y + 1, 14, 12, OUTLINE);
        canvas.fill_rect(x + 2, y + 1, 10, 12, HOVER);
        canvas.fill_rect(x + 1, y + 2, 12, 10, HOVER);
    }
    draw_menu_icon(canvas, x + 1, y + 1, icon);
}

/// A 12x12 icon, drawn with its top-left at `x, y`.
fn draw_menu_icon(canvas: &mut Canvas, x: i32, y: i32, icon: MenuIcon) {
    match icon {
        MenuIcon::Snack => stamp(
            canvas,
            x,
            y,
            &[
                "......ggg...",
                ".....ogg....",
                "...#####....",
                "..#######...",
                ".#########..",
                ".#########..",
                ".#########..",
                ".########+..",
                ".#######++..",
                "..#####++...",
                "...####+....",
                "............",
            ],
            &[('#', BERRY), ('+', BERRY_DEEP), ('g', LEAF), ('o', OUTLINE)],
        ),
        MenuIcon::Toy => stamp(
            canvas,
            x,
            y,
            &[
                ".....##.....",
                ".....##.....",
                "..########..",
                ".##########.",
                "############",
                "llllllllllll",
                "###########+",
                ".#########+.",
                "..#######+..",
                "...#####+...",
                "....###+....",
                ".....##.....",
            ],
            &[('#', TEAL), ('+', TEAL_DEEP), ('l', TEAL_LIGHT)],
        ),
        MenuIcon::Home => stamp(
            canvas,
            x,
            y,
            &[
                ".....o......",
                "....o#o.....",
                "...o###o....",
                "..o#####o...",
                ".o#######o..",
                "o#########o.",
                ".opppppppo..",
                ".opppppppo..",
                ".oppdddppo..",
                ".oppdddppo..",
                ".oppdddppo..",
                ".ooooooooo..",
            ],
            &[('#', LEAF), ('o', OUTLINE), ('p', PAPER), ('d', AMBER)],
        ),
        MenuIcon::Profile => stamp(
            canvas,
            x,
            y,
            &[
                "....#..#....",
                "....####....",
                "...######...",
                "...######...",
                "...######...",
                "....####....",
                ".....##.....",
                "...######...",
                "..########..",
                ".##########.",
                ".##########.",
                ".##########.",
            ],
            &[('#', VIOLET), ('+', VIOLET_DEEP)],
        ),
        MenuIcon::Stay => stamp(
            canvas,
            x,
            y,
            &[
                "..oo######..",
                "..oo#####...",
                "..oo####....",
                "..oo###.....",
                "..oo##......",
                "..oo........",
                "..oo........",
                "..oo........",
                "..oo........",
                "..oo........",
                "..oo........",
                ".oooo.......",
            ],
            &[('#', SEA), ('+', SEA_DEEP), ('o', OUTLINE)],
        ),
        MenuIcon::CopyCode => stamp(
            canvas,
            x,
            y,
            &[
                "............",
                ".ooooooo....",
                ".o+++++o....",
                ".o++ooooooo.",
                ".o++opppppo.",
                ".o++o####po.",
                ".o++opppppo.",
                ".o++o####po.",
                ".oooopppppo.",
                "....opppppo.",
                "....ooooooo.",
                "............",
            ],
            &[('o', OUTLINE), ('+', SHADE), ('p', PAPER), ('#', SLATE)],
        ),
        // Two companions side by side with a heart between them: something to share.
        MenuIcon::Moment => stamp(
            canvas,
            x,
            y,
            &[
                "....#.#.....",
                "...#####....",
                "....###.....",
                ".....#......",
                "............",
                ".aaa....bbb.",
                "aaaaa..bbbbb",
                "aoaoa..bobob",
                "aaaaa..bbbbb",
                "aaaa+..bbbb+",
                ".aa+....bb+.",
                "............",
            ],
            &[
                ('#', ROSE),
                ('a', PERIWINKLE),
                ('b', AMBER),
                ('+', SLATE_DEEP),
                ('o', OUTLINE),
            ],
        ),
        // A wicker basket with an apple peeking out under the handle.
        MenuIcon::Picnic => stamp(
            canvas,
            x,
            y,
            &[
                "............",
                "...oooooo...",
                "..o......o..",
                "..o.g....o..",
                "..orrr...o..",
                ".oooooooooo.",
                ".o#+#+#+#+o.",
                ".o+#+#+#+#o.",
                ".o#+#+#+#+o.",
                "..o+#+#+#o..",
                "..oooooooo..",
                "............",
            ],
            &[
                ('#', TAN),
                ('+', TAN_DEEP),
                ('o', OUTLINE),
                ('r', ROSE),
                ('g', LEAF),
            ],
        ),
        // Two notes on one beam.
        MenuIcon::Dance => stamp(
            canvas,
            x,
            y,
            &[
                "............",
                ".....######.",
                ".....######.",
                ".....#....#.",
                ".....#....#.",
                ".....#....#.",
                "...###..###.",
                "..####.####.",
                "..###+.###+.",
                "...++...++..",
                "............",
                "............",
            ],
            &[('#', VIOLET), ('+', VIOLET_DEEP)],
        ),
        // A crescent moon, and a small z.
        MenuIcon::Nap => stamp(
            canvas,
            x,
            y,
            &[
                "...####.zzz.",
                "..###.....z.",
                ".###.....z..",
                ".##.....zzz.",
                "###.........",
                "###.........",
                "###.........",
                ".###......#.",
                ".####....##.",
                "..#########.",
                "...######...",
                "............",
            ],
            &[('#', GOLD), ('z', PERIWINKLE_DEEP)],
        ),
        // A round sign with a bar across it: that's enough for now.
        MenuIcon::Stop => stamp(
            canvas,
            x,
            y,
            &[
                "............",
                "...######...",
                "..########..",
                ".##########.",
                ".##########.",
                ".#pppppppp#.",
                ".#pppppppp#.",
                ".#########+.",
                ".########++.",
                "..######++..",
                "...####++...",
                "............",
            ],
            &[('#', ROSE), ('+', ROSE_DEEP), ('p', PAPER)],
        ),
        // A door standing ajar, lamplight spilling out round it onto the step.
        MenuIcon::Play => stamp(
            canvas,
            x,
            y,
            &[
                "........g...",
                ".......ggg..",
                "........g...",
                "..........##",
                "........##++",
                "......##++..",
                "....##++....",
                "..##++......",
                "##++........",
                "+....pp.....",
                "....pppp....",
                "...pppppp...",
            ],
            &[('#', PLUM), ('+', OUTLINE), ('p', BERRY_DEEP), ('g', GOLD)],
        ),
        MenuIcon::Move => stamp(
            canvas,
            x,
            y,
            &[
                ".....##.....",
                "....####....",
                "...######...",
                "..#..##..#..",
                ".##..##..##.",
                "############",
                "++++++++++++",
                ".++..++..++.",
                "..+..++..+..",
                "...++++++...",
                "....++++....",
                ".....++.....",
            ],
            &[('#', PERIWINKLE), ('+', PERIWINKLE_DEEP)],
        ),
        MenuIcon::PutAway => stamp(
            canvas,
            x,
            y,
            &[
                ".....tt.....",
                ".....tt.....",
                "...tttttt...",
                "....tttt....",
                ".....tt.....",
                "o..........o",
                "oooooooooooo",
                "o##########o",
                "o#++++++++#o",
                "o##########o",
                "o##########o",
                "oooooooooooo",
            ],
            &[('o', OUTLINE), ('#', TAN), ('+', TAN_DEEP), ('t', TEAL)],
        ),
        MenuIcon::Out => stamp(
            canvas,
            x,
            y,
            &[
                "............",
                "..oooo......",
                "..o##o......",
                "..o##o...t..",
                "..o##o...tt.",
                "..o#+otttttt",
                "..o##o...tt.",
                "..o##o...t..",
                "..o##o......",
                "..o##o......",
                ".oooooo.....",
                "............",
            ],
            &[('o', OUTLINE), ('#', TAN_DEEP), ('+', GOLD), ('t', TEAL)],
        ),
        MenuIcon::Inside => stamp(
            canvas,
            x,
            y,
            &[
                "..oooooooo..",
                "..o##aaaao..",
                "..o##aaaao..",
                "..o##aaaao..",
                "..o##aaaao..",
                "..o#+aaaao..",
                "..o##aaaao..",
                "..o##aaaao..",
                "..o##aaaao..",
                "..o##aaaao..",
                ".oooooooooo.",
                "..ggggg.....",
            ],
            &[
                ('o', OUTLINE),
                ('#', TAN_DEEP),
                ('+', GOLD),
                ('a', AMBER),
                ('g', HOVER),
            ],
        ),
    }
}

/// The framed strip, its cell separators and the pointer notch under its centre.
fn draw_menu_frame(canvas: &mut Canvas, x: i32, y: i32, cells: u32) {
    draw_menu_body(canvas, x, y, cells);
    let width = menu_frame_width(cells) as i32;
    let body = MENU_BODY_HEIGHT as i32;

    // Notch: a five-three-one taper piercing the bottom edge.
    let notch = width / 2;
    canvas.fill_rect(x + notch - 2, y + body - 1, 5, 1, PAPER_SOLID);
    for (row, half) in [(body, 2_i32), (body + 1, 1)] {
        canvas.set(x + notch - half, y + row, OUTLINE);
        canvas.set(x + notch + half, y + row, OUTLINE);
        canvas.fill_rect(x + notch - half + 1, y + row, half * 2 - 1, 1, PAPER_SOLID);
    }
    canvas.set(x + notch, y + body + 2, OUTLINE);
}

/// The strip itself: an outlined paper tray with a hairline between each pair of cells, and no
/// notch, so it can stand beside another strip without pointing at anything.
fn draw_menu_body(canvas: &mut Canvas, x: i32, y: i32, cells: u32) {
    let width = menu_frame_width(cells) as i32;
    let body = MENU_BODY_HEIGHT as i32;
    canvas.fill_rect(x + 1, y, width - 2, body, OUTLINE);
    canvas.fill_rect(x, y + 1, width, body - 2, OUTLINE);
    canvas.fill_rect(x + 2, y + 1, width - 4, body - 2, PAPER_SOLID);
    canvas.fill_rect(x + 1, y + 2, width - 2, body - 4, PAPER_SOLID);

    // A hairline in each gap so four cells read as four, not as one long tray.
    for gap in 1..cells {
        let column = (MENU_BORDER + gap * (MENU_CELL + MENU_GAP) - MENU_GAP) as i32;
        canvas.fill_rect(x + column, y + 4, 1, MENU_CELL as i32 - 4, SHADE);
    }
}

/// A label on its own small paper tab.
fn draw_label_tab(canvas: &mut Canvas, x: i32, y: i32, text: &str) {
    let width = label_tab_width(text) as i32;
    let height = LABEL_TAB_HEIGHT as i32;
    canvas.fill_rect(x + 1, y, width - 2, height, OUTLINE);
    canvas.fill_rect(x, y + 1, width, height - 2, OUTLINE);
    canvas.fill_rect(x + 2, y + 1, width - 4, height - 2, PAPER_SOLID);
    canvas.fill_rect(x + 1, y + 2, width - 2, height - 4, PAPER_SOLID);
    draw_text(canvas, x + 2, y + 2, text, OUTLINE);
}

fn draw_text(canvas: &mut Canvas, x: i32, y: i32, text: &str, color: Rgba) {
    for (index, letter) in text.chars().enumerate() {
        let rows = glyph(letter);
        stamp(
            canvas,
            x + index as i32 * GLYPH_ADVANCE as i32,
            y,
            &rows,
            &[('#', color)],
        );
    }
}

/// A 3x5 pixel face, drawn in code so the art crate keeps no font dependency. The letterforms
/// are small caps: at three pixels wide a true lowercase bowl turns to mush, while these stay
/// crisp at 1 art px. The words themselves are always lowercase.
fn glyph(letter: char) -> [&'static str; GLYPH_HEIGHT as usize] {
    match letter {
        'a' => [".#.", "#.#", "###", "#.#", "#.#"],
        'c' => [".##", "#..", "#..", "#..", ".##"],
        'd' => ["##.", "#.#", "#.#", "#.#", "##."],
        'e' => ["###", "#..", "##.", "#..", "###"],
        'f' => ["###", "#..", "##.", "#..", "#.."],
        'h' => ["#.#", "#.#", "###", "#.#", "#.#"],
        'i' => ["###", ".#.", ".#.", ".#.", "###"],
        'k' => ["#.#", "#.#", "##.", "#.#", "#.#"],
        'l' => ["#..", "#..", "#..", "#..", "###"],
        'm' => ["#.#", "###", "###", "#.#", "#.#"],
        'n' => ["##.", "#.#", "#.#", "#.#", "#.#"],
        'o' => [".#.", "#.#", "#.#", "#.#", ".#."],
        'p' => ["##.", "#.#", "##.", "#..", "#.."],
        'r' => ["##.", "#.#", "##.", "#.#", "#.#"],
        's' => [".##", "#..", ".#.", "..#", "##."],
        't' => ["###", ".#.", ".#.", ".#.", ".#."],
        'u' => ["#.#", "#.#", "#.#", "#.#", "###"],
        'v' => ["#.#", "#.#", "#.#", "#.#", ".#."],
        'w' => ["#.#", "#.#", "###", "###", "#.#"],
        'y' => ["#.#", "#.#", ".#.", ".#.", ".#."],
        _ => ["...", "...", "...", "...", "..."],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shadow_shrinks_evenly_from_both_ends_and_is_gone_by_its_fade_height() {
        let feet = (14, 33);
        let flat = ContactShadow::under(feet, 0.0).expect("a shadow on the ground");
        assert_eq!(flat.width, 19);
        assert_eq!(flat.centre, 23.5);
        // A stride's bob is not a hop.
        assert_eq!(ContactShadow::under(feet, 1.0), Some(flat));
        let mut last = flat.width;
        for lift in 2..SHADOW_FADE_HEIGHT as u32 {
            let Some(shadow) = ContactShadow::under(feet, lift as f32) else {
                continue;
            };
            assert_eq!(
                shadow.centre, flat.centre,
                "it stays centred under the feet"
            );
            assert_eq!(shadow.width % 2, flat.width % 2, "on whole pixels");
            assert!(shadow.width <= last);
            last = shadow.width;
        }
        assert_eq!(ContactShadow::under(feet, SHADOW_FADE_HEIGHT + 1.0), None);
        let mirrored = flat.mirrored();
        assert_eq!(mirrored.left(), crate::FRAME_SIZE as f32 - 33.0);
    }

    #[test]
    fn every_shadow_width_has_its_own_cell_filled_to_its_ends() {
        let sheet = ContactShadow::sheet();
        for ground in [ShadowGround::Desktop, ShadowGround::Village] {
            for width in 1..=SHADOW_MAX_WIDTH {
                let cell = ContactShadow { centre: 0.0, width }.cell(ground);
                assert!(cell.y + cell.height <= SHADOW_SHEET_HEIGHT);
                assert_eq!(cell.height, ground.rows());
                let row = if ground == ShadowGround::Village {
                    1
                } else {
                    0
                };
                let y = (cell.y + row) as i32;
                assert!(sheet.get(0, y).a > 0 && sheet.get(width as i32 - 1, y).a > 0);
                assert_eq!(sheet.get(width as i32, y).a, 0, "{ground:?} {width}");
                // Never darker than the ground's own shading.
                for x in 0..width as i32 {
                    assert!(sheet.get(x, y).a <= GROUND_SHADE.a);
                }
            }
        }
    }

    fn all_sprites() -> Vec<SpriteRect> {
        let mut rects = Vec::new();
        for icon in BubbleIcon::ALL {
            for growth in [
                BubbleGrowth::Small,
                BubbleGrowth::Medium,
                BubbleGrowth::Full,
            ] {
                rects.push(UiAtlasRenderer::bubble(icon, growth));
            }
        }
        for icon in MenuIcon::ALL {
            rects.push(UiAtlasRenderer::menu_icon(icon, false));
            rects.push(UiAtlasRenderer::menu_icon(icon, true));
            rects.push(UiAtlasRenderer::menu_label(icon));
        }
        rects.push(UiAtlasRenderer::menu_unavailable_label());
        for cells in 2..=4 {
            rects.push(UiAtlasRenderer::menu_frame(cells).expect("2..=4 have frames"));
            rects.push(UiAtlasRenderer::menu_frame_plain(cells).expect("2..=4 have frames"));
        }
        rects.sort_by_key(|rect| (rect.y, rect.x, rect.width, rect.height));
        rects.dedup();
        rects
    }

    fn crop(canvas: &Canvas, rect: SpriteRect) -> Vec<Rgba> {
        (0..rect.height)
            .flat_map(|y| (0..rect.width).map(move |x| (x, y)))
            .map(|(x, y)| canvas.get((rect.x + x) as i32, (rect.y + y) as i32))
            .collect()
    }

    #[test]
    fn atlas_is_deterministic_and_inside_its_budget() {
        let first = UiAtlasRenderer::render();
        let second = UiAtlasRenderer::render();
        assert_eq!(first, second, "the ui atlas takes no seed and no clock");
        assert_eq!(first.width(), UI_ATLAS_WIDTH);
        assert_eq!(first.height(), UI_ATLAS_HEIGHT);
        const { assert!(UI_ATLAS_WIDTH <= 256 && UI_ATLAS_HEIGHT <= 128) };
        assert!(
            first.rgba_bytes().len() <= 131_072,
            "the ui atlas must stay within one 256x128 rgba upload"
        );
    }

    #[test]
    fn every_sprite_is_inside_the_atlas_and_claims_its_own_pixels() {
        let sprites = all_sprites();
        for rect in &sprites {
            assert!(
                rect.x + rect.width <= UI_ATLAS_WIDTH && rect.y + rect.height <= UI_ATLAS_HEIGHT,
                "{rect:?} leaves the atlas"
            );
            assert!(rect.width > 0 && rect.height > 0);
        }
        for (index, rect) in sprites.iter().enumerate() {
            for other in &sprites[index + 1..] {
                assert!(!rect.overlaps(other), "{rect:?} overlaps {other:?}");
            }
        }
    }

    #[test]
    fn every_full_bubble_is_a_different_picture_with_a_readable_icon() {
        let atlas = UiAtlasRenderer::render();
        let mut seen: Vec<(BubbleIcon, Vec<Rgba>)> = Vec::new();
        for icon in BubbleIcon::ALL {
            let rect = UiAtlasRenderer::bubble(icon, BubbleGrowth::Full);
            let pixels = crop(&atlas, rect);
            // The icon box is 9x9 at (4, 2) inside the cell; count what the icon itself draws.
            let drawn = (0..9)
                .flat_map(|y| (0..9).map(move |x| (x, y)))
                .filter(|(x, y)| {
                    let pixel = atlas.get((rect.x + 4 + x) as i32, (rect.y + 2 + y) as i32);
                    pixel.a > 0 && pixel != PAPER
                })
                .count();
            assert!(
                drawn >= 14,
                "{icon:?} draws only {drawn} icon pixels, too faint to recognise at 2x"
            );
            for (other, other_pixels) in &seen {
                assert_ne!(
                    pixels, *other_pixels,
                    "{icon:?} and {other:?} render the same bubble"
                );
            }
            seen.push((icon, pixels));
        }
    }

    #[test]
    fn growth_steps_share_one_cell_and_one_tail_tip() {
        for icon in BubbleIcon::ALL {
            for growth in [
                BubbleGrowth::Small,
                BubbleGrowth::Medium,
                BubbleGrowth::Full,
            ] {
                let rect = UiAtlasRenderer::bubble(icon, growth);
                assert_eq!((rect.width, rect.height), BUBBLE_CELL);
            }
        }
        let atlas = UiAtlasRenderer::render();
        for growth in [
            BubbleGrowth::Small,
            BubbleGrowth::Medium,
            BubbleGrowth::Full,
        ] {
            let rect = UiAtlasRenderer::bubble(BubbleIcon::Heart, growth);
            let tip = atlas.get((rect.x + BUBBLE_ANCHOR.0) as i32, (rect.y + 14) as i32);
            assert_eq!(tip, OUTLINE, "{growth:?} should end at the shared tail tip");
            let below = atlas.get((rect.x + BUBBLE_ANCHOR.0) as i32, (rect.y + 15) as i32);
            assert_eq!(below.a, 0, "the anchor row stays clear of the head");
        }
    }

    #[test]
    fn bubble_cells_are_symmetric_about_the_tail_so_a_flip_lands_on_itself() {
        let atlas = UiAtlasRenderer::render();
        for growth in [
            BubbleGrowth::Small,
            BubbleGrowth::Medium,
            BubbleGrowth::Full,
        ] {
            let rect = UiAtlasRenderer::bubble(BubbleIcon::Heart, growth);
            // Only the frame is mirror-symmetric; the icon may not be. Check the tail rows.
            for y in 12..16 {
                for x in 0..BUBBLE_CELL.0 {
                    let left = atlas.get((rect.x + x) as i32, (rect.y + y) as i32);
                    let right =
                        atlas.get((rect.x + BUBBLE_CELL.0 - 1 - x) as i32, (rect.y + y) as i32);
                    assert_eq!(left, right, "tail row {y} is off centre at x {x}");
                }
            }
        }
    }

    #[test]
    fn hovering_a_menu_icon_changes_it_in_brightness_not_only_hue() {
        let atlas = UiAtlasRenderer::render();
        let luma = |color: Rgba| {
            if color.a == 0 {
                // Bare cells let the strip's own paper through.
                return f32::from(PAPER_SOLID.r) * 0.299
                    + f32::from(PAPER_SOLID.g) * 0.587
                    + f32::from(PAPER_SOLID.b) * 0.114;
            }
            f32::from(color.r) * 0.299 + f32::from(color.g) * 0.587 + f32::from(color.b) * 0.114
        };
        for icon in MenuIcon::ALL {
            let normal = crop(&atlas, UiAtlasRenderer::menu_icon(icon, false));
            let hovered = crop(&atlas, UiAtlasRenderer::menu_icon(icon, true));
            assert_ne!(normal, hovered, "{icon:?} looks the same hovered");
            let average = |pixels: &[Rgba]| {
                pixels.iter().copied().map(luma).sum::<f32>() / pixels.len() as f32
            };
            assert!(
                average(&normal) - average(&hovered) > 12.0,
                "{icon:?} hover must be readable without colour vision"
            );
        }
    }

    #[test]
    fn labels_sit_inside_their_tabs() {
        let atlas = UiAtlasRenderer::render();
        for icon in MenuIcon::ALL {
            let rect = UiAtlasRenderer::menu_label(icon);
            assert_eq!(rect.height, LABEL_TAB_HEIGHT);
            assert_eq!(rect.width, label_tab_width(menu_label_text(icon)));
            // The tab's side walls are unbroken: nothing in the text spills into the frame.
            for y in 1..rect.height - 1 {
                for x in [0, rect.width - 1] {
                    assert_eq!(
                        atlas.get((rect.x + x) as i32, (rect.y + y) as i32),
                        OUTLINE,
                        "{icon:?} tab wall is broken at ({x}, {y})"
                    );
                }
            }
            // And the text is actually there, in the paper rows only.
            let ink = (2..rect.height - 2)
                .flat_map(|y| (2..rect.width - 2).map(move |x| (x, y)))
                .filter(|(x, y)| atlas.get((rect.x + x) as i32, (rect.y + y) as i32) == OUTLINE)
                .count();
            assert!(ink >= 8, "{icon:?} label looks empty");
        }
    }

    #[test]
    fn menu_layouts_pack_two_to_four_cells_without_overlap() {
        for count in MENU_MIN_ITEMS..=MENU_MAX_ITEMS {
            let items: Vec<MenuIcon> = MenuIcon::ALL.into_iter().take(count).collect();
            let layout = MenuLayout::new(&items);
            assert_eq!(layout.len(), count);
            let (width, height) = layout.size();
            assert_eq!(height, MENU_STRIP_HEIGHT);
            let frame = UiAtlasRenderer::menu_frame(count as u8).expect("2..=4 cells have a frame");
            assert_eq!((frame.width, frame.height), (width, height));
            for index in 0..count {
                let cell = layout.cell(index).expect("cell exists");
                assert!(cell.x >= 0 && cell.y >= 0);
                assert!(
                    cell.x + cell.width as i32 <= width as i32,
                    "cell {index} leaves the strip"
                );
                assert!(cell.y + cell.height as i32 <= MENU_BODY_HEIGHT as i32);
                for other in 0..index {
                    let earlier = layout.cell(other).expect("cell exists");
                    assert!(
                        earlier.x + earlier.width as i32 <= cell.x,
                        "cells {other} and {index} overlap"
                    );
                }
            }
            assert!(layout.notch_x() > 0 && layout.notch_x() < width as i32);
        }
    }

    /// A strip beside the menu points at nothing: its frame is only the tray, the same width as
    /// the pointing one, with an unbroken bottom edge where the notch would pierce it.
    #[test]
    fn a_plain_frame_is_the_tray_without_its_notch() {
        let atlas = UiAtlasRenderer::render();
        for cells in 2..=4_u8 {
            let plain = UiAtlasRenderer::menu_frame_plain(cells).unwrap();
            let pointing = UiAtlasRenderer::menu_frame(cells).unwrap();
            assert_eq!(plain.width, pointing.width);
            assert_eq!(plain.height, MENU_BODY_HEIGHT);
            let bottom = plain.y + plain.height - 1;
            for x in 1..plain.width - 1 {
                assert_eq!(
                    atlas.get((plain.x + x) as i32, bottom as i32),
                    OUTLINE,
                    "{cells} cells: the bottom edge is broken at {x}"
                );
            }
            // Row for row, the tray is the pointing frame's body.
            for y in 0..MENU_BODY_HEIGHT - 1 {
                for x in 0..plain.width {
                    assert_eq!(
                        atlas.get((plain.x + x) as i32, (plain.y + y) as i32),
                        atlas.get((pointing.x + x) as i32, (pointing.y + y) as i32),
                        "{cells} cells differ at ({x}, {y})"
                    );
                }
            }
        }
    }

    /// Every menu item is its own picture, so a strip never holds two that look alike.
    #[test]
    fn every_menu_icon_is_a_different_picture() {
        let atlas = UiAtlasRenderer::render();
        let mut seen: Vec<(MenuIcon, Vec<Rgba>)> = Vec::new();
        for icon in MenuIcon::ALL {
            let pixels = crop(&atlas, UiAtlasRenderer::menu_icon(icon, false));
            let drawn = pixels.iter().filter(|pixel| pixel.a > 0).count();
            assert!(drawn >= 30, "{icon:?} draws only {drawn} pixels");
            for (other, other_pixels) in &seen {
                assert_ne!(pixels, *other_pixels, "{icon:?} looks like {other:?}");
            }
            seen.push((icon, pixels));
        }
    }

    #[test]
    fn hit_test_answers_at_every_cell_centre_and_nowhere_else() {
        let items = [
            MenuIcon::Snack,
            MenuIcon::Toy,
            MenuIcon::Home,
            MenuIcon::Profile,
        ];
        let layout = MenuLayout::new(&items);
        for index in 0..items.len() {
            let cell = layout.cell(index).expect("cell exists");
            let centre = (
                cell.x as f32 + cell.width as f32 / 2.0,
                cell.y as f32 + cell.height as f32 / 2.0,
            );
            assert_eq!(layout.hit_test(centre.0, centre.1), Some(index));
            // Corners of the cell belong to it; one pixel outside does not.
            assert_eq!(layout.hit_test(cell.x as f32, cell.y as f32), Some(index));
            assert_eq!(layout.hit_test(cell.x as f32 - 1.0, centre.1), None);
        }
        let (width, height) = layout.size();
        for point in [
            (0.0, 0.0),
            (width as f32 / 2.0, 0.5),
            (width as f32 - 0.5, height as f32 / 2.0),
            (width as f32 / 2.0, MENU_BODY_HEIGHT as f32 + 1.0),
            (-1.0, -1.0),
        ] {
            assert_eq!(
                layout.hit_test(point.0, point.1),
                None,
                "{point:?} is frame, gap or notch"
            );
        }
    }

    #[test]
    fn label_tabs_hang_under_the_strip_and_stay_with_it() {
        let items = [
            MenuIcon::Snack,
            MenuIcon::Toy,
            MenuIcon::CopyCode,
            MenuIcon::Profile,
        ];
        let layout = MenuLayout::new(&items);
        let (width, height) = layout.size();
        for (index, icon) in items.into_iter().enumerate() {
            let tab = layout.label_tab(index).expect("tab exists");
            assert_eq!(tab.y, (height + LABEL_TAB_GAP) as i32);
            assert_eq!(tab.height, LABEL_TAB_HEIGHT);
            assert_eq!(
                tab.width,
                UiAtlasRenderer::menu_label(icon).width,
                "the tab is exactly its sprite"
            );
            assert!(tab.x >= 0 && tab.x + tab.width as i32 <= width as i32);
        }
    }

    /// Every label at once: one row where the single tab hangs, in the strip's order, never
    /// touching, centred under the strip.
    #[test]
    fn the_label_row_lays_every_tab_side_by_side_centred_under_the_strip() {
        for items in [
            &[
                MenuIcon::Snack,
                MenuIcon::Toy,
                MenuIcon::Home,
                MenuIcon::Profile,
            ][..],
            &[
                MenuIcon::Snack,
                MenuIcon::Toy,
                MenuIcon::CopyCode,
                MenuIcon::Profile,
            ],
            &[MenuIcon::Inside, MenuIcon::Profile],
        ] {
            let layout = MenuLayout::new(items);
            let row = layout.label_row();
            let tabs: Vec<MenuRect> = row.iter().flatten().copied().collect();
            assert_eq!(tabs.len(), items.len());
            assert!(row[items.len()..].iter().all(Option::is_none));
            for (index, tab) in tabs.iter().enumerate() {
                assert_eq!(
                    tab.y,
                    layout.label_tab(index).unwrap().y,
                    "level with a tab"
                );
                assert_eq!(tab.width, layout.label_tab(index).unwrap().width);
                if let Some(next) = tabs.get(index + 1) {
                    assert_eq!(next.x, tab.x + tab.width as i32 + LABEL_ROW_GAP as i32);
                }
            }
            let left = tabs[0].x;
            let right = tabs.last().map(|tab| tab.x + tab.width as i32).unwrap();
            let overhang = (left, layout.size().0 as i32 - right);
            assert!(
                (overhang.0 - overhang.1).abs() <= 1,
                "centred under the strip: {overhang:?}"
            );
        }
        assert!(MenuLayout::new(&[]).label_row().iter().all(Option::is_none));
    }

    #[test]
    fn a_menu_outside_two_to_four_items_is_empty() {
        for items in [
            vec![],
            vec![MenuIcon::Snack],
            vec![MenuIcon::Snack; 5],
            vec![MenuIcon::Snack; 9],
        ] {
            let layout = MenuLayout::new(&items);
            assert!(layout.is_empty());
            assert_eq!(layout.size(), (0, 0));
            assert_eq!(layout.cell(0), None);
            assert_eq!(layout.item(0), None);
            assert_eq!(layout.label_tab(0), None);
            assert_eq!(layout.hit_test(1.0, 1.0), None);
        }
        assert_eq!(UiAtlasRenderer::menu_frame(1), None);
        assert_eq!(UiAtlasRenderer::menu_frame(5), None);
        assert_eq!(UiAtlasRenderer::menu_frame(0), None);
    }
}
