//! The body half of every clip: how far a body bobs, squashes, steps, leans and folds on each
//! frame.
use super::*;

/// How far a creature shying away from the cursor carries its head away from it, in art pixels.
pub(super) const AVOID_LEAN: i32 = 2;
/// How far above where it hangs a climber's body is carried on each frame of a stroke, in art
/// pixels: hung at full stretch on the catch, hauled up past its paw on the pull, and settled
/// halfway back on the lock. The paws stay where they gripped, so they are placed from the head
/// less this.
pub(super) const CLIMB_LIFT: [i32; 3] = [0, 2, 1];

#[derive(Clone, Copy, Default)]
pub(super) struct Pose {
    pub(super) bob: i32,
    pub(super) squash_x: i32,
    pub(super) squash_y: i32,
    pub(super) step_a: i32,
    pub(super) step_b: i32,
    pub(super) play_lift: i32,
    pub(super) appendage_lift: i32,
    pub(super) tail_sway: i32,
    /// Head carried forward (positive) or back over the body, in art pixels.
    ///
    /// A modular body moves its whole head oval; the older family bodies have no separate head,
    /// so they carry the ears and the reserved face across the mass they already drew. Either way
    /// the feet stay where they are, which is what makes it read as leaning rather than stepping.
    pub(super) lean: i32,
    /// Legs folded under the body: it sinks while the feet stay planted.
    pub(super) crouch: i32,
    /// Ears, antennae or tufts pricked up past their resting height, in art pixels.
    ///
    /// Separate from `appendage_lift`, which lifts the shoulders: this is the pair on top of the
    /// head, and it grows the ear from its base rather than moving it, so a pricked ear stays
    /// attached to the head it grew on. Only a pose that is listening as well as looking asks
    /// for it, so no existing clip changes shape.
    pub(super) ear_perk: i32,
    /// Feet set out to the sides, in art pixels: sat down on something with its legs out in
    /// front. Only a seated pose asks for it, so no standing clip changes shape.
    pub(super) splay: i32,
    /// The front foot drawn up this many pixels off the ground, its knee brought up: hooking a
    /// leg over a ledge. Only the mantle asks for it, so no other clip changes shape.
    pub(super) knee: i32,
    /// The back foot hanging this many pixels below the ground: a leg still trailing over the
    /// edge of a ledge the rest of the body is up on. Only the mantle asks for it.
    pub(super) trail: i32,
    /// Ears laid back flat against the head, in art pixels shorter than they stand: tucked in
    /// for a leap. Only a leap asks for it, so no other clip changes shape.
    pub(super) ears_back: i32,
    /// The whole body carried to one side over feet that stay where they are, in art pixels,
    /// positive the way the body faces: its weight going over onto the foot it is standing on.
    /// Only a stride asks for it.
    pub(super) sway: i32,
    /// How far each foot is off the ground, the back one and the front, where a stride says so
    /// itself. Without it a foot is lifted by as far as it has stepped out, which is right for a
    /// shuffle on the spot; a stride plants a foot out ahead and carries it back along the ground
    /// under the body, so where a foot is and whether it is down are two things.
    pub(super) lifts: Option<[i32; 2]>,
    /// The paws at its sides swung with the stride, in art pixels: the front one forward and up
    /// as the back one goes back and down, and the other way for a negative swing. Only a
    /// stride asks for it.
    pub(super) swing: i32,
    /// Up against a wall on the side it faces: the body is drawn close in to it, the paw on the
    /// far side comes round from behind to hold it too, and the front foot is set against it
    /// rather than under the body. Only a climb and the start of a mantle ask for it.
    pub(super) wall: bool,
    /// A four-pawed body carried forward in its frame by this many pixels, its feet with it: its
    /// front half brought over the edge of a ledge it is hauling the rest of itself onto. Only
    /// its mantle asks for it.
    pub(super) forward: i32,
}

/// Keep-up, beat by beat, in time with [`ActionKind::SoloPlay`]'s four frames: caught against
/// the paw on the crouch (0), knocked up off it as the body springs (1), at the top of its arc on
/// the stretch (2), and dropping back (3). Each beat is how far out ahead and how far up the toy
/// is from where that frame holds it, in art pixels, so a paw raised for the stretch carries the
/// arc up with it, and a body with no paw free keeps it up off the top of its head instead.
pub(super) const KEEP_UP_TOSS: [(i32, i32); 4] = [(0, 0), (2, 5), (3, 9), (1, 4)];

/// The beat of a mouthful one frame of [`ActionKind::Eat`] is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mouthful {
    /// Held up in front of the mouth, looked at.
    Raised,
    /// Taken in as the head dips to meet it: the mouthful itself, with a bite gone.
    Bitten,
    /// In the mouth and being chewed: nothing in the paws.
    Chewing,
}

/// Eating, frame by frame: held up, bitten as the head dips, and chewed for two frames, the
/// crumbs falling on the last of them (see [`Pose::new`] for the body).
pub(super) const MOUTHFUL: [Mouthful; 4] = [
    Mouthful::Raised,
    Mouthful::Bitten,
    Mouthful::Chewing,
    Mouthful::Chewing,
];

/// How far the head is tipped back with the cup on each frame of [`ActionKind::Drink`], in art
/// pixels: lowered, tipped to the mouth, tipped back further for the deepest of the sip, and
/// lowered again.
pub(super) const SIP_TILT: [i32; 4] = [0, 1, 2, 0];

impl Pose {
    pub(super) fn new(
        genome: &AppearanceGenome,
        clip: BodyClip,
        frame: u8,
        reduce_motion: bool,
    ) -> Self {
        let action = match clip {
            BodyClip::Action(action) => action,
            BodyClip::Gesture(Gesture::Stretch) if stretches_long(genome) => {
                let mut pose = Self::long_stretch(frame);
                if reduce_motion {
                    pose.calm();
                }
                return pose;
            }
            BodyClip::Gesture(mood @ (Gesture::Skip | Gesture::Trudge)) => {
                let mut pose = Self::mood_walk(genome, mood, frame, reduce_motion);
                if reduce_motion {
                    pose.calm();
                }
                return pose;
            }
            BodyClip::Gesture(Gesture::Mantle)
                if formiga_core::climbs_on_all_fours(genome)
                    && frame < formiga_core::MANTLE_PULL_FRAMES =>
            {
                // Still upright on the wall: both forepaws up over the ledge and the hind legs
                // hung long under it, and then pulled up by the length of a foreleg, the hind
                // paws walked up under the body.
                return if frame == 0 {
                    Self {
                        step_a: -2,
                        step_b: 2,
                        tail_sway: -1,
                        wall: true,
                        ..Self::default()
                    }
                } else {
                    Self {
                        step_a: 1,
                        step_b: -2,
                        lean: 1,
                        tail_sway: 1,
                        ear_perk: 1,
                        wall: true,
                        ..Self::default()
                    }
                };
            }
            BodyClip::Gesture(Gesture::Mantle)
                if formiga_core::climbs_on_all_fours(genome)
                    && frame == formiga_core::MANTLE_PULL_FRAMES =>
            {
                // Down off the wall and level, its front half over the edge with both forepaws
                // on the ledge and its hind legs still hanging behind.
                let mut pose = Self {
                    wall: false,
                    forward: 6,
                    ..Self::for_gesture(Gesture::Mantle, frame)
                };
                if reduce_motion {
                    pose.calm();
                }
                return pose;
            }
            BodyClip::Gesture(gesture) => {
                let mut pose = Self::for_gesture(gesture, frame);
                if gesture == Gesture::Leap && formiga_core::climbs_on_all_fours(genome) {
                    // A four-pawed body bounds: stretched out full length as it springs, forelegs
                    // forward and hind legs back; gathered under itself over the top; and its
                    // forelegs put out ahead to land on, the hind ones coming down after.
                    let (hind, fore) =
                        [(-3, 3), (2, -2), (-1, 3), (0, 2)][usize::from(frame.min(3))];
                    pose.step_a = hind;
                    pose.step_b = fore;
                    pose.lifts = None;
                }
                if reduce_motion {
                    pose.calm();
                }
                return pose;
            }
        };
        let phase = frame as usize % 6;
        let walk: i32 = [0, 1, 0, -1, 0, 1][phase];
        let alternate: i32 = [1, 0, -1, 0, 1, 0][phase];
        // The same swing stepped every other frame, for a tail that should read as weight
        // rather than tick at the clip's own rate.
        let slow_alternate: i32 = [1, 0, -1, 0, 1, 0][(phase / 2) % 6];
        let bob_amount = genome.gait_bob.max(0.2).round() as i32;
        let mut pose = match action {
            ActionKind::Traverse | ActionKind::Follow => {
                Self::stride(genome, phase, bob_amount, false, reduce_motion)
            }
            ActionKind::SqueezeWindow => Self::shuffle(genome, frame),
            ActionKind::Sprint => {
                Self::stride(genome, phase, bob_amount.max(1), true, reduce_motion)
            }
            // Curled up and breathing, one slow breath across the four frames: full on the breath
            // in, sinking a pixel, drawing in narrower with the ears drooping a touch further,
            // and rising again before it fills out, so each frame moves one thing a pixel.
            ActionKind::Sleep => {
                let (sink, fill) = BREATH[usize::from(frame % 4)];
                Self {
                    bob: sink,
                    squash_x: 3 + fill,
                    squash_y: -3,
                    step_a: 0,
                    step_b: 0,
                    play_lift: 0,
                    appendage_lift: -1,
                    tail_sway: 0,
                    ear_perk: -i32::from(frame % 4 >= 2),
                    ..Self::default()
                }
            }
            // Sat down and breathing, a breath every two seconds: drawn up a pixel taller and
            // narrower as the breath comes in, filling out with the ears lifting at the top of it,
            // let out a pixel wider and lower, and settling as the ears do, a frame behind the
            // chest. Each frame moves the outline a pixel. Every other breath ends on a flick of
            // the tail.
            ActionKind::Perch | ActionKind::Homebound => {
                let breath = usize::from(frame % 4);
                Self {
                    bob: 2,
                    squash_x: [1, 0, 1, 2][breath],
                    squash_y: [-1, 0, 0, -1][breath],
                    step_a: 0,
                    step_b: 0,
                    play_lift: 0,
                    appendage_lift: 0,
                    tail_sway: if frame % 8 == 7 { 2 } else { 0 },
                    ear_perk: i32::from(breath >= 2),
                    ..Self::default()
                }
            }
            // Keeping a toy up off its paw: crouched ready as it rests there, stretched up to knock
            // it, watching it at the top of its arc with its ears up, and bobbing to catch it.
            ActionKind::SoloPlay => match frame % 4 {
                0 => Self {
                    squash_x: 1,
                    squash_y: -1,
                    appendage_lift: 1,
                    tail_sway: 2,
                    crouch: 1,
                    ..Self::default()
                },
                1 => Self {
                    squash_x: -1,
                    squash_y: 1,
                    play_lift: 1,
                    appendage_lift: 2,
                    tail_sway: -1,
                    lean: -1,
                    ear_perk: 1,
                    ..Self::default()
                },
                2 => Self {
                    squash_y: 1,
                    appendage_lift: 2,
                    tail_sway: 2,
                    lean: -1,
                    ear_perk: 2,
                    ..Self::default()
                },
                _ => Self {
                    bob: 1,
                    squash_x: 1,
                    squash_y: -1,
                    appendage_lift: 1,
                    tail_sway: -2,
                    ..Self::default()
                },
            },
            // Playing with a friend: a play bow with the tail up, a spring, a landing, and a
            // bounce off to one side, over and over.
            ActionKind::SocialPlay => match frame % 4 {
                0 => Self {
                    squash_x: 1,
                    squash_y: -1,
                    appendage_lift: -1,
                    tail_sway: 2,
                    lean: 2,
                    crouch: 2,
                    ..Self::default()
                },
                1 => Self {
                    squash_x: -1,
                    squash_y: 1,
                    play_lift: 2,
                    appendage_lift: 2,
                    tail_sway: -1,
                    ear_perk: 2,
                    ..Self::default()
                },
                2 => Self {
                    bob: 1,
                    squash_x: 1,
                    squash_y: -1,
                    appendage_lift: 1,
                    tail_sway: 2,
                    ..Self::default()
                },
                _ => Self {
                    step_a: 2,
                    step_b: -2,
                    play_lift: 1,
                    appendage_lift: 2,
                    tail_sway: -2,
                    lean: -1,
                    ..Self::default()
                },
            },
            // A bite (see [`MOUTHFUL`]): the snack held up and looked at, the head dipped forward
            // and down to take it, then chewed with full cheeks, and a happy flick of the tail.
            ActionKind::Eat => {
                let beat = MOUTHFUL[usize::from(frame % 4)];
                Self {
                    bob: i32::from(beat == Mouthful::Bitten),
                    squash_x: 1 + i32::from(frame % 4 == 2),
                    squash_y: -1 - i32::from(frame % 4 == 2),
                    appendage_lift: 1,
                    tail_sway: if frame % 4 == 3 { 2 } else { 0 },
                    lean: i32::from(beat == Mouthful::Bitten),
                    ear_perk: i32::from(frame % 4 == 2),
                    ..Self::default()
                }
            }
            // A sip (see [`SIP_TILT`]): the cup lifted, the head tipped back with it while it
            // drinks, further back for the deepest of the sip, and lowered again with a satisfied
            // settle. The cup is held at the mouth, so it tips back with the head.
            ActionKind::Drink => {
                let tilt = SIP_TILT[usize::from(frame % 4)];
                let tipped = tilt > 0;
                Self {
                    bob: i32::from(frame % 4 == 3),
                    squash_x: if tipped { -1 } else { 1 },
                    squash_y: if tipped { 1 } else { -1 },
                    appendage_lift: if tipped { 2 } else { 1 },
                    tail_sway: if frame % 4 == 3 { 2 } else { 1 },
                    lean: -tilt,
                    ear_perk: i32::from(frame % 4 == 2),
                    ..Self::default()
                }
            }
            // Hello, or a pet: a wiggle on the spot, bouncing a little, ears up and the tail
            // going.
            ActionKind::Greet => Self {
                bob: i32::from(frame.is_multiple_of(2)),
                squash_x: i32::from(frame.is_multiple_of(2)),
                squash_y: i32::from(frame % 2 == 1) - i32::from(frame.is_multiple_of(2)),
                play_lift: i32::from(frame % 2 == 1),
                appendage_lift: 1,
                tail_sway: if frame.is_multiple_of(2) { 2 } else { -2 },
                lean: [0, 1, 0, -1][usize::from(frame % 4)],
                ear_perk: i32::from(frame % 2 == 1),
                ..Self::default()
            },
            // Startled: a scrambling shuffle on the spot, shoulders up and the tail tucked. A
            // window that jumps is met with it straight away, after a moment drawn down into a
            // crouch (see `BodyPresentation::for_creature`); a cursor that comes too close is
            // shied away from, the head carried forward. The world turns a creature avoiding the
            // cursor to face away from it, so forward is away from the cursor.
            ActionKind::AvoidCursor | ActionKind::ReactToWindow => Self {
                bob: -walk.abs(),
                squash_x: 1,
                squash_y: -1,
                step_a: walk * 3,
                step_b: alternate * 3,
                play_lift: 0,
                appendage_lift: 2,
                tail_sway: -2,
                lean: if action == ActionKind::AvoidCursor {
                    AVOID_LEAN
                } else {
                    0
                },
                ..Self::default()
            },
            // Down, played once: squashed low and wide on the touch with its legs folding under
            // it, drawn back up, a pixel past standing with its ears up, and standing as it rests,
            // so whatever it does next starts from the frame it ends on.
            ActionKind::Landing => match frame {
                0 => Self {
                    bob: 1,
                    squash_x: 2,
                    squash_y: -2,
                    appendage_lift: -1,
                    tail_sway: -1,
                    crouch: 2,
                    ..Self::default()
                },
                1 => Self {
                    squash_x: 1,
                    squash_y: -1,
                    crouch: 1,
                    ..Self::default()
                },
                2 => Self {
                    bob: -1,
                    squash_x: -1,
                    squash_y: 1,
                    appendage_lift: 1,
                    tail_sway: 1,
                    ear_perk: 1,
                    ..Self::default()
                },
                _ => Self::resting(genome, 0),
            },
            ActionKind::ClimbWindow if formiga_core::climbs_on_all_fours(genome) => {
                Self::long_climb(frame)
            }
            ActionKind::ClimbWindow => Self::climb(frame),
            // Hanging by its paws with its legs swinging slowly together, out to one side and
            // up a little, back through the middle, and out to the other, the tail following
            // them round a beat behind. The paws stay where they hold the ledge.
            ActionKind::Dangle => {
                let swing = [0, 2, 0, -2][usize::from(frame % 4)];
                Self {
                    bob: 0,
                    squash_x: -1,
                    squash_y: 1,
                    step_a: swing,
                    step_b: swing,
                    play_lift: 1,
                    appendage_lift: 2,
                    tail_sway: slow_alternate * 2,
                    ..Self::default()
                }
            }
            // The plain peek a creature does while it works out what it is looking at. Its own
            // clip still reads, so it keeps it; it only gains a lean toward the thing, so that
            // even the unposed half of window attention is pointed at something. Going to see
            // what the pointer is up to is the same peek with its ears pricked as well, rather
            // than standing about with a bob.
            ActionKind::InspectScreen | ActionKind::InvestigateCursor => Self {
                bob: i32::from(frame % 2),
                squash_x: 1,
                squash_y: -1,
                step_a: 0,
                step_b: -1,
                play_lift: 1,
                appendage_lift: 1 + walk.abs(),
                tail_sway: alternate,
                lean: 1,
                ear_perk: i32::from(action == ActionKind::InvestigateCursor),
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
            // Thrown: balled up in the air, legs folded up under it, head and tail tucked in and
            // the ears laid back. Both frames are this one ball; the second is drawn turned a
            // little (see `tumble`), so it rolls over as it flies.
            ActionKind::Tossed => Self {
                appendage_lift: -1,
                tail_sway: -2,
                lean: -1,
                crouch: 4,
                ear_perk: -1,
                ..Self::default()
            },
            // Carried: hanging from the hand, drawn out a pixel longer, its paws, ears and tail
            // hanging still, and the head swaying a pixel over and back, slowly.
            ActionKind::Dragged => Self {
                squash_y: 1,
                appendage_lift: -2,
                tail_sway: -2,
                lean: i32::from(frame >= 2),
                ear_perk: -1,
                ..Self::default()
            },
            // Resting keeps a loop of its own rather than the shared bob: see [`Self::resting`].
            ActionKind::Idle => Self::resting(genome, frame),
            _ => Self {
                bob: frame as i32 % 2,
                squash_x: 0,
                squash_y: 0,
                step_a: 0,
                step_b: 0,
                play_lift: 0,
                appendage_lift: 0,
                // A tail hangs on the body's bob rather than ticking with it.
                tail_sway: slow_alternate,
                ..Self::default()
            },
        };
        if reduce_motion {
            pose.calm();
        }
        pose
    }

    /// One frame of a climb up a window's side: a stroke with one paw, frames 0 to 2, and then
    /// the same with the other. Each stroke is a catch, the paw landed high on the wall and the
    /// body hung long under it, the front foot stepped up the wall and the back one low on it; a
    /// pull, the body hauled up past the paw with its head tucked in, the front foot pushing
    /// straight as the back one steps up, the tail down; and a lock, the paw down at the cheek
    /// while the other reaches up for its own catch. This is the climb of a body that is not
    /// drawn turned for it — an original hopper or soft quadruped, and the Echo Fox; every other
    /// is drawn turned to the wall by `modular::clinging_body`, which takes only the lift from
    /// here.
    ///
    /// The world raises the frame as each of these comes (see `formiga_core::CLIMB_FRAME_RISE`),
    /// and the paws are drawn lower by as much. The body goes up past them a little further than
    /// that on the pull and settles back through the lock ([`CLIMB_LIFT`]), which is all that
    /// moves the head the paws are placed from: nothing else here may.
    fn climb(frame: u8) -> Self {
        let second = frame % 6 >= 3;
        let play_lift = CLIMB_LIFT[usize::from(frame % 3)];
        match frame % 3 {
            // The feet go up the wall in turn, both on it: as the front one steps up high for the
            // catch the back one is low, pushing; on the pull they change over, the front one
            // driving down straight; and through the lock the back one stays high while the front
            // one starts up again. `step_a` is how far up the wall the back foot is.
            0 => Self {
                play_lift,
                appendage_lift: 3,
                knee: if second { 2 } else { 3 },
                tail_sway: -1,
                lean: 1,
                ear_perk: 1,
                wall: true,
                ..Self::default()
            },
            1 => Self {
                play_lift,
                squash_x: 1,
                appendage_lift: 1,
                step_a: if second { 2 } else { 3 },
                tail_sway: 2,
                wall: true,
                ..Self::default()
            },
            _ => Self {
                play_lift,
                appendage_lift: 2,
                knee: 1,
                step_a: 3,
                tail_sway: 1,
                wall: true,
                ..Self::default()
            },
        }
    }

    /// One frame of a four-pawed body's climb, which goes up the wall upright with all four paws
    /// on it (see `modular::draw`): the same three beats as [`Self::climb`], a stroke for the near
    /// forepaw and then one for the far. The forepaw whose stroke it is reaches up the wall as
    /// the other comes back, the hind paws stepping against them; on the pull the legs change
    /// over as the body goes up; and on the lock they gather under it.
    fn long_climb(frame: u8) -> Self {
        let lead = if frame % 6 < 3 { 1 } else { -1 };
        let (hind, fore, lean, tail) = match frame % 3 {
            0 => (-1, 2, 1, -1),
            1 => (2, -2, 0, 2),
            _ => (1, 0, 0, 1),
        };
        Self {
            step_a: lead * hind,
            step_b: lead * fore,
            lean,
            tail_sway: tail,
            ear_perk: i32::from(frame.is_multiple_of(3)),
            wall: true,
            ..Self::default()
        }
    }

    /// One frame of a walk or a run, `phase` of six.
    ///
    /// Each body walks the way it is drawn. The family bodies are drawn side on, so their legs
    /// swing past each other, front and back, and the body dips as each foot comes down. A
    /// modular body faces the screen, so it waddles: one foot lifts and steps out, comes down, and
    /// then the other, with the body squashing as both feet land and rising as each one lifts.
    /// A four-pawed body lifts one paw at a time, near hind, near fore, far hind, far fore, so
    /// three are always down.
    ///
    /// The stride has a step for every frame of the loop, so no leg steps twice where the loop
    /// comes round.
    fn stride(
        genome: &AppearanceGenome,
        phase: usize,
        bob: i32,
        running: bool,
        reduce_motion: bool,
    ) -> Self {
        let run = i32::from(running);
        if genome.design.is_none() {
            // Side on: the legs swing through each other, a footfall at each end of the swing.
            let swing = [2, 1, -1, -2, -1, 1][phase] * (1 + run);
            let footfall = phase.is_multiple_of(3);
            return Self {
                bob: if footfall { bob } else { -run },
                squash_x: i32::from(footfall),
                squash_y: -i32::from(footfall) + i32::from(!footfall && running),
                step_a: swing.clamp(-3, 3),
                step_b: -swing.clamp(-3, 3),
                play_lift: i32::from(running && !footfall),
                appendage_lift: i32::from(!footfall) + run,
                tail_sway: [1, 2, 1, -1, -2, -1][phase],
                lean: run,
                ..Self::default()
            };
        }
        if stretches_long(genome) {
            // One paw up at a time; all four down on the second and fifth frames, where the
            // body dips.
            let near_hind = [1, 0, 0, -1, 0, 0][phase];
            let near_fore = [0, 0, 1, 0, 0, -1][phase];
            let down = phase % 3 == 1;
            // A stride two pixels long at a walk and three at a run, the leg stepping forward
            // lifted and its partner on the other side left that far behind on the ground, so
            // the legs open and close under it. Reduced motion keeps the short step it held.
            let reach = if reduce_motion { 1 + run } else { 2 + run };
            // The head nods forward as a forepaw goes out, and comes back as the body dips.
            let nod = i32::from(!reduce_motion && near_fore != 0);
            return Self {
                bob: if down { bob } else { 0 },
                squash_x: i32::from(down) + i32::from(running && !reduce_motion && !down),
                squash_y: -i32::from(down),
                step_a: near_hind * reach,
                step_b: near_fore * reach,
                play_lift: i32::from(running && !down),
                appendage_lift: run,
                tail_sway: if running && !reduce_motion {
                    [-2, -1, -2, -2, -1, -2][phase]
                } else {
                    [1, 2, 1, -1, -2, -1][phase]
                },
                lean: nod + i32::from(running && !reduce_motion),
                ears_back: if running && !reduce_motion { 2 } else { 0 },
                ear_perk: i32::from(!reduce_motion && !running && down),
                ..Self::default()
            };
        }
        // Facing the screen: left foot up and out, down; right foot up and out, down.
        //
        // A blob's feet are tucked under the one mass it is, so a foot lifted a pixel and then
        // two barely shows from under it. Its foot comes up the whole two pixels for both frames
        // it is off the ground, and the mass goes over onto the planted foot while the other one
        // is up, so the walk reads as a waddle from one foot to the other. Reduced motion holds
        // the first frame of the walk it always held, with both feet down.
        // A blob keeps the waddle from foot to foot that is its walk, and runs as everyone does.
        if !reduce_motion && (running || !strides_as_blob(genome)) {
            return Self::stepping(phase, bob, running);
        }
        let blob = strides_as_blob(genome) && !reduce_motion;
        let left = if blob {
            [-2, -2, 0, 0, 0, 0]
        } else {
            [-1, -2, 0, 0, 0, 0]
        }[phase]
            * (1 + run);
        let right = if blob {
            [0, 0, 0, 2, 2, 0]
        } else {
            [0, 0, 0, 1, 2, 0]
        }[phase]
            * (1 + run);
        let landed = phase % 3 == 2;
        let top = phase % 3 == 1;
        Self {
            bob: if landed { bob } else { -i32::from(top) },
            squash_x: i32::from(landed) - i32::from(top),
            squash_y: i32::from(top) - i32::from(landed),
            step_a: left.clamp(-3, 3),
            step_b: right.clamp(-3, 3),
            play_lift: i32::from(running && top),
            appendage_lift: i32::from(top) + run,
            tail_sway: [1, 2, 1, -1, -2, -1][phase],
            lean: 1 + run,
            ear_perk: i32::from(top),
            sway: if blob { [1, 1, 0, -1, -1, 0][phase] } else { 0 },
            ..Self::default()
        }
    }

    /// One frame of a two-legged body's walk or run, `phase` of six: a step with the front foot,
    /// frames 0 to 2, and then one with the back foot.
    ///
    /// A walking step is a contact, the stepping foot planted out ahead with the body sunk onto
    /// it and the other just leaving the ground behind; a passing, the body up and over the
    /// planted foot, its weight gone across onto it, as the other swings through high; and a
    /// reach, the planted foot gone back under the body and the other out ahead, about to come
    /// down. A planted foot goes back along the ground as the body goes on over it, and a lifted
    /// one comes forward, which is what makes it a walk rather than a march on the spot. The paws
    /// at its sides swing against the feet, the ears lift on the way up and the tail trails.
    ///
    /// A run is the same three beats pushed further: it lands crouched on the stepping foot with
    /// the other flung out behind, drives up off it, and is in the air with both feet off the
    /// ground, legs split, ears flat and paws thrown back, before it lands on the other foot.
    fn stepping(phase: usize, bob: i32, running: bool) -> Self {
        let beat = phase % 3;
        // Where the stepping foot and the other are, and how far each is off the ground.
        let (lead, other) = match (running, beat) {
            (false, 0) => ((3, 0), (-2, 1)),
            (false, 1) => ((1, 0), (0, 2)),
            (false, _) => ((-1, 0), (2, 1)),
            (true, 0) => ((3, 0), (-2, 3)),
            (true, 1) => ((0, 0), (1, 3)),
            (true, _) => ((-3, 2), (3, 2)),
        };
        let front_steps = phase < 3;
        let (front, back) = if front_steps {
            (lead, other)
        } else {
            (other, lead)
        };
        // Over onto the planted foot as the other swings through.
        let over = if front_steps { 1 } else { -1 };
        let swing = [-1, 0, 1, 1, 0, -1][phase];
        if running {
            return Self {
                bob: if beat == 0 { bob.max(1) } else { 0 },
                squash_x: [1, 0, -1][beat],
                squash_y: [-1, 0, 1][beat],
                step_a: back.0,
                step_b: front.0,
                lifts: Some([back.1, front.1]),
                play_lift: [0, 1, 3][beat],
                appendage_lift: 1,
                tail_sway: [-2, -1, -2][beat],
                lean: 2,
                crouch: i32::from(beat == 0),
                ears_back: 2,
                sway: if beat == 1 { over } else { 0 },
                swing: swing * 2,
                ..Self::default()
            };
        }
        Self {
            bob: [bob, -1, 0][beat],
            squash_x: [1, -1, 0][beat],
            squash_y: [-1, 1, 0][beat],
            step_a: back.0,
            step_b: front.0,
            lifts: Some([back.1, front.1]),
            appendage_lift: i32::from(beat == 1),
            tail_sway: [1, 2, 1, -1, -2, -1][phase],
            lean: 1,
            ear_perk: i32::from(beat == 1),
            sway: if beat == 1 { over } else { 0 },
            swing,
            ..Self::default()
        }
    }

    /// One frame of edging through a narrow gap, `frame` of two: short shuffling steps, one foot
    /// a pixel forward and then the other, the ears laid back and the tail kept close. The body
    /// is posed at its own width; the pinch is drawn over the whole frame afterwards (see
    /// `squeeze`), so every body is narrowed the same way.
    fn shuffle(genome: &AppearanceGenome, frame: u8) -> Self {
        let first = frame.is_multiple_of(2);
        let (step_a, step_b) = if genome.design.is_none() {
            // Side on: the legs a short swing apart one way, then the other.
            if first { (1, -1) } else { (-1, 1) }
        } else if stretches_long(genome) {
            // One paw forward at a time, the near hind and then the near fore, three down.
            if first { (1, 0) } else { (0, 1) }
        } else {
            // Facing out: a foot lifted a pixel out to its side, then the other.
            if first { (-1, 0) } else { (0, 1) }
        };
        Self {
            step_a,
            step_b,
            tail_sway: if first { 1 } else { -1 },
            ear_perk: -1,
            ..Self::default()
        }
    }

    /// One frame of a walk with a mood in it, `frame` of six: the body's own stride, with a hop
    /// in each step and everything carried high for a skip, or slower-footed with the head, the
    /// ears and the tail hung low for a trudge.
    fn mood_walk(genome: &AppearanceGenome, mood: Gesture, frame: u8, reduce_motion: bool) -> Self {
        let phase = usize::from(frame % 6);
        let bob = genome.gait_bob.max(0.2).round() as i32;
        let mut pose = Self::stride(genome, phase, bob, false, reduce_motion);
        if mood == Gesture::Skip {
            // Up off the ground between footfalls, landing squashed on each.
            let hop = SKIP_HOP[phase];
            pose.play_lift = hop;
            pose.bob = if hop == 0 { bob } else { 0 };
            pose.squash_x = i32::from(hop == 0) - i32::from(hop == 2);
            pose.squash_y = i32::from(hop == 2) - i32::from(hop == 0);
            pose.appendage_lift = 1 + i32::from(hop > 0);
            pose.tail_sway = [2, 3, 2, 2, 3, 2][phase];
            pose.ear_perk = 1 + i32::from(hop == 2);
            pose.lean = 0;
        } else {
            // Steps half the size, the head hung forward and low, nothing lifted.
            pose.step_a /= 2;
            pose.step_b /= 2;
            pose.lifts = pose.lifts.map(|lifts| lifts.map(|lift| lift.min(1)));
            pose.swing = 0;
            pose.sway = 0;
            pose.bob = bob.max(1);
            pose.play_lift = 0;
            pose.appendage_lift = -1;
            pose.tail_sway = -2 + i32::from(phase % 3 == 0);
            pose.lean = 2;
            pose.crouch = 1;
            pose.ear_perk = 0;
        }
        pose
    }

    /// The rest a companion holds when it has nothing else to do: settle, shift its weight, look
    /// off at something, settle back.
    ///
    /// The plain square screen-facing settle is still in here — it is the first beat, and the one
    /// reduced motion draws on its own — so nothing a companion used to do has been taken away.
    /// The other beats are what it does between one settle and the next. Every move is a pixel or
    /// two, because a resting creature that moves more than that reads as fidgeting rather than
    /// resting, and the beats are three frames a second, slow enough that each one is a posture
    /// held rather than a twitch passed through.
    ///
    /// Which side the weight goes onto, and whether the creature slumps into the shift or keeps
    /// its legs under it, come from [`resting_manner`], so a colony sitting about is not a row of
    /// companions doing the same nothing in step.
    pub(super) fn resting(genome: &AppearanceGenome, frame: u8) -> Self {
        let beat = usize::from(frame % 6);
        let (side, slouch) = resting_manner(genome);
        // The weight goes onto one foot and the other goes light, scuffing in a pixel: both feet
        // moving at once lifts the creature, and a lifted resting creature is hopping.
        let scuff = [0, 1, 1, 1, 0, 0][beat];
        let (step_a, step_b) = if side > 0 { (scuff, 0) } else { (0, -scuff) };
        // A long body has its head out in front rather than over its feet, so the look is a neck
        // craned forward; every other plan tips its head toward the side it settled onto.
        let look = [0, 0, 1, 2, 2, 1][beat];
        Self {
            // A settle onto the shifted foot, no deeper than the single pixel resting has always
            // bobbed, so where a companion is seated is exactly where it was.
            bob: [0, 0, 1, 0, 0, 0][beat],
            // Breathing out wide and flat under the shift, and drawn up narrow for the look.
            squash_x: [0, 1, 1, 0, -1, 0][beat],
            squash_y: [0, -1, -1, 0, 1, 0][beat],
            step_a,
            step_b,
            play_lift: 0,
            // Shoulders drop into the shift and come back up with the look.
            appendage_lift: [0, -1, -1, 0, 1, 0][beat],
            // The tail drifts across the whole loop instead of ticking every other frame.
            tail_sway: side * [0, 1, 1, 0, -1, -1][beat],
            lean: if stretches_long(genome) {
                look
            } else {
                side * look
            },
            // A sloucher sinks onto its haunches while its weight is over to one side; everyone
            // else keeps its legs under it and only leans.
            crouch: i32::from(slouch) * [0, 1, 1, 0, 0, 0][beat],
            // Ears up for the look and down again, which is what tells a look from a sway.
            ear_perk: [0, 0, 1, 2, 1, 0][beat],
            splay: 0,
            knee: 0,
            trail: 0,
            ears_back: 0,
            sway: 0,
            lifts: None,
            swing: 0,
            wall: false,
            forward: 0,
        }
    }

    /// The body half of each gesture: squash, lift, lean and footing. The limbs are placed by
    /// each renderer, since where a paw can reach depends on the body it belongs to.
    pub(super) fn for_gesture(gesture: Gesture, frame: u8) -> Self {
        let tick = i32::from(frame % 2);
        let beat = [0, 1, 0, -1][usize::from(frame % 4)];
        // The watching loop is six frames long, so it gets its own slow counters rather than
        // borrowing the two- and four-frame ones every other pose shares.
        let slow = usize::from(frame % 6);
        match gesture {
            // Topping out onto a ledge, a frame for each beat of it (see
            // `formiga_core::MANTLE_FRAMES_PER_SECOND`): hung from the ledge by both paws, long
            // and still against the wall; pulled up on them until the chin is over it, a foot
            // scrabbling up the wall; squat over the edge with the chest on it and the front knee
            // drawn up to it; raised up on both paws for the push, the back leg still trailing
            // below; and stood on the ledge, settling onto its legs. The first three are beside
            // the window, so they keep the height in the frame the ledge is drawn against.
            Gesture::Mantle => match frame.min(4) {
                0 => Self {
                    appendage_lift: 3,
                    tail_sway: -1,
                    knee: 1,
                    trail: 2,
                    wall: true,
                    ..Self::default()
                },
                1 => Self {
                    squash_x: 1,
                    appendage_lift: 3,
                    tail_sway: 1,
                    ear_perk: 1,
                    knee: 4,
                    trail: 1,
                    wall: true,
                    ..Self::default()
                },
                2 => Self {
                    squash_x: 1,
                    squash_y: -1,
                    appendage_lift: 1,
                    tail_sway: -1,
                    lean: 1,
                    knee: 5,
                    wall: true,
                    ..Self::default()
                },
                3 => Self {
                    play_lift: 2,
                    tail_sway: 1,
                    lean: 1,
                    trail: 3,
                    ..Self::default()
                },
                _ => Self {
                    bob: 1,
                    squash_x: 1,
                    squash_y: -1,
                    tail_sway: 1,
                    crouch: 1,
                    ..Self::default()
                },
            },
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
                splay: 0,
                knee: 0,
                trail: 0,
                ears_back: 0,
                sway: 0,
                lifts: None,
                swing: 0,
                wall: false,
                forward: 0,
            },
            // Breathing in, drawn up tall with the head tipped back as the paw comes up to the
            // mouth, held at the top, and settling down a little lower than it began. Played once:
            // the face does the yawning, and the body frames it.
            Gesture::Yawn => {
                let rise = [1, 2, 2, 0][usize::from(frame.min(3))];
                Self {
                    bob: i32::from(frame >= 3),
                    squash_x: -i32::from(rise > 0),
                    squash_y: rise,
                    play_lift: i32::from(frame == 1 || frame == 2),
                    appendage_lift: 1 + rise,
                    tail_sway: i32::from(frame >= 1),
                    lean: -i32::from(frame == 1 || frame == 2),
                    ear_perk: -i32::from(frame >= 3),
                    ..Self::default()
                }
            }
            // Puffed up and square, chin up, with a little hmph on the second frame and the tail
            // flicked across.
            Gesture::Huff => Self {
                squash_x: 1,
                squash_y: tick,
                appendage_lift: 1,
                tail_sway: if tick == 0 { 2 } else { -1 },
                lean: -1,
                ..Self::default()
            },
            // Rocked back with the paw at the brow, sinking a little lower each frame and held at
            // the bottom.
            Gesture::Swoon => {
                let into = usize::from(frame.min(2));
                Self {
                    squash_x: i32::from(into >= 2),
                    squash_y: 1 - i32::from(into >= 2),
                    appendage_lift: 2,
                    tail_sway: -1,
                    lean: -1 - i32::from(into >= 1),
                    crouch: [0, 2, 3][into],
                    ..Self::default()
                }
            }
            // Sat up tall and bouncing, ears up, tail going.
            Gesture::Beg => Self {
                squash_x: -1,
                squash_y: 2,
                play_lift: tick,
                appendage_lift: 2,
                tail_sway: if tick == 0 { 2 } else { -2 },
                lean: -1,
                ear_perk: 1,
                ..Self::default()
            },
            // Tall, chest out and chin up, one foot forward, the tail swishing.
            Gesture::Strut => Self {
                squash_y: 1,
                appendage_lift: 2,
                step_a: 1,
                tail_sway: 2 - tick * 2,
                lean: -1,
                ear_perk: 1,
                ..Self::default()
            },
            // Hunched small, the tail tucked away.
            Gesture::Peek => Self {
                bob: 1,
                squash_x: 1,
                squash_y: -1,
                crouch: 2,
                appendage_lift: 1,
                tail_sway: -2,
                lean: -1,
                ..Self::default()
            },
            // Sat down low with its feet out at its sides, swinging them in turn, its tail
            // swishing along: settled, and pleased about it.
            Gesture::Sit => Self {
                squash_x: 1,
                squash_y: -1,
                step_a: beat,
                step_b: -beat,
                tail_sway: beat,
                crouch: 3,
                splay: 2,
                ..Self::default()
            },
            // Sat low with its feet out in front, leaning in, and dipping with each push of a paw
            // at the ground.
            Gesture::Scoot => Self {
                bob: tick,
                squash_x: tick,
                squash_y: -tick,
                appendage_lift: -1,
                tail_sway: -1 - tick,
                lean: 1,
                crouch: 3,
                splay: 2,
                ..Self::default()
            },
            // Sat back a little, the head tipped down to the paw as it rubs the cheek, and dipped
            // to lick it on the last frame; the tail curls round, content.
            Gesture::Groom => {
                let lick = frame % 4 == 3;
                Self {
                    bob: 1,
                    squash_x: 1,
                    squash_y: -1,
                    appendage_lift: 1,
                    tail_sway: [1, 2, 1, 1][usize::from(frame % 4)],
                    lean: i32::from(lick),
                    crouch: 1,
                    ..Self::default()
                }
            }
            // Braced low and wrung from side to side, ears and tail flung out each way.
            Gesture::Shake => {
                let side = if frame.is_multiple_of(2) { 1 } else { -1 };
                Self {
                    bob: tick,
                    squash_x: side,
                    squash_y: -side,
                    step_a: -side,
                    step_b: side,
                    appendage_lift: 1 + side,
                    tail_sway: side * 2,
                    lean: side * 2,
                    crouch: 1,
                    ear_perk: 1 + side,
                    ..Self::default()
                }
            }
            // Hunched over it with the head dipping in little bites, an ear twitching.
            Gesture::Nibble => Self {
                bob: tick,
                squash_x: 1,
                squash_y: -1,
                appendage_lift: 1,
                tail_sway: tick,
                lean: [1, 2, 1, 2][usize::from(frame % 4)],
                ear_perk: tick,
                ..Self::default()
            },
            // Leaning right in for a big bite, squashed with the effort, and sitting back.
            Gesture::Chomp => {
                let bite = frame % 4 == 1 || frame % 4 == 2;
                Self {
                    bob: i32::from(frame % 4 == 3),
                    squash_x: 1 + i32::from(bite),
                    squash_y: -1 - i32::from(bite),
                    appendage_lift: 2,
                    tail_sway: if frame % 4 == 3 { 2 } else { 0 },
                    lean: [2, 3, 3, 0][usize::from(frame % 4)],
                    ear_perk: i32::from(frame % 4 == 2),
                    // Gathered low for it, then up out of the crouch once it is bitten.
                    crouch: 1 + i32::from(bite) - i32::from(frame % 4 == 3),
                    ..Self::default()
                }
            }
            // Sat back, cheeks working, the tail swishing contentedly.
            Gesture::Chew => Self {
                bob: tick,
                squash_x: 1 + tick,
                squash_y: -1 - tick,
                appendage_lift: 1,
                tail_sway: 1 + tick,
                crouch: 1,
                ..Self::default()
            },
            // Drawn up tall with the head tipped back to catch it, and settling as it goes down.
            Gesture::Gulp => {
                let up = [0, 2, 2, 0][usize::from(frame % 4)];
                Self {
                    squash_x: -i32::from(up > 0),
                    squash_y: i32::from(up > 0),
                    play_lift: up / 2,
                    appendage_lift: 1 + up / 2,
                    tail_sway: up,
                    lean: -up,
                    ear_perk: i32::from(up > 0),
                    ..Self::default()
                }
            }
            // In the air, a frame for each part of the arc (see `leap_frame`): sprung up at full
            // stretch, the legs trailing long under it, the ears laid back and the tail down;
            // tucked at the top, knees drawn up into a body gone round; coming down opened out,
            // legs apart and reaching, the ears and tail lifted by the air; and the feet put down
            // to the ground under it, wide, ready for the touch. The feet are drawn where each
            // says rather than a row past the lift (see `Pose::lifts`).
            Gesture::Leap => match frame.min(3) {
                0 => Self {
                    squash_x: -1,
                    squash_y: 2,
                    step_a: 1,
                    step_b: -1,
                    lifts: Some([2, 2]),
                    play_lift: 3,
                    appendage_lift: 1,
                    tail_sway: -2,
                    lean: 1,
                    ear_perk: -1,
                    ears_back: 2,
                    ..Self::default()
                },
                1 => Self {
                    squash_x: 1,
                    squash_y: -1,
                    lifts: Some([5, 5]),
                    play_lift: 3,
                    tail_sway: -1,
                    ears_back: 1,
                    ..Self::default()
                },
                2 => Self {
                    squash_y: 1,
                    step_a: -2,
                    step_b: 2,
                    lifts: Some([2, 2]),
                    play_lift: 2,
                    appendage_lift: 2,
                    tail_sway: 2,
                    ear_perk: 2,
                    ..Self::default()
                },
                _ => Self {
                    step_a: -2,
                    step_b: 2,
                    lifts: Some([0, 0]),
                    play_lift: 1,
                    appendage_lift: 1,
                    tail_sway: 1,
                    ear_perk: 1,
                    ..Self::default()
                },
            },
            // Drawn up with the nose out, ears forward, the head bobbing in little sniffs.
            Gesture::Sniff => Self {
                bob: 0,
                squash_x: -1,
                squash_y: 1,
                appendage_lift: 1,
                tail_sway: [0, 1, 0, 1][usize::from(frame % 4)],
                lean: [2, 3, 3, 2][usize::from(frame % 4)],
                ear_perk: [1, 2, 2, 1][usize::from(frame % 4)],
                play_lift: i32::from(frame % 4 == 1),
                ..Self::default()
            },
            // Flat out on its side, legs out and the head laid down ahead of it, breathing slowly:
            // drawn in, filling out, sinking into the breath, and drawing in again.
            Gesture::Sprawl => Self {
                bob: BREATH[usize::from((frame + 3) % 4)].0,
                squash_x: 3 + BREATH[usize::from((frame + 3) % 4)].1,
                squash_y: -3,
                appendage_lift: -1,
                tail_sway: 2,
                lean: 2,
                crouch: 3,
                splay: 3,
                ..Self::default()
            },
            // Sat tucked up, nothing showing below it, the head drawn in and nodding as it
            // breathes: nodding down, filling out, coming up, and drawing in again.
            Gesture::Loaf => Self {
                bob: 1 + i32::from(frame % 4 == 1 || frame % 4 == 2),
                squash_x: 2 + i32::from(frame % 4 >= 2),
                squash_y: -1,
                appendage_lift: -1,
                tail_sway: -1,
                lean: -1,
                crouch: 3,
                ..Self::default()
            },
            // Walks with a mood in them are strides; see [`Self::mood_walk`]. A climb seen from
            // behind and its mantle are drawn as the climb and the mantle are.
            Gesture::Skip
            | Gesture::Trudge
            | Gesture::Stride
            | Gesture::Dash
            | Gesture::Depart
            | Gesture::Scale
            | Gesture::Crest => Self::default(),
            // Leant out over the patch, braced, tipping the can further and back.
            Gesture::Pour => {
                let tip = pour_tip(frame);
                Self {
                    bob: tip,
                    squash_x: 1,
                    squash_y: -1,
                    step_a: -1,
                    step_b: 1,
                    appendage_lift: 1,
                    tail_sway: 1 - tip,
                    lean: 2 + tip,
                    crouch: 1,
                    ..Self::default()
                }
            }
            // Down to the ground, a grip, a tug back, and up holding what came away.
            Gesture::Pluck => match frame % 4 {
                0 => Self {
                    squash_x: 1,
                    squash_y: -1,
                    tail_sway: 1,
                    lean: 2,
                    crouch: 2,
                    ..Self::default()
                },
                1 => Self {
                    squash_x: 2,
                    squash_y: -2,
                    tail_sway: 2,
                    lean: 2,
                    crouch: 2,
                    ..Self::default()
                },
                2 => Self {
                    squash_y: 1,
                    step_a: -1,
                    appendage_lift: 1,
                    tail_sway: -1,
                    lean: -1,
                    crouch: 1,
                    ..Self::default()
                },
                _ => Self {
                    squash_y: 1,
                    appendage_lift: 2,
                    tail_sway: 1,
                    ear_perk: 1,
                    ..Self::default()
                },
            },
            // Close in and up on its toes, working with one paw and then the other.
            Gesture::Tend => Self {
                bob: tick,
                squash_x: -1,
                squash_y: 1,
                step_a: beat,
                step_b: -beat,
                appendage_lift: 2,
                tail_sway: beat,
                lean: 1,
                ..Self::default()
            },
            // Pressed flat under the hand, then pushing up into it, the tail going each way.
            Gesture::Bask => {
                let up = frame % 2 == 1;
                Self {
                    bob: i32::from(!up),
                    squash_x: if up { -1 } else { 2 },
                    squash_y: if up { 1 } else { -2 },
                    appendage_lift: -1,
                    tail_sway: if frame % 4 < 2 { 2 } else { -2 },
                    lean: -i32::from(up),
                    crouch: i32::from(!up),
                    ..Self::default()
                }
            }
            // Square and stiff, one foot lifting and coming down again.
            Gesture::Stomp => Self {
                bob: 1 - tick,
                step_a: -tick,
                appendage_lift: -1,
                tail_sway: tick,
                ..Self::default()
            },
            // Up onto its toes and drawn up tall as the paws go overhead, rocked back a touch at
            // the top and held there. Played once rather than looped, so the last frame is the
            // top of the stretch, and the nap it opens is what lets it go.
            Gesture::Stretch => {
                let rise = [0, 1, 2, 2][usize::from(frame.min(3))];
                Self {
                    squash_x: -1,
                    squash_y: rise,
                    play_lift: i32::from(frame >= 1),
                    appendage_lift: 1 + rise,
                    tail_sway: i32::from(frame >= 2),
                    lean: -i32::from(frame >= 2),
                    ear_perk: i32::from(frame >= 2),
                    ..Self::default()
                }
            }
        }
    }

    /// A four-footed stretch: drawn out long with every paw planted and the head carried well
    /// forward, the way a cat stretches, rather than standing up on its hind legs. It stays on
    /// its feet, since sinking down would read as the nap already begun.
    pub(super) fn long_stretch(frame: u8) -> Self {
        let reach = [0, 1, 2, 2][usize::from(frame.min(3))];
        Self {
            squash_x: reach,
            squash_y: -i32::from(frame >= 2),
            appendage_lift: -1,
            tail_sway: 1 + i32::from(frame >= 2),
            lean: 1 + reach,
            ear_perk: i32::from(frame >= 2),
            ..Self::default()
        }
    }

    /// Reduced motion keeps each pose's shape but drops its travel.
    pub(super) fn calm(&mut self) {
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

/// How a companion settles when it has nothing to do: which side its weight goes onto, and
/// whether it slumps into the shift or keeps its legs under it.
///
/// Both are read from appearance bytes the creature already carries, so a companion rests the
/// same way for its whole life, two companions side by side rest differently, and no save has to
/// remember any of it.
pub(super) fn resting_manner(genome: &AppearanceGenome) -> (i32, bool) {
    let mixed =
        genome.marking_seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ u64::from(genome.face_signature);
    (if mixed & 1 == 0 { -1 } else { 1 }, mixed & 2 == 0)
}

/// One slow breath of a body lying down, four frames of how far it has sunk and how much fuller
/// it is than when drawn in: full, sunk, drawn in, risen. Each frame moves one of the two a pixel.
const BREATH: [(i32, i32); 4] = [(0, 1), (1, 1), (1, 0), (0, 0)];

/// Whether a creature stretches on all fours rather than standing up to do it: the original
/// soft quadrupeds, and the long body plan among the modular ones.
/// Whether frame `phase` of six of this body's walk or run is a footfall, a foot just down —
/// where [`Pose::stride`] dips the body: either end of the swing side on, all four down for a
/// long body, a foot landed for one facing the screen.
/// How high a skip is off the ground on each of its six frames: down on 0 and 3, whatever the
/// body, for its hop overrides the stride's own lift.
const SKIP_HOP: [i32; 6] = [0, 2, 1, 0, 2, 1];

/// Which frame of the leap a hop shows `across` of the way over, from 0 to 1: sprung up for the
/// first third of it, tucked over the top, opened out on the way down, and its feet put down for
/// the last of it.
pub(super) fn leap_frame(across: f32) -> u8 {
    match across {
        across if across < 0.3 => 0,
        across if across < 0.55 => 1,
        across if across < 0.82 => 2,
        _ => 3,
    }
}

/// Whether `phase` of a skip has it landed, feet on the ground.
pub(super) fn skip_landing(phase: usize) -> bool {
    SKIP_HOP[phase % 6] == 0
}

/// How far the can is tipped on each of a pour's four frames: level, over, over, and level again.
const POUR_TIP: [i32; 4] = [0, 1, 1, 0];

/// How far frame `frame` of a pour has the can tipped over: 0 while it is level.
pub(super) fn pour_tip(frame: u8) -> i32 {
    POUR_TIP[usize::from(frame % 4)]
}

pub(super) fn footfall(genome: &AppearanceGenome, phase: usize) -> bool {
    let down = if genome.design.is_none() {
        0
    } else if stretches_long(genome) {
        1
    } else if strides_as_blob(genome) {
        2
    } else {
        // A step lands on its contact (see `Pose::stepping`).
        0
    };
    phase % 3 == down
}

/// Whether a body walks as a blob recipe does (see [`Pose::stride`]): drawn from a recipe of the
/// blob plan, not a sculpted form or the fox, which draw their own bodies.
fn strides_as_blob(genome: &AppearanceGenome) -> bool {
    !genome.echo_fox
        && genome.sculpt.is_none()
        && genome
            .design
            .is_some_and(|design| design.body == formiga_core::BodyPlan::Blob)
}

pub(super) fn stretches_long(genome: &AppearanceGenome) -> bool {
    genome
        .design
        .map_or(genome.family == BodyFamily::SoftQuadruped, |design| {
            design.body == formiga_core::BodyPlan::Long
        })
}
