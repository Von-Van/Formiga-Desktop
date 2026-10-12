//! The Echo Fox: what every companion is drawn as while Echo Fox Mode is on.
//!
//! Unlike a companion, the fox is not generated. It is one picture, drawn pixel for pixel from the
//! sketch it came from and cut into the parts that move: the tail, the head with its ears, the
//! body, the two feet and the two forepaws. Each clip moves those parts by the same [`Pose`] a
//! companion's body is moved by, and carries a paw out to wherever a companion's paw goes
//! ([`modular::limbs`]), so the fox walks, climbs, dangles, plays, eats and sleeps everything a
//! companion does. Its face is drawn in the face frame, as a companion's is, so it blinks, looks
//! about and changes its expression the same way.
//!
//! Every fox is the same fox: it is posed as one genome whoever it stands in for ([`posing`]),
//! and nothing a companion is changes.
use super::modular::{self, Limb};
use super::*;
use formiga_core::BodyPlan;

/// Where the sketch's top-left corner sits in a frame. Its feet stand on row 41.
const ORIGIN_X: i32 = 8;
const ORIGIN_Y: i32 = 16;

/// The middle of the face, in the sketch: between the eyes, at the top of the muzzle.
const FACE: (i32, i32) = (16, 8);

/// How far the fox is drawn up its frame to dangle or climb, so the paws it reaches up with meet
/// the ledge a companion's do.
const HANG: i32 = 9;

const OUTLINE: Rgba = Rgba::new(22, 17, 18, 255);
const COAT: Rgba = Rgba::new(247, 145, 34, 255);
const WHITE: Rgba = Rgba::new(255, 252, 246, 255);
const SOOT: Rgba = Rgba::new(90, 80, 84, 255);
const BLUSH: Rgba = Rgba::new(244, 182, 198, 255);
const MOUTH: Rgba = Rgba::new(150, 52, 60, 255);

/// The body, head and ears, without the tail, the feet or the forepaws. `K` is outline, `O` coat,
/// `W` white and `D` the soot of the ears and paws.
const BODY: [&str; 24] = [
    ".........KKK.........KKK",
    "........KWDDK.......KDDWK",
    "........KWWDDKKKKKKKDDWWK",
    "........KWWWDOOOOOOODWWWK",
    ".......KOOOOOOOOOOOOOOOOOK",
    ".......KOOOOOOOOOOOOOOOOOK",
    "......KOOOOOOOOOOOOOOOOOOOK",
    "......KOOOOOOOOOOOOOOOOOOOK",
    ".....KOOOOOOOOOOOOOOOOOOOOOK",
    ".....KOOOOOOOOOOOOOOOOOOOOOK",
    "....KKKOOOOOOOOOOOOOOOOOOOKK",
    ".......KKOOOOOOOOOOOOOOOKK",
    ".........KOOOOOOOOOOOOOK",
    ".........KOOOOOOOOOOOOOK",
    "........KOOOOOOOOOOOOOOOK",
    "........KOOOOOOOOOOOOOOOK",
    ".......KOOOOOOOOOOOOOOOOOK",
    ".......KOOOOOOOOOOOOOOOOOK",
    ".......KOOOOOOOOOOOOOOOOOK",
    ".......KOOOOOOOOOOOOOOOOOK",
    ".......KOOOOOOOOOOOOOOOOOK",
    ".......KOOOOOOOOOOOOOOOOK",
    "........KOOOOOOOOOOOOOOOK",
    "..............KKKKK",
];

/// The last row of the head; the body starts below it.
const NECK_ROW: usize = 11;

/// The last row of the ears; the head starts below it.
const EAR_ROWS: usize = 3;

/// The brush, curling up behind the body with its white tip folded over, from row 9.
const TAIL: [&str; 12] = [
    "...K",
    "..KWK",
    ".KWWWKW",
    ".KOOOKWWW",
    ".KOOOKWWW",
    "KOOOOKWW",
    "KOOOOKWW",
    "KOOOOKW",
    ".KOOOOK",
    "..KOOOO",
    "...KKOO",
    ".....KK",
];
const TAIL_ROW: i32 = 9;

/// A foot, from row 23: the back foot at column 9 and the front one at column 19.
const BACK_FOOT: [&str; 3] = ["KDOOO", "KDDDK", ".KKK."];
const FRONT_FOOT: [&str; 3] = ["OOODK", "KDDDK", ".KKK."];
const FOOT_ROW: i32 = 23;

/// A forepaw folded on the belly, each pixel in the sketch's own place.
const BACK_PAW: [(i32, i32, Rgba); 12] = [
    (12, 14, OUTLINE),
    (13, 14, OUTLINE),
    (13, 15, OUTLINE),
    (13, 16, SOOT),
    (14, 16, OUTLINE),
    (10, 17, OUTLINE),
    (12, 17, SOOT),
    (13, 17, SOOT),
    (14, 17, OUTLINE),
    (11, 18, OUTLINE),
    (12, 18, OUTLINE),
    (13, 18, OUTLINE),
];
const FRONT_PAW: [(i32, i32, Rgba); 12] = [
    (20, 14, OUTLINE),
    (21, 14, OUTLINE),
    (20, 15, OUTLINE),
    (19, 16, OUTLINE),
    (20, 16, SOOT),
    (19, 17, OUTLINE),
    (20, 17, SOOT),
    (21, 17, SOOT),
    (23, 17, OUTLINE),
    (20, 18, OUTLINE),
    (21, 18, OUTLINE),
    (22, 18, OUTLINE),
];

/// Where a forepaw is carried out from: its shoulder, in the sketch.
const SHOULDERS: [(i32, i32); 2] = [(11, 15), (21, 15)];

fn color(key: u8) -> Option<Rgba> {
    match key {
        b'K' => Some(OUTLINE),
        b'O' => Some(COAT),
        b'W' => Some(WHITE),
        b'D' => Some(SOOT),
        _ => None,
    }
}

/// Paints one row of a part with its left end at `x`.
fn row(c: &mut Canvas, x: i32, y: i32, pixels: &str) {
    for (dx, key) in pixels.bytes().enumerate() {
        if let Some(color) = color(key) {
            c.set(x + dx as i32, y, color);
        }
    }
}

/// How the parts are moved in one frame, in frame pixels.
#[derive(Clone, Copy, Debug)]
struct Placement {
    /// The sketch's top-left corner for the feet, which stay on the ground unless the whole fox
    /// leaves it.
    ground: PixelPoint,
    /// How far the body, head and tail sink onto the feet.
    sink: i32,
    /// The head and ears carried forward or back.
    lean: i32,
    ear: i32,
    tail: i32,
    steps: [i32; 2],
    splay: i32,
}

impl Placement {
    fn new(pose: Pose, clip: BodyClip) -> Self {
        let hanging = matches!(
            clip,
            BodyClip::Action(ActionKind::Dangle | ActionKind::ClimbWindow)
        );
        // A hop takes the feet with it; a settle folds the legs under a body that stays round.
        let lift = pose.play_lift.clamp(0, 3) + (-pose.bob).clamp(0, 2);
        let sink =
            (pose.crouch.clamp(0, 3) + (-pose.squash_y).clamp(0, 3) + pose.bob.clamp(0, 2)).min(3);
        Self {
            ground: PixelPoint {
                x: ORIGIN_X,
                y: ORIGIN_Y - lift - if hanging { HANG } else { 0 },
            },
            sink,
            lean: pose.lean.clamp(-2, 2),
            ear: pose.ear_perk.clamp(-1, 1),
            tail: pose.tail_sway.clamp(-1, 1),
            steps: [pose.step_a.clamp(-1, 1), pose.step_b.clamp(-1, 1)],
            splay: pose.splay.clamp(0, 2),
        }
    }

    /// Where a point of the sketch's body is drawn.
    fn body(self, x: i32, y: i32) -> PixelPoint {
        PixelPoint {
            x: self.ground.x + x,
            y: self.ground.y + self.sink + y,
        }
    }

    /// Where a point of the sketch's head is drawn.
    fn head(self, x: i32, y: i32) -> PixelPoint {
        self.body(x + self.lean, y)
    }

    fn face(self) -> PixelPoint {
        self.head(FACE.0, FACE.1)
    }
}

/// The genome every fox is posed as, whoever it stands in for: an upright body with a steady
/// gait, so every fox walks, stretches and hops alike. What it holds and the sparkles it makes
/// are still its own.
pub(super) fn posing(genome: &AppearanceGenome) -> AppearanceGenome {
    AppearanceGenome {
        design: None,
        sculpt: None,
        family: BodyFamily::Hopper,
        gait_bob: 0.5,
        marking_seed: 0,
        face_signature: 0,
        ..genome.clone()
    }
}

/// One body frame of the fox, facing right, and where anything it wears goes.
pub(super) fn draw(c: &mut Canvas, pose: Pose, clip: BodyClip, frame: u8) -> Figure {
    let at = Placement::new(pose, clip);

    // The tail first, behind everything, waving from its root.
    let tail = at.body(0, TAIL_ROW + at.tail);
    for (dy, pixels) in TAIL.iter().enumerate() {
        row(c, tail.x, tail.y + dy as i32, pixels);
    }

    // The feet, which the body settles down over.
    for (side, (column, foot)) in [(9, BACK_FOOT), (19, FRONT_FOOT)].into_iter().enumerate() {
        let step = at.steps[side];
        let out = if side == 0 { -at.splay } else { at.splay };
        let x = at.ground.x + column + step + out;
        let y = at.ground.y + FOOT_ROW - step.abs();
        for (dy, pixels) in foot.iter().enumerate() {
            row(c, x, y + dy as i32, pixels);
        }
    }

    // Where each forepaw is: folded on the belly, or carried out to wherever a companion's
    // would go.
    let face = at.face();
    let body = modular::Body {
        x: at.body(16, 0).x,
        y: at.body(0, 17).y,
        rx: 8,
        ry: 6,
        hx: face.x,
        hy: face.y,
        head: 10,
        floor: at.ground.y + FOOT_ROW + 1,
    };
    let limbs = modular::limbs(clip, frame, body, BodyPlan::Upright);
    let neck = at.head(0, NECK_ROW as i32).y;
    // A paw raised up beside the head goes up behind its cheek, which is wider than the body it
    // rises from; one brought to the face, or anywhere lower, is in front.
    let behind = |hand: PixelPoint| hand.y < neck && (hand.x - face.x).abs() > 9;

    // The body, the paws raised behind the head, then the head over its top, then the ears:
    // pricked, they grow from their base, and drooped they fold down into it, so they stay joined
    // to the head either way.
    for (y, pixels) in BODY.iter().enumerate().skip(NECK_ROW + 1) {
        let p = at.body(0, y as i32);
        row(c, p.x, p.y, pixels);
    }
    for (shoulder, limb) in SHOULDERS.into_iter().zip(limbs) {
        if let Limb::Reach(hand) = limb
            && behind(hand)
        {
            arm(c, at.body(shoulder.0, shoulder.1), hand);
        }
    }
    if at.ear > 0 {
        for (y, pixels) in BODY.iter().enumerate().take(EAR_ROWS + 1) {
            let p = at.head(0, y as i32 - 1);
            row(c, p.x, p.y, pixels);
        }
    } else if at.ear < 0 {
        for (y, pixels) in BODY.iter().enumerate().take(EAR_ROWS) {
            let p = at.head(0, y as i32 + 1);
            row(c, p.x, p.y, pixels);
        }
    }
    let first = if at.ear == 0 { 0 } else { EAR_ROWS };
    for (y, pixels) in BODY.iter().enumerate().take(NECK_ROW + 1).skip(first) {
        let p = at.head(0, y as i32);
        row(c, p.x, p.y, pixels);
    }
    for ((paw, shoulder), limb) in [BACK_PAW, FRONT_PAW].into_iter().zip(SHOULDERS).zip(limbs) {
        match limb {
            Limb::Rest => {
                for (x, y, color) in paw {
                    let p = at.body(x, y);
                    c.set(p.x, p.y, color);
                }
            }
            Limb::Reach(hand) if !behind(hand) => arm(c, at.body(shoulder.0, shoulder.1), hand),
            Limb::Reach(_) => {}
        }
    }

    let floor = at.ground.y + FOOT_ROW + 2;
    Figure {
        face,
        crown: at.head(FACE.0, EAR_ROWS as i32 + 1),
        head_half: 9,
        neck: at.head(FACE.0, NECK_ROW as i32),
        neck_half: 6,
        chest: at.body(FACE.0 + 1, 13),
        hip: at.body(9, 18),
        back: at.body(11, 13),
        floor,
    }
}

/// A forepaw carried out from its shoulder to `hand`, ending in a sooty paw.
fn arm(c: &mut Canvas, root: PixelPoint, hand: PixelPoint) {
    c.line(root.x, root.y, hand.x, hand.y, 3, OUTLINE);
    c.line(root.x, root.y, hand.x, hand.y, 2, COAT);
    c.fill_circle(root.x, root.y, 1, COAT);
    c.fill_rect(hand.x - 1, hand.y - 1, 4, 4, OUTLINE);
    c.fill_rect(hand.x, hand.y, 2, 2, SOOT);
}

/// Where the fox holds what it is using: in its front paw, folded on its belly, and up at its
/// muzzle for a mouthful.
pub(super) fn prop_hold(pose: Pose, clip: BodyClip) -> PropHold {
    let at = Placement::new(pose, clip);
    let face = at.face();
    PropHold {
        hands: at.body(21, 16),
        mouth: PixelPoint {
            x: face.x + 4,
            y: face.y + 3,
        },
        floor: at.ground.y + FOOT_ROW + 1,
        ground: at.ground.y + FOOT_ROW + 2,
        forward: 1,
    }
}

/// The fox's face in its 16-pixel frame: two short dashes for eyes, a blush under each, and its
/// white muzzle with the nose on top.
pub(super) fn draw_face(c: &mut Canvas, state: FaceRenderState) {
    // The frame's top-left is the sketch's column 8, row 0.
    let set = |c: &mut Canvas, x: i32, y: i32, color: Rgba| c.set(x - 8, y, color);
    for (x, y) in [(15, 8), (16, 8), (17, 8), (14, 9), (15, 9), (18, 9)] {
        set(c, x, y, WHITE);
    }
    for y in 10..=11 {
        for x in 13..=19 {
            set(c, x, y, WHITE);
        }
    }
    set(c, 16, 9, OUTLINE);
    set(c, 17, 9, OUTLINE);
    for x in [12, 13, 19, 20] {
        set(c, x, 8, BLUSH);
    }
    if state.expression == ExpressionKind::Yawning {
        set(c, 16, 10, MOUTH);
        set(c, 17, 10, MOUTH);
        set(c, 16, 11, MOUTH);
        set(c, 17, 11, MOUTH);
    }

    let gaze = (
        state.gaze.x.clamp(-1, 1) as i32,
        state.gaze.y.clamp(-1, 0) as i32,
    );
    let shut = state.eyelids == EyelidPose::Closed
        || matches!(
            state.expression,
            ExpressionKind::Sleepy | ExpressionKind::Yawning
        );
    let half = state.eyelids == EyelidPose::Half
        || matches!(
            state.expression,
            ExpressionKind::Bored | ExpressionKind::Grumpy | ExpressionKind::Smug
        );
    for eye in [13, 19] {
        let x = eye + gaze.0;
        let y = 6 + gaze.1;
        if shut {
            // Closed, the dash lies down along where the eye's foot was.
            set(c, x - 1, 7, OUTLINE);
            set(c, x, 7, OUTLINE);
        } else if matches!(
            state.expression,
            ExpressionKind::Joy | ExpressionKind::Affectionate
        ) {
            // Smiling with its eyes: a little arch over each.
            set(c, x - 1, y + 1, OUTLINE);
            set(c, x, y, OUTLINE);
            set(c, x + 1, y + 1, OUTLINE);
        } else if half {
            set(c, x, y + 1, OUTLINE);
        } else if state.expression == ExpressionKind::Startled {
            for dy in -1..=1 {
                set(c, x, y + dy, OUTLINE);
            }
        } else {
            set(c, x, y, OUTLINE);
            set(c, x, y + 1, OUTLINE);
        }
    }
}
