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
    action: ActionKind,
    frame: u8,
) -> PixelPoint {
    let d = design.bounded();
    let long = d.body == BodyPlan::Long;
    // A blob is one soft mass carrying its own face, so it keeps a rounder minimum and
    // never grows the separate head oval every other plan draws.
    let blob = d.body == BodyPlan::Blob;
    let size = size.clamp(0.55, 1.05);
    let mut rx = ((f32::from(d.width) * size).round() as i32 + pose.squash_x).clamp(6, 13);
    let mut ry = ((f32::from(d.height) * size).round() as i32 + pose.squash_y).clamp(5, 11);
    if blob {
        rx = (rx + 2).clamp(8, 13);
        ry = (ry + 2).clamp(7, 11);
    }
    let x = if long { 22 } else { 24 };
    let floor = 40 + pose.bob.clamp(-2, 2) - pose.play_lift.clamp(0, 3);
    // Blobs settle onto stubby feet rather than standing on visible legs.
    let stance = f32::from(d.legs) * size * if blob { 0.4 } else { 1.0 };
    let y = floor - stance.round() as i32 - ry;
    let hx = if long {
        x + (8.0 * size).round() as i32
    } else {
        x
    };
    let hy = match d.body {
        BodyPlan::Round => y - 2,
        BodyPlan::Long => y - 4,
        BodyPlan::Upright | BodyPlan::Winged => y - 6,
        BodyPlan::Blob => y - ry / 3,
    }
    .max(18);
    let head = (f32::from(d.head) * size.sqrt()).round().max(7.0) as i32;
    // Ears ride the top of a blob's mass; every other plan hangs them off the head.
    let (ear_cx, ear_span, ear_top) = if blob {
        (x, rx - 4, y - ry + 2)
    } else {
        (hx, head - 3, hy - head + 2)
    };

    // Tail and ears go behind the body and never through the reserved face area.
    let tx = x - rx + 1;
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
    let ear = (f32::from(d.ear_size) * size).round().max(3.0) as i32;
    for side in [-1, 1] {
        let ex = ear_cx + side * ear_span;
        let ey = ear_top;
        match d.ears {
            EarStyle::None => {}
            EarStyle::Round => oval(c, p, ex, ey - 1, 3, 3, p.accent),
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
            oval(
                c,
                p,
                x + dx + 1,
                floor - 2 + pose.step_b.clamp(-1, 1),
                2,
                2,
                p.shadow,
            );
        }
    }
    // Gestures remain outside the face. Dangle hands share the existing y=7 ledge anchor.
    if matches!(
        action,
        ActionKind::Dangle
            | ActionKind::ClimbWindow
            | ActionKind::Greet
            | ActionKind::SocialPlay
            | ActionKind::PresentDiscovery
            | ActionKind::InvestigateCursor
    ) {
        for side in [-1, 1] {
            let ax = (hx + side * (head + 3)).clamp(4, 43);
            let ay = match action {
                ActionKind::Dangle => 7,
                ActionKind::ClimbWindow => {
                    hy - 7
                        + if side < 0 {
                            i32::from(frame % 2) * 3
                        } else {
                            3 - i32::from(frame % 2) * 3
                        }
                }
                ActionKind::PresentDiscovery => hy - 5,
                _ if side > 0 => hy - 2 - i32::from(frame % 2) * 2,
                _ => y + 4,
            };
            c.line(x + side * (rx - 1), y + 2, ax, ay, 2, p.outline);
            c.line(x + side * (rx - 1), y + 2, ax, ay, 1, p.coat);
            oval(c, p, ax, ay, 2, 2, p.coat);
        }
    }
    oval(c, p, x, y, rx, ry, p.coat);
    c.fill_ellipse(x, y + ry / 2, rx - 3, (ry / 2).max(3), p.shadow);
    match d.marking {
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
    for (side, step) in [(-1, pose.step_a), (1, pose.step_b)] {
        let fx = x + side * (rx - 4) + step.clamp(-2, 2);
        c.line(
            x + side * (rx - 4),
            y + ry - 2,
            fx,
            floor - 2 - step.abs().min(2),
            2,
            p.outline,
        );
        c.line(
            x + side * (rx - 4),
            y + ry - 2,
            fx,
            floor - 2 - step.abs().min(2),
            1,
            p.coat,
        );
        oval(c, p, fx, floor - 1 - step.abs().min(2), 3, 2, p.coat);
        if !long {
            let ax = x + side * (rx - 1);
            let ay = y + 3 - pose.appendage_lift.clamp(-2, 3);
            if d.body == BodyPlan::Winged {
                draw_wing(c, p, wing_style(d), ax, ay, side);
            } else {
                oval(c, p, ax, ay, 2, 3, p.coat);
            }
        }
    }
    if !blob {
        oval(c, p, hx, hy, head, head - 1, p.coat);
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
    c.fill_ellipse(
        hx - 2,
        if blob { y - ry + 3 } else { hy - head + 3 },
        3,
        1,
        p.highlight,
    );
    PixelPoint { x: hx, y: hy }
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

    #[test]
    fn every_wing_style_is_reachable_deterministic_and_visibly_textured() {
        let creature = formiga_core::World::preview_adult(
            [55; 32],
            time::OffsetDateTime::UNIX_EPOCH,
            &formiga_core::DesktopSnapshot::default(),
        );
        let p = crate::palette_for(&creature.appearance);
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
            let mut c = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            let pose = Pose::new(&creature.appearance, ActionKind::Idle, 0, false);
            draw(&mut c, d, p, pose, 1.0, ActionKind::Idle, 0);
            rendered.push(c);
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
        let creature = formiga_core::World::preview_adult(
            [55; 32],
            time::OffsetDateTime::UNIX_EPOCH,
            &formiga_core::DesktopSnapshot::default(),
        );
        let p = crate::palette_for(&creature.appearance);
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
                        let mut c = Canvas::new(FRAME_SIZE, FRAME_SIZE);
                        let pose = Pose::new(&creature.appearance, ActionKind::Idle, 0, false);
                        let anchor = draw(
                            &mut c,
                            d,
                            p,
                            pose,
                            if small { 0.55 } else { 1.05 },
                            ActionKind::Idle,
                            0,
                        );
                        for dx in -5..=5 {
                            for dy in -3..=3 {
                                assert!(
                                    c.get(anchor.x + dx, anchor.y + dy).a > 0,
                                    "reserved face {d:?}"
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
                            "disconnected part {d:?}, small={small}"
                        );
                    }
                }
            }
        }
    }
}
