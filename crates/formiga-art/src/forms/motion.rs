//! What a sculpted form's body is doing, frame by frame. Every app asks for a clip by what is
//! going on (Desktop's actions and gestures); each clip comes down to one of a few intents, and
//! each plan carries out an intent in its own way: a floater bobs where a quadruped steps, and
//! a percher hops.

use crate::{AnimationSpec, BodyClip};
use formiga_core::forms::{Locomotion, Plan};
use formiga_core::{ActionKind, Gesture};
use std::f32::consts::TAU;

/// What a body can be asked to do. Every plan has every one of these, which is what lets a form
/// on any plan stand in for a companion anywhere one is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Intent {
    Idle,
    Move,
    React,
    Inspect,
    Rest,
    Celebrate,
    Social,
    Held,
}

impl Intent {
    pub const ALL: [Self; 8] = [
        Self::Idle,
        Self::Move,
        Self::React,
        Self::Inspect,
        Self::Rest,
        Self::Celebrate,
        Self::Social,
        Self::Held,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Move => "Move",
            Self::React => "React",
            Self::Inspect => "Inspect",
            Self::Rest => "Rest",
            Self::Celebrate => "Celebrate",
            Self::Social => "Social",
            Self::Held => "Held",
        }
    }

    /// The intent a Desktop clip comes down to.
    pub fn for_clip(clip: BodyClip) -> Self {
        use ActionKind as A;
        use Gesture as G;
        match clip.body() {
            BodyClip::Action(action) => match action {
                A::Idle | A::Perch | A::RideWindow | A::Homebound => Self::Idle,
                A::Traverse | A::SqueezeWindow | A::Follow | A::Sprint | A::ClimbWindow => {
                    Self::Move
                }
                A::Sleep => Self::Rest,
                A::InvestigateCursor | A::InspectScreen | A::ReactToWindow | A::Eat | A::Drink => {
                    Self::Inspect
                }
                A::AvoidCursor | A::Greet | A::PetReaction | A::Landing => Self::React,
                A::SoloPlay => Self::Celebrate,
                A::SocialPlay | A::PresentDiscovery => Self::Social,
                A::Dragged | A::Tossed | A::Dangle => Self::Held,
            },
            BodyClip::Gesture(gesture) => match gesture {
                G::Cheer | G::Bop | G::Strut => Self::Celebrate,
                G::Gasp | G::Cover | G::Worry | G::Huff | G::Stomp | G::Beg | G::Peek => {
                    Self::React
                }
                G::Watch | G::Reach | G::Balance | G::Heave => Self::Inspect,
                G::Crouch | G::Stretch | G::Yawn | G::Swoon | G::Sit | G::Scoot => Self::Rest,
            },
        }
    }

    /// A clip that shows the intent, for a preview: what Formiga Farm plays when the owner picks
    /// one.
    pub const fn clip(self) -> BodyClip {
        BodyClip::Action(match self {
            Self::Idle => ActionKind::Idle,
            Self::Move => ActionKind::Traverse,
            Self::React => ActionKind::Greet,
            Self::Inspect => ActionKind::InspectScreen,
            Self::Rest => ActionKind::Sleep,
            Self::Celebrate => ActionKind::SoloPlay,
            Self::Social => ActionKind::SocialPlay,
            Self::Held => ActionKind::Dragged,
        })
    }
}

/// The body's pose in one frame, in frame pixels: every plan reads the parts that mean something
/// to it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Pose {
    /// The whole figure raised off its ground, as in a hop.
    pub(crate) lift: f32,
    /// The body bobbing on its legs, down positive.
    pub(crate) bob: f32,
    /// The head carried forward of where it rests.
    pub(crate) lean: f32,
    /// The head dipped (positive) or raised.
    pub(crate) nod: f32,
    /// Legs folded under, from standing (0) to lying down (1).
    pub(crate) crouch: f32,
    /// Each leg's step, near front, near back, far front, far back: forward of where it stands.
    pub(crate) step: [f32; 4],
    /// Each foot's lift off the ground in the same order.
    pub(crate) raise: [f32; 4],
    /// The tail swung up (positive) or down.
    pub(crate) sway: f32,
    /// Ears pricked up.
    pub(crate) perk: f32,
    /// Wings, or arms, raised from at rest (0) to all the way up (1).
    pub(crate) spread: f32,
    /// Fins and flippers swept back (positive) or forward.
    pub(crate) paddle: f32,
    /// Hanging from the scruff, limbs dangling.
    pub(crate) held: bool,
    /// The body tipped, in radians, nose up positive.
    pub(crate) tip: f32,
}

/// The pose of `plan` in frame `frame` of `clip`. Reduced motion holds every intent still in its
/// first pose: a cut rather than a slower animation.
pub(crate) fn pose(plan: Plan, clip: BodyClip, frame: u8, reduce_motion: bool) -> Pose {
    let intent = Intent::for_clip(clip);
    let frames = AnimationSpec::for_clip(clip).frames.max(1);
    let frame = if reduce_motion { 0 } else { frame % frames };
    let t = f32::from(frame) / f32::from(frames);
    let wave = (t * TAU).sin();
    let half = (t * TAU * 2.0).sin();
    let locomotion = plan.locomotion();
    let mut pose = Pose::default();
    match intent {
        Intent::Idle => {
            // A slow breath, the tail drifting, and the ears pricked once a loop.
            pose.bob = if wave > 0.3 { 1.0 } else { 0.0 };
            pose.sway = wave;
            pose.perk = if frame == 3 { 1.0 } else { 0.0 };
            if locomotion == Locomotion::Float {
                pose.lift = wave.round();
                pose.paddle = wave * 0.5;
            }
        }
        Intent::Move => match locomotion {
            Locomotion::Walk | Locomotion::Amble => {
                let swing = if locomotion == Locomotion::Amble {
                    1.5
                } else {
                    2.5
                };
                let a = wave * swing;
                // Diagonal pairs step together, the near front with the far back.
                pose.step = [a, -a, -a, a];
                let up = |s: f32| if s > 0.5 { 1.0 } else { 0.0 };
                pose.raise = [up(wave), up(-wave), up(-wave), up(wave)];
                pose.bob = if half.abs() > 0.7 { 1.0 } else { 0.0 };
                pose.sway = wave;
                pose.spread = if locomotion == Locomotion::Amble {
                    0.15 + 0.1 * wave
                } else {
                    0.0
                };
                pose.tip = if locomotion == Locomotion::Amble {
                    wave * 0.04
                } else {
                    0.0
                };
            }
            Locomotion::Float => {
                // Never walking on legs it does not have: a swell of the body and a beat of the
                // tail.
                pose.lift = 1.0 + wave * 1.5;
                pose.sway = -wave * 1.5;
                pose.paddle = wave;
                pose.tip = wave * 0.06;
            }
            Locomotion::Crawl => {
                let a = wave * 1.5;
                pose.step = [a, -a, -a, a];
                pose.paddle = wave;
                pose.bob = if half.abs() > 0.7 { 1.0 } else { 0.0 };
                pose.sway = wave * 0.5;
            }
            Locomotion::Hop => {
                // Up and over, wings opening at the top of the hop.
                let arc = (t * TAU / 2.0).sin().max(0.0);
                pose.lift = (arc * 3.0).round();
                pose.spread = arc * 0.5;
                pose.sway = -arc;
                pose.raise = [arc, arc, arc, arc];
            }
        },
        Intent::React => {
            let arc = (t * TAU).sin().max(0.0);
            pose.lift = (arc * 2.0).round();
            pose.perk = 2.0;
            pose.spread = arc * 0.6;
            pose.sway = 1.0;
            pose.lean = -1.0;
        }
        Intent::Inspect => {
            pose.lean = 2.0 + if frame % 2 == 1 { 1.0 } else { 0.0 };
            pose.nod = 1.0;
            pose.perk = 1.0;
            pose.sway = wave * 0.5;
            if locomotion == Locomotion::Float {
                pose.lift = 1.0;
            }
        }
        Intent::Rest => {
            pose.crouch = 1.0;
            pose.nod = 2.0;
            pose.sway = -1.0;
            pose.bob = if wave > 0.0 { 1.0 } else { 0.0 };
            pose.paddle = -0.5;
        }
        Intent::Celebrate => {
            let arc = (t * TAU).sin().abs();
            pose.lift = (arc * 4.0).round();
            pose.spread = 0.4 + arc * 0.6;
            pose.perk = 2.0;
            pose.sway = 1.0 + wave;
            pose.tip = wave * 0.08;
            pose.paddle = wave;
            pose.raise = [arc, arc, arc, arc];
        }
        Intent::Social => {
            pose.lean = 1.0;
            pose.sway = half * 1.5;
            pose.perk = 1.0;
            pose.bob = if wave > 0.0 { 1.0 } else { 0.0 };
            pose.spread = 0.2 + 0.2 * wave;
            if locomotion == Locomotion::Float {
                pose.lift = 1.0 + wave;
            }
        }
        Intent::Held => {
            pose.held = true;
            pose.sway = -1.0 + wave * 0.5;
            pose.step = [wave, -wave, -wave, wave].map(|s| s * 0.8);
            pose.spread = 0.3;
            pose.paddle = wave;
            pose.tip = 0.08;
        }
    }
    pose
}
