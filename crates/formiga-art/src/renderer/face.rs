//! The layered face: eyes, brows, cheeks and mouth, and which expression, eyelids and gaze a
//! creature shows.
use super::*;

/// One of the authored face layouts a recipe drawn since 0.62.0 wears. The layout fixes where the
/// eyes sit, how far apart and how large they are, and how far below them the mouth is; what still
/// varies from one companion to the next — its mouth, brows, cheeks and the light in its eyes — is
/// chosen from the few its layout allows, out of the face genes every companion has always had.
struct FaceTemplate {
    eye_shape: EyeShape,
    eye_size: u8,
    /// Half the distance between the eyes' centres, in art pixels.
    eye_dx: i32,
    /// How far below the middle of the face the eyes sit (negative is above).
    eye_dy: i32,
    /// Rows from the eyes down to the mouth. With `eye_dy` it never takes the mouth below row
    /// 11 of the 16-pixel face, so even a yawn fits.
    mouth_dy: i32,
    mouths: &'static [MouthStyle],
    brows: &'static [BrowStyle],
    cheeks: &'static [CheekStyle],
    highlights: &'static [HighlightStyle],
    pupils: &'static [PupilStyle],
}

/// The layouts, in the order a recipe numbers them from 1. Large readable eyes held apart, set in
/// the middle of the face or low on it, over a very small mouth: the classic modular face is here
/// as it always was, and so are the originals' mask and visor.
const FACE_TEMPLATES: [FaceTemplate; formiga_core::FACE_TEMPLATES as usize] = {
    use BrowStyle as B;
    use CheekStyle as C;
    use HighlightStyle as H;
    use MouthStyle as M;
    use PupilStyle as P;
    [
        // 1 Button: round eyes a little apart, a tiny mouth.
        FaceTemplate {
            eye_shape: EyeShape::Round,
            eye_size: 2,
            eye_dx: 3,
            eye_dy: 0,
            mouth_dy: 4,
            mouths: &[M::Tiny, M::Cat],
            brows: &[B::None, B::Soft],
            cheeks: &[C::Blush, C::None, C::Dots],
            highlights: &[H::Single, H::Double],
            pupils: &[P::Dot, P::Wide],
        },
        // 2 Wide button: the same eyes set wide.
        FaceTemplate {
            eye_shape: EyeShape::Round,
            eye_size: 2,
            eye_dx: 4,
            eye_dy: 0,
            mouth_dy: 4,
            mouths: &[M::Tiny, M::Cat, M::Smile],
            brows: &[B::None],
            cheeks: &[C::Blush, C::Dots],
            highlights: &[H::Double, H::Single],
            pupils: &[P::Dot],
        },
        // 3 Low and wide: big eyes low on the face, the mouth tucked under them.
        FaceTemplate {
            eye_shape: EyeShape::Round,
            eye_size: 2,
            eye_dx: 4,
            eye_dy: 1,
            mouth_dy: 3,
            mouths: &[M::Tiny, M::Cat],
            brows: &[B::None, B::Soft],
            cheeks: &[C::Blush],
            highlights: &[H::Single, H::Double],
            pupils: &[P::Dot, P::Wide],
        },
        // 4 The modular face every recipe before 0.62.0 wears.
        FaceTemplate {
            eye_shape: EyeShape::Round,
            eye_size: 2,
            eye_dx: 3,
            eye_dy: -1,
            mouth_dy: 5,
            mouths: &[M::Tiny, M::Smile, M::Cat],
            brows: &[B::None, B::Soft],
            cheeks: &[C::None, C::Dots, C::Blush],
            highlights: &[H::Single, H::Double, H::Diagonal],
            pupils: &[P::Dot, P::Wide, P::Spark],
        },
        // 5 Beady: small bright eyes set wide.
        FaceTemplate {
            eye_shape: EyeShape::Round,
            eye_size: 1,
            eye_dx: 4,
            eye_dy: 0,
            mouth_dy: 3,
            mouths: &[M::Tiny, M::Cat],
            brows: &[B::None],
            cheeks: &[C::Blush, C::Dots],
            highlights: &[H::Single],
            pupils: &[P::Dot],
        },
        // 6 Low beads: the same eyes low and wide, for a sillier look.
        FaceTemplate {
            eye_shape: EyeShape::Round,
            eye_size: 1,
            eye_dx: 4,
            eye_dy: 1,
            mouth_dy: 3,
            mouths: &[M::Tiny],
            brows: &[B::None, B::Soft],
            cheeks: &[C::Blush, C::None],
            highlights: &[H::Single],
            pupils: &[P::Dot],
        },
        // 7 Tall: tall ovals in the middle of the face.
        FaceTemplate {
            eye_shape: EyeShape::Tall,
            eye_size: 2,
            eye_dx: 3,
            eye_dy: -1,
            mouth_dy: 5,
            mouths: &[M::Cat, M::Tiny],
            brows: &[B::None, B::Soft],
            cheeks: &[C::None, C::Blush],
            highlights: &[H::Single, H::Diagonal],
            pupils: &[P::Dot, P::Spark],
        },
        // 8 Tall and wide: small tall eyes held apart.
        FaceTemplate {
            eye_shape: EyeShape::Tall,
            eye_size: 1,
            eye_dx: 4,
            eye_dy: 0,
            mouth_dy: 4,
            mouths: &[M::Tiny],
            brows: &[B::None],
            cheeks: &[C::Dots, C::Blush],
            highlights: &[H::Single],
            pupils: &[P::Dot],
        },
        // 9 Square: small soft squares.
        FaceTemplate {
            eye_shape: EyeShape::SoftSquare,
            eye_size: 1,
            eye_dx: 3,
            eye_dy: 0,
            mouth_dy: 3,
            mouths: &[M::Tiny, M::Smile],
            brows: &[B::None, B::Bold],
            cheeks: &[C::None, C::Dots],
            highlights: &[H::Single],
            pupils: &[P::Dot],
        },
        // 10 Mask: the originals' close-set eyes that run together.
        FaceTemplate {
            eye_shape: EyeShape::Round,
            eye_size: 2,
            eye_dx: 2,
            eye_dy: -1,
            mouth_dy: 5,
            mouths: &[M::Tiny, M::Cat],
            brows: &[B::None, B::Soft],
            cheeks: &[C::None, C::Blush],
            highlights: &[H::Single, H::Double],
            pupils: &[P::Dot],
        },
        // 11 Visor: the originals' squared eyes run into one band.
        FaceTemplate {
            eye_shape: EyeShape::SoftSquare,
            eye_size: 2,
            eye_dx: 2,
            eye_dy: -1,
            mouth_dy: 5,
            mouths: &[M::Tiny, M::Smile],
            brows: &[B::None, B::Bold],
            cheeks: &[C::None],
            highlights: &[H::Single, H::Diagonal],
            pupils: &[P::Dot, P::Wide],
        },
        // 12 Beak: round eyes over a little beak.
        FaceTemplate {
            eye_shape: EyeShape::Round,
            eye_size: 2,
            eye_dx: 3,
            eye_dy: -1,
            mouth_dy: 4,
            mouths: &[M::Beak],
            brows: &[B::None, B::Soft],
            cheeks: &[C::None, C::Blush],
            highlights: &[H::Single, H::Double],
            pupils: &[P::Dot],
        },
    ]
};

/// The face a recipe's layout gives a companion: the layout's eyes, and the rest of its own face
/// genes kept within what the layout allows.
fn templated(face: formiga_core::FaceGenome, template: &FaceTemplate) -> formiga_core::FaceGenome {
    fn pick<T: Copy>(choices: &[T], gene: usize) -> T {
        choices[gene % choices.len()]
    }
    formiga_core::FaceGenome {
        eye_shape: template.eye_shape,
        eye_size: template.eye_size,
        eye_spacing: (template.eye_dx * 2) as u8,
        vertical_offset: template.eye_dy as i8,
        mouth_style: pick(template.mouths, face.mouth_style as usize),
        brow_style: pick(template.brows, face.brow_style as usize),
        cheek_style: pick(template.cheeks, face.cheek_style as usize),
        highlight_style: pick(template.highlights, face.highlight_style as usize),
        pupil_style: pick(template.pupils, face.pupil_style as usize),
    }
}

pub(super) fn draw_face(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    center_x: i32,
    center_y: i32,
    state: FaceRenderState,
) {
    let mut face = genome.face;
    let design = genome.design.map(formiga_core::CreatureDesign::bounded);
    let template = design
        .filter(|design| design.face_template > 0)
        .and_then(|design| FACE_TEMPLATES.get(usize::from(design.face_template) - 1));
    if let Some(template) = template {
        face = templated(face, template);
        // A body already busy with stripes, spots or a patch wears a plain face: no painted
        // cheeks, though a happy one still blushes.
        if design
            .is_some_and(|design| design.classic.pattern > 0 || matches!(design.marking, 2..=4))
        {
            face.cheek_style = CheekStyle::None;
        }
        // A blob's one round mass is narrower at the top than a head on a body, so it wears the
        // same layout drawn snug: large eyes no wider set than the button face's, and rounder.
        let snug = design.is_some_and(|design| design.body == formiga_core::BodyPlan::Blob);
        draw_layout_face(
            canvas, face, template, snug, palette, center_x, center_y, state,
        );
        return;
    }
    if let Some(design) = genome.design {
        // A recipe sets the eyes; pupils, brows, cheeks, and mouth stay the creature's own. The
        // classic arrangements are the originals' eyes: close-set pairs that run together into a
        // mask or a visor, and smaller or taller pairs held apart.
        let (eye_shape, eye_size, eye_spacing) = match design.classic.face {
            1 => (EyeShape::Round, 2, 4),
            2 => (EyeShape::SoftSquare, 2, 4),
            3 => (EyeShape::Round, 1, 6),
            4 => (EyeShape::Tall, 2, 6),
            5 => (EyeShape::SoftSquare, 1, 6),
            _ => (EyeShape::Round, 2, 6),
        };
        face.eye_shape = eye_shape;
        face.eye_size = eye_size;
        face.eye_spacing = eye_spacing;
        face.vertical_offset = -1;
    }
    let spacing = (face.eye_spacing as i32 / 2).clamp(2, 3);
    let y = center_y + face.vertical_offset as i32;
    let eye_radius = face.eye_size as i32;
    let mouth_dy = eye_radius + 3;
    let eye_y_offsets = match state.expression {
        ExpressionKind::Worried => (1, 0),
        ExpressionKind::Curious => (0, -1),
        _ => (0, 0),
    };
    for (index, x) in [center_x - spacing, center_x + spacing]
        .into_iter()
        .enumerate()
    {
        let eye_y = y + if index == 0 {
            eye_y_offsets.0
        } else {
            eye_y_offsets.1
        };
        draw_eye(canvas, palette, face, x, eye_y, state);
    }
    draw_brows(
        canvas,
        face,
        palette,
        center_x,
        y,
        spacing,
        state.expression,
    );
    draw_cheeks(
        canvas,
        face,
        palette,
        center_x,
        y,
        spacing,
        state.expression,
    );
    draw_mouth(
        canvas,
        face,
        palette,
        center_x,
        y + mouth_dy,
        state.expression,
    );
}

/// An eye of an authored layout, as the rows it fills from the top: each row a bit mask read from
/// the eye's outer edge inward, bit 0 the outermost pixel. The left eye is drawn as it is and the
/// right one mirrored, so an eye an even number of pixels wide stands half a pixel further out on
/// each side and the pair stays symmetric about the middle of the face.
#[derive(Clone, Copy)]
struct EyeMask {
    width: i32,
    height: i32,
    /// The first row, against the row the layout puts the eyes on.
    top: i32,
    rows: [u8; 8],
}

impl EyeMask {
    /// The open eye a layout's shape and size give: rounded or square, and never a lone pixel
    /// sticking out of an edge, so the eye keeps a clean silhouette whatever is cut from it.
    fn of(shape: EyeShape, size: u8) -> Self {
        let (width, top, widths): (i32, i32, &[i32]) = match (shape, size) {
            (EyeShape::Round, 1) => (4, -2, &[2, 4, 4, 2]),
            (EyeShape::Round, _) => (6, -3, &[4, 6, 6, 6, 6, 4]),
            (EyeShape::Tall, 1) => (4, -2, &[2, 4, 4, 4, 2]),
            (EyeShape::Tall, _) => (5, -3, &[3, 5, 5, 5, 5, 5, 3]),
            (EyeShape::SoftSquare, 1) => (4, -2, &[4, 4, 4, 4]),
            (EyeShape::SoftSquare, _) => (6, -3, &[6, 6, 6, 6, 6, 6]),
        };
        let mut rows = [0; 8];
        for (row, &filled) in rows.iter_mut().zip(widths) {
            *row = (((1_u16 << filled) - 1) << ((width - filled) / 2)) as u8;
        }
        Self {
            width,
            height: widths.len() as i32,
            top,
            rows,
        }
    }

    fn filled(&self, column: i32, row: i32) -> bool {
        (0..self.width).contains(&column)
            && (0..self.height).contains(&row)
            && self.rows[row as usize] >> column & 1 == 1
    }

    /// A flat lid lowered over the top `rows`.
    fn lid(mut self, rows: i32) -> Self {
        for row in 0..rows.clamp(0, self.height) {
            self.rows[row as usize] = 0;
        }
        self
    }

    /// The top corner on one side cut away along a diagonal `depth` pixels deep, from the first
    /// row a lid has left: the inner corner for a scowl, the outer one for a worried or pleading
    /// look.
    fn slant(mut self, inner: bool, depth: i32) -> Self {
        let first = (0..self.height)
            .find(|&row| self.rows[row as usize] != 0)
            .unwrap_or(self.height);
        for row in 0..depth.clamp(0, self.height - first) {
            for step in 0..depth - row {
                let column = if inner { self.width - 1 - step } else { step };
                self.rows[(first + row) as usize] &= !(1 << column);
            }
        }
        self
    }

    /// The row a shut eye is drawn on: just below the middle, where the lids meet.
    fn middle(&self) -> i32 {
        self.top + self.height / 2
    }

    /// How deep a slanted corner is cut: most of the way into a large eye, less into a small one.
    fn deep(&self) -> i32 {
        (self.width + 1) / 2
    }

    /// The same eye as round as its size allows: a large round eye loses its corners.
    fn rounder(self) -> Self {
        if self.width == 6 && self.rows[0] != 0b11_1111 {
            let mut round = self;
            for (row, filled) in round.rows.iter_mut().zip([2, 4, 6, 6, 4, 2]) {
                *row = (((1_u16 << filled) - 1) << ((6 - filled) / 2)) as u8;
            }
            return round;
        }
        self
    }
}

/// How an authored face's eyes show once expression and eyelids have both had their say.
#[derive(Clone, Copy)]
enum EyeLook {
    Open(EyeMask),
    /// Shut and curved up in the middle: a smile with the eyes.
    Happy,
    /// Shut and curved down: asleep, or about to be.
    Drowsy,
    /// Shut flat, as in a blink.
    Blink,
    /// Wide, the whites showing round a shrunken pupil.
    Wide(EyeMask),
}

impl EyeLook {
    fn of(mask: EyeMask, expression: ExpressionKind, eyelids: EyelidPose) -> Self {
        use ExpressionKind as E;
        let third = mask.height / 3;
        let half = mask.height / 2;
        let (deep, shallow) = (mask.deep(), mask.deep() - 1);
        if eyelids == EyelidPose::Closed {
            return match expression {
                E::Joy | E::Affectionate | E::Content | E::Smug => Self::Happy,
                E::Sleepy | E::Yawning => Self::Drowsy,
                _ => Self::Blink,
            };
        }
        let open = match expression {
            E::Joy | E::Affectionate => return Self::Happy,
            E::Yawning => return Self::Drowsy,
            // Wide open for a fright, as round as it goes: the outline stays dark and the rest
            // shows white round the pupil.
            E::Startled if eyelids == EyelidPose::Open => return Self::Wide(mask.rounder()),
            // Down to a slit: heavy lids are thin, or they read as dark glasses.
            E::Sleepy => mask.lid(mask.height - 2),
            E::Bored => mask.lid(third),
            E::Smug => mask.lid(third).slant(false, shallow),
            E::Worried => mask.slant(false, deep),
            E::Determined => mask.slant(true, deep),
            E::Grumpy => mask.lid(third).slant(true, shallow),
            E::Pleading => mask.slant(false, shallow),
            _ => mask,
        };
        if eyelids == EyelidPose::Half {
            // Half-closed over whatever the look already was, never less closed than it.
            let mut lidded = open.lid(half);
            for row in half..mask.height {
                lidded.rows[row as usize] &= open.rows[row as usize];
            }
            return Self::Open(lidded);
        }
        Self::Open(open)
    }
}

/// An authored layout's face: eyes that carry most of the expression in their own shape — a lid
/// lowered flat, a corner cut on a slant, a shut curve — so no feature is stacked on another, and
/// brows a clear row above them, on the faces that have brows, only while they move.
#[allow(clippy::too_many_arguments)]
fn draw_layout_face(
    canvas: &mut Canvas,
    face: formiga_core::FaceGenome,
    template: &FaceTemplate,
    snug: bool,
    palette: Palette,
    center_x: i32,
    center_y: i32,
    state: FaceRenderState,
) {
    let y = center_y + template.eye_dy;
    let (eye_dx, mask) = match EyeMask::of(face.eye_shape, face.eye_size) {
        mask if snug && mask.width >= 5 => (template.eye_dx.min(3), mask.rounder()),
        mask => (template.eye_dx, mask),
    };
    let look = EyeLook::of(mask, state.expression, state.eyelids);
    for left in [true, false] {
        let eye_x = if left {
            center_x - eye_dx
        } else {
            center_x + eye_dx
        };
        // Where a column counted from the eye's outer edge lands on the canvas.
        let column_x = |column: i32| {
            if left {
                eye_x - mask.width / 2 + column
            } else {
                eye_x + mask.width / 2 - column
            }
        };
        // Curious looks one way with one eye a little narrowed.
        let look = match look {
            EyeLook::Open(open) if state.expression == ExpressionKind::Curious && !left => {
                EyeLook::Open(open.lid(1))
            }
            look => look,
        };
        draw_layout_eye(canvas, palette, face, mask, look, column_x, y, left, state);
        draw_layout_brow(
            canvas,
            face,
            palette,
            mask,
            column_x,
            y,
            left,
            state.expression,
        );
        draw_layout_cheek(
            canvas,
            face,
            palette,
            mask,
            look,
            column_x,
            y,
            state.expression,
        );
    }
    draw_layout_mouth(
        canvas,
        face,
        palette,
        center_x,
        y + template.mouth_dy,
        state.expression,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_layout_eye(
    canvas: &mut Canvas,
    palette: Palette,
    face: formiga_core::FaceGenome,
    mask: EyeMask,
    look: EyeLook,
    column_x: impl Fn(i32) -> i32,
    y: i32,
    left: bool,
    state: FaceRenderState,
) {
    let white = Rgba::new(255, 255, 245, 255);
    let middle = y + mask.middle();
    let last = mask.width - 1;
    match look {
        EyeLook::Happy => {
            for column in 1..last {
                canvas.set(column_x(column), middle - 1, palette.eye);
            }
            canvas.set(column_x(0), middle, palette.eye);
            canvas.set(column_x(last), middle, palette.eye);
        }
        EyeLook::Drowsy => {
            canvas.set(column_x(0), middle, palette.eye);
            canvas.set(column_x(last), middle, palette.eye);
            for column in 1..last {
                canvas.set(column_x(column), middle + 1, palette.eye);
            }
        }
        EyeLook::Blink => {
            for column in 0..mask.width {
                canvas.set(column_x(column), middle, palette.eye);
            }
        }
        // Too small to show white round a pupil: the pupil shrinks to a point instead.
        EyeLook::Wide(open) if open.width < 5 => {
            let top = y + open.top;
            let (column, row) = ((open.width - 1) / 2, (open.height - 1) / 2);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                canvas.set(column_x(column + dx), top + row + dy, palette.eye);
            }
        }
        EyeLook::Wide(open) => {
            let top = y + open.top;
            let inside = |column: i32, row: i32| {
                [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]
                    .into_iter()
                    .all(|(dx, dy)| open.filled(column + dx, row + dy))
            };
            for row in 0..open.height {
                for column in 0..open.width {
                    if open.filled(column, row) {
                        let color = if inside(column, row) {
                            white
                        } else {
                            palette.eye
                        };
                        canvas.set(column_x(column), top + row, color);
                    }
                }
            }
            // A pupil shrunk to a point in the middle of the white, turned toward the gaze.
            let (pupil_column, pupil_row) = (
                (open.width - 1) / 2 + i32::from(state.gaze.x) * if left { 1 } else { -1 },
                (open.height - 1) / 2 + i32::from(state.gaze.y),
            );
            let size = if open.width >= 6 { 2 } else { 1 };
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                if dx < size && dy < size && inside(pupil_column + dx, pupil_row + dy) {
                    canvas.set(
                        column_x(pupil_column + dx),
                        top + pupil_row + dy,
                        palette.eye,
                    );
                }
            }
        }
        EyeLook::Open(open) => {
            let top = y + open.top;
            for row in 0..open.height {
                for column in 0..open.width {
                    if open.filled(column, row) {
                        canvas.set(column_x(column), top + row, palette.eye);
                    }
                }
            }
            // The shine sits on the same side of both eyes, as light does, and moves with the
            // gaze as far as the open part of the eye lets it.
            let from_left = |column: i32| if left { column } else { last - column };
            let shine = |column: i32, row: i32| open.filled(from_left(column), row);
            // A small eye keeps its shine off the rounded top edge, where it would read as a notch.
            let (anchor_x, anchor_y) = (
                (open.width - 1) / 2,
                (open.height - 1) / 2 + i32::from(open.height < 5),
            );
            // Focused looks where it is going: ahead, unless something has its eye.
            let gaze_x = match state.gaze.x {
                0 if state.expression == ExpressionKind::Focused => 1,
                x => i32::from(x),
            };
            let gaze_y = i32::from(state.gaze.y);
            let mut spots: Vec<(i32, i32)> = match face.highlight_style {
                HighlightStyle::Single => vec![(0, -1)],
                HighlightStyle::Double => vec![(0, -1), (1, 1)],
                HighlightStyle::Diagonal => vec![(-1, -1), (0, 0)],
            };
            match face.pupil_style {
                PupilStyle::Dot => {}
                PupilStyle::Wide => spots.push((-1, 0)),
                PupilStyle::Spark => spots.extend([(1, 0), (0, 1)]),
            }
            match state.expression {
                // Heavy lids have no light in them.
                ExpressionKind::Sleepy => spots.clear(),
                ExpressionKind::Pleading if open.width >= 5 => {
                    spots.extend([(-1, -1), (-1, 0), (1, 1)]);
                }
                ExpressionKind::Pleading => spots.push((1, 1)),
                _ => {}
            }
            let candidates = [
                (anchor_x + gaze_x, anchor_y - 1 + gaze_y),
                (anchor_x + gaze_x, anchor_y + gaze_y),
                (anchor_x + gaze_x, anchor_y - 1),
                (anchor_x + gaze_x, anchor_y),
                (anchor_x, anchor_y),
                (anchor_x, anchor_y + 1),
                (anchor_x, anchor_y + 2),
            ];
            let main = if spots.is_empty() {
                None
            } else {
                candidates
                    .into_iter()
                    .find(|&(column, row)| shine(column, row))
            };
            // Every shine moves with the first, which is the one that has to be seen.
            if let Some((main_x, main_y)) = main {
                let (shift_x, shift_y) = (main_x - anchor_x, main_y - (anchor_y - 1));
                for (dx, dy) in spots {
                    let (column, row) = (anchor_x + dx + shift_x, anchor_y + dy + shift_y);
                    if shine(column, row) {
                        canvas.set(column_x(from_left(column)), top + row, white);
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_layout_brow(
    canvas: &mut Canvas,
    face: formiga_core::FaceGenome,
    palette: Palette,
    mask: EyeMask,
    column_x: impl Fn(i32) -> i32,
    y: i32,
    left: bool,
    expression: ExpressionKind,
) {
    use ExpressionKind as E;
    let outer = match face.brow_style {
        BrowStyle::None => return,
        BrowStyle::Bold => 0,
        _ => 1,
    };
    let inner = if mask.width >= 5 { 3 } else { 2 };
    // A clear row between brow and eye, so the two never run together. Brows show only when
    // they move — down in a scowl, up in the middle for worry, up for a fright, one of them up
    // for a question — and a face at rest is left clean.
    let brow = (y + mask.top - 2).max(1);
    let (outer_y, inner_y) = match expression {
        E::Determined | E::Grumpy => (brow - 1, brow),
        E::Worried | E::Pleading => (brow, brow - 1),
        E::Startled => (brow - 1, brow - 1),
        E::Curious if !left => (brow - 1, brow - 1),
        _ => return,
    };
    canvas.line(
        column_x(outer),
        outer_y,
        column_x(inner),
        inner_y,
        1,
        palette.outline,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_layout_cheek(
    canvas: &mut Canvas,
    face: formiga_core::FaceGenome,
    palette: Palette,
    mask: EyeMask,
    look: EyeLook,
    column_x: impl Fn(i32) -> i32,
    y: i32,
    expression: ExpressionKind,
) {
    let blushing = matches!(
        expression,
        ExpressionKind::Joy | ExpressionKind::Affectionate
    );
    let columns: &[i32] = match face.cheek_style {
        CheekStyle::None if !blushing => return,
        CheekStyle::Dots if !blushing => &[1],
        _ => &[1, 2],
    };
    // Just under the eye, or just under the curve of a shut one.
    let row = match look {
        EyeLook::Open(_) | EyeLook::Wide(_) => y + mask.top + mask.height,
        _ => y + mask.middle() + 2,
    };
    for &column in columns {
        canvas.set(column_x(column), row, palette.accent);
    }
}

fn draw_layout_mouth(
    canvas: &mut Canvas,
    face: formiga_core::FaceGenome,
    palette: Palette,
    x: i32,
    y: i32,
    expression: ExpressionKind,
) {
    use ExpressionKind as E;
    let ink = palette.outline;
    let inside = mouth_inside(palette);
    let put = |canvas: &mut Canvas, pixels: &[(i32, i32)], color: Rgba| {
        for &(dx, dy) in pixels {
            canvas.set(x + dx, y + dy, color);
        }
    };
    // A beak stays a beak whatever the face is doing: it opens, or tips up when pleased.
    if face.mouth_style == MouthStyle::Beak {
        match expression {
            E::Yawning => {
                put(canvas, &[(0, -1), (1, -1), (0, 2), (1, 2)], palette.accent);
                put(canvas, &[(0, 0), (1, 0), (0, 1), (1, 1)], inside);
            }
            E::Joy | E::Startled => {
                put(canvas, &[(0, -1), (1, -1), (0, 1)], palette.accent);
                put(canvas, &[(0, 0), (1, 0)], inside);
            }
            E::Content | E::Affectionate | E::Smug => {
                put(canvas, &[(0, -1), (1, -1)], palette.accent);
            }
            _ => put(canvas, &[(0, -1), (1, 0)], palette.accent),
        }
        return;
    }
    match expression {
        E::Joy => {
            put(
                canvas,
                &[(-2, -1), (-1, -1), (0, -1), (1, -1), (2, -1)],
                ink,
            );
            put(canvas, &[(-1, 0), (1, 0)], ink);
            put(canvas, &[(0, 0)], inside);
        }
        E::Affectionate => put(canvas, &[(-2, -1), (-1, 0), (0, -1), (1, 0), (2, -1)], ink),
        E::Content => put(canvas, &[(-1, 0), (0, 1), (1, 0)], ink),
        E::Curious => put(canvas, &[(0, -1), (-1, 0), (1, 0), (0, 1)], ink),
        E::Startled => {
            put(
                canvas,
                &[(0, -1), (-1, 0), (1, 0), (-1, 1), (1, 1), (0, 2)],
                ink,
            );
            put(canvas, &[(0, 0), (0, 1)], inside);
        }
        E::Worried => put(canvas, &[(-2, 0), (-1, -1), (0, 0), (1, -1), (2, 0)], ink),
        E::Focused | E::Determined => put(canvas, &[(-1, 0), (0, 0), (1, 0)], ink),
        E::Bored => put(canvas, &[(0, 0), (1, 0)], ink),
        E::Grumpy => put(canvas, &[(-2, 0), (-1, -1), (0, -1), (1, -1), (2, 0)], ink),
        E::Smug => put(canvas, &[(-1, 0), (0, 0), (1, 0), (2, -1)], ink),
        E::Pleading => put(canvas, &[(-1, 0), (0, -1), (1, 0)], ink),
        E::Yawning => {
            put(
                canvas,
                &[
                    (-1, -1),
                    (0, -1),
                    (1, -1),
                    (-2, 0),
                    (2, 0),
                    (-2, 1),
                    (2, 1),
                    (-1, 2),
                    (0, 2),
                    (1, 2),
                ],
                ink,
            );
            put(canvas, &[(-1, 0), (0, 0), (1, 0), (-1, 1), (1, 1)], inside);
            put(canvas, &[(0, 1)], palette.accent);
        }
        E::Neutral | E::Sleepy => match face.mouth_style {
            MouthStyle::Tiny | MouthStyle::Beak => put(canvas, &[(0, 0)], ink),
            MouthStyle::Smile => put(canvas, &[(-2, -1), (-1, 0), (0, 0), (1, 0), (2, -1)], ink),
            MouthStyle::Cat => put(canvas, &[(-1, -1), (0, 0), (1, -1)], ink),
        },
    }
}

pub(super) fn draw_eye(
    canvas: &mut Canvas,
    palette: Palette,
    face: formiga_core::FaceGenome,
    x: i32,
    y: i32,
    state: FaceRenderState,
) {
    let radius = face.eye_size as i32;
    // A yawn screws the eyes up: two short slants meeting at the outside corner, whatever the
    // eyelids would otherwise be doing.
    if state.expression == ExpressionKind::Yawning {
        canvas.line(x - radius, y - 1, x + radius, y, 1, palette.eye);
        canvas.set(x - radius, y + 1, palette.eye);
        return;
    }
    if state.eyelids == EyelidPose::Closed {
        let curve = matches!(
            state.expression,
            ExpressionKind::Joy | ExpressionKind::Content | ExpressionKind::Affectionate
        );
        canvas.line(
            x - radius,
            y,
            x + radius,
            y + i32::from(curve),
            1,
            palette.eye,
        );
        return;
    }
    match face.eye_shape {
        EyeShape::Round => canvas.fill_circle(x, y, radius + 1, palette.outline),
        EyeShape::Tall => canvas.fill_ellipse(x, y, radius + 1, radius + 2, palette.outline),
        EyeShape::SoftSquare => canvas.fill_rect(
            x - radius - 1,
            y - radius - 1,
            radius * 2 + 3,
            radius * 2 + 3,
            palette.outline,
        ),
    }
    match face.eye_shape {
        EyeShape::Round => canvas.fill_circle(x, y, radius, palette.eye),
        EyeShape::Tall => canvas.fill_ellipse(x, y, radius, radius + 1, palette.eye),
        EyeShape::SoftSquare => canvas.fill_rect(
            x - radius,
            y - radius,
            radius * 2 + 1,
            radius * 2 + 1,
            palette.eye,
        ),
    }
    if state.eyelids == EyelidPose::Half {
        canvas.fill_rect(
            x - radius - 1,
            y - radius - 2,
            radius * 2 + 3,
            radius + 2,
            palette.coat,
        );
        canvas.line(
            x - radius - 1,
            y - 1,
            x + radius + 1,
            y - 1,
            1,
            palette.outline,
        );
    }
    let pupil_x = x + i32::from(state.gaze.x);
    let pupil_y = y + i32::from(state.gaze.y);
    let white = Rgba::new(255, 255, 245, 255);
    match face.highlight_style {
        HighlightStyle::Single => canvas.set(pupil_x, pupil_y - radius.min(1), white),
        HighlightStyle::Double => {
            canvas.set(pupil_x, pupil_y - radius.min(1), white);
            canvas.set(pupil_x + 1, pupil_y + 1, white);
        }
        HighlightStyle::Diagonal => {
            canvas.set(pupil_x - 1, pupil_y - 1, white);
            canvas.set(pupil_x, pupil_y, white);
        }
    }
    match face.pupil_style {
        PupilStyle::Dot => {}
        PupilStyle::Wide => canvas.set(pupil_x - 1, pupil_y, white),
        PupilStyle::Spark => {
            canvas.set(pupil_x + 1, pupil_y, white);
            canvas.set(pupil_x, pupil_y + 1, white);
        }
    }
    // Pleading eyes shine twice over.
    if state.expression == ExpressionKind::Pleading {
        canvas.set(pupil_x - 1, pupil_y + 1, white);
    }
}

pub(super) fn draw_brows(
    canvas: &mut Canvas,
    face: formiga_core::FaceGenome,
    palette: Palette,
    center_x: i32,
    y: i32,
    spacing: i32,
    expression: ExpressionKind,
) {
    let weight = match face.brow_style {
        BrowStyle::None
            if matches!(
                expression,
                ExpressionKind::Neutral | ExpressionKind::Content | ExpressionKind::Sleepy
            ) =>
        {
            return;
        }
        BrowStyle::Bold => 2,
        _ => 1,
    };
    // How far each brow's inner end is lifted: up for worry and tenderness, down for a scowl.
    let (left_inner, right_inner) = match expression {
        ExpressionKind::Worried | ExpressionKind::Affectionate | ExpressionKind::Pleading => (1, 1),
        ExpressionKind::Focused
        | ExpressionKind::Determined
        | ExpressionKind::Yawning
        | ExpressionKind::Grumpy => (-1, -1),
        ExpressionKind::Startled | ExpressionKind::Curious => (-1, 1),
        ExpressionKind::Bored | ExpressionKind::Sleepy => (1, 0),
        _ => (0, 0),
    };
    canvas.line(
        center_x - spacing - 1,
        y - 3 + left_inner,
        center_x - spacing + 1,
        y - 3 - left_inner,
        weight,
        palette.outline,
    );
    canvas.line(
        center_x + spacing - 1,
        y - 3 - right_inner,
        center_x + spacing + 1,
        y - 3 + right_inner,
        weight,
        palette.outline,
    );
}

pub(super) fn draw_cheeks(
    canvas: &mut Canvas,
    face: formiga_core::FaceGenome,
    palette: Palette,
    center_x: i32,
    y: i32,
    spacing: i32,
    expression: ExpressionKind,
) {
    if face.cheek_style == CheekStyle::None
        && !matches!(
            expression,
            ExpressionKind::Joy | ExpressionKind::Affectionate
        )
    {
        return;
    }
    let cheek_y = y + 3;
    for x in [center_x - spacing - 2, center_x + spacing + 2] {
        canvas.set(x, cheek_y, palette.accent);
        if face.cheek_style == CheekStyle::Blush {
            canvas.set(x + 1, cheek_y, palette.accent);
        }
    }
}

pub(super) fn draw_mouth(
    canvas: &mut Canvas,
    face: formiga_core::FaceGenome,
    palette: Palette,
    x: i32,
    y: i32,
    expression: ExpressionKind,
) {
    match expression {
        ExpressionKind::Joy => {
            canvas.set(x - 2, y - 1, palette.outline);
            canvas.set(x - 1, y - 1, palette.outline);
            canvas.set(x, y, palette.outline);
            canvas.set(x + 1, y - 1, palette.outline);
            canvas.set(x + 2, y - 1, palette.outline);
        }
        ExpressionKind::Affectionate => {
            canvas.set(x - 1, y - 1, palette.accent);
            canvas.set(x, y, palette.outline);
            canvas.set(x + 1, y - 1, palette.accent);
        }
        ExpressionKind::Content => {
            canvas.set(x - 1, y, palette.outline);
            canvas.set(x, y + 1, palette.outline);
            canvas.set(x + 1, y, palette.outline);
        }
        ExpressionKind::Startled | ExpressionKind::Curious => {
            canvas.fill_circle(x, y, 1, palette.outline);
            canvas.set(x, y, palette.coat);
        }
        ExpressionKind::Worried => {
            canvas.set(x - 1, y, palette.outline);
            canvas.set(x, y - 1, palette.outline);
            canvas.set(x + 1, y, palette.outline);
        }
        ExpressionKind::Bored => canvas.line(x - 1, y, x + 1, y, 1, palette.outline),
        ExpressionKind::Focused => canvas.line(x - 1, y, x + 1, y - 1, 1, palette.outline),
        ExpressionKind::Determined => canvas.line(x - 2, y, x + 2, y, 1, palette.outline),
        ExpressionKind::Grumpy => {
            canvas.line(x - 1, y - 1, x + 1, y - 1, 1, palette.outline);
            canvas.set(x - 2, y, palette.outline);
            canvas.set(x + 2, y, palette.outline);
        }
        ExpressionKind::Smug => {
            canvas.line(x - 1, y, x, y, 1, palette.outline);
            canvas.set(x + 1, y - 1, palette.outline);
        }
        ExpressionKind::Pleading => canvas.set(x, y, palette.outline),
        // Wide open and tall: a dark mouth with the tongue showing at the bottom of it.
        ExpressionKind::Yawning => {
            canvas.fill_ellipse(x, y + 1, 2, 3, palette.outline);
            canvas.fill_ellipse(x, y + 1, 1, 2, super::face::mouth_inside(palette));
            canvas.set(x, y + 3, palette.accent);
        }
        _ => match face.mouth_style {
            MouthStyle::Tiny => canvas.set(x, y, palette.outline),
            MouthStyle::Smile => canvas.line(x - 1, y - 1, x + 1, y - 1, 1, palette.outline),
            MouthStyle::Cat => {
                canvas.set(x - 1, y - 1, palette.outline);
                canvas.set(x, y, palette.outline);
                canvas.set(x + 1, y - 1, palette.outline);
            }
            MouthStyle::Beak => {
                canvas.set(x, y - 1, palette.accent);
                canvas.set(x + 1, y, palette.accent);
            }
        },
    }
}

/// The inside of an open mouth: the creature's own outline warmed toward red.
pub(super) fn mouth_inside(palette: Palette) -> Rgba {
    Rgba::new(
        ((u16::from(palette.outline.r) + 150) / 2) as u8,
        ((u16::from(palette.outline.g) + 40) / 2) as u8,
        ((u16::from(palette.outline.b) + 60) / 2) as u8,
        255,
    )
}

pub(super) fn expression_for_action(action: ActionKind) -> ExpressionKind {
    match action {
        ActionKind::Idle => ExpressionKind::Neutral,
        ActionKind::Traverse | ActionKind::SqueezeWindow | ActionKind::RideWindow => {
            ExpressionKind::Focused
        }
        ActionKind::Sprint => ExpressionKind::Determined,
        ActionKind::Perch | ActionKind::Homebound => ExpressionKind::Content,
        ActionKind::Sleep => ExpressionKind::Sleepy,
        ActionKind::InvestigateCursor => ExpressionKind::Curious,
        ActionKind::AvoidCursor => ExpressionKind::Worried,
        ActionKind::ReactToWindow => ExpressionKind::Startled,
        ActionKind::SoloPlay | ActionKind::SocialPlay => ExpressionKind::Joy,
        ActionKind::Eat | ActionKind::Drink => ExpressionKind::Content,
        ActionKind::Greet | ActionKind::Follow | ActionKind::PetReaction => {
            ExpressionKind::Affectionate
        }
        ActionKind::Dragged => ExpressionKind::Curious,
        ActionKind::Landing => ExpressionKind::Determined,
        ActionKind::ClimbWindow => ExpressionKind::Determined,
        ActionKind::Dangle => ExpressionKind::Content,
        ActionKind::InspectScreen => ExpressionKind::Curious,
        ActionKind::PresentDiscovery => ExpressionKind::Joy,
        ActionKind::Tossed => ExpressionKind::Startled,
    }
}

pub(super) fn default_eyelids(action: ActionKind, frame: u8) -> EyelidPose {
    if action == ActionKind::Sleep {
        EyelidPose::Closed
    } else if frame % 8 == 7 {
        EyelidPose::Half
    } else {
        EyelidPose::Open
    }
}

pub(super) fn resolve_expression(creature: &Creature) -> ExpressionKind {
    let drives = &creature.state.drives;
    if creature.state.attention.is_none()
        && let Some(beat) = creature.state.beat
    {
        return super::beats::beat_expression(beat);
    }
    if let Some((habit, ..)) = flourish_shown(creature) {
        return match habit {
            Habit::LooksFoodOver => ExpressionKind::Curious,
            Habit::StretchesBeforeNaps | Habit::CirclesBeforeNaps => ExpressionKind::Content,
            Habit::WavesHello | Habit::PlayBows => ExpressionKind::Joy,
        };
    }
    if creature.state.action == ActionKind::Sleep {
        return ExpressionKind::Sleepy;
    }
    if let Some((own, _)) = own_moment(creature) {
        return match own {
            Gesture::Groom | Gesture::Stretch => ExpressionKind::Content,
            Gesture::Shake => ExpressionKind::Determined,
            Gesture::Sniff => ExpressionKind::Curious,
            Gesture::Skip => ExpressionKind::Joy,
            _ => ExpressionKind::Sleepy,
        };
    }
    if let Some(pose) = creature.state.attention {
        // A temperament's own poses wear their own faces, whatever the moment is about.
        match pose.gesture {
            Some(Gesture::Huff | Gesture::Stomp) => return ExpressionKind::Grumpy,
            Some(Gesture::Peek) => return ExpressionKind::Worried,
            Some(Gesture::Strut) => return ExpressionKind::Smug,
            Some(Gesture::Beg) => return ExpressionKind::Pleading,
            Some(Gesture::Swoon) => return ExpressionKind::Affectionate,
            _ => {}
        }
        return match pose.emotion {
            formiga_core::AttentionEmotion::Curious => ExpressionKind::Curious,
            formiga_core::AttentionEmotion::Startled => ExpressionKind::Startled,
            formiga_core::AttentionEmotion::Enjoying => ExpressionKind::Joy,
            formiga_core::AttentionEmotion::Concerned => ExpressionKind::Worried,
            formiga_core::AttentionEmotion::Averting => ExpressionKind::Worried,
            formiga_core::AttentionEmotion::Relieved => ExpressionKind::Content,
        };
    }
    if drives.arousal > 0.86 {
        return ExpressionKind::Startled;
    }
    match creature.state.action {
        ActionKind::Idle => {
            if drives.sleep_pressure > 0.72 || drives.energy < 0.2 {
                ExpressionKind::Sleepy
            } else if drives.boredom > 0.66 {
                ExpressionKind::Bored
            } else if drives.comfort > 0.7 {
                ExpressionKind::Content
            } else {
                ExpressionKind::Neutral
            }
        }
        ActionKind::Traverse | ActionKind::SqueezeWindow => {
            if drives.arousal > 0.45 {
                ExpressionKind::Focused
            } else {
                ExpressionKind::Content
            }
        }
        ActionKind::Sprint => {
            if creature.personality.playfulness > 0.68 && drives.arousal < 0.72 {
                ExpressionKind::Joy
            } else {
                ExpressionKind::Determined
            }
        }
        ActionKind::Perch => {
            if drives.sleep_pressure > 0.65 {
                ExpressionKind::Sleepy
            } else {
                ExpressionKind::Content
            }
        }
        ActionKind::Homebound => ExpressionKind::Content,
        ActionKind::InvestigateCursor => {
            if drives.arousal > 0.5 {
                ExpressionKind::Focused
            } else {
                ExpressionKind::Curious
            }
        }
        ActionKind::AvoidCursor => ExpressionKind::Worried,
        ActionKind::ReactToWindow => ExpressionKind::Startled,
        ActionKind::RideWindow => {
            if drives.arousal > 0.45 {
                ExpressionKind::Worried
            } else {
                ExpressionKind::Focused
            }
        }
        ActionKind::SoloPlay => ExpressionKind::Joy,
        ActionKind::Eat | ActionKind::Drink => ExpressionKind::Content,
        ActionKind::Greet | ActionKind::Follow | ActionKind::SocialPlay => {
            if creature.tendencies.sociability >= 35.0 || creature.state.drives.comfort > 0.75 {
                ExpressionKind::Affectionate
            } else if creature.state.action == ActionKind::SocialPlay {
                ExpressionKind::Joy
            } else {
                ExpressionKind::Focused
            }
        }
        ActionKind::Dragged => {
            if drives.arousal > 0.55 {
                ExpressionKind::Startled
            } else {
                ExpressionKind::Curious
            }
        }
        ActionKind::Landing => ExpressionKind::Determined,
        ActionKind::ClimbWindow => ExpressionKind::Determined,
        ActionKind::Dangle => {
            if drives.arousal > 0.5 {
                ExpressionKind::Focused
            } else {
                ExpressionKind::Content
            }
        }
        ActionKind::InspectScreen => ExpressionKind::Curious,
        ActionKind::PresentDiscovery => ExpressionKind::Joy,
        ActionKind::Tossed => ExpressionKind::Startled,
        ActionKind::PetReaction => ExpressionKind::Affectionate,
        ActionKind::Sleep => ExpressionKind::Sleepy,
    }
}

pub(super) fn resolve_eyelids(creature: &Creature, at_cursor: bool) -> EyelidPose {
    if creature.state.attention.is_none()
        && let Some(eyelids) = creature.state.beat.and_then(super::beats::beat_eyelids)
    {
        return eyelids;
    }
    if creature.state.attention.is_some_and(|pose| {
        pose.emotion == formiga_core::AttentionEmotion::Averting
            || pose.gesture == Some(Gesture::Cover)
    }) {
        return EyelidPose::Closed;
    }
    let flourish = flourish_shown(creature);
    // Screwed shut at the top of a stretch, as a stretch is.
    if flourish.is_some_and(|(habit, _, into)| habit == Habit::StretchesBeforeNaps && into >= 0.6) {
        return EyelidPose::Closed;
    }
    // Heavy-lidded on the way to bed, and shut once it is there.
    if creature.state.walking_to_sleep() && flourish.is_none() {
        return EyelidPose::Half;
    }
    if creature.state.action == ActionKind::Sleep && flourish.is_none() {
        return EyelidPose::Closed;
    }
    match own_moment(creature) {
        // Eyes shut to wash and to shake, and at the top of a stretch.
        Some((Gesture::Groom | Gesture::Shake, _)) => return EyelidPose::Closed,
        Some((Gesture::Stretch, frame)) => {
            return if frame >= 2 {
                EyelidPose::Closed
            } else {
                EyelidPose::Half
            };
        }
        Some((Gesture::Sniff | Gesture::Trudge, _)) => return EyelidPose::Half,
        _ => {}
    }
    let expression = resolve_expression(creature);
    if matches!(expression, ExpressionKind::Startled) {
        return EyelidPose::Open;
    }
    if creature.state.presence.measured() {
        let blink = blink_at(creature, expression, at_cursor);
        return if blink == EyelidPose::Open && expression == ExpressionKind::Sleepy {
            EyelidPose::Half
        } else {
            blink
        };
    }
    let elapsed = creature.state.action_elapsed.max(0.0);
    let block = (elapsed / 5.0).floor() as u64;
    let local = elapsed % 5.0;
    let seed = u64::from_le_bytes(creature.behavior_seed[..8].try_into().unwrap());
    let mixed = mix_u64(seed ^ block.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let blink_at = 0.65 + (mixed % 360) as f32 / 100.0;
    let blink_delta = (local - blink_at).abs();
    if blink_delta < 0.055 {
        EyelidPose::Closed
    } else if blink_delta < 0.14 || resolve_expression(creature) == ExpressionKind::Sleepy {
        EyelidPose::Half
    } else {
        EyelidPose::Open
    }
}

pub(super) fn mix_u64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// Where a creature is looking, and whether that is at the cursor.
pub(super) fn resolve_gaze(
    creature: &Creature,
    colony: &[Creature],
    cursor: CursorSnapshot,
    cursor_reactions: bool,
    reduce_motion: bool,
) -> (GazeDirection, bool) {
    let gaze = looking_at(creature, cursor, cursor_reactions);
    match gaze {
        Looking::Fixed(gaze) => (gaze, false),
        Looking::Cursor(gaze) => (gaze, true),
        Looking::Free => (
            if reduce_motion || !creature.state.presence.measured() {
                GazeDirection::default()
            } else {
                wandering_gaze(creature, colony)
            },
            false,
        ),
    }
}

/// What decides where a creature looks: something it is looking at, the cursor, or nothing, which
/// leaves its eyes free to wander.
enum Looking {
    Fixed(GazeDirection),
    Cursor(GazeDirection),
    Free,
}

fn looking_at(creature: &Creature, cursor: CursorSnapshot, cursor_reactions: bool) -> Looking {
    let fixed = |gaze| Looking::Fixed(gaze);
    if let Some(pose) = creature.state.attention {
        return fixed(GazeDirection::new(
            axis_direction(pose.target.x - creature.state.position.x, 10.0),
            axis_direction(pose.target.y - (creature.state.position.y - 28.0), 10.0),
        ));
    }
    if let Some(gaze) = creature
        .state
        .beat
        .and_then(|beat| super::beats::beat_gaze(creature, beat))
    {
        return fixed(gaze);
    }
    let forward = if creature.state.facing_right { 1 } else { -1 };
    // Looking the snack over: down at it, closer, then up and along it.
    if let Some((Habit::LooksFoodOver, _, into)) = flourish_shown(creature) {
        return fixed(match into {
            into if into < 0.55 => GazeDirection::new(forward, 1),
            into if into < 1.1 => GazeDirection::new(0, 1),
            _ => GazeDirection::new(forward, 0),
        });
    }
    match own_moment(creature) {
        // Nose up at whatever is on the air; down at the paw it is washing, or the ground ahead.
        Some((Gesture::Sniff, _)) => return fixed(GazeDirection::new(forward, -1)),
        Some((Gesture::Groom | Gesture::Trudge, _)) => {
            return fixed(GazeDirection::new(forward, 1));
        }
        _ => {}
    }
    match creature.state.action {
        ActionKind::InspectScreen => {
            return fixed(GazeDirection::new(
                if creature.state.facing_right { 1 } else { -1 },
                if creature.state.surface.kind == formiga_core::SurfaceKind::WindowLedge {
                    1
                } else {
                    -1
                },
            ));
        }
        ActionKind::Dangle => return fixed(GazeDirection::new(0, 1)),
        ActionKind::PresentDiscovery => return fixed(GazeDirection::new(0, -1)),
        _ => {}
    }
    if !cursor_reactions
        || !cursor.available
        || creature.state.position.distance(cursor.position) > 240.0
    {
        return Looking::Free;
    }
    let face_position = formiga_core::Point {
        x: creature.state.position.x,
        y: creature.state.position.y - 28.0,
    };
    let dx = cursor.position.x - face_position.x;
    let dy = cursor.position.y - face_position.y;
    Looking::Cursor(GazeDirection::new(
        axis_direction(dx, 10.0),
        axis_direction(dy, 10.0),
    ))
}

pub(super) fn axis_direction(delta: f32, dead_zone: f32) -> i8 {
    if delta.abs() <= dead_zone {
        0
    } else if delta > 0.0 {
        1
    } else {
        -1
    }
}

/// A number from a creature's own behaviour seed and a salt, so its blinks and its glances each
/// come from a stream of their own.
fn own_roll(creature: &Creature, salt: u64) -> u64 {
    let seed = u64::from_le_bytes(creature.behavior_seed[..8].try_into().unwrap());
    mix_u64(seed ^ salt)
}

/// A fraction from 0 to 1 out of eight bits of a roll.
fn fraction(roll: u64, shift: u32) -> f32 {
    ((roll >> shift) & 0xff) as f32 / 255.0
}

/// How long a creature lets go between one look about and the next, in seconds: a curious one
/// looks about more often. One glance at most falls in each stretch.
fn glance_stretch(curiosity: f32) -> f32 {
    3.0 + (1.0 - curiosity.clamp(0.0, 1.0)) * 3.5
}

/// One glance: when it starts and ends on the creature's clock, and the roll that says where.
#[derive(Clone, Copy)]
struct Glance {
    start: f32,
    end: f32,
    roll: u64,
}

/// The glance falling in the stretch of the creature's clock that `clock` is in, if one does.
fn glance_near(creature: &Creature, clock: f32) -> Option<Glance> {
    let axes = creature.temperament().axes;
    let stretch = glance_stretch(axes.curiosity);
    // Everyone's stretches start at a different moment, so a colony does not look about in step.
    let offset = fraction(own_roll(creature, 0x6c61_6e63), 0) * stretch;
    let shifted = clock + offset;
    let index = (shifted / stretch).floor();
    let roll = own_roll(
        creature,
        0x676c_616e ^ (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15),
    );
    if fraction(roll, 0) > 0.35 + 0.35 * axes.curiosity {
        return None;
    }
    let from = index * stretch - offset;
    let start = from + fraction(roll, 8) * stretch * 0.4;
    let length = 0.9 + fraction(roll, 16) * stretch * 0.45;
    let end = (start + length).min(from + stretch - 0.3);
    Some(Glance { start, end, roll })
}

/// Where a creature's eyes go when nothing in particular has them: where it is going while it
/// walks, and straight out while it rests, with now and then a glance — at a friend nearby, a
/// sociable companion more often than a solitary one, or off to one side, down or up — and a
/// companion walking with or waiting on a friend keeping an eye on them throughout.
///
/// Nine gaze directions are already in every face texture, so a glance costs nothing to draw,
/// and it lasts a second or two, long enough to be seen at the slowest rate a resting colony is
/// drawn at.
fn wandering_gaze(creature: &Creature, colony: &[Creature]) -> GazeDirection {
    let state = &creature.state;
    let forward = if state.facing_right { 1 } else { -1 };
    let walking = state.presence.stepping;
    let friend = nearest_friend(creature, colony);
    let toward = |other: &Creature| {
        GazeDirection::new(
            axis_direction(other.state.position.x - state.position.x, 10.0),
            axis_direction(other.state.position.y - state.position.y, 18.0),
        )
    };
    let with_friend = matches!(
        state.action,
        ActionKind::Follow | ActionKind::Greet | ActionKind::SocialPlay
    );
    let clock = state.presence.clock;
    let glance = glance_near(creature, clock).filter(|g| (g.start..g.end).contains(&clock));
    let Some(glance) = glance else {
        return match friend {
            Some(friend) if with_friend && !walking => toward(friend),
            _ if walking => GazeDirection::new(forward, 0),
            _ => GazeDirection::default(),
        };
    };
    let social = creature.temperament().axes.social;
    if let Some(friend) = friend
        && (with_friend || fraction(glance.roll, 24) < 0.35 + 0.45 * social)
    {
        return toward(friend);
    }
    let pick = (glance.roll >> 32) as usize;
    if walking {
        // A walker glances out at the desk, or down at where its feet are going.
        return [GazeDirection::new(0, 0), GazeDirection::new(forward, 1)][pick % 2];
    }
    [
        GazeDirection::new(forward, 0),
        GazeDirection::new(-forward, 0),
        GazeDirection::new(forward, 1),
        GazeDirection::new(forward, -1),
        GazeDirection::new(0, -1),
        GazeDirection::new(-forward, -1),
    ][pick % 6]
}

/// The nearest other companion out on the same display and close enough to look at.
fn nearest_friend<'a>(creature: &Creature, colony: &'a [Creature]) -> Option<&'a Creature> {
    const REACH: f32 = 260.0;
    colony
        .iter()
        .filter(|other| {
            other.id != creature.id
                && !other.state.indoors
                && other.state.arrival_delay_secs <= 0.0
                && other.state.surface.monitor_id == creature.state.surface.monitor_id
        })
        .map(|other| {
            (
                other.state.position.distance(creature.state.position),
                other,
            )
        })
        .filter(|(distance, _)| *distance <= REACH)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, other)| other)
}

/// The eyelids a creature's own blinking gives it at this moment on its clock.
///
/// Blinks come irregularly rather than on a beat: one somewhere in each stretch of three and a
/// half to six seconds — longer the calmer the companion — now and then none, and now and
/// then two in quick succession. A content or affectionate companion, or one fond of the person
/// gazing at their cursor, blinks slowly instead: lids easing down and up again, the way a cat
/// says it is at ease. A glance often begins with a blink, as eyes moving do. Each blink is long
/// enough to be caught at the slowest rate a resting colony is drawn at, so blinking asks for no
/// frame of its own.
fn blink_at(creature: &Creature, expression: ExpressionKind, at_cursor: bool) -> EyelidPose {
    let clock = creature.state.presence.clock;
    let axes = creature.temperament().axes;
    let calm = (1.0 - (axes.energy + axes.impulsiveness) / 2.0).clamp(0.0, 1.0);
    let stretch = 3.4 + calm * 2.6;
    let index = (clock / stretch).floor();
    let local = clock - index * stretch;
    let roll = own_roll(
        creature,
        0x626c_696e ^ (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15),
    );
    let fond = at_cursor && axes.affection > 0.55;
    let slow = (matches!(
        expression,
        ExpressionKind::Content | ExpressionKind::Affectionate
    ) || fond)
        && fraction(roll, 24) < 0.5;
    let lids = |delta: f32, closed: f32, half: f32| {
        if delta < closed {
            EyelidPose::Closed
        } else if delta < half {
            EyelidPose::Half
        } else {
            EyelidPose::Open
        }
    };
    let mut shown = EyelidPose::Open;
    let mut close = |pose: EyelidPose| {
        if pose == EyelidPose::Closed || (pose == EyelidPose::Half && shown == EyelidPose::Open) {
            shown = pose;
        }
    };
    if fraction(roll, 0) >= 0.12 {
        let at = 0.45 + fraction(roll, 8) * (stretch - 1.4);
        if slow {
            close(lids((local - at).abs(), 0.18, 0.42));
        } else {
            close(lids((local - at).abs(), 0.08, 0.2));
            if fraction(roll, 16) < 0.18 {
                close(lids((local - at - 0.45).abs(), 0.08, 0.2));
            }
        }
    }
    if let Some(glance) = glance_near(creature, clock)
        && fraction(glance.roll, 40) < 0.6
    {
        close(lids((clock - glance.start).abs(), 0.08, 0.2));
    }
    shown
}

/// Something a companion is doing with itself on its own account — a wash, a shake, a sniff, a
/// stretch on waking, a walk with a mood in it, a way of sleeping — and the frame of it, when
/// nothing its attention is on, no small moment and no habit has the body instead.
fn own_moment(creature: &Creature) -> Option<(Gesture, u8)> {
    let state = &creature.state;
    if state.attention.is_some() || state.beat.is_some() || flourish_shown(creature).is_some() {
        return None;
    }
    match BodyPresentation::for_creature(creature) {
        BodyPresentation {
            clip:
                BodyClip::Gesture(
                    gesture @ (Gesture::Groom
                    | Gesture::Shake
                    | Gesture::Sniff
                    | Gesture::Stretch
                    | Gesture::Skip
                    | Gesture::Trudge
                    | Gesture::Sprawl
                    | Gesture::Loaf),
                ),
            frame,
            ..
        } => Some((gesture, frame)),
        _ => None,
    }
}
