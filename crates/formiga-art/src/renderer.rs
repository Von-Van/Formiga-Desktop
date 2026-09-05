use crate::{Canvas, PALETTES, Palette, Rgba};
use formiga_core::{
    ActionKind, AppearanceGenome, BodyFamily, BrowStyle, CheekStyle, Creature, CursorSnapshot,
    EffectMotif, EyeShape, ForelimbStyle, HeadAppendageStyle, HighlightStyle, LimbTipStyle,
    MouthStyle, PatternKind, PupilStyle, RestPose, TailStyle,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;

pub const FRAME_SIZE: u32 = 48;
pub const FACE_FRAME_SIZE: u32 = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PixelPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlphaMask {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<bool>,
}

impl AlphaMask {
    pub fn from_canvas(canvas: &Canvas) -> Self {
        Self {
            width: canvas.width(),
            height: canvas.height(),
            pixels: canvas.pixels().iter().map(|pixel| pixel.a > 16).collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExpressionKind {
    Neutral,
    Content,
    Curious,
    Focused,
    Joy,
    Affectionate,
    Sleepy,
    Startled,
    Worried,
    Determined,
    Bored,
}

impl ExpressionKind {
    pub const ALL: [Self; 11] = [
        Self::Neutral,
        Self::Content,
        Self::Curious,
        Self::Focused,
        Self::Joy,
        Self::Affectionate,
        Self::Sleepy,
        Self::Startled,
        Self::Worried,
        Self::Determined,
        Self::Bored,
    ];

    pub const fn index(self) -> u32 {
        self as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EyelidPose {
    Open,
    Half,
    Closed,
}

impl EyelidPose {
    pub const ALL: [Self; 3] = [Self::Open, Self::Half, Self::Closed];

    pub const fn index(self) -> u32 {
        self as u32
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GazeDirection {
    pub x: i8,
    pub y: i8,
}

impl GazeDirection {
    pub fn new(x: i8, y: i8) -> Self {
        Self {
            x: x.clamp(-1, 1),
            y: y.clamp(-1, 1),
        }
    }

    pub fn index(self) -> u32 {
        ((self.y.clamp(-1, 1) + 1) as u32) * 3 + (self.x.clamp(-1, 1) + 1) as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FaceRenderState {
    pub expression: ExpressionKind,
    pub eyelids: EyelidPose,
    pub gaze: GazeDirection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedBodyFrame {
    pub canvas: Canvas,
    pub face_anchor: PixelPoint,
    pub alpha_mask: AlphaMask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackMode {
    Loop,
    Hold,
}

#[derive(Clone, Copy, Debug)]
pub struct AnimationSpec {
    pub frames: u8,
    pub fps: u8,
    pub playback: PlaybackMode,
}

impl AnimationSpec {
    pub fn for_action(action: ActionKind) -> Self {
        let (frames, fps, playback) = match action {
            ActionKind::Traverse | ActionKind::SqueezeWindow | ActionKind::Follow => {
                (6, 10, PlaybackMode::Loop)
            }
            ActionKind::Sprint => (6, 12, PlaybackMode::Loop),
            ActionKind::Eat | ActionKind::Drink => (4, 6, PlaybackMode::Loop),
            ActionKind::Sleep => (2, 2, PlaybackMode::Loop),
            ActionKind::Idle | ActionKind::Perch | ActionKind::RideWindow => {
                (4, 4, PlaybackMode::Loop)
            }
            ActionKind::Homebound => (2, 2, PlaybackMode::Loop),
            ActionKind::ClimbWindow => (4, 6, PlaybackMode::Loop),
            ActionKind::Dangle => (4, 3, PlaybackMode::Loop),
            ActionKind::InspectScreen => (4, 4, PlaybackMode::Loop),
            ActionKind::PresentDiscovery => (4, 2, PlaybackMode::Hold),
            _ => (4, 8, PlaybackMode::Loop),
        };
        Self {
            frames,
            fps,
            playback,
        }
    }

    pub const fn body_action(action: ActionKind) -> ActionKind {
        match action {
            ActionKind::Tossed => ActionKind::Dragged,
            ActionKind::PetReaction => ActionKind::Greet,
            ActionKind::SqueezeWindow => ActionKind::Traverse,
            other => other,
        }
    }

    pub fn frame_at(self, elapsed: f32) -> u8 {
        let raw = (elapsed.max(0.0) * f32::from(self.fps)) as u8;
        match self.playback {
            PlaybackMode::Loop => raw % self.frames,
            PlaybackMode::Hold => raw.min(self.frames - 1),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FramePlacement {
    /// Top of the 48x48 body frame relative to the simulation contact point, in art pixels.
    pub origin_y: i32,
}

impl FramePlacement {
    pub fn for_action(action: ActionKind, resting_baseline: u32) -> Self {
        Self {
            origin_y: if action == ActionKind::Dangle {
                // Seat the raised hands on the ledge instead of leaving their final pixels just
                // below it. GPU placement and the native interaction proxy share this offset.
                -7
            } else {
                resting_baseline as i32 - FRAME_SIZE as i32
            },
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
        let state = FaceRenderState {
            expression: expression_for_action(action),
            eyelids: default_eyelids(action, frame),
            gaze: GazeDirection::new(gaze_x, 0),
        };
        Self::render_composited_frame(genome, action, frame, facing_right, reduce_motion, state)
    }

    pub fn render_body_frame(
        genome: &AppearanceGenome,
        action: ActionKind,
        frame: u8,
        reduce_motion: bool,
    ) -> RenderedBodyFrame {
        let frame = if reduce_motion { 0 } else { frame };
        let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        let palette = PALETTES[genome.palette_index as usize % PALETTES.len()];
        let body_action = AnimationSpec::body_action(action);
        let pose = Pose::new(genome, body_action, frame, reduce_motion);
        let mut face_anchor = match genome.family {
            BodyFamily::Blob => draw_blob(&mut canvas, genome, palette, pose, body_action, frame),
            BodyFamily::Hopper => {
                draw_hopper(&mut canvas, genome, palette, pose, body_action, frame)
            }
            BodyFamily::SoftQuadruped => {
                draw_quadruped(&mut canvas, genome, palette, pose, body_action, frame)
            }
        };
        draw_activity_prop(
            &mut canvas,
            genome,
            palette,
            face_anchor,
            body_action,
            frame,
            reduce_motion,
        );
        draw_effects(
            &mut canvas,
            genome,
            palette,
            face_anchor,
            body_action,
            frame,
            reduce_motion,
        );
        let (dx, dy) = keep_atlas_margin(&mut canvas);
        face_anchor.x += dx;
        face_anchor.y += dy;
        let alpha_mask = AlphaMask::from_canvas(&canvas);
        RenderedBodyFrame {
            canvas,
            face_anchor,
            alpha_mask,
        }
    }

    pub fn render_face_frame(genome: &AppearanceGenome, state: FaceRenderState) -> Canvas {
        let mut canvas = Canvas::new(FACE_FRAME_SIZE, FACE_FRAME_SIZE);
        let palette = PALETTES[genome.palette_index as usize % PALETTES.len()];
        draw_face(&mut canvas, genome, palette, 8, 7, state);
        canvas
    }

    pub fn render_trinket(genome: &AppearanceGenome, variant: u8) -> Canvas {
        let mut canvas = Canvas::new(FACE_FRAME_SIZE, FACE_FRAME_SIZE);
        let palette = PALETTES[genome.palette_index as usize % PALETTES.len()];
        draw_generated_trinket(&mut canvas, palette, variant % 8, genome.marking_seed);
        canvas
    }

    pub fn render_composited_frame(
        genome: &AppearanceGenome,
        action: ActionKind,
        frame: u8,
        facing_right: bool,
        reduce_motion: bool,
        face_state: FaceRenderState,
    ) -> Canvas {
        let mut body = Self::render_body_frame(genome, action, frame, reduce_motion);
        if !facing_right {
            body.canvas.mirror_horizontal();
            // A 16-pixel face is centered between logical pixel columns; mirroring its full
            // rectangle uses `width - anchor`, rather than the single-pixel `width - 1 - x`.
            body.face_anchor.x = FRAME_SIZE as i32 - body.face_anchor.x;
        }
        let mut source_face_state = face_state;
        if !facing_right {
            source_face_state.gaze.x = -source_face_state.gaze.x;
        }
        let mut face = Self::render_face_frame(genome, source_face_state);
        if !facing_right {
            face.mirror_horizontal();
        }
        blit_transparent(
            &mut body.canvas,
            &face,
            body.face_anchor.x - FACE_FRAME_SIZE as i32 / 2,
            body.face_anchor.y - FACE_FRAME_SIZE as i32 / 2,
        );
        body.canvas
    }

    /// Empty rows beneath the creature's feet in its resting poses, in art pixels.
    ///
    /// Poses are authored inside the 48x48 frame with clearance under the body so hops and walk
    /// cycles have somewhere to travel. Drawing a frame with its bottom edge on the contact point
    /// therefore leaves the creature hovering above whatever it stands on: one or two pixels for
    /// hoppers and soft quadrupeds, five or six for blobs. Renderers shift the sprite down by this
    /// many pixels so the lowest resting pixel meets the surface.
    ///
    /// Only resting poses are measured, and the smallest clearance among them wins, so the value
    /// is a stable property of the creature. Seating never shifts mid-animation, and airborne
    /// frames keep reading as lift instead of sinking the creature into its perch.
    pub fn resting_baseline(genome: &AppearanceGenome, reduce_motion: bool) -> u32 {
        const RESTING: [ActionKind; 3] =
            [ActionKind::Idle, ActionKind::Perch, ActionKind::Homebound];
        RESTING
            .into_iter()
            .flat_map(|action| {
                (0..AnimationSpec::for_action(action).frames).map(move |frame| (action, frame))
            })
            .filter_map(|(action, frame)| {
                Self::render_body_frame(genome, action, frame, reduce_motion)
                    .canvas
                    .alpha_bounds()
                    .map(|(_, _, _, max_y)| FRAME_SIZE - 1 - max_y)
            })
            .min()
            .unwrap_or(0)
    }

    pub fn resolve_face_state(
        creature: &Creature,
        cursor: CursorSnapshot,
        cursor_reactions: bool,
    ) -> FaceRenderState {
        FaceRenderState {
            expression: resolve_expression(creature),
            eyelids: resolve_eyelids(creature),
            gaze: resolve_gaze(creature, cursor, cursor_reactions),
        }
    }
}

fn keep_atlas_margin(canvas: &mut Canvas) -> (i32, i32) {
    let Some((min_x, min_y, max_x, max_y)) = canvas.alpha_bounds() else {
        return (0, 0);
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
    (dx, dy)
}

#[derive(Clone, Copy)]
struct Pose {
    bob: i32,
    squash_x: i32,
    squash_y: i32,
    step_a: i32,
    step_b: i32,
    play_lift: i32,
    appendage_lift: i32,
    tail_sway: i32,
}

impl Pose {
    fn new(genome: &AppearanceGenome, action: ActionKind, frame: u8, reduce_motion: bool) -> Self {
        let phase = frame as usize % 6;
        let walk: i32 = [0, 1, 0, -1, 0, 1][phase];
        let alternate: i32 = [1, 0, -1, 0, 1, 0][phase];
        let bob_amount = genome.gait_bob.max(0.2).round() as i32;
        let mut pose = match action {
            ActionKind::Traverse | ActionKind::SqueezeWindow | ActionKind::Follow => Self {
                bob: walk.abs() * bob_amount,
                squash_x: 0,
                squash_y: 0,
                step_a: walk * 2,
                step_b: alternate * 2,
                play_lift: 0,
                appendage_lift: walk,
                tail_sway: alternate,
            },
            ActionKind::Sprint => Self {
                bob: -walk.abs() * bob_amount.max(1),
                squash_x: walk.abs(),
                squash_y: -walk.abs(),
                step_a: walk * 3,
                step_b: alternate * 3,
                play_lift: walk.abs(),
                appendage_lift: walk * 2,
                tail_sway: alternate * 2,
            },
            ActionKind::Sleep => Self {
                bob: frame as i32 % 2,
                squash_x: 3,
                squash_y: -3,
                step_a: 0,
                step_b: 0,
                play_lift: 0,
                appendage_lift: -1,
                tail_sway: 0,
            },
            ActionKind::Perch | ActionKind::Homebound => Self {
                bob: 2,
                squash_x: 1,
                squash_y: -1,
                step_a: 0,
                step_b: 0,
                play_lift: 0,
                appendage_lift: 0,
                tail_sway: 0,
            },
            ActionKind::SoloPlay | ActionKind::SocialPlay => Self {
                bob: -walk.abs(),
                squash_x: -walk,
                squash_y: walk,
                step_a: walk * 2,
                step_b: alternate * 2,
                play_lift: walk.abs(),
                appendage_lift: 2 + walk.abs(),
                tail_sway: walk * 2,
            },
            ActionKind::Eat | ActionKind::Drink => Self {
                bob: i32::from(frame % 2),
                squash_x: 1,
                squash_y: -1,
                step_a: 0,
                step_b: 0,
                play_lift: 0,
                appendage_lift: 1,
                tail_sway: if frame.is_multiple_of(3) { 1 } else { 0 },
            },
            ActionKind::AvoidCursor | ActionKind::ReactToWindow => Self {
                bob: -walk.abs(),
                squash_x: 1,
                squash_y: -1,
                step_a: walk * 3,
                step_b: alternate * 3,
                play_lift: 0,
                appendage_lift: 2,
                tail_sway: -2,
            },
            ActionKind::Landing => Self {
                bob: -2 - walk.abs(),
                squash_x: -walk,
                squash_y: walk,
                step_a: walk * 2,
                step_b: alternate * 2,
                play_lift: 1,
                appendage_lift: 1,
                tail_sway: walk,
            },
            ActionKind::ClimbWindow => Self {
                bob: -walk.abs(),
                squash_x: 0,
                squash_y: 1,
                step_a: -walk * 2,
                step_b: walk * 2,
                play_lift: 1,
                appendage_lift: 2 + walk.abs(),
                tail_sway: alternate,
            },
            ActionKind::Dangle => Self {
                bob: 0,
                squash_x: -1,
                squash_y: 1,
                step_a: walk,
                step_b: -walk,
                play_lift: 1,
                appendage_lift: 2,
                tail_sway: alternate * 2,
            },
            ActionKind::InspectScreen => Self {
                bob: i32::from(frame % 2),
                squash_x: 1,
                squash_y: -1,
                step_a: 0,
                step_b: -1,
                play_lift: 1,
                appendage_lift: 1 + walk.abs(),
                tail_sway: alternate,
            },
            ActionKind::PresentDiscovery => Self {
                bob: -i32::from(frame >= 1),
                squash_x: -i32::from(frame >= 1),
                squash_y: i32::from(frame >= 1),
                step_a: 0,
                step_b: 0,
                play_lift: i32::from(frame >= 1),
                appendage_lift: 2 + i32::from(frame >= 2),
                tail_sway: i32::from(frame >= 2),
            },
            _ => Self {
                bob: frame as i32 % 2,
                squash_x: 0,
                squash_y: 0,
                step_a: 0,
                step_b: 0,
                play_lift: 0,
                appendage_lift: 0,
                tail_sway: alternate,
            },
        };
        if reduce_motion {
            pose.bob = 0;
            pose.squash_x = 0;
            pose.squash_y = 0;
            pose.play_lift = 0;
            pose.appendage_lift = pose.appendage_lift.clamp(-1, 1);
            pose.tail_sway = pose.tail_sway.clamp(-1, 1);
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
    action: ActionKind,
    frame: u8,
) -> PixelPoint {
    let s = scale(genome);
    let rx = ((genome.body_width as f32 * s / 2.0).round() as i32 + pose.squash_x).clamp(6, 16);
    let ry = ((genome.body_height as f32 * s / 2.0).round() as i32 + pose.squash_y).clamp(5, 14);
    let cx = 26;
    let cy = 38 - ry + pose.bob - pose.play_lift;
    draw_tail(canvas, genome, palette, cx - rx + 1, cy, s, pose);
    draw_head_appendages(canvas, genome, palette, cx, cy - ry + 2, s, pose);
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
            action,
            frame,
            pose,
            family: BodyFamily::Blob,
        },
    );
    PixelPoint {
        x: cx + 2,
        y: cy - 1,
    }
}

fn draw_hopper(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    pose: Pose,
    action: ActionKind,
    frame: u8,
) -> PixelPoint {
    let s = scale(genome);
    // A rounder, lower crouch reads closer to a resting rabbit and leaves headroom for long ears.
    let rx = ((genome.body_width as f32 * s * 0.42).round() as i32 + pose.squash_x).clamp(6, 11);
    let ry = ((genome.body_height as f32 * s * 0.48).round() as i32 + pose.squash_y).clamp(7, 12);
    let leg = ((genome.leg_length as f32 * s).round() as i32).clamp(3, 7);
    let cx = 24;
    let ground = 43;
    let cy = ground - leg - ry + pose.bob - pose.play_lift;
    // Rooted low on the rear edge so the puff clears the body ellipse drawn over it.
    draw_tail(canvas, genome, palette, cx - rx, cy + ry / 2, s, pose);
    draw_head_appendages(canvas, genome, palette, cx, cy - ry + 2, s, pose);
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
            action,
            frame,
            pose,
            family: BodyFamily::Hopper,
        },
    );
    PixelPoint {
        x: cx + 1,
        y: cy - 2,
    }
}

fn draw_quadruped(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    pose: Pose,
    action: ActionKind,
    frame: u8,
) -> PixelPoint {
    let s = scale(genome);
    let body_rx =
        ((genome.body_width as f32 * s * 0.42).round() as i32 + pose.squash_x).clamp(7, 13);
    let body_ry =
        ((genome.body_height as f32 * s * 0.34).round() as i32 + pose.squash_y).clamp(4, 8);
    let leg = ((genome.leg_length as f32 * s).round() as i32).clamp(3, 8);
    let ground = 43;
    let body_y = ground - leg - body_ry + pose.bob - pose.play_lift;
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
            action,
            frame,
            pose,
            family: BodyFamily::SoftQuadruped,
        },
        ground,
    );
    let head_x = body_x + body_rx - 1;
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
    PixelPoint {
        x: head_x + 1,
        y: head_y - 1,
    }
}

fn draw_face(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    center_x: i32,
    center_y: i32,
    state: FaceRenderState,
) {
    let face = genome.face;
    let spacing = (face.eye_spacing as i32 / 2).clamp(2, 3);
    let y = center_y + face.vertical_offset as i32;
    let eye_radius = face.eye_size as i32;
    let eye_y_offsets = match state.expression {
        ExpressionKind::Worried => (1, 0),
        ExpressionKind::Curious => (0, -1),
        _ => (0, 0),
    };
    for (index, x) in [center_x - spacing, center_x + spacing]
        .into_iter()
        .enumerate()
    {
        let eye_y = y + if index == 0 {
            eye_y_offsets.0
        } else {
            eye_y_offsets.1
        };
        draw_eye(canvas, palette, face, x, eye_y, state);
    }
    draw_brows(
        canvas,
        genome,
        palette,
        center_x,
        y,
        spacing,
        state.expression,
    );
    draw_cheeks(
        canvas,
        genome,
        palette,
        center_x,
        y,
        spacing,
        state.expression,
    );
    draw_mouth(
        canvas,
        genome,
        palette,
        center_x,
        y + eye_radius + 3,
        state.expression,
    );
}

fn draw_eye(
    canvas: &mut Canvas,
    palette: Palette,
    face: formiga_core::FaceGenome,
    x: i32,
    y: i32,
    state: FaceRenderState,
) {
    let radius = face.eye_size as i32;
    if state.eyelids == EyelidPose::Closed {
        let curve = matches!(
            state.expression,
            ExpressionKind::Joy | ExpressionKind::Content | ExpressionKind::Affectionate
        );
        canvas.line(
            x - radius,
            y,
            x + radius,
            y + i32::from(curve),
            1,
            palette.eye,
        );
        return;
    }
    match face.eye_shape {
        EyeShape::Round => canvas.fill_circle(x, y, radius + 1, palette.outline),
        EyeShape::Tall => canvas.fill_ellipse(x, y, radius + 1, radius + 2, palette.outline),
        EyeShape::SoftSquare => canvas.fill_rect(
            x - radius - 1,
            y - radius - 1,
            radius * 2 + 3,
            radius * 2 + 3,
            palette.outline,
        ),
    }
    match face.eye_shape {
        EyeShape::Round => canvas.fill_circle(x, y, radius, palette.eye),
        EyeShape::Tall => canvas.fill_ellipse(x, y, radius, radius + 1, palette.eye),
        EyeShape::SoftSquare => canvas.fill_rect(
            x - radius,
            y - radius,
            radius * 2 + 1,
            radius * 2 + 1,
            palette.eye,
        ),
    }
    if state.eyelids == EyelidPose::Half {
        canvas.fill_rect(
            x - radius - 1,
            y - radius - 2,
            radius * 2 + 3,
            radius + 2,
            palette.coat,
        );
        canvas.line(
            x - radius - 1,
            y - 1,
            x + radius + 1,
            y - 1,
            1,
            palette.outline,
        );
    }
    let pupil_x = x + i32::from(state.gaze.x);
    let pupil_y = y + i32::from(state.gaze.y);
    let white = Rgba::new(255, 255, 245, 255);
    match face.highlight_style {
        HighlightStyle::Single => canvas.set(pupil_x, pupil_y - radius.min(1), white),
        HighlightStyle::Double => {
            canvas.set(pupil_x, pupil_y - radius.min(1), white);
            canvas.set(pupil_x + 1, pupil_y + 1, white);
        }
        HighlightStyle::Diagonal => {
            canvas.set(pupil_x - 1, pupil_y - 1, white);
            canvas.set(pupil_x, pupil_y, white);
        }
    }
    match face.pupil_style {
        PupilStyle::Dot => {}
        PupilStyle::Wide => canvas.set(pupil_x - 1, pupil_y, white),
        PupilStyle::Spark => {
            canvas.set(pupil_x + 1, pupil_y, white);
            canvas.set(pupil_x, pupil_y + 1, white);
        }
    }
}

fn draw_brows(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    center_x: i32,
    y: i32,
    spacing: i32,
    expression: ExpressionKind,
) {
    let weight = match genome.face.brow_style {
        BrowStyle::None
            if matches!(
                expression,
                ExpressionKind::Neutral | ExpressionKind::Content | ExpressionKind::Sleepy
            ) =>
        {
            return;
        }
        BrowStyle::Bold => 2,
        _ => 1,
    };
    let (left_inner, right_inner) = match expression {
        ExpressionKind::Worried | ExpressionKind::Affectionate => (-1, -1),
        ExpressionKind::Focused | ExpressionKind::Determined => (1, 1),
        ExpressionKind::Startled | ExpressionKind::Curious => (-1, 1),
        ExpressionKind::Bored | ExpressionKind::Sleepy => (1, 0),
        _ => (0, 0),
    };
    canvas.line(
        center_x - spacing - 1,
        y - 3 + left_inner,
        center_x - spacing + 1,
        y - 3 - left_inner,
        weight,
        palette.outline,
    );
    canvas.line(
        center_x + spacing - 1,
        y - 3 - right_inner,
        center_x + spacing + 1,
        y - 3 + right_inner,
        weight,
        palette.outline,
    );
}

fn draw_cheeks(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    center_x: i32,
    y: i32,
    spacing: i32,
    expression: ExpressionKind,
) {
    if genome.face.cheek_style == CheekStyle::None
        && !matches!(
            expression,
            ExpressionKind::Joy | ExpressionKind::Affectionate
        )
    {
        return;
    }
    let cheek_y = y + 3;
    for x in [center_x - spacing - 2, center_x + spacing + 2] {
        canvas.set(x, cheek_y, palette.accent);
        if genome.face.cheek_style == CheekStyle::Blush {
            canvas.set(x + 1, cheek_y, palette.accent);
        }
    }
}

fn draw_mouth(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    x: i32,
    y: i32,
    expression: ExpressionKind,
) {
    match expression {
        ExpressionKind::Joy => {
            canvas.set(x - 2, y - 1, palette.outline);
            canvas.set(x - 1, y - 1, palette.outline);
            canvas.set(x, y, palette.outline);
            canvas.set(x + 1, y - 1, palette.outline);
            canvas.set(x + 2, y - 1, palette.outline);
        }
        ExpressionKind::Affectionate => {
            canvas.set(x - 1, y - 1, palette.accent);
            canvas.set(x, y, palette.outline);
            canvas.set(x + 1, y - 1, palette.accent);
        }
        ExpressionKind::Content => {
            canvas.set(x - 1, y, palette.outline);
            canvas.set(x, y + 1, palette.outline);
            canvas.set(x + 1, y, palette.outline);
        }
        ExpressionKind::Startled | ExpressionKind::Curious => {
            canvas.fill_circle(x, y, 1, palette.outline);
            canvas.set(x, y, palette.coat);
        }
        ExpressionKind::Worried => {
            canvas.set(x - 1, y, palette.outline);
            canvas.set(x, y - 1, palette.outline);
            canvas.set(x + 1, y, palette.outline);
        }
        ExpressionKind::Bored => canvas.line(x - 1, y, x + 1, y, 1, palette.outline),
        ExpressionKind::Focused => canvas.line(x - 1, y, x + 1, y - 1, 1, palette.outline),
        ExpressionKind::Determined => canvas.line(x - 2, y, x + 2, y, 1, palette.outline),
        _ => match genome.face.mouth_style {
            MouthStyle::Tiny => canvas.set(x, y, palette.outline),
            MouthStyle::Smile => canvas.line(x - 1, y - 1, x + 1, y - 1, 1, palette.outline),
            MouthStyle::Cat => {
                canvas.set(x - 1, y - 1, palette.outline);
                canvas.set(x, y, palette.outline);
                canvas.set(x + 1, y - 1, palette.outline);
            }
            MouthStyle::Beak => {
                canvas.set(x, y - 1, palette.accent);
                canvas.set(x + 1, y, palette.accent);
            }
        },
    }
}

fn draw_head_appendages(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    cx: i32,
    root_y: i32,
    s: f32,
    pose: Pose,
) {
    let size = ((genome.head_appendages.size as f32 * s).round() as i32).clamp(2, 8);
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
struct EarShape {
    base_x: i32,
    base_y: i32,
    half_width: i32,
    height: i32,
    lean: i32,
}

/// One upright triangular ear, drawn as stacked rows so the tip stays pointed. Each `inset` step
/// shrinks the triangle by a pixel, layering the coat and inner ear inside the outline.
fn fill_triangle_ear(canvas: &mut Canvas, ear: EarShape, inset: i32, color: Rgba) {
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

fn draw_cat_ears(
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

fn draw_rabbit_ears(
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
struct LimbPose {
    left_root: PixelPoint,
    right_root: PixelPoint,
    action: ActionKind,
    frame: u8,
    pose: Pose,
    family: BodyFamily,
}

fn draw_forelimbs(
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
        limb_pose.action,
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

fn draw_quadruped_forelimbs(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    limb_pose: LimbPose,
    ground: i32,
) {
    // Standing on four legs is what separates the cat silhouette from an upright body. Arm-style
    // forelimbs are reserved for the actions where the creature is visibly using its paws.
    if !matches!(
        limb_pose.action,
        ActionKind::SoloPlay
            | ActionKind::SocialPlay
            | ActionKind::Greet
            | ActionKind::PresentDiscovery
            | ActionKind::PetReaction
            | ActionKind::Dangle
            | ActionKind::ClimbWindow
            | ActionKind::InvestigateCursor
            | ActionKind::Dragged
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

fn limb_targets(
    left: PixelPoint,
    right: PixelPoint,
    action: ActionKind,
    frame: u8,
    length: i32,
    rest: RestPose,
    pose: Pose,
) -> (PixelPoint, PixelPoint) {
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

fn offset_pair(
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

fn draw_limb(
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

fn draw_cat_tail(
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

fn draw_cotton_tail(
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
    // A long hind foot planted forward of the ankle.
    canvas.fill_ellipse(x + step + 2, ground, 5, 2, palette.outline);
    canvas.fill_ellipse(x + step + 3, ground, 4, 1, palette.accent);
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

fn draw_activity_prop(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    face: PixelPoint,
    action: ActionKind,
    frame: u8,
    reduce_motion: bool,
) {
    let phase = if reduce_motion { 0 } else { frame % 4 };
    let variant = ((genome.marking_seed ^ u64::from(genome.face_signature)) % 4) as u8;
    match action {
        ActionKind::SoloPlay => {
            let (x, y) = match phase {
                0 => (face.x + 7, face.y + 8),
                1 => (face.x + 10, face.y + 3),
                2 => (face.x + 6, face.y - 7),
                _ => (face.x + 3, face.y + 2),
            };
            draw_generated_toy(canvas, palette, genome.effect_motif, variant, x, y);
        }
        ActionKind::Eat => {
            let x = face.x + 5 - i32::from(phase >= 2);
            let y = face.y + 5 - i32::from(phase % 3);
            draw_generated_snack(canvas, palette, variant, x, y, phase);
        }
        ActionKind::Drink => {
            let x = face.x + 4;
            let y = face.y + 7 - i32::from(phase == 1 || phase == 2) * 2;
            draw_generated_drinkware(canvas, palette, variant, x, y);
        }
        _ => {}
    }
}

fn draw_generated_toy(
    canvas: &mut Canvas,
    palette: Palette,
    motif: EffectMotif,
    variant: u8,
    x: i32,
    y: i32,
) {
    match variant {
        0 => {
            canvas.fill_circle(x, y, 3, palette.outline);
            canvas.fill_circle(x, y, 2, palette.accent);
            canvas.line(x - 1, y - 2, x + 2, y + 1, 1, palette.highlight);
        }
        1 => {
            canvas.fill_circle(x, y, 3, palette.outline);
            canvas.fill_circle(x, y, 2, palette.shadow);
            canvas.line(x - 2, y, x + 2, y - 1, 1, palette.highlight);
            canvas.line(x - 1, y + 2, x + 1, y - 2, 1, palette.accent);
            canvas.line(x + 2, y + 1, x + 4, y + 2, 1, palette.shadow);
        }
        2 => {
            canvas.line(x - 2, y + 2, x + 2, y - 2, 1, palette.outline);
            canvas.fill_ellipse(x - 1, y, 2, 1, palette.accent);
            canvas.fill_ellipse(x + 1, y - 1, 2, 1, palette.highlight);
        }
        _ => draw_motif(
            canvas,
            if motif == EffectMotif::None {
                EffectMotif::Star
            } else {
                motif
            },
            x,
            y,
            palette.accent,
        ),
    }
}

fn draw_generated_snack(
    canvas: &mut Canvas,
    palette: Palette,
    variant: u8,
    x: i32,
    y: i32,
    phase: u8,
) {
    if phase == 3 {
        canvas.set(x - 1, y, palette.accent);
        canvas.set(x + 1, y + 1, palette.highlight);
        return;
    }
    match variant % 3 {
        0 => {
            canvas.fill_circle(x, y, 2, palette.outline);
            canvas.fill_circle(x, y, 1, palette.accent);
            canvas.line(x, y - 2, x + 1, y - 4, 1, palette.shadow);
        }
        1 => {
            canvas.fill_rect(x - 3, y - 2, 6, 5, palette.outline);
            canvas.fill_rect(x - 2, y - 1, 4, 3, palette.accent);
            canvas.set(x - 1, y, palette.shadow);
            canvas.set(x + 1, y + 1, palette.shadow);
        }
        _ => {
            canvas.line(x - 2, y + 2, x + 2, y - 2, 1, palette.outline);
            canvas.fill_ellipse(x, y, 3, 1, palette.accent);
            canvas.line(x - 1, y + 1, x + 1, y - 1, 1, palette.highlight);
        }
    }
}

fn draw_generated_drinkware(canvas: &mut Canvas, palette: Palette, variant: u8, x: i32, y: i32) {
    if variant.is_multiple_of(2) {
        canvas.fill_rect(x - 3, y - 3, 6, 5, palette.outline);
        canvas.fill_rect(x - 2, y - 2, 4, 3, palette.highlight);
        canvas.line(x + 3, y - 2, x + 4, y + 1, 1, palette.outline);
    } else {
        canvas.fill_ellipse(x, y, 4, 2, palette.outline);
        canvas.fill_ellipse(x, y - 1, 3, 1, palette.highlight);
        canvas.line(x - 3, y, x - 2, y + 2, 1, palette.outline);
        canvas.line(x + 3, y, x + 2, y + 2, 1, palette.outline);
    }
    canvas.set(x, y - 2, palette.accent);
}

fn draw_generated_trinket(canvas: &mut Canvas, palette: Palette, variant: u8, detail_seed: u64) {
    let accent = if detail_seed & 1 == 0 {
        palette.accent
    } else {
        palette.highlight
    };
    match variant % 8 {
        // Gem
        0 => {
            canvas.line(8, 2, 13, 7, 1, palette.outline);
            canvas.line(13, 7, 8, 14, 1, palette.outline);
            canvas.line(8, 14, 3, 7, 1, palette.outline);
            canvas.line(3, 7, 8, 2, 1, palette.outline);
            canvas.fill_ellipse(8, 8, 3, 4, accent);
            canvas.line(6, 5, 9, 4, 1, palette.highlight);
        }
        // Key
        1 => {
            canvas.fill_circle(5, 5, 3, palette.outline);
            canvas.fill_circle(5, 5, 1, Rgba::TRANSPARENT);
            canvas.line(7, 7, 13, 13, 2, palette.outline);
            canvas.line(7, 7, 13, 13, 1, accent);
            canvas.line(11, 11, 13, 9, 1, palette.outline);
        }
        // Leaf
        2 => {
            canvas.fill_ellipse(8, 7, 5, 4, palette.outline);
            canvas.fill_ellipse(8, 7, 4, 3, accent);
            canvas.line(4, 11, 12, 3, 1, palette.shadow);
            canvas.line(8, 7, 12, 8, 1, palette.highlight);
        }
        // Shell
        3 => {
            canvas.fill_ellipse(8, 9, 6, 4, palette.outline);
            canvas.fill_ellipse(8, 8, 5, 3, accent);
            canvas.line(8, 5, 8, 12, 1, palette.shadow);
            canvas.line(5, 6, 6, 12, 1, palette.highlight);
            canvas.line(11, 6, 10, 12, 1, palette.shadow);
        }
        // Ring charm
        4 => {
            canvas.fill_circle(8, 8, 5, palette.outline);
            canvas.fill_circle(8, 8, 3, Rgba::TRANSPARENT);
            canvas.fill_circle(8, 2, 2, accent);
            canvas.set(7, 1, palette.highlight);
        }
        // Tiny bottle
        5 => {
            canvas.fill_rect(6, 2, 4, 3, palette.outline);
            canvas.fill_rect(4, 5, 8, 9, palette.outline);
            canvas.fill_rect(5, 6, 6, 7, palette.shadow);
            canvas.fill_rect(5, 9, 6, 4, accent);
            canvas.set(6, 7, palette.highlight);
        }
        // Star relic
        6 => {
            canvas.line(8, 2, 8, 14, 2, palette.outline);
            canvas.line(2, 8, 14, 8, 2, palette.outline);
            canvas.line(4, 4, 12, 12, 1, palette.outline);
            canvas.line(12, 4, 4, 12, 1, palette.outline);
            canvas.fill_circle(8, 8, 2, accent);
            canvas.set(8, 7, palette.highlight);
        }
        // Odd little tablet
        _ => {
            canvas.fill_rect(3, 3, 10, 11, palette.outline);
            canvas.fill_rect(4, 4, 8, 9, accent);
            canvas.fill_circle(8, 7, 2, palette.shadow);
            canvas.line(6, 11, 10, 11, 1, palette.highlight);
        }
    }
}

fn draw_effects(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    face: PixelPoint,
    action: ActionKind,
    frame: u8,
    reduce_motion: bool,
) {
    let pulse = if reduce_motion {
        0
    } else {
        i32::from(frame % 2)
    };
    match action {
        ActionKind::Sleep => {
            let x = (face.x + 7).min(44);
            let y = (face.y - 7 - pulse).max(3);
            canvas.line(x, y + 2, x + 2, y + 2, 1, palette.accent);
            canvas.line(x + 2, y + 2, x, y, 1, palette.accent);
            canvas.line(x, y, x + 2, y, 1, palette.accent);
        }
        ActionKind::InvestigateCursor => {
            let x = (face.x + 7).min(44);
            let y = (face.y - 6 - pulse).max(3);
            canvas.set(x, y, palette.accent);
            canvas.set(x + 1, y - 1, palette.accent);
            canvas.set(x + 1, y + 1, palette.accent);
            canvas.set(x + 1, y + 3, palette.accent);
        }
        ActionKind::SoloPlay => draw_motif(
            canvas,
            genome.effect_motif,
            (face.x + 8).min(44),
            (face.y + pulse - 7).max(3),
            palette.accent,
        ),
        ActionKind::Eat => {
            if frame % 4 == 3 {
                canvas.set((face.x + 6).min(44), face.y + 2, palette.accent);
                canvas.set((face.x + 8).min(45), face.y + 4, palette.highlight);
            }
        }
        ActionKind::Drink => {
            if frame % 4 == 2 {
                canvas.set((face.x + 10).min(45), face.y + 5, palette.highlight);
            }
        }
        ActionKind::Sprint => {
            let y = (face.y + 5).min(43);
            canvas.line(face.x - 10, y - 3, face.x - 7, y - 3, 1, palette.accent);
            if !reduce_motion {
                canvas.line(face.x - 12, y, face.x - 8, y, 1, palette.highlight);
            }
        }
        ActionKind::Greet | ActionKind::SocialPlay => {
            draw_motif(
                canvas,
                if genome.effect_motif == EffectMotif::None {
                    EffectMotif::Spark
                } else {
                    genome.effect_motif
                },
                (face.x + 8).min(44),
                (face.y - 7 - pulse).max(3),
                palette.accent,
            );
        }
        ActionKind::AvoidCursor | ActionKind::ReactToWindow => {
            let y = (face.y - 7).max(3);
            canvas.line(face.x - 7, y + 2, face.x - 9, y, 1, palette.accent);
            canvas.line(face.x + 7, y + 2, face.x + 9, y, 1, palette.accent);
        }
        ActionKind::ClimbWindow => {
            if !reduce_motion && frame.is_multiple_of(2) {
                canvas.set((face.x + 8).min(45), (face.y - 5).max(2), palette.highlight);
            }
        }
        ActionKind::InspectScreen => {
            let x = (face.x + 7).min(43);
            let y = (face.y + 1).clamp(4, 42);
            canvas.fill_circle(x, y, 2, palette.outline);
            canvas.fill_circle(x, y, 1, Rgba::TRANSPARENT);
            canvas.line(x + 2, y + 2, x + 4, y + 4, 1, palette.accent);
        }
        ActionKind::PresentDiscovery => draw_motif(
            canvas,
            if genome.effect_motif == EffectMotif::None {
                EffectMotif::Star
            } else {
                genome.effect_motif
            },
            (face.x + 9).min(44),
            (face.y - 9 - pulse).max(3),
            palette.highlight,
        ),
        _ => {}
    }
}

fn draw_motif(canvas: &mut Canvas, motif: EffectMotif, x: i32, y: i32, color: Rgba) {
    match motif {
        EffectMotif::None => {}
        EffectMotif::Dot => canvas.fill_circle(x, y, 1, color),
        EffectMotif::Star | EffectMotif::Spark => {
            canvas.line(x - 2, y, x + 2, y, 1, color);
            canvas.line(x, y - 2, x, y + 2, 1, color);
        }
        EffectMotif::Heart => {
            canvas.set(x - 1, y - 1, color);
            canvas.set(x + 1, y - 1, color);
            canvas.fill_rect(x - 1, y, 3, 2, color);
            canvas.set(x, y + 2, color);
        }
        EffectMotif::Leaf => {
            canvas.line(x - 1, y + 1, x + 1, y - 1, 1, color);
            canvas.set(x - 1, y, color);
            canvas.set(x, y - 1, color);
        }
    }
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

fn expression_for_action(action: ActionKind) -> ExpressionKind {
    match action {
        ActionKind::Idle => ExpressionKind::Neutral,
        ActionKind::Traverse | ActionKind::SqueezeWindow | ActionKind::RideWindow => {
            ExpressionKind::Focused
        }
        ActionKind::Sprint => ExpressionKind::Determined,
        ActionKind::Perch | ActionKind::Homebound => ExpressionKind::Content,
        ActionKind::Sleep => ExpressionKind::Sleepy,
        ActionKind::InvestigateCursor => ExpressionKind::Curious,
        ActionKind::AvoidCursor => ExpressionKind::Worried,
        ActionKind::ReactToWindow => ExpressionKind::Startled,
        ActionKind::SoloPlay | ActionKind::SocialPlay => ExpressionKind::Joy,
        ActionKind::Eat | ActionKind::Drink => ExpressionKind::Content,
        ActionKind::Greet | ActionKind::Follow | ActionKind::PetReaction => {
            ExpressionKind::Affectionate
        }
        ActionKind::Dragged => ExpressionKind::Curious,
        ActionKind::Landing => ExpressionKind::Determined,
        ActionKind::ClimbWindow => ExpressionKind::Determined,
        ActionKind::Dangle => ExpressionKind::Content,
        ActionKind::InspectScreen => ExpressionKind::Curious,
        ActionKind::PresentDiscovery => ExpressionKind::Joy,
        ActionKind::Tossed => ExpressionKind::Startled,
    }
}

fn default_eyelids(action: ActionKind, frame: u8) -> EyelidPose {
    if action == ActionKind::Sleep {
        EyelidPose::Closed
    } else if frame % 8 == 7 {
        EyelidPose::Half
    } else {
        EyelidPose::Open
    }
}

fn resolve_expression(creature: &Creature) -> ExpressionKind {
    let drives = &creature.state.drives;
    if creature.state.action == ActionKind::Sleep {
        return ExpressionKind::Sleepy;
    }
    if drives.arousal > 0.86 {
        return ExpressionKind::Startled;
    }
    match creature.state.action {
        ActionKind::Idle => {
            if drives.sleep_pressure > 0.72 || drives.energy < 0.2 {
                ExpressionKind::Sleepy
            } else if drives.boredom > 0.66 {
                ExpressionKind::Bored
            } else if drives.comfort > 0.7 {
                ExpressionKind::Content
            } else {
                ExpressionKind::Neutral
            }
        }
        ActionKind::Traverse | ActionKind::SqueezeWindow => {
            if drives.arousal > 0.45 {
                ExpressionKind::Focused
            } else {
                ExpressionKind::Content
            }
        }
        ActionKind::Sprint => {
            if creature.personality.playfulness > 0.68 && drives.arousal < 0.72 {
                ExpressionKind::Joy
            } else {
                ExpressionKind::Determined
            }
        }
        ActionKind::Perch => {
            if drives.sleep_pressure > 0.65 {
                ExpressionKind::Sleepy
            } else {
                ExpressionKind::Content
            }
        }
        ActionKind::Homebound => ExpressionKind::Content,
        ActionKind::InvestigateCursor => {
            if drives.arousal > 0.5 {
                ExpressionKind::Focused
            } else {
                ExpressionKind::Curious
            }
        }
        ActionKind::AvoidCursor => ExpressionKind::Worried,
        ActionKind::ReactToWindow => ExpressionKind::Startled,
        ActionKind::RideWindow => {
            if drives.arousal > 0.45 {
                ExpressionKind::Worried
            } else {
                ExpressionKind::Focused
            }
        }
        ActionKind::SoloPlay => ExpressionKind::Joy,
        ActionKind::Eat | ActionKind::Drink => ExpressionKind::Content,
        ActionKind::Greet | ActionKind::Follow | ActionKind::SocialPlay => {
            if creature.tendencies.sociability >= 35 || creature.state.drives.comfort > 0.75 {
                ExpressionKind::Affectionate
            } else if creature.state.action == ActionKind::SocialPlay {
                ExpressionKind::Joy
            } else {
                ExpressionKind::Focused
            }
        }
        ActionKind::Dragged => {
            if drives.arousal > 0.55 {
                ExpressionKind::Startled
            } else {
                ExpressionKind::Curious
            }
        }
        ActionKind::Landing => ExpressionKind::Determined,
        ActionKind::ClimbWindow => ExpressionKind::Determined,
        ActionKind::Dangle => {
            if drives.arousal > 0.5 {
                ExpressionKind::Focused
            } else {
                ExpressionKind::Content
            }
        }
        ActionKind::InspectScreen => ExpressionKind::Curious,
        ActionKind::PresentDiscovery => ExpressionKind::Joy,
        ActionKind::Tossed => ExpressionKind::Startled,
        ActionKind::PetReaction => ExpressionKind::Affectionate,
        ActionKind::Sleep => ExpressionKind::Sleepy,
    }
}

fn resolve_eyelids(creature: &Creature) -> EyelidPose {
    if creature.state.action == ActionKind::Sleep {
        return EyelidPose::Closed;
    }
    if matches!(resolve_expression(creature), ExpressionKind::Startled) {
        return EyelidPose::Open;
    }
    let elapsed = creature.state.action_elapsed.max(0.0);
    let block = (elapsed / 5.0).floor() as u64;
    let local = elapsed % 5.0;
    let seed = u64::from_le_bytes(creature.behavior_seed[..8].try_into().unwrap());
    let mixed = mix_u64(seed ^ block.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let blink_at = 0.65 + (mixed % 360) as f32 / 100.0;
    let blink_delta = (local - blink_at).abs();
    if blink_delta < 0.055 {
        EyelidPose::Closed
    } else if blink_delta < 0.14 || resolve_expression(creature) == ExpressionKind::Sleepy {
        EyelidPose::Half
    } else {
        EyelidPose::Open
    }
}

fn mix_u64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn resolve_gaze(
    creature: &Creature,
    cursor: CursorSnapshot,
    cursor_reactions: bool,
) -> GazeDirection {
    match creature.state.action {
        ActionKind::InspectScreen => {
            return GazeDirection::new(
                if creature.state.facing_right { 1 } else { -1 },
                if creature.state.surface.kind == formiga_core::SurfaceKind::WindowLedge {
                    1
                } else {
                    -1
                },
            );
        }
        ActionKind::Dangle => return GazeDirection::new(0, 1),
        ActionKind::PresentDiscovery => return GazeDirection::new(0, -1),
        _ => {}
    }
    if !cursor_reactions
        || !cursor.available
        || creature.state.position.distance(cursor.position) > 240.0
    {
        return GazeDirection::default();
    }
    let face_position = formiga_core::Point {
        x: creature.state.position.x,
        y: creature.state.position.y - 28.0,
    };
    let dx = cursor.position.x - face_position.x;
    let dy = cursor.position.y - face_position.y;
    GazeDirection::new(axis_direction(dx, 10.0), axis_direction(dy, 10.0))
}

fn axis_direction(delta: f32, dead_zone: f32) -> i8 {
    if delta.abs() <= dead_zone {
        0
    } else if delta > 0.0 {
        1
    } else {
        -1
    }
}

fn blit_transparent(target: &mut Canvas, source: &Canvas, origin_x: i32, origin_y: i32) {
    for y in 0..source.height() as i32 {
        for x in 0..source.width() as i32 {
            let pixel = source.get(x, y);
            if pixel.a > 0 {
                target.set(origin_x + x, origin_y + y, pixel);
            }
        }
    }
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
            head_appendages: formiga_core::HeadAppendageGenome {
                style: HeadAppendageStyle::Pointed,
                size: 5,
            },
            tail_style: TailStyle::Curl,
            tail_length: 8,
            face: formiga_core::FaceGenome {
                eye_shape: EyeShape::Round,
                eye_size: 1,
                eye_spacing: 5,
                vertical_offset: 0,
                pupil_style: PupilStyle::Dot,
                highlight_style: HighlightStyle::Single,
                brow_style: BrowStyle::Soft,
                mouth_style: MouthStyle::Smile,
                cheek_style: CheekStyle::Dots,
            },
            forelimbs: formiga_core::ForelimbGenome {
                style: match family {
                    BodyFamily::Blob => ForelimbStyle::Pseudopod,
                    BodyFamily::Hopper => ForelimbStyle::MittenArm,
                    BodyFamily::SoftQuadruped => ForelimbStyle::FrontPaw,
                },
                length: 5,
                thickness: 1,
                tip_style: match family {
                    BodyFamily::Blob => LimbTipStyle::Round,
                    BodyFamily::Hopper => LimbTipStyle::Mitten,
                    BodyFamily::SoftQuadruped => LimbTipStyle::Paw,
                },
                rest_pose: RestPose::AtSides,
            },
            effect_motif: EffectMotif::Spark,
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
    fn generated_activity_props_are_deterministic_distinct_and_opaque() {
        let palette = PALETTES[2];
        let mut hashes = std::collections::BTreeSet::new();
        for variant in 0..4 {
            let mut first = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            let mut second = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            draw_generated_toy(&mut first, palette, EffectMotif::Spark, variant, 24, 24);
            draw_generated_toy(&mut second, palette, EffectMotif::Spark, variant, 24, 24);
            assert_eq!(first, second);
            assert!(
                AlphaMask::from_canvas(&first)
                    .pixels
                    .into_iter()
                    .any(|pixel| pixel)
            );
            hashes.insert(Sha256::digest(first.rgba_bytes()).to_vec());
        }
        assert_eq!(hashes.len(), 4);

        let mut snack = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        draw_generated_snack(&mut snack, palette, 1, 24, 24, 0);
        let mut drinkware = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        draw_generated_drinkware(&mut drinkware, palette, 1, 24, 24);
        assert!(snack.alpha_bounds().is_some());
        assert!(drinkware.alpha_bounds().is_some());
        assert_ne!(snack, drinkware);
    }

    #[test]
    fn generated_discoveries_have_eight_deterministic_opaque_silhouettes() {
        let genome = genome(BodyFamily::Blob);
        let mut hashes = std::collections::BTreeSet::new();
        for variant in 0..8 {
            let first = CreatureRenderer::render_trinket(&genome, variant);
            let second = CreatureRenderer::render_trinket(&genome, variant);
            assert_eq!(first, second);
            assert!(first.alpha_bounds().is_some(), "variant {variant} is empty");
            assert!(
                first
                    .pixels()
                    .iter()
                    .filter(|pixel| pixel.a > 0)
                    .all(|pixel| pixel.a == u8::MAX),
                "variant {variant} contains translucent runtime pixels"
            );
            hashes.insert(Sha256::digest(first.rgba_bytes()).to_vec());
        }
        assert_eq!(hashes.len(), 8);
    }

    #[test]
    fn ambient_animation_specs_and_shared_handhold_placement_are_exact() {
        for (action, fps) in [
            (ActionKind::ClimbWindow, 6),
            (ActionKind::Dangle, 3),
            (ActionKind::InspectScreen, 4),
            (ActionKind::PresentDiscovery, 2),
        ] {
            let spec = AnimationSpec::for_action(action);
            assert_eq!(spec.frames, 4);
            assert_eq!(spec.fps, fps);
        }
        let discovery = AnimationSpec::for_action(ActionKind::PresentDiscovery);
        assert_eq!(discovery.playback, PlaybackMode::Hold);
        assert_eq!(discovery.frame_at(20.0), 3);
        assert_eq!(
            AnimationSpec::body_action(ActionKind::Tossed),
            ActionKind::Dragged
        );
        assert_eq!(
            AnimationSpec::body_action(ActionKind::PetReaction),
            ActionKind::Greet
        );
        assert_eq!(
            FramePlacement::for_action(ActionKind::Dangle, 5).origin_y,
            -7
        );
        assert_eq!(
            FramePlacement::for_action(ActionKind::Idle, 5).origin_y,
            -43
        );
    }

    #[test]
    fn reduced_motion_ambient_body_variants_are_static() {
        let genome = genome(BodyFamily::SoftQuadruped);
        for action in [
            ActionKind::ClimbWindow,
            ActionKind::Dangle,
            ActionKind::InspectScreen,
            ActionKind::PresentDiscovery,
        ] {
            let first = CreatureRenderer::render_body_frame(&genome, action, 0, true);
            for frame in 1..AnimationSpec::for_action(action).frames {
                assert_eq!(
                    first,
                    CreatureRenderer::render_body_frame(&genome, action, frame, true),
                    "{action:?} frame {frame} moves in reduced-motion mode"
                );
            }
        }
    }

    #[test]
    fn every_expression_keeps_two_readable_eyes_and_a_distinct_silhouette() {
        use std::collections::BTreeSet;

        for family in [
            BodyFamily::Blob,
            BodyFamily::Hopper,
            BodyFamily::SoftQuadruped,
        ] {
            let genome = genome(family);
            let palette = PALETTES[genome.palette_index as usize % PALETTES.len()];
            let mut hashes = BTreeSet::new();
            for expression in ExpressionKind::ALL {
                let face = CreatureRenderer::render_face_frame(
                    &genome,
                    FaceRenderState {
                        expression,
                        eyelids: EyelidPose::Open,
                        gaze: GazeDirection::default(),
                    },
                );
                let left_eye = face.pixels().iter().enumerate().any(|(index, pixel)| {
                    index as u32 % FACE_FRAME_SIZE < FACE_FRAME_SIZE / 2 && *pixel == palette.eye
                });
                let right_eye = face.pixels().iter().enumerate().any(|(index, pixel)| {
                    index as u32 % FACE_FRAME_SIZE >= FACE_FRAME_SIZE / 2 && *pixel == palette.eye
                });
                assert!(left_eye && right_eye, "{family:?} {expression:?}");
                hashes.insert(Sha256::digest(face.rgba_bytes()).to_vec());
            }
            assert_eq!(hashes.len(), ExpressionKind::ALL.len(), "{family:?}");
        }
    }

    #[test]
    fn all_body_anchors_keep_the_layered_face_inside_the_sprite() {
        let half_face = FACE_FRAME_SIZE as i32 / 2;
        for family in [
            BodyFamily::Blob,
            BodyFamily::Hopper,
            BodyFamily::SoftQuadruped,
        ] {
            let genome = genome(family);
            for action in ActionKind::ALL {
                let spec = AnimationSpec::for_action(action);
                for frame in 0..spec.frames {
                    let rendered =
                        CreatureRenderer::render_body_frame(&genome, action, frame, false);
                    assert!(
                        rendered.face_anchor.x - half_face >= 0,
                        "{family:?} {action:?}"
                    );
                    assert!(
                        rendered.face_anchor.y - half_face >= 0,
                        "{family:?} {action:?}"
                    );
                    assert!(
                        rendered.face_anchor.x + half_face < FRAME_SIZE as i32,
                        "{family:?} {action:?}"
                    );
                    assert!(
                        rendered.face_anchor.y + half_face < FRAME_SIZE as i32,
                        "{family:?} {action:?}"
                    );
                    assert_eq!(
                        rendered.alpha_mask.pixels.len(),
                        (FRAME_SIZE * FRAME_SIZE) as usize
                    );
                }
            }
        }
    }

    #[test]
    fn gaze_supports_all_nine_directions() {
        let genome = genome(BodyFamily::Blob);
        let mut hashes = std::collections::BTreeSet::new();
        for y in -1..=1 {
            for x in -1..=1 {
                let face = CreatureRenderer::render_face_frame(
                    &genome,
                    FaceRenderState {
                        expression: ExpressionKind::Neutral,
                        eyelids: EyelidPose::Open,
                        gaze: GazeDirection::new(x, y),
                    },
                );
                hashes.insert(Sha256::digest(face.rgba_bytes()).to_vec());
            }
        }
        assert_eq!(hashes.len(), 9);
    }

    #[test]
    fn runtime_face_state_combines_drives_activity_cursor_and_seeded_blinks() {
        let desktop = DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: formiga_core::DisplayKey([2; 16]),
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 800.0,
                    height: 600.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 800.0,
                    height: 536.0,
                },
                scale_factor: 1.0,
                primary: true,
            }],
            ..DesktopSnapshot::default()
        };
        let mut world = World::new([23; 32], time::OffsetDateTime::UNIX_EPOCH, &desktop);
        let creature = &mut world.save.creatures[0];
        creature.state.action = ActionKind::Idle;
        creature.state.drives.boredom = 0.9;
        creature.state.drives.comfort = 0.2;
        creature.state.drives.arousal = 0.1;
        let cursor = CursorSnapshot {
            position: formiga_core::Point {
                x: creature.state.position.x + 100.0,
                y: creature.state.position.y - 128.0,
            },
            available: true,
            ..CursorSnapshot::default()
        };
        let state = CreatureRenderer::resolve_face_state(creature, cursor, true);
        assert_eq!(state.expression, ExpressionKind::Bored);
        assert_eq!(state.gaze, GazeDirection::new(1, -1));

        let mut saw_blink = false;
        for step in 0..500 {
            creature.state.action_elapsed = step as f32 / 50.0;
            let first = CreatureRenderer::resolve_face_state(creature, cursor, true);
            let second = CreatureRenderer::resolve_face_state(creature, cursor, true);
            assert_eq!(first, second);
            saw_blink |= first.eyelids != EyelidPose::Open;
        }
        assert!(saw_blink);
    }

    #[test]
    fn reduced_motion_preserves_expression_while_softening_body_motion() {
        let genome = genome(BodyFamily::Hopper);
        let active = CreatureRenderer::render_body_frame(&genome, ActionKind::SoloPlay, 1, false);
        let reduced = CreatureRenderer::render_body_frame(&genome, ActionKind::SoloPlay, 1, true);
        assert_ne!(active.canvas, reduced.canvas);
        let state = FaceRenderState {
            expression: ExpressionKind::Joy,
            eyelids: EyelidPose::Open,
            gaze: GazeDirection::default(),
        };
        let joy = CreatureRenderer::render_face_frame(&genome, state);
        let neutral = CreatureRenderer::render_face_frame(
            &genome,
            FaceRenderState {
                expression: ExpressionKind::Neutral,
                ..state
            },
        );
        assert_ne!(joy, neutral);
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
    fn cat_gesture_paws_cap_upright_arm_length() {
        let mut compact = genome(BodyFamily::SoftQuadruped);
        compact.forelimbs.length = 4;
        let mut extreme = compact.clone();
        extreme.forelimbs.length = 7;
        for action in [
            ActionKind::Greet,
            ActionKind::SocialPlay,
            ActionKind::PresentDiscovery,
        ] {
            for frame in 0..AnimationSpec::for_action(action).frames {
                assert_eq!(
                    CreatureRenderer::render_body_frame(&compact, action, frame, false).canvas,
                    CreatureRenderer::render_body_frame(&extreme, action, frame, false).canvas,
                    "{action:?} frame {frame}"
                );
            }
        }
    }

    #[test]
    fn one_thousand_generated_genomes_render_every_action() {
        let desktop = DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: formiga_core::DisplayKey([1; 16]),
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
            assert!(match genome.family {
                BodyFamily::Blob => matches!(
                    genome.forelimbs.style,
                    ForelimbStyle::SoftNub | ForelimbStyle::Pseudopod
                ),
                BodyFamily::Hopper => genome.forelimbs.style == ForelimbStyle::MittenArm,
                BodyFamily::SoftQuadruped => genome.forelimbs.style == ForelimbStyle::FrontPaw,
            });
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
            let palette = PALETTES[genome.palette_index as usize % PALETTES.len()];
            for expression in ExpressionKind::ALL {
                let face = CreatureRenderer::render_face_frame(
                    genome,
                    FaceRenderState {
                        expression,
                        eyelids: EyelidPose::Open,
                        gaze: GazeDirection::default(),
                    },
                );
                let left_eye = face
                    .pixels()
                    .iter()
                    .enumerate()
                    .any(|(pixel_index, pixel)| {
                        pixel_index as u32 % FACE_FRAME_SIZE < FACE_FRAME_SIZE / 2
                            && *pixel == palette.eye
                    });
                let right_eye = face
                    .pixels()
                    .iter()
                    .enumerate()
                    .any(|(pixel_index, pixel)| {
                        pixel_index as u32 % FACE_FRAME_SIZE >= FACE_FRAME_SIZE / 2
                            && *pixel == palette.eye
                    });
                assert!(
                    left_eye && right_eye,
                    "seed {index}, {expression:?} loses its two-eye grammar"
                );
            }
        }
    }

    #[test]
    fn resting_baseline_seats_every_family_on_its_contact_point() {
        for family in [
            BodyFamily::Blob,
            BodyFamily::Hopper,
            BodyFamily::SoftQuadruped,
        ] {
            let genome = genome(family);
            let baseline = CreatureRenderer::resting_baseline(&genome, false);

            // Seating by the baseline must not push any resting pose through the surface.
            for action in [ActionKind::Idle, ActionKind::Perch, ActionKind::Homebound] {
                for frame in 0..AnimationSpec::for_action(action).frames {
                    let canvas =
                        CreatureRenderer::render_body_frame(&genome, action, frame, false).canvas;
                    let clearance = canvas
                        .alpha_bounds()
                        .map(|(_, _, _, max_y)| FRAME_SIZE - 1 - max_y)
                        .expect("resting pose draws pixels");
                    assert!(
                        clearance >= baseline,
                        "{family:?} {action:?} frame {frame} would sink {} px below its surface",
                        baseline - clearance,
                    );
                }
            }

            // And at least one resting pose has to land exactly on it, or the creature still
            // floats after seating.
            let seated = [ActionKind::Idle, ActionKind::Perch, ActionKind::Homebound]
                .into_iter()
                .flat_map(|action| {
                    (0..AnimationSpec::for_action(action).frames).map(move |frame| (action, frame))
                })
                .any(|(action, frame)| {
                    CreatureRenderer::render_body_frame(&genome, action, frame, false)
                        .canvas
                        .alpha_bounds()
                        .is_some_and(|(_, _, _, max_y)| FRAME_SIZE - 1 - max_y == baseline)
                });
            assert!(seated, "{family:?} never touches its contact point");
        }
    }

    const ALL_APPENDAGES: [HeadAppendageStyle; 6] = [
        HeadAppendageStyle::None,
        HeadAppendageStyle::Round,
        HeadAppendageStyle::Pointed,
        HeadAppendageStyle::Leaf,
        HeadAppendageStyle::Droop,
        HeadAppendageStyle::Antenna,
    ];

    const ALL_TAILS: [TailStyle; 5] = [
        TailStyle::None,
        TailStyle::Stub,
        TailStyle::Taper,
        TailStyle::Tuft,
        TailStyle::Curl,
    ];

    fn rest_pose() -> Pose {
        Pose::new(&genome(BodyFamily::Blob), ActionKind::Idle, 0, true)
    }

    /// Opaque pixels above `row`, split into those left and right of `center_x`.
    fn pixels_above(canvas: &Canvas, row: i32, center_x: i32) -> (usize, usize) {
        let mut left = 0;
        let mut right = 0;
        for y in 0..row.min(canvas.height() as i32) {
            for x in 0..canvas.width() as i32 {
                if canvas.get(x, y).a > 0 {
                    if x < center_x {
                        left += 1;
                    } else if x > center_x {
                        right += 1;
                    }
                }
            }
        }
        (left, right)
    }

    #[test]
    fn every_cat_genome_keeps_a_pair_of_ears_above_the_crown() {
        let palette = PALETTES[2];
        for style in ALL_APPENDAGES {
            let mut genome = genome(BodyFamily::SoftQuadruped);
            genome.head_appendages.style = style;
            for size in 2..=8 {
                let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
                draw_cat_ears(&mut canvas, palette, 24, 20, size, &genome, rest_pose());
                let (left, right) = pixels_above(&canvas, 20, 24);
                assert!(
                    left > 0 && right > 0,
                    "{style:?} size {size} draws {left}/{right} ear pixels above the crown",
                );
                let (min_x, min_y, max_x, max_y) = canvas.alpha_bounds().expect("ears are visible");
                assert!(
                    min_x >= 1 && min_y >= 1 && max_x < FRAME_SIZE - 1 && max_y < FRAME_SIZE - 1,
                    "{style:?} size {size} ears leave the frame margin",
                );
            }
        }
    }

    #[test]
    fn every_rabbit_genome_keeps_long_ears_inside_the_frame() {
        let palette = PALETTES[2];
        for style in ALL_APPENDAGES {
            let mut genome = genome(BodyFamily::Hopper);
            genome.head_appendages.style = style;
            for size in 2..=8 {
                let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
                draw_rabbit_ears(&mut canvas, palette, 24, 20, size, &genome, rest_pose());
                let (min_x, min_y, max_x, max_y) = canvas.alpha_bounds().expect("ears are visible");
                assert!(
                    min_x >= 1 && min_y >= 1 && max_x < FRAME_SIZE - 1 && max_y < FRAME_SIZE - 1,
                    "{style:?} size {size} ears leave the frame margin",
                );
                if style == HeadAppendageStyle::Droop {
                    // A lop set hangs beside the head instead of standing up.
                    assert!(max_y as i32 > 20, "{style:?} size {size} does not lop");
                    continue;
                }
                let (left, right) = pixels_above(&canvas, 20, 24);
                assert!(
                    left > 0 && right > 0,
                    "{style:?} size {size} draws {left}/{right} ear pixels above the head",
                );
                let reach = 20 - min_y as i32;
                assert!(
                    reach >= 5,
                    "{style:?} size {size} ears only reach {reach} px, too short to read as a rabbit",
                );
            }
        }
    }

    #[test]
    fn every_cat_tail_is_carried_above_the_rump() {
        let palette = PALETTES[2];
        for style in ALL_TAILS {
            let mut genome = genome(BodyFamily::SoftQuadruped);
            genome.tail_style = style;
            let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            draw_tail(&mut canvas, &genome, palette, 14, 26, 1.0, rest_pose());
            let (_, min_y, _, _) = canvas.alpha_bounds().expect("every cat carries a tail");
            assert!(
                (26 - min_y as i32) >= 4,
                "{style:?} tail rises only {} px off the rump",
                26 - min_y as i32,
            );
        }
    }

    #[test]
    fn every_rabbit_tail_puffs_behind_the_rump() {
        let palette = PALETTES[2];
        for style in ALL_TAILS {
            let mut genome = genome(BodyFamily::Hopper);
            genome.tail_style = style;
            let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            draw_tail(&mut canvas, &genome, palette, 14, 26, 1.0, rest_pose());
            let (min_x, _, _, _) = canvas.alpha_bounds().expect("every rabbit has a puff");
            assert!(min_x >= 1, "{style:?} puff leaves the frame margin");
            // The body ellipse is painted over everything from its left outline edge inwards, so
            // the puff only reads if it clears that edge by a few columns.
            let clear = (0..13)
                .filter(|x| (0..FRAME_SIZE as i32).any(|y| canvas.get(*x, y).a > 0))
                .count();
            assert!(
                clear >= 3,
                "{style:?} puff leaves only {clear} columns showing behind the rump",
            );
        }
    }

    #[test]
    fn a_resting_cat_plants_all_four_paws_on_its_contact_row() {
        let genome = genome(BodyFamily::SoftQuadruped);
        let walking = CreatureRenderer::render_body_frame(&genome, ActionKind::Traverse, 0, true)
            .canvas
            .alpha_bounds()
            .expect("a walking cat is visible")
            .3;
        for action in [
            ActionKind::Idle,
            ActionKind::Eat,
            ActionKind::Drink,
            ActionKind::ReactToWindow,
            ActionKind::InspectScreen,
        ] {
            let resting = CreatureRenderer::render_body_frame(&genome, action, 0, true)
                .canvas
                .alpha_bounds()
                .expect("a resting cat is visible")
                .3;
            assert_eq!(
                resting, walking,
                "{action:?} does not stand on its legs the way walking does",
            );
        }
    }
}
