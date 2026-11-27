//! Magic fruit and the little things companions do about the desktop, drawn as the overlay draws
//! them.
//!
//!   cargo run -p formiga-tools -- magic-sheet [--output magic-sheet.png]
//!
//! One row per body: the three original families and the five modular plans. Along each row, left
//! to right, each on its own little bit of ground:
//!
//! - the star fruit, whole and half eaten, and the cheer as a spell comes over it;
//! - floating at three points of a bob, with the gold dust falling from its feet;
//! - bouncing: up, at the top, and the puff as it comes down;
//! - zoomies, mid-run, trailing sparks; shimmering, twice; and the still kind for reduced motion;
//! - pointing at a window and looking at it; peering round the end of a ledge;
//! - sat on a window's top with its legs over the front;
//! - a nudge: winding up, the push with the friend at the end of the ledge, and the glee.
//!
//! Every companion is placed by `FramePlacement`, raised by `CreatureState::drawn_lift` and lit by
//! `Spell::glints`, the rules the overlay draws with, and its shadow shrinks as the overlay's does.

use crate::habit_sheet::subjects;
use crate::{blend_pixel, write_png};
use anyhow::Result;
use formiga_art::{
    BodyPresentation, Canvas, ContactShadow, CreatureRenderer, FRAME_SIZE, FramePlacement,
    HeldThing, Holding, ShadowGround, glint_cell,
};
use formiga_core::magic::{MagicKind, Spell};
use formiga_core::meals::Food;
use formiga_core::*;
use std::path::PathBuf;

const SCALE: u32 = 3;
/// One cell, in art pixels: room for a frame, a lift above it, and a window beside it.
const CELL: (u32, u32) = (72, 76);
/// Where the ground is in a cell, from its top.
const GROUND: i32 = 66;

#[derive(Clone, Copy)]
enum Window {
    None,
    /// A window standing to the right of the companion, its side this far from it.
    Beside(i32),
    /// The companion is on a window's top, which ends this far to its right.
    Under(i32),
}

/// One cell: the companion as posed, what it is holding, the window drawn with it, and a friend.
struct Shot {
    creature: Creature,
    holding: Holding,
    window: Window,
    friend: Option<(Creature, i32, i32)>,
}

fn base(subject: &Creature) -> Creature {
    let mut creature = subject.clone();
    let state = &mut creature.state;
    state.facing_right = true;
    state.attention = None;
    state.flourish = None;
    state.beat = None;
    state.velocity = Point::default();
    state.action = ActionKind::Idle;
    state.action_elapsed = 0.0;
    state.position = Point::default();
    creature
}

fn spelled(
    subject: &Creature,
    kind: MagicKind,
    at: f32,
    walking: Option<f32>,
    still: bool,
) -> Shot {
    let mut creature = base(subject);
    let mut spell = Spell::new(kind, 60.0, 0.0, still);
    spell.elapsed = at;
    spell.strength = 1.0;
    let stride = walking.unwrap_or(0.0);
    spell.lift = spell.shape_lift(walking.is_some(), stride);
    if let Some(stride) = walking {
        creature.state.action = ActionKind::Traverse;
        creature.state.action_elapsed = stride * 0.12;
        creature.state.presence.stepping = true;
        creature.state.presence.stride = stride;
    }
    creature.state.magic = Some(spell);
    Shot {
        creature,
        holding: Holding::AsClip,
        window: Window::None,
        friend: None,
    }
}

fn beat(subject: &Creature, kind: BeatKind, length: f32, at: f32, look: Option<Point>) -> Creature {
    let mut creature = base(subject);
    if kind == BeatKind::LedgeSit || kind == BeatKind::PeekRound || kind == BeatKind::Nudge {
        creature.state.action = ActionKind::Perch;
        creature.state.surface.kind = SurfaceKind::WindowLedge;
    }
    creature.state.beat = Some(Beat {
        elapsed: at,
        look,
        ..Beat::new(kind, length)
    });
    creature
}

fn plain(creature: Creature, window: Window) -> Shot {
    Shot {
        creature,
        holding: Holding::AsClip,
        window,
        friend: None,
    }
}

fn shots(subject: &Creature) -> Vec<Shot> {
    let mut shots = Vec::new();
    for bite in [0, 2] {
        let mut eating = base(subject);
        eating.state.action = ActionKind::Eat;
        eating.state.action_elapsed = 0.6 + f32::from(bite) * 0.5;
        shots.push(Shot {
            creature: eating,
            holding: Holding::Thing(HeldThing::Food {
                food: Food::Starfruit,
                bite,
            }),
            window: Window::None,
            friend: None,
        });
    }
    let mut cheer = spelled(subject, MagicKind::Shimmer, 0.35, None, false);
    cheer.creature.state.beat = Some(Beat {
        elapsed: 0.5,
        ..Beat::new(BeatKind::Delight, 1.8)
    });
    shots.push(cheer);
    for at in [2.0, 2.45, 2.9] {
        shots.push(spelled(subject, MagicKind::Float, at, None, false));
    }
    for at in [2.74, 2.83, 3.07] {
        shots.push(spelled(subject, MagicKind::Bounce, at, None, false));
    }
    shots.push(spelled(subject, MagicKind::Zoomies, 3.0, Some(1.0), false));
    shots.push(spelled(subject, MagicKind::Shimmer, 3.0, None, false));
    shots.push(spelled(subject, MagicKind::Shimmer, 3.5, None, false));
    shots.push(spelled(subject, MagicKind::Float, 3.0, None, true));
    let window_at = Point { x: 30.0, y: -20.0 };
    shots.push(plain(
        beat(subject, BeatKind::PointAt, 3.0, 0.5, Some(window_at)),
        Window::Beside(18),
    ));
    shots.push(plain(
        beat(subject, BeatKind::PointAt, 3.0, 2.0, Some(window_at)),
        Window::Beside(18),
    ));
    let past_end = Point { x: 30.0, y: 40.0 };
    shots.push(plain(
        beat(subject, BeatKind::PeekRound, 2.8, 1.6, Some(past_end)),
        Window::Under(12),
    ));
    shots.push(plain(
        beat(subject, BeatKind::LedgeSit, 10.0, 3.0, None),
        Window::Under(40),
    ));
    let friend_at = Point { x: 24.0, y: 0.0 };
    shots.push(plain(
        beat(subject, BeatKind::Nudge, 1.4, 0.2, Some(friend_at)),
        Window::Under(34),
    ));
    let mut friend = base(subject);
    friend.state.action = ActionKind::Perch;
    let mut push = plain(
        beat(subject, BeatKind::Nudge, 1.4, 0.6, Some(friend_at)),
        Window::Under(34),
    );
    push.friend = Some((friend, 22, 0));
    shots.push(push);
    let mut tumbling = base(subject);
    tumbling.state.action = ActionKind::Tossed;
    tumbling.state.facing_right = false;
    let mut glee = plain(
        beat(subject, BeatKind::Nudge, 1.4, 1.1, Some(friend_at)),
        Window::Under(16),
    );
    glee.friend = Some((tumbling, 30, 18));
    shots.push(glee);
    shots
}

fn fill(pixels: &mut [u8], width: u32, (x, y, w, h): (i32, i32, i32, i32), color: [u8; 4]) {
    let height = (pixels.len() / 4) as u32 / width;
    for py in y.max(0)..(y + h) {
        for px in x.max(0)..(x + w) {
            for oy in 0..SCALE {
                for ox in 0..SCALE {
                    let (tx, ty) = (px as u32 * SCALE + ox, py as u32 * SCALE + oy);
                    if tx < width && ty < height {
                        blend_pixel(pixels, width, tx, ty, color);
                    }
                }
            }
        }
    }
}

/// Draws `canvas` with its top-left at art pixel (`x`, `y`) of the sheet.
fn blit(
    pixels: &mut [u8],
    width: u32,
    canvas: &Canvas,
    x: i32,
    y: i32,
    from: (u32, u32, u32, u32),
) {
    let (sx0, sy0, sw, sh) = from;
    for sy in 0..sh {
        for sx in 0..sw {
            let pixel = canvas.get((sx0 + sx) as i32, (sy0 + sy) as i32);
            if pixel.a == 0 {
                continue;
            }
            let (px, py) = (x + sx as i32, y + sy as i32);
            if px < 0 || py < 0 {
                continue;
            }
            fill(
                pixels,
                width,
                (px, py, 1, 1),
                [pixel.r, pixel.g, pixel.b, pixel.a],
            );
        }
    }
}

/// One companion, its contact point at art pixel (`x`, `ground`), lifted as the overlay lifts it,
/// with its shadow and any lights.
fn draw_companion(
    pixels: &mut [u8],
    width: u32,
    creature: &Creature,
    holding: Holding,
    (x, ground): (i32, i32),
    sheet: &Canvas,
) {
    let body = BodyPresentation::for_creature(creature);
    let face = CreatureRenderer::resolve_face_state(creature, CursorSnapshot::default(), false);
    let canvas = CreatureRenderer::render_composited_frame_holding(
        &creature.appearance,
        None,
        body.clip,
        body.frame,
        body.facing_right,
        false,
        face,
        holding,
    );
    let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
    let lift = creature.state.drawn_lift();
    let contact = ground as f32 - lift;
    let rect = FramePlacement::for_creature(creature, baseline).frame_rect(x as f32, contact, 1.0);
    let top = rect.top.round() as i32;
    let left = x - FRAME_SIZE as i32 / 2;
    // Its shadow on the ground, shrinking with the lift, as the overlay lays it.
    if let Some(feet) = ContactShadow::feet(&canvas)
        && let Some(shadow) = ContactShadow::under(feet, lift.max(0.0))
    {
        let shadow = if body.facing_right {
            shadow
        } else {
            shadow.mirrored()
        };
        let cell = shadow.cell(ShadowGround::Desktop);
        blit(
            pixels,
            width,
            sheet,
            left + shadow.left().round() as i32,
            ground - 1,
            (cell.x, cell.y, cell.width, cell.height),
        );
    }
    blit(
        pixels,
        width,
        &canvas,
        left,
        top,
        (0, 0, FRAME_SIZE, FRAME_SIZE),
    );
    if let Some(spell) = creature.state.magic {
        let presence = &creature.state.presence;
        for glint in spell
            .glints(
                presence.stepping,
                presence.stride,
                creature.state.facing_right,
            )
            .into_iter()
            .flatten()
        {
            let cell = glint_cell(glint.size);
            let half = cell.width as i32 / 2;
            blit(
                pixels,
                width,
                sheet,
                x + glint.x.round() as i32 - half,
                ground + glint.y.round() as i32 - half,
                (cell.x, cell.y, cell.width, cell.height),
            );
        }
    }
}

pub fn run(path: PathBuf) -> Result<()> {
    let subjects = subjects();
    let sheet = ContactShadow::sheet();
    let columns = shots(&subjects[0]).len() as u32;
    let width = columns * CELL.0 * SCALE;
    let height = subjects.len() as u32 * CELL.1 * SCALE;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    // A pale desk to see it all against.
    fill(
        &mut pixels,
        width,
        (
            0,
            0,
            (columns * CELL.0) as i32,
            (subjects.len() as u32 * CELL.1) as i32,
        ),
        [228, 232, 238, 255],
    );
    let window_face = [250, 250, 252, 255];
    let window_edge = [120, 128, 142, 255];
    for (row, subject) in subjects.iter().enumerate() {
        for (column, shot) in shots(subject).iter().enumerate() {
            let (cx, cy) = (column as i32 * CELL.0 as i32, row as i32 * CELL.1 as i32);
            let x = cx + 24;
            let mut ground = cy + GROUND;
            // The floor.
            fill(
                &mut pixels,
                width,
                (cx, cy + GROUND, CELL.0 as i32, 1),
                [170, 176, 186, 255],
            );
            match shot.window {
                Window::None => {}
                Window::Beside(gap) => {
                    let rect = (x + gap, cy + 14, CELL.0 as i32 - 24 - gap, GROUND - 14);
                    fill(&mut pixels, width, rect, window_edge);
                    fill(
                        &mut pixels,
                        width,
                        (rect.0 + 1, rect.1 + 1, rect.2 - 1, rect.3 - 1),
                        window_face,
                    );
                }
                Window::Under(end) => {
                    ground = cy + 38;
                    let rect = (cx, ground, x + end - cx, GROUND - 38);
                    fill(&mut pixels, width, rect, window_edge);
                    fill(
                        &mut pixels,
                        width,
                        (rect.0, rect.1 + 1, rect.2 - 1, rect.3 - 1),
                        window_face,
                    );
                }
            }
            if let Some((friend, dx, dy)) = &shot.friend {
                draw_companion(
                    &mut pixels,
                    width,
                    friend,
                    Holding::AsClip,
                    (x + dx, ground + dy),
                    &sheet,
                );
            }
            draw_companion(
                &mut pixels,
                width,
                &shot.creature,
                shot.holding,
                (x, ground),
                &sheet,
            );
        }
    }
    write_png(&path, width, height, &pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}
