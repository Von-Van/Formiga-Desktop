//! The village getting on with things, one moment to a cell: the gardens tended, every kind of
//! house seen to, somebody napping indoors and somebody up on a roof, the three mishaps, and a yawn
//! going round — each drawn the way the overlay draws it.
//!
//!   cargo run -p formiga-tools -- village-life-sheet [--output docs/assets/village-life-sheet.png]
//!
//! A companion's body and face come through `BodyPresentation` and the face the desktop resolves
//! from its beat, and what it holds is drawn at the anchor the overlay holds things at. Houses come
//! from the village atlas, patches and props from the colony's object sheet, all placed on one
//! ground line the way the village places them.

use crate::{blit_scaled_square_alpha, write_png};
use anyhow::Result;
use formiga_art::{
    COLONY_OBJECT_SIZE, Canvas, ColonyObjectRenderer, CreatureRenderer, FRAME_SIZE, FramePlacement,
    PropAnchor, PropSprite, SHELTER_SIZE, ShelterRenderer, VillageCell,
};
use formiga_core::*;
use std::path::PathBuf;
use time::OffsetDateTime;

const SCALE: u32 = 3;
/// One moment's cell, in art pixels, and the ground line inside it.
const CELL_WIDTH: u32 = 120;
const CELL_HEIGHT: u32 = 96;
const GROUND: i32 = 88;
const COLUMNS: u32 = 4;

/// One thing drawn in a cell.
enum Piece {
    /// A house of this kind, with somebody at home or not, standing on the ground at `x`.
    House {
        style: ShelterStyle,
        occupied: bool,
        x: i32,
    },
    /// A garden patch at one stage, on the ground at `x`.
    Patch {
        kind: GardenKind,
        stage: GardenStage,
        x: i32,
    },
    /// A hangout spot on the ground at `x`.
    Spot { kind: HangoutKind, x: i32 },
    /// A companion standing at `x`, `lift` pixels off the ground, in whatever its state says.
    Companion {
        creature: Box<Creature>,
        x: i32,
        lift: i32,
    },
    /// Something loose, centred on a point.
    Loose { prop: PropSprite, x: i32, y: i32 },
}

/// A companion doing `action`, facing `right`, part way through `beat` if it has one.
fn posed(
    subject: &Creature,
    action: ActionKind,
    frame_time: f32,
    right: bool,
    beat: Option<(BeatKind, f32, Option<VillageProp>)>,
) -> Creature {
    let mut creature = subject.clone();
    let state = &mut creature.state;
    state.facing_right = right;
    state.flourish = None;
    state.attention = None;
    state.velocity = Point::default();
    state.action = action;
    state.action_elapsed = frame_time;
    state.beat = beat.map(|(kind, progress, held)| {
        let mut beat = Beat::new(kind, 4.0);
        beat.elapsed = progress * 4.0;
        beat.held = held;
        beat
    });
    creature
}

/// Every moment on the sheet, in order.
fn moments(a: &Creature, b: &Creature) -> Vec<Vec<Piece>> {
    use ActionKind as A;
    use BeatKind as B;
    let companion = |creature: Creature, x: i32| Piece::Companion {
        creature: Box::new(creature),
        x,
        lift: 0,
    };
    let carrot = Some(VillageProp::Produce(GardenKind::Vegetables));
    vec![
        // Watering the flowers, the can tipped and the water running.
        vec![
            Piece::Patch {
                kind: GardenKind::Flowers,
                stage: GardenStage::Grown,
                x: 72,
            },
            companion(
                posed(
                    a,
                    A::Idle,
                    0.0,
                    true,
                    Some((B::Watering, 0.5, Some(VillageProp::WateringCan))),
                ),
                48,
            ),
        ],
        // Crouched over something just coming up.
        vec![
            Piece::Patch {
                kind: GardenKind::Vegetables,
                stage: GardenStage::Sprout,
                x: 72,
            },
            companion(
                posed(a, A::Idle, 0.0, true, Some((B::InspectSprout, 0.5, None))),
                50,
            ),
        ],
        // Picking something ripe, and eating it.
        vec![
            Piece::Patch {
                kind: GardenKind::Vegetables,
                stage: GardenStage::Bounty,
                x: 52,
            },
            companion(
                posed(a, A::Idle, 0.0, true, Some((B::Picking, 0.5, carrot))),
                30,
            ),
            companion(posed(b, A::Eat, 0.6, false, None), 88),
        ],
        // Carried over to a friend, held up, and looked at with pleasure.
        vec![
            companion(
                posed(a, A::Idle, 0.0, true, Some((B::ShowingOff, 0.5, carrot))),
                42,
            ),
            companion(
                posed(b, A::Homebound, 0.0, false, Some((B::Admiring, 0.5, None))),
                80,
            ),
        ],
        // Retying a tent's flap.
        vec![
            Piece::House {
                style: ShelterStyle::Tent,
                occupied: false,
                x: 52,
            },
            companion(
                posed(a, A::Idle, 0.0, false, Some((B::AdjustFlap, 0.5, None))),
                92,
            ),
        ],
        // Plumping up a pillow fort.
        vec![
            Piece::House {
                style: ShelterStyle::PillowFort,
                occupied: false,
                x: 52,
            },
            companion(
                posed(a, A::Idle, 0.0, false, Some((B::FluffCushion, 0.5, None))),
                92,
            ),
        ],
        // Looking up at a mushroom's cap and patting it.
        vec![
            Piece::House {
                style: ShelterStyle::Mushroom,
                occupied: false,
                x: 52,
            },
            companion(
                posed(a, A::Idle, 0.0, false, Some((B::InspectCap, 0.5, None))),
                92,
            ),
        ],
        // Tidying a leaf house's leaves, one of them coming loose.
        vec![
            Piece::House {
                style: ShelterStyle::LeafHouse,
                occupied: false,
                x: 52,
            },
            companion(
                posed(a, A::Idle, 0.0, false, Some((B::TidyLeaves, 0.5, None))),
                92,
            ),
            Piece::Loose {
                prop: PropSprite::Leaf,
                x: 76,
                y: GROUND - 34,
            },
        ],
        // Napping indoors: the curtain drawn, the window lit, and the Zs drifting up.
        vec![
            Piece::House {
                style: ShelterStyle::Mushroom,
                occupied: true,
                x: 60,
            },
            Piece::Loose {
                prop: PropSprite::Snore,
                x: 68,
                y: GROUND - 58,
            },
            Piece::Loose {
                prop: PropSprite::Snore,
                x: 74,
                y: GROUND - 70,
            },
        ],
        // Sitting up on the roof, looking out.
        vec![
            Piece::House {
                style: ShelterStyle::LeafHouse,
                occupied: false,
                x: 60,
            },
            Piece::Companion {
                creature: Box::new(posed(a, A::Perch, 0.0, true, Some((B::RoofSit, 0.5, None)))),
                x: 60,
                lift: house_roof_height(&ShelterGenome::default(), ShelterStyle::LeafHouse, false)
                    .round() as i32,
            },
        ],
        // A leaf lands on a face: the start, then the shake.
        vec![
            companion(
                posed(
                    a,
                    A::Homebound,
                    0.0,
                    true,
                    Some((B::LeafOnFace, 0.15, None)),
                ),
                34,
            ),
            Piece::Loose {
                prop: PropSprite::Leaf,
                x: 34,
                y: GROUND - 26,
            },
            companion(
                posed(a, A::Homebound, 0.0, true, Some((B::LeafOnFace, 0.5, None))),
                86,
            ),
            Piece::Loose {
                prop: PropSprite::Leaf,
                x: 86,
                y: GROUND - 26,
            },
        ],
        // A snack gets away, and is picked up again.
        vec![
            companion(
                posed(a, A::Idle, 0.0, true, Some((B::DroppedSnack, 0.15, None))),
                30,
            ),
            Piece::Loose {
                prop: PropSprite::Apple,
                x: 58,
                y: GROUND - 4,
            },
            companion(
                posed(b, A::Idle, 0.0, true, Some((B::Retrieve, 0.3, None))),
                82,
            ),
            Piece::Loose {
                prop: PropSprite::Apple,
                x: 98,
                y: GROUND - 4,
            },
        ],
        // Sat down just beside the cushion, and a start at finding the ground.
        vec![
            Piece::Spot {
                kind: HangoutKind::Cushion,
                x: 78,
            },
            companion(
                posed(a, A::Idle, 0.0, true, Some((B::MissedCushion, 0.15, None))),
                40,
            ),
            companion(
                posed(b, A::Idle, 0.0, true, Some((B::MissedCushion, 0.42, None))),
                64,
            ),
        ],
        // A yawn: breathing in, the yawn itself, and settling again.
        vec![
            companion(
                posed(a, A::Homebound, 0.0, true, Some((B::Yawn, 0.1, None))),
                22,
            ),
            companion(
                posed(a, A::Homebound, 0.0, true, Some((B::Yawn, 0.45, None))),
                60,
            ),
            companion(
                posed(a, A::Homebound, 0.0, true, Some((B::Yawn, 0.85, None))),
                98,
            ),
        ],
        // A friend looking over at the yawn, and a third holding one back.
        vec![
            companion(
                posed(a, A::Homebound, 0.0, true, Some((B::Yawn, 0.45, None))),
                26,
            ),
            companion(
                posed(b, A::Homebound, 0.0, false, Some((B::Notice, 0.5, None))),
                62,
            ),
            companion(
                posed(
                    b,
                    A::Homebound,
                    0.0,
                    false,
                    Some((B::ResistYawn, 0.5, None)),
                ),
                98,
            ),
        ],
        // Carrying something grown over to a friend, walking.
        vec![
            companion(
                posed(a, A::Traverse, 0.3, true, Some((B::Carrying, 0.2, carrot))),
                44,
            ),
            companion(
                posed(b, A::Homebound, 0.0, false, Some((B::Notice, 0.5, None))),
                88,
            ),
        ],
    ]
}

pub fn run(path: PathBuf) -> Result<()> {
    let desktop = crate::fixture_desktop();
    let a = World::preview_adult([23; 32], OffsetDateTime::UNIX_EPOCH, &desktop);
    let b = World::preview_adult([47; 32], OffsetDateTime::UNIX_EPOCH, &desktop);
    let colony_seed = [61_u8; 32];
    let objects = ColonyObjectRenderer::render_atlas(colony_seed);
    let genome = ShelterGenome::default();
    let styles = ShelterStyle::ALL;
    // One house of each kind by day, in slots 1 to 4 of an atlas, with and without somebody home.
    let village = ShelterRenderer::render_village(
        &genome,
        &[],
        &[None, None, None, None, None, None],
        &[
            genome.style,
            styles[0],
            styles[1],
            styles[2],
            styles[3],
            genome.style,
        ],
        false,
    );
    let cut = |source: &Canvas, (x, y): (u32, u32), size: u32| {
        let mut tile = Canvas::new(size, size);
        for ty in 0..size as i32 {
            for tx in 0..size as i32 {
                tile.set(tx, ty, source.get(x as i32 + tx, y as i32 + ty));
            }
        }
        tile
    };
    let house = |style: ShelterStyle, occupied: bool| {
        let slot = 1 + styles
            .iter()
            .position(|candidate| *candidate == style)
            .unwrap_or(0);
        cut(
            &village,
            ShelterRenderer::village_cell(VillageCell::House {
                slot,
                lit: false,
                occupied,
            }),
            SHELTER_SIZE,
        )
    };
    let object = |cell: u32| {
        cut(
            &objects,
            ColonyObjectRenderer::cell_origin(cell),
            COLONY_OBJECT_SIZE,
        )
    };
    let moments = moments(&a, &b);
    let rows = (moments.len() as u32).div_ceil(COLUMNS);
    let width = COLUMNS * CELL_WIDTH * SCALE;
    let height = rows * CELL_HEIGHT * SCALE;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for (index, pieces) in moments.iter().enumerate() {
        let (column, row) = (index as u32 % COLUMNS, index as u32 / COLUMNS);
        let (left, top) = (column * CELL_WIDTH, row * CELL_HEIGHT);
        // A pale cell with a darker ground under it, so everything stands on something.
        let (paper, ground) = if (column + row) % 2 == 0 {
            ([238, 234, 222, 255], [206, 198, 176, 255])
        } else {
            ([228, 232, 222, 255], [194, 202, 180, 255])
        };
        for y in top..top + CELL_HEIGHT {
            for x in left..left + CELL_WIDTH {
                let colour = if y as i32 - top as i32 >= GROUND {
                    ground
                } else {
                    paper
                };
                for sy in 0..SCALE {
                    for sx in 0..SCALE {
                        let index = (((y * SCALE + sy) * width + x * SCALE + sx) * 4) as usize;
                        pixels[index..index + 4].copy_from_slice(&colour);
                    }
                }
            }
        }
        let place = |pixels: &mut Vec<u8>, canvas: &Canvas, size: u32, x: i32, y: i32| {
            let (px, py) = (left as i32 + x, top as i32 + y);
            if px < 0 || py < 0 {
                return;
            }
            blit_scaled_square_alpha(
                pixels,
                width,
                px as u32 * SCALE,
                py as u32 * SCALE,
                &canvas.rgba_bytes(),
                size,
                SCALE,
            );
        };
        // Houses first, then what stands on the ground, then the companions, then what they
        // hold and what is loose — as the overlay layers them.
        for piece in pieces {
            match piece {
                Piece::House { style, occupied, x } => {
                    let tile = house(*style, *occupied);
                    let half = SHELTER_SIZE as i32 / 2;
                    place(
                        &mut pixels,
                        &tile,
                        SHELTER_SIZE,
                        x - half,
                        GROUND - SHELTER_SIZE as i32,
                    );
                }
                Piece::Patch { kind, stage, x } => {
                    let tile = object(ColonyObjectRenderer::garden_cell(*kind, *stage));
                    place(&mut pixels, &tile, COLONY_OBJECT_SIZE, x - 8, GROUND - 16);
                }
                Piece::Spot { kind, x } => {
                    let tile = object(ColonyObjectRenderer::hangout_cell(*kind));
                    place(&mut pixels, &tile, COLONY_OBJECT_SIZE, x - 8, GROUND - 16);
                }
                Piece::Companion { .. } | Piece::Loose { .. } => {}
            }
        }
        let mut held: Vec<(PropSprite, bool, i32, i32)> = Vec::new();
        for piece in pieces {
            let Piece::Companion { creature, x, lift } = piece else {
                continue;
            };
            let body = formiga_art::BodyPresentation::for_creature(creature);
            let face =
                CreatureRenderer::resolve_face_state(creature, CursorSnapshot::default(), false);
            let frame = CreatureRenderer::render_dressed_composited_frame(
                &creature.appearance,
                None,
                body.clip,
                body.frame,
                body.facing_right,
                false,
                face,
            );
            let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
            let placement = FramePlacement::for_creature(creature, baseline);
            let contact = GROUND - lift;
            let frame_top = contact + placement.origin_y;
            let frame_left = x - FRAME_SIZE as i32 / 2;
            place(&mut pixels, &frame, FRAME_SIZE, frame_left, frame_top);
            // What it holds, at the anchor the overlay holds things at: in front of the face,
            // the way it is facing.
            if let Some(beat) = creature.state.beat
                && let Some(prop) = beat.held
            {
                let anchor_frame = CreatureRenderer::render_body_frame(
                    &creature.appearance,
                    body.clip,
                    body.frame,
                    false,
                );
                let anchor = anchor_frame.face_anchor;
                let face_x = if body.facing_right {
                    anchor.x
                } else {
                    FRAME_SIZE as i32 - anchor.x
                };
                let hand = PropAnchor::facing(body.facing_right);
                let centre_x = frame_left + face_x + hand.dx.round() as i32;
                let centre_y = frame_top + anchor.y + hand.dy.round() as i32;
                held.push((PropSprite::of(prop), !body.facing_right, centre_x, centre_y));
                if beat.kind == BeatKind::Watering {
                    let ahead = if body.facing_right { 7 } else { -7 };
                    held.push((
                        PropSprite::Water,
                        !body.facing_right,
                        centre_x + ahead,
                        centre_y + 7,
                    ));
                }
            }
        }
        for (prop, mirrored, x, y) in held {
            let mut tile = object(ColonyObjectRenderer::prop_cell(prop));
            if mirrored {
                tile.mirror_horizontal();
            }
            place(&mut pixels, &tile, COLONY_OBJECT_SIZE, x - 8, y - 8);
        }
        for piece in pieces {
            if let Piece::Loose { prop, x, y } = piece {
                let tile = object(ColonyObjectRenderer::prop_cell(*prop));
                place(&mut pixels, &tile, COLONY_OBJECT_SIZE, x - 8, y - 8);
            }
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}
