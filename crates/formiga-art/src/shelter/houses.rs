//! The four kinds of house, each drawn in the creatures' own pixel-art language: separate
//! materials for roof, walls and trim, each with a base, a shade and a light, one light from the
//! upper left, a recessed doorway on a threshold, and one or two things that say somebody lives
//! there. They stay quieter than the residents in front of them: nothing here outshines a face.

use crate::{Canvas, Palette, Rgba};
use formiga_core::ShelterStyle;
use std::f32::consts::PI;

/// The doorway every style opens into the same near-black, so the village can measure exactly
/// how wide a door is from its pixels.
pub(super) const DOORWAY: Rgba = Rgba::new(25, 23, 31, 255);

/// Window glass: dark, but never the doorway's own near-black, which is how a door is measured.
const GLASS: Rgba = Rgba::new(40, 46, 64, 255);

/// Warm lamplight, for a window or a doorway after dark.
const GLOW: Rgba = Rgba::new(255, 206, 118, 255);
const GLOW_CORE: Rgba = Rgba::new(255, 236, 178, 255);

/// Cream, for mushroom stems, spots and paper; bark and straw for the leaf tent; stone for a
/// step. Fixed, like wood and paper are, so the colony's own colours stay on what is dyed.
const CREAM: Rgba = Rgba::new(240, 228, 204, 255);
const PAPER: Rgba = Rgba::new(242, 230, 204, 255);
const BARK: Rgba = Rgba::new(128, 88, 60, 255);
const STRAW: Rgba = Rgba::new(226, 190, 116, 255);
const STONE: Rgba = Rgba::new(176, 170, 162, 255);
const LEAF: Rgba = Rgba::new(92, 158, 80, 255);
const WHITE: Rgba = Rgba::new(255, 252, 240, 255);

pub(super) fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let channel = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Rgba::new(channel(a.r, b.r), channel(a.g, b.g), channel(a.b, b.b), 255)
}

/// One material: its own colour, the same in shadow, and the same where the light catches it.
#[derive(Clone, Copy)]
pub(super) struct Tone {
    pub(super) base: Rgba,
    pub(super) shade: Rgba,
    pub(super) light: Rgba,
}

impl Tone {
    pub(super) fn of(base: Rgba, ink: Rgba) -> Self {
        Self {
            base,
            shade: mix(base, ink, 0.3),
            light: mix(base, WHITE, 0.38),
        }
    }
}

/// The colours one house is built from: the colony's own shelter palette dyes the roof or the
/// canopy, its accent palette the trim, and the walls are whatever the style is made of.
#[derive(Clone, Copy)]
pub(super) struct Materials {
    pub(super) outline: Rgba,
    pub(super) roof: Tone,
    pub(super) wall: Tone,
    pub(super) trim: Tone,
    pub(super) second: Tone,
}

impl Materials {
    pub(super) fn for_style(style: ShelterStyle, palette: Palette, accent: Palette) -> Self {
        let ink = palette.outline;
        let tone = |base| Tone::of(base, ink);
        match style {
            // Leaves tinted toward the colony's colour, bark supports, twine trim.
            ShelterStyle::LeafTent => Self {
                outline: ink,
                roof: tone(mix(LEAF, palette.coat, 0.15)),
                wall: tone(BARK),
                trim: tone(accent.coat),
                second: tone(STRAW),
            },
            // A cap in the colony's colour on a cream stem.
            ShelterStyle::MushroomHut => Self {
                outline: ink,
                roof: tone(palette.coat),
                wall: tone(CREAM),
                trim: tone(accent.coat),
                second: tone(STONE),
            },
            // Two coordinated fabrics: the canopy and the cushions.
            ShelterStyle::CushionDen => Self {
                outline: ink,
                roof: tone(palette.coat),
                wall: tone(BARK),
                trim: tone(mix(palette.highlight, WHITE, 0.3)),
                second: tone(accent.coat),
            },
            // Warm card walls under a roof folded from the accent colour it has always had, and a
            // mat at the door in the colony's own.
            ShelterStyle::PaperHouse => Self {
                outline: ink,
                roof: tone(accent.coat),
                wall: tone(mix(PAPER, palette.coat, 0.08)),
                trim: tone(palette.coat),
                second: tone(accent.highlight),
            },
        }
    }
}

/// Everything one house needs to know about itself.
#[derive(Clone, Copy)]
pub(super) struct House {
    pub(super) style: ShelterStyle,
    pub(super) cx: i32,
    pub(super) bottom: i32,
    pub(super) width: i32,
    pub(super) height: i32,
    pub(super) span: i32,
    pub(super) seed: u64,
    /// Lit from inside, after dark.
    pub(super) lit: bool,
    /// Whose house it is.
    pub(super) mark: Option<super::ResidentMark>,
}

impl House {
    fn unit(&self, value: i32) -> i32 {
        (value * self.span / super::MAIN_SPAN).max(1)
    }

    fn left(&self) -> i32 {
        self.cx - self.width / 2
    }

    fn right(&self) -> i32 {
        self.left() + self.width
    }

    fn top(&self) -> i32 {
        self.bottom - self.height
    }

    /// Width and height of the doorway: a quarter of the house wide, a third of it tall.
    pub(super) fn door(&self) -> (i32, i32) {
        (
            (self.width / 4).clamp(5, self.width / 2),
            (self.height / 3).clamp(6, self.height / 2),
        )
    }

    /// A stable small number from the house's own seed, for the choices that make one house of a
    /// style not quite another.
    fn pick(&self, salt: u32, range: u64) -> u64 {
        let mixed = (self.seed.rotate_left(salt * 11) ^ 0x9e37_79b9_7f4a_7c15)
            .wrapping_mul(0xbf58_476d_1ce4_e5b9);
        (mixed >> 40) % range.max(1)
    }
}

pub(super) fn draw_house(canvas: &mut Canvas, house: House, m: Materials) {
    match house.style {
        ShelterStyle::LeafTent => leaf_tent(canvas, house, m),
        ShelterStyle::MushroomHut => mushroom_hut(canvas, house, m),
        ShelterStyle::CushionDen => cushion_den(canvas, house, m),
        ShelterStyle::PaperHouse => paper_house(canvas, house, m),
    }
}

/// A shape filled row by row between two edges, with a one-pixel outline around it.
fn fill_rows(
    canvas: &mut Canvas,
    top: i32,
    bottom: i32,
    edges: impl Fn(i32) -> (i32, i32),
    fill: Rgba,
    outline: Rgba,
) {
    for y in top - 1..=bottom + 1 {
        let (left, right) = edges(y.clamp(top, bottom));
        canvas.fill_rect(left - 1, y, right - left + 3, 1, outline);
    }
    for y in top..=bottom {
        let (left, right) = edges(y);
        canvas.fill_rect(left, y, right - left + 1, 1, fill);
    }
}

/// A leaf standing on its stalk end: pointed at `apex`, as wide as `base_left..=base_right` at
/// `base_y`, swelling out past the straight line on each side by up to `bulge`.
#[allow(clippy::too_many_arguments)]
fn leaf(
    canvas: &mut Canvas,
    apex: (i32, i32),
    base_left: i32,
    base_right: i32,
    base_y: i32,
    bulge: (f32, f32),
    fill: Tone,
    lit_left: bool,
    outline: Rgba,
) {
    let height = (base_y - apex.1).max(1) as f32;
    let edges = |y: i32| {
        let t = ((y - apex.1) as f32 / height).clamp(0.0, 1.0);
        let swell = (t * PI * 0.9).sin();
        let left = apex.0 as f32 + (base_left - apex.0) as f32 * t - bulge.0 * swell;
        let right = apex.0 as f32 + (base_right - apex.0) as f32 * t + bulge.1 * swell;
        (left.round() as i32, right.round() as i32)
    };
    fill_rows(canvas, apex.1, base_y, edges, fill.base, outline);
    // The side facing the light, and the side turned away from it.
    for y in apex.1 + 1..=base_y {
        let (left, right) = edges(y);
        if lit_left {
            canvas.set(left, y, fill.light);
        } else {
            canvas.set(right, y, fill.shade);
            canvas.set(right - 1, y, fill.shade);
        }
    }
    // The vein down the middle, and a few ribs off it.
    let foot = ((base_left + base_right) / 2, base_y - 1);
    let vein = if lit_left { fill.light } else { fill.base };
    canvas.line(apex.0, apex.1 + 2, foot.0, foot.1, 1, vein);
    for step in 1..=3 {
        let t = step as f32 / 4.0;
        let x = (apex.0 as f32 + (foot.0 - apex.0) as f32 * t).round() as i32;
        let y = (apex.1 as f32 + (foot.1 - apex.1) as f32 * t).round() as i32;
        canvas.line(x, y, x - 2, y + 2, 1, vein);
        canvas.line(x, y, x + 2, y + 2, 1, vein);
    }
}

/// The door every house has: an arch recessed into the wall, darkest at the back, with a step or
/// a threshold in front. After dark the back of it glows.
fn doorway(canvas: &mut Canvas, house: House, frame: Rgba, threshold: Rgba) {
    let (door_width, door_height) = house.door();
    let (cx, bottom) = (house.cx, house.bottom);
    let (rx, ry) = (door_width / 2, door_height / 2);
    canvas.fill_ellipse(cx, bottom - ry - 1, rx + 1, ry + 1, frame);
    canvas.fill_rect(cx - rx - 1, bottom - ry - 1, door_width + 2, ry + 2, frame);
    canvas.fill_ellipse(cx, bottom - ry - 1, rx, ry, DOORWAY);
    canvas.fill_rect(cx - rx, bottom - ry - 1, door_width, ry + 1, DOORWAY);
    if house.lit {
        // Lamplight filling the room inside, brightest low down where the lamp is, with a dark
        // rim left round the frame so the doorway still reads as a hole in the wall.
        canvas.fill_ellipse(cx, bottom - ry, (rx - 1).max(1), (ry - 1).max(1), GLOW);
        canvas.fill_rect(
            cx - rx + 1,
            bottom - ry,
            (door_width - 2).max(1),
            ry - 1,
            GLOW,
        );
        canvas.fill_ellipse(
            cx,
            bottom - ry / 2 - 1,
            (rx - 2).max(1),
            (ry / 2).max(1),
            GLOW_CORE,
        );
    }
    canvas.fill_rect(cx - rx - 1, bottom - 1, door_width + 2, 1, threshold);
    if let Some(mark) = house.mark {
        curtain(canvas, house, mark);
    }
}

/// The resident's own curtain, hung in the doorway and tied back to one side: a third of the door
/// in the resident's colours, with a fold down it and a tie across it.
fn curtain(canvas: &mut Canvas, house: House, mark: super::ResidentMark) {
    let (door_width, door_height) = house.door();
    let (cx, bottom) = (house.cx, house.bottom);
    let rx = door_width / 2;
    let width = ((door_width + 2) / 3).max(2);
    let top = bottom - door_height + 1;
    let (x0, fold_x) = if mark.tied_left {
        (cx - rx, cx - rx + width - 1)
    } else {
        (cx + rx - width + 1, cx + rx - width + 1)
    };
    for y in top..bottom - 1 {
        for x in x0..x0 + width {
            if canvas.get(x, y) == DOORWAY || house.lit && canvas.get(x, y).a > 0 {
                canvas.set(x, y, mark.cloth);
            }
        }
        if canvas.get(fold_x, y) == mark.cloth {
            canvas.set(fold_x, y, mark.fold);
        }
    }
    // Tied back a little below halfway, where it gathers.
    let tie_y = bottom - door_height / 2 + 1;
    for x in x0..x0 + width {
        if canvas.get(x, tie_y) == mark.cloth || canvas.get(x, tie_y) == mark.fold {
            canvas.set(x, tie_y, mark.tie);
        }
    }
}

/// A round blob `size` pixels across: a square with its corners taken off, which at this scale
/// reads rounder than a small circle does.
fn blob(canvas: &mut Canvas, x: i32, y: i32, size: i32, color: Rgba) {
    let size = size.max(2);
    for row in 0..size {
        for column in 0..size {
            let corner = (row == 0 || row == size - 1) && (column == 0 || column == size - 1);
            if !corner || size == 2 {
                canvas.set(x + column, y + row, color);
            }
        }
    }
}

/// A small round window: a frame, dark glass, and a glint, or lamplight after dark.
fn round_window(canvas: &mut Canvas, x: i32, y: i32, size: i32, frame: Rgba, lit: bool) {
    blob(canvas, x - 1, y - 1, size + 2, frame);
    blob(canvas, x, y, size, if lit { GLOW } else { GLASS });
    if lit {
        canvas.set(x + size / 2, y + size / 2, GLOW_CORE);
    } else {
        canvas.set(x + 1, y + 1, mix(GLASS, WHITE, 0.55));
    }
}

/// Two or three overlapping leaves on crossed twigs, tied at the top, the near leaf lifted at
/// the door and a little planter set down beside it.
fn leaf_tent(canvas: &mut Canvas, house: House, m: Materials) {
    let (top, left, right, bottom) = (house.top(), house.left(), house.right(), house.bottom);
    let base = bottom - 2;
    let u = |value| house.unit(value);
    // Crossed twigs holding the leaves up, poking out above them where they are tied.
    for (from, to) in [
        ((house.cx - u(4), top - u(3)), (house.cx + u(3), top + u(6))),
        ((house.cx + u(4), top - u(3)), (house.cx - u(3), top + u(6))),
    ] {
        canvas.line(from.0, from.1, to.0, to.1, 2, m.outline);
        canvas.line(from.0, from.1, to.0, to.1, 1, m.wall.base);
    }
    // Three big leaves leaned together: the one at the back tallest and in shadow, one on the
    // right half-shaded, and the near one on the left catching the light, each with its own tip.
    let back = Tone {
        base: m.roof.shade,
        shade: mix(m.roof.shade, m.outline, 0.3),
        light: m.roof.base,
    };
    leaf(
        canvas,
        (house.cx + u(1), top),
        house.cx - u(6),
        house.cx + u(8),
        base,
        (1.5, 1.5),
        back,
        false,
        m.outline,
    );
    leaf(
        canvas,
        (house.cx + u(4), top + u(4)),
        house.cx - u(1),
        right + u(1),
        base,
        (0.0, 2.5),
        m.roof,
        false,
        m.outline,
    );
    leaf(
        canvas,
        (house.cx - u(3), top + u(3)),
        left - u(1),
        house.cx + u(3),
        base,
        (2.5, 0.0),
        m.roof,
        true,
        m.outline,
    );
    // Curled tips at the foot of each leaf.
    for (x, out) in [(left - u(1), -1), (right + u(1), 1)] {
        canvas.set(x + out, base - 1, m.roof.light);
        canvas.set(x + out * 2, base - 2, m.roof.light);
        canvas.set(x + out * 2, base - 1, m.outline);
    }
    // The twine where the twigs cross.
    canvas.fill_rect(house.cx - 1, top + u(1), 3, 2, m.trim.base);
    canvas.set(house.cx - 1, top + u(1), m.trim.light);
    // The door, on a straw floor, with the near leaf's corner folded back beside it.
    doorway(canvas, house, m.outline, m.second.base);
    let (door_width, door_height) = house.door();
    canvas.fill_rect(
        house.cx - door_width / 2 + 1,
        bottom - 3,
        door_width - 2,
        2,
        m.second.shade,
    );
    let flap = house.cx + door_width / 2 + 1;
    let flap_top = bottom - door_height;
    for row in 0..door_height / 2 {
        canvas.fill_rect(
            flap,
            flap_top + row,
            (row / 2 + 1).min(u(4)),
            1,
            m.roof.light,
        );
    }
    canvas.line(
        flap,
        flap_top,
        flap + u(3),
        flap_top + door_height / 2,
        1,
        m.outline,
    );
    // A cup of an acorn, planted with a sprout, by the door.
    let pot = house.cx - door_width / 2 - u(5);
    if house.span >= super::COTTAGE_SPAN {
        canvas.fill_rect(pot - 2, bottom - 3, 5, 3, m.outline);
        canvas.fill_rect(pot - 1, bottom - 3, 3, 2, m.wall.base);
        canvas.set(pot - 1, bottom - 3, m.wall.light);
        canvas.line(pot, bottom - 4, pot, bottom - 6, 1, LEAF);
        canvas.set(pot - 1, bottom - 6, mix(LEAF, WHITE, 0.3));
        canvas.set(pot + 1, bottom - 5, mix(LEAF, WHITE, 0.3));
    }
}

/// A fuller domed cap with a shaded underside and a few spots, on a pale stem with a round
/// window and a doorstep.
fn mushroom_hut(canvas: &mut Canvas, house: House, m: Materials) {
    let (top, bottom) = (house.top(), house.bottom);
    let u = |value| house.unit(value);
    let cap_height = house.height * 11 / 20;
    let cap_bottom = top + cap_height;
    let stem_half = house.width * 7 / 20;
    // The stem, flaring a little at its foot, lit on the left and shaded on the right.
    let stem = |y: i32| {
        let flare = ((y - (bottom - u(4))).max(0) + 1) / 2;
        (house.cx - stem_half - flare, house.cx + stem_half + flare)
    };
    fill_rows(
        canvas,
        cap_bottom - u(2),
        bottom - 1,
        stem,
        m.wall.base,
        m.outline,
    );
    for y in cap_bottom..bottom {
        let (left, right) = stem(y);
        canvas.set(left, y, m.wall.light);
        canvas.fill_rect(right - u(2), y, u(2), 1, m.wall.shade);
    }
    // The cap: a dome, lit from the upper left, rimmed underneath with its gills in shadow.
    let rx = house.width / 2 + u(2);
    let dome = |y: i32| {
        let t = ((cap_bottom - y) as f32 / cap_height as f32).clamp(0.0, 1.0);
        let half = (rx as f32 * (1.0 - t * t).max(0.0).sqrt()).round() as i32;
        (house.cx - half, house.cx + half)
    };
    fill_rows(canvas, top + 1, cap_bottom, dome, m.roof.base, m.outline);
    for y in top + 1..=cap_bottom {
        let (left, right) = dome(y);
        let width = right - left;
        canvas.fill_rect(left, y, (width / 5).max(1), 1, m.roof.light);
        canvas.fill_rect(right - width / 6, y, (width / 6).max(1), 1, m.roof.shade);
    }
    canvas.fill_rect(
        house.cx - rx + u(2),
        cap_bottom,
        (rx - u(2)) * 2 + 1,
        1,
        m.roof.shade,
    );
    canvas.fill_rect(
        house.cx - stem_half,
        cap_bottom + 1,
        stem_half * 2 + 1,
        1,
        m.wall.shade,
    );
    // Spots: one large one where the light falls, and smaller ones around it.
    let spots = [
        (-rx / 2, cap_height * 2 / 5, 3),
        (rx / 4, cap_height / 4, 2),
        (rx * 3 / 5, cap_height * 3 / 5, 2),
    ];
    for (index, (dx, dy, size)) in spots.into_iter().enumerate() {
        let shift = house.pick(index as u32, 3) as i32 - 1;
        let (x, y) = (house.cx + dx + shift, top + 1 + dy);
        let size = u(size + 1);
        blob(canvas, x, y, size, CREAM);
        canvas.set(x + size - 1, y + size - 2, mix(CREAM, m.roof.base, 0.45));
    }
    canvas.set(house.cx - rx / 6, top + 2, CREAM);
    // A round window set into the stem, beside the door.
    let (door_width, door_height) = house.door();
    let size = if house.span >= super::COTTAGE_SPAN {
        4
    } else {
        3
    };
    let window_x = house.cx - door_width / 2 - u(3) - size;
    if window_x - 1 > house.cx - stem_half {
        round_window(
            canvas,
            window_x,
            bottom - door_height,
            size,
            m.trim.base,
            house.lit,
        );
    }
    doorway(canvas, house, m.outline, m.wall.shade);
    // A flat stepping stone in front of the door.
    canvas.fill_ellipse(house.cx, bottom, door_width / 2 + u(2), 1, m.second.shade);
    canvas.fill_rect(
        house.cx - door_width / 2,
        bottom,
        door_width,
        1,
        m.second.base,
    );
}

/// A pillow fort: two stacks of cushions for walls, a gingham blanket thrown over them for a roof
/// and hanging in scallops over the door, a patch sewn on it, a pillow glimpsed inside and a
/// cushion on the doorstep.
fn cushion_den(canvas: &mut Canvas, house: House, m: Materials) {
    let (top, left, right, bottom) = (house.top(), house.left(), house.right(), house.bottom);
    let u = |value| house.unit(value);
    let (door_width, door_height) = house.door();
    let roof_bottom = bottom - door_height - u(2);
    // Two stacks of cushions either side of the door, buttoned, the left stack lit and the right
    // one in shade.
    let stack_width = ((house.width - door_width) / 2 - u(1)).max(u(4));
    let cushion_height = ((bottom - roof_bottom + u(3)) / 2).max(u(3));
    for (index, x) in [left + u(1), right - u(1) - stack_width]
        .into_iter()
        .enumerate()
    {
        let lit = index == 0;
        for level in 0..2 {
            let y = bottom - cushion_height * (level + 1);
            canvas.fill_rect(x, y + 1, stack_width, cushion_height - 1, m.outline);
            canvas.fill_rect(x + 1, y, stack_width - 2, cushion_height + 1, m.outline);
            let fill = if lit { m.second.base } else { m.second.shade };
            canvas.fill_rect(x + 1, y + 1, stack_width - 2, cushion_height - 1, fill);
            canvas.fill_rect(
                x + 2,
                y + 1,
                stack_width - 4,
                1,
                if lit { m.second.light } else { m.second.base },
            );
            canvas.set(x + stack_width / 2, y + cushion_height / 2 + 1, m.outline);
        }
    }
    // The blanket: a soft arch thrown over both stacks, overhanging them, gingham woven through
    // it, lit on the left.
    let overhang = u(3);
    let (roof_left, roof_right) = (left - overhang, right + overhang);
    let half = ((roof_right - roof_left) / 2).max(1) as f32;
    let crown = move |x: i32| {
        let dx = (x - house.cx) as f32 / half;
        (top as f32 + (dx * dx) * (roof_bottom - top) as f32 * 0.55).round() as i32
    };
    // The blanket's corners fall lower than its middle, draped down over the cushions.
    let hang = move |x: i32| {
        let from_edge = (x - roof_left).min(roof_right - x);
        roof_bottom + (u(5) - from_edge).clamp(0, u(5))
    };
    for x in roof_left - 1..=roof_right + 1 {
        let clamped = x.clamp(roof_left, roof_right);
        let y0 = crown(clamped);
        canvas.fill_rect(x, y0 - 1, 1, hang(clamped) - y0 + 3, m.outline);
    }
    for x in roof_left..=roof_right {
        for y in crown(x)..=hang(x) {
            // Gingham: bands two pixels wide crossing every four, darker where they cross.
            let (across, down) = ((x - left).rem_euclid(4) < 2, (y - top).rem_euclid(4) < 2);
            let color = match (across, down) {
                (true, true) => m.roof.shade,
                (true, false) | (false, true) => m.roof.base,
                _ => m.roof.light,
            };
            canvas.set(x, y, color);
        }
        canvas.set(
            x,
            crown(x),
            if x < house.cx {
                m.roof.light
            } else {
                m.roof.base
            },
        );
    }
    // Folds where the draped corners hang, in shadow on the right.
    for x in [roof_left + u(2), roof_right - u(2)] {
        for y in roof_bottom - u(3)..=hang(x) {
            canvas.set(x, y, m.roof.shade);
        }
    }
    // Scallops along the front edge, hanging over the door.
    let mut x = roof_left + u(5);
    while x <= roof_right - u(5) {
        canvas.set(x, roof_bottom + 1, m.roof.base);
        canvas.set(x + 1, roof_bottom + 1, m.roof.shade);
        canvas.set(x + 1, roof_bottom + 2, m.outline);
        canvas.set(x, roof_bottom + 2, m.outline);
        canvas.set(x + 2, roof_bottom + 1, m.outline);
        x += 3;
    }
    // A patch sewn on the blanket in the cushions' fabric, its stitches running round it.
    if house.span >= super::COTTAGE_SPAN {
        let (px, py) = (
            house.cx + house.width / 8,
            crown(house.cx + house.width / 8) + u(3),
        );
        let (pw, ph) = (u(6), u(5));
        canvas.fill_rect(px, py, pw, ph, m.second.base);
        canvas.fill_rect(px, py, pw, 1, m.second.light);
        for step in 0..(pw + ph) * 2 {
            if step % 2 == 1 {
                continue;
            }
            let (x, y) = match step {
                s if s < pw => (px + s, py - 1),
                s if s < pw + ph => (px + pw, py + s - pw),
                s if s < pw * 2 + ph => (px + pw * 2 + ph - 1 - s, py + ph),
                s => (px - 1, py + (pw + ph) * 2 - 1 - s),
            };
            canvas.set(x, y, m.trim.base);
        }
    }
    doorway(canvas, house, m.outline, m.second.shade);
    // A pillow glimpsed inside, on the floor at the back.
    let rx = door_width / 2;
    canvas.fill_rect(house.cx - rx, bottom - u(4), rx + u(1), u(3), m.outline);
    canvas.fill_rect(
        house.cx - rx + 1,
        bottom - u(4) + 1,
        (rx - 1).max(1),
        (u(3) - 2).max(1),
        CREAM,
    );
    // A floor cushion on the doorstep, in the blanket's colour, buttoned in its middle.
    if house.span >= super::COTTAGE_SPAN {
        let x = house.cx + door_width / 2 + u(4);
        canvas.fill_ellipse(x, bottom - 1, u(4), u(2), m.outline);
        canvas.fill_ellipse(x, bottom - 2, u(3), u(1), m.roof.base);
        canvas.set(x - 1, bottom - 3, m.roof.light);
        canvas.set(x, bottom - 2, m.outline);
    }
}

/// Folded card: a front wall and a side wall turned away from the light, a roof folded along its
/// ridge and overlapping the walls, a strip of tape, a cut-out window and a mat at the door.
fn paper_house(canvas: &mut Canvas, house: House, m: Materials) {
    let (top, left, right, bottom) = (house.top(), house.left(), house.right(), house.bottom);
    let u = |value| house.unit(value);
    let eave = top + u(12);
    let side = u(4);
    // The front wall, lit, and the side wall folded back from it, in shade.
    canvas.fill_rect(
        left,
        eave - u(1),
        house.width - side + 1,
        bottom - eave + u(1),
        m.outline,
    );
    canvas.fill_rect(
        left + 1,
        eave,
        house.width - side - 1,
        bottom - eave - 1,
        m.wall.base,
    );
    canvas.fill_rect(left + 1, eave, 1, bottom - eave - 1, m.wall.light);
    canvas.fill_rect(
        right - side,
        eave - u(1),
        side + 1,
        bottom - eave + u(1),
        m.outline,
    );
    canvas.fill_rect(
        right - side + 1,
        eave,
        side - 1,
        bottom - eave - 1,
        m.wall.shade,
    );
    // The fold between the two walls.
    canvas.line(
        right - side,
        eave,
        right - side,
        bottom - 2,
        1,
        m.wall.shade,
    );
    // The roof: folded along its ridge, light on the near slope and shaded on the far one, its
    // edge overhanging both walls and casting a shadow under it.
    let peak = (house.cx - u(1), top);
    let slope = move |y: i32| {
        let t = ((y - peak.1) as f32 / (eave - peak.1).max(1) as f32).clamp(0.0, 1.0);
        let reach = (t * (house.width as f32 / 2.0 + 3.0)).round() as i32;
        (peak.0 - reach, peak.0 + reach)
    };
    fill_rows(canvas, peak.1, eave, slope, m.roof.base, m.outline);
    for y in peak.1 + 1..=eave {
        let (l, _) = slope(y);
        let width = (peak.0 - l).max(0);
        canvas.fill_rect(l, y, width, 1, m.roof.light);
        canvas.set(peak.0, y, m.roof.shade);
    }
    canvas.fill_rect(left + 1, eave + 2, house.width - side - 1, 1, m.wall.shade);
    // A strip of tape holding the roof to the front wall.
    canvas.fill_rect(left + u(3), eave, u(3), u(3), mix(STRAW, WHITE, 0.45));
    // A cut-out window with its panes drawn in.
    let (door_width, door_height) = house.door();
    let window_x = left + (house.cx - door_width / 2 - left) / 2 - u(2);
    let window_y = bottom - door_height - u(2);
    if house.span >= super::COTTAGE_SPAN {
        canvas.fill_rect(window_x - 1, window_y - 1, u(6), u(6), m.outline);
        canvas.fill_rect(
            window_x,
            window_y,
            u(4),
            u(4),
            if house.lit { GLOW } else { GLASS },
        );
        canvas.line(
            window_x + u(2),
            window_y,
            window_x + u(2),
            window_y + u(3),
            1,
            m.outline,
        );
        canvas.line(
            window_x,
            window_y + u(2),
            window_x + u(3),
            window_y + u(2),
            1,
            m.outline,
        );
        if !house.lit {
            canvas.set(window_x, window_y, mix(GLASS, WHITE, 0.5));
        }
    }
    doorway(canvas, house, m.outline, m.wall.shade);
    // A little mat in front of the door, striped.
    let mat = door_width + u(2);
    canvas.fill_rect(house.cx - mat / 2, bottom, mat, 1, m.trim.base);
    for x in (house.cx - mat / 2..house.cx + mat / 2).step_by(2) {
        canvas.set(x, bottom, m.trim.light);
    }
}
