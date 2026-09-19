#[cfg(test)]
use crate::PALETTES;
use crate::{Canvas, Palette, Rgba};
use formiga_core::{
    ActionKind, AppearanceGenome, BodyFamily, BrowStyle, CheekStyle, Creature, CursorSnapshot,
    EffectMotif, EyeShape, ForelimbStyle, Gesture, HeadAppendageStyle, HighlightStyle,
    LimbTipStyle, MouthStyle, PatternKind, PupilStyle, RestPose, TailStyle,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
mod modular;

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

/// One animated body clip baked into a creature atlas: an action's own motion, or a gesture pose
/// shown over whatever the creature is doing while its attention asks for one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BodyClip {
    Action(ActionKind),
    Gesture(Gesture),
}

impl From<ActionKind> for BodyClip {
    fn from(action: ActionKind) -> Self {
        Self::Action(action)
    }
}

impl From<Gesture> for BodyClip {
    fn from(gesture: Gesture) -> Self {
        Self::Gesture(gesture)
    }
}

impl BodyClip {
    /// What the creature's body shows right now. A gesture stands in for the action's clip only
    /// while the runtime attention pose carries one; the action still moves and places it.
    pub fn for_creature(creature: &Creature) -> Self {
        creature
            .state
            .attention
            .and_then(|pose| pose.gesture)
            .map_or(Self::Action(creature.state.action), Self::Gesture)
    }

    /// Every clip with frames of its own, in atlas order: the body actions, then the gestures.
    pub fn baked() -> impl Iterator<Item = Self> {
        ActionKind::BODY_CLIPS
            .into_iter()
            .map(Self::Action)
            .chain(Gesture::ALL.into_iter().map(Self::Gesture))
    }

    /// The clip whose frames are actually drawn, after actions that share a body are folded.
    pub const fn body(self) -> Self {
        match self {
            Self::Action(action) => Self::Action(AnimationSpec::body_action(action)),
            gesture => gesture,
        }
    }
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

    /// Frames and timing for any baked clip. Gestures loop: they are held for as long as the
    /// moment lasts, and the attention system decides when that is.
    pub fn for_clip(clip: impl Into<BodyClip>) -> Self {
        let gesture = match clip.into() {
            BodyClip::Action(action) => return Self::for_action(action),
            BodyClip::Gesture(gesture) => gesture,
        };
        let (frames, fps) = match gesture {
            Gesture::Cheer => (4, 8),
            Gesture::Gasp => (2, 6),
            Gesture::Cover => (2, 3),
            Gesture::Worry => (4, 4),
            Gesture::Crouch => (2, 3),
            Gesture::Heave => (4, 5),
            Gesture::Balance => (4, 5),
            Gesture::Reach => (2, 4),
            Gesture::Bop => (4, 6),
            // Held interest, not a beat: six frames at three a second is a two-second breath,
            // slow enough that the tilt and the ear twitch read as one creature paying attention
            // rather than as a body doing something.
            Gesture::Watch => (6, 3),
        };
        Self {
            frames,
            fps,
            playback: PlaybackMode::Loop,
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

/// Stable per-creature timing. The same creature always walks, rests, greets, and recovers with
/// its own cadence and phase, from data that already exists: its identity and personality. Nothing
/// here is stored, no frame is added, and one-shot clips keep their exact timing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionSignature {
    tempo: [f32; 4],
    phase: [u8; 4],
}

/// Walking, resting, greeting, and recovery share one small vocabulary of timings. Gestures are
/// expressive, so they keep the creature's greeting tempo.
fn motion_group(clip: BodyClip) -> usize {
    let action = match clip {
        BodyClip::Action(action) => action,
        BodyClip::Gesture(_) => return 2,
    };
    match action {
        ActionKind::Traverse
        | ActionKind::Follow
        | ActionKind::Sprint
        | ActionKind::SqueezeWindow
        | ActionKind::Homebound => 0,
        ActionKind::Idle | ActionKind::Perch | ActionKind::RideWindow | ActionKind::Sleep => 1,
        ActionKind::Greet
        | ActionKind::SocialPlay
        | ActionKind::SoloPlay
        | ActionKind::PetReaction => 2,
        _ => 3,
    }
}

impl MotionSignature {
    pub fn for_creature(creature: &Creature) -> Self {
        // One stable stream per identity, mixed with traits the creature already has.
        let mut state = creature.id ^ 0x9e37_79b9_7f4a_7c15;
        let mut next = move || {
            state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        };
        let unit = |value: u64| (value & 0xff) as f32 / 255.0;
        let personality = &creature.personality;
        let tempo = [
            0.85 + personality.activity * 0.3 + unit(next()) * 0.1,
            0.8 + unit(next()) * 0.25 + personality.activity * 0.1,
            0.9 + personality.playfulness * 0.3 + unit(next()) * 0.1,
            0.9 + unit(next()) * 0.2,
        ];
        let phase = std::array::from_fn(|_| (next() & 0x7) as u8);
        Self { tempo, phase }
    }

    /// The body frame this creature shows, in place of the shared `AnimationSpec::frame_at`.
    pub fn frame(self, clip: impl Into<BodyClip>, elapsed: f32) -> u8 {
        let clip = clip.into();
        let spec = AnimationSpec::for_clip(clip);
        if spec.playback == PlaybackMode::Hold {
            return spec.frame_at(elapsed);
        }
        let group = motion_group(clip);
        let advanced = elapsed.max(0.0) * self.tempo[group] * f32::from(spec.fps);
        ((advanced as u32).wrapping_add(u32::from(self.phase[group])) % u32::from(spec.frames))
            as u8
    }
}

/// Where a carried thing rides, relative to the creature's own face anchor, in art pixels.
///
/// A held object belongs in front of the creature at chest height, on the side it is facing, so
/// that a prop changing hands reads as one creature passing something to another rather than two
/// objects swapping places in mid-air. The anchor mirrors with facing for exactly that reason,
/// and stays inside the frame so a prop is clipped and occluded with its holder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropAnchor {
    pub dx: f32,
    pub dy: f32,
}

impl PropAnchor {
    /// Forward of the face and the same distance below it, where a companion's forelimbs meet in
    /// front of its chest — the same place an eaten snack or a played-with toy is drawn inside
    /// the frame. The prop quad is the only one drawn over the layered face, so it has to clear
    /// the eyes rather than merely miss the frame edge: lifting it instead of dropping it hangs
    /// the thing in the air above the crown and pushes its quad out through the top of the frame
    /// on the taller body plans.
    const FORWARD: f32 = 8.0;
    const DROP: f32 = 8.0;

    pub fn for_creature(creature: &Creature) -> Self {
        Self::facing(creature.state.facing_right)
    }

    pub fn facing(facing_right: bool) -> Self {
        Self {
            dx: if facing_right {
                Self::FORWARD
            } else {
                -Self::FORWARD
            },
            dy: Self::DROP,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FramePlacement {
    /// Top of the 48x48 body frame relative to the simulation contact point, in art pixels.
    pub origin_y: i32,
}

impl FramePlacement {
    pub fn for_creature(creature: &Creature, resting_baseline: u32) -> Self {
        if let Some(pose) = creature.state.attention {
            let standing = resting_baseline as f32 - FRAME_SIZE as f32;
            Self {
                origin_y: (standing + (-7.0 - standing) * pose.hanging.clamp(0.0, 1.0)).round()
                    as i32,
            }
        } else {
            Self::for_action(creature.state.action, resting_baseline)
        }
    }

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
        clip: impl Into<BodyClip>,
        frame: u8,
        reduce_motion: bool,
    ) -> RenderedBodyFrame {
        let frame = if reduce_motion { 0 } else { frame };
        let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        let palette = crate::palette_for(genome);
        let clip = clip.into().body();
        let pose = Pose::new(genome, clip, frame, reduce_motion);
        let mut face_anchor = if let Some(design) = genome.design {
            modular::draw(
                &mut canvas,
                design,
                palette,
                pose,
                scale(genome),
                clip,
                frame,
            )
        } else {
            match genome.family {
                BodyFamily::Blob => draw_blob(&mut canvas, genome, palette, pose, clip, frame),
                BodyFamily::Hopper => draw_hopper(&mut canvas, genome, palette, pose, clip, frame),
                BodyFamily::SoftQuadruped => {
                    draw_quadruped(&mut canvas, genome, palette, pose, clip, frame)
                }
            }
        };
        match clip {
            BodyClip::Action(action) => {
                draw_activity_prop(
                    &mut canvas,
                    genome,
                    palette,
                    face_anchor,
                    pose,
                    action,
                    frame,
                    reduce_motion,
                );
                draw_effects(
                    &mut canvas,
                    genome,
                    palette,
                    face_anchor,
                    action,
                    frame,
                    reduce_motion,
                );
            }
            BodyClip::Gesture(gesture) => draw_gesture_effects(
                &mut canvas,
                genome,
                palette,
                face_anchor,
                gesture,
                frame,
                reduce_motion,
            ),
        }
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
        let palette = crate::palette_for(genome);
        draw_face(&mut canvas, genome, palette, 8, 7, state);
        canvas
    }

    /// One trinket at the size the prop quad samples, coloured against this creature alone. The
    /// desktop draws found things from the colony atlas instead; this is for the single-sprite
    /// paths — the review sheets, the contact sheet, and the slots kept in the face texture.
    pub fn render_trinket(genome: &AppearanceGenome, variant: u8) -> Canvas {
        let mut canvas = Canvas::new(FACE_FRAME_SIZE, FACE_FRAME_SIZE);
        draw_generated_trinket(
            &mut canvas,
            crate::palette_for(genome),
            variant,
            genome.marking_seed,
        );
        canvas
    }

    /// Draw a soft edge in the transparent pixels immediately around a creature, so it stays
    /// readable on bright or busy wallpaper. This reads only the frame's own alpha — never the
    /// desktop behind it — and writes only inside the existing frame, so the silhouette used for
    /// clicking and dragging is unchanged. Edges stay one pixel and hard, never blurred.
    pub fn outline_frame(canvas: &mut Canvas) {
        let (width, height) = (canvas.width() as i32, canvas.height() as i32);
        let solid = |canvas: &Canvas, x: i32, y: i32| canvas.get(x, y).a > 16;
        let mut edges = Vec::new();
        for y in 0..height {
            for x in 0..width {
                if solid(canvas, x, y) {
                    continue;
                }
                // A cardinal neighbour makes a full edge pixel; a diagonal one only softens the
                // corner, which is what keeps a pixel silhouette from growing a halo.
                let cardinal = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .into_iter()
                    .any(|(dx, dy)| solid(canvas, x + dx, y + dy));
                let diagonal = [(1, 1), (1, -1), (-1, 1), (-1, -1)]
                    .into_iter()
                    .any(|(dx, dy)| solid(canvas, x + dx, y + dy));
                if cardinal || diagonal {
                    edges.push((x, y, if cardinal { 150 } else { 70 }));
                }
            }
        }
        for (x, y, alpha) in edges {
            canvas.set(x, y, Rgba::new(18, 26, 22, alpha));
        }
    }

    pub fn render_composited_frame(
        genome: &AppearanceGenome,
        clip: impl Into<BodyClip>,
        frame: u8,
        facing_right: bool,
        reduce_motion: bool,
        face_state: FaceRenderState,
    ) -> Canvas {
        let mut body = Self::render_body_frame(genome, clip, frame, reduce_motion);
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

#[derive(Clone, Copy, Default)]
struct Pose {
    bob: i32,
    squash_x: i32,
    squash_y: i32,
    step_a: i32,
    step_b: i32,
    play_lift: i32,
    appendage_lift: i32,
    tail_sway: i32,
    /// Head carried forward (positive) or back over the body, in art pixels.
    ///
    /// A modular body moves its whole head oval; the older family bodies have no separate head,
    /// so they carry the ears and the reserved face across the mass they already drew. Either way
    /// the feet stay where they are, which is what makes it read as leaning rather than stepping.
    lean: i32,
    /// Legs folded under the body: it sinks while the feet stay planted.
    crouch: i32,
    /// Ears, antennae or tufts pricked up past their resting height, in art pixels.
    ///
    /// Separate from `appendage_lift`, which lifts the shoulders: this is the pair on top of the
    /// head, and it grows the ear from its base rather than moving it, so a pricked ear stays
    /// attached to the head it grew on. Only a pose that is listening as well as looking asks
    /// for it, so no existing clip changes shape.
    ear_perk: i32,
}

impl Pose {
    fn new(genome: &AppearanceGenome, clip: BodyClip, frame: u8, reduce_motion: bool) -> Self {
        let action = match clip {
            BodyClip::Action(action) => action,
            BodyClip::Gesture(gesture) => {
                let mut pose = Self::for_gesture(gesture, frame);
                if reduce_motion {
                    pose.calm();
                }
                return pose;
            }
        };
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
                ..Self::default()
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
                ..Self::default()
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
                ..Self::default()
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
                ..Self::default()
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
                ..Self::default()
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
                ..Self::default()
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
                ..Self::default()
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
                ..Self::default()
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
                ..Self::default()
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
                ..Self::default()
            },
            // The plain peek a creature does while it works out what it is looking at. Its own
            // clip still reads, so it keeps it; it only gains a lean toward the thing, so that
            // even the unposed half of window attention is pointed at something.
            ActionKind::InspectScreen => Self {
                bob: i32::from(frame % 2),
                squash_x: 1,
                squash_y: -1,
                step_a: 0,
                step_b: -1,
                play_lift: 1,
                appendage_lift: 1 + walk.abs(),
                tail_sway: alternate,
                lean: 1,
                ..Self::default()
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
                ..Self::default()
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
                ..Self::default()
            },
        };
        if reduce_motion {
            pose.calm();
        }
        pose
    }

    /// The body half of each gesture: squash, lift, lean and footing. The limbs are placed by
    /// each renderer, since where a paw can reach depends on the body it belongs to.
    fn for_gesture(gesture: Gesture, frame: u8) -> Self {
        let tick = i32::from(frame % 2);
        let beat = [0, 1, 0, -1][usize::from(frame % 4)];
        // The watching loop is six frames long, so it gets its own slow counters rather than
        // borrowing the two- and four-frame ones every other pose shares.
        let slow = usize::from(frame % 6);
        match gesture {
            // A hop on every other beat, landing squashed and springing up stretched.
            Gesture::Cheer => {
                let hop = [0, 2, 3, 1][usize::from(frame % 4)];
                Self {
                    squash_x: i32::from(hop == 0),
                    squash_y: i32::from(hop == 3) - i32::from(hop == 0),
                    play_lift: hop,
                    appendage_lift: 2,
                    tail_sway: beat * 2,
                    ..Self::default()
                }
            }
            // Drawn up tall and rocked back, with a shiver between the two frames.
            Gesture::Gasp => Self {
                squash_x: -1,
                squash_y: 1,
                play_lift: 1 - tick,
                appendage_lift: 3,
                tail_sway: -2,
                lean: -1,
                ..Self::default()
            },
            // Hunched small, peeking on the second frame.
            Gesture::Cover => Self {
                bob: 1,
                squash_x: 1,
                squash_y: -1,
                appendage_lift: 1,
                tail_sway: -1,
                crouch: 1,
                ..Self::default()
            },
            // A fidgeting shift from foot to foot.
            Gesture::Worry => Self {
                bob: tick,
                step_a: beat,
                step_b: -beat,
                appendage_lift: 1,
                tail_sway: beat,
                ..Self::default()
            },
            // Low and wound tight, breathing.
            Gesture::Crouch => Self {
                squash_x: 2,
                squash_y: -1 - tick,
                step_a: -1,
                step_b: 1,
                appendage_lift: -1,
                tail_sway: 1 - tick,
                crouch: 3,
                ..Self::default()
            },
            // Braced and leaning back, the pull coming in waves.
            Gesture::Heave => Self {
                squash_x: 1,
                squash_y: -1,
                step_a: -2,
                step_b: 1,
                appendage_lift: 1,
                tail_sway: -1,
                lean: -1 - beat.abs(),
                crouch: 1,
                ..Self::default()
            },
            // Teetering from side to side, a foot lifting with each tip.
            Gesture::Balance => Self {
                step_a: beat,
                step_b: -beat,
                appendage_lift: 2,
                tail_sway: -beat * 2,
                lean: beat,
                ..Self::default()
            },
            // Up on tiptoe and stretched toward the thing.
            Gesture::Reach => Self {
                squash_x: -1,
                squash_y: 1,
                play_lift: 1,
                appendage_lift: 1,
                tail_sway: 1,
                lean: 1 + tick,
                ..Self::default()
            },
            // Swaying on the beat, stepping in time.
            Gesture::Bop => Self {
                bob: tick,
                step_a: beat,
                step_b: -beat,
                appendage_lift: 1,
                tail_sway: beat * 2,
                lean: beat,
                ..Self::default()
            },
            // Settled forward over a planted, staggered stance with the ears up: the weight stays
            // put, the head tilts out and back across the loop, and one ear drops for a single
            // frame. Everything here is small on purpose — a watching creature is nearly still,
            // and the pose has to hold for as long as the thing is worth watching.
            Gesture::Watch => Self {
                bob: 0,
                // Drawn in narrower and up taller, which also carries the head higher: attention
                // gathers a body rather than spreading it, and the lifted head is half of what
                // says the creature is looking at something rather than standing about.
                squash_x: -1,
                squash_y: 2,
                // Front foot forward, back foot braced: a stance already turned to the thing.
                step_a: -1,
                step_b: 2,
                play_lift: 0,
                // Shoulders held high and tight, dropping once in the middle of the loop.
                appendage_lift: [3, 3, 3, 1, 3, 3][slow],
                // The tail drifts rather than swings.
                tail_sway: [0, 1, 1, 1, 0, 0][slow],
                // The head carried out over the forward foot, easing further and settling back.
                lean: [2, 3, 3, 3, 2, 2][slow],
                crouch: 0,
                // Ears up the whole time, with a single flick on the fourth frame.
                ear_perk: [2, 2, 2, 1, 2, 2][slow],
            },
        }
    }

    /// Reduced motion keeps each pose's shape but drops its travel.
    fn calm(&mut self) {
        self.bob = 0;
        self.squash_x = 0;
        self.squash_y = 0;
        self.play_lift = 0;
        self.appendage_lift = self.appendage_lift.clamp(-1, 1);
        self.tail_sway = self.tail_sway.clamp(-1, 1);
        self.step_a /= 2;
        self.step_b /= 2;
        self.lean = self.lean.clamp(-1, 1);
        self.crouch = self.crouch.min(1);
        // A pricked ear is shape rather than travel, so it survives at half height: the pose still
        // reads as listening, and it holds that one shape without twitching.
        self.ear_perk = self.ear_perk.clamp(0, 1);
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
    clip: BodyClip,
    frame: u8,
) -> PixelPoint {
    let s = scale(genome);
    let rx = ((genome.body_width as f32 * s / 2.0).round() as i32 + pose.squash_x).clamp(6, 16);
    let ry = ((genome.body_height as f32 * s / 2.0).round() as i32 + pose.squash_y).clamp(5, 14);
    let cx = 26;
    let cy = 38 - ry + pose.bob - pose.play_lift;
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
    PixelPoint {
        x: cx + 2 + lean,
        y: cy - 1,
    }
}

fn draw_hopper(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    pose: Pose,
    clip: BodyClip,
    frame: u8,
) -> PixelPoint {
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
    PixelPoint {
        x: cx + 1 + lean,
        y: cy - 2,
    }
}

fn draw_quadruped(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    pose: Pose,
    clip: BodyClip,
    frame: u8,
) -> PixelPoint {
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
    let mut face = genome.face;
    if genome.design.is_some() {
        face.eye_size = 2;
        face.eye_spacing = 6;
        face.vertical_offset = -1;
        face.eye_shape = EyeShape::Round;
    }
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
    clip: BodyClip,
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

fn draw_quadruped_forelimbs(
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

fn limb_targets(
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
fn gesture_limb_targets(
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

/// How many kinds of each belonging the generator can produce. Every one is resolved from genes
/// the appearance genome already stores, so nothing is added to a save: an existing companion
/// simply reaches for a clearer toy from a larger shelf.
pub const TOY_KINDS: u8 = 8;
pub const SNACK_KINDS: u8 = 4;
pub const DRINK_KINDS: u8 = 3;

/// The one number every belonging is resolved from.
fn prop_signature(genome: &AppearanceGenome) -> u64 {
    genome.marking_seed ^ u64::from(genome.face_signature)
}

/// Which toy, snack, and cup one appearance owns. Stable for the life of a creature.
pub fn prop_variants(genome: &AppearanceGenome) -> (u8, u8, u8) {
    let signature = prop_signature(genome);
    (
        (signature % u64::from(TOY_KINDS)) as u8,
        ((signature >> 7) % u64::from(SNACK_KINDS)) as u8,
        ((signature >> 13) % u64::from(DRINK_KINDS)) as u8,
    )
}

/// Where a body holds what it is using. Props are placed from these two points rather than from an
/// offset guessed against the face, so a toy lands in the paw that is actually drawn and a snack
/// arrives at the mouth that is eating it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PropHold {
    pub(crate) hands: PixelPoint,
    pub(crate) mouth: PixelPoint,
    pub(crate) floor: i32,
    /// Which way is away from this body's own face: `1` for a body whose paws are in front of its
    /// head, `-1` for one whose head leads and whose chest trails behind it.
    pub(crate) forward: i32,
}

fn prop_hold(genome: &AppearanceGenome, pose: Pose, face: PixelPoint) -> PropHold {
    if let Some(design) = genome.design {
        return modular::prop_hold(design, pose, scale(genome));
    }
    // The original families carry their face on the body itself, so a belonging is placed out to
    // the side of the mass rather than in front of a separate head.
    let (dx, dy) = match genome.family {
        BodyFamily::Blob => (12, 4),
        BodyFamily::Hopper => (12, 5),
        // A soft quadruped's head is the anchor and its chest is behind it.
        BodyFamily::SoftQuadruped => (-12, 7),
    };
    PropHold {
        hands: PixelPoint {
            x: face.x + dx,
            y: face.y + dy,
        },
        mouth: PixelPoint {
            x: face.x + 6,
            y: face.y + 5,
        },
        floor: 42,
        forward: dx.signum(),
    }
}

/// One round of play, one mouthful, or one sip — placed where the body can reach it.
#[allow(clippy::too_many_arguments)]
fn draw_activity_prop(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    face: PixelPoint,
    pose: Pose,
    action: ActionKind,
    frame: u8,
    reduce_motion: bool,
) {
    let phase = if reduce_motion { 0 } else { frame % 4 };
    let (toy, snack, drink) = prop_variants(genome);
    // Belongings are coloured against the creature, not from it, so a held toy stays readable.
    let palette = crate::prop_palette(palette, prop_signature(genome));
    let hold = prop_hold(genome, pose, face);
    match action {
        ActionKind::SoloPlay => {
            // One round of keep-up, read straight off the paw: knocked up off the paw, over the
            // head, caught coming down, and back onto the paw. The toy is the thing that moves,
            // and it passes through the paw twice a loop, so play reads as play even on a body
            // whose forelimbs stay folded.
            let at = |phase: u8| play_path(hold, phase);
            let (x, y, spin) = at(phase);
            if !reduce_motion {
                let (px, py, _) = at((phase + 3) % 4);
                draw_prop_trail(canvas, palette, (px, py), (x, y));
            }
            draw_generated_toy(canvas, palette, genome.effect_motif, toy, x, y, spin);
            if phase == 0 || phase == 3 {
                // The knock itself: two ticks where the toy meets the paw that is keeping it up.
                for (dx, dy) in [(-1, 3), (1, 4)] {
                    canvas.set(x + dx * hold.forward, y + dy, palette.highlight);
                }
            }
        }
        ActionKind::Eat => {
            // Carried up to the mouth and taken down to a crumb.
            let lift = [0, 2, 3, 1][usize::from(phase)];
            let x = hold.mouth.x - 1 + i32::from(phase == 0) * 2;
            let y = hold.mouth.y + 2 - lift;
            draw_generated_snack(canvas, palette, snack, x, y, phase);
            if phase == 3 {
                // Crumbs, so the last frame is the end of a mouthful rather than an empty hand.
                canvas.set(hold.mouth.x - 2, hold.mouth.y + 5, palette.shadow);
                canvas.set(hold.mouth.x + 1, hold.mouth.y + 6, palette.highlight);
            }
        }
        ActionKind::Drink => {
            let tipped = phase == 1 || phase == 2;
            let x = hold.mouth.x - 1;
            let y = hold.mouth.y + 4 - i32::from(tipped) * 3;
            draw_generated_drinkware(canvas, palette, drink, x, y, tipped, phase);
        }
        _ => {}
    }
}

/// Where the toy is on each beat of the play loop, and how far it has turned by then.
fn play_path(hold: PropHold, phase: u8) -> (i32, i32, u8) {
    let (hx, hy) = (hold.hands.x, hold.hands.y);
    let (dx, dy, spin) = match phase {
        // Resting on the paw.
        0 => (0, -1, 0),
        // Knocked up and away from the body.
        1 => (3, -5, 1),
        // The top of its arc: clear of the creature, never over its face, and never so high that
        // the paw it came off has visibly let go of it.
        2 => (5, -8, 2),
        // Coming back down onto the paw.
        _ => (1, -4, 3),
    };
    let dx = dx * hold.forward;
    // Nothing sinks into the surface underfoot, and the widest toy still keeps the one-pixel
    // margin the atlas needs without the whole sprite being shifted to make room for it.
    (
        (hx + dx).clamp(8, FRAME_SIZE as i32 - 9),
        (hy + dy)
            .min(hold.floor - 4)
            .clamp(8, FRAME_SIZE as i32 - 9),
        spin,
    )
}

/// Two short dashes along the way the toy has just come. Nothing here is animated on its own: the
/// marks are simply drawn between this beat's position and the last one's.
fn draw_prop_trail(canvas: &mut Canvas, palette: Palette, from: (i32, i32), to: (i32, i32)) {
    for step in [2, 4, 6] {
        let x = from.0 + (to.0 - from.0) * step / 8;
        let y = from.1 + (to.1 - from.1) * step / 8;
        canvas.set(x, y, palette.outline);
        canvas.set(x + 1, y, palette.highlight);
    }
}

/// A shape with the shared dark edge every belonging carries, so nothing melts into a coat.
fn prop_blob(canvas: &mut Canvas, palette: Palette, x: i32, y: i32, rx: i32, ry: i32, fill: Rgba) {
    canvas.fill_ellipse(x, y, rx + 1, ry + 1, palette.outline);
    canvas.fill_ellipse(x, y, rx, ry, fill);
}

fn prop_box(canvas: &mut Canvas, palette: Palette, x: i32, y: i32, rx: i32, ry: i32, fill: Rgba) {
    canvas.fill_rect(
        x - rx - 1,
        y - ry - 1,
        rx * 2 + 3,
        ry * 2 + 3,
        palette.outline,
    );
    canvas.fill_rect(x - rx, y - ry, rx * 2 + 1, ry * 2 + 1, fill);
}

/// Eight playthings, each with a silhouette that survives being shrunk to 2x on a busy desktop:
/// a ball, a spinning top, a plush, a block, a yo-yo, a rattle, a pinwheel, and a hoop.
fn draw_generated_toy(
    canvas: &mut Canvas,
    palette: Palette,
    motif: EffectMotif,
    variant: u8,
    x: i32,
    y: i32,
    spin: u8,
) {
    let tilt = [0, 1, 0, -1][usize::from(spin % 4)];
    match variant % TOY_KINDS {
        // Ball, with a seam that turns as it flies.
        0 => {
            prop_blob(canvas, palette, x, y, 4, 4, palette.accent);
            match spin % 4 {
                0 => canvas.line(x, y - 3, x, y + 3, 1, palette.shadow),
                1 => canvas.line(x - 3, y - 2, x + 3, y + 2, 1, palette.shadow),
                2 => canvas.line(x - 3, y, x + 3, y, 1, palette.shadow),
                _ => canvas.line(x - 3, y + 2, x + 3, y - 2, 1, palette.shadow),
            }
            canvas.set(x - 2, y - 2, palette.highlight);
            canvas.set(x - 1, y - 3, palette.highlight);
        }
        // Spinning top: a broad disc on a point, leaning further over the faster it goes.
        1 => {
            canvas.fill_ellipse(x + tilt, y - 1, 6, 4, palette.outline);
            canvas.fill_ellipse(x + tilt, y - 1, 5, 3, palette.accent);
            canvas.line(x + tilt, y + 1, x + tilt * 2, y + 5, 3, palette.outline);
            canvas.line(x + tilt, y + 1, x + tilt * 2, y + 5, 1, palette.highlight);
            canvas.fill_rect(x + tilt - 2, y - 6, 5, 3, palette.outline);
            canvas.fill_rect(x + tilt - 1, y - 5, 3, 2, palette.highlight);
            canvas.line(x + tilt - 4, y - 1, x + tilt + 4, y - 2, 1, palette.shadow);
        }
        // Plush, with its own two ears and two stitched eyes.
        2 => {
            for side in [-1, 1] {
                prop_blob(canvas, palette, x + side * 3, y - 4, 2, 2, palette.accent);
            }
            prop_blob(canvas, palette, x, y + 1, 4, 4, palette.accent);
            canvas.set(x - 2, y - 1, palette.outline);
            canvas.set(x + 2, y - 1, palette.outline);
            canvas.line(x - 1, y + 2, x + 1, y + 2, 1, palette.shadow);
            canvas.set(x - 3, y - 1, palette.highlight);
        }
        // Building block, with a mark cut into its face.
        3 => {
            prop_box(canvas, palette, x, y, 4, 4, palette.accent);
            canvas.fill_rect(x - 4, y - 4, 9, 1, palette.highlight);
            canvas.fill_rect(x - 4, y - 4, 1, 9, palette.highlight);
            canvas.fill_rect(x - 2, y - 2, 5, 5, palette.shadow);
            canvas.fill_rect(x - 1, y - 1, 3, 3, palette.accent);
        }
        // Yo-yo, on a string from the paw above it.
        4 => {
            canvas.line(x, y - 10, x, y - 4, 1, palette.outline);
            prop_blob(canvas, palette, x, y, 4, 4, palette.accent);
            canvas.fill_rect(x - 4, y - 1, 9, 2, palette.shadow);
            canvas.set(x - 2, y - 2, palette.highlight);
        }
        // Rattle: a bulb on a handle, shaking the way it is carried.
        5 => {
            canvas.line(x - tilt, y + 1, x - tilt * 2, y + 7, 3, palette.outline);
            canvas.line(x - tilt, y + 1, x - tilt * 2, y + 7, 1, palette.highlight);
            prop_blob(canvas, palette, x, y - 3, 5, 5, palette.accent);
            canvas.fill_rect(x - 2, y - 4, 4, 2, palette.shadow);
            canvas.set(x - 3, y - 5, palette.highlight);
            canvas.set(x - 2, y - 6, palette.highlight);
        }
        // Pinwheel: four sails on a stick, turning with the beat.
        6 => {
            canvas.line(x, y, x, y + 7, 3, palette.outline);
            canvas.line(x, y, x, y + 7, 1, palette.highlight);
            let mut sail = [
                (0, -1),
                (0, -2),
                (0, -3),
                (0, -4),
                (1, -2),
                (1, -3),
                (1, -4),
                (2, -3),
                (2, -4),
                (3, -4),
            ];
            for turn in 0..4 {
                let fill = if (turn + spin).is_multiple_of(2) {
                    palette.accent
                } else {
                    palette.shadow
                };
                for (dx, dy) in sail {
                    canvas.fill_circle(x + dx, y + dy, 1, palette.outline);
                }
                for (dx, dy) in sail {
                    canvas.set(x + dx, y + dy, fill);
                }
                sail = sail.map(|(dx, dy)| (-dy, dx));
            }
            canvas.fill_circle(x, y, 1, palette.outline);
            canvas.set(x, y, palette.highlight);
        }
        // Hoop, wide enough to be a hoop rather than a bead.
        7 => {
            canvas.fill_ellipse(x, y, 5, 5, palette.outline);
            canvas.fill_ellipse(x, y, 4, 4, palette.accent);
            canvas.fill_ellipse(x, y, 2, 2, palette.shadow);
            canvas.set(x - 3, y - 2, palette.highlight);
            canvas.set(x - 2, y - 3, palette.highlight);
        }
        // Unreachable while TOY_KINDS is 8; kept so a larger shelf never draws nothing.
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

/// Four things worth eating, each one visibly smaller by the mouthful.
fn draw_generated_snack(
    canvas: &mut Canvas,
    palette: Palette,
    variant: u8,
    x: i32,
    y: i32,
    phase: u8,
) {
    // Whole, bitten, half gone, and down to the last of it.
    let left = [3, 3, 2, 1][usize::from(phase % 4)];
    match variant % SNACK_KINDS {
        // A berry on its stem.
        0 => {
            canvas.line(x, y - left - 1, x + 2, y - left - 3, 1, palette.shadow);
            prop_blob(canvas, palette, x, y, left, left, palette.accent);
            canvas.set(x - 1, y - 1, palette.highlight);
        }
        // A biscuit with seeds in it.
        1 => {
            prop_box(canvas, palette, x, y, left, left - 1, palette.accent);
            for (dx, dy) in [(-1, -1), (1, 0), (0, 1)] {
                canvas.set(x + dx, y + dy, palette.shadow);
            }
            canvas.fill_rect(x - left, y - left + 1, left, 1, palette.highlight);
        }
        // A seed, pointed at one end.
        2 => {
            prop_blob(canvas, palette, x, y, left - 1, left, palette.accent);
            canvas.line(x, y - left, x + 1, y - left - 2, 1, palette.outline);
            canvas.set(x - 1, y, palette.highlight);
        }
        // A slice, rind side down.
        _ => {
            canvas.fill_ellipse(x, y, left + 1, left, palette.outline);
            canvas.fill_ellipse(x, y, left, left - 1, palette.accent);
            canvas.fill_rect(x - left, y + left - 1, left * 2 + 1, 1, palette.shadow);
            canvas.set(x - 1, y - 1, palette.highlight);
        }
    }
    if phase > 0 {
        // The bite taken out of it, on the side the mouth is.
        canvas.fill_circle(x + left, y - 1, 1, Rgba::TRANSPARENT);
    }
}

/// Three things to drink from, each tipped toward the mouth on the swallow.
fn draw_generated_drinkware(
    canvas: &mut Canvas,
    palette: Palette,
    variant: u8,
    x: i32,
    y: i32,
    tipped: bool,
    phase: u8,
) {
    let lean = i32::from(tipped);
    let x = x + lean;
    match variant % DRINK_KINDS {
        // A tall mug with a C handle on the side, steaming while it rests.
        0 => {
            prop_box(canvas, palette, x - 1, y, 3, 4, palette.highlight);
            canvas.fill_rect(x - 4, y - 4 + lean * 2, 7, 2, palette.accent);
            canvas.fill_ellipse(x + 4, y + 1, 3, 3, palette.outline);
            canvas.fill_ellipse(x + 4, y + 1, 2, 2, Rgba::TRANSPARENT);
            canvas.fill_rect(x + 1, y - 1, 3, 5, palette.highlight);
            if !tipped {
                for (dx, dy) in [(-1, -7), (0, -9), (1, -11)] {
                    canvas.set(x + dx, y + dy, palette.highlight);
                }
            }
        }
        // A wide shallow bowl, with the surface tilting as it is lifted.
        1 => {
            canvas.fill_ellipse(x, y, 6, 4, palette.outline);
            canvas.fill_rect(x - 6, y - 4, 13, 4, Rgba::TRANSPARENT);
            canvas.fill_ellipse(x, y, 5, 3, palette.highlight);
            canvas.fill_rect(x - 5, y - 3, 11, 3, Rgba::TRANSPARENT);
            canvas.fill_rect(x - 5, y - 1 - lean, 11, 2, palette.accent);
            canvas.fill_rect(x - 6, y - 2, 13, 1, palette.outline);
            canvas.fill_rect(x - 5, y - 2, 4, 1, palette.highlight);
        }
        // A narrow bottle with a cap and a bent straw standing out of it.
        _ => {
            prop_box(canvas, palette, x, y + 1, 2, 3, palette.highlight);
            canvas.fill_rect(x - 2, y + 1, 5, 4, palette.accent);
            canvas.fill_rect(x - 2, y - 4, 5, 2, palette.outline);
            canvas.fill_rect(x - 1, y - 3, 3, 1, palette.highlight);
            canvas.fill_rect(x - 1, y - 6, 3, 2, palette.outline);
            // The straw leans a little further out on alternate beats, so a sip reads as a sip.
            let bend = 2 + i32::from(phase % 2);
            canvas.line(x + 1, y - 5, x + bend, y - 10, 2, palette.outline);
            canvas.line(x + 1, y - 5, x + bend, y - 10, 1, palette.accent);
        }
    }
}

/// One found thing, drawn from the shared catalogue so a keepsake looks the same wherever it is
/// shown — the scrapbook, a review sheet, or a companion holding it up.
fn draw_generated_trinket(canvas: &mut Canvas, palette: Palette, variant: u8, detail_seed: u64) {
    let ink = crate::trinkets::ink_against(palette, detail_seed ^ u64::from(variant));
    crate::draw_trinket(canvas, ink, variant, crate::TRINKET_FRAME_REST, 0, 0);
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

/// Small marks that finish a gesture, kept clear of the face and the raised limbs: a burst over
/// a cheer, startle lines for a gasp, a bead of worry, a note for a dance, puffs of effort.
fn draw_gesture_effects(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    face: PixelPoint,
    gesture: Gesture,
    frame: u8,
    reduce_motion: bool,
) {
    let pulse = if reduce_motion {
        0
    } else {
        i32::from(frame % 2)
    };
    match gesture {
        Gesture::Cheer => draw_motif(
            canvas,
            if genome.effect_motif == EffectMotif::None {
                EffectMotif::Spark
            } else {
                genome.effect_motif
            },
            face.x,
            (face.y - 15 - pulse).max(3),
            palette.highlight,
        ),
        Gesture::Gasp => {
            let y = (face.y - 13).max(4);
            canvas.line(face.x - 4, y, face.x - 5, y - 2, 1, palette.accent);
            canvas.line(face.x, y - 1, face.x, y - 3, 1, palette.accent);
            canvas.line(face.x + 4, y, face.x + 5, y - 2, 1, palette.accent);
        }
        Gesture::Worry => {
            let x = (face.x + 8).min(44);
            let y = (face.y - 6 + pulse).clamp(3, 41);
            canvas.set(x, y, palette.highlight);
            canvas.fill_rect(x - 1, y + 1, 3, 2, palette.highlight);
        }
        Gesture::Heave => {
            if frame % 4 >= 2 {
                canvas.fill_circle(
                    (face.x - 12).max(3),
                    (face.y + 8).min(43),
                    1,
                    palette.highlight,
                );
                canvas.set(
                    (face.x - 15).max(2),
                    (face.y + 6).min(43),
                    palette.highlight,
                );
            }
        }
        Gesture::Bop => {
            let x = if frame % 4 < 2 {
                (face.x - 11).max(4)
            } else {
                (face.x + 11).min(42)
            };
            let y = (face.y - 9 - pulse).clamp(5, 40);
            canvas.line(x + 1, y - 3, x + 1, y, 1, palette.accent);
            canvas.set(x + 2, y - 3, palette.accent);
            canvas.fill_circle(x, y + 1, 1, palette.accent);
        }
        // Nothing floats over a watching creature. A mark here would be the creature telling the
        // viewer it is interested; the pose has to say that by itself.
        Gesture::Cover | Gesture::Crouch | Gesture::Balance | Gesture::Reach | Gesture::Watch => {}
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
    if let Some(pose) = creature.state.attention {
        return match pose.emotion {
            formiga_core::AttentionEmotion::Curious => ExpressionKind::Curious,
            formiga_core::AttentionEmotion::Startled => ExpressionKind::Startled,
            formiga_core::AttentionEmotion::Enjoying => ExpressionKind::Joy,
            formiga_core::AttentionEmotion::Concerned => ExpressionKind::Worried,
            formiga_core::AttentionEmotion::Averting => ExpressionKind::Worried,
            formiga_core::AttentionEmotion::Relieved => ExpressionKind::Content,
        };
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
    if creature.state.attention.is_some_and(|pose| {
        pose.emotion == formiga_core::AttentionEmotion::Averting
            || pose.gesture == Some(Gesture::Cover)
    }) {
        return EyelidPose::Closed;
    }
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
    if let Some(pose) = creature.state.attention {
        return GazeDirection::new(
            axis_direction(pose.target.x - creature.state.position.x, 10.0),
            axis_direction(pose.target.y - (creature.state.position.y - 28.0), 10.0),
        );
    }
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
            design: None,
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
        let palette = crate::prop_palette(PALETTES[2], 17);
        let mut hashes = std::collections::BTreeSet::new();
        for variant in 0..TOY_KINDS {
            let mut first = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            let mut second = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            draw_generated_toy(&mut first, palette, EffectMotif::Spark, variant, 24, 24, 0);
            draw_generated_toy(&mut second, palette, EffectMotif::Spark, variant, 24, 24, 0);
            assert_eq!(first, second);
            // Every toy is big enough to be recognised, not a speck beside a paw.
            let opaque = first.pixels().iter().filter(|pixel| pixel.a > 0).count();
            assert!(opaque >= 40, "toy {variant} covers only {opaque} pixels");
            let (min_x, min_y, max_x, max_y) = first.alpha_bounds().expect("a toy is visible");
            assert!(
                max_x - min_x >= 6 && max_y - min_y >= 6,
                "toy {variant} is {}x{} and would vanish at 2x",
                max_x - min_x + 1,
                max_y - min_y + 1
            );
            hashes.insert(Sha256::digest(first.rgba_bytes()).to_vec());
        }
        assert_eq!(hashes.len(), usize::from(TOY_KINDS));

        // A toy that turns looks different as it turns.
        let turned = |spin| {
            let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            draw_generated_toy(&mut canvas, palette, EffectMotif::Spark, 0, 24, 24, spin);
            canvas
        };
        assert_ne!(turned(0), turned(2));

        let mut snacks = std::collections::BTreeSet::new();
        for variant in 0..SNACK_KINDS {
            let mut snack = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            draw_generated_snack(&mut snack, palette, variant, 24, 24, 0);
            assert!(snack.alpha_bounds().is_some());
            // A mouthful visibly goes out of it.
            let mut later = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            draw_generated_snack(&mut later, palette, variant, 24, 24, 2);
            let count = |canvas: &Canvas| canvas.pixels().iter().filter(|p| p.a > 0).count();
            assert!(
                count(&later) < count(&snack),
                "snack {variant} is never actually eaten"
            );
            snacks.insert(Sha256::digest(snack.rgba_bytes()).to_vec());
        }
        assert_eq!(snacks.len(), usize::from(SNACK_KINDS));

        let mut cups = std::collections::BTreeSet::new();
        for variant in 0..DRINK_KINDS {
            let mut cup = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            draw_generated_drinkware(&mut cup, palette, variant, 24, 24, false, 0);
            assert!(cup.alpha_bounds().is_some());
            let mut tipped = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            draw_generated_drinkware(&mut tipped, palette, variant, 24, 24, true, 1);
            assert_ne!(cup, tipped, "cup {variant} never tips toward the mouth");
            cups.insert(Sha256::digest(cup.rgba_bytes()).to_vec());
        }
        assert_eq!(cups.len(), usize::from(DRINK_KINDS));
    }

    /// Exactly the pixels one belonging adds to a frame, drawn on their own so they can be
    /// measured against the body that is using them.
    fn prop_only(genome: &AppearanceGenome, action: ActionKind, frame: u8) -> (Canvas, PixelPoint) {
        let body = CreatureRenderer::render_body_frame(genome, action, frame, false);
        let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        draw_activity_prop(
            &mut canvas,
            genome,
            crate::palette_for(genome),
            body.face_anchor,
            Pose::new(genome, BodyClip::Action(action), frame, false),
            action,
            frame,
            false,
        );
        (canvas, body.face_anchor)
    }

    /// A toy is something the creature is playing with, not something hanging near it: it stays in
    /// the frame, keeps off the face, travels over the round, and comes back to the body.
    #[test]
    fn a_toy_is_carried_through_the_paws_clear_of_the_face_and_inside_the_frame() {
        for family in [
            BodyFamily::Blob,
            BodyFamily::Hopper,
            BodyFamily::SoftQuadruped,
        ] {
            let genome = genome(family);
            for action in [ActionKind::SoloPlay, ActionKind::Eat, ActionKind::Drink] {
                let mut places = std::collections::BTreeSet::new();
                let mut touching = 0;
                for frame in 0..4 {
                    let (prop, anchor) = prop_only(&genome, action, frame);
                    let bounds = prop
                        .alpha_bounds()
                        .unwrap_or_else(|| panic!("{family:?} {action:?} {frame} draws nothing"));
                    assert!(
                        bounds.0 > 0
                            && bounds.1 > 0
                            && bounds.2 < FRAME_SIZE - 1
                            && bounds.3 < FRAME_SIZE - 1,
                        "{family:?} {action:?} {frame}: a prop at {bounds:?} leaves the frame"
                    );
                    // The eyes the layered face draws over this body: anything on top of them is
                    // worn rather than held.
                    for dx in -4..=4 {
                        for dy in -3..=0 {
                            assert_eq!(
                                prop.get(anchor.x + dx, anchor.y + dy).a,
                                0,
                                "{family:?} {action:?} {frame} draws a prop across the face"
                            );
                        }
                    }
                    places.insert((bounds.0, bounds.1));
                    // Contact: the prop's own pixels sit against the body actually drawn.
                    let body = CreatureRenderer::render_body_frame(&genome, action, frame, false);
                    let close = (0..FRAME_SIZE as i32).any(|y| {
                        (0..FRAME_SIZE as i32).any(|x| {
                            prop.get(x, y).a > 0
                                && (-2..=2).any(|dx| {
                                    (-2..=2).any(|dy| {
                                        prop.get(x + dx, y + dy).a == 0
                                            && body.canvas.get(x + dx, y + dy).a > 0
                                    })
                                })
                        })
                    });
                    touching += usize::from(close);
                }
                assert!(
                    touching >= 2,
                    "{family:?} never touches what it is {action:?}-ing"
                );
                if action == ActionKind::SoloPlay {
                    assert!(
                        places.len() >= 3,
                        "{family:?} holds its toy still instead of playing with it"
                    );
                }
            }
        }
    }

    #[test]
    fn generated_discoveries_have_sixteen_deterministic_opaque_silhouettes() {
        let genome = genome(BodyFamily::Blob);
        let mut hashes = std::collections::BTreeSet::new();
        for variant in 0..formiga_core::TRINKET_VARIANTS {
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
        assert_eq!(hashes.len(), usize::from(formiga_core::TRINKET_VARIANTS));
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
            let palette = crate::palette_for(&genome);
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
            let clips = ActionKind::ALL
                .into_iter()
                .map(BodyClip::Action)
                .chain(Gesture::ALL.into_iter().map(BodyClip::Gesture));
            for action in clips {
                let spec = AnimationSpec::for_clip(action);
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
    fn movement_signatures_are_stable_individual_and_stay_inside_their_clips() {
        let desktop = DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: formiga_core::DisplayKey([3; 16]),
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
        let world = World::new([77; 32], time::OffsetDateTime::UNIX_EPOCH, &desktop);
        let base = world.save.creatures[0].clone();
        let signature = MotionSignature::for_creature(&base);
        assert_eq!(signature, MotionSignature::for_creature(&base));

        let mut neighbor = base.clone();
        neighbor.id = base.id ^ 0x5151;
        assert_ne!(MotionSignature::for_creature(&neighbor), signature);

        // Every frame stays inside its clip, and a loop still visits all of its frames.
        let clips = ActionKind::ALL
            .into_iter()
            .map(BodyClip::Action)
            .chain(Gesture::ALL.into_iter().map(BodyClip::Gesture));
        for action in clips {
            let spec = AnimationSpec::for_clip(action);
            let mut seen = std::collections::BTreeSet::new();
            for step in 0..400 {
                let frame = signature.frame(action, step as f32 / 20.0);
                assert!(frame < spec.frames, "{action:?} frame {frame}");
                seen.insert(frame);
                if spec.playback == PlaybackMode::Hold {
                    assert_eq!(frame, spec.frame_at(step as f32 / 20.0));
                }
            }
            if spec.playback == PlaybackMode::Loop {
                assert_eq!(seen.len(), usize::from(spec.frames), "{action:?}");
            }
        }

        // Liveliness and playfulness change cadence without touching stored appearance.
        let appearance = base.appearance.clone();
        let mut lively = base.clone();
        lively.personality.activity = 1.0;
        lively.personality.playfulness = 1.0;
        let mut still = base.clone();
        still.personality.activity = 0.0;
        still.personality.playfulness = 0.0;
        for action in [ActionKind::Traverse, ActionKind::Greet] {
            let (quick, slow) = (
                MotionSignature::for_creature(&lively),
                MotionSignature::for_creature(&still),
            );
            assert!(
                (0..160).any(|step| {
                    let elapsed = step as f32 / 20.0;
                    quick.frame(action, elapsed) != slow.frame(action, elapsed)
                }),
                "{action:?} should differ with temperament"
            );
        }
        assert_eq!(lively.appearance, appearance);
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
            let palette = crate::palette_for(genome);
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

    /// The simulation crate cannot depend on this one, so `world::spacing` approximates where a
    /// face and a body sit inside the 48x48 frame with two fixed boxes, both measured from the
    /// frame's centre column: a face box 15 art pixels to either side, and a body box 23. Their
    /// sum, 38, is why `FACE_CLEAR_RATIO` is rounded up from 38/48 of how wide a creature draws,
    /// and twice the body box is why full separation is 46/48. The watching pose leans a long
    /// body's head furthest, and is what sets the face box; every other clip stays within 14.
    ///
    /// This is the evidence for those numbers. If a new body plan, size, or clip ever reached
    /// further than the boxes, the simulation would space companions too closely and a face would
    /// stay behind a body; this is what catches that here rather than on screen.
    #[test]
    fn every_face_and_body_stays_inside_the_boxes_the_simulation_spaces_by() {
        /// Half-width of the simulation's face box, in art pixels from the frame centre.
        const FACE_BOX_HALF: i32 = 15;
        /// Half-width of the simulation's body box, in art pixels from the frame centre.
        const BODY_BOX_HALF: i32 = 23;
        const CENTRE: i32 = FRAME_SIZE as i32 / 2;

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
        let mut creature =
            World::preview_adult([61; 32], time::OffsetDateTime::UNIX_EPOCH, &desktop);
        let mut design = creature
            .appearance
            .design
            .expect("every generated creature carries a design");
        for plan in formiga_core::BodyPlan::ALL {
            design.body = plan;
            creature.appearance.design = Some(design);
            // An adult sits at the top of the range and the smallest mini at the bottom, so these
            // four values bracket every `logical_size` a colony can hold.
            for logical_size in [19_u8, 25, 34, 40] {
                creature.appearance.logical_size = logical_size;
                let genome = &creature.appearance;
                // How far the drawn face spreads from the middle of its own 16x16 tile, at its
                // widest across every expression, eyelid, and gaze.
                let mut face_reach = 0;
                for expression in ExpressionKind::ALL {
                    for eyelids in EyelidPose::ALL {
                        for gaze in [
                            GazeDirection::default(),
                            GazeDirection { x: 1, y: 1 },
                            GazeDirection { x: -1, y: -1 },
                        ] {
                            let face = CreatureRenderer::render_face_frame(
                                genome,
                                FaceRenderState {
                                    expression,
                                    eyelids,
                                    gaze,
                                },
                            );
                            let (min_x, _, max_x, _) =
                                face.alpha_bounds().expect("a face is never rendered empty");
                            face_reach = face_reach
                                .max(FACE_FRAME_SIZE as i32 / 2 - min_x as i32)
                                .max(max_x as i32 - FACE_FRAME_SIZE as i32 / 2);
                        }
                    }
                }
                for clip in BodyClip::baked() {
                    for frame in 0..AnimationSpec::for_clip(clip).frames {
                        let rendered =
                            CreatureRenderer::render_body_frame(genome, clip, frame, false);
                        let (min_x, _, max_x, _) = rendered
                            .canvas
                            .alpha_bounds()
                            .expect("a body is never rendered empty");
                        let label = format!("{plan:?} size {logical_size} {clip:?} frame {frame}");
                        // Mirroring sends the anchor to `FRAME_SIZE - anchor.x` and the silhouette
                        // to `FRAME_SIZE - 1 - x`, so both facings are measured together.
                        for anchor_x in [
                            rendered.face_anchor.x,
                            FRAME_SIZE as i32 - rendered.face_anchor.x,
                        ] {
                            let reach = (CENTRE - (anchor_x - face_reach))
                                .max(anchor_x + face_reach - CENTRE);
                            assert!(
                                reach <= FACE_BOX_HALF,
                                "{label}: a face reaches {reach} from the frame centre, past the \
                                 {FACE_BOX_HALF} the simulation spaces by"
                            );
                        }
                        for body_x in [
                            min_x as i32,
                            max_x as i32,
                            FRAME_SIZE as i32 - 1 - max_x as i32,
                            FRAME_SIZE as i32 - 1 - min_x as i32,
                        ] {
                            let reach = (CENTRE - body_x).max(body_x - CENTRE);
                            assert!(
                                reach <= BODY_BOX_HALF,
                                "{label}: a body reaches {reach} from the frame centre, past the \
                                 {BODY_BOX_HALF} the simulation spaces by"
                            );
                        }
                    }
                }
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
        Pose::new(
            &genome(BodyFamily::Blob),
            BodyClip::Action(ActionKind::Idle),
            0,
            true,
        )
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
    /// A carried thing rides in front of whoever is holding it. The anchor is one explicit
    /// contract shared by the overlay and the review sheets, so a prop changing hands reads as a
    /// hand-off rather than two objects swapping places in the air.
    #[test]
    fn a_carried_prop_rides_on_the_side_its_holder_is_facing_and_stays_inside_the_frame() {
        let facing = PropAnchor::facing(true);
        let away = PropAnchor::facing(false);
        assert_eq!(facing.dx, -away.dx, "the anchor mirrors with facing");
        assert_eq!(
            facing.dy, away.dy,
            "and rides at the same height either way"
        );
        assert!(facing.dx > 0.0, "a held thing is in front, not behind");
        assert!(
            facing.dy > 0.0,
            "and carried at the chest, not floated above the head"
        );
        // The prop is drawn from the face anchor, in a face-sized quad. Both offsets have to keep
        // that quad inside the body frame, or a prop would be clipped differently from its holder.
        let margin = (FRAME_SIZE - FACE_FRAME_SIZE) as f32 / 2.0;
        for anchor in [facing, away] {
            assert!(
                anchor.dx.abs() <= margin,
                "{anchor:?} leaves the frame sideways"
            );
            assert!(
                anchor.dy.abs() <= margin,
                "{anchor:?} leaves the frame vertically"
            );
        }
        // Two creatures facing each other reach toward one another, which is what makes a
        // hand-off read: the gap between their anchors is smaller than the gap between them.
        let mut creature = World::preview_adult(
            [23; 32],
            time::OffsetDateTime::UNIX_EPOCH,
            &DesktopSnapshot::default(),
        );
        creature.state.facing_right = true;
        let giver = PropAnchor::for_creature(&creature);
        creature.state.facing_right = false;
        let taker = PropAnchor::for_creature(&creature);
        assert!(giver.dx > taker.dx);
    }
    #[test]
    fn every_gesture_is_a_distinct_looping_pose_on_every_body() {
        let preview = World::preview_adult(
            [29; 32],
            time::OffsetDateTime::UNIX_EPOCH,
            &DesktopSnapshot::default(),
        );
        let mut bodies: Vec<(String, AppearanceGenome)> = [
            BodyFamily::Blob,
            BodyFamily::Hopper,
            BodyFamily::SoftQuadruped,
        ]
        .into_iter()
        .map(|family| (format!("{family:?}"), genome(family)))
        .collect();
        for plan in formiga_core::BodyPlan::ALL {
            let mut appearance = preview.appearance.clone();
            let mut design = appearance.design.expect("preview adults are modular");
            design.body = plan;
            appearance.design = Some(design);
            bodies.push((format!("{plan:?}"), appearance));
        }
        for (body, genome) in &bodies {
            // The pose a gesture most often stands in for is the plain inspecting one.
            let inspecting =
                CreatureRenderer::render_body_frame(genome, ActionKind::InspectScreen, 0, false);
            let mut poses: Vec<(Gesture, Canvas)> = Vec::new();
            for gesture in Gesture::ALL {
                let spec = AnimationSpec::for_clip(gesture);
                assert_eq!(spec.playback, PlaybackMode::Loop, "{gesture:?}");
                assert!(spec.frames >= 2, "{gesture:?} is a pose that moves");
                let frames: Vec<_> = (0..spec.frames)
                    .map(|frame| CreatureRenderer::render_body_frame(genome, gesture, frame, false))
                    .collect();
                for rendered in &frames {
                    let (min_x, min_y, max_x, max_y) = rendered
                        .canvas
                        .alpha_bounds()
                        .expect("a gesture is visible");
                    assert!(
                        min_x > 0 && min_y > 0 && max_x < FRAME_SIZE - 1 && max_y < FRAME_SIZE - 1,
                        "{body} {gesture:?} leaves the frame"
                    );
                }
                assert!(
                    frames
                        .windows(2)
                        .any(|pair| pair[0].canvas != pair[1].canvas),
                    "{body} {gesture:?} never moves"
                );
                assert_ne!(
                    frames[0].canvas, inspecting.canvas,
                    "{body} {gesture:?} looks like plain inspecting"
                );
                for (other, canvas) in &poses {
                    assert_ne!(
                        &frames[0].canvas, canvas,
                        "{body}: {gesture:?} and {other:?} share a pose"
                    );
                }
                poses.push((gesture, frames[0].canvas.clone()));
            }
        }
    }

    #[test]
    fn a_body_shows_a_gesture_only_while_its_attention_carries_one() {
        let mut creature = World::preview_adult(
            [31; 32],
            time::OffsetDateTime::UNIX_EPOCH,
            &DesktopSnapshot::default(),
        );
        creature.state.action = ActionKind::InspectScreen;
        creature.state.attention = None;
        assert_eq!(
            BodyClip::for_creature(&creature),
            BodyClip::Action(ActionKind::InspectScreen)
        );
        let mut pose = formiga_core::AttentionPose {
            target: creature.state.position,
            emotion: formiga_core::AttentionEmotion::Curious,
            hanging: 0.0,
            gesture: None,
        };
        creature.state.attention = Some(pose);
        assert_eq!(
            BodyClip::for_creature(&creature),
            BodyClip::Action(ActionKind::InspectScreen)
        );
        let open =
            CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
        for gesture in Gesture::ALL {
            pose.gesture = Some(gesture);
            creature.state.attention = Some(pose);
            assert_eq!(
                BodyClip::for_creature(&creature),
                BodyClip::Gesture(gesture)
            );
            let face =
                CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
            // Covering the face shuts the eyes behind the paws; nothing else changes the face.
            if gesture == Gesture::Cover {
                assert_eq!(face.eyelids, EyelidPose::Closed);
            } else {
                assert_eq!(face, open, "{gesture:?}");
            }
        }
        // Actions keep their own clips: no action is folded into a gesture or the other way.
        let baked: Vec<_> = BodyClip::baked().collect();
        assert_eq!(
            baked.len(),
            ActionKind::BODY_CLIPS.len() + Gesture::ALL.len()
        );
        for clip in &baked {
            assert_eq!(clip.body(), *clip, "{clip:?} is baked under its own name");
        }
    }
}
