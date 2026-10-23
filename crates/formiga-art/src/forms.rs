//! Drawing a sculpted form, one 48-pixel frame at a time, the same size and the same way as every
//! other Formiga companion. A creature whose genome keeps a sculpt (see [`formiga_core::forms`])
//! is drawn through here by [`CreatureRenderer`] itself, so it is drawn wherever a companion is:
//! on the desktop, in the notebook, on a card. Its body is painted here, then given the very face
//! a companion's recipe draws, so it keeps every expression, and dressed, given what it holds and
//! shown its effects as a companion is.
//!
//! [`DesignRenderer`] draws a [`Design`] over a creature's genome before any creature has it, as
//! Formiga Farm's window does. A companion recipe, or a look from before recipes, is drawn as
//! Desktop always draws it, untouched.
//!
//! A sculpted form can also be painted in finer detail, up to [`FINEST_DETAIL`] pixels to each of
//! its frame's: the same shapes in the same places, still pixel art in the same style, every
//! pixel solid or clear and the outline as heavy as a companion's, with smaller steps in its
//! curves, three tones of shade and its coat's grain. Its face is Formiga's own, redrawn larger
//! with its corners rounded. This is the drawing Formiga Farm shows on its stage.

mod draw;
mod figure;
mod motion;
mod paint;

pub use motion::Intent;

use crate::renderer::Figure as Worn;
use crate::{
    BodyClip, Canvas, CreatureRenderer, FRAME_SIZE, FaceRenderState, PixelPoint, RenderedBodyFrame,
    Rgba,
};
use figure::{FLOOR, Figure, HOVER};
use formiga_core::forms::{Design, Plan, Sculpt, face_carrier};
use formiga_core::{ActionKind, AppearanceGenome};
use paint::Sheet;

/// The finest a sculpted form is drawn, in pixels to each unit of its 48-unit frame. Desktop
/// shows a creature at 2, 3 or 4 screen pixels to the unit, and bakes a sculpted form as finely as
/// this allows and as its screen shows it, and never finer, so it is never shrunk and never
/// blurred.
pub const FINEST_DETAIL: u32 = 4;

/// The largest a sculpted figure may stand across the frame, and how far above the ground its
/// top may reach, leaving room for a hop.
const MAX_WIDTH: u32 = 44;
const HEADROOM: u32 = 7;

/// Where a design's figure stands in its frame, in frame pixels with the figure facing right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anchors {
    /// The row its feet rest on when standing, or that a floater hovers above.
    pub ground_row: i32,
    /// The middle of its face.
    pub face: PixelPoint,
    /// Where someone picks it up.
    pub scruff: PixelPoint,
    /// The pixels it covers at rest: left, top, right, bottom, inclusive.
    pub bounds: (u32, u32, u32, u32),
}

/// How a design is drawn: over `base`, the genome of the creature it is for, which keeps
/// everything the design does not say (see [`Design::genome`]).
pub struct DesignRenderer;

impl DesignRenderer {
    /// One body frame of `clip`, facing right, with no face: what Desktop bakes into an atlas.
    pub fn body_frame(
        design: &Design,
        base: &AppearanceGenome,
        clip: impl Into<BodyClip>,
        frame: u8,
        reduce_motion: bool,
    ) -> RenderedBodyFrame {
        CreatureRenderer::render_body_frame(&design.genome(base), clip, frame, reduce_motion)
    }

    /// One whole frame, face and all, as it would be drawn on the desktop.
    pub fn frame(
        design: &Design,
        base: &AppearanceGenome,
        clip: impl Into<BodyClip>,
        frame: u8,
        facing_right: bool,
        reduce_motion: bool,
        face_state: FaceRenderState,
    ) -> Canvas {
        CreatureRenderer::render_composited_frame(
            &design.genome(base),
            clip,
            frame,
            facing_right,
            reduce_motion,
            face_state,
        )
    }

    /// A whole frame of the clip that shows `intent`, with the face Desktop shows for it.
    pub fn intent_frame(
        design: &Design,
        base: &AppearanceGenome,
        intent: Intent,
        frame: u8,
        facing_right: bool,
        reduce_motion: bool,
    ) -> Canvas {
        let clip = intent.clip();
        let BodyClip::Action(action) = clip else {
            unreachable!("every intent shows an action")
        };
        let mut state = FaceRenderState {
            expression: expression_for(intent),
            eyelids: crate::EyelidPose::Open,
            gaze: crate::GazeDirection::new(0, 0),
        };
        if action == ActionKind::Sleep {
            state.eyelids = crate::EyelidPose::Closed;
        }
        Self::frame(
            design,
            base,
            clip,
            frame,
            facing_right,
            reduce_motion,
            state,
        )
    }

    /// How many frames, and how many a second, `intent`'s clip plays on the desktop.
    pub fn timing(intent: Intent) -> (u8, u8) {
        let spec = crate::AnimationSpec::for_clip(intent.clip());
        (spec.frames, spec.fps)
    }

    /// Empty rows beneath the figure when it rests, as Desktop measures them, so it can be stood
    /// on whatever it rests on.
    pub fn resting_baseline(design: &Design, base: &AppearanceGenome, reduce_motion: bool) -> u32 {
        CreatureRenderer::resting_baseline(&design.genome(base), reduce_motion)
    }

    /// Where the figure stands in its frame, at rest.
    pub fn anchors(design: &Design, base: &AppearanceGenome) -> Anchors {
        let genome = design.genome(base);
        let body = CreatureRenderer::render_body_frame(&genome, ActionKind::Idle, 0, true);
        let bounds = body.canvas.alpha_bounds().unwrap_or((0, 0, 0, 0));
        let baseline = CreatureRenderer::resting_baseline(&genome, true) as i32;
        let scruff = match &genome.sculpt {
            Some(sculpt) => {
                let (_, figure) = sculpted(sculpt, &genome, ActionKind::Idle.into(), 0, true, 1);
                point(figure.scruff)
            }
            None => PixelPoint {
                x: body.face_anchor.x - 4,
                y: bounds.1 as i32 + 2,
            },
        };
        Anchors {
            ground_row: FRAME_SIZE as i32 - 1 - baseline,
            face: body.face_anchor,
            scruff,
            bounds,
        }
    }
}

/// The genome a sculpted form's face, and whatever it holds, are drawn from: its own, wearing
/// the recipe that carries its face layout and its coat's colours ([`face_carrier`]).
pub(crate) fn face_genome(sculpt: &Sculpt, genome: &AppearanceGenome) -> AppearanceGenome {
    AppearanceGenome {
        design: Some(face_carrier(sculpt)),
        sculpt: None,
        ..genome.clone()
    }
}

/// One body frame of a sculpted form, facing right, and where anything it wears goes.
pub(crate) fn body(
    sculpt: &Sculpt,
    genome: &AppearanceGenome,
    clip: BodyClip,
    frame: u8,
    reduce_motion: bool,
) -> (Canvas, Worn) {
    let (sheet, figure) = sculpted(sculpt, genome, clip, frame, reduce_motion, 1);
    (sheet.canvas, worn(&figure))
}

/// The body [`body`] draws, painted `detail` pixels to each of its frame's (see [`FINEST_DETAIL`]),
/// standing exactly where that frame stands it.
pub(crate) fn body_in_detail(
    sculpt: &Sculpt,
    genome: &AppearanceGenome,
    clip: BodyClip,
    frame: u8,
    reduce_motion: bool,
    detail: u32,
) -> Canvas {
    let res = detail.clamp(1, FINEST_DETAIL) as i32;
    sculpted(sculpt, genome, clip, frame, reduce_motion, res)
        .0
        .canvas
}

/// A finely painted body with whatever was drawn on it at frame size: a frame pixel that what it
/// wears, holds or shows changed in `dressed` from `bare` is drawn as a square of that pixel, and
/// every other is `fine`'s own.
pub(crate) fn dressed_in_detail(
    dressed: &Canvas,
    bare: &Canvas,
    fine: &Canvas,
    detail: u32,
) -> Canvas {
    let res = detail as i32;
    let mut out = fine.clone();
    for y in 0..dressed.height() as i32 {
        for x in 0..dressed.width() as i32 {
            let pixel = dressed.get(x, y);
            if pixel == bare.get(x, y) {
                continue;
            }
            for fy in 0..res {
                for fx in 0..res {
                    out.set(x * res + fx, y * res + fy, pixel);
                }
            }
        }
    }
    out
}

/// Empty rows beneath a sculpted form at rest, measured from its own resting frame as a
/// companion's are. A floater's are measured to the ground it hovers above, never to its belly,
/// so it is never set down there.
pub(crate) fn resting_baseline(sculpt: &Sculpt, genome: &AppearanceGenome) -> u32 {
    let (sheet, _) = sculpted(sculpt, genome, ActionKind::Idle.into(), 0, true, 1);
    let bottom = sheet
        .canvas
        .alpha_bounds()
        .map_or(FLOOR as u32, |(_, _, _, bottom)| bottom);
    let hover = if sculpt.plan == Plan::Floater {
        HOVER as u32
    } else {
        0
    };
    (FRAME_SIZE - 1 - bottom).saturating_sub(hover)
}

/// Where anything worn goes on a sculpted figure, as a companion's figure places it: a hat on the
/// crown of its head, a collar where the head meets the neck, a bag at the back hip and a pack on
/// the top of the back.
fn worn(figure: &Figure) -> Worn {
    let round = |value: f32| value.round() as i32;
    let (head, body) = (figure.head, figure.body);
    let face = point(figure.face);
    let head_half = round(head.r);
    let neck_y = round(head.y + head.r) - 1;
    // A pack rides the top of the back, a little behind the middle, as on a companion.
    let back_x = round(body.cx - body.rx / 2.0) - 1;
    let across = ((back_x as f32 - body.cx) / body.rx).clamp(-1.0, 1.0);
    let back_top = body.cy - body.ry * (1.0 - across * across).sqrt();
    let floor = figure
        .legs
        .iter()
        .map(|leg| leg.foot.1 + leg.radius)
        .fold(body.cy + body.ry, f32::max);
    Worn {
        face,
        crown: PixelPoint {
            x: round(head.x),
            y: round(head.y - head.r),
        },
        head_half,
        neck: PixelPoint {
            x: round(head.x),
            y: neck_y,
        },
        neck_half: (head_half - 2).max(3),
        chest: PixelPoint {
            x: face.x + 1,
            y: (neck_y + 2).min(round(body.cy + body.ry) - 1),
        },
        hip: PixelPoint {
            x: round(body.cx - body.rx) + 2,
            y: round(body.cy + body.ry / 3.0),
        },
        back: PixelPoint {
            x: back_x,
            y: round(back_top) + 2,
        },
        floor: round(floor).min(FRAME_SIZE as i32 - 2) - 1,
    }
}

/// `canvas` with each pixel drawn as a `detail` by `detail` square.
pub(crate) fn blocks(canvas: &Canvas, detail: u32) -> Canvas {
    let res = detail.max(1) as i32;
    if res == 1 {
        return canvas.clone();
    }
    let mut out = Canvas::new(canvas.width() * res as u32, canvas.height() * res as u32);
    for y in 0..out.height() as i32 {
        for x in 0..out.width() as i32 {
            out.set(x, y, canvas.get(x / res, y / res));
        }
    }
    out
}

/// A pixel face drawn `detail` times larger with its corners rounded, as a pixel artist would
/// redraw it larger: each pixel doubled, tripled or doubled twice, so that a corner between two
/// runs of one colour is filled and a lone corner cut. Every expression keeps every feature,
/// still in flat pixels, only rounder.
pub(crate) fn rounded_face(face: &Canvas, detail: u32) -> Canvas {
    match detail.clamp(1, FINEST_DETAIL) {
        2 => scale2x(face),
        3 => scale3x(face),
        4 => scale2x(&scale2x(face)),
        _ => face.clone(),
    }
}

/// One pass of the Scale3x pixel-art enlargement.
fn scale3x(canvas: &Canvas) -> Canvas {
    let (w, h) = (canvas.width() as i32, canvas.height() as i32);
    let mut out = Canvas::new(w as u32 * 3, h as u32 * 3);
    let at = |x: i32, y: i32| canvas.get(x.clamp(0, w - 1), y.clamp(0, h - 1));
    for y in 0..h {
        for x in 0..w {
            let (a, b, c) = (at(x - 1, y - 1), at(x, y - 1), at(x + 1, y - 1));
            let (d, e, f) = (at(x - 1, y), at(x, y), at(x + 1, y));
            let (g, hh, i) = (at(x - 1, y + 1), at(x, y + 1), at(x + 1, y + 1));
            let mut cells = [e; 9];
            if b != hh && d != f {
                if d == b {
                    cells[0] = d;
                }
                if (d == b && e != c) || (b == f && e != a) {
                    cells[1] = b;
                }
                if b == f {
                    cells[2] = f;
                }
                if (d == b && e != g) || (d == hh && e != a) {
                    cells[3] = d;
                }
                if (b == f && e != i) || (hh == f && e != c) {
                    cells[5] = f;
                }
                if d == hh {
                    cells[6] = d;
                }
                if (d == hh && e != i) || (hh == f && e != g) {
                    cells[7] = hh;
                }
                if hh == f {
                    cells[8] = f;
                }
            }
            for (n, cell) in cells.into_iter().enumerate() {
                out.set(x * 3 + n as i32 % 3, y * 3 + n as i32 / 3, cell);
            }
        }
    }
    out
}

/// One pass of the Scale2x pixel-art enlargement.
fn scale2x(canvas: &Canvas) -> Canvas {
    let (w, h) = (canvas.width() as i32, canvas.height() as i32);
    let mut out = Canvas::new(w as u32 * 2, h as u32 * 2);
    let at = |x: i32, y: i32| canvas.get(x.clamp(0, w - 1), y.clamp(0, h - 1));
    for y in 0..h {
        for x in 0..w {
            let e = at(x, y);
            let (b, d, f, hh) = (at(x, y - 1), at(x - 1, y), at(x + 1, y), at(x, y + 1));
            let (mut e0, mut e1, mut e2, mut e3) = (e, e, e, e);
            if b != hh && d != f {
                if d == b {
                    e0 = d;
                }
                if b == f {
                    e1 = f;
                }
                if d == hh {
                    e2 = d;
                }
                if hh == f {
                    e3 = f;
                }
            }
            out.set(x * 2, y * 2, e0);
            out.set(x * 2 + 1, y * 2, e1);
            out.set(x * 2, y * 2 + 1, e2);
            out.set(x * 2 + 1, y * 2 + 1, e3);
        }
    }
    out
}

fn point((x, y): (f32, f32)) -> PixelPoint {
    PixelPoint {
        x: x.round() as i32,
        y: y.round() as i32,
    }
}

/// What face a preview of `intent` wears.
fn expression_for(intent: Intent) -> crate::ExpressionKind {
    use crate::ExpressionKind as E;
    match intent {
        Intent::Idle => E::Content,
        Intent::Move => E::Content,
        Intent::React => E::Startled,
        Intent::Inspect => E::Curious,
        Intent::Rest => E::Sleepy,
        Intent::Celebrate => E::Joy,
        Intent::Social => E::Affectionate,
        Intent::Held => E::Pleading,
    }
}

/// How large a sculpted form is drawn: its creature's size, as Desktop draws a companion's.
fn size_of(genome: &AppearanceGenome) -> f32 {
    (f32::from(genome.logical_size) / 38.0).clamp(0.55, 1.2)
}

/// Paint a sculpted form, fitted to its frame: a form too large for the frame is drawn smaller
/// as a whole, never cropped, and the figure is set in the middle. The fit is measured once at
/// rest, so every frame of every clip shares it and nothing jumps between frames.
fn sculpted(
    sculpt: &Sculpt,
    genome: &AppearanceGenome,
    clip: BodyClip,
    frame: u8,
    reduce_motion: bool,
    res: i32,
) -> (Sheet, Figure) {
    let sculpt = sculpt.normalized();
    let eye_spacing = genome.face.eye_spacing;
    let (scale, dx) = fit(&sculpt, size_of(genome), eye_spacing);
    let pose = motion::pose(sculpt.plan, clip, frame, reduce_motion);
    let figure = Figure::lay_out(&sculpt, scale, pose).shifted(dx);
    let mut sheet = Sheet::new();
    draw::draw(&mut sheet, &sculpt, &figure, pose, eye_spacing);
    // A leap that would reach past the frame is held inside it, a pixel from the edge, as
    // Desktop's atlas keeps every companion's.
    let laid_out = figure.clone();
    let mut figure = figure;
    let (mut moved_x, mut moved_y) = (0, 0);
    if let Some((left, top, right, bottom)) = sheet.canvas.alpha_bounds() {
        let edge = FRAME_SIZE as i32 - 2;
        let shift = |low: u32, high: u32| {
            if (low as i32) < 1 {
                1 - low as i32
            } else if high as i32 > edge {
                edge - high as i32
            } else {
                0
            }
        };
        let (dx, dy) = (shift(left, right), shift(top, bottom));
        if dx != 0 || dy != 0 {
            sheet.canvas.translate(dx, dy);
            figure = figure.moved(dx as f32, dy as f32);
            (moved_x, moved_y) = (dx, dy);
        }
    }
    if res > 1 {
        // The same figure painted finely, held exactly where the frame above holds it.
        let mut fine = Sheet::with_res(res);
        draw::draw(&mut fine, &sculpt, &laid_out, pose, eye_spacing);
        if moved_x != 0 || moved_y != 0 {
            fine.canvas.translate(moved_x * res, moved_y * res);
        }
        return (fine, figure);
    }
    (sheet, figure)
}

/// The fit of `sculpt`, remembered: every frame of every clip shares it, and measuring it means
/// drawing the form a few times over.
fn fit(sculpt: &Sculpt, size: f32, eye_spacing: u8) -> (f32, f32) {
    type Key = (Sculpt, u32, u8);
    thread_local! {
        static FITS: std::cell::RefCell<std::collections::HashMap<Key, (f32, f32)>> =
            std::cell::RefCell::default();
    }
    let key = (sculpt.clone(), size.to_bits(), eye_spacing);
    if let Some(fit) = FITS.with(|fits| fits.borrow().get(&key).copied()) {
        return fit;
    }
    let fit = measure_fit(sculpt, size, eye_spacing);
    FITS.with(|fits| {
        let mut fits = fits.borrow_mut();
        if fits.len() >= 256 {
            fits.clear();
        }
        fits.insert(key, fit);
    });
    fit
}

fn measure_fit(sculpt: &Sculpt, size: f32, eye_spacing: u8) -> (f32, f32) {
    let rest = motion::pose(sculpt.plan, ActionKind::Idle.into(), 0, true);
    let mut scale = size;
    for _ in 0..8 {
        let figure = Figure::lay_out(sculpt, scale, rest);
        let mut sheet = Sheet::new();
        draw::draw(&mut sheet, sculpt, &figure, rest, eye_spacing);
        let Some((left, top, right, _)) = sheet.canvas.alpha_bounds() else {
            return (scale, 0.0);
        };
        let width = right - left + 1;
        let wide = width as f32 / MAX_WIDTH as f32;
        let tall = (FLOOR - HEADROOM as f32 + 1.0) / (FLOOR - top as f32 + 1.0);
        let over = wide.max(1.0 / tall);
        if over <= 1.0 {
            let middle = (left + right) as f32 / 2.0;
            return (scale, (FRAME_SIZE as f32 / 2.0 - 0.5 - middle).round());
        }
        scale *= (1.0 / over).max(0.7) * 0.98;
    }
    (scale, 0.0)
}

/// The eyes in a sculpted form's own colour: every pixel of the face drawn in the palette's eye
/// ink is redrawn in `eyes`. `genome` is the one the face was drawn from ([`face_genome`]).
pub(crate) fn recolor_eyes(face: &mut Canvas, genome: &AppearanceGenome, eyes: [u8; 3]) {
    let ink = crate::palette_for(genome).eye;
    let to = Rgba::new(eyes[0], eyes[1], eyes[2], 255);
    for y in 0..face.height() as i32 {
        for x in 0..face.width() as i32 {
            let pixel = face.get(x, y);
            if pixel.a > 0 && (pixel.r, pixel.g, pixel.b) == (ink.r, ink.g, ink.b) {
                face.set(x, y, Rgba { a: pixel.a, ..to });
            }
        }
    }
}

#[cfg(test)]
mod tests;
