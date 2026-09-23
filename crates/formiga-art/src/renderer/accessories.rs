//! What a companion wears, drawn onto every frame of its body as its atlas is baked, so a hat
//! walks, sleeps, climbs and dances with it and costs nothing to show.
//!
//! Every piece is placed from the `Figure` its body reported for that frame — the top of the head,
//! the neck, the chest, the hip — so the same leaf hat sits on a round head, a long one, and a
//! blob with its face on its front, at a mini's size as well as an adult's. Frames are drawn facing
//! right, like everything else in the atlas; the overlay mirrors the whole frame for the other way.
//!
//! Each piece is coloured in the inks of the find it was made from, as the colony's own trinket
//! sheet colours it, so a leaf hat is the colour of the leaf the colony found, and it stands apart
//! from every coat in the colony for the same reason the keepsake does.

use super::*;
use crate::trinkets::{TrinketAtlasRenderer, TrinketInk};
use formiga_core::{Accessory, AccessoryKind};

/// An accessory and the inks it is drawn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccessoryArt {
    pub accessory: Accessory,
    pub ink: TrinketInk,
}

impl AccessoryArt {
    /// The accessory in the colours of the find it is made from, as this colony's trinket sheet
    /// draws it.
    pub fn resolve(accessory: Accessory, colony_seed: [u8; 32], members: &[Palette]) -> Self {
        let variant = match accessory {
            Accessory::Worn(kind) => kind.made_from(),
            Accessory::Pin(variant) => variant,
        };
        Self {
            accessory,
            ink: TrinketAtlasRenderer::ink(colony_seed, members, variant),
        }
    }
}

/// Petals, pearls and pompoms: white whatever they are made from.
const WHITE: Rgba = Rgba::new(250, 246, 236, 255);
/// The middle of a flower.
const POLLEN: Rgba = Rgba::new(255, 216, 96, 255);
/// A leaf that is a leaf.
const LEAF: Rgba = Rgba::new(96, 164, 84, 255);
const LEAF_DARK: Rgba = Rgba::new(58, 112, 62, 255);

pub(super) fn draw_accessory(canvas: &mut Canvas, art: AccessoryArt, figure: Figure) {
    let mut worn = Canvas::new(canvas.width(), canvas.height());
    let ink = art.ink;
    match art.accessory {
        Accessory::Pin(variant) => pin(&mut worn, ink, variant, figure),
        Accessory::Worn(kind) => match kind {
            AccessoryKind::LeafHat => leaf_hat(&mut worn, ink, figure),
            AccessoryKind::FlowerCrown => flower_crown(&mut worn, ink, figure),
            AccessoryKind::AcornCap => acorn_cap(&mut worn, ink, figure),
            AccessoryKind::FeatherCap => feather_cap(&mut worn, ink, figure),
            AccessoryKind::MushroomCap => mushroom_cap(&mut worn, ink, figure),
            AccessoryKind::RibbonBow => ribbon_bow(&mut worn, ink, figure),
            AccessoryKind::PartyHat => party_hat(&mut worn, ink, figure),
            AccessoryKind::Sprout => sprout(&mut worn, ink, figure),
            AccessoryKind::StarClip => star_clip(&mut worn, ink, figure),
            AccessoryKind::MoonClip => moon_clip(&mut worn, ink, figure),
            AccessoryKind::CloudEarmuffs => earmuffs(&mut worn, ink, figure),
            AccessoryKind::Nightcap => nightcap(&mut worn, ink, figure),
            AccessoryKind::KnittedScarf => scarf(&mut worn, ink, figure),
            AccessoryKind::Bandana => bandana(&mut worn, ink, figure),
            AccessoryKind::BellCollar => bell_collar(&mut worn, ink, figure),
            AccessoryKind::ShellNecklace => shell_necklace(&mut worn, ink, figure),
            AccessoryKind::PetalRuff => petal_ruff(&mut worn, ink, figure),
            AccessoryKind::FriendshipCord => friendship_cord(&mut worn, ink, figure),
            AccessoryKind::TinySatchel => satchel(&mut worn, ink, figure),
            AccessoryKind::SnailPack => snail_pack(&mut worn, ink, figure),
        },
    }
    // Laid over the body, but never through the ground it stands on, never on the frame's own
    // edge, and never where the body shows a pixel through it a scarf or strap is behind.
    let (width, height) = (canvas.width() as i32, canvas.height() as i32);
    for y in 1..(height - 1).min(figure.floor + 1) {
        for x in 1..width - 1 {
            let pixel = worn.get(x, y);
            if pixel.a > 0 {
                canvas.set(x, y, pixel);
            }
        }
    }
}

/// The top half of an ellipse standing on a flat base: a cap, a dome, a hat's crown. Outlined all
/// round except along the base, which sits on the head.
fn dome(
    canvas: &mut Canvas,
    cx: i32,
    base: i32,
    half: i32,
    height: i32,
    fill: Rgba,
    outline: Rgba,
) {
    let height = height.max(1);
    for y in base - height..=base {
        let t = (base - y) as f32 / height as f32;
        let reach = (half as f32 * (1.0 - t * t).max(0.0).sqrt()).round() as i32;
        canvas.fill_rect(cx - reach - 1, y, reach * 2 + 3, 1, outline);
    }
    canvas.fill_rect(cx - 1, base - height - 1, 3, 1, outline);
    for y in base - height + 1..=base {
        let t = (base - y) as f32 / height as f32;
        let reach = (half as f32 * (1.0 - t * t).max(0.0).sqrt()).round() as i32 - 1;
        if reach >= 0 {
            canvas.fill_rect(cx - reach, y, reach * 2 + 1, 1, fill);
        }
    }
}

/// A band across the body `thickness` rows tall and `half` either side of `x`, outlined.
fn band(canvas: &mut Canvas, x: i32, y: i32, half: i32, thickness: i32, fill: Rgba, outline: Rgba) {
    canvas.fill_rect(x - half - 1, y - 1, half * 2 + 3, thickness + 2, outline);
    canvas.fill_rect(x - half, y, half * 2 + 1, thickness, fill);
}

/// A flower seen face on, three pixels across, outlined.
fn flower(canvas: &mut Canvas, x: i32, y: i32, petals: Rgba, outline: Rgba) {
    canvas.fill_circle(x, y, 2, outline);
    for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
        canvas.set(x + dx, y + dy, petals);
    }
    canvas.set(x, y, POLLEN);
}

// ---------------------------------------------------------------------------------------------
// On the head.
// ---------------------------------------------------------------------------------------------

/// A broad leaf laid across the top of the head, its stalk curling up at the back.
fn leaf_hat(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let half = (figure.head_half + 1).clamp(4, 10);
    let (cx, top) = (figure.crown.x, figure.crown.y);
    canvas.fill_ellipse(cx, top - 1, half + 1, 3, ink.outline);
    canvas.fill_ellipse(cx, top - 1, half, 2, ink.body);
    canvas.fill_rect(cx - half + 2, top - 2, half, 1, ink.light);
    canvas.line(cx - half + 1, top, cx + half - 1, top - 2, 1, ink.deep);
    canvas.set(cx - half - 2, top - 1, ink.outline);
    canvas.set(cx - half - 2, top - 2, ink.deep);
    canvas.set(cx - half - 1, top - 3, ink.deep);
}

/// Three or four small flowers across the top of the head, their petals in turn white and the
/// colour of the daisy the crown was made from.
fn flower_crown(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let half = figure.head_half.clamp(3, 9);
    let (cx, top) = (figure.crown.x, figure.crown.y);
    let spots: &[i32] = if half >= 6 {
        &[-3, -1, 1, 3]
    } else {
        &[-2, 0, 2]
    };
    let step = (half as f32 / 3.0).max(1.4);
    // A band of green they are woven onto.
    canvas.line(cx - half, top + 1, cx + half, top + 1, 1, LEAF_DARK);
    for (index, spot) in spots.iter().enumerate() {
        let x = cx + (*spot as f32 * step).round() as i32;
        let dip = i32::from(spot.abs() >= 3);
        let petals = if index % 2 == 0 { WHITE } else { ink.light };
        flower(canvas, x, top - 1 + dip, petals, ink.outline);
    }
    canvas.set(cx - half - 1, top + 1, LEAF);
    canvas.set(cx + half + 1, top + 1, LEAF);
}

/// The cap off an acorn, worn like a beret: a scaly dome with a little stalk on top.
fn acorn_cap(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let half = (figure.head_half - 1).clamp(3, 8);
    let (cx, top) = (figure.crown.x, figure.crown.y + 1);
    dome(canvas, cx, top, half, 4, ink.deep, ink.outline);
    // Scales in rows, offset like a real cup's.
    for row in 0..3 {
        let y = top - 1 - row;
        let offset = row % 2;
        for x in (cx - half + 1 + offset..=cx + half - 1).step_by(2) {
            if canvas.get(x, y) == ink.deep {
                canvas.set(x, y, ink.body);
            }
        }
    }
    canvas.set(cx - half + 1, top - 3, ink.light);
    canvas.fill_rect(cx, top - 6, 1, 2, ink.outline);
    canvas.set(cx + 1, top - 6, ink.outline);
}

/// A snug cap with one long feather tucked into it, sweeping up and back.
fn feather_cap(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let half = (figure.head_half - 1).clamp(3, 8);
    let (cx, top) = (figure.crown.x, figure.crown.y + 1);
    dome(canvas, cx, top, half, 2, ink.deep, ink.outline);
    canvas.set(cx - half + 1, top - 1, ink.body);
    // The feather: a vane of the find's own colour off a dark quill.
    let (fx, fy) = (cx - half / 2, top - 2);
    canvas.line(fx, fy, fx - 3, fy - 6, 2, ink.outline);
    canvas.line(fx, fy, fx - 3, fy - 6, 1, ink.body);
    canvas.set(fx - 1, fy - 3, ink.light);
    canvas.set(fx - 2, fy - 5, ink.light);
    canvas.set(fx - 3, fy - 7, ink.outline);
}

/// A toadstool worn as a hat: a tall round cap in the find's colour with white spots.
fn mushroom_cap(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let half = (figure.head_half + 1).clamp(4, 10);
    let (cx, top) = (figure.crown.x, figure.crown.y + 1);
    dome(canvas, cx, top, half, 5, ink.body, ink.outline);
    canvas.fill_rect(cx - half + 1, top, half * 2 - 1, 1, ink.deep);
    for (dx, dy, size) in [(-half / 2, -3, 1), (half / 3, -4, 0), (half - 2, -2, 0)] {
        canvas.fill_circle(cx + dx, top + dy, size, WHITE);
    }
    canvas.set(cx - 1, top - 5, ink.light);
}

/// A bow tied at the back of the head.
fn ribbon_bow(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let (x, y) = (
        figure.crown.x - figure.head_half / 2 - 1,
        figure.crown.y + 1,
    );
    for side in [-1, 1] {
        let (lx, ly) = (x + side * 2, y - 1);
        canvas.fill_ellipse(lx, ly, 2, 2, ink.outline);
        canvas.fill_ellipse(lx, ly, 1, 1, ink.body);
        canvas.set(lx - side, ly - 1, ink.light);
    }
    canvas.fill_rect(x - 1, y - 2, 3, 3, ink.outline);
    canvas.set(x, y - 1, ink.accent);
    canvas.set(x - 1, y + 2, ink.body);
    canvas.set(x + 1, y + 2, ink.body);
}

/// A striped paper cone with a pompom, worn a little to the back.
fn party_hat(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let half = (figure.head_half / 2 + 1).clamp(3, 5);
    let (cx, base) = (figure.crown.x - 1, figure.crown.y + 1);
    let height = half * 2 + 2;
    for row in 0..=height {
        let reach = half * (height - row) / height;
        let y = base - row;
        canvas.fill_rect(cx - reach - 1, y, reach * 2 + 3, 1, ink.outline);
        if reach > 0 {
            let color = if (row / 2) % 2 == 0 {
                ink.body
            } else {
                ink.light
            };
            canvas.fill_rect(cx - reach, y, reach * 2 + 1, 1, color);
        }
    }
    canvas.fill_circle(cx, base - height - 1, 1, WHITE);
    canvas.set(cx, base - height - 2, ink.outline);
}

/// A seedling growing straight out of the top of the head.
fn sprout(canvas: &mut Canvas, _ink: TrinketInk, figure: Figure) {
    let (x, y) = (figure.crown.x, figure.crown.y);
    canvas.line(x, y, x, y - 4, 1, LEAF_DARK);
    for (side, color) in [(-1, LEAF), (1, LEAF_DARK)] {
        canvas.fill_ellipse(x + side * 2, y - 4 - i32::from(side > 0), 2, 1, color);
        canvas.set(x + side * 3, y - 5 - i32::from(side > 0), LEAF_DARK);
    }
    canvas.set(x - 2, y - 5, WHITE);
}

/// A small star clipped on at the front of the head.
fn star_clip(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let (x, y) = (
        figure.crown.x + figure.head_half / 2 + 1,
        figure.crown.y + 1,
    );
    for (dx, dy) in [(0, -2), (-2, 0), (2, 0), (-1, 2), (1, 2)] {
        canvas.line(x, y, x + dx, y + dy, 1, ink.outline);
    }
    for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
        canvas.set(x + dx, y + dy, ink.body);
    }
    canvas.set(x, y, ink.light);
}

/// A crescent moon clipped on at the front of the head.
fn moon_clip(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let (x, y) = (figure.crown.x + figure.head_half / 2 + 1, figure.crown.y);
    canvas.fill_circle(x, y, 4, ink.outline);
    canvas.fill_circle(x, y, 3, ink.light);
    canvas.fill_circle(x + 2, y - 2, 3, Rgba::TRANSPARENT);
    // Put the outline back along the inside of the curve.
    for (dx, dy) in [(-1, -2), (0, -1), (0, 0), (-1, 1), (1, 1)] {
        if canvas.get(x + dx, y + dy) == Rgba::TRANSPARENT {
            continue;
        }
        if canvas.get(x + dx + 1, y + dy - 1) == Rgba::TRANSPARENT {
            canvas.set(x + dx, y + dy, ink.outline);
        }
    }
    canvas.set(x - 2, y, WHITE);
    canvas.set(x - 1, y + 2, ink.body);
}

/// Two puffs of cloud over the ears on a band across the top of the head.
fn earmuffs(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let half = figure.head_half.clamp(4, 10);
    let (cx, top) = (figure.crown.x, figure.crown.y);
    for x in cx - half + 1..=cx + half - 1 {
        let t = (x - cx) as f32 / half as f32;
        let lift = ((1.0 - t * t) * 2.0).round() as i32;
        canvas.set(x, top - lift, ink.outline);
    }
    for side in [-1, 1] {
        let (x, y) = (cx + side * half, top + 3);
        canvas.fill_circle(x, y, 3, ink.outline);
        canvas.fill_circle(x, y, 2, WHITE);
        canvas.set(x - 1, y - 1, ink.light);
        canvas.set(x + 1, y + 1, ink.light);
    }
}

/// A floppy nightcap: a tall soft cone flopped over to the back, its tip hanging down beside the
/// head with a pompom on the end.
fn nightcap(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let half = (figure.head_half - 1).clamp(3, 8);
    let (cx, base) = (figure.crown.x, figure.crown.y + 1);
    dome(canvas, cx, base, half, 4, ink.body, ink.outline);
    // The part that flops: from the top of the cap over to the back and down.
    let (top_x, top_y) = (cx - 1, base - 4);
    let (tip_x, tip_y) = (cx - half - 3, base + 1);
    canvas.line(top_x, top_y, tip_x, tip_y - 3, 3, ink.outline);
    canvas.line(tip_x, tip_y - 3, tip_x, tip_y, 3, ink.outline);
    canvas.line(top_x, top_y, tip_x, tip_y - 3, 1, ink.body);
    canvas.line(tip_x, tip_y - 3, tip_x, tip_y - 1, 1, ink.body);
    canvas.fill_circle(tip_x, tip_y + 1, 2, ink.outline);
    canvas.fill_circle(tip_x, tip_y + 1, 1, WHITE);
    // A soft white brim round the bottom.
    canvas.fill_rect(cx - half, base, half * 2 + 1, 1, WHITE);
    canvas.set(cx - half + 1, base - 2, ink.light);
}

// ---------------------------------------------------------------------------------------------
// Round the neck.
// ---------------------------------------------------------------------------------------------

/// A knitted scarf, striped, with one end hanging down the chest.
fn scarf(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let half = figure.neck_half.clamp(3, 10);
    let (x, y) = (figure.neck.x, figure.neck.y);
    band(canvas, x, y, half, 2, ink.body, ink.outline);
    for stripe in (x - half..=x + half).step_by(3) {
        canvas.set(stripe, y, ink.light);
        canvas.set(stripe, y + 1, ink.light);
    }
    // The hanging end, on the front.
    let (end_x, end_top) = (x + half / 2, y + 2);
    canvas.fill_rect(end_x - 1, end_top, 4, 5, ink.outline);
    canvas.fill_rect(end_x, end_top, 2, 4, ink.body);
    canvas.set(end_x, end_top + 2, ink.light);
    canvas.set(end_x, end_top + 4, ink.accent);
    canvas.set(end_x + 1, end_top + 4, ink.accent);
}

/// A kerchief knotted at the back with its point down the front.
fn bandana(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let half = figure.neck_half.clamp(3, 10);
    let (x, y) = (figure.neck.x, figure.neck.y);
    band(canvas, x, y, half, 1, ink.body, ink.outline);
    let (point_x, depth) = (x + 1, (half / 2 + 2).clamp(3, 5));
    for row in 0..depth {
        let reach = (half - 1) * (depth - row) / depth;
        canvas.fill_rect(
            point_x - reach - 1,
            y + 1 + row,
            reach * 2 + 3,
            1,
            ink.outline,
        );
        if reach > 0 {
            canvas.fill_rect(point_x - reach, y + 1 + row, reach * 2 + 1, 1, ink.body);
        }
    }
    canvas.set(point_x, y + depth, ink.outline);
    canvas.set(point_x - 1, y + 2, WHITE);
    canvas.set(point_x + 1, y + 3, WHITE);
    canvas.set(x - half - 1, y + 1, ink.body);
}

/// A thin collar with a small bell hanging from the front of it.
fn bell_collar(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let half = figure.neck_half.clamp(3, 10);
    let (x, y) = (figure.neck.x, figure.neck.y);
    band(canvas, x, y, half, 1, ink.deep, ink.outline);
    let bell_x = x + 1;
    canvas.fill_circle(bell_x, y + 3, 2, ink.outline);
    canvas.fill_circle(bell_x, y + 3, 1, ink.accent);
    canvas.set(bell_x - 1, y + 2, WHITE);
    canvas.set(bell_x, y + 4, ink.outline);
}

/// A cord round the neck with a shell hanging from it.
fn shell_necklace(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let half = figure.neck_half.clamp(3, 10);
    let (x, y) = (figure.neck.x, figure.neck.y);
    for dx in -half..=half {
        let sag = i32::from(dx.abs() < half / 2);
        canvas.set(x + dx, y + sag, ink.outline);
    }
    let (sx, sy) = (x + 1, y + 3);
    canvas.fill_rect(sx - 2, sy - 1, 5, 4, ink.outline);
    canvas.fill_rect(sx - 1, sy, 3, 2, WHITE);
    canvas.set(sx, sy, ink.light);
    canvas.set(sx - 1, sy + 1, ink.body);
    canvas.set(sx + 1, sy + 1, ink.body);
    canvas.set(sx, sy + 2, ink.outline);
}

/// A frill of petals all the way round the neck.
fn petal_ruff(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let half = figure.neck_half.clamp(3, 10) + 1;
    let (x, y) = (figure.neck.x, figure.neck.y);
    for dx in (-half..=half).step_by(2) {
        canvas.fill_circle(x + dx, y + 1, 1, ink.outline);
    }
    for dx in (-half..=half).step_by(2) {
        canvas.set(
            x + dx,
            y + 1,
            if (dx / 2) % 2 == 0 {
                ink.body
            } else {
                ink.light
            },
        );
        canvas.set(x + dx, y + 2, ink.outline);
    }
}

/// A cord of two colours twisted together, tied at the front.
fn friendship_cord(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let half = figure.neck_half.clamp(3, 10);
    let (x, y) = (figure.neck.x, figure.neck.y);
    canvas.fill_rect(x - half - 1, y - 1, half * 2 + 3, 3, ink.outline);
    for dx in -half..=half {
        let color = if dx.rem_euclid(2) == 0 {
            ink.body
        } else {
            ink.accent
        };
        canvas.set(x + dx, y, color);
    }
    canvas.fill_rect(x, y + 1, 2, 2, ink.outline);
    canvas.set(x, y + 3, ink.body);
    canvas.set(x + 1, y + 3, ink.accent);
}

// ---------------------------------------------------------------------------------------------
// Carried.
// ---------------------------------------------------------------------------------------------

/// A little bag at the back hip, on a strap from the far shoulder.
fn satchel(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let (bag_x, bag_y) = (figure.hip.x, figure.hip.y.min(figure.floor - 5));
    let shoulder = (figure.neck.x + figure.neck_half / 2, figure.neck.y + 1);
    canvas.line(shoulder.0, shoulder.1, bag_x + 2, bag_y, 1, ink.deep);
    canvas.fill_rect(bag_x - 2, bag_y, 6, 5, ink.outline);
    canvas.fill_rect(bag_x - 1, bag_y + 1, 4, 3, ink.body);
    canvas.fill_rect(bag_x - 1, bag_y + 1, 4, 1, ink.light);
    canvas.set(bag_x + 1, bag_y + 2, ink.accent);
}

/// A snail's shell carried on the back like a pack.
fn snail_pack(canvas: &mut Canvas, ink: TrinketInk, figure: Figure) {
    let (x, y) = (figure.back.x, figure.back.y + 1);
    canvas.fill_circle(x, y, 4, ink.outline);
    canvas.fill_circle(x, y, 3, ink.body);
    canvas.fill_circle(x, y, 2, ink.deep);
    canvas.fill_circle(x, y, 1, ink.body);
    canvas.set(x, y, ink.light);
    canvas.set(x - 2, y - 2, ink.light);
    canvas.set(x + 2, y + 3, ink.outline);
    let strap = (figure.neck.x, figure.neck.y + 1);
    canvas.line(x + 3, y - 1, strap.0, strap.1, 1, ink.deep);
}

// ---------------------------------------------------------------------------------------------
// Pinned on.
// ---------------------------------------------------------------------------------------------

/// A find worn as a pin: the keepsake's own drawing shrunk to half its size, each pixel the
/// colour most of its four were, and outlined again so it holds its shape on the chest.
fn pin(canvas: &mut Canvas, ink: TrinketInk, variant: u8, figure: Figure) {
    let small = shrunk_trinket(ink, variant);
    let (x0, y0) = (figure.chest.x - 5, figure.chest.y - 4);
    for y in 0..small.height() as i32 {
        for x in 0..small.width() as i32 {
            let pixel = small.get(x, y);
            if pixel.a > 0 {
                canvas.set(x0 + x, y0 + y, pixel);
            }
        }
    }
}

/// The keepsake drawing at half size, eight pixels square.
pub(crate) fn shrunk_trinket(ink: TrinketInk, variant: u8) -> Canvas {
    let mut full = Canvas::new(crate::TRINKET_CELL, crate::TRINKET_CELL);
    crate::draw_trinket(&mut full, ink, variant, crate::TRINKET_FRAME_REST, 0, 0);
    let mut small = Canvas::new(8, 8);
    for y in 0..8 {
        for x in 0..8 {
            // The colour most of the four agree on, leaving the old outline out of the vote so a
            // shape keeps its colour; two or more empty pixels and the new one is empty too.
            let quad = [
                full.get(x * 2, y * 2),
                full.get(x * 2 + 1, y * 2),
                full.get(x * 2, y * 2 + 1),
                full.get(x * 2 + 1, y * 2 + 1),
            ];
            let filled: Vec<Rgba> = quad.iter().copied().filter(|p| p.a > 0).collect();
            if filled.len() < 2 {
                continue;
            }
            let colored: Vec<Rgba> = filled
                .iter()
                .copied()
                .filter(|p| *p != ink.outline)
                .collect();
            let pool = if colored.is_empty() {
                &filled
            } else {
                &colored
            };
            let pick = pool
                .iter()
                .copied()
                .max_by_key(|candidate| pool.iter().filter(|p| **p == *candidate).count())
                .unwrap_or(ink.body);
            small.set(x, y, pick);
        }
    }
    // A fresh outline round the shrunk shape.
    let mut edges = Vec::new();
    for y in -1..9 {
        for x in -1..9 {
            if small.get(x, y).a > 0 {
                continue;
            }
            let touches = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .into_iter()
                .any(|(dx, dy)| {
                    small.get(x + dx, y + dy).a > 0 && small.get(x + dx, y + dy) != ink.outline
                });
            if touches {
                edges.push((x, y));
            }
        }
    }
    let mut outlined = Canvas::new(10, 10);
    for y in 0..8 {
        for x in 0..8 {
            outlined.set(x + 1, y + 1, small.get(x, y));
        }
    }
    for (x, y) in edges {
        outlined.set(x + 1, y + 1, ink.outline);
    }
    outlined
}
