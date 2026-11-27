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
    /// The front foot drawn up and the back foot hanging, topping out onto a ledge.
    knee: i32,
    trail: i32,
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
            knee: pose.knee.clamp(0, 6),
            trail: pose.trail.clamp(0, 3),
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
        let y = at.ground.y + FOOT_ROW - step.abs() + if side == 0 { at.trail } else { -at.knee };
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
    let hanging = matches!(
        clip,
        BodyClip::Action(ActionKind::Dangle | ActionKind::ClimbWindow)
    );
    // Each paw in use, from its shoulder to its hand. The fox's forelegs are short, so a paw goes
    // the way a companion's does but only a stub's length; holding on to a ledge is the one reach
    // that has to arrive, and goes all the way.
    let limbs = modular::limbs(clip, frame, body, BodyPlan::Upright);
    // Pulling itself up onto a ledge, a paw has to arrive at the ledge too.
    let arrives = hanging || clip == BodyClip::Gesture(Gesture::Mantle);
    let arms: [Option<(PixelPoint, PixelPoint)>; 2] = std::array::from_fn(|side| {
        let Limb::Reach(hand) = limbs[side] else {
            return None;
        };
        let root = at.body(SHOULDERS[side].0, SHOULDERS[side].1);
        Some((root, if arrives { hand } else { stub(root, hand) }))
    });
    // A paw stretched up to a ledge goes up behind the cheek, which is wider than the body it
    // rises from; every other paw is in front.
    let behind = hanging;

    // The body, the paws raised behind the head, then the head over its top, then the ears:
    // pricked, they grow from their base, and drooped they fold down into it, so they stay joined
    // to the head either way.
    for (y, pixels) in BODY.iter().enumerate().skip(NECK_ROW + 1) {
        let p = at.body(0, y as i32);
        row(c, p.x, p.y, pixels);
    }
    if behind {
        for (root, hand) in arms.into_iter().flatten() {
            arm(c, root, hand);
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
    for (paw, reach) in [BACK_PAW, FRONT_PAW].into_iter().zip(arms) {
        match reach {
            None => {
                for (x, y, color) in paw {
                    let p = at.body(x, y);
                    c.set(p.x, p.y, color);
                }
            }
            Some((root, hand)) if !behind => arm(c, root, hand),
            Some(_) => {}
        }
    }

    let floor = at.ground.y + FOOT_ROW + 2;
    // What it holds is in the near forepaw: on its sooty end, carried out or folded on the belly.
    let hand = match arms[1] {
        Some((_, hand)) => hand,
        None => at.body(20, 17),
    };
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
        hand: Some(hand),
    }
}

// ---------------------------------------------------------------------------------------------
// Turned: the same fox side-on, walking and running along, and from behind, walking away.
// ---------------------------------------------------------------------------------------------

/// The fox's head side-on, facing right: the far ear behind the near one, its muzzle pointed and
/// white under the eye, outlined all round so it sits over the body as the head of the sketch
/// does. The eye is drawn on it.
const SIDE_HEAD: [&str; 14] = [
    "...........K......K...........",
    "...........KK....KWK..........",
    "..........KDDK..KWWDK.........",
    "..........KDDKKKWWDDK.........",
    "..........KOOOOOOOOOKK........",
    ".........KOOOOOOOOOOOOK.......",
    "........KOOOOOOOOOOOOOOK......",
    ".......KOOOOOOOOOOOOOOOOK.....",
    ".......KOOOOOOOOOOOOOOOOOKK...",
    ".......KOOOOOOOOOOOOOWWWWWKK..",
    ".......KOOOOOOOOOOWWWWWWWKK...",
    "........KOOOOOOOWWWWWWWKK.....",
    ".........KKKOOOWWWWWWKK.......",
    "............KKKKKKKK..........",
];

/// The last row of its ears.
const SIDE_EAR_ROWS: usize = 4;

/// The head's first row is drawn a row above the sketch's, its ears standing that much taller.
const SIDE_HEAD_TOP: i32 = -1;

/// Where the eye is on the head side-on: the top of a short upright dash.
const SIDE_EYE: (i32, i32) = (20, 6);

/// The fox's ears from behind: pointed, as they are seen from the front, their backs dark.
const BACK_EARS: [&str; 5] = [
    ".........K.............K",
    "........KDK...........KDK",
    "........KDDK.........KDDK",
    "........KDDDK.......KDDDK",
    "........KDDDKKKKKKKKKDDDK",
];

/// The fox's body side-on under the head, from row 10, with the white bib down its chest.
const SIDE_BODY: [&str; 11] = [
    "........KOOOOOOOOOOOK.........",
    ".......KOOOOOOOOOOOWWK........",
    ".....KKOOOOOOOOOOOWWWK........",
    "....KOOOOOOOOOOOOOWWWWK.......",
    "....KOOOOOOOOOOOOOWWWWK.......",
    "....KOOOOOOOOOOOOOOWWWK.......",
    "....KOOOOOOOOOOOOOOOOOK.......",
    "....KOOOOOOOOOOOOOOOOOK.......",
    ".....KOOOOOOOOOOOOOOOK........",
    "......KOOOOOOOOOOOOOK.........",
    ".......KKKKKKKKKKKKK..........",
];
const SIDE_BODY_ROW: i32 = 10;

/// The brush side-on, out behind and curling up to its white tip, from row 4.
const SIDE_TAIL: [&str; 12] = [
    "..KK....", ".KWWK...", "KWWWWK..", "KWWWOK..", "KOWOOK..", "KOOOOK..", "KOOOOOK.", ".KOOOOK.",
    ".KOOOOOK", "..KOOOOK", "...KOOOO", "....KOOO",
];
const SIDE_TAIL_ROW: i32 = 4;

/// A foot side-on, toes forward: the near one at column 13, the far one at column 9.
const SIDE_FOOT: [&str; 3] = ["KOOOK.", "KDDDDK", ".KKKK."];
const SIDE_FOOT_ROW: i32 = 20;

/// The folded forepaw on the chest side-on, each pixel in its place.
const SIDE_PAW: [(i32, i32, Rgba); 6] = [
    (18, 15, OUTLINE),
    (19, 15, SOOT),
    (20, 15, SOOT),
    (21, 15, OUTLINE),
    (19, 16, OUTLINE),
    (20, 16, OUTLINE),
];

/// Where the side-on sketch's top-left corner sits in a frame: its feet on row 41.
const SIDE_ORIGIN: (i32, i32) = (9, 19);

/// The brush from behind: standing up the middle of the back toward whoever is looking, its
/// white tip on top.
const BACK_TAIL: [&str; 12] = [
    "...KKK...",
    "..KWWWK..",
    ".KWWWWWK.",
    ".KWWWWWK.",
    "KOWWWWWOK",
    "KOOOWOOOK",
    "KOOOOOOOK",
    "KOOOOOOOK",
    ".KOOOOOK.",
    ".KOOOOOK.",
    "..KOOOK..",
    "...KOK...",
];
const BACK_TAIL_ROW: i32 = 9;

/// A heel from behind, and the sole of a foot that is up.
const HEEL: [&str; 2] = ["KOOOK", ".KKK."];
const SOLE: [&str; 2] = ["KDDDK", ".KKK."];

/// One frame of a turned walk or run, as `modular::turned` works it out for every body: how far
/// each foot is from where it stands and how high off the ground, and what the body does over
/// them.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Step {
    pub(super) near_x: i32,
    pub(super) near_lift: i32,
    pub(super) far_x: i32,
    pub(super) far_lift: i32,
    pub(super) bob: i32,
    pub(super) lean: i32,
    pub(super) tail: i32,
    pub(super) ears_back: i32,
    pub(super) sway: i32,
}

/// Paints a part drawn on the far side of the body, a third of the way to the outline.
fn far_row(c: &mut Canvas, x: i32, y: i32, pixels: &str) {
    let mix = |a: u8, b: u8| ((u16::from(a) * 2 + u16::from(b)) / 3) as u8;
    for (dx, key) in pixels.bytes().enumerate() {
        if let Some(color) = color(key) {
            let color = if color == OUTLINE {
                color
            } else {
                Rgba::new(
                    mix(color.r, OUTLINE.r),
                    mix(color.g, OUTLINE.g),
                    mix(color.b, OUTLINE.b),
                    255,
                )
            };
            c.set(x + dx as i32, y, color);
        }
    }
}

/// The fox turned to `view` for one frame of a walk, a run or standing still, facing right, and
/// where anything it wears goes. Side-on its face is drawn in: the one eye showing, shut when it
/// blinks. From behind it shows no face at all.
pub(super) fn draw_turned(
    c: &mut Canvas,
    view: modular::View,
    step: Step,
    face: FaceRenderState,
) -> Figure {
    match view {
        modular::View::Back => fox_from_behind(c, step),
        _ => fox_side_on(c, step, face),
    }
}

fn fox_side_on(c: &mut Canvas, step: Step, face: FaceRenderState) -> Figure {
    let (ox, oy) = SIDE_ORIGIN;
    // The body sinks onto a planted foot; the head goes ahead of it in a run.
    let sink = step.bob.clamp(-1, 1);
    let lean = (step.lean / 2).clamp(0, 1);
    let ears_down = step.ears_back.clamp(0, 1);
    let tail = step.tail.clamp(-1, 2);
    // The brush first, behind everything, lifting a frame after the body.
    for (dy, pixels) in SIDE_TAIL.iter().enumerate() {
        row(c, ox, oy + sink + SIDE_TAIL_ROW - tail + dy as i32, pixels);
    }
    // The far foot, in shade behind the body.
    let foot = |column: i32, along: i32, lift: i32| PixelPoint {
        x: ox + column + along.clamp(-5, 5),
        y: oy + SIDE_FOOT_ROW - lift.clamp(0, 3),
    };
    let far = foot(9, step.far_x, step.far_lift);
    for (dy, pixels) in SIDE_FOOT.iter().enumerate() {
        far_row(c, far.x, far.y + dy as i32, pixels);
    }
    for (dy, pixels) in SIDE_BODY.iter().enumerate() {
        row(c, ox, oy + sink + SIDE_BODY_ROW + dy as i32, pixels);
    }
    // The head over the top of the body, its ears laid back into it in a run.
    if ears_down > 0 {
        for (y, pixels) in SIDE_HEAD.iter().enumerate().take(SIDE_EAR_ROWS) {
            row(
                c,
                ox + lean,
                oy + sink + SIDE_HEAD_TOP + y as i32 + ears_down,
                pixels,
            );
        }
    }
    let first = if ears_down > 0 { SIDE_EAR_ROWS } else { 0 };
    for (y, pixels) in SIDE_HEAD.iter().enumerate().skip(first) {
        row(c, ox + lean, oy + sink + SIDE_HEAD_TOP + y as i32, pixels);
    }
    // The eye: a short dash, lying down when it is shut, and a blush under it.
    let eye = PixelPoint {
        x: ox + lean + SIDE_EYE.0,
        y: oy + sink + SIDE_EYE.1,
    };
    let shut = face.eyelids == EyelidPose::Closed
        || matches!(
            face.expression,
            ExpressionKind::Sleepy | ExpressionKind::Yawning
        );
    if shut {
        c.set(eye.x - 1, eye.y + 1, OUTLINE);
        c.set(eye.x, eye.y + 1, OUTLINE);
    } else {
        c.set(eye.x, eye.y, OUTLINE);
        c.set(eye.x, eye.y + 1, OUTLINE);
    }
    c.set(eye.x - 2, eye.y + 2, BLUSH);
    c.set(eye.x - 3, eye.y + 2, BLUSH);
    // The near foot, over the body, and the forepaw folded on the chest, swung a pixel against it.
    let near = foot(13, step.near_x, step.near_lift);
    for (dy, pixels) in SIDE_FOOT.iter().enumerate() {
        row(c, near.x, near.y + dy as i32, pixels);
    }
    let swing = -step.near_x.signum();
    for (x, y, color) in SIDE_PAW {
        c.set(ox + x + swing, oy + sink + y, color);
    }
    let at = |x: i32, y: i32| PixelPoint {
        x: ox + x,
        y: oy + sink + y,
    };
    Figure {
        face: PixelPoint {
            x: ox + lean + 19,
            y: oy + sink + 7,
        },
        crown: PixelPoint {
            x: ox + lean + 16,
            y: oy + sink + 3,
        },
        head_half: 8,
        neck: at(15, 11),
        neck_half: 5,
        chest: at(19, 13),
        hip: at(7, 16),
        back: at(10, 12),
        floor: oy + SIDE_FOOT_ROW + 2,
        hand: Some(at(19 + swing, 15)),
    }
}

fn fox_from_behind(c: &mut Canvas, step: Step) -> Figure {
    let at = |x: i32, y: i32| PixelPoint {
        x: ORIGIN_X + step.sway.clamp(-1, 1) + x,
        y: ORIGIN_Y + step.bob.clamp(-1, 1) + y,
    };
    // The heels, and the sole of a foot that is up, under the body.
    for (column, lift) in [(9, step.far_lift), (19, step.near_lift)] {
        let lift = lift.clamp(0, 3);
        let foot = if lift > 0 { SOLE } else { HEEL };
        for (dy, pixels) in foot.iter().enumerate() {
            row(
                c,
                ORIGIN_X + column,
                ORIGIN_Y + FOOT_ROW + 1 - lift + dy as i32,
                pixels,
            );
        }
    }
    // The back of it: the body and head in its coat, the ears showing their dark backs.
    let ears_down = step.ears_back.clamp(0, 1);
    for (y, pixels) in BACK_EARS.iter().enumerate() {
        let p = at(0, y as i32 - 1 + ears_down);
        row(c, p.x, p.y, pixels);
    }
    for (y, pixels) in BODY.iter().enumerate().skip(EAR_ROWS + 1) {
        let back = pixels.replace('W', "O");
        let p = at(0, y as i32);
        row(c, p.x, p.y, &back);
    }
    // The brush up the middle of the back, swinging across behind it.
    // Out past its side and curling up, as it is side-on, its white tip uppermost.
    let tail = at(21 + step.tail.clamp(-1, 1), BACK_TAIL_ROW);
    for (dy, pixels) in BACK_TAIL.iter().enumerate() {
        row(c, tail.x, tail.y + dy as i32, pixels);
    }
    Figure {
        face: at(FACE.0, FACE.1),
        crown: at(FACE.0, EAR_ROWS as i32 + 1),
        head_half: 9,
        neck: at(FACE.0, NECK_ROW as i32),
        neck_half: 6,
        chest: at(FACE.0, 13),
        hip: at(9, 18),
        back: at(FACE.0, 15),
        floor: ORIGIN_Y + FOOT_ROW + 2,
        hand: None,
    }
}

/// How far a forepaw reaches from its shoulder, in art pixels.
const REACH: f32 = 6.0;

/// `hand`, brought in along the way to it until it is no more than [`REACH`] from `root`.
fn stub(root: PixelPoint, hand: PixelPoint) -> PixelPoint {
    let (dx, dy) = ((hand.x - root.x) as f32, (hand.y - root.y) as f32);
    let length = dx.hypot(dy);
    if length <= REACH {
        return hand;
    }
    let scale = REACH / length;
    PixelPoint {
        x: root.x + (dx * scale).round() as i32,
        y: root.y + (dy * scale).round() as i32,
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

/// Where the middle of the fox's mouth is against its face anchor: under the nose on its white
/// muzzle, where a yawn opens.
pub(super) const MOUTH_AT: PixelPoint = PixelPoint { x: 0, y: 2 };

/// How low what the fox holds may go: the floor under its feet, and the lowest row they reach.
pub(super) fn footing(pose: Pose, clip: BodyClip) -> (i32, i32) {
    let at = Placement::new(pose, clip);
    (at.ground.y + FOOT_ROW + 1, at.ground.y + FOOT_ROW + 2)
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
