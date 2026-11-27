//! A companion turned side-on, or turned away: the same body its recipe or its genes draw facing
//! the screen, measured the same way and put together from the same parts, seen from another
//! side. A walk and a run seen side-on and a walk away are baked into every atlas as the
//! [`Gesture::Stride`], [`Gesture::Dash`] and [`Gesture::Depart`] clips, which the app shows from
//! which way a companion's feet are carrying it; `formiga-tools turn-sheet` draws them, and the
//! standing views, to be judged.
//!
//! A body facing the screen is already turned a little to its right: its tail is out behind it on
//! the left and its face sits right of the middle. Side-on it faces right the whole way, one eye
//! showing; turned away it shows its back, its tail over it, and no face.
use super::*;

/// Which way a companion is turned to whoever is looking at it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// Facing the screen, as every baked frame is.
    Front,
    /// Side-on, facing the way it is going.
    Side,
    /// Turned away, its back to the screen.
    Back,
}

/// How many frames a turned walk takes to come round.
pub const TURNED_STRIDE_FRAMES: u8 = 6;

/// How a turned body is getting along.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::renderer) enum Gait {
    Walk,
    Run,
}

impl Gait {
    /// The clip a body nothing draws turned shows in place of a turned one.
    pub(in crate::renderer) const fn facing_the_screen(self) -> BodyClip {
        BodyClip::Action(match self {
            Self::Walk => ActionKind::Traverse,
            Self::Run => ActionKind::Sprint,
        })
    }
}

/// Which way, and how, a turned clip shows a body going.
pub(in crate::renderer) const fn turned_clip(clip: BodyClip) -> Option<(View, Gait)> {
    match clip {
        BodyClip::Gesture(Gesture::Stride) => Some((View::Side, Gait::Walk)),
        BodyClip::Gesture(Gesture::Dash) => Some((View::Side, Gait::Run)),
        BodyClip::Gesture(Gesture::Depart) => Some((View::Back, Gait::Walk)),
        _ => None,
    }
}

/// Whether a turned frame has a foot just down: the two contacts of its six-frame stride.
pub(in crate::renderer) const fn turned_footfall(frame: u8) -> bool {
    (frame % TURNED_STRIDE_FRAMES).is_multiple_of(3)
}

/// Whether anything draws this body turned: an original blob, and every recipe body but the
/// four-pawed one. A hopper, a soft quadruped, the fox and a sculpted form walk facing the screen
/// as they always have.
pub fn can_turn(genome: &AppearanceGenome) -> bool {
    if genome.echo_fox || genome.sculpt.is_some() {
        return false;
    }
    match genome.design {
        Some(design) => design.bounded().body != BodyPlan::Long,
        None => genome.family == BodyFamily::Blob,
    }
}

/// One frame of a turned walk or run, or of standing still: how far each foot is from where it
/// stands and how high off the ground, and what the body does over them. Side-on the two feet
/// are the near and the far one, and go forward and back; turned away they are the right and the
/// left, and only come up.
#[derive(Clone, Copy, Default)]
struct Stride {
    near_x: i32,
    near_lift: i32,
    far_x: i32,
    far_lift: i32,
    /// Rows the body presses down as its weight lands; less than nought, rows it comes up off
    /// its feet between them.
    bob: i32,
    /// Pixels the body goes over onto the planted foot, seen from behind.
    sway: i32,
    /// Pixels the head is carried ahead of where it rides standing: leant into a run.
    lean: i32,
    /// What trails behind does a frame late: the tail, and the ears laid back or pricked.
    tail: i32,
    ears_back: i32,
    ear_perk: i32,
    /// Up against a wall on the side it faces rather than walking: where its paws and feet hold
    /// it.
    cling: Option<Cling>,
}

/// How far above its crown a paw reaches for a ledge at the most, in rows: short arms, so a ledge
/// further up than this is not held yet and the paws stay on the wall under it.
const CLING_REACH: i32 = 3;

/// A body up against a wall, climbing it or topping out over the ledge at the top of it: where
/// its paws and feet hold. Side-on, the wall is on the side it faces, its edge line the column
/// [`CLIMB_WALL`] of the frame, and the paws and feet are the near one and the far. With its back
/// to the screen the wall is what it is on, and they are the right one and the left.
///
/// Rows are measured from the body as it hangs, before a stretch or a lift carries it up, so a
/// paw or a foot that keeps its row stays where it gripped.
#[derive(Clone, Copy)]
struct Cling {
    /// How many rows above its shoulder each paw holds: less than nought, below it.
    paws: [i32; 2],
    /// How many pixels each paw is off the wall, come up off it to reach for its next hold.
    off: [i32; 2],
    /// How many rows up the wall from where it would stand each foot is set.
    feet: [i32; 2],
    /// Taller and narrower by this many pixels, stretched up to a catch; less than nought,
    /// bunched up on a pull.
    stretch: i32,
    /// Rows the body is carried up past where it hangs.
    lift: i32,
    /// How far below a ledge its feet are, in rows, once the top of the wall is that near. Paws
    /// that can reach the ledge go on top of it, and none holds the wall above it.
    drop: Option<i32>,
    /// Side-on: pixels the body is carried over the edge line, its belly on the corner of the
    /// ledge. No further than keeps its face inside the box the simulation spaces faces by.
    over: i32,
    /// Side-on: the feet off the wall and kicking in the air behind.
    kicking: bool,
    /// Back to the screen: its right knee hooked up, that foot on the ledge beside it.
    knee: bool,
}

impl Cling {
    /// The row of the frame the ledge is on.
    fn ledge(self) -> Option<i32> {
        self.drop.map(|drop| FRAME_SIZE as i32 - 5 - drop)
    }

    /// The row a paw holds at, for a body whose shoulder and crown are on those rows as it
    /// hangs, and whether that is on top of the ledge rather than on the wall.
    fn paw_row(self, which: usize, shoulder: i32, crown: i32) -> (i32, bool) {
        let row = shoulder - self.paws[which];
        match self.ledge() {
            Some(ledge) if ledge >= crown - CLING_REACH => (ledge - 2, true),
            Some(ledge) => (row.max(ledge + 2), false),
            None => (row, false),
        }
    }

    /// Side-on: where a paw holds, the near one or the far.
    fn paw(self, far: bool, shoulder: i32, crown: i32) -> PixelPoint {
        let which = usize::from(far);
        // Side-on its arms go out from its chest, so what it can reach is measured from there:
        // no higher than its crown, and no further than a short arm goes.
        let (y, on_ledge) = self.paw_row(which, shoulder, crown.max(shoulder - 4));
        let x = if on_ledge {
            // The far paw further in along the ledge.
            CLIMB_WALL + (self.over - 1).max(1) + 2 * which as i32
        } else {
            CLIMB_WALL - 1 - self.off[which]
        };
        PixelPoint { x, y }
    }

    /// Side-on: where a foot is, the near one or the far, for a body that would stand on the
    /// row `stand` and whose rump is at the column `rump`.
    fn foot(self, far: bool, stand: i32, rump: i32) -> PixelPoint {
        let step = self.feet[usize::from(far)];
        if self.kicking {
            PixelPoint {
                x: rump - step,
                y: stand + 1 - i32::from(far),
            }
        } else {
            // Lower than it would stand: there is no ground under a climber's feet.
            PixelPoint {
                x: CLIMB_WALL - 2,
                y: stand + 1 - step,
            }
        }
    }

    /// One frame of a climb: a stroke with one paw, frames 0 to 2, and then the same with the
    /// other. A stroke is a catch, the paw landed `top` rows over the shoulder and the body
    /// stretched up after it; a pull, the body bunched up and hauled past the paw; and a lock,
    /// the paw down under the shoulder while the other comes up off the wall for its own catch.
    /// The world raises the frame as each of these comes (see `formiga_core::CLIMB_FRAME_RISE`)
    /// and whatever holds the wall is drawn lower by as much, so it stays where it gripped. The
    /// feet go up against the paws: the one across from the catching paw is planted high, pushes
    /// straight through the pull, and steps up again as the other plants.
    fn climbing(frame: u8, top: i32) -> Self {
        let rise = formiga_core::CLIMB_FRAME_RISE.map(i32::from);
        let whole: i32 = rise.iter().sum();
        let lift = crate::renderer::pose::CLIMB_LIFT[usize::from(frame % 3)];
        // Each paw's rows over the shoulder and pixels off the wall; the stretch; and the feet,
        // the one on the catching paw's side first.
        let (lead, other, stretch, feet) = match frame % 3 {
            0 => ((top, 0), (top - whole, 0), 1, [0, 3]),
            1 => ((top - rise[1], 0), (top - whole + 4, 2), -1, [2, -1]),
            _ => ((top - rise[1] - rise[2], 0), (top - 1, 1), 0, [4, -1]),
        };
        let second = frame % 6 >= 3;
        let (first, last) = if second { (other, lead) } else { (lead, other) };
        Self {
            paws: [first.0, last.0],
            off: [first.1, last.1],
            feet: if second { [feet[1], feet[0]] } else { feet },
            stretch,
            lift,
            drop: None,
            over: 0,
            kicking: false,
            knee: false,
        }
    }

    /// One of a mantle's frames beside the window, side-on, for a ledge `drop` rows above the
    /// feet: hung under it or still climbing to it, pulled up on it, and flopped over its corner
    /// on its belly with its feet kicking behind.
    fn topping_out(frame: u8, drop: i32) -> Self {
        let hold = Self {
            paws: [4, 1],
            off: [0, 0],
            feet: [0, 2],
            stretch: 1,
            lift: 0,
            drop: Some(drop),
            over: 0,
            kicking: false,
            knee: false,
        };
        match frame {
            0 => hold,
            1 => Self {
                paws: [2, 4],
                feet: [3, 0],
                stretch: -1,
                lift: 1,
                over: 2,
                ..hold
            },
            _ => Self {
                paws: [0, 0],
                feet: [3, 6],
                stretch: -1,
                over: 3,
                kicking: true,
                ..hold
            },
        }
    }

    /// The same frames with its back to the screen, on the window's face: reaching for the
    /// ledge, pulled up with a paw on it either side, and pressed up on them with a knee hooked
    /// over.
    fn topping_out_back(frame: u8, drop: i32) -> Self {
        let hold = Self {
            paws: [6, 3],
            off: [0, 0],
            feet: [0, 3],
            stretch: 1,
            lift: 0,
            drop: Some(drop),
            over: 0,
            kicking: false,
            knee: false,
        };
        match frame {
            0 => hold,
            1 => Self {
                paws: [4, 6],
                feet: [3, 0],
                stretch: -1,
                lift: 1,
                ..hold
            },
            _ => Self {
                paws: [0, 0],
                feet: [0, 0],
                stretch: 0,
                knee: true,
                ..hold
            },
        }
    }
}

/// The frames of a climb and of the mantle it tops out with that a body which [`can_turn`] is
/// drawn turned for: all of the climb, and the mantle until it is up on the ledge and faces the
/// screen again.
pub(in crate::renderer) fn clings(clip: BodyClip, frame: u8) -> bool {
    match clip {
        BodyClip::Action(ActionKind::ClimbWindow) => true,
        BodyClip::Gesture(Gesture::Mantle) => frame <= formiga_core::MANTLE_PULL_FRAMES + 1,
        _ => false,
    }
}

/// The face a climb and a mantle are baked with (see `BodyClip::face_in_frame`): set on it and
/// looking up the wall to where it is going, and straight ahead once it is over the top.
pub(in crate::renderer) fn climbing_face(clip: BodyClip, frame: u8) -> FaceRenderState {
    let up = match clip {
        BodyClip::Action(ActionKind::ClimbWindow) => true,
        _ => frame < formiga_core::MANTLE_PULL_FRAMES,
    };
    let stood =
        clip == BodyClip::Gesture(Gesture::Mantle) && frame > formiga_core::MANTLE_PULL_FRAMES + 1;
    FaceRenderState {
        expression: if stood {
            ExpressionKind::Content
        } else {
            ExpressionKind::Determined
        },
        eyelids: EyelidPose::Open,
        gaze: GazeDirection {
            x: i8::from(!stood),
            y: -i8::from(up),
        },
    }
}

/// A frame of a climb or of the start of a mantle (see [`clings`]) for a body that [`can_turn`],
/// its face drawn in where it shows one: side-on, facing right with the wall's edge line on the
/// column [`CLIMB_WALL`]; or with its `back` to the screen, in the middle of its frame.
pub(in crate::renderer) fn clinging_body(
    genome: &AppearanceGenome,
    clip: BodyClip,
    frame: u8,
    back: bool,
) -> (Canvas, Figure) {
    let view = if back { View::Back } else { View::Side };
    let (view, stride) = match clip {
        BodyClip::Action(_) => (
            view,
            Stride {
                tail: [-1, 1, 0, 1, -1, 0][usize::from(frame % 6)],
                // Its hips go over to the side whose paw is reaching.
                sway: if back {
                    [1, 0, 0, -1, 0, 0][usize::from(frame % 6)]
                } else {
                    0
                },
                // Side-on its head is tipped back to look up the wall.
                lean: -i32::from(!back),
                ear_perk: i32::from(frame.is_multiple_of(3)),
                ears_back: i32::from(frame % 3 == 1),
                cling: Some(Cling::climbing(frame, if back { 7 } else { 4 })),
                ..Stride::default()
            },
        ),
        // Up on the ledge. Having climbed its face it is half turned round, crouched; over its
        // corner, it scrambles in on its belly, low over a near foot planted under it, the far
        // one still trailing off the edge behind.
        _ if frame > formiga_core::MANTLE_PULL_FRAMES => (
            View::Side,
            if back {
                Stride {
                    near_x: 1,
                    far_x: -1,
                    bob: 2,
                    ears_back: 1,
                    ..Stride::default()
                }
            } else {
                Stride {
                    near_x: 3,
                    far_x: -7,
                    far_lift: -2,
                    bob: 2,
                    lean: 1,
                    tail: 1,
                    ears_back: 1,
                    ..Stride::default()
                }
            },
        ),
        _ => {
            let drops = formiga_core::mantle_frame_drop(genome);
            let drop = i32::from(drops[usize::from(frame).min(drops.len() - 1)]);
            (
                view,
                Stride {
                    tail: [-1, 1, 1][usize::from(frame.min(2))],
                    ear_perk: i32::from(frame == 1),
                    lean: -i32::from(!back && frame == 0),
                    cling: Some(if back {
                        Cling::topping_out_back(frame, drop)
                    } else {
                        Cling::topping_out(frame, drop)
                    }),
                    ..Stride::default()
                },
            )
        }
    };
    draw_turned(genome, view, stride, climbing_face(clip, frame))
}

impl Stride {
    fn of(view: View, step: Option<(Gait, u8)>) -> Self {
        let Some((gait, step)) = step else {
            return match view {
                // Standing side-on, the far foot is a little ahead of the near one, so both show.
                View::Side => Self {
                    near_x: -1,
                    far_x: 2,
                    ..Self::default()
                },
                _ => Self::default(),
            };
        };
        let at = usize::from(step % TURNED_STRIDE_FRAMES);
        match (view, gait) {
            // The near foot is planted and carried back under the body for three frames while
            // the far one comes forward through the air, and then the other way about. The body
            // sinks as each foot takes its weight and comes back up over it; the tail lifts and
            // the ears prick a frame after the body does, and drop a frame after it lands.
            (View::Side, Gait::Walk) => Self {
                near_x: [3, 1, -1, -3, -1, 2][at],
                near_lift: [0, 0, 0, 0, 2, 1][at],
                far_x: [-3, -1, 2, 3, 1, -1][at],
                far_lift: [0, 2, 1, 0, 0, 0][at],
                bob: [1, 0, 0, 1, 0, 0][at],
                tail: [0, 1, 0, 0, 1, 0][at],
                ear_perk: [0, 0, 1, 0, 0, 1][at],
                ..Self::default()
            },
            // A run throws each foot out ahead and well behind, with both off the ground as one
            // passes the other; the body is leant into it, its ears laid back and its tail out.
            (View::Side, Gait::Run) => Self {
                near_x: [5, 2, -2, -5, -2, 3][at],
                near_lift: [0, 0, 1, 2, 3, 2][at],
                far_x: [-5, -2, 3, 5, 2, -2][at],
                far_lift: [2, 3, 2, 0, 0, 1][at],
                bob: [1, 0, -1, 1, 0, -1][at],
                lean: 2,
                tail: [1, 2, 1, 1, 2, 1][at],
                ears_back: [1, 2, 1, 1, 2, 1][at],
                ..Self::default()
            },
            // Going away it waddles: each foot up in turn, showing its sole, the body over the
            // other one, the tail swinging across behind it.
            _ => Self {
                near_lift: [0, 0, 0, 0, 2, 1][at],
                far_lift: [0, 2, 1, 0, 0, 0][at],
                bob: [1, 0, 0, 1, 0, 0][at],
                sway: [0, 1, 1, 0, -1, -1][at],
                tail: [0, 1, 1, 0, -1, -1][at],
                ear_perk: [0, 0, 1, 0, 0, 1][at],
                ..Self::default()
            },
        }
    }
}

/// The mass a face is carried on: a head, or the whole of a body that carries its own face.
#[derive(Clone, Copy)]
struct Mass {
    x: i32,
    y: i32,
    rx: i32,
    ry: i32,
    /// The row a face on it is centred on.
    face_y: i32,
}

impl Mass {
    /// The last column of the mass on `row`, on the side it faces.
    fn front(self, row: i32) -> i32 {
        self.x + ellipse_half_width(self.rx, self.ry, row - self.y)
    }
}

impl CreatureRenderer {
    /// One whole frame of a companion turned to `view`, standing or on frame `step` of a walk:
    /// `None` for a body nothing draws turned (see [`can_turn`]). Facing the screen it is the frame
    /// the app has always drawn.
    pub fn render_turned_frame(
        genome: &AppearanceGenome,
        view: View,
        step: Option<u8>,
        facing_right: bool,
        face: FaceRenderState,
    ) -> Option<Canvas> {
        if view == View::Front {
            let (clip, frame) = match step {
                Some(step) => {
                    let frames = AnimationSpec::for_clip(ActionKind::Traverse).frames;
                    (ActionKind::Traverse, step % frames)
                }
                None => (ActionKind::Idle, 0),
            };
            return Some(Self::render_composited_frame(
                genome,
                clip,
                frame,
                facing_right,
                step.is_none(),
                face,
            ));
        }
        if !can_turn(genome) {
            return None;
        }
        let stride = Stride::of(view, step.map(|step| (Gait::Walk, step)));
        let (mut canvas, _) = draw_turned(genome, view, stride, face);
        if !facing_right {
            canvas.mirror_horizontal();
        }
        Some(canvas)
    }
}

/// The face a turned clip is baked with: the overlay draws none over it, so it does not blink or
/// change with what the companion feels while it walks turned.
pub(in crate::renderer) const TURNED_FACE: FaceRenderState = FaceRenderState {
    expression: ExpressionKind::Neutral,
    eyelids: EyelidPose::Open,
    gaze: GazeDirection { x: 0, y: 0 },
};

/// A frame of a turned clip for a body that [`can_turn`], facing right, its face drawn in: the body
/// and where its parts are, for whatever it wears and holds.
pub(in crate::renderer) fn turned_body(
    genome: &AppearanceGenome,
    view: View,
    gait: Gait,
    frame: u8,
) -> (Canvas, Figure) {
    draw_turned(
        genome,
        view,
        Stride::of(view, Some((gait, frame))),
        TURNED_FACE,
    )
}

fn draw_turned(
    genome: &AppearanceGenome,
    view: View,
    stride: Stride,
    face: FaceRenderState,
) -> (Canvas, Figure) {
    let palette = crate::palette_for(genome);
    let mut canvas = Canvas::new(FRAME_SIZE, FRAME_SIZE);
    let (mass, figure) = match genome.design.map(CreatureDesign::bounded) {
        Some(design) => recipe_turned(&mut canvas, design, palette, genome, view, stride),
        None => blob_turned(&mut canvas, genome, palette, view, stride),
    };
    if view == View::Side {
        profile_face(&mut canvas, genome, mass, face);
    }
    (canvas, figure)
}

/// A body standing still, to be moved by a [`Stride`].
fn standing(genome: &AppearanceGenome, stride: Stride) -> Pose {
    let mut pose = Pose::new(genome, BodyClip::Action(ActionKind::Idle), 0, true);
    pose.bob = stride.bob;
    pose.sway = stride.sway;
    pose.tail_sway = stride.tail;
    pose.ears_back = stride.ears_back;
    pose.ear_perk = stride.ear_perk;
    pose
}

/// `draw`n on its own and put down a third of the way to the outline's colour, the outline
/// itself left as it is: something on the far side of a body, in its shade.
fn shaded(c: &mut Canvas, p: Palette, draw: impl FnOnce(&mut Canvas)) {
    let mut layer = Canvas::new(FRAME_SIZE, FRAME_SIZE);
    draw(&mut layer);
    let mix = |a: u8, b: u8| ((u16::from(a) * 2 + u16::from(b)) / 3) as u8;
    for y in 0..FRAME_SIZE as i32 {
        for x in 0..FRAME_SIZE as i32 {
            let pixel = layer.get(x, y);
            if pixel.a == 0 {
                continue;
            }
            let pixel = if pixel == p.outline {
                pixel
            } else {
                Rgba::new(
                    mix(pixel.r, p.outline.r),
                    mix(pixel.g, p.outline.g),
                    mix(pixel.b, p.outline.b),
                    pixel.a,
                )
            };
            c.set(x, y, pixel);
        }
    }
}

/// `draw`n on its own and put down only inside the body's coat, never over its outline or past
/// it: a marking or a belly on a body narrower than the one it was placed for.
fn inside(c: &mut Canvas, p: Palette, body: (i32, i32, i32, i32), draw: impl FnOnce(&mut Canvas)) {
    let (x, y, rx, ry) = body;
    let mut layer = Canvas::new(FRAME_SIZE, FRAME_SIZE);
    draw(&mut layer);
    for row in 0..FRAME_SIZE as i32 {
        for column in 0..FRAME_SIZE as i32 {
            let pixel = layer.get(column, row);
            let under = c.get(column, row);
            if pixel.a > 0
                && under.a > 0
                && under != p.outline
                && inside_ellipse(column, row, x, y, rx, ry)
            {
                c.set(column, row, pixel);
            }
        }
    }
}

/// The face side-on: the eye on the side it faces, as its face draws it with its brow, lid and
/// cheek, set just inside the front of the head; and the half of its mouth on that side, turned
/// so the middle of it is at the very front and its corner runs back along the cheek. Every
/// expression a face has is therefore one it has side-on.
fn profile_face(c: &mut Canvas, genome: &AppearanceGenome, mass: Mass, state: FaceRenderState) {
    let cell = CreatureRenderer::render_face_frame(genome, state);
    let middle = FACE_FRAME_SIZE as i32 / 2;
    // The face is drawn about row 7 of its cell, and the cell is centred on the anchor.
    let top = mass.face_y - middle;
    let mouth_row = 7 + mouth_offset(genome).y + 1;
    let size = FACE_FRAME_SIZE as i32;
    // The mouth is drawn about the middle of the face, under the eyes; a cheek mark under an
    // eye belongs with the eye.
    let mouth = |column: i32, row: i32| row >= mouth_row - 1 && column <= middle + 2;
    let half = || (0..size).flat_map(move |row| (middle..size).map(move |column| (column, row)));
    let last = half()
        .filter(|&(column, row)| !mouth(column, row) && cell.get(column, row).a > 0)
        .map(|(column, _)| column)
        .max();
    let shift = last.map_or(0, |last| mass.front(mass.face_y - 1) - 2 - last);
    for (column, row) in half() {
        let pixel = cell.get(column, row);
        if pixel.a == 0 {
            continue;
        }
        let x = if mouth(column, row) {
            mass.front(top + row) - 2 - (column - middle)
        } else {
            column + shift
        };
        if c.get(x, top + row).a > 0 {
            c.set(x, top + row, pixel);
        }
    }
}

/// One of an original blob's two ears or antennae, rooted at `x` and swept a little back.
fn blob_ear(c: &mut Canvas, genome: &AppearanceGenome, p: Palette, x: i32, root_y: i32, size: i32) {
    match genome.head_appendages.style {
        HeadAppendageStyle::None => {}
        HeadAppendageStyle::Round => {
            c.fill_circle(x, root_y, size, p.outline);
            c.fill_circle(x, root_y, size - 1, p.accent);
        }
        HeadAppendageStyle::Pointed | HeadAppendageStyle::Leaf => {
            c.line(x, root_y + 2, x - 2, root_y - size, 2, p.outline);
            c.line(x, root_y + 1, x - 2, root_y - size + 1, 1, p.accent);
        }
        HeadAppendageStyle::Droop => {
            c.line(x, root_y, x - 4, root_y + size, 2, p.outline);
            c.line(x, root_y, x - 4, root_y + size - 1, 1, p.accent);
        }
        HeadAppendageStyle::Antenna => {
            c.line(x, root_y + 1, x - 1, root_y - size, 1, p.outline);
            c.fill_circle(x - 1, root_y - size, 1, p.accent);
        }
    }
}

/// One of an original blob's feet, toes the way it faces.
fn blob_foot(c: &mut Canvas, p: Palette, x: i32, y: i32, foot: i32) {
    c.fill_ellipse(x, y, foot, 2, p.outline);
    c.fill_ellipse(x + 1, y, (foot - 1).max(1), 1, p.accent);
}

/// An original blob's tail seen from behind: short, coming toward whoever is looking, over the
/// body it grows from.
fn blob_back_tail(
    c: &mut Canvas,
    genome: &AppearanceGenome,
    p: Palette,
    x: i32,
    y: i32,
    sway: i32,
) {
    match genome.tail_style {
        TailStyle::None => {}
        TailStyle::Stub => c.fill_circle(x, y, 2, p.outline),
        TailStyle::Taper => {
            c.line(x, y + 2, x - 2 + sway, y - 2, 2, p.outline);
            c.line(x, y + 2, x - 2 + sway, y - 2, 1, p.coat);
        }
        TailStyle::Tuft => {
            c.line(x, y + 2, x - 1, y, 2, p.outline);
            c.fill_circle(x - 1 + sway, y - 1, 3, p.outline);
            c.fill_circle(x - 1 + sway, y - 1, 2, p.accent);
        }
        TailStyle::Curl => {
            c.line(x, y + 3, x - 1, y + 1, 2, p.outline);
            c.fill_circle(x - 1 + sway, y - 1, 3, p.outline);
            c.fill_circle(x - 1 + sway, y - 1, 1, p.shadow);
        }
    }
}

/// An original blob (see `classic::draw_blob`) side-on or turned away.
fn blob_turned(
    c: &mut Canvas,
    genome: &AppearanceGenome,
    p: Palette,
    view: View,
    stride: Stride,
) -> (Mass, Figure) {
    let s = scale(genome);
    let pose = standing(genome, stride);
    let cling = stride.cling;
    let (stretch, lift) = cling.map_or((0, 0), |cling| (cling.stretch, cling.lift));
    let wide = ((genome.body_width as f32 * s / 2.0).round() as i32).clamp(6, 16);
    // As facing the screen, a blob presses down onto feet that stay where they are.
    let ry =
        ((genome.body_height as f32 * s / 2.0).round() as i32 - stride.bob).clamp(5, 14) + stretch;
    // Where it stands, and its middle over that: carried up past it on a pull. Stretched, it is
    // longer above its middle and below it, hanging down as far as it reaches up.
    let stand = 38;
    let cy = stand - (ry - stretch) - lift;
    let root_y = cy - ry + 2;
    let ground = cy + ry;
    let foot = i32::from(genome.foot_size);
    let size = ((genome.head_appendages.size as f32 * s).round() as i32).clamp(2, 8);
    // Its ears ride up a row as it comes up off a step, and lie a pixel back in a run.
    let root_y = root_y - stride.ear_perk;
    let mass = |c: &mut Canvas, cx: i32, rx: i32, shade_x: i32| {
        c.fill_ellipse(cx, cy + 1, rx + 1, ry + 1, p.outline);
        c.fill_ellipse(cx, cy, rx, ry, p.coat);
        c.fill_ellipse(
            shade_x,
            cy + ry / 2,
            (rx - 2).max(2),
            (ry / 3).max(2),
            p.shadow,
        );
        c.fill_ellipse(cx, cy - 1, rx - 2, (ry - 2).max(2), p.coat);
        apply_pattern(c, genome, p, cx, cy, rx - 1, ry - 1);
    };
    if view == View::Side {
        // A round mass is nearly as deep as it is wide.
        let deep = (wide - 1).max(6);
        let rx = (deep - stretch).max(5);
        // Against a wall, its belly is a pixel or two off the edge line, however deep it is.
        let cx = match cling {
            Some(cling) => (CLIMB_WALL - 5 - deep).max(19) + cling.over,
            None => 24,
        };
        // Nearer the crown than they are facing the screen, where the mass is at its tallest.
        let root_y = root_y - 2;
        draw_tail(c, genome, p, cx - rx + 1, cy + 1, s, pose);
        // Both ears are on the one line across the head: the far one shows ahead of the near.
        let back = stride.ears_back.min(1);
        shaded(c, p, |far| {
            blob_ear(far, genome, p, cx + 4 - back, root_y, size)
        });
        blob_ear(c, genome, p, cx - 2 - back, root_y, size);
        let mass_at = Mass {
            x: cx,
            y: cy,
            rx,
            ry,
            face_y: cy - 1,
        };
        let face = PixelPoint {
            x: mass_at.front(cy - 1) - 4,
            y: cy - 1,
        };
        if let Some(cling) = cling {
            let crown = stand - 2 * (ry - stretch);
            // Its paws go out from its chest, under its face.
            let shoulder = stand - (ry - stretch) + (ry - stretch) / 3;
            let rump = cx - rx / 2;
            // A short arm out to the wall from the nearest of the body's own edge to the paw.
            let arm = |c: &mut Canvas, far: bool| {
                let paw = cling.paw(far, shoulder, crown);
                let row = paw.y.clamp(cy - ry * 2 / 3, cy + ry * 2 / 3);
                // The far one shows only where it meets the wall.
                let root = if far {
                    PixelPoint {
                        x: paw.x - 2,
                        y: paw.y,
                    }
                } else {
                    PixelPoint {
                        x: mass_at.front(row) - 1,
                        y: row,
                    }
                };
                classic_arm(c, p, root, paw);
            };
            let leg = |c: &mut Canvas, far: bool| {
                let at = cling.foot(far, stand, rump);
                if cling.kicking {
                    c.line(rump, ground - 2, at.x, at.y, 2, p.outline);
                    blob_foot(c, p, at.x - 1, at.y, foot);
                } else {
                    // Its sole flat to the wall, toes up it.
                    c.line(cx + rx / 2, ground - 2, at.x, at.y, 2, p.outline);
                    c.fill_ellipse(at.x, at.y, 2, foot.clamp(2, 3), p.outline);
                    c.fill_ellipse(at.x, at.y - 1, 1, (foot - 1).clamp(1, 2), p.accent);
                }
            };
            shaded(c, p, |far| {
                arm(far, true);
                leg(far, true);
            });
            mass(c, cx, rx, cx - 2);
            leg(c, false);
            arm(c, false);
            return (mass_at, blob_figure(mass_at, face, None));
        }
        shaded(c, p, |far| {
            blob_foot(far, p, cx + stride.far_x, ground - stride.far_lift, foot);
        });
        mass(c, cx, rx, cx - 2);
        blob_foot(c, p, cx + stride.near_x, ground - stride.near_lift, foot);
        // The near paw, folded on the flank, swung a pixel against the near foot.
        let root = PixelPoint { x: cx, y: cy + 1 };
        let tip = PixelPoint {
            x: cx + 2 - stride.near_x.signum(),
            y: cy + 3,
        };
        c.line(root.x, root.y, tip.x, tip.y, 2, p.outline);
        c.fill_circle(tip.x, tip.y, 2, p.outline);
        c.line(root.x, root.y, tip.x, tip.y, 1, p.coat);
        c.fill_circle(tip.x, tip.y, 1, p.accent);
        return (mass_at, blob_figure(mass_at, face, Some(tip)));
    }
    let rx = wide;
    let cx = 24 + stride.sway;
    if let Some(cling) = cling {
        // On a wall, its back to the screen: a paw out past each side of it and a foot out
        // under each, all four behind the body they belong to.
        let crown = stand - 2 * (ry - stretch);
        let shoulder = stand - (ry - stretch);
        let half_at = |row: i32| {
            if (row - cy).abs() < ry {
                ellipse_half_width(rx, ry, row - cy)
            } else {
                rx / 2
            }
        };
        for (which, side) in [(0, 1), (1, -1)] {
            let (row, _) = cling.paw_row(which, shoulder, crown);
            let paw = PixelPoint {
                x: cx + side * (half_at(row) + 3),
                y: row,
            };
            let root = PixelPoint {
                x: cx + side * (rx - 3),
                y: row.clamp(cy - ry / 2, cy + ry / 2),
            };
            classic_arm(c, p, root, paw);
            let at = match cling.ledge() {
                Some(ledge) if cling.knee && which == 0 => PixelPoint {
                    x: cx + rx + 2,
                    y: ledge - 2,
                },
                _ => PixelPoint {
                    x: cx + side * (rx * 2 / 3 + 1),
                    y: stand - 1 - cling.feet[which],
                },
            };
            c.line(cx + side * rx / 2, ground - 3, at.x, at.y, 2, p.outline);
            c.fill_ellipse(at.x, at.y, (foot - 1).max(2), 2, p.outline);
            c.fill_ellipse(at.x, at.y, (foot - 2).max(1), 1, p.accent);
        }
        draw_head_appendages(c, genome, p, cx, root_y, s, pose);
        mass(c, cx, rx, cx);
        blob_back_tail(c, genome, p, cx, cy + ry / 2, stride.tail);
        let mass = Mass {
            x: cx,
            y: cy,
            rx,
            ry,
            face_y: cy - 1,
        };
        let face = PixelPoint { x: cx, y: cy - 1 };
        return (mass, blob_figure(mass, face, None));
    }
    // The paws rest on its front: only their tips show past its sides.
    for side in [-1, 1] {
        c.fill_circle(cx + side * rx, cy + 2, 2, p.outline);
        c.fill_circle(cx + side * rx, cy + 2, 1, p.accent);
    }
    draw_head_appendages(c, genome, p, cx, root_y, s, pose);
    // Heels, and the sole of a foot that is up.
    for (side, lift) in [(-1, stride.far_lift), (1, stride.near_lift)] {
        let x = 24 + side * (rx / 2);
        c.fill_ellipse(x, ground - lift, (foot - 1).max(2), 2, p.outline);
        if lift > 0 {
            c.fill_ellipse(x, ground - lift, (foot - 2).max(1), 1, p.accent);
        }
    }
    mass(c, cx, rx, cx);
    blob_back_tail(c, genome, p, cx, cy + ry / 2, stride.tail);
    let mass = Mass {
        x: cx,
        y: cy,
        rx,
        ry,
        face_y: cy - 1,
    };
    let face = PixelPoint { x: cx, y: cy - 1 };
    (mass, blob_figure(mass, face, None))
}

/// Where an original blob's parts are (see `classic::draw_blob`), for one turned so its face is
/// at `face` and its near paw at `hand`.
fn blob_figure(mass: Mass, face: PixelPoint, hand: Option<PixelPoint>) -> Figure {
    let Mass { x, y, rx, ry, .. } = mass;
    let neck_y = (face.y + 5).min(y + ry - 2);
    Figure {
        face,
        crown: PixelPoint { x, y: y - ry },
        head_half: (rx - 2).max(3),
        neck: PixelPoint { x, y: neck_y },
        neck_half: ellipse_half_width(rx, ry, neck_y - y),
        chest: PixelPoint {
            x: x + rx / 2,
            y: neck_y + 2,
        },
        hip: PixelPoint {
            x: x - rx + 2,
            y: y + ry / 2,
        },
        back: PixelPoint {
            x: x - rx + 2,
            y: y - ry / 3,
        },
        floor: y + ry + 1,
        hand,
    }
}

/// A recipe's marking as `draw_parts` paints it, about a body at `x`, `y`; `way` is 1 as it faces
/// the screen and -1 from behind, where what was on its left is on its right.
fn marking(c: &mut Canvas, d: CreatureDesign, p: Palette, body: (i32, i32, i32, i32), way: i32) {
    let (x, y, rx, ry) = body;
    match d.marking {
        1 => c.fill_ellipse(x, y + 3, (rx - 4).max(3), (ry - 2).max(3), p.highlight),
        2 => {
            for dx in [-5, 0, 5] {
                c.fill_ellipse(x + dx, y + 3, 1, 3, p.accent);
            }
        }
        3 => {
            for (dx, dy) in [(-5, 1), (3, 5), (5, -2)] {
                c.fill_circle(x + way * dx, y + dy, 2, p.accent);
            }
        }
        4 => c.fill_ellipse(x - way * 4, y + 2, 4, 4, p.accent),
        5 => c.fill_ellipse(x, y + 4, rx - 3, 2, p.accent),
        6 => {
            c.fill_circle(x - 4, y + 4, 2, p.highlight);
            c.fill_circle(x + 4, y + 4, 2, p.highlight);
        }
        _ => {}
    }
}

/// A recipe's body mass and what is painted on it, as `draw_parts` does both.
fn trunk(c: &mut Canvas, d: CreatureDesign, p: Palette, body: (i32, i32, i32, i32), way: i32) {
    let (x, y, rx, ry) = body;
    if d.classic.coat > 0 {
        lit_oval(c, p, x, y, rx, ry, p.coat);
        c.fill_ellipse(
            x - 2 * way.max(0),
            y + ry / 2,
            (rx - 2).max(2),
            (ry / 3).max(2),
            p.shadow,
        );
        c.fill_ellipse(x, y - 1, rx - 2, (ry - 2).max(2), p.coat);
    } else {
        oval(c, p, x, y, rx, ry, p.coat);
        c.fill_ellipse(x, y + ry / 2, rx - 3, (ry / 2).max(3), p.shadow);
    }
    if d.classic.pattern > 0 {
        classic_pattern(c, p, d, x, y, rx - 1, ry - 1);
    } else {
        inside(c, p, body, |layer| marking(layer, d, p, body, way));
    }
}

/// A leg from `hip` down to a foot at `x`, `lift` rows off the floor, toes forward by `toe`.
#[allow(clippy::too_many_arguments)]
fn leg(
    c: &mut Canvas,
    d: CreatureDesign,
    p: Palette,
    hip: PixelPoint,
    x: i32,
    floor: i32,
    lift: i32,
    (toe, half): (i32, i32),
) {
    let k = d.classic;
    // Stick legs are drawn in shade, so they read as thin legs rather than coat.
    let fill = if k.limbs == 2 { p.shadow } else { p.coat };
    c.line(hip.x, hip.y, x, floor - 2 - lift, 2, p.outline);
    c.line(hip.x, hip.y, x, floor - 2 - lift, 1, fill);
    let foot = PixelPoint {
        x: x + toe,
        y: floor - 1 - lift,
    };
    if k.limbs > 0 {
        classic_foot(c, p, k.limbs, foot);
    } else {
        oval(c, p, foot.x, foot.y, half, 2, p.coat);
    }
}

/// `figure` for a body drawn `dx` pixels along from where it was measured.
fn carried(figure: Figure, dx: i32) -> Figure {
    let along = |at: PixelPoint| PixelPoint { x: at.x + dx, ..at };
    Figure {
        face: along(figure.face),
        crown: along(figure.crown),
        neck: along(figure.neck),
        chest: along(figure.chest),
        hip: along(figure.hip),
        back: along(figure.back),
        hand: figure.hand.map(along),
        ..figure
    }
}

/// A recipe's body (see `draw_parts`) side-on or turned away.
fn recipe_turned(
    c: &mut Canvas,
    d: CreatureDesign,
    p: Palette,
    genome: &AppearanceGenome,
    view: View,
    stride: Stride,
) -> (Mass, Figure) {
    let k = d.classic;
    let blob = d.body == BodyPlan::Blob;
    let size = scale(genome);
    let cling = stride.cling;
    let hung = measure(d, standing(genome, stride), size);
    let mut pose = standing(genome, stride);
    if let Some(cling) = cling {
        pose.squash_x = -cling.stretch;
        pose.squash_y = cling.stretch;
        pose.play_lift = cling.lift;
    }
    let Body {
        x,
        y,
        rx,
        ry,
        hy,
        head,
        floor,
        ..
    } = measure(d, pose, size);
    // Against a wall, the front of it is a pixel or two off the edge line, however it is built.
    // Stretched on a wall, it is longer below its middle as well as above it, hanging down as
    // far as it reaches up, under a head that stays where it was.
    let stretched = cling.map_or(0, |cling| cling.stretch);
    let (y, hy) = (y + stretched, hy + stretched);
    let shift = cling.filter(|_| view == View::Side).map_or(0, |cling| {
        let deep = if blob {
            hung.rx * 6 / 7
        } else {
            hung.rx * 4 / 5
        }
        .max(5);
        let front = if blob {
            hung.x + deep + 1
        } else {
            (hung.x + deep + 2).max(hung.x + 2 + stride.lean + hung.head + i32::from(d.muzzle > 0))
        };
        CLIMB_WALL - 4 - front + cling.over
    });
    // Where its crown and its shoulders are as it hangs: on a body that carries its own face,
    // across its middle; under a head, just under its chin side-on, and up beside it from
    // behind, where the arms come out past it.
    let hung_top = if blob {
        hung.y - hung.ry
    } else {
        hung.hy - hung.head
    };
    let hung_shoulder = if blob && view == View::Side {
        hung.y + hung.ry / 3
    } else if blob {
        hung.y
    } else if view == View::Side {
        (hung.hy + hung.head).min(hung.y + 1)
    } else {
        hung.y - hung.ry / 2
    };
    let x = x + shift;
    let ear = ((f32::from(d.ear_size) * drawn_size(d, size))
        .round()
        .max(3.0) as i32
        + pose.ear_perk.clamp(0, 2)
        - pose.ears_back.clamp(0, 3))
    .max(2);
    let standing = carried(
        standing_figure(d, pose, size, BodyClip::Action(ActionKind::Idle), 0),
        shift,
    );
    let belly = crate::palette::detail_color(&d, d.details.belly_color);
    let horn_shade = if d.details.belly > 0 {
        belly
    } else {
        p.highlight
    };
    let arm = |c: &mut Canvas, at: PixelPoint, side: i32| {
        if d.body == BodyPlan::Winged {
            let lowest = feet_reach(d, floor) - 6;
            draw_wing(c, p, wing_style(d), at.x, at.y.min(lowest), side);
        } else if k.limbs > 0 {
            classic_nub(c, p, at, side);
        } else {
            oval(c, p, at.x, at.y, 2, 3, p.coat);
        }
    };
    let head_oval = |c: &mut Canvas, hx: i32| {
        if blob {
            return;
        }
        if k.coat > 0 {
            lit_oval(c, p, hx, hy, head, head - 1, p.coat);
        } else {
            oval(c, p, hx, hy, head, head - 1, p.coat);
        }
    };
    let top = if blob { y - ry } else { hy - head };
    let feet = 24 + shift;
    if view == View::Side {
        // Deeper than a head is, but not as deep as it is wide across the shoulders.
        let rx = if blob { rx * 6 / 7 } else { rx * 4 / 5 }.max(5);
        // A head is carried a little ahead of the body it sits on.
        let hx = if blob { x } else { x + 2 + stride.lean };
        let body = (x, y, rx, ry);
        let mass = if blob {
            Mass {
                x,
                y,
                rx,
                ry,
                face_y: hy,
            }
        } else {
            Mass {
                x: hx,
                y: hy,
                rx: head,
                ry: head - 1,
                face_y: hy,
            }
        };
        tail_behind(c, d, p, pose, x - rx + 1, y, ry);
        let lift = pose.appendage_lift.clamp(-1, 2);
        if d.details.wings > 0 {
            let root = PixelPoint {
                x: x + 1,
                y: y - 1 - lift,
            };
            wing_nub(c, p, d.details.wings, root, -1, false);
        }
        // Both ears are on the one line across the head: the far one shows ahead of the near.
        let (ear_cx, ear_top) = (hx, top + 2);
        let floppy = d.ears == EarStyle::Floppy;
        if k.crown > 0 {
            classic_crown(c, p, k.crown, ear_cx, 2, ear_top, ear, pose);
        } else {
            shaded(c, p, |far| {
                ear_at(far, d, p, pose, ear_cx + 2, 0, ear_top, ear, 1);
                ear_tuft(far, d, p, ear_cx + 2, 0, ear_top, ear, 1);
            });
            if !floppy {
                ear_at(c, d, p, pose, ear_cx - 1, 0, ear_top, ear, -1);
                ear_tuft(c, d, p, ear_cx - 1, 0, ear_top, ear, -1);
            }
        }
        if (1..=3).contains(&d.details.horns) {
            // Antlers branch, so the far one stands well clear of the near one.
            let pair = if d.details.horns == 3 {
                [(5, false), (0, true)]
            } else {
                [(4, false), (1, true)]
            };
            for (dx, near) in pair {
                let root = PixelPoint {
                    x: ear_cx + dx,
                    y: ear_top,
                };
                if near {
                    horn(c, p, horn_shade, d.details.horns, root, -1);
                } else {
                    shaded(c, p, |far| {
                        horn(far, p, horn_shade, d.details.horns, root, -1);
                    });
                }
            }
        }
        let hip = PixelPoint { x, y: y + ry - 2 };
        // Against a wall: a paw out to it, or a wing; and a leg braced forward against it, its
        // sole flat to it, or trailing in the air behind.
        let reach = |c: &mut Canvas, cling: Cling, far: bool| {
            let paw = cling.paw(far, hung_shoulder, hung_top);
            let root = if far {
                // The far one shows only where it meets the wall.
                PixelPoint {
                    x: paw.x - 2,
                    y: paw.y,
                }
            } else if blob {
                // A short arm from the nearest of its own edge.
                let row = paw.y.clamp(y - ry * 2 / 3, y + ry * 2 / 3);
                PixelPoint {
                    x: mass.front(row) - 1,
                    y: row,
                }
            } else {
                // From the front of the chest, under the chin.
                PixelPoint {
                    x: x + rx - 2,
                    y: (hung_shoulder + 1 - cling.lift).min(y + ry - 3),
                }
            };
            if d.body == BodyPlan::Winged {
                draw_open_wing(c, p, wing_style(d), root, paw, 1);
            } else if k.limbs > 0 {
                classic_arm(c, p, root, paw);
            } else {
                draw_arm(c, p, root, paw);
            }
        };
        let brace = |c: &mut Canvas, cling: Cling, far: bool| {
            let rump = x - rx / 2;
            let at = cling.foot(far, floor - 1, rump);
            // Stick legs are drawn in shade, so they read as thin legs rather than coat.
            let fill = if k.limbs == 2 { p.shadow } else { p.coat };
            let from = if far && !cling.kicking {
                PixelPoint {
                    x: at.x - 2,
                    y: at.y,
                }
            } else {
                PixelPoint {
                    x: if cling.kicking { rump } else { x + rx / 2 },
                    y: hip.y,
                }
            };
            c.line(from.x, from.y, at.x, at.y, 2, p.outline);
            c.line(from.x, from.y, at.x, at.y, 1, fill);
            if cling.kicking {
                if k.limbs > 0 {
                    classic_foot(c, p, k.limbs, at);
                } else {
                    oval(c, p, at.x, at.y, 3, 2, p.coat);
                }
            } else if k.limbs > 0 {
                let (rim, fork) = if k.limbs == 2 { (3, 2) } else { (2, 1) };
                c.fill_ellipse(at.x, at.y, 2, rim, p.outline);
                c.fill_ellipse(at.x, at.y - 1, 1, fork, p.accent);
            } else {
                oval(c, p, at.x - 1, at.y, 2, 2, p.coat);
            }
        };
        shaded(c, p, |far| match cling {
            Some(cling) => {
                reach(far, cling, true);
                brace(far, cling, true);
            }
            None => leg(
                far,
                d,
                p,
                hip,
                feet + stride.far_x,
                floor,
                stride.far_lift,
                (1, 3),
            ),
        });
        // A muzzle stands a pixel out past the front of a head: its edge goes down first, and
        // its colour over the head. On a body that carries its own face it stays inside the mass.
        let muzzle = (d.muzzle > 0).then(|| {
            let half = if d.muzzle == 2 { 4 } else { 3 };
            (mass.front(hy + 3) - half + i32::from(!blob), half)
        });
        trunk(c, d, p, body, 1);
        if d.details.belly == 1 {
            // Down its front.
            inside(c, p, body, |layer| {
                layer.fill_ellipse(
                    x + rx / 2 + 1,
                    y + 2,
                    (rx / 2).max(3),
                    (ry - 3).max(3),
                    belly,
                );
            });
        }
        match cling {
            Some(cling) => brace(c, cling, false),
            None => leg(
                c,
                d,
                p,
                hip,
                feet + stride.near_x,
                floor,
                stride.near_lift,
                (1, 3),
            ),
        }
        if let Some((mx, half)) = muzzle
            && !blob
        {
            c.fill_ellipse(mx, hy + 3, half + 1, 4, p.outline);
        }
        // A ruff stands out under the chin, round the front of the neck.
        if d.details.belly == 2 && !blob {
            ruff(c, d, p, hx - 1, hy + head - 2, head - 2, 1);
        }
        head_oval(c, hx);
        if let Some((mx, half)) = muzzle {
            let face = (
                mass.x,
                mass.y,
                mass.rx + i32::from(!blob),
                mass.ry + i32::from(!blob),
            );
            inside(c, p, face, |layer| {
                layer.fill_ellipse(mx, hy + 3, half, 3, p.highlight)
            });
        }
        if k.coat == 0 {
            c.fill_ellipse(hx - 1, top + 3, 3, 1, p.highlight);
        }
        if floppy && k.crown == 0 {
            ear_at(c, d, p, pose, ear_cx + 1, 0, ear_top, ear, -1);
        }
        // The near paw, folded on the flank, swung a pixel against the near foot.
        let paw = PixelPoint {
            x: x + 1 - stride.near_x.signum(),
            y: y + 3,
        };
        match cling {
            // Over its own chin, where the paw is up past it.
            Some(cling) => reach(c, cling, false),
            None => arm(c, paw, 1),
        }
        if d.details.wings > 0 {
            let root = PixelPoint {
                x: x - rx / 3 - 1,
                y: y - 2 - lift,
            };
            wing_nub(c, p, d.details.wings, root, -1, true);
        }
        let figure = Figure {
            face: PixelPoint {
                x: mass.front(hy) - 4,
                y: hy,
            },
            crown: PixelPoint {
                x: hx,
                y: standing.crown.y,
            },
            // A folded wing holds nothing, nor a paw that is holding a wall: what it carries
            // goes in its mouth.
            hand: (d.body != BodyPlan::Winged && cling.is_none()).then_some(paw),
            ..standing
        };
        return (mass, figure);
    }
    let body = (x, y, rx, ry);
    let (ear_cx, ear_span) = if blob { (x, rx - 4) } else { (x, head - 3) };
    let ear_top = top + 2;
    if k.crown > 0 {
        classic_crown(c, p, k.crown, ear_cx, ear_span, ear_top, ear, pose);
    } else {
        for side in [-1, 1] {
            ear_at(c, d, p, pose, ear_cx, ear_span, ear_top, ear, side);
            ear_tuft(c, d, p, ear_cx, ear_span, ear_top, ear, side);
        }
    }
    if (1..=3).contains(&d.details.horns) {
        for side in [-1, 1] {
            let root = PixelPoint {
                x: ear_cx
                    + side
                        * if d.details.horns == 3 {
                            (ear_span - 1).max(3)
                        } else {
                            (ear_span - 4).max(2)
                        },
                y: ear_top,
            };
            horn(c, p, horn_shade, d.details.horns, root, side);
        }
    }
    // On a wall, its back to the screen: a paw out past each side of it, up beside its head or
    // down beside its body, and a foot out under each hip, all behind the body. Wings on its
    // back would hide a paw beside them, so its paws go on after those.
    let paws_out = |c: &mut Canvas, cling: Cling| {
        let half_at = |row: i32| {
            let trunk = if (row - y).abs() < ry {
                ellipse_half_width(rx, ry, row - y)
            } else {
                0
            };
            let over = if !blob && (row - hy).abs() < head - 1 {
                ellipse_half_width(head, head - 1, row - hy)
            } else {
                0
            };
            trunk.max(over).max(rx / 2)
        };
        for (which, side) in [(0, 1), (1, -1)] {
            let (row, _) = cling.paw_row(which, hung_shoulder, hung_top);
            let paw = PixelPoint {
                x: x + side * (half_at(row) + 3),
                y: row,
            };
            let root = PixelPoint {
                x: x + side * (rx - 3),
                y: row.clamp(y - ry / 2, y + ry / 2),
            };
            if d.body == BodyPlan::Winged {
                draw_open_wing(c, p, wing_style(d), root, paw, side);
            } else if k.limbs > 0 {
                classic_arm(c, p, root, paw);
            } else {
                draw_arm(c, p, root, paw);
            }
        }
    };
    if let Some(cling) = cling {
        if d.details.wings == 0 {
            paws_out(c, cling);
        }
        for (which, side) in [(0, 1), (1, -1)] {
            let hip = PixelPoint {
                x: x + side * (rx - 4),
                y: y + ry - 2,
            };
            match cling.ledge() {
                Some(ledge) if cling.knee && which == 0 => {
                    let at = PixelPoint {
                        x: x + rx + 3,
                        y: ledge - 2,
                    };
                    let fill = if k.limbs == 2 { p.shadow } else { p.coat };
                    c.line(hip.x, hip.y, at.x, at.y, 2, p.outline);
                    c.line(hip.x, hip.y, at.x, at.y, 1, fill);
                    if k.limbs > 0 {
                        classic_foot(c, p, k.limbs, at);
                    } else {
                        oval(c, p, at.x, at.y, 2, 2, p.coat);
                    }
                }
                _ => leg(
                    c,
                    d,
                    p,
                    hip,
                    feet + side * (rx - 2),
                    floor,
                    cling.feet[which] + 1,
                    (0, 2),
                ),
            }
        }
    } else {
        // Heels, toes away: a foot is shorter from behind than from in front.
        for (side, lift) in [(-1, stride.far_lift), (1, stride.near_lift)] {
            let hip = PixelPoint {
                x: x + side * (rx - 4),
                y: y + ry - 2,
            };
            leg(c, d, p, hip, feet + side * (rx - 4), floor, lift, (0, 2));
        }
    }
    trunk(c, d, p, body, -1);
    if cling.is_none() {
        for side in [-1, 1] {
            arm(c, shoulder(measure(d, pose, size), pose, side), side);
        }
    }
    // Its wings are on its back, over the body they grow from and under the head.
    if d.details.wings > 0 {
        for side in [-1, 1] {
            let root = PixelPoint {
                x: x + side * (rx - 2),
                y: y - 1 - pose.appendage_lift.clamp(-1, 2),
            };
            wing_nub(c, p, d.details.wings, root, side, true);
        }
        if let Some(cling) = cling {
            paws_out(c, cling);
        }
    }
    // A ruff shows round the back of the neck, under the head.
    if d.details.belly == 2 && !blob {
        ruff(c, d, p, x, hy + head - 2, head - 1, -1);
    }
    head_oval(c, x);
    if k.coat == 0 {
        c.fill_ellipse(x + 2, top + 3, 3, 1, p.highlight);
    }
    // The tail comes toward whoever is looking, over the body, and round to one side.
    let sway = stride.tail;
    if k.tail > 0 {
        classic_tail(c, p, k.tail, x + 3, y + ry / 2 - 1, pose);
    } else {
        match d.tail {
            1 => oval(c, p, x, y + ry - 3, 3, 3, p.highlight),
            // A plume stands up the middle of the back, over it, its end curled over at the top.
            2 | 3 if d.details.tip == 4 => {
                let tip = crate::palette::detail_color(&d, d.details.tip_color);
                plume_from_behind(c, p, tip, x - 1 + sway, y, ry);
            }
            2 | 3 => {
                let root = PixelPoint { x, y: y + ry - 3 };
                let end = PixelPoint {
                    x: x - rx / 3 - 2 + sway,
                    y: y + 2,
                };
                c.line(root.x, root.y, end.x, end.y, 3, p.outline);
                c.line(root.x, root.y, end.x, end.y, 2, p.shadow);
                if d.details.tip > 0 {
                    let tip = crate::palette::detail_color(&d, d.details.tip_color);
                    tail_tip(c, p, d.details.tip, tip, end.x, end.y);
                } else if d.tail == 3 {
                    oval(c, p, end.x, end.y, 2, 2, p.accent);
                } else {
                    c.fill_circle(end.x, end.y, 1, p.accent);
                }
            }
            4 => oval(c, p, x, y + ry / 2, 4, 4, p.accent),
            _ => {}
        }
    }
    let mass = Mass {
        x,
        y: hy,
        rx: head,
        ry: head - 1,
        face_y: hy,
    };
    (
        mass,
        Figure {
            face: PixelPoint { x, y: hy },
            // Both paws are on the wall: what it carries goes in its mouth.
            hand: standing.hand.filter(|_| cling.is_none()),
            ..standing
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A paw or a foot that holds the wall stays where it gripped. The world raises the body by
    /// a set number of pixels as each frame of a climb comes (`formiga_core::CLIMB_FRAME_RISE`),
    /// so whatever is on the wall in one frame and still on it in the next is placed lower
    /// against the body by exactly that, whichever way the climb is seen. Only a paw come up off
    /// the wall to reach, or a foot stepping up, moves on it.
    #[test]
    fn what_holds_the_wall_stays_where_it_gripped_as_the_body_goes_up_past_it() {
        let rise = formiga_core::CLIMB_FRAME_RISE.map(i32::from);
        for top in [4, 7] {
            for frame in 0..6_u8 {
                let now = Cling::climbing(frame, top);
                let next = Cling::climbing((frame + 1) % 6, top);
                let coming = rise[usize::from((frame + 1) % 3)];
                let mut held = 0;
                for which in 0..2 {
                    if now.off[which] == 0 && next.off[which] == 0 {
                        assert_eq!(
                            next.paws[which],
                            now.paws[which] - coming,
                            "frame {frame}, paw {which}"
                        );
                        held += 1;
                    }
                    // A foot that is lower against the body is one the body has gone up past:
                    // by the rise and no more, down to a leg at full stretch.
                    if next.feet[which] < now.feet[which] {
                        assert!(
                            now.feet[which] - next.feet[which] <= coming || next.feet[which] == -1,
                            "frame {frame}, foot {which}"
                        );
                    }
                }
                assert!(held >= 1, "frame {frame}: a paw is on the wall throughout");
            }
            // One paw lands at the top of each stroke, and they take turns.
            assert_eq!(Cling::climbing(0, top).paws[0], top);
            assert_eq!(Cling::climbing(3, top).paws[1], top);
        }
    }

    /// The simulation decides which way a climb is made, and can only pick a way the art draws:
    /// what it takes to be a body that turns for a climb is exactly what the art draws turned.
    #[test]
    fn the_simulation_turns_for_a_climb_whatever_the_art_draws_turned() {
        let desktop = formiga_core::DesktopSnapshot::default();
        for index in 0..120_u8 {
            let mut seed = [index; 32];
            seed[1] = index.wrapping_mul(37);
            let modular = formiga_core::World::preview_adult(
                seed,
                time::OffsetDateTime::UNIX_EPOCH,
                &desktop,
            )
            .appearance;
            let mut classic = modular.clone();
            classic.design = None;
            let mut fox = modular.clone();
            fox.echo_fox = true;
            for genome in [modular, classic, fox] {
                assert_eq!(
                    formiga_core::climbs_turned(&genome),
                    can_turn(&genome),
                    "{:?} {:?}",
                    genome.family,
                    genome.design.map(|design| design.body)
                );
            }
        }
    }
}
