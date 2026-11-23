//! Slipping through a narrow gap: the body drawn pinched to about three quarters of its width.
//!
//! The squeeze is drawn from the body's own pose for it, face and all, by taking out about a
//! quarter of the columns the body spans. They are not taken out evenly, as a narrowed quad would
//! drop them: each one is a seam from the top of the frame to the bottom, one pixel a row, that
//! goes where taking a pixel out changes the least — through flat coat, and the gaps between ears
//! and between legs — and, while anything else will do, never through an outline, the face, a
//! thin leg or ear, or the last pixel keeping two parts apart. So the outline stays whole all the
//! way round, the eyes come closer together across a narrower head, and the feet keep their own
//! shapes, only nearer each other.
use super::{
    Canvas, EyelidPose, FaceRenderState, GazeDirection, PixelPoint, Rgba, expression_for_action,
};
use formiga_core::ActionKind;

/// How much of its width a squeezing body keeps.
pub(super) const SQUEEZED_WIDTH: f32 = 0.75;

/// The face drawn into a squeeze: intent on getting through, eyes open on the way ahead.
pub(super) fn face() -> FaceRenderState {
    FaceRenderState {
        expression: expression_for_action(ActionKind::SqueezeWindow),
        eyelids: EyelidPose::Open,
        gaze: GazeDirection::new(1, 0),
    }
}

/// The cost of taking out a pixel outside what a row draws. Seams cross empty rows freely, but in
/// a row that draws something they go through it, so that row is narrowed rather than moved.
const BESIDE: u32 = 60;
/// The cost of taking out the edge of a shape (a pixel beside a clear one in its row): never,
/// while anything else will do, so the outline is never broken.
const EDGE: u32 = 400;
/// The cost of taking out a pixel unlike both of its neighbours: a one-pixel detail, a pupil, a
/// highlight or a line of outline across the body.
const DETAIL: u32 = 120;
/// The cost of taking out a pixel of something thin — a leg, an ear, a tail seen end on — which
/// would leave it a line.
const THIN: u32 = 300;
/// How wide, outline and all, a part can be and still count as thin.
const THIN_WIDTH: usize = 5;
/// The cost of taking out a pixel of the face: its eyes and mouth keep their shapes, and only
/// the coat between them is taken in.
const FACE: u32 = 240;
/// The cost of taking out a clear pixel beside a drawn one, in a gap: the parts either side of
/// it come a pixel closer.
const GAP_EDGE: u32 = 1;
/// The cost of taking out the last clear pixel between two parts, which would run them together:
/// two legs into one, an ear into the next.
const GAP_CLOSE: u32 = 400;

/// Which columns of each row a squeeze takes out, and how far what is left is moved over so the
/// body stays centred where it stood.
#[derive(Clone, Debug)]
pub(super) struct Pinch {
    /// For each row, whether each of its pixels is kept.
    kept: Vec<Vec<bool>>,
    /// Where the first kept pixel of every row goes.
    offset: i32,
}

impl Pinch {
    /// The pinch that narrows what `canvas` draws to [`SQUEEZED_WIDTH`] of its width, keeping
    /// whatever `face`, drawn with its top-left corner at `face_at`, puts over it.
    pub(super) fn of(canvas: &Canvas, face: &Canvas, face_at: PixelPoint) -> Self {
        let (width, height) = (canvas.width() as usize, canvas.height() as usize);
        let in_face =
            |x: usize, y: usize| face.get(x as i32 - face_at.x, y as i32 - face_at.y).a > 0;
        let mut rows: Vec<Vec<Pixel>> = (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| Pixel {
                        column: x,
                        colour: canvas.get(x as i32, y as i32),
                        face: in_face(x, y),
                    })
                    .collect()
            })
            .collect();
        let Some((min_x, _, max_x, _)) = canvas.alpha_bounds() else {
            return Self {
                kept: vec![vec![true; width]; height],
                offset: 0,
            };
        };
        let span = (max_x - min_x + 1) as f32;
        let seams = (span * (1.0 - SQUEEZED_WIDTH)).round() as usize;
        for _ in 0..seams {
            let seam = cheapest_seam(&rows);
            for (row, x) in rows.iter_mut().zip(seam) {
                row.remove(x);
            }
        }
        let mut kept = vec![vec![false; width]; height];
        for (y, row) in rows.iter().enumerate() {
            for pixel in row {
                kept[y][pixel.column] = true;
            }
        }
        // Centred where it was: the middle of what is left over the middle of what was drawn.
        let (mut left, mut right) = (usize::MAX, 0);
        for row in &rows {
            for (at, pixel) in row.iter().enumerate() {
                if pixel.colour.a > 0 {
                    left = left.min(at);
                    right = right.max(at);
                }
            }
        }
        let was = (min_x + max_x) as f32 / 2.0;
        let now = (left + right) as f32 / 2.0;
        Self {
            kept,
            offset: (was - now).round() as i32,
        }
    }

    /// `canvas`, drawn `detail` pixels to each pixel the pinch was found on, pinched.
    pub(super) fn apply(&self, canvas: &Canvas, detail: u32) -> Canvas {
        let detail = detail.max(1) as i32;
        let mut out = Canvas::new(canvas.width(), canvas.height());
        for y in 0..canvas.height() as i32 {
            let row = &self.kept[(y / detail) as usize];
            let mut to = self.offset * detail;
            for (column, kept) in row.iter().enumerate() {
                if !kept {
                    continue;
                }
                for sub in 0..detail {
                    let pixel = canvas.get(column as i32 * detail + sub, y);
                    if pixel.a > 0 {
                        out.set(to, y, pixel);
                    }
                    to += 1;
                }
            }
        }
        out
    }

    /// Where a point of the frame goes, in frame pixels.
    pub(super) fn point(&self, at: PixelPoint) -> PixelPoint {
        let Some(row) = usize::try_from(at.y).ok().and_then(|y| self.kept.get(y)) else {
            return at;
        };
        let before = row
            .iter()
            .take(at.x.clamp(0, row.len() as i32) as usize)
            .filter(|kept| **kept)
            .count() as i32;
        PixelPoint {
            x: before + self.offset,
            y: at.y,
        }
    }
}

/// One pixel of a row being pinched: the column of the frame it came from, its colour, and
/// whether it is part of the face.
#[derive(Clone, Copy, Debug)]
struct Pixel {
    column: usize,
    colour: Rgba,
    face: bool,
}

/// The seam whose pixels cost least to take out, as the column it takes from each row.
///
/// Besides what each pixel costs, a seam pays for the lines its own steps would break: where it
/// steps sideways between two rows, the pixels that end up side by side across the step are
/// compared too, so a slanting stripe or an edge of shading is cut where it stays a line.
fn cheapest_seam(rows: &[Vec<Pixel>]) -> Vec<usize> {
    let height = rows.len();
    let width = rows.first().map_or(0, Vec::len);
    let mut total = vec![vec![0_u32; width]; height];
    let mut from = vec![vec![0_usize; width]; height];
    let colour = |y: usize, x: Option<usize>| {
        x.and_then(|x| rows[y].get(x))
            .map_or(Rgba::new(0, 0, 0, 0), |pixel| pixel.colour)
    };
    for (y, row) in rows.iter().enumerate() {
        let drawn: Vec<usize> = (0..width).filter(|x| row[*x].colour.a > 0).collect();
        let extent = drawn.first().zip(drawn.last());
        for x in 0..width {
            let cost = match extent {
                None => 0,
                Some((first, last)) if x < *first || x > *last => BESIDE,
                Some(_) => cost(row, x),
            };
            if y == 0 {
                total[y][x] = cost;
                from[y][x] = x;
                continue;
            }
            let up = colour(y - 1, Some(x));
            let left = colour(y, x.checked_sub(1));
            let right = colour(y, Some(x + 1));
            let (best, came) = [
                x.checked_sub(1),
                Some(x),
                Some(x + 1).filter(|at| *at < width),
            ]
            .into_iter()
            .flatten()
            .map(|at| {
                let step = match at.cmp(&x) {
                    std::cmp::Ordering::Less => difference(up, left),
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => difference(up, right),
                };
                (total[y - 1][at] + step, at.abs_diff(x), at)
            })
            .min()
            .map(|(sum, _, at)| (sum, at))
            .unwrap_or((0, x));
            total[y][x] = cost + best;
            from[y][x] = came;
        }
    }
    let mut seam = vec![0; height];
    let Some(last) = total.last() else {
        return seam;
    };
    // Ties go to the middle, so a seam through nothing much runs down the body's centre line.
    let middle = width as i32 / 2;
    let mut x = (0..width)
        .min_by_key(|x| (last[*x], (*x as i32 - middle).abs()))
        .unwrap_or(0);
    for y in (0..height).rev() {
        seam[y] = x;
        x = from[y][x];
    }
    seam
}

/// How unlike two colours are, as a seam's cost: nothing for the same colour, a little for two
/// shades of a coat, a lot for coat against outline or against nothing.
fn difference(a: Rgba, b: Rgba) -> u32 {
    let channel = |a: u8, b: u8| u32::from(a.abs_diff(b));
    (channel(a.r, b.r) + channel(a.g, b.g) + channel(a.b, b.b) + channel(a.a, b.a)) / 8
}

/// What taking out pixel `x` of a row that draws something costs.
fn cost(row: &[Pixel], x: usize) -> u32 {
    let clear = Rgba::new(0, 0, 0, 0);
    let pixel = row[x].colour;
    let left = if x == 0 { clear } else { row[x - 1].colour };
    let right = row.get(x + 1).map_or(clear, |pixel| pixel.colour);
    if pixel.a == 0 {
        return match (left.a > 0, right.a > 0) {
            (true, true) => GAP_CLOSE,
            (false, false) => 0,
            _ => GAP_EDGE,
        };
    }
    if left.a == 0 || right.a == 0 {
        return EDGE;
    }
    // The face, and the last of the coat between two parts of it, so the eyes never run into
    // each other.
    if row[x].face || (x > 0 && row[x - 1].face && row.get(x + 1).is_some_and(|p| p.face)) {
        return FACE;
    }
    let run = |step: &dyn Fn(usize) -> Option<usize>| {
        std::iter::successors(Some(x), |at| step(*at))
            .take_while(|at| row.get(*at).is_some_and(|pixel| pixel.colour.a > 0))
            .count()
    };
    if run(&|at| at.checked_sub(1)) + run(&|at| Some(at + 1)) - 1 <= THIN_WIDTH {
        return THIN;
    }
    // The two pixels either side end up side by side.
    let joined = difference(left, right);
    if pixel != left && pixel != right {
        DETAIL + joined
    } else {
        joined
    }
}
