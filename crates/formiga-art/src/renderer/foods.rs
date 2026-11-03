//! Every food a meal can be made of, drawn as itself at each stage of being eaten: whole, bitten,
//! half gone, and down to the last of it. Each keeps the colours it grows or bakes in, and the
//! bites come out of the side the mouth is on.

use super::*;
use formiga_core::meals::Food;

const EDGE: Rgba = Rgba::new(58, 42, 50, 255);
const RED: Rgba = Rgba::new(206, 50, 66, 255);
const RED_DARK: Rgba = Rgba::new(140, 32, 52, 255);
const PINK: Rgba = Rgba::new(250, 176, 184, 255);
const LEAF: Rgba = Rgba::new(84, 148, 70, 255);
const LEAF_DARK: Rgba = Rgba::new(52, 104, 54, 255);
const CRUST: Rgba = Rgba::new(204, 150, 86, 255);
const CRUST_LIGHT: Rgba = Rgba::new(240, 204, 146, 255);
const CRUST_DARK: Rgba = Rgba::new(140, 92, 50, 255);
const SEED: Rgba = Rgba::new(176, 130, 74, 255);
const SEED_LIGHT: Rgba = Rgba::new(226, 190, 128, 255);
const PEA: Rgba = Rgba::new(122, 184, 78, 255);
const PEA_LIGHT: Rgba = Rgba::new(184, 226, 128, 255);
const CREAM: Rgba = Rgba::new(246, 236, 214, 255);
const APPLE: Rgba = Rgba::new(214, 58, 52, 255);
const APPLE_LIGHT: Rgba = Rgba::new(252, 150, 132, 255);
const FLESH: Rgba = Rgba::new(250, 236, 196, 255);
const PUMPKIN: Rgba = Rgba::new(236, 136, 48, 255);
const PUMPKIN_LIGHT: Rgba = Rgba::new(252, 196, 120, 255);
const MELON: Rgba = Rgba::new(240, 96, 104, 255);
const RIND: Rgba = Rgba::new(70, 140, 66, 255);
const SPONGE: Rgba = Rgba::new(244, 214, 150, 255);
const ICING: Rgba = Rgba::new(252, 242, 246, 255);
const JAM: Rgba = Rgba::new(214, 64, 92, 255);
const CARROT: Rgba = Rgba::new(238, 132, 46, 255);
const CARROT_DARK: Rgba = Rgba::new(190, 92, 34, 255);
const SHELL: Rgba = Rgba::new(150, 104, 62, 255);
const SHELL_LIGHT: Rgba = Rgba::new(200, 152, 96, 255);
const KERNEL: Rgba = Rgba::new(232, 200, 150, 255);
const CAP: Rgba = Rgba::new(176, 70, 52, 255);
const STALK: Rgba = Rgba::new(240, 228, 206, 255);
const BOWL: Rgba = Rgba::new(120, 168, 206, 255);
const BOWL_LIGHT: Rgba = Rgba::new(196, 224, 242, 255);
const SOUP: Rgba = Rgba::new(222, 96, 60, 255);
const PORRIDGE: Rgba = Rgba::new(236, 222, 190, 255);

/// Where a food's middle sits in its cell, below the cell's middle. A bowl sits low so the steam
/// off it fits; everything else sits in the middle.
pub(super) fn food_offset(food: Food) -> i32 {
    match food {
        Food::Soup | Food::Porridge => 2,
        Food::Carrot | Food::PeaPod | Food::Breadstick => 0,
        _ => 0,
    }
}

fn blob(canvas: &mut Canvas, x: i32, y: i32, rx: i32, ry: i32, fill: Rgba) {
    canvas.fill_ellipse(x, y, rx + 1, ry + 1, EDGE);
    canvas.fill_ellipse(x, y, rx, ry, fill);
}

/// Takes a round bite out of the right-hand side, where the mouth is.
fn bite(canvas: &mut Canvas, x: i32, y: i32, radius: i32) {
    canvas.fill_circle(x, y, radius, Rgba::TRANSPARENT);
}

/// Clears everything right of column `x`: the part already eaten from the mouth's side.
fn eaten_past(canvas: &mut Canvas, x: i32) {
    for yy in 0..canvas.height() as i32 {
        for xx in x.max(0)..canvas.width() as i32 {
            canvas.set(xx, yy, Rgba::TRANSPARENT);
        }
    }
}

/// Draws `food`, `bites` mouthfuls in (0 whole, 3 the last of it), with its middle at `x`, `y`.
pub(super) fn draw_food(canvas: &mut Canvas, food: Food, x: i32, y: i32, bites: u8) {
    let bites = i32::from(bites.min(3));
    // Each food is drawn on its own tile first so a bite only ever takes the food away.
    let mut tile = Canvas::new(canvas.width(), canvas.height());
    let t = &mut tile;
    match food {
        // Three berries in a bunch, one fewer each mouthful.
        Food::Berries => {
            let berries = [(-2, 1), (2, 1), (0, -2)];
            for (dx, dy) in berries.iter().take((3 - bites).max(1) as usize) {
                blob(t, x + dx, y + dy, 2, 2, RED);
                t.set(x + dx - 1, y + dy - 1, PINK);
            }
            t.line(x, y - 5, x + 2, y - 6, 1, LEAF_DARK);
        }
        // A little heap of seeds, smaller each time.
        Food::Seeds => {
            let seeds = [(-3, 1), (0, 2), (3, 1), (-1, -1), (2, -1)];
            for (dx, dy) in seeds.iter().take((5 - bites * 2).max(1) as usize) {
                blob(t, x + dx, y + dy, 1, 1, SEED);
                t.set(x + dx, y + dy - 1, SEED_LIGHT);
            }
        }
        // A pod split open with peas in a row, one fewer each time.
        Food::Peas => {
            t.fill_ellipse(x, y + 1, 7, 3, EDGE);
            t.fill_ellipse(x, y + 1, 6, 2, LEAF);
            for i in 0..(3 - bites).max(1) {
                let px = x - 3 + i * 3;
                blob(t, px, y, 1, 1, PEA);
                t.set(px - 1, y - 1, PEA_LIGHT);
            }
        }
        // A strawberry, pointed end down, with its leafy cap.
        Food::Strawberry => {
            blob(t, x, y, 4, 4, RED);
            t.fill_rect(x - 2, y + 4, 5, 1, EDGE);
            t.fill_rect(x - 1, y + 4, 3, 1, RED);
            for (dx, dy) in [(-2, -1), (1, 1), (-1, 2), (2, -2)] {
                t.set(x + dx, y + dy, CREAM);
            }
            t.fill_rect(x - 3, y - 5, 7, 2, LEAF_DARK);
            t.fill_rect(x - 2, y - 5, 5, 1, LEAF);
            if bites > 0 {
                bite(t, x + 4, y - 1, 1 + bites);
            }
        }
        // A round biscuit with seeds in it.
        Food::Biscuit => {
            blob(t, x, y, 5, 4, CRUST);
            t.fill_ellipse(x - 1, y - 1, 3, 2, CRUST_LIGHT);
            for (dx, dy) in [(-2, 0), (1, -2), (2, 1), (-1, 2)] {
                t.set(x + dx, y + dy, CRUST_DARK);
            }
            if bites > 0 {
                bite(t, x + 5, y - 1, bites + 1);
            }
            if bites > 1 {
                bite(t, x + 3, y + 3, bites);
            }
        }
        // Two cherries on joined stems; one goes, then the other is bitten.
        Food::Cherries => {
            if bites < 2 {
                blob(t, x + 3, y + 2, 2, 2, RED_DARK);
                t.set(x + 2, y + 1, PINK);
                t.line(x + 3, y, x, y - 5, 1, LEAF_DARK);
            }
            blob(t, x - 2, y + 2, 2, 2, RED_DARK);
            t.set(x - 3, y + 1, PINK);
            t.line(x - 2, y, x, y - 5, 1, LEAF_DARK);
            t.set(x + 1, y - 6, LEAF);
            if bites == 3 {
                bite(t, x - 1, y + 2, 1);
            }
        }
        // A mushroom: a red cap with spots on a pale stalk.
        Food::MushroomCap => {
            t.fill_rect(x - 2, y, 5, 5, EDGE);
            t.fill_rect(x - 1, y, 3, 4, STALK);
            t.fill_ellipse(x, y - 1, 6, 3, EDGE);
            t.fill_ellipse(x, y - 1, 5, 2, CAP);
            t.fill_rect(x - 5, y, 11, 1, EDGE);
            for (dx, dy) in [(-3, -2), (1, -3), (3, -1)] {
                t.set(x + dx, y + dy, CREAM);
            }
            if bites > 0 {
                bite(t, x + 5, y - 1, bites + 1);
            }
        }
        // A red apple with a stalk and leaf; bitten into white flesh, then down to the core.
        Food::Apple => {
            if bites < 3 {
                blob(t, x, y + 1, 5, 5, APPLE);
                t.set(x - 3, y - 1, APPLE_LIGHT);
                t.set(x - 2, y - 2, APPLE_LIGHT);
                if bites > 0 {
                    t.fill_circle(x + 5, y, bites + 1, EDGE);
                    t.fill_circle(x + 5, y, bites, FLESH);
                    bite(t, x + 6, y, bites);
                }
                if bites > 1 {
                    t.fill_circle(x - 5, y + 2, 2, EDGE);
                    t.fill_circle(x - 5, y + 2, 1, FLESH);
                    bite(t, x - 6, y + 2, 1);
                }
            } else {
                // The core: a thin waist of flesh between two ends.
                t.fill_rect(x - 2, y - 3, 5, 2, EDGE);
                t.fill_rect(x - 1, y - 3, 3, 1, APPLE);
                t.fill_rect(x - 1, y - 1, 3, 4, EDGE);
                t.fill_rect(x, y - 1, 1, 4, FLESH);
                t.fill_rect(x - 2, y + 3, 5, 2, EDGE);
                t.fill_rect(x - 1, y + 3, 3, 1, APPLE);
            }
            t.line(x, y - 4, x + 1, y - 6, 1, CRUST_DARK);
            t.fill_rect(x + 1, y - 6, 2, 1, LEAF);
        }
        // A wedge of pumpkin, skin side down.
        Food::PumpkinSlice => {
            for row in 0..6 {
                let half = 6 - row;
                t.fill_rect(x - half - 1, y + 3 - row, half * 2 + 3, 1, EDGE);
                t.fill_rect(x - half, y + 3 - row, half * 2 + 1, 1, PUMPKIN_LIGHT);
            }
            t.fill_rect(x - 7, y + 3, 15, 2, EDGE);
            t.fill_rect(x - 6, y + 3, 13, 1, PUMPKIN);
            t.set(x, y, SEED_LIGHT);
            t.set(x - 2, y + 1, SEED_LIGHT);
            if bites > 0 {
                eaten_past(t, x + 6 - bites * 3);
                t.line(
                    x + 6 - bites * 3,
                    y - 2 + bites,
                    x + 6 - bites * 3,
                    y + 4,
                    1,
                    EDGE,
                );
            }
        }
        // A wedge of melon: pink flesh, dark seeds, green rind along the bottom.
        Food::MelonWedge => {
            for row in 0..6 {
                let half = 6 - row;
                t.fill_rect(x - half - 1, y + 3 - row, half * 2 + 3, 1, EDGE);
                t.fill_rect(x - half, y + 3 - row, half * 2 + 1, 1, MELON);
            }
            t.fill_rect(x - 7, y + 3, 15, 2, EDGE);
            t.fill_rect(x - 6, y + 3, 13, 1, RIND);
            for (dx, dy) in [(-2, 1), (1, 0), (3, 2), (-4, 2)] {
                t.set(x + dx, y + dy, EDGE);
            }
            if bites > 0 {
                eaten_past(t, x + 6 - bites * 3);
                t.line(
                    x + 6 - bites * 3,
                    y - 2 + bites,
                    x + 6 - bites * 3,
                    y + 4,
                    1,
                    EDGE,
                );
            }
        }
        // A slice of layer cake: sponge, jam, sponge, icing on top.
        Food::CakeSlice => {
            t.fill_rect(x - 6, y - 4, 12, 9, EDGE);
            t.fill_rect(x - 5, y - 3, 10, 7, SPONGE);
            t.fill_rect(x - 5, y, 10, 1, JAM);
            t.fill_rect(x - 5, y - 3, 10, 2, ICING);
            blob(t, x - 2, y - 5, 1, 1, RED);
            if bites > 0 {
                eaten_past(t, x + 6 - bites * 3);
                t.line(x + 5 - bites * 3, y - 4, x + 5 - bites * 3, y + 4, 1, EDGE);
            }
        }
        // A carrot held sideways, its green top on the left, shorter each bite.
        Food::Carrot => {
            let tip = x + 6 - bites * 3;
            t.fill_rect(x - 4, y - 2, tip - x + 5, 5, EDGE);
            t.fill_rect(x - 3, y - 1, tip - x + 3, 3, CARROT);
            for i in [x - 1, x + 2, x + 5] {
                if i < tip - 1 {
                    t.set(i, y - 1, CARROT_DARK);
                }
            }
            t.line(x - 4, y, x - 7, y - 3, 1, LEAF_DARK);
            t.line(x - 4, y, x - 7, y + 1, 1, LEAF);
        }
        // A pea pod held sideways, shorter each bite.
        Food::PeaPod => {
            let end = x + 6 - bites * 3;
            t.fill_ellipse(x, y, 7, 3, EDGE);
            t.fill_ellipse(x, y, 6, 2, LEAF);
            for px in [x - 3, x, x + 3] {
                t.set(px, y, PEA_LIGHT);
            }
            t.line(x - 7, y, x - 7, y - 2, 1, LEAF_DARK);
            if bites > 0 {
                eaten_past(t, end);
                t.line(end, y - 2, end, y + 2, 1, EDGE);
            }
        }
        // A breadstick held sideways, shorter each bite.
        Food::Breadstick => {
            let end = x + 7 - bites * 3;
            t.fill_rect(x - 7, y - 2, end - x + 8, 4, EDGE);
            t.fill_rect(x - 6, y - 1, end - x + 6, 2, CRUST);
            for i in (x - 5..end - 1).step_by(3) {
                t.set(i, y - 1, CRUST_LIGHT);
            }
        }
        // A walnut: whole, cracked open, then the kernel nibbled down.
        Food::Walnut => match bites {
            0 => {
                blob(t, x, y, 4, 4, SHELL);
                t.line(x, y - 4, x, y + 4, 1, EDGE);
                t.set(x - 2, y - 2, SHELL_LIGHT);
                t.set(x + 2, y + 1, SHELL_LIGHT);
            }
            _ => {
                // The two halves of the shell either side, the kernel in the middle.
                blob(t, x - 5, y + 2, 2, 2, SHELL);
                if bites < 2 {
                    blob(t, x + 5, y + 2, 2, 2, SHELL);
                }
                blob(t, x, y, 3 - bites / 2, 3 - bites / 2, KERNEL);
                t.set(x - 1, y - 1, CREAM);
            }
        },
        // An acorn in its cup: whole, the cup off, then nibbled.
        Food::Acorn => {
            blob(t, x, y + 1, 3, 4 - bites / 2, SEED_LIGHT);
            t.set(x - 1, y, CREAM);
            if bites == 0 {
                t.fill_ellipse(x, y - 3, 4, 2, EDGE);
                t.fill_ellipse(x, y - 3, 3, 1, SHELL);
                t.line(x, y - 5, x + 1, y - 7, 1, EDGE);
            }
            if bites > 1 {
                bite(t, x + 3, y, bites - 1);
            }
        }
        // A blue bowl with soup or porridge in it, the level dropping, steam while it is full.
        Food::Soup | Food::Porridge => {
            let fill = if food == Food::Soup { SOUP } else { PORRIDGE };
            t.fill_ellipse(x, y, 7, 4, EDGE);
            t.fill_rect(x - 7, y - 4, 15, 4, Rgba::TRANSPARENT);
            t.fill_ellipse(x, y, 6, 3, BOWL);
            t.fill_rect(x - 6, y - 3, 13, 3, Rgba::TRANSPARENT);
            t.fill_rect(x - 7, y - 1, 15, 1, EDGE);
            t.fill_rect(x - 6, y - 1 + bites, 13 - bites * 2 / 3, 1, fill);
            t.fill_rect(x - 5, y + 1, 4, 1, BOWL_LIGHT);
            if bites == 0 {
                for (dx, dy) in [(-2, -3), (-1, -5), (2, -4), (3, -6)] {
                    t.set(x + dx, y + dy, CREAM);
                }
            }
        }
    }
    blit_transparent(canvas, &tile, 0, 0);
}
