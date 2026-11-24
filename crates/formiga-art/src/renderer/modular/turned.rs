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
    let wide = ((genome.body_width as f32 * s / 2.0).round() as i32).clamp(6, 16);
    // As facing the screen, a blob presses down onto feet that stay where they are.
    let ry = ((genome.body_height as f32 * s / 2.0).round() as i32 - stride.bob).clamp(5, 14);
    let cy = 38 - ry;
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
        let rx = (wide - 1).max(6);
        let cx = 24;
        // Nearer the crown than they are facing the screen, where the mass is at its tallest.
        let root_y = root_y - 2;
        draw_tail(c, genome, p, cx - rx + 1, cy + 1, s, pose);
        // Both ears are on the one line across the head: the far one shows ahead of the near.
        let back = stride.ears_back.min(1);
        shaded(c, p, |far| {
            blob_ear(far, genome, p, cx + 4 - back, root_y, size)
        });
        blob_ear(c, genome, p, cx - 2 - back, root_y, size);
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
        let mass = Mass {
            x: cx,
            y: cy,
            rx,
            ry,
            face_y: cy - 1,
        };
        let face = PixelPoint {
            x: mass.front(cy - 1) - 4,
            y: cy - 1,
        };
        return (mass, blob_figure(mass, face, Some(tip)));
    }
    let rx = wide;
    let cx = 24 + stride.sway;
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
    let pose = standing(genome, stride);
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
    let ear = ((f32::from(d.ear_size) * drawn_size(d, size))
        .round()
        .max(3.0) as i32
        + pose.ear_perk.clamp(0, 2)
        - pose.ears_back.clamp(0, 3))
    .max(2);
    let standing = standing_figure(d, pose, size, BodyClip::Action(ActionKind::Idle), 0);
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
    let feet = 24;
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
            });
            if !floppy {
                ear_at(c, d, p, pose, ear_cx - 1, 0, ear_top, ear, -1);
            }
        }
        if d.details.horns > 0 {
            for (dx, near) in [(4, false), (1, true)] {
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
        shaded(c, p, |far| {
            leg(
                far,
                d,
                p,
                hip,
                feet + stride.far_x,
                floor,
                stride.far_lift,
                (1, 3),
            );
        });
        // A muzzle stands a pixel out past the front of a head: its edge goes down first, and
        // its colour over the head. On a body that carries its own face it stays inside the mass.
        let muzzle = (d.muzzle > 0).then(|| {
            let half = if d.muzzle == 2 { 4 } else { 3 };
            (mass.front(hy + 3) - half + i32::from(!blob), half)
        });
        trunk(c, d, p, body, 1);
        if d.details.belly > 0 {
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
        leg(
            c,
            d,
            p,
            hip,
            feet + stride.near_x,
            floor,
            stride.near_lift,
            (1, 3),
        );
        if let Some((mx, half)) = muzzle
            && !blob
        {
            c.fill_ellipse(mx, hy + 3, half + 1, 4, p.outline);
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
        arm(c, paw, 1);
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
            // A folded wing holds nothing: what it carries goes in its mouth.
            hand: (d.body != BodyPlan::Winged).then_some(paw),
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
        }
    }
    if d.details.horns > 0 {
        for side in [-1, 1] {
            let root = PixelPoint {
                x: ear_cx + side * (ear_span - 4).max(2),
                y: ear_top,
            };
            horn(c, p, horn_shade, d.details.horns, root, side);
        }
    }
    // Heels, toes away: a foot is shorter from behind than from in front.
    for (side, lift) in [(-1, stride.far_lift), (1, stride.near_lift)] {
        let hip = PixelPoint {
            x: x + side * (rx - 4),
            y: y + ry - 2,
        };
        leg(c, d, p, hip, feet + side * (rx - 4), floor, lift, (0, 2));
    }
    trunk(c, d, p, body, -1);
    for side in [-1, 1] {
        arm(c, shoulder(measure(d, pose, size), pose, side), side);
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
            ..standing
        },
    )
}
