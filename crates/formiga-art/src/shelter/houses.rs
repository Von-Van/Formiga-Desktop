//! The four kinds of house, each drawn in the creatures' own pixel-art language: separate
//! materials for roof, walls and trim, each with a base, a shade and a light, one light from the
//! upper left, a recessed doorway on a threshold, and one or two things that say somebody lives
//! there. They stay quieter than the residents in front of them: nothing here outshines a face.

use crate::{Canvas, Palette, Rgba};
use formiga_core::ShelterStyle;

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
            // Canvas in the colony's colour, bark poles and pegs, a pennant in the accent, and a
            // straw floor inside.
            ShelterStyle::Tent => Self {
                outline: ink,
                roof: tone(palette.coat),
                wall: tone(BARK),
                trim: tone(accent.coat),
                second: tone(STRAW),
            },
            // A cap in the colony's colour on a cream stem.
            ShelterStyle::Mushroom => Self {
                outline: ink,
                roof: tone(palette.coat),
                wall: tone(CREAM),
                trim: tone(accent.coat),
                second: tone(STONE),
            },
            // Two coordinated fabrics: the blanket in the colony's colour and the cushions in
            // the accent, with pale piping.
            ShelterStyle::PillowFort => Self {
                outline: ink,
                roof: tone(palette.coat),
                wall: tone(mix(palette.coat, WHITE, 0.5)),
                trim: tone(mix(palette.highlight, WHITE, 0.45)),
                second: tone(accent.coat),
            },
            // Warm card walls under a roof thatched with leaves in the accent colour the roof has
            // always had, touched with green, and a mat at the door in the colony's own.
            ShelterStyle::LeafHouse => Self {
                outline: ink,
                roof: tone(mix(accent.coat, LEAF, 0.22)),
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
    /// Its resident is at home: the curtain is drawn across the door and a lamp is lit inside,
    /// whatever the hour.
    pub(super) occupied: bool,
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
        ShelterStyle::Tent => tent(canvas, house, m),
        ShelterStyle::Mushroom => mushroom_hut(canvas, house, m),
        ShelterStyle::PillowFort => pillow_fort(canvas, house, m),
        ShelterStyle::LeafHouse => leaf_house(canvas, house, m),
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
    hang_curtain(canvas, house, frame);
}

/// The resident's curtain: tied back to one side while the house is empty, drawn right across
/// while somebody is home.
fn hang_curtain(canvas: &mut Canvas, house: House, plain: Rgba) {
    match (house.mark, house.occupied) {
        (Some(mark), false) => curtain(canvas, house, mark),
        (Some(mark), true) => drawn_curtain(canvas, house, mark),
        // A house with nobody's colours still shows when somebody is in.
        (None, true) => drawn_curtain(
            canvas,
            house,
            super::ResidentMark {
                cloth: mix(plain, WHITE, 0.4),
                fold: plain,
                tie: plain,
                tied_left: true,
            },
        ),
        (None, false) => {}
    }
}

/// The curtain drawn right across the doorway: the resident's cloth wherever the doorway shows,
/// hanging in folds, parted a little in the middle, with lamplight showing along the bottom.
fn drawn_curtain(canvas: &mut Canvas, house: House, mark: super::ResidentMark) {
    let (door_width, door_height) = house.door();
    let (cx, bottom) = (house.cx, house.bottom);
    let rx = door_width / 2 + 1;
    let top = bottom - door_height - 1;
    let inside = |pixel: Rgba| pixel == DOORWAY || pixel == GLOW || pixel == GLOW_CORE;
    for y in top..bottom - 1 {
        for x in cx - rx..=cx + rx {
            if !inside(canvas.get(x, y)) {
                continue;
            }
            // A fold every third column, counted out from the parting in the middle where the
            // two halves meet.
            let color = if (x - cx).rem_euclid(3) == 0 {
                mark.fold
            } else {
                mark.cloth
            };
            canvas.set(x, y, color);
        }
    }
    // Lamplight under the hem, and between the halves near the floor.
    for x in cx - rx..=cx + rx {
        let y = bottom - 2;
        let pixel = canvas.get(x, y);
        if pixel == mark.cloth || pixel == mark.fold {
            canvas.set(x, y, GLOW);
        }
    }
    let parting = bottom - 3;
    if canvas.get(cx, parting) == mark.fold {
        canvas.set(cx, parting, GLOW_CORE);
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
    // Somebody at home lights the lamp whatever the hour, so a window shows it from outside.
    blob(canvas, x - 1, y - 1, size + 2, frame);
    blob(canvas, x, y, size, if lit { GLOW } else { GLASS });
    if lit {
        canvas.set(x + size / 2, y + size / 2, GLOW_CORE);
    } else {
        canvas.set(x + 1, y + 1, mix(GLASS, WHITE, 0.55));
    }
}

/// A tent pitched from triangles: a canvas cut into triangular panels meeting at the peak, lit on
/// the left and shaded on the right, a triangular door with its flap tied back, a triangle of a
/// pennant flying from the pole, and guy ropes out to pegs either side.
fn tent(canvas: &mut Canvas, house: House, m: Materials) {
    let (top, left, right, bottom) = (house.top(), house.left(), house.right(), house.bottom);
    let u = |value| house.unit(value);
    let base = bottom - 1;
    let apex = (house.cx, top + u(3));
    let (foot_left, foot_right) = (left - u(1), right + u(1));
    let height = (base - apex.1).max(1) as f32;
    let across = move |y: i32, foot: i32| {
        let t = ((y - apex.1) as f32 / height).clamp(0.0, 1.0);
        (apex.0 as f32 + (foot - apex.0) as f32 * t).round() as i32
    };
    // Guy ropes, drawn first so the canvas sits over their knots: from halfway down each side out
    // to a peg in the ground.
    let middle = apex.1 + (base - apex.1) / 2;
    for (side, foot) in [(-1, foot_left), (1, foot_right)] {
        let knot = (across(middle, foot), middle);
        let peg = (foot + side * u(4), bottom - 1);
        canvas.line(knot.0, knot.1, peg.0, peg.1, 1, m.wall.light);
        canvas.fill_rect(peg.0 - 1, peg.1 - 2, 2, 3, m.outline);
        canvas.set(peg.0 - 1 + i32::from(side < 0), peg.1 - 2, m.wall.base);
    }
    // The canvas: one triangle, cut into four panels by seams running down from the peak.
    fill_rows(
        canvas,
        apex.1,
        base,
        move |y| (across(y, foot_left), across(y, foot_right)),
        m.roof.base,
        m.outline,
    );
    let seams = [
        foot_left + (foot_right - foot_left) / 4,
        house.cx,
        foot_left + (foot_right - foot_left) * 3 / 4,
    ];
    for y in apex.1 + 1..=base {
        let (row_left, row_right) = (across(y, foot_left), across(y, foot_right));
        let cuts = seams.map(|foot| across(y, foot));
        for x in row_left..=row_right {
            let panel = cuts.iter().filter(|cut| x > **cut).count();
            let tone = match panel {
                0 => m.roof.light,
                1 => m.roof.base,
                2 => mix(m.roof.base, m.roof.shade, 0.45),
                _ => m.roof.shade,
            };
            canvas.set(x, y, tone);
        }
        for cut in cuts {
            if cut > row_left && cut < row_right {
                canvas.set(cut, y, mix(m.roof.shade, m.outline, 0.3));
            }
        }
    }
    // A hem of the lit colour along the foot.
    canvas.fill_rect(
        across(base, foot_left) + 1,
        base,
        (across(base, foot_right) - across(base, foot_left) - 1).max(1),
        1,
        m.roof.shade,
    );
    // The pole through the peak, and a pennant flying from it.
    canvas.line(apex.0, apex.1, apex.0, apex.1 - u(3), 1, m.outline);
    let flag_top = apex.1 - u(3);
    for row in 0..u(3).max(2) {
        let reach = (u(3).max(2) - row).max(1);
        canvas.fill_rect(apex.0 + 1, flag_top + row, reach, 1, m.trim.base);
    }
    canvas.set(apex.0 + 1, flag_top, m.trim.light);
    triangle_doorway(canvas, house, m);
}

/// A tent's door: a triangle opening at the middle of the foot, as wide at the bottom as any other
/// door, the flap folded back beside it and tied, and a straw floor across the threshold.
fn triangle_doorway(canvas: &mut Canvas, house: House, m: Materials) {
    let (door_width, door_height) = house.door();
    let (cx, bottom) = (house.cx, house.bottom);
    let half = door_width / 2;
    let top = bottom - door_height;
    let edge = move |y: i32| {
        let t = ((y - top) as f32 / door_height.max(1) as f32).clamp(0.0, 1.0);
        (half as f32 * t).round() as i32
    };
    // The rim first, then the dark inside, then lamplight after dark.
    for y in top - 1..bottom {
        let reach = edge(y.max(top)) + 1;
        canvas.fill_rect(cx - reach, y, reach * 2 + 1, 1, m.outline);
    }
    for y in top..bottom - 1 {
        let reach = edge(y);
        canvas.fill_rect(cx - reach, y, reach * 2 + 1, 1, DOORWAY);
        if house.lit && y > top + 1 {
            let inner = (reach - 1).max(0);
            canvas.fill_rect(cx - inner, y, inner * 2 + 1, 1, GLOW);
            if y > bottom - door_height / 2 {
                canvas.fill_rect(cx - inner / 2, y, inner + 1, 1, GLOW_CORE);
            }
        }
    }
    // The flap, folded back to the right side of the door and tied there.
    let flap_x = cx + half + 1;
    for row in 0..door_height - 1 {
        let reach = ((door_height - 1 - row) / 3).clamp(0, house.unit(3));
        canvas.fill_rect(flap_x, top + row, reach + 1, 1, m.trim.base);
        canvas.set(flap_x, top + row, m.trim.light);
    }
    canvas.set(flap_x + 1, top + door_height / 2, m.outline);
    canvas.fill_rect(cx - half - 1, bottom - 1, door_width + 2, 1, m.second.base);
    hang_curtain(canvas, house, m.trim.base);
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
            house.lit || house.occupied,
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

/// A house made of pillows: one big plump cushion standing on the ground for the walls, its sides
/// bowed out and its corners pinched to soft points, piped round its edge and buttoned with the
/// fabric drawn in round each button; and a flatter pillow lying across the top for a roof, wider
/// than the walls, with a tassel at each corner. The doorway is let into the front of the big one.
fn pillow_fort(canvas: &mut Canvas, house: House, m: Materials) {
    let (top, bottom) = (house.top(), house.bottom);
    let u = |value| house.unit(value);
    let half = house.width / 2;
    // The roof pillow takes the top third or so; the wall pillow the rest, down to the ground.
    let roof_bottom = top + house.height * 3 / 10 + u(1);
    let body_top = roof_bottom - u(2);
    // The wall pillow: sides bowing out towards the middle, its top edge puffed up in the middle
    // and dipping to a point at each corner, flat where it sits on the ground.
    let body = move |y: i32| {
        let t = ((y - body_top) as f32 / (bottom - 1 - body_top).max(1) as f32).clamp(0.0, 1.0);
        let bulge = ((t * std::f32::consts::PI).sin() * 2.2).round() as i32;
        (house.cx - half - bulge, house.cx + half + bulge)
    };
    let body_top_at = move |x: i32| {
        let s = ((x - (house.cx - half)) as f32 / (half * 2).max(1) as f32).clamp(0.0, 1.0);
        body_top - ((s * std::f32::consts::PI).sin() * 1.5).round() as i32
    };
    pillow(
        canvas,
        body,
        body_top_at,
        body_top - 2,
        bottom - 1,
        m.second,
        m.trim,
        m.outline,
    );
    // Two buttons across the upper half of the wall, the fabric drawn in round each.
    let button_y = body_top + (bottom - body_top) / 3;
    for x in [house.cx - half / 2 - 1, house.cx + half / 2 + 1] {
        tuft(canvas, x, button_y, m.second, m.outline);
    }
    // The roof pillow: flatter and wider than the walls, lying across them.
    let overhang = u(3);
    let roof_half = half + overhang;
    let roof_top = top;
    let roof = move |y: i32| {
        let t = ((y - roof_top) as f32 / (roof_bottom - roof_top).max(1) as f32).clamp(0.0, 1.0);
        let bulge = ((t * std::f32::consts::PI).sin() * 2.0).round() as i32;
        (house.cx - roof_half - bulge, house.cx + roof_half + bulge)
    };
    let roof_top_at = move |x: i32| {
        let s =
            ((x - (house.cx - roof_half)) as f32 / (roof_half * 2).max(1) as f32).clamp(0.0, 1.0);
        roof_top - ((s * std::f32::consts::PI).sin() * 2.5).round() as i32
    };
    pillow(
        canvas,
        roof,
        roof_top_at,
        roof_top - 3,
        roof_bottom,
        m.roof,
        m.trim,
        m.outline,
    );
    tuft(
        canvas,
        house.cx,
        (roof_top + roof_bottom) / 2,
        m.roof,
        m.outline,
    );
    // A tassel hanging from each corner of the roof pillow.
    for (x, lean) in [
        (house.cx - roof_half - 1, -1),
        (house.cx + roof_half + 1, 1),
    ] {
        canvas.set(x, roof_bottom, m.outline);
        canvas.set(x + lean, roof_bottom + 1, m.trim.base);
        canvas.set(x + lean, roof_bottom + 2, m.trim.shade);
    }
    // A round window let into the wall on the left of the door, and the door itself.
    let (door_width, door_height) = house.door();
    let size = if house.span >= super::MAIN_SPAN { 4 } else { 3 };
    let window_x = house.cx - door_width / 2 - u(3) - size;
    let window_y = bottom - door_height - u(1);
    if window_x - 1 > house.cx - half + 1 && window_y > button_y + u(2) {
        round_window(
            canvas,
            window_x,
            window_y,
            size,
            m.outline,
            house.lit || house.occupied,
        );
    }
    doorway(canvas, house, m.outline, m.second.shade);
    // A pillow glimpsed on the floor inside.
    let rx = door_width / 2;
    canvas.fill_rect(house.cx - rx + 1, bottom - u(3), rx, u(2), CREAM);
    // A little square cushion on the doorstep, in the roof's fabric.
    if house.span >= super::COTTAGE_SPAN {
        let cushion = u(4);
        let x = house.cx + door_width / 2 + u(2);
        if x + cushion <= house.cx + half + 1 {
            square_cushion(
                canvas,
                x,
                bottom - cushion + 1,
                cushion,
                m.roof,
                m.trim,
                m.outline,
            );
        }
    }
}

/// One plump pillow, filled row by row between `edges` from its puffed top (`top_at`, no higher
/// than `ceiling`) down to `floor`, outlined, lit along the top and left and shaded down the
/// right and along the bottom, with piping running just inside its top edge.
#[allow(clippy::too_many_arguments)]
fn pillow(
    canvas: &mut Canvas,
    edges: impl Fn(i32) -> (i32, i32),
    top_at: impl Fn(i32) -> i32,
    ceiling: i32,
    floor: i32,
    fabric: Tone,
    piping: Tone,
    outline: Rgba,
) {
    let (widest_left, widest_right) = (ceiling..=floor)
        .map(&edges)
        .fold((i32::MAX, i32::MIN), |(l, r), (a, b)| (l.min(a), r.max(b)));
    for x in widest_left - 1..=widest_right + 1 {
        let top = top_at(x.clamp(widest_left, widest_right));
        for y in (top - 1).max(ceiling)..=floor + 1 {
            let (left, right) = edges(y.clamp(top, floor));
            if x >= left - 1 && x <= right + 1 {
                canvas.set(x, y, outline);
            }
        }
    }
    for y in ceiling..=floor {
        let (left, right) = edges(y);
        for x in left..=right {
            if y < top_at(x) {
                continue;
            }
            let from_top = y - top_at(x);
            let color = if x >= right - 1 || y >= floor - 1 {
                fabric.shade
            } else if x <= left + 1 || from_top <= 1 {
                fabric.light
            } else {
                fabric.base
            };
            canvas.set(x, y, color);
        }
    }
    // The piping, one row in from the top edge and stopping short of the corners.
    for x in widest_left + 3..=widest_right - 3 {
        let y = top_at(x) + 2;
        if y <= floor - 2 {
            canvas.set(x, y, piping.base);
        }
    }
    // A soft sheen where it swells nearest the light, up and to the left.
    let sheen_x = widest_left + (widest_right - widest_left) / 4;
    let sheen_y = top_at(sheen_x) + 4;
    if sheen_y + 1 < floor - 2 {
        for (dx, dy) in [(0, 0), (1, 0), (2, 0), (0, 1), (1, 1)] {
            canvas.set(sheen_x + dx, sheen_y + dy, fabric.light);
        }
    }
}

/// A button sewn through a cushion, the fabric drawn in round it in four little creases.
fn tuft(canvas: &mut Canvas, x: i32, y: i32, fabric: Tone, outline: Rgba) {
    for (dx, dy) in [(-2, -1), (2, -1), (-2, 1), (2, 1)] {
        canvas.set(x + dx, y + dy, fabric.shade);
    }
    canvas.set(x - 1, y, fabric.shade);
    canvas.set(x + 1, y, fabric.shade);
    canvas.set(x, y, outline);
    canvas.set(x, y - 1, fabric.light);
}

/// One square cushion, plumped: rounded off at the corners, lit along its top and left edges and
/// shaded along its bottom and right, piped across the top, and buttoned in its middle with the
/// fabric drawn in round the button.
fn square_cushion(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    size: i32,
    fabric: Tone,
    piping: Tone,
    outline: Rgba,
) {
    let size = size.max(4);
    canvas.fill_rect(x, y + 1, size, size - 2, outline);
    canvas.fill_rect(x + 1, y, size - 2, size, outline);
    canvas.fill_rect(x + 1, y + 1, size - 2, size - 2, fabric.base);
    canvas.fill_rect(x + 1, y + 1, size - 2, 1, piping.base);
    canvas.fill_rect(x + 1, y + 2, 1, size - 4, fabric.light);
    canvas.fill_rect(x + 2, y + size - 2, size - 3, 1, fabric.shade);
    canvas.fill_rect(x + size - 2, y + 2, 1, size - 4, fabric.shade);
    let (bx, by) = (x + size / 2, y + size / 2);
    canvas.set(bx, by, outline);
    if size >= 7 {
        canvas.set(bx - 1, by - 1, fabric.shade);
        canvas.set(bx + 1, by + 1, fabric.light);
    }
}

/// A cottage roofed in leaves: a card front wall and a side wall folded back into shade, under a
/// steep roof thatched with rows of overlapping leaves, each hanging point-down like a shingle and
/// veined down its middle, their tips scalloping the eave. A sprout curls up off the ridge, a vine
/// climbs the fold between the walls, and a window and a mat sit by the door.
fn leaf_house(canvas: &mut Canvas, house: House, m: Materials) {
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
    canvas.line(
        right - side,
        eave,
        right - side,
        bottom - 2,
        1,
        m.wall.shade,
    );
    // The roof: its outline and the shadow between the leaves first, then the leaves in rows
    // from the eave up, so each row overlaps the tops of the row below and leaves its tips
    // showing, the near slope lit and the far one in shade.
    let peak = (house.cx - u(1), top);
    let slope = move |y: i32| {
        let t = ((y - peak.1) as f32 / (eave - peak.1).max(1) as f32).clamp(0.0, 1.0);
        let reach = (t * (house.width as f32 / 2.0 + 3.0)).round() as i32;
        (peak.0 - reach, peak.0 + reach)
    };
    fill_rows(canvas, peak.1, eave, slope, m.roof.shade, m.outline);
    let size = u(7).max(5) | 1;
    let step = (size - 3).max(2);
    let mut rows = Vec::new();
    let mut y = eave - size + 2;
    while y > peak.1 {
        rows.push(y);
        y -= step;
    }
    for (index, row) in rows.iter().enumerate() {
        let offset = if index % 2 == 0 { 0 } else { size / 2 };
        let (row_left, row_right) = slope(*row + size / 2);
        let mut x = peak.0 - offset - size * ((peak.0 - row_left) / size + 1);
        while x <= row_right {
            // Lit on the near slope and shaded on the far one, and no two neighbours quite the
            // same, so every leaf reads on its own.
            let lit = x + size / 2 < peak.0;
            let turn = ((x - peak.0).div_euclid(size) + index as i32) % 2 == 0;
            let (fill, vein) = match (lit, turn) {
                (true, true) => (m.roof.light, m.roof.base),
                (true, false) => (mix(m.roof.light, m.roof.base, 0.5), m.roof.base),
                (false, true) => (m.roof.base, m.roof.shade),
                (false, false) => (mix(m.roof.base, m.roof.shade, 0.4), m.roof.shade),
            };
            shingle(
                canvas,
                x,
                *row,
                size,
                fill,
                vein,
                mix(m.roof.shade, m.outline, 0.35),
                slope,
                row_left,
                row_right,
                eave,
            );
            x += size;
        }
    }
    // The ridge, where the two slopes meet.
    for y in peak.1 + 1..eave - size / 2 {
        if canvas.get(peak.0, y) != m.outline {
            canvas.set(peak.0, y, mix(m.roof.base, m.roof.shade, 0.5));
        }
    }
    canvas.fill_rect(left + 1, eave + 2, house.width - side - 1, 1, m.wall.shade);
    // A sprout curling up off the ridge.
    canvas.line(
        peak.0,
        peak.1 - 1,
        peak.0,
        peak.1 - u(2),
        1,
        mix(LEAF, m.outline, 0.35),
    );
    canvas.set(peak.0 - 1, peak.1 - u(2), LEAF);
    canvas.set(peak.0 + 1, peak.1 - u(2) - 1, mix(LEAF, WHITE, 0.3));
    canvas.set(peak.0 + 2, peak.1 - u(2), LEAF);
    // A vine climbing the fold between the two walls.
    let vine = right - side;
    let mut y = bottom - 2;
    let mut turn = 0;
    while y > eave + u(3) {
        let leaf = if turn % 2 == 0 { vine - 1 } else { vine + 1 };
        canvas.set(vine, y, mix(LEAF, m.outline, 0.35));
        canvas.set(
            leaf,
            y - 1,
            if turn % 2 == 0 {
                LEAF
            } else {
                mix(LEAF, WHITE, 0.3)
            },
        );
        y -= 2;
        turn += 1;
    }
    // A cut-out window with its panes drawn in.
    let (door_width, door_height) = house.door();
    let window_x = left + (house.cx - door_width / 2 - left) / 2 - u(2);
    let window_y = bottom - door_height - u(2);
    if house.span >= super::COTTAGE_SPAN {
        canvas.fill_rect(window_x - 1, window_y - 1, u(6), u(6), m.outline);
        let glowing = house.lit || house.occupied;
        canvas.fill_rect(
            window_x,
            window_y,
            u(4),
            u(4),
            if glowing { GLOW } else { GLASS },
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
        if !glowing {
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

/// One leaf of a leaf roof, hanging point-down from `(x, y)`: round-shouldered at the top,
/// narrowing to a point, veined down the middle and edged in shadow where it tapers, and kept
/// inside the roof except where the lowest row's tips hang over the eave.
#[allow(clippy::too_many_arguments)]
fn shingle(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    size: i32,
    fill: Rgba,
    vein: Rgba,
    edge: Rgba,
    slope: impl Fn(i32) -> (i32, i32),
    row_left: i32,
    row_right: i32,
    eave: i32,
) {
    let half = size / 2;
    let middle = x + half;
    for row in 0..size {
        let reach = match row {
            0 => (half - 1).max(0),
            r if r < size - 3 => half,
            r if r == size - 3 => (half * 2 / 3).max(1),
            r if r == size - 2 => (half / 3).max(1),
            _ => 0,
        };
        let py = y + row;
        let (inside_left, inside_right) = if py <= eave {
            let (l, r) = slope(py);
            (l + 1, r - 1)
        } else {
            (row_left + 1, row_right - 1)
        };
        let paint = |canvas: &mut Canvas, px: i32, color: Rgba| {
            if px >= inside_left && px <= inside_right {
                canvas.set(px, py, color);
            }
        };
        for px in middle - reach..=middle + reach {
            let color = if px == middle && row > 0 && row < size - 1 {
                vein
            } else {
                fill
            };
            paint(canvas, px, color);
        }
        // The taper, edged so each point reads against the leaf behind it.
        if row >= size - 3 {
            paint(canvas, middle - reach - 1, edge);
            paint(canvas, middle + reach + 1, edge);
        }
        if row == size - 1 {
            paint(canvas, middle, edge);
        }
    }
}
