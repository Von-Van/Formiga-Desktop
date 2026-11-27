//! Bounded, interchangeable pixel parts. No image tracing or unbounded geometry.
use super::*;
use formiga_core::{BodyPlan, CreatureDesign, EarStyle};

mod turned;
pub(super) use turned::{
    TURNED_FACE, climbing_face, clinging_body, clings, turned_body, turned_clip, turned_footfall,
};
pub use turned::{TURNED_STRIDE_FRAMES, View, can_turn, walks_turned};

/// The largest a body is drawn, against the average: a companion of the greatest stature.
const MAX_SIZE: f32 = formiga_core::STATURE_MAX as f32 / 100.0 + 0.01;

/// The largest a four-pawed body is drawn. It is the longest plan, its head out in front and its
/// tail behind, and past this it would not fit its frame with a cup in its paws, so a large one
/// stops growing here.
const LONG_MAX_SIZE: f32 = 1.10;

/// The size `d` is drawn at, between the smallest a mini is drawn and the largest its plan allows.
/// The column of a climber's frame the edge line of the window it climbs is drawn on, facing
/// right: as far past the middle of the frame as the simulation hangs it out from the edge (see
/// `formiga_core::climb_grip_reach`), less the grip. Everything from here on is over the window.
pub(super) const CLIMB_WALL: i32 = FRAME_SIZE as i32 / 2 + 14;
/// The column a climber's paw holds the wall at: on the edge line, its tip over it.
const CLIMB_PAW: i32 = CLIMB_WALL - 2;

/// The row of a frame the ledge is drawn on in each of a mantle's first frames, for feet that
/// reach down to `feet`: as far above the row under them as the simulation has the ledge above
/// the companion then (see `formiga_core::MANTLE_FRAME_DROP`).
pub(super) fn mantle_ledge(feet: i32, frame: u8, all_fours: bool) -> i32 {
    let drops = formiga_core::mantle_frame_drop_for(all_fours);
    feet + 1 - i32::from(drops[usize::from(frame).min(drops.len() - 1)])
}

fn drawn_size(d: CreatureDesign, size: f32) -> f32 {
    let largest = if d.body == BodyPlan::Long {
        LONG_MAX_SIZE
    } else {
        MAX_SIZE
    };
    size.clamp(0.55, largest)
}

fn oval(c: &mut Canvas, p: Palette, x: i32, y: i32, rx: i32, ry: i32, color: Rgba) {
    c.fill_ellipse(x, y, rx + 1, ry + 1, p.outline);
    c.fill_ellipse(x, y, rx, ry, color);
}

/// Which of a body `draw_parts` draws: all of it, as nearly every frame is; or the head and what
/// grows on it apart from everything else, for a four-pawed body turned upright against a wall
/// under a head that stays level.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Parts {
    All,
    Trunk,
    Head,
}

pub(super) fn draw(
    c: &mut Canvas,
    design: CreatureDesign,
    p: Palette,
    pose: Pose,
    size: f32,
    clip: BodyClip,
    frame: u8,
) -> PixelPoint {
    let d = design.bounded();
    if d.body == BodyPlan::Long && pose.wall {
        return draw_upright(c, d, p, pose, size, clip, frame);
    }
    let body = measure(d, pose, size);
    draw_parts(c, d, p, pose, size, clip, frame, body, Parts::All)
}

/// Where a four-pawed body is drawn in its frame while it is upright against a wall: its trunk as
/// [`upright`] turns it, and its head, level, where that puts it.
#[derive(Clone, Copy)]
struct Upright {
    /// The column the row under its feet is turned to, and the row its column 0 is turned to.
    under: i32,
    top: i32,
    head: PixelPoint,
}

impl Upright {
    /// Where a point of the body as it stands is once it is turned upright.
    fn turn(self, at: PixelPoint) -> PixelPoint {
        PixelPoint {
            x: self.under + at.y,
            y: self.top - at.x,
        }
    }
}

/// How a four-pawed body is turned upright against the wall for a frame of its climb, or of the
/// start of its mantle: a quarter turn, head up and paws to the wall, placed so that the paws
/// reach the wall's edge line and a little over it, and so that its forepaws at full reach are on
/// the ledge the mantle's first frame hangs from. That keeps it in one place in its frame for the
/// whole of the climb and the pull-up.
fn upright(d: CreatureDesign, body: Body, pose: Pose) -> Upright {
    // The edge line for a body that reaches this far (see `formiga_core::climb_grip_reach`):
    // the paws' last two columns are over it.
    let wall = FRAME_SIZE as i32 / 2 + 16;
    let feet = feet_reach(d, body.floor);
    let ledge = mantle_ledge(feet, 0, true);
    // The far forepaw, reached two pixels up the wall, rests its lower edge on the ledge.
    let (_, far_fore) = leg_roots(body, false);
    // The far foot of a stick leg is drawn a row lower than anything else stands.
    let lowest = feet + i32::from(d.classic.limbs == 2);
    let mut place = Upright {
        under: wall + 1 - lowest,
        top: ledge + far_fore.x,
        head: PixelPoint { x: 0, y: 0 },
    };
    place.head = place.turn(PixelPoint {
        x: body.hx,
        y: body.hy,
    });
    // A head craned forward goes up the wall with the turn; carried level, it also looks a
    // little toward the wall it is climbing.
    place.head.x += pose.lean.clamp(0, 1);
    place
}

/// A four-pawed body upright against a wall: its trunk, legs and tail drawn as it stands and
/// walks, and turned a quarter turn so that its paws are on the wall and its head is uppermost;
/// and its head, with its ears and whatever else grows on it, drawn level over that, so the face
/// it carries is the right way up.
fn draw_upright(
    c: &mut Canvas,
    d: CreatureDesign,
    p: Palette,
    pose: Pose,
    size: f32,
    clip: BodyClip,
    frame: u8,
) -> PixelPoint {
    // The trunk is measured standing, without the lean, which belongs to the head.
    let standing = measure(d, Pose { lean: 0, ..pose }, size);
    let place = upright(d, standing, pose);
    let mut trunk = Canvas::new(FRAME_SIZE, FRAME_SIZE);
    draw_parts(
        &mut trunk,
        d,
        p,
        pose,
        size,
        clip,
        frame,
        standing,
        Parts::Trunk,
    );
    for y in 0..FRAME_SIZE as i32 {
        for x in 0..FRAME_SIZE as i32 {
            let pixel = trunk.get(x, y);
            if pixel.a > 0 {
                let at = place.turn(PixelPoint { x, y });
                c.set(at.x, at.y, pixel);
            }
        }
    }
    let head = Body {
        hx: place.head.x,
        hy: place.head.y,
        ..standing
    };
    draw_parts(c, d, p, pose, size, clip, frame, head, Parts::Head)
}

#[allow(clippy::too_many_arguments)]
fn draw_parts(
    c: &mut Canvas,
    d: CreatureDesign,
    p: Palette,
    pose: Pose,
    size: f32,
    clip: BodyClip,
    frame: u8,
    body: Body,
    parts: Parts,
) -> PixelPoint {
    let k = d.classic;
    let long = d.body == BodyPlan::Long;
    // A blob is one soft mass carrying its own face, so it keeps a rounder minimum and
    // never grows the separate head oval every other plan draws.
    let blob = d.body == BodyPlan::Blob;
    // A blob's ears and gleam ride its one mass, so they are part of its trunk.
    let trunk = parts != Parts::Head;
    let headed = parts != Parts::Trunk;
    let Body {
        x,
        y,
        rx,
        ry,
        hx,
        hy,
        head,
        floor,
    } = body;
    let size = drawn_size(d, size);
    // Ears ride the top of a blob's mass; every other plan hangs them off the head.
    let (ear_cx, ear_span, ear_top) = if blob {
        (x, rx - 4, y - ry + 2)
    } else {
        (hx, head - 3, hy - head + 2)
    };

    // Tail and ears go behind the body and never through the reserved face area.
    let tx = x - rx + 1;
    if trunk {
        tail_behind(c, d, p, pose, tx, y, ry);
    }
    // A pricked ear grows out of the base it already had, so it stays joined to the head.
    let ear = ((f32::from(d.ear_size) * size).round().max(3.0) as i32 + pose.ear_perk.clamp(0, 2)
        - pose.ears_back.clamp(0, 3))
    .max(2);
    if k.crown > 0 && headed {
        classic_crown(c, p, k.crown, ear_cx, ear_span, ear_top, ear, pose);
    }
    // Wing-nubs and horns grow from behind the body and the head, which are drawn over their
    // roots, so they stay joined to the silhouette and never reach the reserved face.
    if d.details.wings > 0 && trunk {
        if long {
            for (dx, near) in [(1, false), (-2, true)] {
                let root = PixelPoint {
                    x: x + dx,
                    y: y - ry + 3 - pose.appendage_lift.clamp(-1, 2),
                };
                wing_nub(c, p, d.details.wings, root, -1, near);
            }
        } else {
            // From the shoulders, low enough that they spread beside the body rather than the
            // head, where they would read as a second pair of ears.
            for side in [-1, 1] {
                let root = PixelPoint {
                    x: x + side * (rx - 2),
                    y: y - 1 - pose.appendage_lift.clamp(-1, 2),
                };
                wing_nub(c, p, d.details.wings, root, side, true);
            }
        }
    }
    if (1..=3).contains(&d.details.horns) && headed {
        // Inside the ears, so the two never cover one another, and in the belly's cream where
        // there is one.
        // Antlers branch outward, so they stand further apart than horns, over the ears.
        let reach = if d.details.horns == 3 {
            (ear_span - 1).max(3)
        } else {
            (ear_span - 4).max(2)
        };
        let shade = horn_shade(d, p);
        for side in [-1, 1] {
            horn(
                c,
                p,
                shade,
                d.details.horns,
                PixelPoint {
                    x: ear_cx + side * reach,
                    y: ear_top,
                },
                side,
            );
        }
    }
    for side in [-1, 1] {
        if k.crown == 0 && headed {
            ear_at(c, d, p, pose, ear_cx, ear_span, ear_top, ear, side);
            ear_tuft(c, d, p, ear_cx, ear_span, ear_top, ear, side);
        }
    }
    let limbs = limbs(clip, frame, body, d.body);
    let arm = if k.limbs > 0 { classic_arm } else { draw_arm };
    let shoulder = |side: i32| shoulder(body, pose, side);
    if trunk {
        // A four-pawed body stands on four legs: the far pair in shade behind the body, and the near
        // pair in front of it after the body is drawn.
        if long {
            four_legs(c, p, d, body, pose, clip, limbs, false);
        }
        if k.coat > 0 {
            lit_oval(c, p, x, y, rx, ry, p.coat);
            // A crescent of shade under a lit top, rather than a shaded lower half.
            c.fill_ellipse(
                x - 2,
                y + ry / 2,
                (rx - 2).max(2),
                (ry / 3).max(2),
                p.shadow,
            );
            c.fill_ellipse(x, y - 1, rx - 2, (ry - 2).max(2), p.coat);
        } else {
            oval(c, p, x, y, rx, ry, p.coat);
            c.fill_ellipse(x, y + ry / 2, rx - 3, (ry / 2).max(3), p.shadow);
        }
        match d.marking {
            _ if k.pattern > 0 => classic_pattern(c, p, d, x, y, rx - 1, ry - 1),
            1 => c.fill_ellipse(x, y + 3, (rx - 4).max(3), (ry - 2).max(3), p.highlight),
            2 => {
                for dx in [-5, 0, 5] {
                    c.fill_ellipse(x + dx, y + 3, 1, 3, p.accent);
                }
            }
            3 => {
                for (dx, dy) in [(-5, 1), (3, 5), (5, -2)] {
                    c.fill_circle(x + dx, y + dy, 2, p.accent);
                }
            }
            4 => c.fill_ellipse(x - 4, y + 2, 4, 4, p.accent),
            5 => c.fill_ellipse(x, y + 4, rx - 3, 2, p.accent),
            6 => {
                c.fill_circle(x - 4, y + 4, 2, p.highlight);
                c.fill_circle(x + 4, y + 4, 2, p.highlight);
            }
            _ => {}
        }
        if d.details.belly == 1 {
            let belly = crate::palette::detail_color(&d, d.details.belly_color);
            if long {
                // Along the underside, toward the chest.
                c.fill_ellipse(x + 2, y + ry / 2, (rx - 4).max(3), (ry / 3).max(2), belly);
            } else {
                c.fill_ellipse(x, y + 2, (rx - 4).max(3), (ry - 3).max(3), belly);
            }
        }
        if long {
            four_legs(c, p, d, body, pose, clip, limbs, true);
        } else {
            // Balled up in the air, the feet are drawn in under the body; leaping, drawn up off
            // the ground as well.
            let leap = airborne_feet(clip, pose);
            let tucked = i32::from(clip == BodyClip::Action(ActionKind::Tossed)) * 2 + leap.min(1);
            for ((side, step), limb) in [(-1, pose.step_a), (1, pose.step_b)].into_iter().zip(limbs)
            {
                // A knee drawn up takes its foot up and a little out; a trailing leg hangs straight.
                let knee = if side > 0 { pose.knee.clamp(0, 6) } else { 0 };
                let trail = if side < 0 { pose.trail.clamp(0, 4) } else { 0 };
                // A foot stays where it was put while the body sways over it. A stride that
                // says how high its own feet are steps a pixel further than a shuffle does.
                let reach = if pose.lifts.is_some() { 3 } else { 2 };
                let fx = x - sway(d, pose)
                    + side * (rx - 4 - tucked + pose.splay.clamp(0, 3) + knee / 2)
                    + step.clamp(-reach, reach);
                let stepped = pose.lifts.map_or(step.abs().min(2), |lifts| {
                    lifts[usize::from(side > 0)].clamp(0, 5)
                });
                let lift = stepped + knee - trail + leap;
                // Stick legs are drawn in shade, so they read as thin legs rather than coat.
                let leg = if k.limbs == 2 { p.shadow } else { p.coat };
                c.line(
                    x + side * (rx - 4),
                    y + ry - 2,
                    fx,
                    floor - 2 - lift,
                    2,
                    p.outline,
                );
                c.line(
                    x + side * (rx - 4),
                    y + ry - 2,
                    fx,
                    floor - 2 - lift,
                    1,
                    leg,
                );
                if k.limbs > 0 {
                    classic_foot(
                        c,
                        p,
                        k.limbs,
                        PixelPoint {
                            x: fx,
                            y: floor - 1 - lift,
                        },
                    );
                } else {
                    oval(c, p, fx, floor - 1 - lift, 3, 2, p.coat);
                }
                if limb == Limb::Rest {
                    let at = shoulder(side);
                    if d.body == BodyPlan::Winged {
                        // A folded wing hangs from the shoulder and its outline reaches six rows
                        // below it, so a body settling onto its legs, as a sleeper does with each
                        // breath, would carry the tip down past the feet. The wing comes to rest on
                        // the ground instead.
                        let lowest = feet_reach(d, floor) - 6;
                        draw_wing(c, p, wing_style(d), at.x, at.y.min(lowest), side);
                    } else if k.limbs > 0 {
                        classic_nub(c, p, at, side);
                    } else {
                        oval(c, p, at.x, at.y, 2, 3, p.coat);
                    }
                }
            }
        }
    }
    // A ruff goes round the neck under the head, which is drawn over the top of it.
    if d.details.belly == 2 && headed && !blob {
        let dx = if long { -1 } else { 0 };
        ruff(
            c,
            d,
            p,
            (hx + dx, hy + head - 2, head - 1),
            1,
            feet_reach(d, floor),
        );
    }
    if !blob && headed {
        if k.coat > 0 {
            lit_oval(c, p, hx, hy, head, head - 1, p.coat);
        } else {
            oval(c, p, hx, hy, head, head - 1, p.coat);
        }
    }
    if d.muzzle > 0 && headed {
        c.fill_ellipse(
            hx,
            hy + 3,
            if d.muzzle == 2 { 5 } else { 3 },
            3,
            p.highlight,
        );
    }
    // Candy coats are lit from above already, so only the modular one carries a painted gleam.
    if k.coat == 0 && headed {
        c.fill_ellipse(
            hx - 2,
            if blob { y - ry + 3 } else { hy - head + 3 },
            3,
            1,
            p.highlight,
        );
    }
    // A limb in use is the same limb carried out from where it rests, drawn last so a paw raised
    // to the face or across the chest stays in front of what it covers.
    for (side, limb) in [-1, 1].into_iter().zip(limbs) {
        let Limb::Reach(hand) = limb else {
            continue;
        };
        if !trunk {
            continue;
        }
        if long {
            // The far forepaw went up behind the body with the far legs; this is the near one.
            if side > 0 {
                raised_paw(c, p, k.limbs, raised_root(body, clip, true), hand, true);
            }
        } else if d.body == BodyPlan::Winged {
            draw_open_wing(c, p, wing_style(d), shoulder(side), hand, side);
        } else {
            arm(c, p, shoulder(side), hand);
        }
    }
    PixelPoint { x: hx, y: hy }
}

/// The tail, out behind the body from `tx`, the body's back edge, level with its middle at `y`.
/// It goes behind the body and never through the reserved face area.
fn tail_behind(
    c: &mut Canvas,
    d: CreatureDesign,
    p: Palette,
    pose: Pose,
    tx: i32,
    y: i32,
    ry: i32,
) {
    if d.classic.tail > 0 {
        classic_tail(c, p, d.classic.tail, tx, y, pose);
        return;
    }
    match d.tail {
        1 => oval(c, p, tx - 2, y + 1, 3, 3, p.highlight),
        // A stalk out behind from the rump, below where the paws rest, rising a row or two to
        // its end, in the coat's shade with only the tip in the accent. Held up from the
        // shoulder at the angle of a raised arm, in the accent a paw is tipped with, it read
        // as a paw waving all day. Its root starts inside the body, which covers it.
        2 | 3 if d.details.tip == 4 => {
            let tip = crate::palette::detail_color(&d, d.details.tip_color);
            let long = d.body == BodyPlan::Long;
            plume(c, p, tip, tx, y, ry, pose.tail_sway.clamp(-1, 1), long);
        }
        2 | 3 => {
            let root = PixelPoint {
                x: tx + 2,
                y: y + ry / 2,
            };
            // A row on a small body, whose head sits down beside its middle; two on a big one.
            let ty = root.y - (ry / 4).clamp(1, 2) + pose.tail_sway.clamp(-1, 1);
            c.line(root.x, root.y, (tx - 6).max(3), ty, 3, p.outline);
            c.line(root.x, root.y, (tx - 6).max(3), ty, 2, p.shadow);
            if d.details.tip > 0 {
                let tip = crate::palette::detail_color(&d, d.details.tip_color);
                // A bobble is a pixel rounder than a flame, so it keeps a pixel further in
                // from the edge of the frame, and a brush, longer, further still.
                let end = (tx - 6).max(match d.details.tip {
                    2 => 5,
                    3 => 6,
                    _ => 4,
                });
                tail_tip(c, p, d.details.tip, tip, end, ty);
            } else if d.tail == 3 {
                oval(c, p, (tx - 6).max(4), ty, 2, 2, p.accent);
            } else {
                c.fill_circle((tx - 6).max(3), ty, 1, p.accent);
            }
        }
        4 => oval(c, p, tx - 2, y, 4, 5, p.accent),
        _ => {}
    }
}

/// One ear, `ear` rows tall, on the `side` of a head whose ears are rooted `ear_span` either side
/// of `ear_cx` on the row `ear_top`.
#[allow(clippy::too_many_arguments)]
fn ear_at(
    c: &mut Canvas,
    d: CreatureDesign,
    p: Palette,
    pose: Pose,
    ear_cx: i32,
    ear_span: i32,
    ear_top: i32,
    ear: i32,
    side: i32,
) {
    let ex = ear_cx + side * ear_span;
    let ey = ear_top;
    match d.ears {
        EarStyle::None => {}
        // A round ear has no height to grow, so it rides up instead, staying on the head.
        EarStyle::Round => oval(c, p, ex, ey - 1 - pose.ear_perk.clamp(0, 2), 3, 3, p.accent),
        EarStyle::Long => oval(c, p, ex, ey - ear / 2, 2, ear.min(6), p.coat),
        EarStyle::Floppy => oval(
            c,
            p,
            ear_cx + side * (ear_span + 3),
            ey + 4,
            2,
            ear,
            p.accent,
        ),
        EarStyle::Pointed | EarStyle::Tuft => {
            let top = ey - ear;
            for row in 0..=ear {
                let half = if d.ears == EarStyle::Tuft {
                    row / 3
                } else {
                    row / 2
                };
                c.fill_rect(ex - half, top + row, half * 2 + 1, 1, p.outline);
                if half > 0 {
                    c.fill_rect(ex - half + 1, top + row, half * 2 - 1, 1, p.accent);
                }
            }
        }
    }
}

/// A small wing folded up off the back, reaching up and out to `side`: a membrane over two ribs,
/// or three rounded feathers. Every outline goes down before any fill, so the parts read as one
/// shape. The far wing of a four-pawed body is drawn in shade behind the near one.
fn wing_nub(c: &mut Canvas, p: Palette, style: u8, root: PixelPoint, side: i32, near: bool) {
    let fill = if near { p.accent } else { p.shadow };
    let lobes: [(i32, i32, i32, i32); 3] = if style == 2 {
        [(2, -1, 3, 2), (4, -4, 2, 2), (6, -6, 1, 2)]
    } else {
        [(2, -1, 3, 3), (4, -5, 2, 3), (6, -8, 1, 2)]
    };
    let at = |dx: i32, dy: i32| (root.x + side * dx, root.y + dy);
    for (dx, dy, rx, ry) in lobes {
        let (cx, cy) = at(dx, dy);
        c.fill_ellipse(cx, cy, rx + 1, ry + 1, p.outline);
    }
    for (dx, dy, rx, ry) in lobes {
        let (cx, cy) = at(dx, dy);
        c.fill_ellipse(cx, cy, rx, ry, fill);
    }
    if !near {
        return;
    }
    let (tip_x, tip_y) = at(6, -8);
    if style == 2 {
        // Feathered: a lit leading edge and a line between each feather.
        let (lx, ly) = at(1, -3);
        c.line(lx, ly, tip_x - side, tip_y + 1, 1, p.highlight);
        for (dx, dy) in [(3, -1), (4, -4)] {
            let (fx, fy) = at(dx, dy);
            c.line(fx, fy, fx + side, fy + 1, 1, p.shadow);
        }
    } else {
        // Membrane: two ribs from the root out to the trailing edge.
        let (rx0, ry0) = at(1, -2);
        for (dx, dy) in [(4, 1), (5, -4)] {
            let (ex, ey) = at(dx, dy);
            c.line(rx0, ry0, ex, ey, 1, p.shadow);
        }
    }
}

/// A horn on top of the head, its root hidden under the head drawn after it: a short nub, or a
/// longer horn swept back and out to `side`. Horns are drawn pale, the way horn reads against a
/// coat of any colour.
fn horn(c: &mut Canvas, p: Palette, shade: Rgba, style: u8, root: PixelPoint, side: i32) {
    if style == 3 {
        antler(c, p, shade, root, side);
        return;
    }
    let rows = if style == 2 { 6 } else { 4 };
    for row in 0..=rows {
        // Widest at the root, a point at the tip; a swept horn leans out as it rises.
        let half = (row + 1) / 2;
        let lean = if style == 2 {
            side * (rows - row) / 2
        } else {
            0
        };
        let cx = root.x + lean;
        let cy = root.y - rows + row;
        c.fill_rect(cx - half - 1, cy, half * 2 + 3, 1, p.outline);
        if row > 0 {
            c.fill_rect(cx - half, cy, half * 2 + 1, 1, shade);
        }
    }
    c.fill_rect(
        root.x + if style == 2 { side * 2 } else { 0 },
        root.y - rows,
        1,
        1,
        p.outline,
    );
}

/// What a horn is drawn in: the belly's cream where there is one, and otherwise the pale of the
/// coat's highlight.
fn horn_shade(d: CreatureDesign, p: Palette) -> Rgba {
    if d.details.belly > 0 {
        crate::palette::detail_color(&d, d.details.belly_color)
    } else {
        p.highlight
    }
}

/// A little antler rooted under the head: a single-pixel beam rising and leaning out to `side`,
/// with one tine forking up and off it toward the outside, filled in `shade` and outlined once all
/// round, so two of them stand clear of each other on top of the head rather than clumping.
fn antler(c: &mut Canvas, p: Palette, shade: Rgba, root: PixelPoint, side: i32) {
    const FILL: [(i32, i32); 9] = [
        (0, 0),
        (0, -1),
        (0, -2),
        (1, -3),
        (1, -4),
        (1, -5),
        (1, -6),
        (2, -4),
        (3, -5),
    ];
    let at = |(dx, dy): (i32, i32)| (root.x + side * dx, root.y + dy);
    let filled: Vec<(i32, i32)> = FILL.into_iter().map(at).collect();
    for &(x, y) in &filled {
        for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if !filled.contains(&(nx, ny)) {
                c.set(nx, ny, p.outline);
            }
        }
    }
    for (x, y) in filled {
        c.set(x, y, shade);
    }
}

/// The tip of an ear [`ear_at`] drew at column `ex`, for an ear that comes to one.
fn ear_tip(d: CreatureDesign, ex: i32, ear_top: i32, ear: i32) -> Option<PixelPoint> {
    match d.ears {
        EarStyle::Pointed | EarStyle::Tuft => Some(PixelPoint {
            x: ex,
            y: ear_top - ear,
        }),
        EarStyle::Long => Some(PixelPoint {
            x: ex,
            y: ear_top - ear / 2 - ear.min(6) - 1,
        }),
        _ => None,
    }
}

/// A dark tuft of fur standing up from the tip of each ear that comes to one, three pixels tall
/// and leaning a little out, the way a lynx's does. Only a recipe with ear tufts has one.
#[allow(clippy::too_many_arguments)]
fn ear_tuft(
    c: &mut Canvas,
    d: CreatureDesign,
    p: Palette,
    ear_cx: i32,
    ear_span: i32,
    ear_top: i32,
    ear: i32,
    side: i32,
) {
    if d.details.horns != 4 {
        return;
    }
    let Some(tip) = ear_tip(d, ear_cx + side * ear_span, ear_top, ear) else {
        return;
    };
    // Shorter on an ear already reaching the top of its frame, which nothing may touch.
    for (dx, up) in [(0, 1), (0, 2), (side, 3)] {
        if tip.y - up >= 1 {
            c.set(tip.x + dx, tip.y - up, p.outline);
        }
    }
}

/// A soft collar of fur round the neck, sitting on the shoulders under the head: wider than the
/// head, its edge broken into uneven tufts that hang lower in some places than others and poke out
/// at its ends, in a shade between the coat's light and the belly colour, so it reads as more of
/// the coat grown fluffy rather than something worn. Placed for a head whose lower edge is near `y`
/// and which is `half` wide either side of `x`. `way` is 1 as it faces the screen and side-on,
/// -1 from behind, and puts the light on the side it comes from; nothing goes below `lowest`.
fn ruff(
    canvas: &mut Canvas,
    d: CreatureDesign,
    p: Palette,
    (x, y, half): (i32, i32, i32),
    way: i32,
    lowest: i32,
) {
    // Painted on its own and put down no lower than the feet, so a body crouched low keeps its
    // ruff above the ground it stands on.
    let mut layer = Canvas::new(canvas.width(), canvas.height());
    let c = &mut layer;
    let belly = crate::palette::detail_color(&d, d.details.belly_color);
    let coat_light = blend(p.coat, p.highlight);
    let fur = blend(coat_light, belly);
    let shade = mix_toward(fur, p.shadow);
    let half = half.max(3) + 1;
    let y = y + 1;
    // Tufts along the lower edge, hanging one or two rows, and one out past each end.
    let tufts: Vec<(i32, i32, i32)> = (-half + 1..=half - 1)
        .step_by(2)
        .enumerate()
        .map(|(index, dx)| {
            (
                dx,
                2 + (index as i32 % 3).min(1),
                1 + i32::from(index % 2 == 0),
            )
        })
        .chain([(-half, 1, 1), (half, 1, 1)])
        .collect();
    c.fill_ellipse(x, y, half + 1, 3, p.outline);
    for &(dx, drop, r) in &tufts {
        c.fill_circle(x + dx, y + drop, r + 1, p.outline);
    }
    c.fill_ellipse(x, y, half, 2, fur);
    for &(dx, drop, r) in &tufts {
        c.fill_circle(x + dx, y + drop, r, fur);
    }
    // Shade under each hanging tuft, and a few strands of light on the side the light comes from.
    for &(dx, drop, r) in &tufts {
        if drop > 0 {
            c.set(x + dx, y + drop + r, shade);
        }
    }
    let lit = lighter(fur);
    for step in [half - 2, half - 5] {
        c.set(x - way * step, y - 1, lit);
        c.set(x - way * step + way, y, lit);
    }
    for row in 0..=lowest.min(canvas.height() as i32 - 1) {
        for column in 0..canvas.width() as i32 {
            let pixel = layer.get(column, row);
            if pixel.a > 0 {
                canvas.set(column, row, pixel);
            }
        }
    }
}

/// Halfway between two colours.
fn blend(a: Rgba, b: Rgba) -> Rgba {
    let mid = |x: u8, y: u8| ((u16::from(x) + u16::from(y)) / 2) as u8;
    Rgba::new(mid(a.r, b.r), mid(a.g, b.g), mid(a.b, b.b), 255)
}

/// `color` a third of the way to `toward`.
fn mix_toward(color: Rgba, toward: Rgba) -> Rgba {
    let mix = |a: u8, b: u8| ((u16::from(a) * 2 + u16::from(b)) / 3) as u8;
    Rgba::new(
        mix(color.r, toward.r),
        mix(color.g, toward.g),
        mix(color.b, toward.b),
        color.a,
    )
}

/// A whole tail that is a soft plume, curled up behind the back from the rump at `tx`, the body's
/// back edge, with its end turned over in `tip`: a big oval of coat with a smaller one at its
/// top, both outlined before either is filled so they read as one, and fur marked down its length
/// in shade. It leans with `sway`.
#[allow(clippy::too_many_arguments)]
fn plume(c: &mut Canvas, p: Palette, tip: Rgba, tx: i32, y: i32, ry: i32, sway: i32, long: bool) {
    // A four-pawed body is long already: its plume sits closer in, reaching no further out behind
    // it than a stalk tail does, which keeps it in its frame when the body is turned upright
    // against a wall.
    let px = if long { tx - 1 } else { tx - 3 }.max(6);
    let tall = (ry - 1).clamp(4, 7);
    let (cx, cy) = (px + 1 + sway, y - tall - 1);
    c.fill_ellipse(px, y, 5, tall + 1, p.outline);
    c.fill_ellipse(cx, cy.max(5), 4, 4, p.outline);
    c.fill_ellipse(px, y, 4, tall, p.coat);
    c.fill_ellipse(px - 1, y + tall / 2, 2, (tall / 2).max(1), p.shadow);
    c.fill_ellipse(cx, cy.max(5), 3, 3, tip);
    c.set(cx - 1, cy.max(5) - 2, lighter(tip));
    for row in [y - tall / 2, y, y + tall / 2 - 1] {
        c.set(px - 2, row, p.shadow);
        c.set(px - 3, row + 1, p.shadow);
    }
}

/// A plume seen from behind, standing up the middle of the back from the rump at `x`, over the
/// body: an upright oval of coat, shaded down one side and marked with fur, its curled-over end in
/// `tip` at the top.
fn plume_from_behind(c: &mut Canvas, p: Palette, tip: Rgba, x: i32, y: i32, ry: i32) {
    let tall = (ry - 1).clamp(4, 7);
    let (cx, cy) = (x, y + ry / 2 - tall);
    c.fill_ellipse(cx, cy, 4, tall + 1, p.outline);
    c.fill_ellipse(cx, cy - tall + 1, 3, 3, p.outline);
    c.fill_ellipse(cx, cy, 3, tall, p.coat);
    c.fill_ellipse(cx + 2, cy + 1, 1, (tall - 2).max(1), p.shadow);
    c.fill_ellipse(cx, cy - tall + 1, 2, 2, tip);
    c.set(cx - 1, cy - tall, lighter(tip));
    for row in [cy - 1, cy + 2] {
        c.set(cx - 1, row, p.shadow);
    }
}

/// The tip of a tapering tail: a flame flickering up from it, a round bobble, or a fluffy brush.
fn tail_tip(c: &mut Canvas, p: Palette, style: u8, color: Rgba, x: i32, y: i32) {
    if style == 3 {
        // Longer than it is tall, along the tail, with a tuft at its very end and fur marked
        // across it.
        c.fill_ellipse(x - 1, y, 4, 3, p.outline);
        c.set(x - 5, y - 1, p.outline);
        c.fill_ellipse(x - 1, y, 3, 2, color);
        c.set(x - 4, y, color);
        c.set(x, y - 1, lighter(color));
        c.set(x + 1, y + 1, mix_toward(color, p.shadow));
        return;
    }
    if style == 2 {
        oval(c, p, x, y - 1, 3, 3, color);
        c.fill_rect(x - 1, y - 3, 2, 1, lighter(color));
        return;
    }
    oval(c, p, x, y - 2, 2, 3, color);
    oval(c, p, x + 1, y - 5, 1, 2, color);
    c.fill_ellipse(x, y - 1, 2, 2, color);
    // A bright core low in the flame.
    c.fill_rect(x, y - 2, 1, 2, lighter(color));
}

fn lighter(color: Rgba) -> Rgba {
    let lift = |v: u8| ((u16::from(v) + 255 * 2) / 3) as u8;
    Rgba::new(lift(color.r), lift(color.g), lift(color.b), color.a)
}

/// A mass lit from above, the way the original bodies are drawn: the fill is carried a row up
/// inside its outline, so the top edge meets the light with no ink across it and the underside
/// sits on a heavier line. The silhouette is exactly the outlined one, so everything measured
/// against it, from the reserved face to the spacing boxes, holds as it does for a plain oval.
fn lit_oval(c: &mut Canvas, p: Palette, x: i32, y: i32, rx: i32, ry: i32, color: Rgba) {
    c.fill_ellipse(x, y, rx + 1, ry + 1, p.outline);
    c.fill_ellipse(x, y - 1, rx, ry, color);
}

/// A foot of the original kind, pointing the way the creature faces: a small accent cross in a
/// dark rim, or under a stick leg a longer, forked one.
fn classic_foot(c: &mut Canvas, p: Palette, limbs: u8, at: PixelPoint) {
    let (rim, fork) = if limbs == 2 { (4, 3) } else { (3, 2) };
    c.fill_ellipse(at.x, at.y, rim, 2, p.outline);
    c.fill_ellipse(at.x + 1, at.y, fork, 1, p.accent);
}

/// A small paw of the original kind, folded at the side: a short stub from the shoulder ending in
/// an accent tip. It covers the same spot beside the body a modular paw does, so raising it
/// leaves that spot empty exactly as raising a modular paw would.
fn classic_nub(c: &mut Canvas, p: Palette, at: PixelPoint, side: i32) {
    let tip = PixelPoint {
        x: at.x + side,
        y: at.y + 2,
    };
    c.line(at.x - side, at.y, tip.x, tip.y, 2, p.outline);
    c.fill_circle(tip.x, tip.y, 2, p.outline);
    c.line(at.x - side, at.y, tip.x, tip.y, 1, p.coat);
    c.fill_circle(tip.x, tip.y, 1, p.accent);
}

/// The same paw carried out: a thin arm to an accent tip, its root filled back into the body.
fn classic_arm(c: &mut Canvas, p: Palette, root: PixelPoint, hand: PixelPoint) {
    c.line(root.x, root.y, hand.x, hand.y, 2, p.outline);
    c.fill_circle(hand.x, hand.y, 2, p.outline);
    c.line(root.x, root.y, hand.x, hand.y, 1, p.coat);
    c.fill_circle(hand.x, hand.y, 1, p.accent);
    c.fill_circle(root.x, root.y, 1, p.coat);
}

/// Antennae or sprouts in place of ears, rooted where the ears would be so they stay on the head.
/// Antenna tips bob against each other as the body moves.
#[allow(clippy::too_many_arguments)]
fn classic_crown(
    c: &mut Canvas,
    p: Palette,
    crown: u8,
    cx: i32,
    span: i32,
    top: i32,
    reach: i32,
    pose: Pose,
) {
    for side in [-1, 1] {
        let root = PixelPoint {
            x: cx + side * (span - 1),
            y: top + 1,
        };
        if crown == 1 {
            let tip = PixelPoint {
                x: cx + side * (span + 1),
                y: (top - reach - side * pose.bob.clamp(-1, 1)).max(3),
            };
            c.line(root.x, root.y, tip.x, tip.y, 1, p.outline);
            c.fill_circle(tip.x, tip.y, 1, p.accent);
        } else {
            let tip = PixelPoint {
                x: cx + side * (span + 3),
                y: (top - reach + 1).max(3),
            };
            c.line(root.x, root.y + 1, tip.x, tip.y, 2, p.outline);
            c.line(root.x, root.y, tip.x, tip.y + 1, 1, p.accent);
        }
    }
}

/// A tail of the original kind: a thin dark stalk ending in an open curl, or in an accent star.
fn classic_tail(c: &mut Canvas, p: Palette, tail: u8, tx: i32, y: i32, pose: Pose) {
    let sway = pose.tail_sway.clamp(-2, 2);
    if tail == 1 {
        let tip = PixelPoint {
            x: (tx - 5).max(4),
            y: y - 2 + sway,
        };
        c.line(tx, y + 2, tip.x, tip.y, 2, p.outline);
        c.fill_circle(tip.x, tip.y - 2, 3, p.outline);
        c.fill_circle(tip.x, tip.y - 2, 1, Rgba::TRANSPARENT);
    } else {
        // The star rides higher and further out, so its stalk clears the body it grows from.
        let tip = PixelPoint {
            x: (tx - 6).max(3),
            y: y - 5 + sway,
        };
        c.line(tx, y + 2, tip.x, tip.y, 2, p.outline);
        c.fill_circle(tip.x, tip.y, 2, p.outline);
        c.fill_circle(tip.x, tip.y, 1, p.accent);
    }
}

/// Stripes, spots, or patches of accent across the body, as the original coats wore them. They
/// are placed from the recipe's own bytes, so a creature's pattern never moves, and like the
/// originals a patch near the edge may run over the outline rather than stop short of it.
fn classic_pattern(
    c: &mut Canvas,
    p: Palette,
    d: CreatureDesign,
    x: i32,
    y: i32,
    rx: i32,
    ry: i32,
) {
    if rx <= 2 || ry <= 2 {
        return;
    }
    if d.classic.pattern == 1 {
        for offset in (-rx + 3..rx - 2).step_by(4) {
            for row in y - ry..=y + ry {
                let column = x + offset + (row - y).div_euclid(4);
                if inside_ellipse(column, row, x, y, rx, ry) {
                    c.set(column, row, p.accent);
                }
            }
        }
        return;
    }
    let mut seed = [0_u8; 32];
    seed[..20].copy_from_slice(&d.to_bytes());
    let mut rng = ChaCha12Rng::from_seed(seed);
    let (count, radius) = if d.classic.pattern == 2 {
        (rng.random_range(3..=6), 1)
    } else {
        (rng.random_range(2..=3), 3)
    };
    for _ in 0..count {
        let spot_x = rng.random_range(x - rx + 2..=x + rx - 2);
        let spot_y = rng.random_range(y - ry + 2..=y + ry - 2);
        if inside_ellipse(spot_x, spot_y, x, y, rx, ry) {
            c.fill_circle(spot_x, spot_y, radius, p.accent);
        }
    }
}

/// How low what this body holds may go: the floor it stands on, and the lowest row its feet
/// reach. Where it holds things is read from the frame itself (see [`figure`]).
pub(super) fn footing(design: CreatureDesign, pose: Pose, size: f32) -> (i32, i32) {
    let d = design.bounded();
    let floor = measure(d, pose, size).floor;
    (floor, feet_reach(d, floor))
}

/// Where the near forepaw that `draw` draws in this frame holds things: the hand of a paw in use,
/// or the paw folded at its side, on the drawn paw either way. A folded wing holds nothing, and
/// a four-pawed body standing on all four has no paw free, so for those it is `None` and what
/// it carries goes in its mouth.
fn near_hand(
    d: CreatureDesign,
    body: Body,
    pose: Pose,
    clip: BodyClip,
    frame: u8,
) -> Option<PixelPoint> {
    let [far, near] = limbs(clip, frame, body, d.body);
    let folded = matches!(d.body, BodyPlan::Long | BodyPlan::Winged);
    match (near, far) {
        // A raised paw, a carried-out arm and an opened wing all end on the point they reach.
        (Limb::Reach(hand), _) => Some(hand),
        // A four-pawed body using its far forepaw for a beat holds with that one meanwhile,
        // rather than dropping what it holds into its mouth and taking it back.
        (Limb::Rest, Limb::Reach(hand)) if folded => Some(hand),
        (Limb::Rest, _) if folded => None,
        (Limb::Rest, _) => {
            let at = shoulder(body, pose, 1);
            Some(if d.classic.limbs > 0 {
                // The accent tip of a folded classic nub.
                PixelPoint {
                    x: at.x + 1,
                    y: at.y + 2,
                }
            } else {
                // The lower front of the folded paw's oval, on its edge, where a thing held
                // against the body rests on it.
                PixelPoint {
                    x: at.x + 1,
                    y: at.y + 1,
                }
            })
        }
    }
}

/// Where this body's parts are in one frame, read from the same measurements `draw` places them
/// by, for whatever it is wearing.
pub(super) fn figure(
    design: CreatureDesign,
    pose: Pose,
    size: f32,
    clip: BodyClip,
    frame: u8,
) -> Figure {
    standing_figure(design.bounded(), pose, size, clip, frame)
}

/// [`figure`], for a design already bounded.
fn standing_figure(d: CreatureDesign, pose: Pose, size: f32, clip: BodyClip, frame: u8) -> Figure {
    // Upright against a wall, the trunk is turned and the head is carried level over it.
    let turned = (d.body == BodyPlan::Long && pose.wall).then(|| {
        let standing = measure(d, Pose { lean: 0, ..pose }, size);
        (standing, upright(d, standing, pose))
    });
    let body = turned.map_or_else(|| measure(d, pose, size), |(standing, _)| standing);
    let hand = near_hand(d, body, pose, clip, frame);
    let Body {
        x,
        y,
        rx,
        ry,
        hx,
        hy,
        head,
        floor,
    } = body;
    let place = |at: PixelPoint| turned.map_or(at, |(_, place)| place.turn(at));
    let (hx, hy) = turned.map_or((hx, hy), |(_, place)| (place.head.x, place.head.y));
    let face = PixelPoint { x: hx, y: hy };
    let hip = place(PixelPoint {
        x: x - rx + 2,
        y: y + ry / 3,
    });
    let back = place(PixelPoint {
        x: x - rx / 2 - 1,
        y: y - ry / 2,
    });
    let floor = feet_reach(d, floor) - 1;
    if d.body == BodyPlan::Blob {
        // One mass with the face on it: the collar goes round the mass just below the face.
        let neck_y = (hy + 5).min(y + ry - 2);
        return Figure {
            face,
            crown: PixelPoint { x, y: y - ry - 1 },
            head_half: (rx - 3).max(3),
            neck: PixelPoint { x, y: neck_y },
            neck_half: super::classic::ellipse_half_width(rx, ry, neck_y - y),
            chest: PixelPoint {
                x: x + rx / 2,
                y: neck_y + 2,
            },
            hip,
            back,
            floor,
            hand,
        };
    }
    Figure {
        face,
        crown: PixelPoint {
            x: hx,
            y: hy - head,
        },
        head_half: head,
        neck: PixelPoint {
            x: hx,
            y: hy + head - 1,
        },
        neck_half: (head - 2).max(3),
        chest: PixelPoint {
            x: hx + 1,
            y: (hy + head + 1).min(y + ry - 1),
        },
        hip,
        back,
        floor,
        hand,
    }
}

/// One side's paw or wing in a frame: folded where it rests, or carried out to a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Limb {
    Rest,
    Reach(PixelPoint),
}

/// The measurements every part of a modular body is placed from, in frame pixels.
#[derive(Clone, Copy, Debug)]
pub(super) struct Body {
    /// Centre and radii of the body mass.
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) rx: i32,
    pub(super) ry: i32,
    /// Centre and radius of the head, which is also the face anchor.
    pub(super) hx: i32,
    pub(super) hy: i32,
    pub(super) head: i32,
    /// The row the feet stand on.
    pub(super) floor: i32,
}

fn measure(d: CreatureDesign, pose: Pose, size: f32) -> Body {
    let long = d.body == BodyPlan::Long;
    let blob = d.body == BodyPlan::Blob;
    let size = drawn_size(d, size);
    let mut rx = ((f32::from(d.width) * size).round() as i32 + pose.squash_x).clamp(6, 13);
    let mut ry = ((f32::from(d.height) * size).round() as i32 + pose.squash_y).clamp(5, 11);
    if blob {
        rx = (rx + 2).clamp(8, 13);
        ry = (ry + 2).clamp(7, 11);
    }
    // A four-pawed body is long and low, its rump well behind the head, so it reads as standing
    // on all four rather than sitting up on two.
    if long {
        rx = (rx + 2).clamp(8, 12);
        ry = (ry - 1).clamp(5, 9);
    }
    let head = (f32::from(d.head) * size.sqrt()).round().max(7.0) as i32;
    let x = if long {
        20 + pose.forward.clamp(0, 9)
    } else {
        24 + sway(d, pose)
    };
    // The floor is the same row in every clip. A bob or a lift moves the body over feet that
    // stay on it, the way a crouch already does, so a companion that sets off walking keeps the
    // ground it was standing on instead of rising a pixel or two above it for the length of the
    // walk and dropping back when it stops.
    let floor = 40;
    // Blobs settle onto stubby feet rather than standing on visible legs, and stick legs lift
    // every body higher, a blob most of all, since it otherwise barely shows a leg.
    let stilts = d.classic.limbs == 2;
    let stance = f32::from(d.legs)
        * size
        * match (blob, stilts) {
            (true, false) => 0.4,
            (true, true) => 1.1,
            // Four short legs rather than two long ones: a critter's body rides low.
            (false, false) if long => 0.8,
            (false, false) => 1.0,
            (false, true) => 1.35,
        };
    // A crouch folds the legs rather than sinking the feet, so contact with the surface holds.
    let stance = (stance.round() as i32 - pose.crouch.clamp(0, 4)).max(0);
    // A settle can only fold the legs it has: a plan that already stands low sinks less rather
    // than pressing its body down through the floor it is standing on.
    let sink = pose.bob.clamp(-2, 2).min(stance);
    let y = floor - stance - ry + sink - pose.play_lift.clamp(0, 3);
    // A large four-pawed body carries its head no further forward than the largest did before
    // statures, so a face turned to watch something stays inside the box the simulation keeps
    // faces clear by; the head itself still grows.
    let hx = if long {
        // A trunk carried forward over a ledge goes forward under a head that stays where it is,
        // so the face keeps inside the same box.
        x - pose.forward.clamp(0, 9) + 2 + (8.0 * size.min(1.05)).round() as i32
    } else {
        x
    } + if blob {
        // A blob has no head to move: its face rides the one mass it is, so carrying that face
        // more than a pixel slides it off the shape that is supposed to be holding it.
        pose.lean.clamp(-1, 1)
    } else {
        pose.lean.clamp(-3, 3)
    };
    let hy = match d.body {
        BodyPlan::Round => y - 2,
        // Held up off the chest, so the forelegs stand under the body rather than the chin.
        BodyPlan::Long => y - 7,
        BodyPlan::Upright | BodyPlan::Winged => y - 6,
        BodyPlan::Blob => y - ry / 3,
    }
    .max(18);
    // A head larger than a body that a crouch has squashed flat would reach past the feet, since
    // it hangs from the body's centre. Like everything else, it stops at the ground; its outline
    // reaches `head` rows below its centre.
    let hy = hy.min(feet_reach(d, floor) - head);
    // A floppy or round ear hangs three pixels out from the side of the head, so a large head
    // carried to the front of a long body, or leaning, stops where its ears still clear the
    // edge of the frame.
    let hx = hx.clamp(head + 4, FRAME_SIZE as i32 - 5 - head);
    Body {
        x,
        y,
        rx,
        ry,
        hx,
        hy,
        head,
        floor,
    }
}

/// How far a body has gone over to one side of its feet: a pixel at most, only ever asked for by
/// a stride (see `Pose::sway`). A four-pawed body stands between its feet and does not.
fn sway(d: CreatureDesign, pose: Pose) -> i32 {
    if d.body == BodyPlan::Long {
        0
    } else {
        pose.sway.clamp(-1, 1)
    }
}

/// The lowest row a body's feet reach: one below the floor for the classic feet, and two for the
/// rounder modular ones. Nothing a body folds against itself goes lower.
fn feet_reach(d: CreatureDesign, floor: i32) -> i32 {
    floor + if d.classic.limbs > 0 { 1 } else { 2 }
}

/// Where a side's paw or wing sits folded against the body, and where it is carried out from.
fn shoulder(body: Body, pose: Pose, side: i32) -> PixelPoint {
    // Swung with a stride: the front paw forward and up as the back one goes back and down.
    let swing = pose.swing.clamp(-2, 2);
    PixelPoint {
        x: body.x + side * (body.rx - 1) + swing.signum(),
        y: body.y + 3 - pose.appendage_lift.clamp(-2, 3) - side * swing,
    }
}

/// Where each side's limb is this frame, back side first. There is exactly one limb per side:
/// the paw or wing a creature rests against its body is the one it raises, so no gesture ever
/// adds an appendage beside the one that was already there.
pub(super) fn limbs(clip: BodyClip, frame: u8, body: Body, plan: BodyPlan) -> [Limb; 2] {
    if plan == BodyPlan::Long {
        return long_limbs(clip, frame, body);
    }
    let Body {
        x,
        y,
        rx,
        hx,
        hy,
        head,
        floor,
        ..
    } = body;
    let tick = i32::from(frame % 2);
    let beat = [0, 1, 0, -1][usize::from(frame % 4)];
    let to = |x: i32, y: i32| {
        Limb::Reach(PixelPoint {
            x: x.clamp(4, 43),
            y: y.clamp(3, 44),
        })
    };
    let beside_head = |side: i32| hx + side * (head + 3);
    let both = |aim: &dyn Fn(i32) -> Limb| [aim(-1), aim(1)];
    let front = |limb: Limb| [Limb::Rest, limb];
    match clip {
        // Dangling hands share the existing y=7 ledge anchor.
        BodyClip::Action(ActionKind::Dangle) => both(&|side| to(beside_head(side), 7)),
        // A recipe body is drawn turned to the wall for these (see `turned::clinging_body`); the
        // fox, which places its paws from here, still climbs facing the screen.
        // A stroke with one paw and then the other (see `Pose::climb`): the reaching paw goes
        // high up beside the head while the other holds at the cheek; on the pull the reached paw
        // comes down to the head as the body rises to it, and on the settle the two meet halfway,
        // ready for the other paw to reach.
        BodyClip::Action(ActionKind::ClimbWindow) => {
            // A blob has no head to reach above: its paws go up over the top of the one mass it
            // is, from further down its side.
            let from = if plan == BodyPlan::Blob {
                hy.min(y - body.ry + 6)
            } else {
                hy
            };
            let (lead, other) = climb_grip(frame, from);
            let reaching = if frame % 6 < 3 { 1 } else { -1 };
            both(&|side| {
                let at = if side == reaching { lead } else { other };
                to(at.x, at.y)
            })
        }
        BodyClip::Action(ActionKind::PresentDiscovery) => {
            both(&|side| to(beside_head(side), hy - 5))
        }
        BodyClip::Action(
            ActionKind::Greet | ActionKind::SocialPlay | ActionKind::InvestigateCursor,
        ) => front(to(beside_head(1), hy - 2 - tick * 2)),
        // Keep-up with the near paw (or wing), in time with the toy it keeps up: held out low to
        // catch it on the crouch, out at the chin to knock it up, up beside the head on the
        // stretch, and coming back down.
        BodyClip::Action(ActionKind::SoloPlay) => front(match frame % 4 {
            0 => to(x + rx + 3, y + 2),
            1 => to(x + rx + 4, hy + 1),
            2 => to(beside_head(1), hy - head + 2),
            _ => to(x + rx + 4, y - 1),
        }),
        // Carried, the paws hang down the sides; folded wings stay folded.
        BodyClip::Action(ActionKind::Dragged) if plan != BodyPlan::Winged => {
            both(&|side| to(x + side * rx, y + body.ry - 1))
        }
        BodyClip::Action(_) => [Limb::Rest; 2],
        BodyClip::Gesture(Gesture::Cheer) => {
            both(&|side| to(beside_head(side), hy - head - 2 - tick))
        }
        // In the air, wings spread, beating down as it comes down.
        BodyClip::Gesture(Gesture::Leap) if plan == BodyPlan::Winged => {
            let beat = [-6, -4, -5, -2][usize::from(frame.min(3))];
            both(&|side| to(x + side * (rx + 5), y + beat))
        }
        // Thrown up with the spring, drawn in at the top, out to the sides for balance on the
        // way down, and lower and wider still to meet the ground.
        BodyClip::Gesture(Gesture::Leap) => match frame.min(3) {
            0 => both(&|side| to(x + side * (rx + 2), y - 6)),
            1 => [Limb::Rest; 2],
            2 => both(&|side| to(x + side * (rx + 5), y - 2)),
            _ => both(&|side| to(x + side * (rx + 4), y + 1)),
        },
        BodyClip::Gesture(Gesture::Gasp) => both(&|side| to(x + side * (rx + 5), y - 5 - tick)),
        BodyClip::Gesture(Gesture::Cover) => {
            both(&|side| to(hx + side * 3, hy - 1 + if side > 0 { tick * 2 } else { 0 }))
        }
        // Paws wrung together below the chin, clear of the mouth.
        BodyClip::Gesture(Gesture::Worry) => [to(hx - 2, y + 6 + beat), to(hx + 2, y + 5 - beat)],
        // Wings fold; everyone else plants their paws.
        BodyClip::Gesture(Gesture::Crouch) if plan == BodyPlan::Winged => [Limb::Rest; 2],
        BodyClip::Gesture(Gesture::Crouch) => both(&|side| to(x + side * (rx + 1), floor - 2)),
        BodyClip::Gesture(Gesture::Heave) => {
            let pull = [0, 1, 2, 1][usize::from(frame % 4)];
            [to(x + rx + 3 - pull, y + 3), to(x + rx + 6 - pull, y + 1)]
        }
        BodyClip::Gesture(Gesture::Balance) => {
            both(&|side| to(x + side * (rx + 6), y - 1 - side * beat * 2))
        }
        BodyClip::Gesture(Gesture::Reach) => front(to(x + rx + 7, hy - 3 - tick)),
        // Watching gathers a body instead of putting a part of it out. Both limbs stay folded and
        // ride high on the shoulders the pose lifts, which is what tells it apart from the reach
        // it would otherwise be mistaken for: a raised paw at this size only ever reads as
        // pointing, and pointing is a different thing to say.
        BodyClip::Gesture(Gesture::Watch) => [Limb::Rest; 2],
        BodyClip::Gesture(Gesture::Bop) => match frame % 4 {
            0 => front(to(beside_head(1), hy - 4)),
            2 => [to(beside_head(-1), hy - 4), Limb::Rest],
            _ => [Limb::Rest; 2],
        },
        // A paw brought up in front of the mouth for the middle of a yawn. A wing is not a paw.
        BodyClip::Gesture(Gesture::Yawn)
            if (1..=2).contains(&frame) && plan != BodyPlan::Winged =>
        {
            front(to(hx + head / 2 + 2, hy + 3))
        }
        BodyClip::Gesture(Gesture::Yawn) => [Limb::Rest; 2],
        // Folded across the chest, each paw reaching past the middle to the other side. Wings
        // fold, which reads as puffing up, and so does a blob, whose short paws folded across
        // it would only draw a belt.
        BodyClip::Gesture(Gesture::Huff) if matches!(plan, BodyPlan::Winged | BodyPlan::Blob) => {
            [Limb::Rest; 2]
        }
        BodyClip::Gesture(Gesture::Huff) => [to(hx + 2, y + 1), to(hx - 2, y + 2)],
        // Wings fling open as it goes over.
        BodyClip::Gesture(Gesture::Swoon) if plan == BodyPlan::Winged => {
            let sink = i32::from(frame.min(2));
            [to(x - (rx + 5), y - 4 + sink), to(x + rx + 5, y - 6 + sink)]
        }
        // One paw to the brow, the other flung out and sinking with the body.
        BodyClip::Gesture(Gesture::Swoon) => [
            to(x - (rx + 4), y - 3 + i32::from(frame.min(2))),
            to(hx + 2, hy - head + 2),
        ],
        // Both paws together under the chin, bouncing with the body. Wings stay folded.
        BodyClip::Gesture(Gesture::Beg) if plan == BodyPlan::Winged => [Limb::Rest; 2],
        BodyClip::Gesture(Gesture::Beg) => [
            to(hx - 1, hy + head - 1 + tick),
            to(hx + 1, hy + head - 1 + tick),
        ],
        // One paw on the hip and the other up in a flourish.
        BodyClip::Gesture(Gesture::Strut) => [
            to(x - rx - 1, y + 2),
            to(beside_head(1) + 1, hy - head - tick),
        ],
        // Paws over the face, the near one dropping to peek on the second frame.
        BodyClip::Gesture(Gesture::Peek) => [to(hx - 3, hy - 1), to(hx + 3, hy - 1 + tick * 4)],
        // Paws stiff at the sides: the foot does the talking.
        BodyClip::Gesture(Gesture::Stomp) => [Limb::Rest; 2],
        // Paws in its lap: the swinging feet are the whole of it.
        BodyClip::Gesture(Gesture::Sit) => [Limb::Rest; 2],
        // Wings stay folded in a sled: the leaning does the work.
        BodyClip::Gesture(Gesture::Scoot) if plan == BodyPlan::Winged => [Limb::Rest; 2],
        // One paw down at the ground beside it and pushing, and then the other.
        BodyClip::Gesture(Gesture::Scoot) => {
            let push = |side: i32| to(x + side * (rx + 2), floor - 2);
            if tick == 0 {
                [push(-1), Limb::Rest]
            } else {
                [Limb::Rest, push(1)]
            }
        }
        // A wing brought round to the face, preening along it.
        BodyClip::Gesture(Gesture::Groom) if plan == BodyPlan::Winged => {
            front(to(hx + head, hy + 1 + tick * 2))
        }
        // A paw up at the cheek, rubbing down it, and brought to the mouth to be licked.
        BodyClip::Gesture(Gesture::Groom) => {
            let paw = hx + head / 2 + 2;
            front(match frame % 4 {
                1 => to(paw, hy + 3),
                3 => to(paw - 1, hy + 4),
                _ => to(paw, hy + 1),
            })
        }
        // Wings flung out one way and the other as it shakes; paws stay down.
        BodyClip::Gesture(Gesture::Shake) if plan == BodyPlan::Winged => {
            both(&|side| to(x + side * (rx + 3), y - 2 - (tick + side).rem_euclid(2) * 2))
        }
        // A wing curled forward round the can.
        BodyClip::Gesture(Gesture::Pour) if plan == BodyPlan::Winged => {
            front(to(hx + head + 3, hy + 7))
        }
        // Both paws out low in front, holding the can where it hangs.
        BodyClip::Gesture(Gesture::Pour) => [to(hx + 5, hy + 8), to(hx + 8, hy + 7)],
        // A wing can't pick: it stoops and the beak does it.
        BodyClip::Gesture(Gesture::Pluck) if plan == BodyPlan::Winged => [Limb::Rest; 2],
        // A paw down at the ground, gripping, tugging back, and up with what came away.
        BodyClip::Gesture(Gesture::Pluck) => front(match frame % 4 {
            0 | 1 => to(x + rx + 3, floor - 2),
            2 => to(x + rx + 2, y + 2),
            _ => to(hx + 7, hy + 6),
        }),
        // A wing working at it.
        BodyClip::Gesture(Gesture::Tend) if plan == BodyPlan::Winged => {
            front(to(hx + head + 2, hy + 2 + tick))
        }
        // Both paws up in front, working in turn.
        BodyClip::Gesture(Gesture::Tend) => [to(hx + 5, hy + 3 + beat), to(hx + 8, hy + 2 - beat)],
        BodyClip::Gesture(Gesture::Bask) => [Limb::Rest; 2],
        // A beak needs no hands: wings stay folded through a meal.
        BodyClip::Gesture(Gesture::Nibble | Gesture::Chomp | Gesture::Chew | Gesture::Gulp)
            if plan == BodyPlan::Winged =>
        {
            [Limb::Rest; 2]
        }
        // The near paw up at the mouth with it, dipping in quick little bites.
        BodyClip::Gesture(Gesture::Nibble) => front(to(hx + head / 2 + 3, hy + 3 + tick)),
        // Both paws round something big, pushed in to the mouth for the bite.
        BodyClip::Gesture(Gesture::Chomp) => {
            let push = i32::from(frame % 4 == 1 || frame % 4 == 2);
            [
                to(hx + head / 2 + 1 - push, hy + 4),
                to(hx + head / 2 + 4 - push, hy + 3),
            ]
        }
        // Lowered in front of the chest while the mouth works.
        BodyClip::Gesture(Gesture::Chew) => [to(hx + 4, y + 3), to(hx + 6, y + 2 - tick)],
        // Up over the face to drop it in, and down again.
        BodyClip::Gesture(Gesture::Gulp) => {
            let up = [1, 4, 3, 0][usize::from(frame % 4)];
            front(to(hx + head / 2 + 3, hy + 2 - up))
        }
        // Wings laid out flat on the ground to either side.
        BodyClip::Gesture(Gesture::Sprawl) if plan == BodyPlan::Winged => {
            both(&|side| to(x + side * (rx + 4), floor - 3))
        }
        // The near paw flopped out on the ground ahead of it.
        BodyClip::Gesture(Gesture::Sprawl) => front(to(x + rx + 4, floor - 2)),
        BodyClip::Gesture(
            Gesture::Shake
            | Gesture::Sniff
            | Gesture::Loaf
            | Gesture::Skip
            | Gesture::Trudge
            | Gesture::Stride
            | Gesture::Dash
            | Gesture::Depart
            | Gesture::Scale
            | Gesture::Crest,
        ) => [Limb::Rest; 2],
        // Wings open up and out as far as they go.
        BodyClip::Gesture(Gesture::Stretch) if plan == BodyPlan::Winged => {
            let up = [0, 1, 2, 2][usize::from(frame.min(3))];
            both(&|side| to(x + side * (rx + 4 + up), hy - head - up))
        }
        // Paws up and in over the head, higher each frame until they are at full stretch.
        BodyClip::Gesture(Gesture::Stretch) => {
            let up = [0, 2, 3, 3][usize::from(frame.min(3))];
            both(&|side| to(hx + side * (head / 2 + 2), hy - head - 2 - up))
        }
        // Topping out onto a ledge (see `Pose::for_gesture`): the forearms laid over the edge
        // in front of the chest, both paws planted flat on the ledge at the hips for the push,
        // and down at its sides once it stands. The ledge is where the world has it in each
        // frame: up at the chest, and then down at the feet.
        BodyClip::Gesture(Gesture::Mantle) => match frame.min(4) {
            // Hung from the ledge, and pulled up on it: both paws hooked over its corner, the
            // far one a little further in along it.
            // The chest brought up over the edge, both paws still where they were, pressed down
            // on the ledge now that it is at the waist.
            0..=2 => {
                let ledge = mantle_ledge(floor + 2, frame, false);
                [to(CLIMB_WALL + 3, ledge - 3), to(CLIMB_WALL, ledge - 3)]
            }
            3 => [to(x - rx + 1, floor - 2), to(x + rx + 2, floor - 2)],
            _ => [Limb::Rest; 2],
        },
    }
}

/// Where the two paws of a climber are in a frame of its climb, the one whose stroke it is and
/// the other, for a head at row `hy`. The stroke's paw lands high on the wall, well above the
/// head, and stays where it landed: each frame it is drawn lower by what the world has raised the
/// body since (see `formiga_core::CLIMB_FRAME_RISE`), down to the cheek. The other paw holds the
/// wall under it for the catch, a row lower again, and then lets go and comes up off the wall,
/// past the face and on up to make the next catch.
fn climb_grip(frame: u8, hy: i32) -> (PixelPoint, PixelPoint) {
    let rise = formiga_core::CLIMB_FRAME_RISE.map(i32::from);
    // The head is carried up past the paws on the pull, and the paws stay where they are.
    let hy = hy + super::pose::CLIMB_LIFT[usize::from(frame % 3)];
    let top = hy - rise[1] - rise[2];
    let at = |x: i32, y: i32| PixelPoint { x, y };
    match frame % 3 {
        0 => (
            at(CLIMB_PAW, top),
            at(CLIMB_PAW, top + rise.iter().sum::<i32>()),
        ),
        1 => (
            at(CLIMB_PAW, top + rise[1]),
            at(CLIMB_PAW - 3, top + rise[1] + 2),
        ),
        _ => (
            at(CLIMB_PAW, top + rise[1] + rise[2]),
            at(CLIMB_PAW - 2, top + 3),
        ),
    }
}

/// Where a four-pawed body's hind and fore legs meet it on one side, the near side or the far: a
/// step apart from their partners on the other side, so all four paws show. Each far leg stands a
/// step toward the head from its near partner, as it does on a body turned a little toward the
/// person looking at it, which is the way its face is turned.
fn leg_roots(body: Body, near: bool) -> (PixelPoint, PixelPoint) {
    let Body { x, y, rx, ry, .. } = body;
    let top = y + ry - 3;
    let (hind, fore) = if near {
        (x - rx + 3, x + rx - 8)
    } else {
        (x - rx + 6, x + rx - 5)
    };
    (
        PixelPoint { x: hind, y: top },
        PixelPoint { x: fore, y: top },
    )
}

/// Where a four-pawed body's forepaw on one side is raised from.
fn fore_shoulder(body: Body, near: bool) -> PixelPoint {
    leg_roots(body, near).1
}

/// Where a raised forepaw comes from in a clip. Reaching up for a ledge, the near forepaw goes up
/// from the chest in front of the face and the far one from behind the head, so the face between
/// them is neither crossed nor half hidden; anything else lifts a forepaw from where it stood.
fn raised_root(body: Body, clip: BodyClip, near: bool) -> PixelPoint {
    let Body { hx, hy, head, .. } = body;
    match clip {
        BodyClip::Action(ActionKind::Dangle | ActionKind::ClimbWindow) => {
            if near {
                PixelPoint {
                    x: hx + head - 2,
                    y: hy + head - 2,
                }
            } else {
                PixelPoint {
                    x: hx - head + 3,
                    y: hy + head - 3,
                }
            }
        }
        _ => fore_shoulder(body, near),
    }
}

/// One side of a four-pawed body's legs, hind and fore, planted on the floor and stepping in turn:
/// hind then fore on the near side, a quarter of a stride apart, and the far side half a stride
/// behind the near, which is how four legs walk. A forepaw in use is raised instead — the far one
/// here, behind the body, and the near one last of all, in front of what it covers.
#[allow(clippy::too_many_arguments)]
fn four_legs(
    c: &mut Canvas,
    p: Palette,
    d: CreatureDesign,
    body: Body,
    pose: Pose,
    clip: BodyClip,
    limbs: [Limb; 2],
    near: bool,
) {
    let k = d.classic;
    // It lifts a leg only to walk, and not while a forepaw is busy: it stands on the other three.
    let walking = walks(clip) && limbs == [Limb::Rest; 2];
    let (hind, fore) = leg_roots(body, near);
    // Hung from a ledge and pulling up on it, both sides reach and hang together; walking, on
    // the ground or up a wall, the far side is half a stride behind the near.
    // A bound's forelegs go out together too, and its hind legs back.
    let together = clip == BodyClip::Gesture(Gesture::Leap)
        || (pose.wall && matches!(clip, BodyClip::Gesture(Gesture::Mantle)));
    let (hind_step, fore_step) = if near || together {
        (pose.step_a, pose.step_b)
    } else {
        (-pose.step_a, -pose.step_b)
    };
    // The far legs are in shade, as the far side of a body is; stick legs are shade either way.
    let fill = if near && k.limbs != 2 {
        p.coat
    } else {
        p.shadow
    };
    // Nubs tip each near leg in the accent colour; a plain leg is its own colour to the toe.
    let toe = if near && k.limbs == 1 { p.accent } else { fill };
    let bottom = feet_reach(d, body.floor);
    // Topping out onto a ledge, the near hind leg is drawn up to it and then the far one trails.
    let (knee, trail) = if near {
        (pose.knee.clamp(0, 6), 0)
    } else {
        (0, pose.trail.clamp(0, 4))
    };
    for (root, step, limb, hind) in [
        (hind, hind_step, Limb::Rest, true),
        (fore, fore_step, limbs[usize::from(near)], false),
    ] {
        if let Limb::Reach(paw) = limb {
            if !near {
                raised_paw(c, p, k.limbs, raised_root(body, clip, false), paw, false);
            }
            continue;
        }
        // Four legs are tucked by how far the body has risen, whatever a pose says of two feet.
        let lift = leg_lift(step, walking)
            + if hind { knee - trail } else { 0 }
            + airborne_feet(
                clip,
                Pose {
                    lifts: None,
                    ..pose
                },
            );
        let x = root.x + step.clamp(-3, 3) + if hind { knee / 2 } else { 0 };
        if k.limbs == 2 {
            // Stick legs, thin and in shade, on small forked feet.
            let foot = PixelPoint {
                x,
                y: bottom - 2 - lift,
            };
            c.line(root.x, root.y, foot.x, foot.y, 2, p.outline);
            c.line(root.x, root.y, foot.x, foot.y, 1, p.shadow);
            if near {
                classic_foot(c, p, 1, foot);
            } else {
                oval(c, p, foot.x + 1, foot.y + 1, 2, 1, p.shadow);
            }
        } else {
            u_leg(
                c,
                p,
                root,
                PixelPoint {
                    x,
                    y: bottom - lift,
                },
                fill,
                toe,
            );
        }
    }
}

/// How many rows a body's feet are drawn up off the ground in a frame, besides what its pose says
/// of each foot itself (`Pose::lifts`): a four-pawed body's leap tucks them up under it, a row
/// past the lift it rises by. Every other clip stands on them.
fn airborne_feet(clip: BodyClip, pose: Pose) -> i32 {
    if clip == BodyClip::Gesture(Gesture::Leap) && pose.lifts.is_none() {
        pose.play_lift.clamp(0, 3) + 1
    } else {
        0
    }
}

/// Whether a clip carries a body along on its feet, which is the only time a four-pawed body lifts
/// one of them: anything else it does, it does with every paw down.
fn walks(clip: BodyClip) -> bool {
    matches!(
        clip,
        BodyClip::Action(
            ActionKind::Traverse
                | ActionKind::Sprint
                | ActionKind::Follow
                | ActionKind::SqueezeWindow
                | ActionKind::SoloPlay
                | ActionKind::SocialPlay
                | ActionKind::AvoidCursor
                | ActionKind::ReactToWindow
                | ActionKind::Landing
        ) | BodyClip::Gesture(
            Gesture::Skip | Gesture::Trudge | Gesture::Stride | Gesture::Dash | Gesture::Depart
        )
    )
}

/// How far a four-pawed body's leg is off the ground at a point in its stride: only the leg
/// swinging forward is lifted, and only while it walks, so three paws are always down.
fn leg_lift(step: i32, walking: bool) -> i32 {
    if walking { step.clamp(0, 2) } else { 0 }
}

/// A leg like a letter U: two straight sides and a rounded foot, its fill running down into the
/// curve, so the paw is the end of the leg rather than a pad stuck on it. A step slants it, each
/// row a little further toward where the foot is planted.
fn u_leg(c: &mut Canvas, p: Palette, top: PixelPoint, foot: PixelPoint, fill: Rgba, toe: Rgba) {
    let rows = (foot.y - top.y).max(1);
    for row in 0..=rows {
        let y = top.y + row;
        let x = top.x + (foot.x - top.x) * row / rows;
        if row == rows {
            c.fill_rect(x, y, 2, 1, p.outline);
        } else {
            c.set(x - 1, y, p.outline);
            c.set(x + 2, y, p.outline);
            c.fill_rect(x, y, 2, 1, if row + 1 == rows { toe } else { fill });
        }
    }
}

/// A forepaw raised from its shoulder to `paw`: a short leg of the same weight as those it
/// stands on, rounded off at the end as a planted one is, and tipped in the accent colour on a
/// companion whose paws are.
fn raised_paw(
    c: &mut Canvas,
    p: Palette,
    limbs: u8,
    root: PixelPoint,
    paw: PixelPoint,
    near: bool,
) {
    let fill = if near && limbs != 2 { p.coat } else { p.shadow };
    thick_leg(c, root, paw, p.outline, fill);
    if near && limbs == 1 {
        c.fill_rect(paw.x, paw.y, 2, 1, p.accent);
    }
}

/// A leg two pixels wide inside its outline, from where it meets the body down to its paw.
fn thick_leg(c: &mut Canvas, top: PixelPoint, foot: PixelPoint, outline: Rgba, fill: Rgba) {
    for dx in [0, 1] {
        c.line(top.x + dx, top.y, foot.x + dx, foot.y, 2, outline);
    }
    for dx in [0, 1] {
        c.line(top.x + dx, top.y, foot.x + dx, foot.y, 1, fill);
    }
}

/// A four-pawed body's forepaws in a frame, far then near. It does with one forepaw what another
/// body does with a hand, and stands on the other three while it does: a body this heavy at the
/// front never rears up onto its hind legs. The paw is lifted a little forward from where it stood
/// and never higher than the chin, as a forepaw goes, so it is seen under the face rather than
/// drawn across it. Covering the eyes and peeking through them go to the face on purpose, and only
/// hanging from a ledge, with nothing to stand on, takes both forepaws.
fn long_limbs(clip: BodyClip, frame: u8, body: Body) -> [Limb; 2] {
    let Body { hx, hy, head, .. } = body;
    let tick = i32::from(frame % 2);
    let beat = [0, 1, 0, -1][usize::from(frame % 4)];
    let (near, far) = (fore_shoulder(body, true), fore_shoulder(body, false));
    // How many rows a forepaw can come up before it reaches the chin.
    let chin = hy + head - 2;
    let room = (near.y - chin).max(1);
    let to = |x: i32, y: i32| {
        Limb::Reach(PixelPoint {
            x: x.clamp(4, 42),
            y: y.clamp(3, 44),
        })
    };
    // Lifted `forward` columns ahead of the leg it stood on and `up` rows, no higher than the chin.
    let lift =
        |root: PixelPoint, forward: i32, up: i32| to(root.x + forward, root.y - up.clamp(0, room));
    let rest = Limb::Rest;
    match clip {
        // Dangling forepaws share the existing y=7 ledge anchor. Both go up the sides of the
        // head, the near one in front of the face and the far one behind.
        BodyClip::Action(ActionKind::Dangle) => [to(hx - head - 1, 7), to(hx + head + 1, 7)],
        // The same strokes as a two-legged climber's, the far forepaw up behind the head and the
        // near one in front of the face; the hind legs brace in turn on the pulls.
        // Upright on the wall, it climbs on all four paws: none is raised.
        BodyClip::Action(ActionKind::ClimbWindow) => [rest; 2],
        BodyClip::Action(ActionKind::PresentDiscovery) => [rest, lift(near, 6, room - 1)],
        BodyClip::Action(ActionKind::Greet) => [rest, lift(near, 6, room - tick)],
        BodyClip::Action(ActionKind::SocialPlay) => [rest, lift(near, 6 + tick, room - 1)],
        BodyClip::Action(ActionKind::InvestigateCursor) => [rest, lift(near, 8, 1 + tick)],
        // Keep-up with the near forepaw alone, the other three down: lifted low under the chest to
        // catch the toy coming back over its shoulders on the crouch, out and up to the chin to
        // knock it and on the stretch, and coming back down.
        BodyClip::Action(ActionKind::SoloPlay) => {
            let beat = usize::from(frame % 4);
            let up = [1, room - 1, room, room / 2][beat];
            [rest, lift(near, [0, 5, 6, 3][beat], up)]
        }
        BodyClip::Action(_) => [rest; 2],
        BodyClip::Gesture(Gesture::Cheer) => [rest, lift(near, 6 + tick, room)],
        BodyClip::Gesture(Gesture::Gasp) => [rest, lift(near, 4, room - tick)],
        BodyClip::Gesture(Gesture::Cover) => [rest, to(hx + 3, hy + tick * 2)],
        BodyClip::Gesture(Gesture::Worry) => [rest, lift(near, 4, 1 + beat.max(0))],
        BodyClip::Gesture(Gesture::Heave) => {
            let pull = [0, 1, 2, 1][usize::from(frame % 4)];
            [rest, lift(near, 8 - pull, 1)]
        }
        BodyClip::Gesture(Gesture::Balance) => [rest, lift(near, 8, room - beat.max(0))],
        BodyClip::Gesture(Gesture::Reach) => [rest, lift(near, 9, room - tick)],
        BodyClip::Gesture(Gesture::Bop) => match frame % 4 {
            0 => [rest, lift(near, 6, room)],
            2 => [lift(far, 5, room), rest],
            _ => [rest; 2],
        },
        BodyClip::Gesture(Gesture::Swoon) => [rest, lift(near, 5, room)],
        BodyClip::Gesture(Gesture::Beg) => [rest, lift(near, 5, room - tick)],
        BodyClip::Gesture(Gesture::Strut) => [rest, lift(near, 7 + tick, room)],
        BodyClip::Gesture(Gesture::Peek) => [rest, to(hx + 3, hy - 1 + tick * 4)],
        BodyClip::Gesture(Gesture::Huff) => [rest, lift(near, 3, room / 2)],
        // The near forepaw lifted under the chin and the head dipped to it: licked, and lifted
        // again, rather than rubbed across the face, so the face stays clear.
        BodyClip::Gesture(Gesture::Groom) => [rest, lift(near, 5, room - i32::from(frame % 2))],
        // The near forepaw lifted out low, steadying the can.
        BodyClip::Gesture(Gesture::Pour) => [rest, lift(near, 7, 1)],
        // Head down to the ground for it, then the near forepaw up with what came away.
        BodyClip::Gesture(Gesture::Pluck) if frame % 4 == 3 => [rest, lift(near, 6, room - 1)],
        // On four feet a meal is held in the mouth; only a nibble lifts a forepaw to steady it.
        BodyClip::Gesture(Gesture::Nibble) => [rest, lift(near, 5, room - 1 - tick)],
        BodyClip::Gesture(Gesture::Chomp | Gesture::Chew | Gesture::Gulp) => [rest; 2],
        // Working at it with one forepaw and then the other.
        BodyClip::Gesture(Gesture::Tend) => match frame % 4 {
            2 => [lift(far, 5, room - 1), rest],
            _ => [rest, lift(near, 6, room - 1 - tick)],
        },
        BodyClip::Gesture(
            Gesture::Shake
            | Gesture::Sniff
            | Gesture::Sprawl
            | Gesture::Loaf
            | Gesture::Skip
            | Gesture::Trudge | Gesture::Stride | Gesture::Dash | Gesture::Depart
            | Gesture::Scale | Gesture::Crest
            | Gesture::Pluck
            | Gesture::Bask,
        ) => [rest; 2],
        BodyClip::Gesture(
            Gesture::Crouch
            | Gesture::Leap
            | Gesture::Watch
            | Gesture::Yawn
            | Gesture::Stomp
            | Gesture::Stretch
            | Gesture::Sit
            // Four paws down in the leaf, leaning into it: the dip of each push is the scoot.
            | Gesture::Scoot,
        ) => [rest; 2],
        // Topping out: both forepaws reached forward over the edge, up at the chest where the
        // ledge is, then planted on it ahead for the push, and down on all four once it stands.
        BodyClip::Gesture(Gesture::Mantle) => match frame.min(4) {
            // Both forepaws on the ledge past its edge, the far one further along it.
            2 => {
                let ledge = mantle_ledge(body.floor + 2, 2, true);
                let wall = FRAME_SIZE as i32 / 2 + 16;
                [to(wall + 2, ledge - 2), to(wall, ledge - 3)]
            }
            3 => [
                to(far.x + 3, body.floor - 1),
                to(near.x + 4, body.floor - 1),
            ],
            // Still upright on the wall for the hang and the pull-up, and stood at the last.
            _ => [rest; 2],
        },
    }
}

/// A paw carried out from its shoulder. Outline first and fill second, so the arm reads as one
/// piece, and the root is filled back into the body so it grows from the side instead of
/// sitting on top of it.
fn draw_arm(c: &mut Canvas, p: Palette, root: PixelPoint, hand: PixelPoint) {
    c.line(root.x, root.y, hand.x, hand.y, 3, p.outline);
    c.fill_ellipse(hand.x, hand.y, 3, 3, p.outline);
    c.line(root.x, root.y, hand.x, hand.y, 2, p.coat);
    c.fill_ellipse(hand.x, hand.y, 2, 2, p.coat);
    c.fill_circle(root.x, root.y, 2, p.coat);
}

/// A wing opened toward a point: the folded wing's own membrane swept out from the shoulder and
/// tapering to its tip, textured in the same style it shows when folded.
fn draw_open_wing(
    c: &mut Canvas,
    p: Palette,
    style: u8,
    root: PixelPoint,
    tip: PixelPoint,
    side: i32,
) {
    let (dx, dy) = ((tip.x - root.x) as f32, (tip.y - root.y) as f32);
    let length = (dx * dx + dy * dy).sqrt().max(1.0);
    let steps = (length / 2.0).ceil() as i32;
    let along = |t: f32| PixelPoint {
        x: root.x + (dx * t).round() as i32,
        y: root.y + (dy * t).round() as i32,
    };
    let radius = |t: f32| (2.6 - t * 1.8).round() as i32;
    for pass in [true, false] {
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let at = along(t);
            let r = radius(t);
            if pass {
                c.fill_ellipse(at.x, at.y, r + 1, r + 1, p.outline);
            } else {
                c.fill_ellipse(at.x, at.y, r, r, p.accent);
            }
        }
    }
    c.fill_circle(root.x, root.y, 2, p.accent);
    // The leading edge faces up and the trailing edge down, whichever way the wing is carried.
    let (mut nx, mut ny) = (-dy / length, dx / length);
    if ny > 0.0 || (ny == 0.0 && nx * side as f32 > 0.0) {
        nx = -nx;
        ny = -ny;
    }
    let offset = |at: PixelPoint, by: f32| PixelPoint {
        x: at.x + (nx * by).round() as i32,
        y: at.y + (ny * by).round() as i32,
    };
    let (base, mid, end) = (along(0.15), along(0.55), along(0.85));
    let (lead_base, lead_end) = (offset(base, 2.0), offset(end, 1.0));
    c.line(
        lead_base.x,
        lead_base.y,
        lead_end.x,
        lead_end.y,
        1,
        p.highlight,
    );
    match style {
        // Feathered: quills fanning back from the leading edge.
        0 => {
            for t in [0.3, 0.55, 0.8] {
                let from = offset(along(t), 1.0);
                let to = offset(along(t - 0.12), -2.0);
                c.line(from.x, from.y, to.x, to.y, 1, p.shadow);
            }
        }
        // Membrane: ribs from the shoulder out to the trailing edge.
        1 => {
            for t in [0.6, 1.0] {
                let to = offset(along(t), -1.0);
                c.line(base.x, base.y, to.x, to.y, 1, p.shadow);
            }
        }
        // Panelled: a pale inner panel with a single vein.
        _ => {
            let from = offset(base, -1.0);
            let to = offset(mid, -1.0);
            c.line(from.x, from.y, to.x, to.y, 1, p.shadow);
        }
    }
}

/// Which wing a creature grew. Derived from bytes already in the recipe, so wings vary between
/// creatures, stay identical every time one is drawn, and travel intact inside a shared code
/// without spending another recipe byte.
fn wing_style(d: CreatureDesign) -> u8 {
    ((u16::from(d.accent[0]) + u16::from(d.marking) * 5 + u16::from(d.tail) * 3) % 3) as u8
}

/// A wing rather than a large nub. The membrane keeps its existing silhouette, so the body
/// connection and the reserved face are untouched; the shape is carried by value instead. The
/// underside is shaded and the leading edge lit, which sweeps the bright part up and outward the
/// way a folded wing reads, and the style adds its own structure over that.
fn draw_wing(c: &mut Canvas, p: Palette, style: u8, x: i32, y: i32, side: i32) {
    oval(c, p, x, y, 4, 5, p.accent);
    // A tip carried up past the shoulder, so a wing is never read as another arm.
    oval(c, p, x + side, y - 4, 2, 3, p.accent);
    oval(c, p, x + side * 3, y - 6, 1, 2, p.accent);
    // Shade the trailing underside, nearest the body and lowest on the wing.
    c.fill_ellipse(x - side, y + 3, 3, 2, p.shadow);
    match style {
        // Feathered: parallel quills sweeping out to the tip, over a lit leading edge.
        0 => {
            c.line(x - side * 2, y - 4, x + side * 3, y - 2, 1, p.highlight);
            for offset in 0..3 {
                let row = y - 2 + offset * 2;
                c.line(x - side, row, x + side * 3, row + 1, 1, p.shadow);
            }
        }
        // Membrane: two clean ribs running from the shoulder to a drawn-down tip.
        1 => {
            c.fill_ellipse(x + side, y - 2, 3, 3, p.highlight);
            for dy in [0, 3] {
                c.line(x - side * 2, y - 4, x + side * 3, y + dy, 1, p.shadow);
            }
            c.line(x + side * 3, y + 1, x + side * 3, y + 4, 1, p.accent);
        }
        // Panelled: a pale inner panel behind a darker outer rim, with a single vein.
        _ => {
            c.fill_ellipse(x - side, y - 1, 2, 4, p.highlight);
            c.line(x + side * 3, y - 3, x + side * 3, y + 3, 1, p.shadow);
            c.line(x - side, y - 3, x + side * 2, y + 1, 1, p.shadow);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::ClassicParts;

    /// A plain modular companion. Classic parts are switched off here so these tests keep
    /// covering the modular parts alone; the classic ones are exercised on their own below.
    fn preview() -> formiga_core::Creature {
        let mut creature = formiga_core::World::preview_adult(
            [55; 32],
            time::OffsetDateTime::UNIX_EPOCH,
            &formiga_core::DesktopSnapshot::default(),
        );
        let design = CreatureDesign::modular([55; 32], 0, None);
        formiga_core::apply_creature_design(&mut creature, Some(design));
        creature
    }

    /// Every combination of the classic parts that changes a body's shape. The face is drawn on
    /// its own layer, so it is covered by the face tests instead.
    fn classic_bodies() -> Vec<ClassicParts> {
        let mut all = Vec::new();
        for coat in 0..=1 {
            for limbs in 0..=2 {
                for crown in 0..=2 {
                    for pattern in 0..=3 {
                        for tail in 0..=2 {
                            all.push(ClassicParts {
                                coat,
                                face: 0,
                                limbs,
                                crown,
                                pattern,
                                tail,
                            });
                        }
                    }
                }
            }
        }
        all
    }

    /// Draws one modular frame exactly as the body renderer does, before props and effects.
    fn render(
        creature: &formiga_core::Creature,
        d: CreatureDesign,
        clip: BodyClip,
        frame: u8,
        size: f32,
    ) -> (Canvas, PixelPoint, Pose) {
        let p = crate::palette_for(&creature.appearance);
        // Posed as the design it is drawn as: a four-pawed body has poses of its own.
        let mut posed = creature.appearance.clone();
        posed.design = Some(d);
        let pose = Pose::new(&posed, clip, frame, false);
        let mut c = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        let anchor = draw(&mut c, d, p, pose, size, clip, frame);
        (c, anchor, pose)
    }

    /// The face area stays covered and every drawn pixel belongs to one connected creature.
    fn assert_whole(c: &Canvas, anchor: PixelPoint, label: &str) {
        for dx in -5..=5 {
            for dy in -3..=3 {
                assert!(
                    c.get(anchor.x + dx, anchor.y + dy).a > 0,
                    "reserved face {label}"
                );
            }
        }
        let count = c.pixels().iter().filter(|p| p.a > 0).count();
        let mut seen = vec![false; (FRAME_SIZE * FRAME_SIZE) as usize];
        let mut pending = vec![(anchor.x, anchor.y)];
        while let Some((x, y)) = pending.pop() {
            if c.get(x, y).a == 0 {
                continue;
            }
            let index = (y * FRAME_SIZE as i32 + x) as usize;
            if seen[index] {
                continue;
            }
            seen[index] = true;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    pending.push((x + dx, y + dy));
                }
            }
        }
        let apart: Vec<(i32, i32)> = (0..FRAME_SIZE as i32)
            .flat_map(|y| (0..FRAME_SIZE as i32).map(move |x| (x, y)))
            .filter(|(x, y)| c.get(*x, *y).a > 0 && !seen[(y * FRAME_SIZE as i32 + x) as usize])
            .collect();
        assert_eq!(
            seen.iter().filter(|v| **v).count(),
            count,
            "disconnected part {label}: {apart:?}"
        );
    }

    #[test]
    fn every_wing_style_is_reachable_deterministic_and_visibly_textured() {
        let creature = preview();
        let base = creature.appearance.design.unwrap();
        let mut rendered = Vec::new();
        for style in 0..3_u8 {
            let mut d = base;
            d.body = BodyPlan::Winged;
            d.marking = 0;
            d.tail = 0;
            // Only the style-selecting byte moves, so any difference is the wing itself.
            d.accent[0] = 150 + style;
            assert_eq!(wing_style(d), (150 + style) % 3);
            assert_eq!(
                wing_style(d),
                wing_style(d),
                "a recipe always grows one wing"
            );
            let idle = BodyClip::Action(ActionKind::Idle);
            rendered.push(render(&creature, d, idle, 0, 1.0).0);
        }
        for (index, canvas) in rendered.iter().enumerate() {
            for other in rendered.iter().skip(index + 1) {
                assert_ne!(
                    canvas.pixels(),
                    other.pixels(),
                    "wing styles should not render alike"
                );
            }
            // A wing carries structure rather than one flat block of accent.
            let colors: std::collections::HashSet<_> = canvas
                .pixels()
                .iter()
                .filter(|pixel| pixel.a > 0)
                .map(|pixel| (pixel.r, pixel.g, pixel.b))
                .collect();
            assert!(colors.len() >= 4, "style {index} should be textured");
        }
        // Ordinary generation reaches all three.
        let styles: std::collections::HashSet<_> = (0..512_u64)
            .map(|index| {
                formiga_core::CreatureDesign::generated(
                    formiga_core::SeedStream::new([13; 32]).bytes("wing-reach", index),
                    0,
                    None,
                )
            })
            .map(wing_style)
            .collect();
        assert_eq!(styles.len(), 3);
    }

    #[test]
    fn all_part_combinations_keep_connected_bodies_and_a_reserved_face_at_extreme_sizes() {
        let creature = preview();
        for body in BodyPlan::ALL {
            for ears in EarStyle::ALL {
                for tail in 0..5 {
                    for small in [false, true] {
                        let mut d = creature.appearance.design.unwrap();
                        d.body = body;
                        d.ears = ears;
                        d.tail = tail;
                        d.width = if small { 8 } else { 12 };
                        d.height = if small { 7 } else { 11 };
                        d.head = if small { 7 } else { 9 };
                        d.legs = if small { 3 } else { 6 };
                        d.ear_size = 7;
                        let (c, anchor, _) = render(
                            &creature,
                            d,
                            BodyClip::Action(ActionKind::Idle),
                            0,
                            if small { 0.55 } else { 1.05 },
                        );
                        assert_whole(&c, anchor, &format!("{d:?}, small={small}"));
                    }
                }
            }
        }
    }

    #[test]
    fn every_action_and_gesture_keeps_one_connected_body_and_a_reserved_face() {
        let creature = preview();
        for body in BodyPlan::ALL {
            for small in [false, true] {
                let mut d = creature.appearance.design.unwrap();
                d.body = body;
                d.width = if small { 8 } else { 12 };
                d.height = if small { 7 } else { 11 };
                d.head = if small { 7 } else { 9 };
                d.legs = if small { 3 } else { 6 };
                for clip in BodyClip::baked() {
                    for frame in 0..AnimationSpec::for_clip(clip).frames {
                        let size = if small { 0.55 } else { 1.05 };
                        let (c, anchor, _) = render(&creature, d, clip, frame, size);
                        assert_whole(&c, anchor, &format!("{body:?} {clip:?} {frame} {small}"));
                    }
                }
            }
        }
    }

    /// Wing-nubs, horns, a belly and a tail tip, every style of each together, on every body at
    /// both extremes of size and through every action and gesture.
    #[test]
    fn every_detail_keeps_one_connected_body_and_a_reserved_face_through_every_clip() {
        let creature = preview();
        for body in BodyPlan::ALL {
            for style in 1..=2 {
                for small in [false, true] {
                    let mut d = creature.appearance.design.unwrap();
                    d.archetype = d.archetype.max(1);
                    d.face_template = d.face_template.max(1);
                    d.body = body;
                    d.classic = ClassicParts::default();
                    d.tail = 3;
                    d.ears = EarStyle::Pointed;
                    d.ear_size = 7;
                    d.width = if small { 8 } else { 12 };
                    d.height = if small { 7 } else { 11 };
                    d.head = if small { 7 } else { 9 };
                    d.legs = if small { 3 } else { 6 };
                    d.details = formiga_core::DetailParts {
                        wings: style,
                        horns: style,
                        belly: 1,
                        tip: style,
                        belly_color: [250, 228, 160],
                        tip_color: [255, 170, 40],
                    };
                    let d = d.bounded();
                    assert_eq!(d.details.horns, style, "{d:?}");
                    for clip in BodyClip::baked() {
                        for frame in 0..AnimationSpec::for_clip(clip).frames {
                            let size = if small { 0.55 } else { 1.05 };
                            let (c, anchor, _) = render(&creature, d, clip, frame, size);
                            assert_whole(
                                &c,
                                anchor,
                                &format!("{body:?} style {style} {clip:?} {frame} {small}"),
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_gesture_carries_out_the_limb_already_there_instead_of_growing_another() {
        let creature = preview();
        let d = creature.appearance.design.unwrap();
        for plan in BodyPlan::ALL {
            let mut d = d;
            d.body = plan;
            d.ears = EarStyle::None;
            for clip in BodyClip::baked() {
                for frame in 0..AnimationSpec::for_clip(clip).frames {
                    let (canvas, _, pose) = render(&creature, d, clip, frame, 1.0);
                    let body = measure(d, pose, 1.0);
                    let limbs = limbs(clip, frame, body, plan);
                    // Actions that never used their paws still keep both folded.
                    if let BodyClip::Action(action) = clip {
                        let reaching = match action {
                            // A four-pawed body goes up a wall on all four paws.
                            ActionKind::ClimbWindow if plan == BodyPlan::Long => [false, false],
                            ActionKind::Dangle | ActionKind::ClimbWindow => [true, true],
                            // A four-pawed body stands on three paws to show what it found.
                            ActionKind::PresentDiscovery if plan == BodyPlan::Long => [false, true],
                            ActionKind::PresentDiscovery => [true, true],
                            // Carried, the paws hang down; a wing stays folded, and a four-pawed
                            // body hangs on its legs.
                            ActionKind::Dragged
                                if matches!(plan, BodyPlan::Winged | BodyPlan::Long) =>
                            {
                                [false, false]
                            }
                            ActionKind::Dragged => [true, true],
                            ActionKind::Greet
                            | ActionKind::SocialPlay
                            | ActionKind::SoloPlay
                            | ActionKind::InvestigateCursor => [false, true],
                            _ => [false, false],
                        };
                        assert_eq!(
                            limbs.map(|limb| limb != Limb::Rest),
                            reaching,
                            "{plan:?} {clip:?}"
                        );
                    }
                    if plan == BodyPlan::Long {
                        continue;
                    }
                    // On a wall a body is drawn turned to it, not from here (see
                    // `turned::clinging_body`).
                    if pose.wall {
                        continue;
                    }
                    for (side, limb) in [-1, 1].into_iter().zip(limbs) {
                        let at = shoulder(body, pose, side);
                        let raised = matches!(limb, Limb::Reach(hand) if hand.y <= at.y - 3);
                        if limb != Limb::Rest && !raised {
                            continue;
                        }
                        // Just outside the body, below the shoulder, is covered by the folded
                        // paw or wing and by nothing else. Raising that limb has to clear it.
                        let below = if plan == BodyPlan::Winged { 4 } else { 2 };
                        // Two columns out from the shoulder it hangs from, wherever a stride has
                        // swung that.
                        let probe = canvas.get(at.x + side * 2, at.y + below);
                        assert_eq!(
                            probe.a > 0,
                            limb == Limb::Rest,
                            "{plan:?} {clip:?} frame {frame} side {side}: a raised limb must not \
                             leave its resting shape behind"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_winged_creature_reaches_with_its_wings_rather_than_a_separate_arm() {
        let creature = preview();
        let p = crate::palette_for(&creature.appearance);
        let clip = BodyClip::Action(ActionKind::Dangle);
        let colours_above_the_crown = |plan: BodyPlan| {
            let mut d = creature.appearance.design.unwrap();
            d.body = plan;
            d.ears = EarStyle::None;
            let (canvas, _, _) = render(&creature, d, clip, 0, 1.0);
            let mut seen = Vec::new();
            for y in 0..=7 {
                for x in 0..FRAME_SIZE as i32 {
                    let pixel = canvas.get(x, y);
                    if pixel.a > 0 {
                        seen.push(pixel);
                    }
                }
            }
            seen
        };
        // Hands on a ledge are the only thing drawn this high.
        let winged = colours_above_the_crown(BodyPlan::Winged);
        assert!(winged.contains(&p.accent), "raised wings reach the ledge");
        assert!(
            !winged.contains(&p.coat),
            "a winged creature grew a coat-coloured arm beside its wings"
        );
        let pawed = colours_above_the_crown(BodyPlan::Round);
        assert!(
            pawed.contains(&p.coat),
            "a round body reaches with its paws"
        );
    }

    #[test]
    fn every_classic_part_keeps_a_connected_body_and_a_reserved_face_at_extreme_sizes() {
        let creature = preview();
        for body in BodyPlan::ALL {
            for classic in classic_bodies() {
                for small in [false, true] {
                    let mut d = creature.appearance.design.unwrap();
                    d.body = body;
                    d.classic = classic;
                    d.width = if small { 8 } else { 12 };
                    d.height = if small { 7 } else { 11 };
                    d.head = if small { 7 } else { 9 };
                    d.legs = if small { 3 } else { 6 };
                    d.ear_size = 7;
                    d.tail = 4;
                    d.marking = 6;
                    let (c, anchor, _) = render(
                        &creature,
                        d,
                        BodyClip::Action(ActionKind::Idle),
                        0,
                        if small { 0.55 } else { 1.05 },
                    );
                    assert_whole(&c, anchor, &format!("{body:?} {classic:?}, small={small}"));
                }
            }
        }
    }

    #[test]
    fn classic_limbs_and_crowns_keep_one_connected_body_through_every_action_and_gesture() {
        let creature = preview();
        for body in BodyPlan::ALL {
            for (limbs, crown) in [(1, 1), (2, 2), (2, 1)] {
                for small in [false, true] {
                    let mut d = creature.appearance.design.unwrap();
                    d.body = body;
                    d.classic = ClassicParts {
                        coat: 1,
                        limbs,
                        crown,
                        tail: 1,
                        ..ClassicParts::default()
                    };
                    d.width = if small { 8 } else { 12 };
                    d.height = if small { 7 } else { 11 };
                    d.head = if small { 7 } else { 9 };
                    d.legs = if small { 3 } else { 6 };
                    d.ear_size = 7;
                    for clip in BodyClip::baked() {
                        for frame in 0..AnimationSpec::for_clip(clip).frames {
                            let size = if small { 0.55 } else { 1.05 };
                            let (c, anchor, _) = render(&creature, d, clip, frame, size);
                            let label =
                                format!("{body:?} {limbs} {crown} {clip:?} {frame} {small}");
                            assert_whole(&c, anchor, &label);
                            let (min_x, min_y, max_x, max_y) = c.alpha_bounds().unwrap();
                            assert!(
                                min_x > 0
                                    && min_y > 0
                                    && max_x < FRAME_SIZE - 1
                                    && max_y < FRAME_SIZE - 1,
                                "{label} leaves the frame: columns {min_x} to {max_x}, rows {min_y} \
                                 to {max_y}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// A classic nub or stick leg is still the one limb on its side: folded, it covers the spot
    /// beside the body a modular paw covers, and raised, it leaves that spot empty.
    #[test]
    fn a_classic_paw_is_carried_out_rather_than_grown_beside_the_one_at_rest() {
        let creature = preview();
        for kind in 1..=2 {
            for plan in [BodyPlan::Round, BodyPlan::Upright, BodyPlan::Blob] {
                let mut d = creature.appearance.design.unwrap();
                d.body = plan;
                d.ears = EarStyle::None;
                d.classic = ClassicParts {
                    limbs: kind,
                    ..ClassicParts::default()
                };
                for clip in BodyClip::baked() {
                    for frame in 0..AnimationSpec::for_clip(clip).frames {
                        let (canvas, _, pose) = render(&creature, d, clip, frame, 1.0);
                        // On a wall a body is drawn turned to it, not from here.
                        if pose.wall {
                            continue;
                        }
                        let body = measure(d, pose, 1.0);
                        for (side, limb) in [-1, 1].into_iter().zip(limbs(clip, frame, body, plan))
                        {
                            let at = shoulder(body, pose, side);
                            let raised = matches!(limb, Limb::Reach(hand) if hand.y <= at.y - 3);
                            if limb != Limb::Rest && !raised {
                                continue;
                            }
                            let probe = canvas.get(body.x + side * (body.rx + 1), at.y + 2);
                            assert_eq!(
                                probe.a > 0,
                                limb == Limb::Rest,
                                "{plan:?} limbs {kind} {clip:?} frame {frame} side {side}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// A four-pawed body stands on both hind paws through everything it does, and on each
    /// forepaw whenever that paw is not in use: what another body does with a hand, it does with
    /// a forepaw, and nothing lifts a hind leg. Up a wall it is the wall its paws are on (see
    /// `a_four_pawed_body_goes_up_a_wall_upright_with_every_paw_on_it`).
    #[test]
    fn a_four_pawed_body_keeps_its_hind_paws_down_and_a_forepaw_down_until_it_uses_it() {
        let creature = preview();
        for limbs in 0..=2 {
            let mut d = creature.appearance.design.unwrap();
            d.body = BodyPlan::Long;
            d.classic.limbs = limbs;
            for size in [0.55, 1.0] {
                for clip in BodyClip::baked() {
                    for frame in 0..AnimationSpec::for_clip(clip).frames {
                        let (canvas, _, pose) = render(&creature, d, clip, frame, size);
                        // Up a wall its paws are on the wall, and in a leap on nothing.
                        if pose.wall || clip == BodyClip::Gesture(Gesture::Leap) {
                            continue;
                        }
                        let body = measure(d, pose, size);
                        let paws = long_limbs(clip, frame, body);
                        for near in [false, true] {
                            let (hind, fore) = leg_roots(body, near);
                            let fore_down = paws[usize::from(near)] == Limb::Rest;
                            for (root, down, leg) in
                                [(hind, true, "hind"), (fore, fore_down, "fore")]
                            {
                                // Anywhere a step can carry the paw, on the rows just above
                                // the floor.
                                let planted = (root.x - 3..=root.x + 4).any(|x| {
                                    (body.floor - 2..=body.floor).any(|y| canvas.get(x, y).a > 0)
                                });
                                if down {
                                    assert!(
                                        planted,
                                        "limbs {limbs} size {size} {clip:?} frame {frame}: the \
                                         {} {leg} paw is off the floor",
                                        if near { "near" } else { "far" }
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// A body this heavy at the front never stands on two paws: it uses one forepaw at a time, a
    /// walk lifts one leg at a time, and nothing it does rears it up onto its hind legs. Only
    /// hanging from a ledge or pulling itself up onto one, with nothing underfoot, takes both
    /// forepaws.
    #[test]
    fn a_four_pawed_body_always_stands_on_three_paws_or_more() {
        let mut creature = preview();
        let mut d = creature.appearance.design.unwrap();
        d.body = BodyPlan::Long;
        // Posed as the long body it is.
        creature.appearance.design = Some(d);
        for clip in BodyClip::baked() {
            if matches!(
                clip,
                BodyClip::Action(ActionKind::Dangle | ActionKind::ClimbWindow)
                    | BodyClip::Gesture(Gesture::Mantle)
            ) {
                continue;
            }
            for frame in 0..AnimationSpec::for_clip(clip).frames {
                let pose = Pose::new(&creature.appearance, clip, frame, false);
                let body = measure(d, pose, 1.0);
                let raised = long_limbs(clip, frame, body)
                    .iter()
                    .filter(|paw| **paw != Limb::Rest)
                    .count();
                let walking = walks(clip) && raised == 0;
                let lifted = [pose.step_a, pose.step_b, -pose.step_a, -pose.step_b]
                    .into_iter()
                    .filter(|step| leg_lift(*step, walking) > 0)
                    .count();
                assert!(
                    raised + lifted <= 1,
                    "{clip:?} frame {frame}: {raised} forepaws in use and {lifted} legs lifted"
                );
            }
        }
    }

    /// A four-pawed body goes up a wall upright: turned a quarter turn with its head uppermost
    /// and all four paws on the wall, their tips over the window's edge line by the grip the
    /// simulation hangs it out by, in every frame of the climb and of the mantle's pull-up. Its
    /// head is carried level above the trunk. Hung from the ledge, a forepaw rests on top of it.
    #[test]
    fn a_four_pawed_body_goes_up_a_wall_upright_with_every_paw_on_it() {
        let mut creature = preview();
        // The edge line for a body hung out as far as a long one is, and the last column its
        // paws' tips reach.
        let wall = FRAME_SIZE as i32 / 2 + 16;
        for limbs in 0..=2 {
            let mut d = creature.appearance.design.unwrap();
            d.body = BodyPlan::Long;
            d.classic.limbs = limbs;
            // Posed as the long body it is drawn as.
            creature.appearance.design = Some(d);
            for size in [0.55, 0.85, 1.0, 1.15] {
                let frames = (0..AnimationSpec::for_action(ActionKind::ClimbWindow).frames)
                    .map(|frame| (BodyClip::Action(ActionKind::ClimbWindow), frame))
                    .chain(
                        (0..formiga_core::MANTLE_PULL_FRAMES)
                            .map(|frame| (BodyClip::Gesture(Gesture::Mantle), frame)),
                    );
                for (clip, frame) in frames {
                    let (canvas, face, pose) = render(&creature, d, clip, frame, size);
                    let label = format!("limbs {limbs} size {size} {clip:?} frame {frame}");
                    assert!(pose.wall, "{label}");
                    let (_, top, right, bottom) = canvas.alpha_bounds().expect("drawn");
                    assert_eq!(right as i32, wall + 1, "{label}: the paws' reach");
                    // The paws' tips are over the wall's edge line, up and down the length of the
                    // trunk: a small body's, or a pair's mid-stride, are drawn one against another.
                    let on_wall: Vec<i32> = (0..FRAME_SIZE as i32)
                        .filter(|y| (wall..=wall + 1).any(|x| canvas.get(x, *y).a > 0))
                        .collect();
                    assert!(
                        on_wall.len() >= 6 && on_wall[on_wall.len() - 1] - on_wall[0] >= 8,
                        "{label}: paws on the wall at rows {on_wall:?}"
                    );
                    // Taller than it is wide, the head at the top of it.
                    assert!(
                        face.y < (top + bottom) as i32 / 2,
                        "{label}: head at {face:?}"
                    );
                    if clip == BodyClip::Gesture(Gesture::Mantle) && frame == 0 {
                        let standing = measure(d, Pose { lean: 0, ..pose }, size);
                        let ledge = mantle_ledge(feet_reach(d, standing.floor), 0, true);
                        let upper = on_wall[0];
                        assert!(
                            (ledge - 5..ledge).contains(&upper),
                            "{label}: the upper forepaw, from row {upper}, rests on a ledge at \
                             row {ledge}"
                        );
                    }
                }
            }
        }
    }

    /// A forepaw in use is held out in front of the face or below it, so it is seen beside the
    /// face rather than drawn across it, and reaching up for a ledge the two go up the sides of
    /// the head, the near one in front of the face and the far one behind. Only covering the eyes
    /// and peeking through them go to the face, which is the point.
    #[test]
    fn a_four_pawed_body_holds_a_forepaw_beside_its_face_not_across_it() {
        let creature = preview();
        let mut d = creature.appearance.design.unwrap();
        d.body = BodyPlan::Long;
        for size in [0.55, 1.0] {
            for clip in BodyClip::baked() {
                if matches!(clip, BodyClip::Gesture(Gesture::Cover | Gesture::Peek)) {
                    continue;
                }
                let hanging = matches!(
                    clip,
                    BodyClip::Action(ActionKind::Dangle | ActionKind::ClimbWindow)
                );
                for frame in 0..AnimationSpec::for_clip(clip).frames {
                    let pose = Pose::new(&creature.appearance, clip, frame, false);
                    let body = measure(d, pose, size);
                    for (paw, near) in long_limbs(clip, frame, body).into_iter().zip([false, true])
                    {
                        let Limb::Reach(at) = paw else {
                            continue;
                        };
                        let beside = if !hanging {
                            at.x >= body.hx + body.head - 3 || at.y >= body.hy + body.head - 3
                        } else if near {
                            at.x >= body.hx + body.head
                        } else {
                            at.x <= body.hx - body.head
                        };
                        assert!(
                            beside,
                            "size {size} {clip:?} frame {frame}: a forepaw at {at:?} is held \
                             across the face"
                        );
                    }
                }
            }
        }
    }

    /// Stick legs stand a body up off the floor the others share, and never move the feet.
    #[test]
    fn stick_legs_lift_the_body_but_keep_the_feet_on_the_floor() {
        let creature = preview();
        for plan in BodyPlan::ALL {
            let mut d = creature.appearance.design.unwrap();
            d.body = plan;
            d.legs = 6;
            let pose = Pose::default();
            let plain = measure(d, pose, 1.0);
            d.classic.limbs = 2;
            let stilts = measure(d, pose, 1.0);
            assert_eq!(plain.floor, stilts.floor, "{plan:?}");
            assert!(
                stilts.y < plain.y,
                "{plan:?} stands no taller on stick legs"
            );
        }
    }

    #[test]
    fn a_stalk_tail_trails_out_behind_lower_than_a_greeting_paw_is_raised() {
        // A greeting paw is raised beside the head, two to four rows above its middle; a stalk
        // held up that high reads as a second paw waving. A flame or a bobble on the end is
        // its own colour and shape, and a flame burns upward, so it is left out.
        let creature = preview();
        let clips = [
            BodyClip::Action(ActionKind::Idle),
            BodyClip::Action(ActionKind::Traverse),
            BodyClip::Action(ActionKind::Greet),
            BodyClip::Gesture(Gesture::Skip),
        ];
        for body in BodyPlan::ALL {
            for tail in [2, 3] {
                for small in [false, true] {
                    let mut d = creature.appearance.design.unwrap();
                    d.body = body;
                    d.tail = tail;
                    d.details.tip = 0;
                    d.width = if small { 8 } else { 12 };
                    d.height = if small { 7 } else { 11 };
                    d.head = if small { 7 } else { 9 };
                    d.legs = if small { 3 } else { 6 };
                    let mut bare = d;
                    bare.tail = 0;
                    let size = if small { 0.55 } else { 1.05 };
                    for clip in clips {
                        for frame in 0..AnimationSpec::for_clip(clip).frames {
                            let (with, _, pose) = render(&creature, d, clip, frame, size);
                            let (without, ..) = render(&creature, bare, clip, frame, size);
                            let top = (0..FRAME_SIZE as i32)
                                .find(|&y| {
                                    (0..FRAME_SIZE as i32)
                                        .any(|x| with.get(x, y) != without.get(x, y))
                                })
                                .expect("the tail is drawn");
                            let at = measure(d, pose, size);
                            assert!(
                                top > at.hy - 2,
                                "{body:?} tail {tail} small={small} {clip:?} \
                                     frame {frame}: the tail reaches row {top}, as high as \
                                     a greeting paw at row {}",
                                at.hy - 2
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_blob_waddles_from_foot_to_foot_and_holds_still_under_reduced_motion() {
        let mut creature = preview();
        let mut d = creature.appearance.design.unwrap();
        d.body = BodyPlan::Blob;
        creature.appearance.design = Some(d);
        // Its walk; it runs as every two-legged body does (see `Pose::stepping`).
        for clip in [BodyClip::Action(ActionKind::Traverse)] {
            let pose = |frame| Pose::new(&creature.appearance, clip, frame, false);
            let x: Vec<i32> = (0..6).map(|frame| measure(d, pose(frame), 1.0).x).collect();
            // Over onto the front foot while the back one is up, and back again.
            assert!(x[1] > x[2] && x[4] < x[2], "{clip:?}: {x:?}");
            // Each foot is up the whole two pixels for both frames it is off the ground.
            for frame in [0, 1] {
                assert!(pose(frame).step_a.abs() >= 2 && pose(frame).step_b == 0);
            }
            for frame in [3, 4] {
                assert!(pose(frame).step_b.abs() >= 2 && pose(frame).step_a == 0);
            }
            let calm = Pose::new(&creature.appearance, clip, 0, true);
            assert_eq!(calm.sway, 0, "{clip:?} sways under reduced motion");
        }
        // Every other two-legged plan goes over onto the foot it stands on as the other swings
        // through, a pixel each way, and is over its feet when either lands; a four-pawed body
        // stands between its feet and does not sway at all.
        let mut round = d;
        round.body = BodyPlan::Round;
        creature.appearance.design = Some(round);
        let sways: Vec<i32> = (0..6)
            .map(|frame| {
                Pose::new(
                    &creature.appearance,
                    BodyClip::Action(ActionKind::Traverse),
                    frame,
                    false,
                )
                .sway
            })
            .collect();
        assert_eq!(sways, [0, 1, 0, 0, -1, 0]);
        let mut long = d;
        long.body = BodyPlan::Long;
        creature.appearance.design = Some(long);
        for frame in 0..6 {
            let pose = Pose::new(
                &creature.appearance,
                BodyClip::Action(ActionKind::Traverse),
                frame,
                false,
            );
            assert_eq!(sway(long, pose), 0);
        }
    }

    /// A two-legged body's walk is a walk rather than a march on the spot: each foot is planted
    /// out ahead, goes back along the ground under the body while it is down, and comes forward
    /// again off the ground. One foot or the other is always down at a walk, and a run has a beat
    /// with both off it. Reduced motion holds the short step it always held, with both feet down.
    #[test]
    fn a_stride_plants_a_foot_ahead_and_carries_it_back_along_the_ground() {
        let mut creature = preview();
        let mut d = creature.appearance.design.unwrap();
        d.body = BodyPlan::Upright;
        creature.appearance.design = Some(d);
        for (action, running) in [(ActionKind::Traverse, false), (ActionKind::Sprint, true)] {
            let poses: Vec<Pose> = (0..6)
                .map(|frame| {
                    Pose::new(&creature.appearance, BodyClip::Action(action), frame, false)
                })
                .collect();
            let feet = |pose: &Pose| {
                let lifts = pose.lifts.expect("a stride says how high its feet are");
                [(pose.step_a, lifts[0]), (pose.step_b, lifts[1])]
            };
            for foot in 0..2 {
                let mut planted_ahead = false;
                for frame in 0..6 {
                    let (at, lift) = feet(&poses[frame])[foot];
                    let (next_at, next_lift) = feet(&poses[(frame + 1) % 6])[foot];
                    planted_ahead |= lift == 0 && at >= 3;
                    if lift == 0 && next_lift == 0 {
                        assert!(
                            next_at < at,
                            "{action:?} foot {foot}: slid forward on the ground"
                        );
                    }
                    if lift > 0 && next_lift > 0 {
                        assert!(
                            next_at > at,
                            "{action:?} foot {foot}: swung back in the air"
                        );
                    }
                }
                assert!(planted_ahead, "{action:?} foot {foot} never lands ahead");
            }
            let airborne = poses
                .iter()
                .filter(|pose| feet(pose).iter().all(|(_, lift)| *lift > 0))
                .count();
            assert_eq!(airborne, if running { 2 } else { 0 }, "{action:?}");
            let still = Pose::new(&creature.appearance, BodyClip::Action(action), 0, true);
            assert!(still.lifts.is_none() && still.sway == 0 && still.swing == 0);
        }
    }
}
