//! What a companion has in its paws or at its mouth while it eats, drinks or plays, drawn apart
//! from its body.
//!
//! A body frame says where the thing is held; the thing itself is a cell of its own on the held
//! sheet, one texture every companion shares, drawn over the body and face where the frame says. Keeping the two apart
//! is what lets a companion hold something other than the one snack and toy it was born with:
//! the body is baked once, and whatever is in its paws is chosen as it is drawn.
//!
//! Held things are drawn in their own colours, not the holder's, so food looks like food and a
//! toy keeps its colour whoever picks it up. The shared dark edge keeps each one readable against
//! any coat.

use super::foods::{draw_food, food_offset};
use super::*;
use formiga_core::meals::Food;

/// How big a held thing's cell is, in art pixels.
pub const HELD_CELL: u32 = 16;
/// Cells to a row of the held sheet.
pub const HELD_SHEET_COLUMNS: u32 = 16;

/// How many colourways each toy is made in. A companion's own toy comes in one of them.
pub const TOY_COLOURWAYS: u8 = 3;

/// One picture of something held: which thing, and how far through being eaten, turned or
/// sipped it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeldThing {
    /// One of the snacks, `bite` mouthfuls in (0 whole, 3 the last of it).
    Snack { kind: u8, bite: u8 },
    /// One of the toys in one of its colourways, turned `spin` quarters.
    Toy { kind: u8, colourway: u8, spin: u8 },
    /// One of the cups, tipped to the mouth or resting, on the first or second beat of a sip.
    Cup { kind: u8, tipped: bool, sip: u8 },
    /// Something from a meal (see [`formiga_core::meals::Food`]), `bite` mouthfuls in.
    Food { food: Food, bite: u8 },
}

const SNACK_CELLS: u32 = SNACK_KINDS as u32 * 4;
const TOY_CELLS: u32 = TOY_KINDS as u32 * TOY_COLOURWAYS as u32 * 4;
const CUP_CELLS: u32 = DRINK_KINDS as u32 * 4;
const FOOD_CELLS: u32 = Food::ALL.len() as u32 * 4;

/// Every held picture, one cell each.
pub const HELD_CELLS: u32 = SNACK_CELLS + TOY_CELLS + CUP_CELLS + FOOD_CELLS;
pub const HELD_SHEET_WIDTH: u32 = HELD_CELL * HELD_SHEET_COLUMNS;
pub const HELD_SHEET_HEIGHT: u32 = HELD_CELL * HELD_CELLS.div_ceil(HELD_SHEET_COLUMNS);

impl HeldThing {
    /// Every picture, in cell order.
    pub fn all() -> impl Iterator<Item = Self> {
        let snacks =
            (0..SNACK_KINDS).flat_map(|kind| (0..4).map(move |bite| Self::Snack { kind, bite }));
        let toys = (0..TOY_KINDS).flat_map(|kind| {
            (0..TOY_COLOURWAYS).flat_map(move |colourway| {
                (0..4).map(move |spin| Self::Toy {
                    kind,
                    colourway,
                    spin,
                })
            })
        });
        let cups = (0..DRINK_KINDS).flat_map(|kind| {
            [false, true]
                .into_iter()
                .flat_map(move |tipped| (0..2).map(move |sip| Self::Cup { kind, tipped, sip }))
        });
        let foods = Food::ALL
            .into_iter()
            .flat_map(|food| (0..4).map(move |bite| Self::Food { food, bite }));
        snacks.chain(toys).chain(cups).chain(foods)
    }

    /// Which of the held cells this picture is, from 0.
    pub fn index(self) -> u32 {
        match self {
            Self::Snack { kind, bite } => {
                u32::from(kind % SNACK_KINDS) * 4 + u32::from(bite.min(3))
            }
            Self::Toy {
                kind,
                colourway,
                spin,
            } => {
                SNACK_CELLS
                    + (u32::from(kind % TOY_KINDS) * u32::from(TOY_COLOURWAYS)
                        + u32::from(colourway % TOY_COLOURWAYS))
                        * 4
                    + u32::from(spin % 4)
            }
            Self::Cup { kind, tipped, sip } => {
                SNACK_CELLS
                    + TOY_CELLS
                    + u32::from(kind % DRINK_KINDS) * 4
                    + u32::from(tipped) * 2
                    + u32::from(sip % 2)
            }
            Self::Food { food, bite } => {
                SNACK_CELLS
                    + TOY_CELLS
                    + CUP_CELLS
                    + u32::from(food.index()) * 4
                    + u32::from(bite.min(3))
            }
        }
    }

    /// Where the thing's own middle sits in its cell, measured from the cell's middle. Most sit
    /// in the middle; a yo-yo hangs low so its string fits above it, and a cup sits low so the
    /// steam off it or the straw out of it does.
    pub fn offset(self) -> PixelPoint {
        let dy = match self {
            Self::Toy { kind, .. } => match kind % TOY_KINDS {
                4 => 2,
                5 => 1,
                _ => 0,
            },
            Self::Cup { .. } => 3,
            Self::Food { food, .. } => food_offset(food),
            Self::Snack { .. } => 0,
        };
        PixelPoint { x: 0, y: dy }
    }

    /// Draws the thing with its middle at `x`, `y`.
    pub fn draw(self, canvas: &mut Canvas, x: i32, y: i32) {
        match self {
            Self::Snack { kind, bite } => {
                draw_generated_snack(canvas, snack_palette(kind), kind, x, y, bite)
            }
            Self::Toy {
                kind,
                colourway,
                spin,
            } => draw_generated_toy(
                canvas,
                toy_palette(colourway),
                EffectMotif::Star,
                kind,
                x,
                y,
                spin,
            ),
            Self::Cup { kind, tipped, sip } => {
                draw_generated_drinkware(canvas, cup_palette(kind), kind, x, y, tipped, sip)
            }
            Self::Food { food, bite } => draw_food(canvas, food, x, y, bite),
        }
    }

    /// Where this picture's cell sits on the held sheet, as its left, top, right and bottom
    /// texture coordinates.
    pub fn uv(self) -> (f32, f32, f32, f32) {
        let cell = self.index();
        let (column, row) = (
            (cell % HELD_SHEET_COLUMNS) as f32,
            (cell / HELD_SHEET_COLUMNS) as f32,
        );
        let (width, height) = (HELD_SHEET_WIDTH as f32, HELD_SHEET_HEIGHT as f32);
        let size = HELD_CELL as f32;
        (
            column * size / width,
            row * size / height,
            (column + 1.0) * size / width,
            (row + 1.0) * size / height,
        )
    }

    /// The thing alone in its cell, as the held sheet carries it.
    pub fn cell(self) -> Canvas {
        let mut canvas = Canvas::new(HELD_CELL, HELD_CELL);
        let middle = HELD_CELL as i32 / 2;
        let offset = self.offset();
        self.draw(&mut canvas, middle + offset.x, middle + offset.y);
        canvas
    }
}

/// Every held picture on one sheet, [`HELD_SHEET_COLUMNS`] to a row, in [`HeldThing::index`]
/// order. Nothing on it depends on the colony or the companion holding it, so one is made once.
pub fn render_held_sheet() -> Canvas {
    let mut sheet = Canvas::new(HELD_SHEET_WIDTH, HELD_SHEET_HEIGHT);
    for thing in HeldThing::all() {
        let index = thing.index();
        let x0 = (index % HELD_SHEET_COLUMNS * HELD_CELL) as i32;
        let y0 = (index / HELD_SHEET_COLUMNS * HELD_CELL) as i32;
        blit_transparent(&mut sheet, &thing.cell(), x0, y0);
    }
    sheet
}

/// Something held in one body frame: what it is, and where the middle of its cell lands in the
/// frame, in frame pixels, with the frame facing right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeldPlacement {
    pub thing: HeldThing,
    pub at: PixelPoint,
}

impl HeldPlacement {
    /// Holding `thing` with its own middle at `x`, `y` of the frame.
    pub(super) fn centred(thing: HeldThing, x: i32, y: i32) -> Self {
        let offset = thing.offset();
        Self {
            thing,
            at: PixelPoint {
                x: x - offset.x,
                y: y - offset.y,
            },
        }
    }

    /// The same hold with something else in it, its middle where this one's was.
    pub fn holding(self, thing: HeldThing) -> Self {
        let offset = self.thing.offset();
        Self::centred(thing, self.at.x + offset.x, self.at.y + offset.y)
    }

    pub(super) fn translated(self, dx: i32, dy: i32) -> Self {
        Self {
            at: PixelPoint {
                x: self.at.x + dx,
                y: self.at.y + dy,
            },
            ..self
        }
    }

    /// Paints the held thing onto a whole frame, over the body and face, as the overlay draws
    /// it. `facing_right` is the way the frame on `canvas` faces, already mirrored or not.
    pub fn paint(self, canvas: &mut Canvas, facing_right: bool) {
        let mut cell = self.thing.cell();
        let half = HELD_CELL as i32 / 2;
        let origin_x = if facing_right {
            self.at.x - half
        } else {
            cell.mirror_horizontal();
            FRAME_SIZE as i32 - half - self.at.x
        };
        blit_transparent(canvas, &cell, origin_x, self.at.y - half);
    }
}

/// The shared dark edge every held thing carries.
const HELD_OUTLINE: Rgba = Rgba::new(58, 42, 50, 255);

const fn held_palette(shadow: Rgba, fill: Rgba, highlight: Rgba) -> Palette {
    Palette {
        outline: HELD_OUTLINE,
        shadow,
        coat: fill,
        highlight,
        accent: fill,
        eye: HELD_OUTLINE,
    }
}

/// Each snack in the colours it grows or bakes in.
fn snack_palette(kind: u8) -> Palette {
    match kind % SNACK_KINDS {
        // A red berry on a green stem.
        0 => held_palette(
            Rgba::new(74, 128, 62, 255),
            Rgba::new(204, 52, 70, 255),
            Rgba::new(250, 176, 184, 255),
        ),
        // A golden biscuit with darker seeds.
        1 => held_palette(
            Rgba::new(132, 88, 50, 255),
            Rgba::new(214, 164, 98, 255),
            Rgba::new(242, 208, 152, 255),
        ),
        // A brown seed.
        2 => held_palette(
            Rgba::new(118, 82, 46, 255),
            Rgba::new(176, 128, 72, 255),
            Rgba::new(222, 184, 124, 255),
        ),
        // Pink melon flesh on a green rind.
        _ => held_palette(
            Rgba::new(78, 148, 72, 255),
            Rgba::new(236, 98, 98, 255),
            Rgba::new(255, 192, 182, 255),
        ),
    }
}

/// A toy's colourway: red, blue or yellow.
fn toy_palette(colourway: u8) -> Palette {
    match colourway % TOY_COLOURWAYS {
        0 => held_palette(
            Rgba::new(150, 42, 46, 255),
            Rgba::new(220, 72, 64, 255),
            Rgba::new(255, 192, 172, 255),
        ),
        1 => held_palette(
            Rgba::new(46, 88, 150, 255),
            Rgba::new(78, 142, 214, 255),
            Rgba::new(192, 224, 255, 255),
        ),
        _ => held_palette(
            Rgba::new(188, 132, 40, 255),
            Rgba::new(240, 196, 66, 255),
            Rgba::new(255, 240, 182, 255),
        ),
    }
}

/// A cream cup with what is in it: cocoa in the mug, water in the bowl, juice in the bottle.
fn cup_palette(kind: u8) -> Palette {
    let drink = match kind % DRINK_KINDS {
        0 => Rgba::new(150, 96, 64, 255),
        1 => Rgba::new(96, 160, 214, 255),
        _ => Rgba::new(240, 150, 60, 255),
    };
    held_palette(
        Rgba::new(168, 156, 146, 255),
        drink,
        Rgba::new(244, 234, 212, 255),
    )
}

/// The colourway a companion's own toy comes in, from the same genes as which toy it is.
pub(super) fn toy_colourway(genome: &AppearanceGenome) -> u8 {
    ((prop_signature(genome) >> 19) % u64::from(TOY_COLOURWAYS)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_held_sheet_has_every_picture_visible_in_its_own_cell() {
        let sheet = render_held_sheet();
        assert_eq!(sheet, render_held_sheet());
        assert_eq!(sheet.width(), HELD_SHEET_WIDTH);
        assert_eq!(sheet.height(), HELD_SHEET_HEIGHT);
        let things: Vec<HeldThing> = HeldThing::all().collect();
        assert_eq!(things.len() as u32, HELD_CELLS);
        let mut indexes: Vec<u32> = things.iter().map(|thing| thing.index()).collect();
        indexes.sort_unstable();
        indexes.dedup();
        assert_eq!(indexes, (0..HELD_CELLS).collect::<Vec<_>>());
        let mut firsts = Vec::new();
        for thing in things {
            let cell = thing.cell();
            let opaque = cell.pixels().iter().filter(|pixel| pixel.a > 100).count();
            // The last of a snack is meant to be small; it still has to be there.
            let least = if matches!(
                thing,
                HeldThing::Snack { bite: 3, .. } | HeldThing::Food { bite: 3, .. }
            ) {
                4
            } else {
                12
            };
            assert!(opaque >= least, "{thing:?} covers only {opaque} pixels");
            let (left, top, right, bottom) = thing.uv();
            assert!(left < right && top < bottom && right <= 1.0 && bottom <= 1.0);
            // A toy turning or a cup between sips can look the same on two beats, so only each
            // thing's first picture has to be its own.
            if matches!(
                thing,
                HeldThing::Snack { bite: 0, .. }
                    | HeldThing::Food { bite: 0, .. }
                    | HeldThing::Toy { spin: 0, .. }
                    | HeldThing::Cup {
                        tipped: false,
                        sip: 0,
                        ..
                    }
            ) {
                assert!(!firsts.contains(&cell), "{thing:?} looks like another");
                firsts.push(cell);
            }
        }
    }

    /// Every food goes down mouthful by mouthful: the last of it is less than all of it.
    #[test]
    fn every_food_is_eaten_down() {
        for food in Food::ALL {
            let opaque = |bite| {
                HeldThing::Food { food, bite }
                    .cell()
                    .pixels()
                    .iter()
                    .filter(|pixel| pixel.a > 0)
                    .count()
            };
            assert!(opaque(3) < opaque(0), "{food:?} is never eaten");
            assert!(opaque(0) >= 20, "{food:?} is too small to read as food");
        }
    }

    #[test]
    fn a_held_thing_painted_facing_left_is_the_mirror_of_facing_right() {
        let placement = HeldPlacement::centred(
            HeldThing::Toy {
                kind: 4,
                colourway: 1,
                spin: 0,
            },
            30,
            20,
        );
        let mut right = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        placement.paint(&mut right, true);
        let mut left = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        placement.paint(&mut left, false);
        left.mirror_horizontal();
        assert_eq!(left, right);
    }
}
