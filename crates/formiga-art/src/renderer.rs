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
mod fox;
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
    /// Put out: lids low and scowling over a pout.
    Grumpy,
    /// Pleased with itself: lids lowered over a lopsided smile.
    Smug,
    /// Asking with its eyes: wide and shining, worried at the corners.
    Pleading,
}

impl ExpressionKind {
    pub const ALL: [Self; 15] = [
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
        Self::Grumpy,
        Self::Smug,
        Self::Pleading,
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
pub(crate) struct Figure {
    pub(crate) face: PixelPoint,
    /// The middle of the top of the head: its topmost drawn row.
    pub(crate) crown: PixelPoint,
    /// Half the head's width.
    pub(crate) head_half: i32,
    /// Where a collar sits, and half its width: the neck, or for a body that carries its face on
    /// its front, a band across the body just under the face.
    pub(crate) neck: PixelPoint,
    pub(crate) neck_half: i32,
    /// On the front of the chest.
    pub(crate) chest: PixelPoint,
    /// On the back hip, where a bag hangs.
    pub(crate) hip: PixelPoint,
    /// The top of the back, where a pack rides.
    pub(crate) back: PixelPoint,
    /// The lowest row anything worn may reach: the feet.
    pub(crate) floor: i32,
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
            // A hmph, and again.
            Gesture::Huff => (2, 3),
            // Going, going, gone: played once and held at the bottom.
            Gesture::Swoon => (3, 3),
            // Bouncing on the spot.
            Gesture::Beg => (2, 4),
            // A flourish, held a beat, and again.
            Gesture::Strut => (2, 3),
            // Covered, then a peek.
            Gesture::Peek => (2, 2),
            // Tap, tap.
            Gesture::Stomp => (2, 4),
            // Feet swinging, slowly: a whole swing back and forth is a second and a third.
            Gesture::Sit => (4, 3),
            // Push, and push: one paw and then the other.
            Gesture::Scoot => (4, 6),
            // Up to the cheek, a rub down it, up again, and a lick of the paw: a second a wash.
            Gesture::Groom => (4, 4),
            // Side to side and back, fast: a shake is over in under a second.
            Gesture::Shake => (4, 10),
            // Sniff, sniff.
            Gesture::Sniff => (4, 5),
            // Breathing, as asleep in a curl.
            Gesture::Sprawl | Gesture::Loaf => (2, 2),
            // Strides, counted from the ground covered like any walk's.
            Gesture::Skip | Gesture::Trudge => (6, 10),
        };
        Self {
            frames,
            fps,
            // A stretch and a yawn are done once, from the moment they start, and held at their
            // last frame until they are let go; every other gesture loops for as long as the
            // moment asks for it.
            playback: if matches!(gesture, Gesture::Stretch | Gesture::Yawn | Gesture::Swoon) {
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

    /// The frame shown `elapsed` seconds into the clip. Frames are counted in `u32`, as
    /// [`MotionSignature::frame`] counts them: counted in `u8`, the count stopped at 255, so a
    /// loop asked for after 255 frames (85 seconds of a 3 fps idle) stood still on one frame.
    pub fn frame_at(self, elapsed: f32) -> u8 {
        let raw = (elapsed.max(0.0) * f32::from(self.fps)) as u32;
        let frame = match self.playback {
            PlaybackMode::Loop => raw % u32::from(self.frames),
            PlaybackMode::Hold => raw.min(u32::from(self.frames) - 1),
        };
        frame as u8
    }
}

/// Stable per-creature timing. The same creature always walks, rests, greets, and recovers with
/// its own cadence and phase, from data that already exists: its identity and personality. Nothing
/// here is stored, no frame is added, and one-shot clips keep their exact timing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionSignature {
    tempo: [f32; 4],
    phase: [u8; 4],
    /// Where its own run of rests comes from: which beats it does between settles, and how long
    /// it holds still in between.
    rest_seed: u64,
    /// How calm and patient it is, from 0 to 1: a calm companion holds still longer between
    /// shifts of its weight.
    calm: f32,
    /// How curious it is, from 0 to 1: a curious one looks about more often when it rests.
    curiosity: f32,
    /// How energetic it is, from 0 to 1: a lively one shakes itself off more than it washes.
    energy: f32,
    /// How playful it is, from 0 to 1: a playful one skips more of its walks.
    playful: f32,
    /// How trusting it is, from 0 to 1: a trusting one sleeps sprawled out more often than a
    /// wary one, which dozes sitting up.
    trusting: f32,
}

/// One moment of a companion's rest: holding still on its settle, one of the authored beats of
/// the rest, or something it does with itself — a wash, a shake, a sniff of the air — and the
/// frame of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestMoment {
    Settle,
    Beat(u8),
    Own(Gesture, u8),
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
        let rest_seed = next();
        let axes = creature.temperament().axes;
        Self {
            tempo,
            phase,
            rest_seed,
            calm: (1.0 - (axes.energy + axes.impulsiveness) / 2.0).clamp(0.0, 1.0),
            curiosity: axes.curiosity.clamp(0.0, 1.0),
            energy: axes.energy.clamp(0.0, 1.0),
            playful: axes.playfulness.clamp(0.0, 1.0),
            trusting: (1.0 - axes.suspicion).clamp(0.0, 1.0),
        }
    }

    /// A fraction from 0 to 1 out of its own stream, for `salt` and `index`.
    fn roll(self, salt: u64, index: u64) -> f32 {
        let mixed =
            face::mix_u64(self.rest_seed ^ salt ^ index.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        (mixed & 0xffff) as f32 / 65_535.0
    }

    /// Which of the things a companion does with itself it does now, from a fraction: washing
    /// more the calmer it is, shaking itself off more the livelier, and sniffing the air more
    /// the more curious.
    fn fidget(self, pick: f32) -> Gesture {
        let groom = 0.35 + 0.4 * self.calm;
        let shake = 0.15 + 0.4 * self.energy;
        let sniff = 0.2 + 0.5 * self.curiosity;
        let at = pick * (groom + shake + sniff);
        if at < groom {
            Gesture::Groom
        } else if at < groom + shake {
            Gesture::Shake
        } else {
            Gesture::Sniff
        }
    }

    /// How long one of its own moments lasts, in seconds: a wash twice over, three times for a
    /// calm companion; a shake twice; two sniffs.
    fn fidget_secs(self, gesture: Gesture) -> f32 {
        let spec = AnimationSpec::for_clip(gesture);
        let loops = match gesture {
            Gesture::Groom if self.calm > 0.6 => 3.0,
            _ => 2.0,
        };
        loops * f32::from(spec.frames) / f32::from(spec.fps)
    }

    /// The frame of a walk or a run its feet have reached, `stride` walking frames along: the
    /// count the world keeps from how far the creature has actually gone, so its steps keep up
    /// with the ground going by under it.
    pub fn stride_frame(self, stride: f32) -> u8 {
        ((stride.max(0.0) as u32).wrapping_add(u32::from(self.phase[0])) % 6) as u8
    }

    /// The frame of the authored rest loop it shows `elapsed` seconds after it came to rest:
    /// [`Self::rest_moment`] without anything of its own, the settle drawn for any stillness.
    pub fn rest_frame(self, elapsed: f32) -> u8 {
        match self.rest_moment(elapsed, false) {
            RestMoment::Beat(beat) => beat,
            RestMoment::Settle | RestMoment::Own(..) => 0,
        }
    }

    /// What it is doing with its rest `elapsed` seconds after it came to rest.
    ///
    /// The six authored beats of resting are not played round and round. A companion comes to
    /// rest with a settle onto one foot, and then holds still on the plain settle for a while —
    /// longer the calmer it is — before it does something: shifts its weight over and back,
    /// looks off to one side and back, plays the whole of the authored loop, or, where `fidgets`
    /// allows, something of its own — washes its face, shakes itself off, or sniffs the air, as
    /// its temperament leans. Which, and how long each stillness lasts, come from its own stream,
    /// so the same companion rests the same way every time it is shown and no two rest in step.
    pub fn rest_moment(self, elapsed: f32, fidgets: bool) -> RestMoment {
        let spec = AnimationSpec::for_action(ActionKind::Idle);
        let rate = self.tempo[1] * f32::from(spec.fps);
        // A rest this long starts over: the run is irregular, so nobody can see where.
        const ROUND_SECS: f32 = 300.0;
        let mut left = elapsed.max(0.0) % ROUND_SECS;
        let mut run = 0_u64;
        loop {
            let roll = face::mix_u64(self.rest_seed ^ run.wrapping_mul(0x9e37_79b9_7f4a_7c15));
            let pick = (roll & 0xff) as f32 / 255.0;
            let own = run > 0 && fidgets && ((roll >> 16) & 0xff) as f32 / 255.0 < 0.3;
            if own {
                let gesture = self.fidget(((roll >> 24) & 0xff) as f32 / 255.0);
                let secs = self.fidget_secs(gesture);
                if left < secs {
                    let spec = AnimationSpec::for_clip(gesture);
                    let frame = (left * f32::from(spec.fps)) as u32 % u32::from(spec.frames);
                    return RestMoment::Own(gesture, frame as u8);
                }
                left -= secs;
            } else {
                let beats: &[u8] = if run == 0 {
                    // Coming to rest: a settle onto the foot, before the first stillness.
                    &[1]
                } else if pick < 0.25 + 0.3 * self.curiosity {
                    // A look off to the side, and back.
                    &[5, 3, 4, 3, 5]
                } else if pick < 0.5 + 0.3 * self.curiosity {
                    // Weight over onto the other foot, and back.
                    &[1, 2, 1]
                } else {
                    &[1, 2, 3, 4, 5]
                };
                let secs = beats.len() as f32 / rate;
                if left < secs {
                    return RestMoment::Beat(beats[((left * rate) as usize).min(beats.len() - 1)]);
                }
                left -= secs;
            }
            // Still, on the plain settle, for a third of a second to about three and a half.
            let shortest = 1 + (self.calm * 3.0).round() as u64;
            let longest = shortest + 3 + (self.calm * 3.0).round() as u64;
            let hold = (shortest + (roll >> 8) % (longest - shortest + 1)) as f32 / rate;
            if left < hold {
                return RestMoment::Settle;
            }
            left -= hold;
            run += 1;
        }
    }

    /// How it lies `into` seconds into a nap that began at `began` on its clock and lasts
    /// `length`: curled up, sprawled out, or loafed sitting up — a trusting companion sprawls
    /// more and a wary one dozes sitting up — and, in a long nap, now and then turned over into
    /// another way partway through.
    pub fn nap_pose(self, began: f32, into: f32, length: f32, turns: bool) -> Option<Gesture> {
        let nap = (began * 4.0).max(0.0) as u64;
        let way = |pick: f32| {
            let sprawl = 0.15 + 0.35 * self.trusting;
            let loaf = 0.1 + 0.3 * (1.0 - self.trusting);
            if pick < sprawl {
                Some(Gesture::Sprawl)
            } else if pick < sprawl + loaf {
                Some(Gesture::Loaf)
            } else {
                None
            }
        };
        let first = way(self.roll(0x6e61_7031, nap));
        let turn_at = length * (0.4 + 0.25 * self.roll(0x6e61_7032, nap));
        if turns && length > 25.0 && self.roll(0x6e61_7033, nap) < 0.45 && into >= turn_at {
            let mut pick = self.roll(0x6e61_7034, nap);
            // Over into a different way, not the same one again.
            for _ in 0..4 {
                let then = way(pick);
                if then != first {
                    return then;
                }
                pick = (pick + 0.37) % 1.0;
            }
        }
        first
    }

    /// The mood a walk is taken in, if it has one: a trudge when the companion is tired or on
    /// its way to bed, and otherwise, on some of the walks a contented companion takes — more of
    /// a playful one's — a skip. `began` is when on its clock this walk set off, so the whole of
    /// one walk is taken the same way.
    pub fn walk_mood(self, creature: &Creature, began: f32, eager: bool) -> Option<Gesture> {
        let state = &creature.state;
        let drives = &state.drives;
        if state.walking_to_sleep() || drives.sleep_pressure > 0.75 || drives.energy < 0.22 {
            return Some(Gesture::Trudge);
        }
        let content = drives.comfort > 0.5 && drives.boredom < 0.6 && drives.arousal < 0.8;
        let chance = 0.04 + 0.3 * self.playful + if eager { 0.25 } else { 0.0 };
        (content && self.roll(0x736b_6970, (began * 4.0).max(0.0) as u64) < chance)
            .then_some(Gesture::Skip)
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
            // Getting down to sleep rather than dropping into it: a crouch for a moment first.
            if state.action == ActionKind::Sleep
                && !state.walking_to_sleep()
                && state.action_elapsed < LIE_DOWN_SECS
            {
                return shown(BodyClip::Gesture(Gesture::Crouch), state.action_elapsed);
            }
            return Self::footing(creature, motion);
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

    /// The action's own clip, drawn to agree with what the creature's feet are actually doing.
    ///
    /// An action says what a creature is about, not always whether it is moving: a walk to a
    /// friend ends on its mark with time left on the walk, and a greeting can still have a few
    /// steps to go. Once the world has measured it (see [`formiga_core::Presence`]), a walk that
    /// is going nowhere is drawn as a rest, and a rest, a greeting, a meal or a look that its feet
    /// are carrying along is drawn as the walk it is, until it gets there. A walk's steps are
    /// counted from the ground it has covered rather than the time it has spent, so its feet
    /// keep pace with it. A creature never measured — on a review sheet, say — is drawn exactly
    /// as its action says.
    fn footing(creature: &Creature, motion: MotionSignature) -> Self {
        let state = &creature.state;
        let presence = state.presence;
        let action = if state.walking_to_sleep() {
            // On its way to bed it walks there, and lies down when it arrives.
            ActionKind::Traverse
        } else {
            state.action
        };
        let body = |clip: BodyClip, frame: u8| Self {
            clip,
            frame,
            facing_right: state.facing_right,
        };
        if !presence.measured() {
            return body(
                BodyClip::Action(action),
                motion.frame(BodyClip::Action(action), state.action_elapsed),
            );
        }
        let walking = matches!(
            AnimationSpec::body_action(action),
            ActionKind::Traverse | ActionKind::Follow | ActionKind::Sprint
        );
        let planted = matches!(
            action,
            ActionKind::Idle
                | ActionKind::Greet
                | ActionKind::Eat
                | ActionKind::Drink
                | ActionKind::InspectScreen
                | ActionKind::InvestigateCursor
                | ActionKind::Homebound
                | ActionKind::Perch
        );
        if presence.stepping && (walking || planted) {
            let walk = if walking {
                action
            } else {
                ActionKind::Traverse
            };
            let frame = motion.stride_frame(presence.stride);
            // A plain walk can be taken in a mood; a run, a squeeze and a bob along a ledge
            // cannot. A greeting walking over is eager about it.
            let mood = matches!(walk, ActionKind::Traverse | ActionKind::Follow)
                .then(|| {
                    motion.walk_mood(
                        creature,
                        presence.clock - presence.since,
                        action == ActionKind::Greet,
                    )
                })
                .flatten();
            return match mood {
                Some(mood) => body(BodyClip::Gesture(mood), frame),
                None => body(BodyClip::Action(walk), frame),
            };
        }
        let resting = walking || matches!(action, ActionKind::Idle | ActionKind::Homebound);
        // Just up from a nap, or just set down: a stretch or a shake before anything else.
        if resting
            && !presence.reduced
            && let Some((was, ago)) = presence.after
            && let Some((gesture, lasts)) = motion.after_moment(was)
            && ago < lasts
        {
            return body(BodyClip::Gesture(gesture), motion.frame(gesture, ago));
        }
        if action == ActionKind::Sleep && state.nudge.is_none() {
            let into = state.action_elapsed;
            if let Some(way) = motion.nap_pose(
                presence.clock - into,
                into,
                state.action_duration,
                !presence.reduced,
            ) {
                return body(BodyClip::Gesture(way), motion.frame(way, into));
            }
        }
        if resting {
            // At rest since it stopped, or since the rest began, whichever came later.
            let rested = if walking {
                presence.since
            } else {
                state.action_elapsed.min(presence.since)
            };
            let at_home = action == ActionKind::Homebound;
            return match motion.rest_moment(rested, !presence.reduced) {
                RestMoment::Own(gesture, frame) => body(BodyClip::Gesture(gesture), frame),
                // At home it sits as residents do there, and keeps the settle through the beats.
                _ if at_home => body(
                    BodyClip::Action(action),
                    motion.frame(BodyClip::Action(action), state.action_elapsed),
                ),
                RestMoment::Beat(beat) => body(BodyClip::Action(ActionKind::Idle), beat),
                RestMoment::Settle => body(BodyClip::Action(ActionKind::Idle), 0),
            };
        }
        body(
            BodyClip::Action(action),
            motion.frame(BodyClip::Action(action), state.action_elapsed),
        )
    }
}

impl MotionSignature {
    /// What a companion does on stopping `was`, and for how long: up from a nap with a stretch
    /// if it is calm and a shake if it is lively, and a shake on being set down from a hand or
    /// landing from a throw.
    fn after_moment(self, was: ActionKind) -> Option<(Gesture, f32)> {
        match was {
            ActionKind::Sleep if self.calm >= 0.5 => Some((Gesture::Stretch, 1.6)),
            ActionKind::Sleep => Some((Gesture::Shake, 0.8)),
            ActionKind::Dragged | ActionKind::Tossed | ActionKind::Landing => {
                Some((Gesture::Shake, 0.8))
            }
            _ => None,
        }
    }
}

/// How long a companion crouches as it gets down to sleep.
const LIE_DOWN_SECS: f32 = 0.45;

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

pub struct CreatureRenderer;

impl CreatureRenderer {
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

    /// One frame of how a new face is first shown in the creature studio: out for a walk and
    /// pleased about it, eyes open, and then a wave. Out on the desktop a walk is focused on
    /// where it is going; a first look should show the face at its friendliest.
    pub fn render_studio_frame(genome: &AppearanceGenome, action: ActionKind, frame: u8) -> Canvas {
        let state = FaceRenderState {
            expression: match action {
                ActionKind::Traverse => ExpressionKind::Content,
                action => expression_for_action(action),
            },
            eyelids: EyelidPose::Open,
            gaze: GazeDirection::new(0, 0),
        };
        Self::render_composited_frame(genome, action, frame, true, false, state)
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

    /// A piece on its own, as it sits on this creature: every pixel wearing it changes in the
    /// creature's standing frame, cropped to them with a clear pixel all round, so it can be
    /// pictured beside its name at whatever whole-number size a page shows it. `None` when the
    /// piece changes nothing.
    pub fn render_worn_piece(genome: &AppearanceGenome, dress: AccessoryArt) -> Option<Canvas> {
        let bare = Self::render_dressed_frame(genome, None, ActionKind::Idle, 0, true);
        let dressed = Self::render_dressed_frame(genome, Some(dress), ActionKind::Idle, 0, true);
        let mut piece = Canvas::new(dressed.width(), dressed.height());
        for y in 0..dressed.height() as i32 {
            for x in 0..dressed.width() as i32 {
                let pixel = dressed.get(x, y);
                if pixel.a > 0 && pixel != bare.get(x, y) {
                    piece.set(x, y, pixel);
                }
            }
        }
        let (left, top, right, bottom) = piece.alpha_bounds()?;
        let mut cropped = Canvas::new(right - left + 3, bottom - top + 3);
        for y in top..=bottom {
            for x in left..=right {
                let pixel = piece.get(x as i32, y as i32);
                cropped.set((x - left + 1) as i32, (y - top + 1) as i32, pixel);
            }
        }
        Some(cropped)
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
    /// this frame draws it. A sculpted form's body is drawn by [`crate::forms`], and what it
    /// holds and shows is coloured as its face is.
    pub fn render_dressed_body_frame(
        genome: &AppearanceGenome,
        dress: Option<AccessoryArt>,
        clip: impl Into<BodyClip>,
        frame: u8,
        reduce_motion: bool,
    ) -> RenderedBodyFrame {
        Self::dress_body_frame(genome, dress, clip.into(), frame, reduce_motion, 1).0
    }

    /// [`Self::render_dressed_body_frame`], and the same frame drawn `detail` pixels to each of
    /// its own, from 1 to [`crate::forms::FINEST_DETAIL`]. A sculpted form's body is painted that
    /// finely (see [`crate::forms`]); whatever it wears, holds and shows is drawn as at frame
    /// size, each pixel a square, and so is all of a companion, which is pixel art at one pixel
    /// to the unit. The face anchor, like the frame, stays in frame pixels.
    pub fn render_dressed_body_frame_in_detail(
        genome: &AppearanceGenome,
        dress: Option<AccessoryArt>,
        clip: impl Into<BodyClip>,
        frame: u8,
        reduce_motion: bool,
        detail: u32,
    ) -> (RenderedBodyFrame, Canvas) {
        let detail = detail.clamp(1, crate::forms::FINEST_DETAIL);
        let (rendered, fine) =
            Self::dress_body_frame(genome, dress, clip.into(), frame, reduce_motion, detail);
        let fine = fine.unwrap_or_else(|| crate::forms::blocks(&rendered.canvas, detail));
        (rendered, fine)
    }

    /// A dressed body frame, and a sculpted form's painted `detail` pixels to each of its own
    /// when that is more than one.
    fn dress_body_frame(
        genome: &AppearanceGenome,
        dress: Option<AccessoryArt>,
        clip: BodyClip,
        frame: u8,
        reduce_motion: bool,
        detail: u32,
    ) -> (RenderedBodyFrame, Option<Canvas>) {
        let frame = if reduce_motion { 0 } else { frame };
        let clip = clip.body();
        let face_genome;
        // The body as painted, before anything is drawn on it, and the same body painted finely.
        let mut fine = None;
        let (genome, mut canvas, figure) = if genome.echo_fox {
            let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
            let pose = Pose::new(&fox::posing(genome), clip, frame, reduce_motion);
            let figure = fox::draw(&mut canvas, pose, clip, frame);
            (genome, canvas, figure)
        } else if let Some(sculpt) = &genome.sculpt {
            face_genome = crate::forms::face_genome(sculpt, genome);
            let (canvas, figure) = crate::forms::body(sculpt, genome, clip, frame, reduce_motion);
            if detail > 1 {
                let body = crate::forms::body_in_detail(
                    sculpt,
                    genome,
                    clip,
                    frame,
                    reduce_motion,
                    detail,
                );
                fine = Some((canvas.clone(), body));
            }
            (&face_genome, canvas, figure)
        } else {
            let (canvas, figure) = Self::draw_recipe_body(genome, clip, frame, reduce_motion);
            (genome, canvas, figure)
        };
        let palette = crate::palette_for(genome);
        let pose = if genome.echo_fox {
            Pose::new(&fox::posing(genome), clip, frame, reduce_motion)
        } else {
            Pose::new(genome, clip, frame, reduce_motion)
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
        let fine = fine.map(|(mut bare, mut body)| {
            bare.translate(dx, dy);
            body.translate(dx * detail as i32, dy * detail as i32);
            crate::forms::dressed_in_detail(&canvas, &bare, &body, detail)
        });
        let alpha_mask = AlphaMask::from_canvas(&canvas);
        let rendered = RenderedBodyFrame {
            canvas,
            face_anchor,
            alpha_mask,
        };
        (rendered, fine)
    }

    /// A body drawn from its recipe, or from its genes for a look from before recipes.
    fn draw_recipe_body(
        genome: &AppearanceGenome,
        clip: BodyClip,
        frame: u8,
        reduce_motion: bool,
    ) -> (Canvas, Figure) {
        let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
        let palette = crate::palette_for(genome);
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
        (canvas, figure)
    }

    /// The face, in its 16-pixel frame. A sculpted form wears the face its layout's recipe draws,
    /// with its own eye colour, so it keeps every expression a companion has.
    pub fn render_face_frame(genome: &AppearanceGenome, state: FaceRenderState) -> Canvas {
        if genome.echo_fox {
            let mut canvas = Canvas::new(FACE_FRAME_SIZE, FACE_FRAME_SIZE);
            fox::draw_face(&mut canvas, state);
            return canvas;
        }
        if let Some(sculpt) = &genome.sculpt {
            let face_genome = crate::forms::face_genome(sculpt, genome);
            let mut face = Self::render_face_frame(&face_genome, state);
            crate::forms::recolor_eyes(&mut face, &face_genome, sculpt.coat.eyes);
            return face;
        }
        let mut canvas = Canvas::new(FACE_FRAME_SIZE, FACE_FRAME_SIZE);
        let palette = crate::palette_for(genome);
        draw_face(&mut canvas, genome, palette, 8, 7, state);
        canvas
    }

    /// [`Self::render_face_frame`] drawn `detail` pixels to each of its own, from 1 to
    /// [`crate::forms::FINEST_DETAIL`]: a sculpted form's redrawn larger with its corners
    /// rounded, to sit on its finely painted body, and a companion's as squares.
    pub fn render_face_frame_in_detail(
        genome: &AppearanceGenome,
        state: FaceRenderState,
        detail: u32,
    ) -> Canvas {
        let face = Self::render_face_frame(genome, state);
        if genome.sculpt.is_some() && !genome.echo_fox {
            crate::forms::rounded_face(&face, detail)
        } else {
            crate::forms::blocks(&face, detail.clamp(1, crate::forms::FINEST_DETAIL))
        }
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
        Self::outline_frame_in_detail(canvas, 1);
    }

    /// [`Self::outline_frame`] for a frame drawn `detail` pixels to each of its own: the same soft
    /// edge, `detail` pixels wide so it is as heavy as at frame size, hugging the finer
    /// silhouette. At one it is exactly [`Self::outline_frame`].
    pub fn outline_frame_in_detail(canvas: &mut Canvas, detail: u32) {
        let (width, height) = (canvas.width() as usize, canvas.height() as usize);
        let reach = detail.max(1) as usize;
        let solid: Vec<bool> = (0..width * height)
            .map(|at| canvas.get((at % width) as i32, (at / width) as i32).a > 16)
            .collect();
        // Whether anything solid lies within `reach` along a row, along a column, and anywhere in
        // the square around a pixel. A pixel near something solid along its row or column makes
        // a full edge pixel; one near it only across a corner softens the corner, which is what
        // keeps a pixel silhouette from growing a halo.
        let near = |line: &dyn Fn(usize) -> bool, length: usize| -> Vec<bool> {
            let mut before = vec![0_usize; length + 1];
            for i in 0..length {
                before[i + 1] = before[i] + usize::from(line(i));
            }
            (0..length)
                .map(|i| before[(i + reach + 1).min(length)] > before[i.saturating_sub(reach)])
                .collect()
        };
        let mut along_row = vec![false; width * height];
        for y in 0..height {
            let row = near(&|x| solid[y * width + x], width);
            along_row[y * width..(y + 1) * width].copy_from_slice(&row);
        }
        let mut along_column = vec![false; width * height];
        let mut around = vec![false; width * height];
        for x in 0..width {
            let column = near(&|y| solid[y * width + x], height);
            let square = near(&|y| along_row[y * width + x], height);
            for y in 0..height {
                along_column[y * width + x] = column[y];
                around[y * width + x] = square[y];
            }
        }
        for at in 0..width * height {
            if solid[at] || !around[at] {
                continue;
            }
            let alpha = if along_row[at] || along_column[at] {
                150
            } else {
                70
            };
            canvas.set(
                (at % width) as i32,
                (at / width) as i32,
                Rgba::new(18, 26, 22, alpha),
            );
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
    ///
    /// A sculpted form is measured from its own resting frame, and a floater to the ground it
    /// hovers above, so it is never set down there.
    pub fn resting_baseline(genome: &AppearanceGenome, reduce_motion: bool) -> u32 {
        if let Some(sculpt) = genome.sculpt.as_ref().filter(|_| !genome.echo_fox) {
            return crate::forms::resting_baseline(sculpt, genome);
        }
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

    /// The face a creature wears on its own, with nobody else about to look at.
    pub fn resolve_face_state(
        creature: &Creature,
        cursor: CursorSnapshot,
        cursor_reactions: bool,
    ) -> FaceRenderState {
        Self::resolve_face_state_among(creature, &[], cursor, cursor_reactions, false)
    }

    /// The face a creature wears among the rest of its `colony`, which its eyes wander to when
    /// nothing else has them. Under reduced motion they do not wander, and look out as they
    /// always have.
    pub fn resolve_face_state_among(
        creature: &Creature,
        colony: &[Creature],
        cursor: CursorSnapshot,
        cursor_reactions: bool,
        reduce_motion: bool,
    ) -> FaceRenderState {
        let (gaze, at_cursor) =
            resolve_gaze(creature, colony, cursor, cursor_reactions, reduce_motion);
        FaceRenderState {
            expression: resolve_expression(creature),
            eyelids: resolve_eyelids(creature, at_cursor),
            gaze,
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
