//! The body half of every clip: how far a body bobs, squashes, steps, leans and folds on each
//! frame.
use super::*;

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
}

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
            ActionKind::Traverse | ActionKind::SqueezeWindow | ActionKind::Follow => {
                Self::stride(genome, phase, bob_amount, false)
            }
            ActionKind::Sprint => Self::stride(genome, phase, bob_amount.max(1), true),
            // Curled up and breathing: a little fuller on the breath in, the ears drooping a
            // touch further on the breath out.
            ActionKind::Sleep => Self {
                bob: frame as i32 % 2,
                squash_x: 3 + i32::from(frame.is_multiple_of(2)),
                squash_y: -3,
                step_a: 0,
                step_b: 0,
                play_lift: 0,
                appendage_lift: -1,
                tail_sway: 0,
                ear_perk: -i32::from(frame % 2 == 1),
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
            // A nibble or a sip on every other frame: the head dips in to it, and the tail
            // gives a happy flick.
            ActionKind::Eat | ActionKind::Drink => Self {
                bob: i32::from(frame % 2),
                squash_x: 1,
                squash_y: -1,
                step_a: 0,
                step_b: 0,
                play_lift: 0,
                appendage_lift: 1,
                tail_sway: if frame.is_multiple_of(3) { 2 } else { 0 },
                lean: i32::from(frame % 2),
                ..Self::default()
            },
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
                tail_sway: alternate,
                ..Self::default()
            },
        };
        if reduce_motion {
            pose.calm();
        }
        pose
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
    fn stride(genome: &AppearanceGenome, phase: usize, bob: i32, running: bool) -> Self {
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
            return Self {
                bob: if down { bob } else { 0 },
                squash_x: i32::from(down),
                squash_y: -i32::from(down),
                step_a: near_hind * (1 + run),
                step_b: near_fore * (1 + run),
                play_lift: i32::from(running && !down),
                appendage_lift: run,
                tail_sway: [1, 2, 1, -1, -2, -1][phase],
                ..Self::default()
            };
        }
        // Facing the screen: left foot up and out, down; right foot up and out, down.
        let left = [-1, -2, 0, 0, 0, 0][phase] * (1 + run);
        let right = [0, 0, 0, 1, 2, 0][phase] * (1 + run);
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
            ..Self::default()
        }
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

/// Whether a creature stretches on all fours rather than standing up to do it: the original
/// soft quadrupeds, and the long body plan among the modular ones.
pub(super) fn stretches_long(genome: &AppearanceGenome) -> bool {
    genome
        .design
        .map_or(genome.family == BodyFamily::SoftQuadruped, |design| {
            design.body == formiga_core::BodyPlan::Long
        })
}
