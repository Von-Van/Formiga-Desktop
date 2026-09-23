#[cfg(test)]
use crate::PALETTES;
use crate::{Canvas, Palette, Rgba};
use formiga_core::{
    ActionKind, AppearanceGenome, BodyFamily, BrowStyle, Celebration, CheekStyle, Creature,
    CursorSnapshot, EffectMotif, EyeShape, ForelimbStyle, Gesture, Habit, HeadAppendageStyle,
    HighlightStyle, LimbTipStyle, MouthStyle, PatternKind, PupilStyle, RestPose, TailStyle,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
mod accessories;
mod beats;
mod classic;
mod effects;
mod face;
mod modular;
mod pose;
mod props;
pub use accessories::AccessoryArt;
use classic::*;
use effects::*;
use face::*;
use pose::*;
use props::*;
pub use props::{DRINK_KINDS, SNACK_KINDS, TOY_KINDS, prop_variants};

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
    /// Mid-yawn: eyes screwed shut and the mouth wide open.
    Yawning,
}

impl ExpressionKind {
    pub const ALL: [Self; 12] = [
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
        Self::Yawning,
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

/// Where a body's parts are in one frame, as its drawing placed them: what anything it wears is
/// put on against. Frames are drawn facing right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Figure {
    face: PixelPoint,
    /// The middle of the top of the head: its topmost drawn row.
    crown: PixelPoint,
    /// Half the head's width.
    head_half: i32,
    /// Where a collar sits, and half its width: the neck, or for a body that carries its face on
    /// its front, a band across the body just under the face.
    neck: PixelPoint,
    neck_half: i32,
    /// On the front of the chest.
    chest: PixelPoint,
    /// On the back hip, where a bag hangs.
    hip: PixelPoint,
    /// The top of the back, where a pack rides.
    back: PixelPoint,
    /// The lowest row anything worn may reach: the feet.
    floor: i32,
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
            // Resting is the longest clip a companion plays, because it is the one it plays
            // most: six frames at three a second is a two-second loop, room enough to settle,
            // shift its weight, look off at something and settle back without any of those four
            // reading as hurried. Six is also what was left — the two spare slots in the atlas's
            // last row — so the longer rest costs a creature no texture at all.
            ActionKind::Idle => (6, 3, PlaybackMode::Loop),
            ActionKind::Perch | ActionKind::RideWindow => (4, 4, PlaybackMode::Loop),
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
            // One long breath: a second to get all the way up, then held at the top.
            Gesture::Stretch => (4, 3),
            // In, up, held, and down again, in a little under two seconds.
            Gesture::Yawn => (4, 2),
        };
        Self {
            frames,
            fps,
            // A stretch and a yawn are done once, from the moment they start, and held at their
            // last frame until they are let go; every other gesture loops for as long as the
            // moment asks for it.
            playback: if matches!(gesture, Gesture::Stretch | Gesture::Yawn) {
                PlaybackMode::Hold
            } else {
                PlaybackMode::Loop
            },
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

/// What a creature's body shows this frame, with its own ways of doing things laid over the
/// action: which clip, which frame of it, and which way it faces. The overlay, the hit mask and
/// the review sheets all draw this, so a twirl or a stretch looks, and is grabbed, the same
/// everywhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyPresentation {
    pub clip: BodyClip,
    pub frame: u8,
    pub facing_right: bool,
}

/// A twirl turns the creature round twice a second, with its hop.
const TWIRL_TURN_SECS: f32 = 0.25;
/// Circling before a nap turns it round a little slower, stepping as it goes.
const CIRCLE_TURN_SECS: f32 = 0.4;

impl BodyPresentation {
    pub fn for_creature(creature: &Creature) -> Self {
        let state = &creature.state;
        let motion = MotionSignature::for_creature(creature);
        let shown = |clip: BodyClip, elapsed: f32| Self {
            clip,
            frame: motion.frame(clip, elapsed),
            facing_right: state.facing_right,
        };
        if let Some(gesture) = state.attention.and_then(|pose| pose.gesture) {
            if gesture != Gesture::Cheer {
                return shown(BodyClip::Gesture(gesture), state.action_elapsed);
            }
            // Celebrating is done the creature's own way.
            let celebration = Celebration::for_creature(creature);
            let mut body = shown(
                BodyClip::Gesture(celebration.gesture()),
                state.action_elapsed,
            );
            if celebration == Celebration::Twirl {
                body.facing_right ^= turned(state.action_elapsed, TWIRL_TURN_SECS);
            }
            return body;
        }
        // A small moment of its own comes next: a yawn, a start, a turn at the garden.
        if let Some(beat) = state.beat
            && let Some((gesture, into)) = beats::beat_pose(beat)
        {
            return shown(BodyClip::Gesture(gesture), into);
        }
        let Some((habit, action, into)) = flourish_shown(creature) else {
            // Wriggling over in its sleep: the breaths come quicker, a squirm rather than a slide.
            if state.action == ActionKind::Sleep
                && state.nudge == Some(formiga_core::SleepNudge::Wriggling)
            {
                return shown(
                    BodyClip::Action(ActionKind::Sleep),
                    state.action_elapsed * WRIGGLE_RATE,
                );
            }
            // On its way to bed it walks there, and lies down when it arrives.
            if state.walking_to_sleep() {
                return shown(BodyClip::Action(ActionKind::Traverse), state.action_elapsed);
            }
            return shown(BodyClip::Action(state.action), state.action_elapsed);
        };
        match habit {
            // The snack or the drink held up in front before the first bite or sip: the opening
            // frame of the meal, kept while the creature looks it over.
            Habit::LooksFoodOver => Self {
                clip: BodyClip::Action(action),
                frame: 0,
                facing_right: state.facing_right,
            },
            Habit::StretchesBeforeNaps => shown(BodyClip::Gesture(Gesture::Stretch), into),
            // Stepping round on the spot, turning as it goes.
            Habit::CirclesBeforeNaps => {
                let mut body = shown(BodyClip::Action(ActionKind::Traverse), into);
                body.facing_right ^= turned(into, CIRCLE_TURN_SECS);
                body
            }
            Habit::WavesHello => shown(BodyClip::Gesture(Gesture::Reach), into),
            Habit::PlayBows => shown(BodyClip::Gesture(Gesture::Crouch), into),
        }
    }
}

/// How many times faster a sleeper's breaths come while it wriggles over in its sleep.
const WRIGGLE_RATE: f32 = 4.0;

/// The habit a creature is doing right now, the action it opens and how far into it the creature
/// is. Anything its attention is on comes first, so a flourish never shows under a scene.
fn flourish_shown(creature: &Creature) -> Option<(Habit, ActionKind, f32)> {
    let state = &creature.state;
    if state.attention.is_some() {
        return None;
    }
    let flourish = state.flourish?;
    Some((flourish.habit, flourish.action, flourish.progress(state)?))
}

/// Whether something turning round every `period` seconds faces the other way `elapsed` in.
fn turned(elapsed: f32, period: f32) -> bool {
    (elapsed.max(0.0) / period) as u32 % 2 == 1
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

    /// One whole frame of an action with whatever the creature is wearing, looking ahead: how
    /// the colony page shows something being tried on, in each of a few poses.
    pub fn render_dressed_frame(
        genome: &AppearanceGenome,
        dress: Option<AccessoryArt>,
        action: ActionKind,
        frame: u8,
        facing_right: bool,
    ) -> Canvas {
        let state = FaceRenderState {
            expression: expression_for_action(action),
            eyelids: default_eyelids(action, frame),
            gaze: GazeDirection::new(0, 0),
        };
        Self::render_dressed_composited_frame(
            genome,
            dress,
            action,
            frame,
            facing_right,
            false,
            state,
        )
    }

    pub fn render_body_frame(
        genome: &AppearanceGenome,
        clip: impl Into<BodyClip>,
        frame: u8,
        reduce_motion: bool,
    ) -> RenderedBodyFrame {
        Self::render_dressed_body_frame(genome, None, clip, frame, reduce_motion)
    }

    /// One body frame with whatever the creature is wearing drawn on, placed against the body as
    /// this frame draws it.
    pub fn render_dressed_body_frame(
        genome: &AppearanceGenome,
        dress: Option<AccessoryArt>,
        clip: impl Into<BodyClip>,
        frame: u8,
        reduce_motion: bool,
    ) -> RenderedBodyFrame {
        let frame = if reduce_motion { 0 } else { frame };
        let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        let palette = crate::palette_for(genome);
        let clip = clip.into().body();
        let pose = Pose::new(genome, clip, frame, reduce_motion);
        let figure = if let Some(design) = genome.design {
            modular::draw(
                &mut canvas,
                design,
                palette,
                pose,
                scale(genome),
                clip,
                frame,
            );
            modular::figure(design, pose, scale(genome))
        } else {
            match genome.family {
                BodyFamily::Blob => draw_blob(&mut canvas, genome, palette, pose, clip, frame),
                BodyFamily::Hopper => draw_hopper(&mut canvas, genome, palette, pose, clip, frame),
                BodyFamily::SoftQuadruped => {
                    draw_quadruped(&mut canvas, genome, palette, pose, clip, frame)
                }
            }
        };
        let mut face_anchor = figure.face;
        if let Some(dress) = dress {
            accessories::draw_accessory(&mut canvas, dress, figure);
        }
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
        Self::render_dressed_composited_frame(
            genome,
            None,
            clip,
            frame,
            facing_right,
            reduce_motion,
            face_state,
        )
    }

    /// One whole frame, face and all, with whatever the creature is wearing: what the overlay
    /// draws, as one picture, for previews and review sheets.
    pub fn render_dressed_composited_frame(
        genome: &AppearanceGenome,
        dress: Option<AccessoryArt>,
        clip: impl Into<BodyClip>,
        frame: u8,
        facing_right: bool,
        reduce_motion: bool,
        face_state: FaceRenderState,
    ) -> Canvas {
        let mut body = Self::render_dressed_body_frame(genome, dress, clip, frame, reduce_motion);
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

fn scale(genome: &AppearanceGenome) -> f32 {
    genome.logical_size as f32 / 38.0
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
mod tests;
