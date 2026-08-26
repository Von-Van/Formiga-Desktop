use crate::{Canvas, PALETTES, Palette, Rgba};
use formiga_core::{
    ActionKind, AppearanceGenome, AppendageStyle, BodyFamily, PatternKind, TailStyle,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;

pub const FRAME_SIZE: u32 = 48;

#[derive(Clone, Copy, Debug)]
pub struct AnimationSpec {
    pub frames: u8,
    pub fps: u8,
}

impl AnimationSpec {
    pub fn for_action(action: ActionKind) -> Self {
        match action {
            ActionKind::Traverse | ActionKind::Follow => Self { frames: 6, fps: 10 },
            ActionKind::Sleep => Self { frames: 2, fps: 2 },
            ActionKind::Idle | ActionKind::Perch | ActionKind::RideWindow => {
                Self { frames: 4, fps: 4 }
            }
            _ => Self { frames: 4, fps: 8 },
        }
    }
}

#[derive(Clone)]
pub struct AnimationAtlas {
    pub action: ActionKind,
    pub frames: Vec<Canvas>,
}

pub struct CreatureRenderer;

impl CreatureRenderer {
    pub fn render_atlas(
        genome: &AppearanceGenome,
        action: ActionKind,
        facing_right: bool,
    ) -> AnimationAtlas {
        let spec = AnimationSpec::for_action(action);
        let frames = (0..spec.frames)
            .map(|frame| Self::render_frame(genome, action, frame, facing_right))
            .collect();
        AnimationAtlas { action, frames }
    }

    pub fn render_frame(
        genome: &AppearanceGenome,
        action: ActionKind,
        frame: u8,
        facing_right: bool,
    ) -> Canvas {
        Self::render_frame_with_options(genome, action, frame, facing_right, false, 0)
    }

    pub fn render_frame_with_options(
        genome: &AppearanceGenome,
        action: ActionKind,
        frame: u8,
        facing_right: bool,
        reduce_motion: bool,
        gaze_x: i8,
    ) -> Canvas {
        let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        let palette = PALETTES[genome.palette_index as usize % PALETTES.len()];
        let pose = Pose::new(genome, action, frame, reduce_motion);
        match genome.family {
            BodyFamily::Blob => draw_blob(&mut canvas, genome, palette, pose, gaze_x),
            BodyFamily::Hopper => draw_hopper(&mut canvas, genome, palette, pose, gaze_x),
            BodyFamily::SoftQuadruped => draw_quadruped(&mut canvas, genome, palette, pose, gaze_x),
        }
        keep_atlas_margin(&mut canvas);
        if !facing_right {
            canvas.mirror_horizontal();
        }
        canvas
    }
}

fn keep_atlas_margin(canvas: &mut Canvas) {
    let Some((min_x, min_y, max_x, max_y)) = canvas.alpha_bounds() else {
        return;
    };
    let margin = 1_i32;
    let max_allowed = FRAME_SIZE as i32 - margin - 1;
    let dx = if min_x < margin as u32 {
        margin - min_x as i32
    } else if max_x > max_allowed as u32 {
        max_allowed - max_x as i32
    } else {
        0
    };
    let dy = if min_y < margin as u32 {
        margin - min_y as i32
    } else if max_y > max_allowed as u32 {
        max_allowed - max_y as i32
    } else {
        0
    };
    canvas.translate(dx, dy);
}

#[derive(Clone, Copy)]
struct Pose {
    bob: i32,
    squash_x: i32,
    squash_y: i32,
    step_a: i32,
    step_b: i32,
    eyes_closed: bool,
    play_lift: i32,
}

impl Pose {
    fn new(genome: &AppearanceGenome, action: ActionKind, frame: u8, reduce_motion: bool) -> Self {
        let phase = frame as usize % 6;
        let walk: i32 = [0, 1, 0, -1, 0, 1][phase];
        let alternate: i32 = [1, 0, -1, 0, 1, 0][phase];
        let bob_amount = genome.gait_bob.max(0.2).round() as i32;
        let mut pose = match action {
            ActionKind::Traverse | ActionKind::Follow => Self {
                bob: walk.abs() * bob_amount,
                squash_x: 0,
                squash_y: 0,
                step_a: walk * 2,
                step_b: alternate * 2,
                eyes_closed: false,
                play_lift: 0,
            },
            ActionKind::Sleep => Self {
                bob: frame as i32 % 2,
                squash_x: 3,
                squash_y: -3,
                step_a: 0,
                step_b: 0,
                eyes_closed: true,
                play_lift: 0,
            },
            ActionKind::Perch => Self {
                bob: 2,
                squash_x: 1,
                squash_y: -1,
                step_a: 0,
                step_b: 0,
                eyes_closed: frame % 4 == 3,
                play_lift: 0,
            },
            ActionKind::SoloPlay | ActionKind::SocialPlay => Self {
                bob: -walk.abs(),
                squash_x: -walk,
                squash_y: walk,
                step_a: walk * 2,
                step_b: alternate * 2,
                eyes_closed: false,
                play_lift: walk.abs(),
            },
            ActionKind::AvoidCursor | ActionKind::ReactToWindow => Self {
                bob: -walk.abs(),
                squash_x: 1,
                squash_y: -1,
                step_a: walk * 3,
                step_b: alternate * 3,
                eyes_closed: false,
                play_lift: 0,
            },
            _ => Self {
                bob: frame as i32 % 2,
                squash_x: 0,
                squash_y: 0,
                step_a: 0,
                step_b: 0,
                eyes_closed: frame % 4 == 3,
                play_lift: 0,
            },
        };
        if reduce_motion {
            pose.bob = 0;
            pose.squash_x = 0;
            pose.squash_y = 0;
            pose.play_lift = 0;
            pose.step_a /= 2;
            pose.step_b /= 2;
        }
        pose
    }
}

fn scale(genome: &AppearanceGenome) -> f32 {
    genome.logical_size as f32 / 38.0
}

fn draw_blob(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    pose: Pose,
    gaze_x: i8,
) {
    let s = scale(genome);
    let rx = ((genome.body_width as f32 * s / 2.0).round() as i32 + pose.squash_x).clamp(6, 16);
    let ry = ((genome.body_height as f32 * s / 2.0).round() as i32 + pose.squash_y).clamp(5, 14);
    let cx = 26;
    let cy = 38 - ry + pose.bob - pose.play_lift;
    draw_tail(canvas, genome, palette, cx - rx + 1, cy, s, pose);
    draw_appendages(canvas, genome, palette, cx, cy - ry + 2, s, pose);
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
    draw_face(
        canvas,
        genome,
        palette,
        cx + 2,
        cy - 1,
        pose.eyes_closed,
        gaze_x,
    );
}

fn draw_hopper(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    pose: Pose,
    gaze_x: i8,
) {
    let s = scale(genome);
    let rx = ((genome.body_width as f32 * s * 0.38).round() as i32 + pose.squash_x).clamp(5, 11);
    let ry = ((genome.body_height as f32 * s * 0.55).round() as i32 + pose.squash_y).clamp(8, 15);
    let leg = ((genome.leg_length as f32 * s).round() as i32).clamp(3, 9);
    let cx = 24;
    let ground = 43;
    let cy = ground - leg - ry + pose.bob - pose.play_lift;
    draw_tail(canvas, genome, palette, cx - rx + 1, cy + 2, s, pose);
    draw_appendages(canvas, genome, palette, cx, cy - ry + 2, s, pose);
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
    draw_face(
        canvas,
        genome,
        palette,
        cx + 1,
        cy - 2,
        pose.eyes_closed,
        gaze_x,
    );
}

fn draw_quadruped(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    pose: Pose,
    gaze_x: i8,
) {
    let s = scale(genome);
    let body_rx =
        ((genome.body_width as f32 * s * 0.42).round() as i32 + pose.squash_x).clamp(7, 13);
    let body_ry =
        ((genome.body_height as f32 * s * 0.34).round() as i32 + pose.squash_y).clamp(4, 8);
    let leg = ((genome.leg_length as f32 * s).round() as i32).clamp(3, 8);
    let ground = 43;
    let body_y = ground - leg - body_ry + pose.bob - pose.play_lift;
    let body_x = 22;
    let head_radius = ((body_ry as f32 * genome.head_ratio).round() as i32 + 2).clamp(5, 9);
    draw_tail(
        canvas,
        genome,
        palette,
        body_x - body_rx + 1,
        body_y,
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
    let head_x = body_x + body_rx - 1;
    let head_y = body_y - 2;
    draw_appendages(
        canvas,
        genome,
        palette,
        head_x,
        head_y - head_radius + 2,
        s,
        pose,
    );
    canvas.fill_circle(head_x, head_y, head_radius + 1, palette.outline);
    canvas.fill_circle(head_x, head_y - 1, head_radius, palette.coat);
    canvas.fill_ellipse(
        head_x + head_radius - 2,
        head_y + 2,
        3,
        2,
        palette.highlight,
    );
    draw_quad_leg(
        canvas,
        palette,
        body_x - body_rx / 2 + 2,
        body_y + body_ry - 1,
        ground,
        pose.step_a,
        true,
    );
    draw_quad_leg(
        canvas,
        palette,
        body_x + body_rx / 2 + 1,
        body_y + body_ry - 1,
        ground,
        pose.step_b,
        true,
    );
    draw_face(
        canvas,
        genome,
        palette,
        head_x + 1,
        head_y - 1,
        pose.eyes_closed,
        gaze_x,
    );
}

fn draw_face(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    center_x: i32,
    center_y: i32,
    closed: bool,
    gaze_x: i8,
) {
    let spacing = (genome.eye_spacing as i32 / 2).max(2);
    let y = center_y + genome.eye_height as i32;
    if closed {
        canvas.line(
            center_x - spacing - 1,
            y,
            center_x - spacing + 1,
            y,
            1,
            palette.eye,
        );
        canvas.line(
            center_x + spacing - 1,
            y,
            center_x + spacing + 1,
            y,
            1,
            palette.eye,
        );
    } else {
        let eye_radius = genome.eye_size as i32;
        for x in [center_x - spacing, center_x + spacing] {
            canvas.fill_circle(x, y, eye_radius + 1, palette.outline);
            canvas.fill_circle(x, y, eye_radius, palette.eye);
            let gaze_x = i32::from(gaze_x.clamp(-1, 1));
            let highlight_y = if gaze_x == 0 { y - eye_radius } else { y };
            canvas.set(x + gaze_x, highlight_y, Rgba::new(255, 255, 245, 255));
        }
    }
    if genome.face_signature & 1 == 0 {
        canvas.set(center_x, y + 3, palette.outline);
        canvas.set(center_x + 1, y + 3, palette.outline);
    } else {
        canvas.set(center_x, y + 3, palette.accent);
    }
}

fn draw_appendages(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    cx: i32,
    root_y: i32,
    s: f32,
    pose: Pose,
) {
    let size = ((genome.appendage_size as f32 * s).round() as i32).clamp(2, 8);
    match genome.appendage_style {
        AppendageStyle::None => {}
        AppendageStyle::Round => {
            canvas.fill_circle(cx - 5, root_y, size, palette.outline);
            canvas.fill_circle(cx + 5, root_y, size, palette.outline);
            canvas.fill_circle(cx - 5, root_y, size - 1, palette.accent);
            canvas.fill_circle(cx + 5, root_y, size - 1, palette.accent);
        }
        AppendageStyle::Pointed | AppendageStyle::Leaf => {
            let spread = if genome.appendage_style == AppendageStyle::Leaf {
                7
            } else {
                5
            };
            canvas.line(
                cx - 4,
                root_y + 2,
                cx - spread,
                root_y - size,
                2,
                palette.outline,
            );
            canvas.line(
                cx + 4,
                root_y + 2,
                cx + spread,
                root_y - size,
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
        AppendageStyle::Droop => {
            canvas.line(cx - 4, root_y, cx - 8, root_y + size, 2, palette.outline);
            canvas.line(cx + 4, root_y, cx + 8, root_y + size, 2, palette.outline);
            canvas.line(cx - 4, root_y, cx - 8, root_y + size - 1, 1, palette.accent);
            canvas.line(cx + 4, root_y, cx + 8, root_y + size - 1, 1, palette.accent);
        }
        AppendageStyle::Antenna => {
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

fn draw_tail(
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
    match genome.tail_style {
        TailStyle::None => {}
        TailStyle::Stub => canvas.fill_circle(root_x - 1, root_y, 2, palette.outline),
        TailStyle::Taper => {
            canvas.line(
                root_x,
                root_y,
                root_x - length,
                root_y - 3 - pose.bob,
                2,
                palette.outline,
            );
            canvas.line(
                root_x,
                root_y,
                root_x - length,
                root_y - 3 - pose.bob,
                1,
                palette.coat,
            );
        }
        TailStyle::Tuft => {
            canvas.line(
                root_x,
                root_y,
                root_x - length + 2,
                root_y - 2,
                2,
                palette.outline,
            );
            canvas.fill_circle(root_x - length, root_y - 3, 3, palette.outline);
            canvas.fill_circle(root_x - length, root_y - 3, 2, palette.accent);
        }
        TailStyle::Curl => {
            canvas.line(
                root_x,
                root_y,
                root_x - length,
                root_y - 3,
                2,
                palette.outline,
            );
            canvas.fill_circle(root_x - length, root_y - 5, 3, palette.outline);
            canvas.fill_circle(root_x - length, root_y - 5, 1, Rgba::TRANSPARENT);
        }
    }
}

fn draw_feet(
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

fn draw_hopper_leg(
    canvas: &mut Canvas,
    palette: Palette,
    x: i32,
    root_y: i32,
    ground: i32,
    step: i32,
) {
    canvas.line(x, root_y, x + step, ground - 2, 2, palette.outline);
    canvas.line(x, root_y, x + step, ground - 2, 1, palette.shadow);
    canvas.fill_ellipse(x + step + 1, ground, 4, 2, palette.outline);
    canvas.fill_ellipse(x + step + 2, ground, 3, 1, palette.accent);
}

fn draw_quad_leg(
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

fn apply_pattern(
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

fn inside_ellipse(x: i32, y: i32, cx: i32, cy: i32, rx: i32, ry: i32) -> bool {
    let dx = x - cx;
    let dy = y - cy;
    let rx2 = (rx * rx) as i64;
    let ry2 = (ry * ry) as i64;
    dx as i64 * dx as i64 * ry2 + dy as i64 * dy as i64 * rx2 <= rx2 * ry2
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::{DesktopRect, DesktopSnapshot, MonitorInfo, World};
    use sha2::{Digest, Sha256};

    fn genome(family: BodyFamily) -> AppearanceGenome {
        AppearanceGenome {
            family,
            logical_size: 38,
            body_width: 22,
            body_height: 18,
            head_ratio: 0.8,
            roundness: 0.8,
            leg_length: 6,
            foot_size: 3,
            appendage_style: AppendageStyle::Pointed,
            appendage_size: 5,
            tail_style: TailStyle::Curl,
            tail_length: 8,
            eye_size: 1,
            eye_spacing: 5,
            eye_height: 0,
            palette_index: 2,
            pattern: PatternKind::Spots,
            pattern_density: 0.5,
            marking_seed: 42,
            gait_bob: 0.6,
            face_signature: 7,
        }
    }

    #[test]
    fn every_family_renders_inside_frame() {
        for family in [
            BodyFamily::Blob,
            BodyFamily::Hopper,
            BodyFamily::SoftQuadruped,
        ] {
            for action in ActionKind::ALL {
                let atlas = CreatureRenderer::render_atlas(&genome(family), action, true);
                assert!(!atlas.frames.is_empty());
                for frame in atlas.frames {
                    let bounds = frame.alpha_bounds().expect("creature is visible");
                    assert!(
                        bounds.0 > 0
                            && bounds.1 > 0
                            && bounds.2 < FRAME_SIZE - 1
                            && bounds.3 < FRAME_SIZE - 1,
                        "{family:?} {action:?}: {bounds:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn atlas_is_deterministic() {
        let first =
            CreatureRenderer::render_frame(&genome(BodyFamily::Blob), ActionKind::Idle, 0, true);
        let second =
            CreatureRenderer::render_frame(&genome(BodyFamily::Blob), ActionKind::Idle, 0, true);
        assert_eq!(
            Sha256::digest(first.rgba_bytes()),
            Sha256::digest(second.rgba_bytes())
        );
    }

    #[test]
    fn left_facing_is_exact_mirror() {
        let right = CreatureRenderer::render_frame(
            &genome(BodyFamily::Hopper),
            ActionKind::Traverse,
            2,
            true,
        );
        let mut expected = right.clone();
        expected.mirror_horizontal();
        let left = CreatureRenderer::render_frame(
            &genome(BodyFamily::Hopper),
            ActionKind::Traverse,
            2,
            false,
        );
        assert_eq!(expected, left);
    }

    #[test]
    fn one_thousand_generated_genomes_render_every_action() {
        let desktop = DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1440.0,
                    height: 900.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 1440.0,
                    height: 836.0,
                },
                scale_factor: 2.0,
                primary: true,
            }],
            ..DesktopSnapshot::default()
        };
        for index in 0_u64..1_000 {
            let mut seed = [0_u8; 32];
            seed.copy_from_slice(&Sha256::digest(index.to_le_bytes()));
            let world = World::new(seed, time::OffsetDateTime::UNIX_EPOCH, &desktop);
            let genome = &world.save.creatures[0].appearance;
            for action in ActionKind::ALL {
                let spec = AnimationSpec::for_action(action);
                let frame = (index as u8) % spec.frames;
                let rendered = CreatureRenderer::render_frame(genome, action, frame, true);
                let bounds = rendered
                    .alpha_bounds()
                    .expect("generated creature is visible");
                assert!(
                    bounds.0 > 0
                        && bounds.1 > 0
                        && bounds.2 < FRAME_SIZE - 1
                        && bounds.3 < FRAME_SIZE - 1,
                    "seed {index}, {action:?}: {bounds:?}"
                );
            }
        }
    }
}
