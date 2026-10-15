//! What a companion wears, drawn onto every frame of its body as its atlas is baked, so a hat
//! walks, sleeps, climbs and dances with it and costs nothing to show.
//!
//! Every piece is placed from the `Figure` its body reported for that frame — the top of the head,
//! the neck, the chest, the hip — so the same leaf hat sits on a round head, a long one, and a
//! blob with its face on its front, at a mini's size as well as an adult's. Frames are drawn facing
//! right, like everything else in the atlas; the overlay mirrors the whole frame for the other way.
//!
//! A piece is drawn at the body's own pixel size, so it is pixel art of a piece with the companion
//! wearing it. Its parts are painted in what they are made of, as a find is (see
//! `trinkets::materials`): a leaf hat is leaf green, an acorn cap an acorn's cup, a bell brass.
//! What is dyed — felt, wool, ribbon, cloth — is in the colours the colony's trinket sheet gives the
//! find the piece is made from, so it stands apart from every coat in the colony for the same
//! reason the keepsake does. Each piece is painted as solid shapes first and then outlined all
//! round in one pass, so it holds its shape against any coat; a strap or a cord too thin to carry
//! an outline is laid on afterwards without one.

use super::*;
use crate::paint::mix;
use crate::trinkets::materials::{self, Ink};
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

/// A material's five tones, by name.
#[derive(Clone, Copy)]
struct Tones {
    edge: Rgba,
    shadow: Rgba,
    base: Rgba,
    light: Rgba,
    shine: Rgba,
}

impl Tones {
    fn of(material: Ink, ink: TrinketInk) -> Self {
        let [edge, shadow, base, light, shine] = material.tones(ink);
        Self {
            edge,
            shadow,
            base,
            light,
            shine,
        }
    }
}

/// Leather: a satchel, a strap.
const LEATHER: Ink = materials::n(0x3e2016, 0x6a3a22, 0x945432, 0xb4744a, 0xd29c70);

/// What one piece is painted on: `solid` is outlined all round once it is done, `bare` is laid on
/// top afterwards as it is.
struct Layers {
    solid: Canvas,
    bare: Canvas,
}

pub(super) fn draw_accessory(canvas: &mut Canvas, art: AccessoryArt, figure: Figure) {
    let (width, height) = (canvas.width(), canvas.height());
    let mut layers = Layers {
        solid: Canvas::new(width, height),
        bare: Canvas::new(width, height),
    };
    let ink = art.ink;
    match art.accessory {
        Accessory::Pin(variant) => pin(&mut layers.bare, ink, variant, figure),
        Accessory::Worn(kind) => {
            let l = &mut layers;
            match kind {
                AccessoryKind::LeafHat => leaf_hat(l, ink, figure),
                AccessoryKind::FlowerCrown => flower_crown(l, ink, figure),
                AccessoryKind::AcornCap => acorn_cap(l, ink, figure),
                AccessoryKind::FeatherCap => feather_cap(l, ink, figure),
                AccessoryKind::MushroomCap => mushroom_cap(l, ink, figure),
                AccessoryKind::RibbonBow => ribbon_bow(l, ink, figure),
                AccessoryKind::PartyHat => party_hat(l, ink, figure),
                AccessoryKind::Sprout => sprout(l, ink, figure),
                AccessoryKind::StarClip => star_clip(l, ink, figure),
                AccessoryKind::MoonClip => moon_clip(l, ink, figure),
                AccessoryKind::CloudEarmuffs => earmuffs(l, ink, figure),
                AccessoryKind::Nightcap => nightcap(l, ink, figure),
                AccessoryKind::KnittedScarf => scarf(l, ink, figure),
                AccessoryKind::Bandana => bandana(l, ink, figure),
                AccessoryKind::BellCollar => bell_collar(l, ink, figure),
                AccessoryKind::ShellNecklace => shell_necklace(l, ink, figure),
                AccessoryKind::PetalRuff => petal_ruff(l, ink, figure),
                AccessoryKind::FriendshipCord => friendship_cord(l, ink, figure),
                AccessoryKind::TinySatchel => satchel(l, ink, figure),
                AccessoryKind::SnailPack => snail_pack(l, ink, figure),
            }
        }
    }
    let mut worn = layers.solid;
    outline(&mut worn, ink.outline);
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let pixel = layers.bare.get(x, y);
            if pixel.a > 0 {
                worn.set(x, y, pixel);
            }
        }
    }
    // Laid over the body, but never through the ground it stands on and never on the frame's own
    // edge.
    let (width, height) = (width as i32, height as i32);
    for y in 1..(height - 1).min(figure.floor + 1) {
        for x in 1..width - 1 {
            let pixel = worn.get(x, y);
            if pixel.a > 0 {
                canvas.set(x, y, pixel);
            }
        }
    }
}

/// One dark pixel round everything painted, holes included.
fn outline(canvas: &mut Canvas, color: Rgba) {
    let (width, height) = (canvas.width() as i32, canvas.height() as i32);
    let mut edges = Vec::new();
    for y in 0..height {
        for x in 0..width {
            if canvas.get(x, y).a > 0 {
                continue;
            }
            let touches = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .into_iter()
                .any(|(dx, dy)| canvas.get(x + dx, y + dy).a > 0);
            if touches {
                edges.push((x, y));
            }
        }
    }
    for (x, y) in edges {
        canvas.set(x, y, color);
    }
}

/// The top half of an ellipse standing on a flat base — a cap, a dome, a hat's crown — shaded as
/// a round thing lit from the upper left: light along its upper-left curve, shadow down its right
/// side and along the bottom.
fn dome(canvas: &mut Canvas, cx: i32, base: i32, half: i32, height: i32, tones: Tones) {
    let height = height.max(1);
    for y in base - height + 1..=base {
        let t = (base - y) as f32 / height as f32;
        let reach = (half as f32 * (1.0 - t * t).max(0.0).sqrt()).round() as i32;
        for x in cx - reach..=cx + reach {
            let color = if x >= cx + reach - (reach / 3).max(1) || y == base {
                tones.shadow
            } else if x <= cx - reach + 1 && y < base - 1 || y == base - height + 1 {
                tones.light
            } else {
                tones.base
            };
            canvas.set(x, y, color);
        }
    }
    canvas.set(cx - half / 2, base - height + 2, tones.shine);
}

/// A band across the body `thickness` rows tall and `half` either side of `x`, its lower row in
/// shadow.
fn band(canvas: &mut Canvas, x: i32, y: i32, half: i32, thickness: i32, tones: Tones) {
    canvas.fill_rect(x - half, y, half * 2 + 1, thickness, tones.base);
    if thickness > 1 {
        canvas.fill_rect(x - half, y + thickness - 1, half * 2 + 1, 1, tones.shadow);
    }
    canvas.set(x - half, y, tones.light);
}

/// A five-petalled flower seen face on, three pixels across.
fn flower(canvas: &mut Canvas, x: i32, y: i32, petals: Tones, middle: Tones) {
    for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
        canvas.set(x + dx, y + dy, petals.light);
    }
    canvas.set(x + 1, y + 1, petals.shadow);
    canvas.set(x, y, middle.base);
}

// ---------------------------------------------------------------------------------------------
// On the head.
// ---------------------------------------------------------------------------------------------

/// A broad leaf laid across the top of the head, pointed at the front, veined, and its stalk
/// curling up at the back.
fn leaf_hat(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let leaf = Tones::of(materials::LEAF, ink);
    let half = (figure.head_half + 2).clamp(5, 11);
    let (cx, top) = (figure.crown.x, figure.crown.y);
    // Three rows: lit along the top, full in the middle, in shadow underneath, narrowing to a
    // point at the front.
    for (row, inset, tone) in [
        (-3, 2, leaf.light),
        (-2, 0, leaf.base),
        (-1, 1, leaf.shadow),
    ] {
        let back = cx - half + inset;
        let front = cx + half - inset / 2;
        l.solid
            .fill_rect(back, top + row, front - back + 1, 1, tone);
    }
    l.solid.set(cx + half + 1, top - 2, leaf.base);
    // Side veins slanting back from the midrib.
    for vein in [cx - half / 2, cx, cx + half / 2] {
        l.solid.set(vein, top - 3, leaf.base);
        l.solid.set(vein + 1, top - 1, leaf.base);
    }
    l.solid.set(cx - half / 2 - 1, top - 3, leaf.shine);
    // The stalk, too fine to outline.
    l.bare.set(cx - half - 1, top - 2, leaf.edge);
    l.bare.set(cx - half - 2, top - 3, leaf.edge);
    l.bare.set(cx - half - 2, top - 4, leaf.edge);
}

/// Small flowers across the top of the head on a band of green, their petals in turn white and the
/// colour of the daisy the crown was made from.
fn flower_crown(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let (leaf, white, dyed, pollen) = (
        Tones::of(materials::LEAF, ink),
        Tones::of(materials::PETAL, ink),
        Tones::of(materials::DYE, ink),
        Tones::of(materials::POLLEN, ink),
    );
    let half = (figure.head_half + 1).clamp(4, 10);
    let (cx, top) = (figure.crown.x, figure.crown.y);
    let spots: &[i32] = if half >= 7 {
        &[-3, -1, 1, 3]
    } else {
        &[-2, 0, 2]
    };
    let step = (half as f32 / 3.2).max(1.6);
    for (index, spot) in spots.iter().enumerate() {
        let x = cx + (*spot as f32 * step).round() as i32;
        let dip = i32::from(spot.abs() >= 3);
        let petals = if index % 2 == 0 { white } else { dyed };
        flower(&mut l.solid, x, top - 1 + dip, petals, pollen);
    }
    // The woven band, and a leaf at each end.
    l.bare
        .line(cx - half, top + 1, cx + half, top + 1, 1, leaf.shadow);
    l.bare.set(cx - half - 1, top, leaf.base);
    l.bare.set(cx + half + 1, top, leaf.base);
}

/// The cap off an acorn, worn like a beret: a scaly dome with its little stalk on top.
fn acorn_cap(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let cup = Tones::of(materials::CUP, ink);
    let bark = Tones::of(materials::BARK, ink);
    let half = figure.head_half.clamp(4, 9);
    let (cx, top) = (figure.crown.x, figure.crown.y + 1);
    dome(&mut l.solid, cx, top, half, 4, cup);
    // Scales in offset rows, as a real cup's are.
    for row in 0..3 {
        let y = top - 1 - row;
        for x in (cx - half + 1 + row % 2..=cx + half - 1).step_by(2) {
            if l.solid.get(x, y) == cup.base {
                l.solid.set(x, y, cup.shadow);
            }
        }
    }
    // A rim a shade lighter where it meets the head.
    l.solid
        .fill_rect(cx - half, top, half * 2 + 1, 1, cup.light);
    l.solid.set(cx + half, top, cup.shadow);
    l.solid.fill_rect(cx, top - 5, 2, 2, bark.base);
    l.solid.set(cx, top - 5, bark.light);
}

/// A snug cap of felt with one long pale feather tucked into its band, sweeping up and back.
fn feather_cap(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let felt = Tones::of(materials::DYE, ink);
    let band_tones = Tones::of(materials::ACCENT, ink);
    let quill = Tones::of(materials::FEATHER, ink);
    let half = figure.head_half.clamp(4, 9);
    let (cx, top) = (figure.crown.x, figure.crown.y + 1);
    dome(&mut l.solid, cx, top, half, 3, felt);
    l.solid
        .fill_rect(cx - half + 1, top - 1, half * 2 - 1, 1, band_tones.base);
    // The feather: a vane that widens from the band and narrows to a curled tip, pale on its
    // upper edge and barred across.
    let (fx, fy) = (cx - half / 2, top - 2);
    const VANE: [(i32, i32, i32); 8] = [
        (0, 0, 1),
        (-1, -1, 2),
        (-1, -2, 2),
        (-2, -3, 2),
        (-2, -4, 2),
        (-3, -5, 2),
        (-4, -6, 2),
        (-5, -6, 1),
    ];
    for (index, (dx, dy, width)) in VANE.into_iter().enumerate() {
        for step in 0..width {
            let tone = if step == 0 { quill.shine } else { quill.light };
            l.solid.set(fx + dx + step, fy + dy, tone);
        }
        if index % 3 == 2 {
            l.solid.set(fx + dx + width - 1, fy + dy, quill.base);
        }
    }
    l.solid.set(fx + 1, fy, quill.shadow);
}

/// A toadstool worn as a hat: a tall round red cap with white spots and pale gills underneath.
fn mushroom_cap(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let cap = Tones::of(materials::TOADSTOOL, ink);
    let flesh = Tones::of(materials::IVORY, ink);
    let half = (figure.head_half + 2).clamp(5, 11);
    let (cx, top) = (figure.crown.x, figure.crown.y + 1);
    dome(&mut l.solid, cx, top, half, 5, cap);
    l.solid
        .fill_rect(cx - half + 1, top, half * 2 - 1, 1, flesh.shadow);
    for (dx, dy, big) in [
        (-half / 2, -3, true),
        (half / 3 + 1, -4, false),
        (half - 2, -2, false),
        (-half + 2, -1, false),
    ] {
        l.solid.set(cx + dx, top + dy, flesh.shine);
        if big {
            l.solid.set(cx + dx + 1, top + dy, flesh.light);
            l.solid.set(cx + dx, top + dy + 1, flesh.light);
        }
    }
}

/// A bow of ribbon tied at the back of the head.
fn ribbon_bow(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let ribbon = Tones::of(materials::DYE, ink);
    let (x, y) = (figure.crown.x - figure.head_half / 2 - 1, figure.crown.y);
    for side in [-1, 1] {
        let lx = x + side * 2;
        l.solid.fill_rect(lx - 1, y - 2, 3, 3, ribbon.base);
        l.solid.set(lx - side, y - 2, ribbon.light);
        l.solid.set(lx + side, y, ribbon.shadow);
    }
    // The knot, and two tails hanging from it.
    l.solid.set(x, y - 1, ribbon.shadow);
    l.solid.set(x - 1, y + 2, ribbon.base);
    l.solid.set(x + 1, y + 2, ribbon.shadow);
    l.solid.set(x - 1, y + 1, ribbon.base);
    l.solid.set(x + 1, y + 1, ribbon.base);
}

/// A striped paper cone with a pompom, worn a little to the back.
fn party_hat(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let (card, stripe, tissue) = (
        Tones::of(materials::DYE, ink),
        Tones::of(materials::ACCENT, ink),
        Tones::of(materials::SUGAR, ink),
    );
    let half = (figure.head_half / 2 + 1).clamp(3, 5);
    let (cx, base) = (figure.crown.x - 1, figure.crown.y + 1);
    let height = half * 2 + 2;
    for row in 0..=height {
        let reach = half * (height - row) / height;
        let y = base - row;
        let tones = if (row / 2) % 2 == 0 { card } else { stripe };
        for x in cx - reach..=cx + reach {
            let color = if x == cx + reach && reach > 0 {
                tones.shadow
            } else if x == cx - reach && reach > 0 {
                tones.light
            } else {
                tones.base
            };
            l.solid.set(x, y, color);
        }
    }
    l.solid.fill_circle(cx, base - height, 1, tissue.base);
    l.solid.set(cx - 1, base - height - 1, tissue.shine);
}

/// A seedling growing straight out of the top of the head.
fn sprout(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let green = Tones::of(materials::SPROUT, ink);
    let (x, y) = (figure.crown.x, figure.crown.y);
    l.solid.fill_ellipse(x - 2, y - 5, 2, 1, green.base);
    l.solid.fill_ellipse(x + 2, y - 6, 2, 1, green.base);
    l.solid.set(x - 3, y - 6, green.light);
    l.solid.set(x + 1, y - 7, green.light);
    l.solid.set(x + 3, y - 5, green.shadow);
    l.solid.set(x - 1, y - 4, green.shadow);
    l.bare.line(x, y, x, y - 5, 1, green.edge);
}

/// A small gold star clipped on at the front of the head.
fn star_clip(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let gold = Tones::of(materials::BRASS, ink);
    let (x, y) = (figure.crown.x + figure.head_half / 2 + 1, figure.crown.y);
    const STAR: [&str; 6] = ["..l..", "..l..", "lllbb", ".lbs.", ".b.s.", "s...s"];
    for (row, line) in STAR.iter().enumerate() {
        for (column, letter) in line.chars().enumerate() {
            let color = match letter {
                'l' => gold.light,
                'b' => gold.base,
                's' => gold.shadow,
                _ => continue,
            };
            l.solid
                .set(x - 2 + column as i32, y - 3 + row as i32, color);
        }
    }
    l.solid.set(x - 1, y - 1, gold.shine);
}

/// A pale crescent moon clipped on at the front of the head.
fn moon_clip(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let moon = Tones::of(materials::STARLIGHT, ink);
    let (x, y) = (figure.crown.x + figure.head_half / 2 + 1, figure.crown.y);
    const MOON: [&str; 6] = [".ll..", "lb...", "lb...", "bb...", "bbs.s", ".sss."];
    for (row, line) in MOON.iter().enumerate() {
        for (column, letter) in line.chars().enumerate() {
            let color = match letter {
                'l' => moon.light,
                'b' => moon.base,
                's' => moon.shadow,
                _ => continue,
            };
            l.solid
                .set(x - 2 + column as i32, y - 3 + row as i32, color);
        }
    }
    l.solid.set(x - 1, y - 3, moon.shine);
}

/// Two puffs of cloud over the ears on a band across the top of the head.
fn earmuffs(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let cloud = Tones::of(materials::SUGAR, ink);
    let strap = Tones::of(materials::DYE, ink);
    let half = figure.head_half.clamp(4, 10);
    let (cx, top) = (figure.crown.x, figure.crown.y);
    for x in cx - half + 1..=cx + half - 1 {
        let t = (x - cx) as f32 / half as f32;
        let lift = ((1.0 - t * t) * 2.0).round() as i32;
        l.bare.set(x, top - lift, strap.shadow);
    }
    for side in [-1, 1] {
        let (x, y) = (cx + side * half, top + 3);
        l.solid.fill_circle(x, y, 2, cloud.base);
        l.solid.set(x - 1, y - 1, cloud.shine);
        l.solid.set(x, y - 1, cloud.light);
        l.solid.set(x + 1, y + 1, cloud.shadow);
        l.solid.set(x, y + 2, cloud.shadow);
    }
}

/// A floppy nightcap: a soft cone leaning back off the head and flopping down behind it, a
/// stripe round it, a pompom on its tip and a white brim.
fn nightcap(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let (cloth, stripe, fluff) = (
        Tones::of(materials::DYE, ink),
        Tones::of(materials::ACCENT, ink),
        Tones::of(materials::SUGAR, ink),
    );
    let half = figure.head_half.clamp(4, 9);
    let (cx, base) = (figure.crown.x, figure.crown.y + 1);
    // The cone, narrowing as it rises and leaning further back with every row.
    let height = (half + 2).clamp(5, 9);
    for row in 0..height {
        let reach = half - (half * row) / height;
        let lean = row * 2 / 3;
        let y = base - 1 - row;
        let middle = cx - lean;
        let tones = if row == height / 2 { stripe } else { cloth };
        for x in middle - reach..=middle + reach {
            let color = if x == middle + reach {
                tones.shadow
            } else if x == middle - reach {
                tones.light
            } else {
                tones.base
            };
            l.solid.set(x, y, color);
        }
    }
    // The tip flops over the back and hangs down, with a pompom on its end.
    let (tip_x, tip_y) = (cx - (height - 1) * 2 / 3 - 2, base - height + 1);
    l.solid.fill_rect(tip_x, tip_y - 1, 2, 3, cloth.shadow);
    l.solid.fill_circle(tip_x - 1, tip_y + 3, 1, fluff.base);
    l.solid.set(tip_x - 2, tip_y + 2, fluff.shine);
    // A soft white brim round the bottom.
    l.solid
        .fill_rect(cx - half - 1, base, half * 2 + 3, 1, fluff.light);
    l.solid.set(cx + half + 1, base, fluff.shadow);
}

// ---------------------------------------------------------------------------------------------
// Round the neck.
// ---------------------------------------------------------------------------------------------

/// A knitted scarf, striped, with one end hanging down the chest and fringed.
fn scarf(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let (wool, stripe) = (
        Tones::of(materials::DYE, ink),
        Tones::of(materials::ACCENT, ink),
    );
    let half = figure.neck_half.clamp(3, 10);
    let (x, y) = (figure.neck.x, figure.neck.y);
    band(&mut l.solid, x, y, half, 2, wool);
    for column in (x - half + 1..=x + half).step_by(3) {
        l.solid.set(column, y, stripe.base);
        l.solid.set(column, y + 1, stripe.shadow);
    }
    // The hanging end, on the front, with a stripe across it.
    let (end_x, end_top) = (x + half / 2, y + 2);
    l.solid.fill_rect(end_x, end_top, 3, 4, wool.base);
    l.solid.fill_rect(end_x + 2, end_top, 1, 4, wool.shadow);
    l.solid.fill_rect(end_x, end_top + 2, 3, 1, stripe.base);
    l.bare.set(end_x, end_top + 5, wool.shadow);
    l.bare.set(end_x + 2, end_top + 5, wool.shadow);
}

/// A kerchief knotted at the back with its point down the front, spotted white.
fn bandana(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let (cloth, spot) = (
        Tones::of(materials::DYE, ink),
        Tones::of(materials::PETAL, ink),
    );
    let half = figure.neck_half.clamp(3, 10);
    let (x, y) = (figure.neck.x, figure.neck.y);
    band(&mut l.solid, x, y, half, 1, cloth);
    let (point_x, depth) = (x + 1, (half / 2 + 3).clamp(3, 6));
    for row in 0..depth {
        let reach = (half - 1) * (depth - row) / depth;
        for column in point_x - reach..=point_x + reach {
            let color = if column == point_x + reach {
                cloth.shadow
            } else {
                cloth.base
            };
            l.solid.set(column, y + 1 + row, color);
        }
    }
    l.solid.set(point_x - 1, y + 2, spot.light);
    l.solid.set(point_x + 1, y + 3, spot.light);
    l.solid.set(point_x - 2, y + 1, spot.light);
    l.solid.set(x - half - 1, y + 1, cloth.base);
}

/// A collar with a brass bell hanging from the front of it.
fn bell_collar(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let (strap, brass) = (
        Tones::of(materials::DYE, ink),
        Tones::of(materials::BRASS, ink),
    );
    let half = figure.neck_half.clamp(3, 10);
    let (x, y) = (figure.neck.x, figure.neck.y);
    band(&mut l.solid, x, y, half, 1, strap);
    let bell_x = x + 1;
    l.solid.fill_circle(bell_x, y + 3, 2, brass.base);
    l.solid.set(bell_x - 1, y + 2, brass.shine);
    l.solid.set(bell_x - 1, y + 3, brass.light);
    l.solid.set(bell_x + 1, y + 4, brass.shadow);
    l.solid.set(bell_x + 2, y + 3, brass.shadow);
    l.solid.set(bell_x, y + 4, ink.outline);
}

/// A cord round the neck with a fan of shell hanging from it.
fn shell_necklace(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let (twine, shell) = (
        Tones::of(materials::TWINE, ink),
        Tones::of(materials::SHELL, ink),
    );
    let half = figure.neck_half.clamp(3, 10);
    let (x, y) = (figure.neck.x, figure.neck.y);
    let (sx, sy) = (x + 1, y + 3);
    const FAN: [&str; 4] = [".lll.", "lbbbs", "lbsbs", ".bss."];
    for (row, line) in FAN.iter().enumerate() {
        for (column, letter) in line.chars().enumerate() {
            let color = match letter {
                'l' => shell.light,
                'b' => shell.base,
                's' => shell.shadow,
                _ => continue,
            };
            l.solid
                .set(sx - 2 + column as i32, sy - 1 + row as i32, color);
        }
    }
    l.solid.set(sx - 1, sy - 1, shell.shine);
    for dx in -half..=half {
        let sag = i32::from(dx.abs() < half / 2);
        if l.solid.get(x + dx, y + sag).a == 0 {
            l.bare.set(x + dx, y + sag, twine.shadow);
        }
    }
}

/// A frill of rounded petals all the way round the neck, white and the colony's colour in turn.
fn petal_ruff(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let (white, dyed) = (
        Tones::of(materials::PETAL, ink),
        Tones::of(materials::DYE, ink),
    );
    let half = figure.neck_half.clamp(3, 10) + 1;
    let (x, y) = (figure.neck.x, figure.neck.y);
    for (index, dx) in (-half..=half).step_by(3).enumerate() {
        let tones = if index % 2 == 0 { white } else { dyed };
        l.solid.fill_rect(x + dx, y, 3, 2, tones.base);
        l.solid.set(x + dx, y, tones.light);
        l.solid.set(x + dx + 2, y + 1, tones.shadow);
        l.solid.set(x + dx + 1, y + 2, tones.shadow);
    }
}

/// A cord of two colours twisted together, tied at the front with its ends hanging.
fn friendship_cord(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let (one, two) = (
        Tones::of(materials::DYE, ink),
        Tones::of(materials::ACCENT, ink),
    );
    let half = figure.neck_half.clamp(3, 10);
    let (x, y) = (figure.neck.x, figure.neck.y);
    for dx in -half..=half {
        let (top, bottom) = if dx.rem_euclid(2) == 0 {
            (one.light, two.shadow)
        } else {
            (two.light, one.shadow)
        };
        l.solid.set(x + dx, y, top);
        l.solid.set(x + dx, y + 1, bottom);
    }
    l.solid.fill_rect(x, y + 2, 2, 1, one.base);
    l.bare.set(x, y + 3, one.shadow);
    l.bare.set(x + 1, y + 4, two.shadow);
}

// ---------------------------------------------------------------------------------------------
// Carried.
// ---------------------------------------------------------------------------------------------

/// A little leather bag at the back hip with a dyed flap and a brass clasp, on a strap from the
/// far shoulder.
fn satchel(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let (leather, flap, brass) = (
        Tones::of(LEATHER, ink),
        Tones::of(materials::DYE, ink),
        Tones::of(materials::BRASS, ink),
    );
    let (bag_x, bag_y) = (figure.hip.x, figure.hip.y.min(figure.floor - 6));
    let shoulder = (figure.neck.x + figure.neck_half / 2, figure.neck.y + 1);
    l.solid.fill_rect(bag_x - 2, bag_y, 6, 5, leather.base);
    l.solid
        .fill_rect(bag_x - 2, bag_y + 4, 6, 1, leather.shadow);
    l.solid.fill_rect(bag_x + 3, bag_y, 1, 5, leather.shadow);
    l.solid.fill_rect(bag_x - 2, bag_y, 6, 2, flap.base);
    l.solid.set(bag_x - 2, bag_y, flap.light);
    l.solid.set(bag_x + 3, bag_y + 1, flap.shadow);
    l.solid.set(bag_x + 1, bag_y + 2, brass.light);
    let strap = mix(leather.shadow, leather.edge, 0.4);
    l.bare
        .line(shoulder.0, shoulder.1, bag_x + 2, bag_y - 1, 1, strap);
}

/// A snail's shell carried on the back like a pack, its spiral turning in to the middle.
fn snail_pack(l: &mut Layers, ink: TrinketInk, figure: Figure) {
    let shell = Tones::of(materials::SAND, ink);
    let whorl = Tones::of(materials::NUT, ink);
    let (x, y) = (figure.back.x, figure.back.y + 1);
    l.solid.fill_circle(x, y, 3, shell.base);
    for (dx, dy) in [(-2, -2), (-3, -1), (-1, -3), (-3, 0)] {
        l.solid.set(x + dx, y + dy, shell.light);
    }
    for (dx, dy) in [(3, 1), (2, 2), (1, 3), (3, 0)] {
        l.solid.set(x + dx, y + dy, shell.shadow);
    }
    // The spiral: round from the rim to the middle.
    for (dx, dy) in [
        (1, -2),
        (2, -1),
        (2, 0),
        (1, 1),
        (0, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
    ] {
        l.solid.set(x + dx, y + dy, whorl.shadow);
    }
    l.solid.set(x, y, whorl.base);
    let strap = Tones::of(LEATHER, ink);
    l.bare.line(
        x + 3,
        y - 1,
        figure.neck.x,
        figure.neck.y + 1,
        1,
        strap.shadow,
    );
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
