//! The original families (blob, hopper and soft quadruped): bodies, heads, limbs, tails, feet and
//! markings.
use super::*;

pub(super) fn draw_blob(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    pose: Pose,
    clip: BodyClip,
    frame: u8,
) -> Figure {
    let s = scale(genome);
    let rx = ((genome.body_width as f32 * s / 2.0).round() as i32 + pose.squash_x).clamp(6, 16);
    // A blob carries its feet on the underside of the one mass it is, so it has no legs to fold
    // and a bob that moved the mass would carry the ground with it. It squashes instead: the
    // bottom of the silhouette stays on the same row in every clip, and a settle or a step reads
    // as the blob pressing down and springing back rather than hopping off the floor.
    let ry = ((genome.body_height as f32 * s / 2.0).round() as i32 + pose.squash_y
        - pose.bob.clamp(-2, 2)
        + pose.play_lift.clamp(0, 3))
    .clamp(5, 14);
    let cx = 26;
    let cy = 38 - ry;
    // A blob is one mass with its face on it, so leaning carries the face and the ears together.
    let lean = pose.lean.clamp(-2, 2);
    draw_tail(canvas, genome, palette, cx - rx + 1, cy, s, pose);
    draw_head_appendages(canvas, genome, palette, cx + lean, cy - ry + 2, s, pose);
    canvas.fill_ellipse(cx, cy + 1, rx + 1, ry + 1, palette.outline);
    canvas.fill_ellipse(cx, cy, rx, ry, palette.coat);
    canvas.fill_ellipse(
        cx - 2,
        cy + ry / 2,
        (rx - 2).max(2),
        (ry / 3).max(2),
        palette.shadow,
    );
    canvas.fill_ellipse(cx, cy - 1, rx - 2, (ry - 2).max(2), palette.coat);
    apply_pattern(canvas, genome, palette, cx, cy, rx - 1, ry - 1);
    draw_feet(canvas, palette, cx, cy + ry - 1, rx, genome.foot_size, pose);
    draw_forelimbs(
        canvas,
        genome,
        palette,
        LimbPose {
            left_root: PixelPoint {
                x: cx - rx + 2,
                y: cy,
            },
            right_root: PixelPoint {
                x: cx + rx - 2,
                y: cy,
            },
            clip,
            frame,
            pose,
            family: BodyFamily::Blob,
        },
    );
    // One mass carrying its face: the "neck" a collar goes round is a band across the mass just
    // under the face, as wide as the mass is there.
    let face = PixelPoint {
        x: cx + 2 + lean,
        y: cy - 1,
    };
    let neck_y = (face.y + 5).min(cy + ry - 2);
    Figure {
        face,
        crown: PixelPoint {
            x: cx + lean,
            y: cy - ry,
        },
        head_half: (rx - 2).max(3),
        neck: PixelPoint { x: cx, y: neck_y },
        neck_half: ellipse_half_width(rx, ry, neck_y - cy),
        chest: PixelPoint {
            x: cx + rx / 2,
            y: neck_y + 2,
        },
        hip: PixelPoint {
            x: cx - rx + 2,
            y: cy + ry / 2,
        },
        back: PixelPoint {
            x: cx - rx + 2,
            y: cy - ry / 3,
        },
        floor: cy + ry + 1,
    }
}

/// Half the width of an ellipse `dy` rows from its middle.
pub(super) fn ellipse_half_width(rx: i32, ry: i32, dy: i32) -> i32 {
    let t = (dy as f32 / ry.max(1) as f32).clamp(-1.0, 1.0);
    ((rx as f32) * (1.0 - t * t).max(0.0).sqrt()).round() as i32
}

pub(super) fn draw_hopper(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    pose: Pose,
    clip: BodyClip,
    frame: u8,
) -> Figure {
    let s = scale(genome);
    // A rounder, lower crouch reads closer to a resting rabbit and leaves headroom for long ears.
    let rx = ((genome.body_width as f32 * s * 0.42).round() as i32 + pose.squash_x).clamp(6, 11);
    let ry = ((genome.body_height as f32 * s * 0.48).round() as i32 + pose.squash_y).clamp(7, 12);
    let leg = ((genome.leg_length as f32 * s).round() as i32).clamp(3, 7);
    let cx = 24;
    let ground = 43;
    // A crouch folds the legs: the body sinks and the feet stay on the ground.
    let cy = ground - leg - ry + pose.bob - pose.play_lift + pose.crouch.clamp(0, leg - 2);
    // A hopper carries its face on the front of its mass, so the ears and the face lean together.
    let lean = pose.lean.clamp(-2, 2);
    // Rooted low on the rear edge so the puff clears the body ellipse drawn over it.
    draw_tail(canvas, genome, palette, cx - rx, cy + ry / 2, s, pose);
    draw_head_appendages(canvas, genome, palette, cx + lean, cy - ry + 2, s, pose);
    draw_hopper_leg(
        canvas,
        palette,
        cx - rx / 2,
        cy + ry - 2,
        ground,
        -2 + pose.step_a,
    );
    draw_hopper_leg(
        canvas,
        palette,
        cx + rx / 2,
        cy + ry - 2,
        ground,
        3 + pose.step_b,
    );
    canvas.fill_ellipse(cx, cy, rx + 1, ry + 1, palette.outline);
    canvas.fill_ellipse(cx, cy - 1, rx, ry, palette.coat);
    canvas.fill_ellipse(cx - 2, cy + 3, rx - 2, (ry / 3).max(2), palette.shadow);
    canvas.fill_ellipse(cx, cy - 3, rx - 2, ry - 4, palette.coat);
    apply_pattern(canvas, genome, palette, cx, cy, rx - 1, ry - 1);
    draw_forelimbs(
        canvas,
        genome,
        palette,
        LimbPose {
            left_root: PixelPoint {
                x: cx - rx + 2,
                y: cy,
            },
            right_root: PixelPoint {
                x: cx + rx - 2,
                y: cy,
            },
            clip,
            frame,
            pose,
            family: BodyFamily::Hopper,
        },
    );
    // The face rides the front of the one mass, which is drawn a row up from `cy`.
    Figure {
        face: PixelPoint {
            x: cx + 1 + lean,
            y: cy - 2,
        },
        crown: PixelPoint {
            x: cx + lean,
            y: cy - ry - 1,
        },
        head_half: (rx - 2).max(3),
        neck: PixelPoint {
            x: cx + 1,
            y: cy + 2,
        },
        neck_half: ellipse_half_width(rx, ry, 3),
        chest: PixelPoint {
            x: cx + rx / 2 + 1,
            y: cy + 4,
        },
        hip: PixelPoint {
            x: cx - rx + 1,
            y: cy + ry / 2,
        },
        back: PixelPoint {
            x: cx - rx + 2,
            y: cy - ry / 2,
        },
        floor: ground,
    }
}

pub(super) fn draw_quadruped(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    pose: Pose,
    clip: BodyClip,
    frame: u8,
) -> Figure {
    let s = scale(genome);
    let body_rx =
        ((genome.body_width as f32 * s * 0.42).round() as i32 + pose.squash_x).clamp(7, 13);
    let body_ry =
        ((genome.body_height as f32 * s * 0.34).round() as i32 + pose.squash_y).clamp(4, 8);
    let leg = ((genome.leg_length as f32 * s).round() as i32).clamp(3, 8);
    let ground = 43;
    let body_y = ground - leg - body_ry + pose.bob - pose.play_lift + pose.crouch.clamp(0, leg - 2);
    let body_x = 22;
    // A head smaller than the body keeps the feline proportion the ears and tail build on.
    let head_radius = ((body_ry as f32 * genome.head_ratio * 0.85).round() as i32 + 2).clamp(5, 8);
    // Rooted inside the upper rear of the body, so the body ellipse buries the base and the tail
    // reads as attached rather than floating behind the rump.
    draw_tail(
        canvas,
        genome,
        palette,
        body_x - body_rx + 2,
        body_y - body_ry / 2,
        s,
        pose,
    );
    draw_quad_leg(
        canvas,
        palette,
        body_x - body_rx / 2,
        body_y + body_ry - 1,
        ground,
        pose.step_b,
        false,
    );
    draw_quad_leg(
        canvas,
        palette,
        body_x + body_rx / 2 - 1,
        body_y + body_ry - 1,
        ground,
        pose.step_a,
        false,
    );
    canvas.fill_ellipse(
        body_x,
        body_y + 1,
        body_rx + 1,
        body_ry + 1,
        palette.outline,
    );
    canvas.fill_ellipse(body_x, body_y, body_rx, body_ry, palette.coat);
    canvas.fill_ellipse(
        body_x - 3,
        body_y + body_ry / 2,
        body_rx - 3,
        (body_ry / 2).max(2),
        palette.shadow,
    );
    canvas.fill_ellipse(body_x, body_y - 1, body_rx - 2, body_ry - 2, palette.coat);
    apply_pattern(
        canvas,
        genome,
        palette,
        body_x,
        body_y,
        body_rx - 1,
        body_ry - 1,
    );
    draw_quadruped_forelimbs(
        canvas,
        genome,
        palette,
        LimbPose {
            left_root: PixelPoint {
                x: body_x + body_rx / 5,
                y: body_y + body_ry - 1,
            },
            right_root: PixelPoint {
                x: body_x + body_rx / 2,
                y: body_y + body_ry - 1,
            },
            clip,
            frame,
            pose,
            family: BodyFamily::SoftQuadruped,
        },
        ground,
    );
    // A cat has a head of its own, so the whole head — ears, muzzle and reserved face — leans
    // out over the forward paws while the body and the legs stay exactly where they stood.
    let head_x = body_x + body_rx - 1 + pose.lean.clamp(-3, 3);
    let head_y = body_y - 2;
    // Rooted at the crown so the head circle only buries the base of each ear.
    draw_head_appendages(
        canvas,
        genome,
        palette,
        head_x,
        head_y - head_radius,
        s,
        pose,
    );
    canvas.fill_circle(head_x, head_y, head_radius + 1, palette.outline);
    canvas.fill_circle(head_x, head_y - 1, head_radius, palette.coat);
    // A small muzzle on the lower front of the head, sitting under the composited mouth.
    let muzzle_x = head_x + (head_radius - 3).clamp(1, 3);
    canvas.fill_ellipse(muzzle_x, head_y + 2, 2, 1, palette.highlight);
    canvas.set(muzzle_x, head_y, palette.accent);
    Figure {
        face: PixelPoint {
            x: head_x + 1,
            y: head_y - 1,
        },
        crown: PixelPoint {
            x: head_x,
            y: head_y - head_radius - 1,
        },
        head_half: head_radius,
        neck: PixelPoint {
            x: head_x - 2,
            y: head_y + head_radius - 1,
        },
        neck_half: (head_radius - 1).max(3),
        chest: PixelPoint {
            x: head_x - 1,
            y: head_y + head_radius + 1,
        },
        hip: PixelPoint {
            x: body_x - body_rx + 2,
            y: body_y,
        },
        back: PixelPoint {
            x: body_x - 2,
            y: body_y - body_ry,
        },
        floor: ground,
    }
}

pub(super) fn draw_head_appendages(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    cx: i32,
    root_y: i32,
    s: f32,
    pose: Pose,
) {
    let size = ((genome.head_appendages.size as f32 * s).round() as i32).clamp(2, 8)
        + pose.ear_perk.clamp(0, 2);
    match genome.family {
        // Cats and rabbits always keep ears; the style gene varies their shape instead of
        // removing them, so both families stay recognizable across every genome.
        BodyFamily::SoftQuadruped => {
            draw_cat_ears(canvas, palette, cx, root_y, size, genome, pose);
            return;
        }
        BodyFamily::Hopper => {
            draw_rabbit_ears(canvas, palette, cx, root_y, size, genome, pose);
            return;
        }
        BodyFamily::Blob => {}
    }
    match genome.head_appendages.style {
        HeadAppendageStyle::None => {}
        HeadAppendageStyle::Round => {
            canvas.fill_circle(cx - 5, root_y, size, palette.outline);
            canvas.fill_circle(cx + 5, root_y, size, palette.outline);
            canvas.fill_circle(cx - 5, root_y, size - 1, palette.accent);
            canvas.fill_circle(cx + 5, root_y, size - 1, palette.accent);
        }
        HeadAppendageStyle::Pointed | HeadAppendageStyle::Leaf => {
            let spread = if genome.head_appendages.style == HeadAppendageStyle::Leaf {
                7
            } else {
                5
            };
            canvas.line(
                cx - 4,
                root_y + 2,
                cx - spread,
                root_y - size - pose.appendage_lift,
                2,
                palette.outline,
            );
            canvas.line(
                cx + 4,
                root_y + 2,
                cx + spread,
                root_y - size - pose.appendage_lift,
                2,
                palette.outline,
            );
            canvas.line(
                cx - 4,
                root_y + 1,
                cx - spread,
                root_y - size + 1,
                1,
                palette.accent,
            );
            canvas.line(
                cx + 4,
                root_y + 1,
                cx + spread,
                root_y - size + 1,
                1,
                palette.accent,
            );
        }
        HeadAppendageStyle::Droop => {
            canvas.line(cx - 4, root_y, cx - 8, root_y + size, 2, palette.outline);
            canvas.line(cx + 4, root_y, cx + 8, root_y + size, 2, palette.outline);
            canvas.line(cx - 4, root_y, cx - 8, root_y + size - 1, 1, palette.accent);
            canvas.line(cx + 4, root_y, cx + 8, root_y + size - 1, 1, palette.accent);
        }
        HeadAppendageStyle::Antenna => {
            canvas.line(
                cx - 3,
                root_y + 1,
                cx - 5,
                root_y - size - pose.bob,
                1,
                palette.outline,
            );
            canvas.line(
                cx + 3,
                root_y + 1,
                cx + 5,
                root_y - size + pose.bob,
                1,
                palette.outline,
            );
            canvas.fill_circle(cx - 5, root_y - size - pose.bob, 1, palette.accent);
            canvas.fill_circle(cx + 5, root_y - size + pose.bob, 1, palette.accent);
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct EarShape {
    pub(super) base_x: i32,
    pub(super) base_y: i32,
    pub(super) half_width: i32,
    pub(super) height: i32,
    pub(super) lean: i32,
}

/// One upright triangular ear, drawn as stacked rows so the tip stays pointed. Each `inset` step
/// shrinks the triangle by a pixel, layering the coat and inner ear inside the outline.
pub(super) fn fill_triangle_ear(canvas: &mut Canvas, ear: EarShape, inset: i32, color: Rgba) {
    for step in inset..=(ear.height - inset) {
        let progress = step as f32 / ear.height.max(1) as f32;
        let width = (ear.half_width as f32 * (1.0 - progress)).round() as i32 - inset;
        if width < 0 {
            continue;
        }
        let x = ear.base_x + (ear.lean as f32 * progress).round() as i32;
        canvas.fill_rect(x - width, ear.base_y - step, width * 2 + 1, 1, color);
    }
}

pub(super) fn draw_cat_ears(
    canvas: &mut Canvas,
    palette: Palette,
    cx: i32,
    root_y: i32,
    size: i32,
    genome: &AppearanceGenome,
    pose: Pose,
) {
    let base_y = root_y + 1;
    let lift = pose.appendage_lift.clamp(-1, 1);
    // A base at least three pixels either side of centre leaves room for the coat and inner-ear
    // passes; anything narrower collapses into a solid outline nub.
    let (half_width, height, lean) = match genome.head_appendages.style {
        // Rounded and folded sets stay short and tip further outward.
        HeadAppendageStyle::Round => ((size / 2).clamp(3, 4), (size + 1).clamp(3, 5), 2),
        HeadAppendageStyle::Droop => ((size / 2).clamp(3, 4), size.clamp(3, 4), 3),
        // Tufted sets stand tall and nearly straight.
        HeadAppendageStyle::Leaf | HeadAppendageStyle::Antenna => {
            ((size / 2).clamp(3, 4), (size + 3).clamp(5, 8), 1)
        }
        HeadAppendageStyle::None | HeadAppendageStyle::Pointed => {
            ((size / 2).clamp(3, 4), (size + 2).clamp(4, 7), 2)
        }
    };
    let height = (height + lift).clamp(2, (base_y - 2).max(2));
    for direction in [-1, 1] {
        let ear = EarShape {
            base_x: cx + 4 * direction,
            base_y,
            half_width,
            height,
            lean: lean * direction,
        };
        fill_triangle_ear(canvas, ear, 0, palette.outline);
        fill_triangle_ear(canvas, ear, 1, palette.coat);
        fill_triangle_ear(canvas, ear, 2, palette.accent);
    }
}

pub(super) fn draw_rabbit_ears(
    canvas: &mut Canvas,
    palette: Palette,
    cx: i32,
    root_y: i32,
    size: i32,
    genome: &AppearanceGenome,
    pose: Pose,
) {
    let lift = pose.appendage_lift.clamp(-1, 1);
    if genome.head_appendages.style == HeadAppendageStyle::Droop {
        // A lop set falls alongside the head instead of standing up.
        let reach = (size + 2).clamp(4, 7);
        let drop = (size + 3).clamp(5, 9);
        for direction in [-1, 1] {
            let base_x = cx + 3 * direction;
            let tip_x = base_x + reach * direction;
            let tip_y = root_y + drop + lift;
            canvas.line(base_x, root_y, tip_x, tip_y, 3, palette.outline);
            canvas.line(base_x, root_y, tip_x, tip_y - 1, 2, palette.coat);
            canvas.line(base_x, root_y + 1, tip_x, tip_y - 1, 1, palette.accent);
        }
        return;
    }
    let height = match genome.head_appendages.style {
        HeadAppendageStyle::Round => (size + 2).clamp(4, 7),
        _ => (size + 6).clamp(7, 14),
    };
    // Thickness 3 rounds the tip two pixels past `tip_y`, so leave that much headroom.
    let height = (height + lift).clamp(3, (root_y - 3).max(3));
    for direction in [-1, 1] {
        let base_x = cx + 3 * direction;
        let tip_x = base_x + direction;
        let tip_y = root_y - height;
        canvas.line(base_x, root_y, tip_x, tip_y, 3, palette.outline);
        canvas.line(base_x, root_y - 1, tip_x, tip_y + 1, 2, palette.coat);
        canvas.line(
            tip_x,
            tip_y + 2,
            base_x,
            root_y - height / 2,
            1,
            palette.accent,
        );
    }
}

#[derive(Clone, Copy)]
pub(super) struct LimbPose {
    pub(super) left_root: PixelPoint,
    pub(super) right_root: PixelPoint,
    pub(super) clip: BodyClip,
    pub(super) frame: u8,
    pub(super) pose: Pose,
    pub(super) family: BodyFamily,
}

pub(super) fn draw_forelimbs(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    limb_pose: LimbPose,
) {
    // Quadruped genes share the same stored range as upright families, but reading that full range
    // as an arm puts paws across the chest and face. Keep expressive cat paws compact; ordinary
    // actions still use the planted-leg rig below.
    let length = match limb_pose.family {
        BodyFamily::SoftQuadruped => i32::from(genome.forelimbs.length).min(4),
        _ => i32::from(genome.forelimbs.length),
    };
    let (left_target, right_target) = limb_targets(
        limb_pose.left_root,
        limb_pose.right_root,
        limb_pose.clip,
        limb_pose.frame,
        length,
        genome.forelimbs.rest_pose,
        limb_pose.pose,
    );
    let inner = match limb_pose.family {
        BodyFamily::Blob => palette.coat,
        BodyFamily::Hopper => palette.shadow,
        BodyFamily::SoftQuadruped => palette.coat,
    };
    draw_limb(
        canvas,
        genome,
        palette,
        limb_pose.left_root,
        left_target,
        inner,
    );
    draw_limb(
        canvas,
        genome,
        palette,
        limb_pose.right_root,
        right_target,
        inner,
    );
}

pub(super) fn draw_quadruped_forelimbs(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    limb_pose: LimbPose,
    ground: i32,
) {
    // Standing on four legs is what separates the cat silhouette from an upright body. Arm-style
    // forelimbs are reserved for the actions where the creature is visibly using its paws.
    // A crouch keeps all four paws down; every other gesture is made with the paws.
    if !matches!(
        limb_pose.clip,
        BodyClip::Action(
            ActionKind::SoloPlay
                | ActionKind::SocialPlay
                | ActionKind::Greet
                | ActionKind::PresentDiscovery
                | ActionKind::PetReaction
                | ActionKind::Dangle
                | ActionKind::ClimbWindow
                | ActionKind::InvestigateCursor
                | ActionKind::Dragged
        ) | BodyClip::Gesture(
            Gesture::Cheer
                | Gesture::Gasp
                | Gesture::Cover
                | Gesture::Worry
                | Gesture::Heave
                | Gesture::Balance
                | Gesture::Reach
                | Gesture::Bop
                | Gesture::Watch
        )
    ) {
        draw_quad_leg(
            canvas,
            palette,
            limb_pose.left_root.x,
            limb_pose.left_root.y,
            ground,
            limb_pose.pose.step_a,
            true,
        );
        draw_quad_leg(
            canvas,
            palette,
            limb_pose.right_root.x,
            limb_pose.right_root.y,
            ground,
            limb_pose.pose.step_b,
            true,
        );
    } else {
        draw_forelimbs(canvas, genome, palette, limb_pose);
    }
}

pub(super) fn limb_targets(
    left: PixelPoint,
    right: PixelPoint,
    clip: BodyClip,
    frame: u8,
    length: i32,
    rest: RestPose,
    pose: Pose,
) -> (PixelPoint, PixelPoint) {
    let action = match clip {
        BodyClip::Action(action) => action,
        BodyClip::Gesture(gesture) => {
            return gesture_limb_targets(left, right, gesture, frame, length, rest);
        }
    };
    let pulse = [0, 1, 0, -1, 0, 1][frame as usize % 6];
    let side_rest = || {
        let targets = match rest {
            RestPose::AtSides => ((-2, length - 1), (2, length - 1)),
            RestPose::Folded => ((2, 2), (-2, 2)),
            RestPose::Together => ((4, 3), (-4, 3)),
        };
        offset_pair(left, right, targets)
    };
    match action {
        ActionKind::Idle => side_rest(),
        ActionKind::Traverse | ActionKind::SqueezeWindow | ActionKind::Sprint => offset_pair(
            left,
            right,
            (
                (-2 + pose.step_a, length - 2),
                (2 + pose.step_b, length - 2),
            ),
        ),
        ActionKind::Perch => offset_pair(left, right, ((3, 3), (-3, 3))),
        ActionKind::Homebound => offset_pair(left, right, ((4, 3), (-4, 3))),
        ActionKind::Sleep => offset_pair(left, right, ((3, 1), (-3, 1))),
        ActionKind::InvestigateCursor => {
            offset_pair(left, right, ((2, 2), (length + 2, -2 + pulse)))
        }
        ActionKind::AvoidCursor => offset_pair(left, right, ((-length, 2), (2, -length + 2))),
        ActionKind::ReactToWindow => {
            offset_pair(left, right, ((-length, -length), (length, -length)))
        }
        ActionKind::RideWindow => {
            offset_pair(left, right, ((-length - 2, pulse), (length + 2, -pulse)))
        }
        ActionKind::SoloPlay => offset_pair(
            left,
            right,
            match frame % 4 {
                0 => ((2, length), (-2, length)),
                1 => ((2, 1), (length + 1, -2)),
                2 => ((length - 1, -length), (-length + 1, -length)),
                _ => ((-length - 1, -2), (-2, 1)),
            },
        ),
        ActionKind::Eat => offset_pair(
            left,
            right,
            ((length + 1, -1 + pulse), (-length - 1, -1 - pulse)),
        ),
        ActionKind::Drink => offset_pair(left, right, ((length - 1, 1), (-1, 1 - pulse))),
        ActionKind::Greet | ActionKind::PetReaction => {
            offset_pair(left, right, ((2, 2), (length + pulse, -length - pulse)))
        }
        ActionKind::Follow => offset_pair(left, right, ((-2, length - 2), (length + 2, -1))),
        ActionKind::SocialPlay => offset_pair(left, right, ((2, 1), (length + 2, -length + pulse))),
        ActionKind::Dragged => offset_pair(
            left,
            right,
            ((-1 + pulse, length + 2), (1 - pulse, length + 2)),
        ),
        ActionKind::Landing => {
            offset_pair(left, right, ((-length, length - 1), (length, length - 1)))
        }
        ActionKind::ClimbWindow => offset_pair(
            left,
            right,
            (
                (-length + pulse, -length - 3),
                (length - pulse, -length + 1),
            ),
        ),
        ActionKind::Dangle => offset_pair(
            left,
            right,
            ((-length + 1, -length - 4), (length - 1, -length - 4)),
        ),
        ActionKind::InspectScreen => {
            offset_pair(left, right, ((1, -length + 2), (length - 1, 1 + pulse)))
        }
        ActionKind::PresentDiscovery => {
            offset_pair(left, right, ((-2, -length - 4), (2, -length - 4)))
        }
        ActionKind::Tossed => offset_pair(
            left,
            right,
            ((-1 + pulse, length + 2), (1 - pulse, length + 2)),
        ),
    }
}

/// The same gestures on the original families. Each paw moves from where it rests, so a gesture
/// is the creature's own pair of paws carried somewhere, never another pair.
pub(super) fn gesture_limb_targets(
    left: PixelPoint,
    right: PixelPoint,
    gesture: Gesture,
    frame: u8,
    length: i32,
    rest: RestPose,
) -> (PixelPoint, PixelPoint) {
    let tick = i32::from(frame % 2);
    let beat = [0, 1, 0, -1][usize::from(frame % 4)];
    // The face sits over the middle of the body, a little forward.
    let middle = (left.x + right.x) / 2 + 1;
    let at = |x: i32, y: i32| PixelPoint { x, y };
    let resting = || {
        let targets = match rest {
            RestPose::AtSides => ((-2, length - 1), (2, length - 1)),
            RestPose::Folded => ((2, 2), (-2, 2)),
            RestPose::Together => ((4, 3), (-4, 3)),
        };
        offset_pair(left, right, targets)
    };
    match gesture {
        Gesture::Cheer => offset_pair(
            left,
            right,
            ((-2, -length - 6 - tick), (2, -length - 6 - tick)),
        ),
        Gesture::Gasp => offset_pair(
            left,
            right,
            ((-length - 2, -length + tick), (length + 2, -length - tick)),
        ),
        Gesture::Cover => (
            at(middle - 3, left.y - 2),
            at(middle + 3, right.y - 2 + tick * 2),
        ),
        Gesture::Worry => (
            at(middle - 1, left.y + 2 + beat),
            at(middle + 1, right.y + 2 - beat),
        ),
        Gesture::Crouch => offset_pair(left, right, ((-2, length + 1), (2, length + 1))),
        Gesture::Heave => {
            let pull = [0, 1, 2, 1][usize::from(frame % 4)];
            (
                at(right.x + 2 - pull, right.y + 1),
                at(right.x + length + 3 - pull, right.y),
            )
        }
        Gesture::Balance => offset_pair(
            left,
            right,
            ((-length - 3, beat * 2), (length + 3, -beat * 2)),
        ),
        Gesture::Reach => {
            let (resting_left, _) = resting();
            (
                resting_left,
                at(right.x + length + 4, right.y - length - 2 - tick),
            )
        }
        Gesture::Bop => {
            let (resting_left, resting_right) = resting();
            match frame % 4 {
                0 => (resting_left, at(right.x + length, right.y - length - 2)),
                2 => (at(left.x - length, left.y - length - 2), resting_right),
                _ => offset_pair(left, right, ((-length, 0), (length, 0))),
            }
        }
        // The near paw comes up in front of the mouth for the middle of the yawn and goes down
        // again; the far one stays where it rests.
        Gesture::Yawn => {
            let (resting_left, resting_right) = resting();
            if (1..=2).contains(&frame) {
                (resting_left, at(middle + 3, right.y - length - 1))
            } else {
                (resting_left, resting_right)
            }
        }
        // Both paws go up and in over the head, higher each frame until they are at full stretch.
        Gesture::Stretch => {
            let up = [3, 5, 7, 7][usize::from(frame.min(3))];
            offset_pair(left, right, ((1, -length - up), (-1, -length - up)))
        }
        // The far paw stays where it rests and the near one is gathered up in front of the chest,
        // curled rather than stretched: half the span a reach uses, and nowhere near the face.
        // Both limbs gathered in against the chest and held there: a body drawn together around
        // what it is looking at, rather than a part of it put out toward the thing.
        Gesture::Watch => {
            let curl = [0, 1, 1, 0, 0, 0][usize::from(frame % 6)];
            offset_pair(left, right, ((2, 3 - curl), (-1, 2 - curl)))
        }
    }
}

pub(super) fn offset_pair(
    left: PixelPoint,
    right: PixelPoint,
    offsets: ((i32, i32), (i32, i32)),
) -> (PixelPoint, PixelPoint) {
    (
        PixelPoint {
            x: left.x + offsets.0.0,
            y: left.y + offsets.0.1,
        },
        PixelPoint {
            x: right.x + offsets.1.0,
            y: right.y + offsets.1.1,
        },
    )
}

pub(super) fn draw_limb(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    root: PixelPoint,
    target: PixelPoint,
    inner: Rgba,
) {
    let thickness = i32::from(genome.forelimbs.thickness).clamp(1, 2);
    let bend = match genome.forelimbs.style {
        ForelimbStyle::Pseudopod => PixelPoint {
            x: (root.x + target.x) / 2,
            y: (root.y + target.y) / 2 + 1,
        },
        _ => root,
    };
    if bend != root {
        canvas.line(
            root.x,
            root.y,
            bend.x,
            bend.y,
            thickness + 1,
            palette.outline,
        );
        canvas.line(
            bend.x,
            bend.y,
            target.x,
            target.y,
            thickness + 1,
            palette.outline,
        );
        canvas.line(root.x, root.y, bend.x, bend.y, thickness, inner);
        canvas.line(bend.x, bend.y, target.x, target.y, thickness, inner);
    } else {
        canvas.line(
            root.x,
            root.y,
            target.x,
            target.y,
            thickness + 1,
            palette.outline,
        );
        canvas.line(root.x, root.y, target.x, target.y, thickness, inner);
    }
    match genome.forelimbs.tip_style {
        LimbTipStyle::Round => {
            canvas.fill_circle(target.x, target.y, thickness + 1, palette.outline);
            canvas.fill_circle(target.x, target.y, thickness, palette.accent);
        }
        LimbTipStyle::Mitten => {
            canvas.fill_ellipse(
                target.x,
                target.y,
                thickness + 2,
                thickness + 1,
                palette.outline,
            );
            canvas.fill_ellipse(target.x, target.y, thickness + 1, thickness, palette.accent);
        }
        LimbTipStyle::Paw => {
            canvas.fill_ellipse(
                target.x + 1,
                target.y,
                thickness + 2,
                thickness + 1,
                palette.outline,
            );
            canvas.fill_ellipse(
                target.x + 1,
                target.y,
                thickness + 1,
                thickness,
                palette.accent,
            );
        }
    }
}

pub(super) fn draw_tail(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    root_x: i32,
    root_y: i32,
    s: f32,
    pose: Pose,
) {
    let length = ((genome.tail_length as f32 * s).round() as i32)
        .clamp(2, 10)
        .min((root_x - 4).max(2));
    match genome.family {
        // Both families always carry a tail; the style gene shapes it instead of removing it.
        BodyFamily::SoftQuadruped => {
            draw_cat_tail(canvas, genome, palette, root_x, root_y, s, pose);
            return;
        }
        BodyFamily::Hopper => {
            draw_cotton_tail(canvas, genome, palette, root_x, root_y, s);
            return;
        }
        BodyFamily::Blob => {}
    }
    match genome.tail_style {
        TailStyle::None => {}
        TailStyle::Stub => canvas.fill_circle(root_x - 1, root_y, 2, palette.outline),
        TailStyle::Taper => {
            canvas.line(
                root_x,
                root_y,
                root_x - length,
                root_y - 3 - pose.bob + pose.tail_sway,
                2,
                palette.outline,
            );
            canvas.line(
                root_x,
                root_y,
                root_x - length,
                root_y - 3 - pose.bob + pose.tail_sway,
                1,
                palette.coat,
            );
        }
        TailStyle::Tuft => {
            canvas.line(
                root_x,
                root_y,
                root_x - length + 2,
                root_y - 2 + pose.tail_sway,
                2,
                palette.outline,
            );
            canvas.fill_circle(
                root_x - length,
                root_y - 3 + pose.tail_sway,
                3,
                palette.outline,
            );
            canvas.fill_circle(
                root_x - length,
                root_y - 3 + pose.tail_sway,
                2,
                palette.accent,
            );
        }
        TailStyle::Curl => {
            canvas.line(
                root_x,
                root_y,
                root_x - length,
                root_y - 3 + pose.tail_sway,
                2,
                palette.outline,
            );
            canvas.fill_circle(
                root_x - length,
                root_y - 5 + pose.tail_sway,
                3,
                palette.outline,
            );
            canvas.fill_circle(
                root_x - length,
                root_y - 5 + pose.tail_sway,
                1,
                Rgba::TRANSPARENT,
            );
        }
    }
}

pub(super) fn draw_cat_tail(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    root_x: i32,
    root_y: i32,
    s: f32,
    pose: Pose,
) {
    let length = ((genome.tail_length as f32 * s).round() as i32).clamp(2, 10);
    let sway = pose.tail_sway.clamp(-2, 2);
    // Carrying the tail up off the rump is the strongest feline cue at this size, so every
    // style arcs upward and only the reach and tip differ. The heights clear the tallest back.
    let (height, hook) = match genome.tail_style {
        TailStyle::None | TailStyle::Stub => ((4 + length / 3).clamp(5, 7), 1),
        TailStyle::Taper => ((7 + length / 2).clamp(8, 12), 2),
        TailStyle::Tuft => ((7 + length / 2).clamp(8, 11), 2),
        TailStyle::Curl => ((7 + length / 2).clamp(8, 11), 3),
    };
    let mid_x = root_x - 2;
    let mid_y = root_y - height / 2;
    let tip_x = mid_x + hook;
    let tip_y = root_y - height + sway;
    canvas.line(root_x, root_y, mid_x, mid_y, 2, palette.outline);
    canvas.line(mid_x, mid_y, tip_x, tip_y, 2, palette.outline);
    canvas.line(root_x, root_y, mid_x, mid_y, 1, palette.coat);
    canvas.line(mid_x, mid_y, tip_x, tip_y, 1, palette.coat);
    match genome.tail_style {
        TailStyle::Tuft => {
            canvas.fill_circle(tip_x, tip_y - 1, 2, palette.outline);
            canvas.fill_circle(tip_x, tip_y - 1, 1, palette.accent);
        }
        TailStyle::Curl => {
            canvas.line(tip_x, tip_y, tip_x + 3, tip_y + 2, 2, palette.outline);
            canvas.line(tip_x, tip_y, tip_x + 3, tip_y + 2, 1, palette.accent);
        }
        _ => {}
    }
}

pub(super) fn draw_cotton_tail(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    root_x: i32,
    root_y: i32,
    s: f32,
) {
    let length = ((genome.tail_length as f32 * s).round() as i32).clamp(2, 10);
    let radius = match genome.tail_style {
        TailStyle::Tuft | TailStyle::Curl => (2 + length / 3).clamp(3, 4),
        _ => (2 + length / 4).clamp(3, 4),
    };
    // The body ellipse paints over the inner half afterwards, leaving a puff behind the rump.
    let x = (root_x - radius + 1).max(radius + 1);
    canvas.fill_circle(x, root_y, radius, palette.outline);
    canvas.fill_circle(x, root_y - 1, radius - 1, palette.highlight);
}

pub(super) fn draw_feet(
    canvas: &mut Canvas,
    palette: Palette,
    cx: i32,
    ground: i32,
    rx: i32,
    foot_size: u8,
    pose: Pose,
) {
    let foot = foot_size as i32;
    for (x, step) in [
        (cx - rx / 2 + pose.step_a, 0),
        (cx + rx / 2 + pose.step_b, 1),
    ] {
        canvas.fill_ellipse(x, ground + step, foot, 2, palette.outline);
        canvas.fill_ellipse(x + 1, ground + step, (foot - 1).max(1), 1, palette.accent);
    }
}

pub(super) fn draw_hopper_leg(
    canvas: &mut Canvas,
    palette: Palette,
    x: i32,
    root_y: i32,
    ground: i32,
    step: i32,
) {
    canvas.line(x, root_y, x + step, ground - 2, 2, palette.outline);
    canvas.line(x, root_y, x + step, ground - 2, 1, palette.shadow);
    // A long hind foot planted forward of the ankle.
    canvas.fill_ellipse(x + step + 2, ground, 5, 2, palette.outline);
    canvas.fill_ellipse(x + step + 3, ground, 4, 1, palette.accent);
}

pub(super) fn draw_quad_leg(
    canvas: &mut Canvas,
    palette: Palette,
    x: i32,
    root_y: i32,
    ground: i32,
    step: i32,
    near: bool,
) {
    let coat = if near { palette.coat } else { palette.shadow };
    canvas.line(x, root_y, x + step, ground - 1, 2, palette.outline);
    canvas.line(x, root_y, x + step, ground - 1, 1, coat);
    canvas.fill_ellipse(x + step + 1, ground, 3, 2, palette.outline);
    canvas.fill_ellipse(x + step + 1, ground, 2, 1, palette.accent);
}

pub(super) fn apply_pattern(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    cx: i32,
    cy: i32,
    rx: i32,
    ry: i32,
) {
    if genome.pattern == PatternKind::Solid || rx <= 2 || ry <= 2 {
        return;
    }
    let mut seed = [0_u8; 32];
    seed[..8].copy_from_slice(&genome.marking_seed.to_le_bytes());
    let mut rng = ChaCha12Rng::from_seed(seed);
    match genome.pattern {
        PatternKind::Spots | PatternKind::Patches => {
            let count = (genome.pattern_density * 8.0).round() as usize + 1;
            for _ in 0..count {
                let x = rng.random_range(cx - rx + 2..=cx + rx - 2);
                let y = rng.random_range(cy - ry + 2..=cy + ry - 2);
                if inside_ellipse(x, y, cx, cy, rx, ry) {
                    let radius = if genome.pattern == PatternKind::Patches {
                        3
                    } else {
                        1
                    };
                    canvas.fill_circle(x, y, radius, palette.accent);
                }
            }
        }
        PatternKind::Stripes => {
            for offset in (-rx + 3..rx - 2).step_by(4) {
                for y in cy - ry..=cy + ry {
                    let x = cx + offset + (y - cy).div_euclid(4);
                    if inside_ellipse(x, y, cx, cy, rx, ry) {
                        canvas.set(x, y, palette.accent);
                    }
                }
            }
        }
        PatternKind::Mask => {
            canvas.fill_ellipse(
                cx + 2,
                cy - ry / 3,
                (rx / 2).max(2),
                (ry / 3).max(2),
                palette.accent,
            );
        }
        PatternKind::Socks => {
            for x in cx - rx..=cx + rx {
                for y in cy + ry / 2..=cy + ry {
                    if inside_ellipse(x, y, cx, cy, rx, ry) {
                        canvas.set(x, y, palette.accent);
                    }
                }
            }
        }
        PatternKind::Tips => {
            for x in cx - rx..=cx + rx {
                for y in cy - ry..=cy - ry / 2 {
                    if inside_ellipse(x, y, cx, cy, rx, ry) {
                        canvas.set(x, y, palette.highlight);
                    }
                }
            }
        }
        PatternKind::Solid => {}
    }
}

pub(super) fn inside_ellipse(x: i32, y: i32, cx: i32, cy: i32, rx: i32, ry: i32) -> bool {
    let dx = x - cx;
    let dy = y - cy;
    let rx2 = (rx * rx) as i64;
    let ry2 = (ry * ry) as i64;
    dx as i64 * dx as i64 * ry2 + dy as i64 * dy as i64 * rx2 <= rx2 * ry2
}
