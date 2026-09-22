//! Bounded, interchangeable pixel parts. No image tracing or unbounded geometry.
use super::*;
use formiga_core::{BodyPlan, CreatureDesign, EarStyle};

fn oval(c: &mut Canvas, p: Palette, x: i32, y: i32, rx: i32, ry: i32, color: Rgba) {
    c.fill_ellipse(x, y, rx + 1, ry + 1, p.outline);
    c.fill_ellipse(x, y, rx, ry, color);
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
    let k = d.classic;
    let long = d.body == BodyPlan::Long;
    // A blob is one soft mass carrying its own face, so it keeps a rounder minimum and
    // never grows the separate head oval every other plan draws.
    let blob = d.body == BodyPlan::Blob;
    let body = measure(d, pose, size);
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
    let size = size.clamp(0.55, 1.05);
    // Ears ride the top of a blob's mass; every other plan hangs them off the head.
    let (ear_cx, ear_span, ear_top) = if blob {
        (x, rx - 4, y - ry + 2)
    } else {
        (hx, head - 3, hy - head + 2)
    };

    // Tail and ears go behind the body and never through the reserved face area.
    let tx = x - rx + 1;
    if k.tail > 0 {
        classic_tail(c, p, k.tail, tx, y, pose);
    } else {
        match d.tail {
            1 => oval(c, p, tx - 2, y + 1, 3, 3, p.highlight),
            2 | 3 => {
                let ty = y - 3 + pose.tail_sway.clamp(-2, 2);
                c.line(tx, y + 3, (tx - 6).max(3), ty, 3, p.outline);
                c.line(tx, y + 3, (tx - 6).max(3), ty, 2, p.accent);
                if d.tail == 3 {
                    oval(c, p, (tx - 6).max(4), ty - 2, 2, 3, p.accent);
                }
            }
            4 => oval(c, p, tx - 2, y, 4, 5, p.accent),
            _ => {}
        }
    }
    // A pricked ear grows out of the base it already had, so it stays joined to the head.
    let ear = (f32::from(d.ear_size) * size).round().max(3.0) as i32 + pose.ear_perk.clamp(0, 2);
    if k.crown > 0 {
        classic_crown(c, p, k.crown, ear_cx, ear_span, ear_top, ear, pose);
    }
    for side in [-1, 1] {
        let ex = ear_cx + side * ear_span;
        let ey = ear_top;
        match d.ears {
            _ if k.crown > 0 => {}
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
    // Four-pawed plans have two staggered back feet, all below the face.
    if long {
        for dx in [-rx + 3, rx - 3] {
            c.line(x + dx + 1, y + ry - 2, x + dx + 1, floor - 2, 2, p.outline);
            let at = PixelPoint {
                x: x + dx + 1,
                y: floor - 2 + pose.step_b.clamp(-1, 1),
            };
            if k.limbs > 0 {
                classic_foot(c, p, k.limbs, at);
            } else {
                oval(c, p, at.x, at.y, 2, 2, p.shadow);
            }
        }
    }
    let limbs = limbs(clip, frame, body, d.body);
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
    let shoulder = |side: i32| shoulder(body, pose, side);
    for ((side, step), limb) in [(-1, pose.step_a), (1, pose.step_b)].into_iter().zip(limbs) {
        // A four-pawed plan lifts the front paw it gestures with off the ground.
        if !(long && limb != Limb::Rest) {
            let fx = x + side * (rx - 4) + step.clamp(-2, 2);
            let lift = step.abs().min(2);
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
        }
        if !long && limb == Limb::Rest {
            let at = shoulder(side);
            if d.body == BodyPlan::Winged {
                // A folded wing hangs from the shoulder and its outline reaches six rows below
                // it, so a body settling onto its legs, as a sleeper does with each breath, would
                // carry the tip down past the feet. The wing comes to rest on the ground instead.
                let lowest = feet_reach(d, floor) - 6;
                draw_wing(c, p, wing_style(d), at.x, at.y.min(lowest), side);
            } else if k.limbs > 0 {
                classic_nub(c, p, at, side);
            } else {
                oval(c, p, at.x, at.y, 2, 3, p.coat);
            }
        }
    }
    if !blob {
        if k.coat > 0 {
            lit_oval(c, p, hx, hy, head, head - 1, p.coat);
        } else {
            oval(c, p, hx, hy, head, head - 1, p.coat);
        }
    }
    if d.muzzle > 0 {
        c.fill_ellipse(
            hx,
            hy + 3,
            if d.muzzle == 2 { 5 } else { 3 },
            3,
            p.highlight,
        );
    }
    // Candy coats are lit from above already, so only the modular one carries a painted gleam.
    if k.coat == 0 {
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
    let arm = if k.limbs > 0 { classic_arm } else { draw_arm };
    for (side, limb) in [-1, 1].into_iter().zip(limbs) {
        let Limb::Reach(hand) = limb else {
            continue;
        };
        if long {
            // The front paw comes up from the flank, where its leg was planted.
            let root = PixelPoint {
                x: x + side * (rx - 3),
                y: y + 2,
            };
            arm(c, p, root, hand);
        } else if d.body == BodyPlan::Winged {
            draw_open_wing(c, p, wing_style(d), shoulder(side), hand, side);
        } else {
            arm(c, p, shoulder(side), hand);
        }
    }
    PixelPoint { x: hx, y: hy }
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

/// Where this body actually holds what it is using, measured the same way the body itself is
/// measured. A toy, a snack, or a cup is placed from here rather than from a guess against the
/// face anchor, so the paw that is drawn is the paw the thing sits in. Nothing about the limbs
/// changes: this only reads the same numbers `draw` does.
pub(super) fn prop_hold(design: CreatureDesign, pose: Pose, size: f32) -> PropHold {
    let d = design.bounded();
    let body = measure(d, pose, size);
    let long = d.body == BodyPlan::Long;
    let hands = if long {
        // A long body keeps all four paws down, so it plays with what is on the floor in front of
        // its forepaws rather than with something held up at a shoulder.
        PixelPoint {
            x: body.x + body.rx + 1,
            y: body.floor - 3,
        }
    } else {
        // The folded near paw, which `draw` puts one body-radius out and three rows down.
        let at = shoulder(body, pose, 1);
        PixelPoint {
            x: at.x + 2,
            y: at.y + 1,
        }
    };
    PropHold {
        hands,
        // In front of the muzzle and below the eyes, so a mouthful is carried up to the mouth
        // rather than held over the face the layered art draws on this head.
        mouth: PixelPoint {
            x: body.hx + body.head - 1,
            y: body.hy + 5,
        },
        floor: body.floor,
        ground: feet_reach(d, body.floor),
        forward: 1,
    }
}

/// One side's paw or wing in a frame: folded where it rests, or carried out to a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Limb {
    Rest,
    Reach(PixelPoint),
}

/// The measurements every part of a modular body is placed from, in frame pixels.
#[derive(Clone, Copy, Debug)]
struct Body {
    /// Centre and radii of the body mass.
    x: i32,
    y: i32,
    rx: i32,
    ry: i32,
    /// Centre and radius of the head, which is also the face anchor.
    hx: i32,
    hy: i32,
    head: i32,
    /// The row the feet stand on.
    floor: i32,
}

fn measure(d: CreatureDesign, pose: Pose, size: f32) -> Body {
    let long = d.body == BodyPlan::Long;
    let blob = d.body == BodyPlan::Blob;
    let size = size.clamp(0.55, 1.05);
    let mut rx = ((f32::from(d.width) * size).round() as i32 + pose.squash_x).clamp(6, 13);
    let mut ry = ((f32::from(d.height) * size).round() as i32 + pose.squash_y).clamp(5, 11);
    if blob {
        rx = (rx + 2).clamp(8, 13);
        ry = (ry + 2).clamp(7, 11);
    }
    let x = if long { 22 } else { 24 };
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
            (false, false) => 1.0,
            (false, true) => 1.35,
        };
    // A crouch folds the legs rather than sinking the feet, so contact with the surface holds.
    let stance = (stance.round() as i32 - pose.crouch.clamp(0, 4)).max(0);
    // A settle can only fold the legs it has: a plan that already stands low sinks less rather
    // than pressing its body down through the floor it is standing on.
    let sink = pose.bob.clamp(-2, 2).min(stance);
    let y = floor - stance - ry + sink - pose.play_lift.clamp(0, 3);
    let hx = if long {
        x + (8.0 * size).round() as i32
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
        BodyPlan::Long => y - 4,
        BodyPlan::Upright | BodyPlan::Winged => y - 6,
        BodyPlan::Blob => y - ry / 3,
    }
    .max(18);
    let head = (f32::from(d.head) * size.sqrt()).round().max(7.0) as i32;
    // A head larger than a body that a crouch has squashed flat would reach past the feet, since
    // it hangs from the body's centre. Like everything else, it stops at the ground; its outline
    // reaches `head` rows below its centre.
    let hy = hy.min(feet_reach(d, floor) - head);
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

/// The lowest row a body's feet reach: one below the floor for the classic feet, and two for the
/// rounder modular ones. Nothing a body folds against itself goes lower.
fn feet_reach(d: CreatureDesign, floor: i32) -> i32 {
    floor + if d.classic.limbs > 0 { 1 } else { 2 }
}

/// Where a side's paw or wing sits folded against the body, and where it is carried out from.
fn shoulder(body: Body, pose: Pose, side: i32) -> PixelPoint {
    PixelPoint {
        x: body.x + side * (body.rx - 1),
        y: body.y + 3 - pose.appendage_lift.clamp(-2, 3),
    }
}

/// Where each side's limb is this frame, back side first. There is exactly one limb per side:
/// the paw or wing a creature rests against its body is the one it raises, so no gesture ever
/// adds an appendage beside the one that was already there.
fn limbs(clip: BodyClip, frame: u8, body: Body, plan: BodyPlan) -> [Limb; 2] {
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
    let aimed = match clip {
        // Dangling hands share the existing y=7 ledge anchor.
        BodyClip::Action(ActionKind::Dangle) => both(&|side| to(beside_head(side), 7)),
        BodyClip::Action(ActionKind::ClimbWindow) => both(&|side| {
            let hold = if side < 0 { tick * 3 } else { 3 - tick * 3 };
            to(beside_head(side), hy - 7 + hold)
        }),
        BodyClip::Action(ActionKind::PresentDiscovery) => {
            both(&|side| to(beside_head(side), hy - 5))
        }
        BodyClip::Action(
            ActionKind::Greet | ActionKind::SocialPlay | ActionKind::InvestigateCursor,
        ) => front(to(beside_head(1), hy - 2 - tick * 2)),
        BodyClip::Action(_) => [Limb::Rest; 2],
        BodyClip::Gesture(Gesture::Cheer) => {
            both(&|side| to(beside_head(side), hy - head - 2 - tick))
        }
        BodyClip::Gesture(Gesture::Gasp) => both(&|side| to(x + side * (rx + 5), y - 5 - tick)),
        BodyClip::Gesture(Gesture::Cover) => {
            both(&|side| to(hx + side * 3, hy - 1 + if side > 0 { tick * 2 } else { 0 }))
        }
        // Paws wrung together below the chin, clear of the mouth.
        BodyClip::Gesture(Gesture::Worry) => [to(hx - 2, y + 6 + beat), to(hx + 2, y + 5 - beat)],
        // Wings fold and long bodies keep all four paws down; everyone else plants their paws.
        BodyClip::Gesture(Gesture::Crouch) if matches!(plan, BodyPlan::Long | BodyPlan::Winged) => {
            [Limb::Rest; 2]
        }
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
        // A long body stretches along the ground with every paw planted.
        BodyClip::Gesture(Gesture::Stretch) if plan == BodyPlan::Long => [Limb::Rest; 2],
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
    };
    // A long body's far paw would have to cross the whole body to reach its face or its chest,
    // so it stays planted and the near paw makes the gesture alone.
    if plan == BodyPlan::Long
        && matches!(
            clip,
            BodyClip::Gesture(Gesture::Cover | Gesture::Worry | Gesture::Heave)
        )
    {
        return [Limb::Rest, aimed[1]];
    }
    aimed
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
        let pose = Pose::new(&creature.appearance, clip, frame, false);
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
        assert_eq!(
            seen.iter().filter(|v| **v).count(),
            count,
            "disconnected part {label}"
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
                            ActionKind::Dangle
                            | ActionKind::ClimbWindow
                            | ActionKind::PresentDiscovery => [true, true],
                            ActionKind::Greet
                            | ActionKind::SocialPlay
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
                    for (side, limb) in [-1, 1].into_iter().zip(limbs) {
                        let at = shoulder(body, pose, side);
                        let raised = matches!(limb, Limb::Reach(hand) if hand.y <= at.y - 3);
                        if limb != Limb::Rest && !raised {
                            continue;
                        }
                        // Just outside the body, below the shoulder, is covered by the folded
                        // paw or wing and by nothing else. Raising that limb has to clear it.
                        let below = if plan == BodyPlan::Winged { 4 } else { 2 };
                        let probe = canvas.get(body.x + side * (body.rx + 1), at.y + below);
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
                                "{label} leaves the frame"
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
}
