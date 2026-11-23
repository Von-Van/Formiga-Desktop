//! Vignettes: the little pictures the notebook tapes in beside a moment worth one — an arrival,
//! two companions growing close, a first find, a ritual, a visitor at the houses. Each is
//! composed from frames the creatures already have, standing on a strip of ground under a patch
//! of sky, with nothing drawn that the desktop does not draw. The same moment and the same
//! companions always make the same picture.

use crate::{Canvas, CreatureRenderer, Rgba, standing_row};
use formiga_core::{ActionKind, AppearanceGenome, JournalMoment, RitualKind};

/// How big a vignette is, in art pixels. The notebook draws it at a whole number of screen points
/// to each.
pub const VIGNETTE_WIDTH: u32 = 176;
pub const VIGNETTE_HEIGHT: u32 = 72;

/// The row the companions stand on.
const GROUND: i32 = 60;

/// What a vignette shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Vignette {
    /// A companion arriving: it waves hello under a little shower of sparkles.
    Arrival,
    /// Two companions growing close: facing each other with a heart between them.
    Friendship,
    /// A first find: the finder holding it up, with room over its paws for the find itself.
    Find,
    /// The colony sharing a ritual, everyone doing it together.
    Ritual(RitualKind),
    /// Someone at the houses: a companion waving to a visitor, who is a shape against the sky.
    Visit,
}

impl Vignette {
    /// The vignette a journal moment is pictured with, if it is one worth a picture.
    pub fn for_moment(moment: &JournalMoment) -> Option<Self> {
        Some(match moment {
            JournalMoment::Arrival => Self::Arrival,
            JournalMoment::Friendship(_) => Self::Friendship,
            JournalMoment::Discovery => Self::Find,
            JournalMoment::Ritual(kind) => Self::Ritual(*kind),
            JournalMoment::Visit(_) | JournalMoment::Revisit(..) => Self::Visit,
            _ => return None,
        })
    }

    /// Every kind, one ritual standing for each pose a ritual is drawn with, for the review sheet.
    pub const ALL: [Self; 9] = [
        Self::Arrival,
        Self::Friendship,
        Self::Find,
        Self::Ritual(RitualKind::Picnic),
        Self::Ritual(RitualKind::GroupNap),
        Self::Ritual(RitualKind::LateNightSleepPile),
        Self::Ritual(RitualKind::Dance),
        Self::Ritual(RitualKind::FloorRace),
        Self::Visit,
    ];

    /// Where the find goes over the finder's paws, for the notebook to draw it there: the top left
    /// of a 16-pixel square, in art pixels. Only a find has one.
    pub const fn find_spot(self) -> Option<(i32, i32)> {
        match self {
            Self::Find => Some((VIGNETTE_WIDTH as i32 / 2 - 8, 14)),
            _ => None,
        }
    }

    /// How many companions it shows at most.
    pub const fn cast_size(self) -> usize {
        match self {
            Self::Arrival | Self::Find | Self::Visit => 1,
            Self::Friendship => 2,
            Self::Ritual(_) => 4,
        }
    }

    fn night(self) -> bool {
        matches!(
            self,
            Self::Ritual(RitualKind::LateNightSleepPile | RitualKind::QuietDayHuddle)
        )
    }
}

const SKY: Rgba = Rgba::new(222, 236, 226, 255);
const NIGHT_SKY: Rgba = Rgba::new(58, 70, 92, 255);
const GROUND_FILL: Rgba = Rgba::new(150, 186, 118, 255);
const GROUND_EDGE: Rgba = Rgba::new(98, 140, 82, 255);
const TUFT: Rgba = Rgba::new(84, 124, 70, 255);
const SPARKLE: Rgba = Rgba::new(246, 206, 92, 255);
const HEART: Rgba = Rgba::new(222, 96, 112, 255);
const BLANKET: Rgba = Rgba::new(214, 112, 96, 255);
const BLANKET_CHECK: Rgba = Rgba::new(246, 226, 204, 255);
const STAR: Rgba = Rgba::new(250, 244, 210, 255);
const STRANGER: Rgba = Rgba::new(96, 104, 112, 255);

/// Pastes `frame` with its feet on the ground, its middle at `x`.
fn stand(canvas: &mut Canvas, frame: &Canvas, x: i32) {
    let feet = standing_row(frame) as i32;
    let left = x - frame.width() as i32 / 2;
    let top = GROUND - feet;
    for row in 0..frame.height() as i32 {
        for column in 0..frame.width() as i32 {
            let pixel = frame.get(column, row);
            if pixel.a > 0 {
                canvas.set(left + column, top + row, pixel);
            }
        }
    }
}

/// A small plus-shaped twinkle.
fn sparkle(canvas: &mut Canvas, x: i32, y: i32, colour: Rgba) {
    canvas.set(x, y, colour);
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        canvas.set(x + dx, y + dy, colour);
    }
}

/// A pixel heart, seven wide.
fn heart(canvas: &mut Canvas, x: i32, y: i32) {
    for (row, line) in [
        " xx xx ", "xxxxxxx", "xxxxxxx", " xxxxx ", "  xxx  ", "   x   ",
    ]
    .iter()
    .enumerate()
    {
        for (column, c) in line.chars().enumerate() {
            if c == 'x' {
                canvas.set(x + column as i32, y + row as i32, HEART);
            }
        }
    }
}

/// What a companion is doing in a ritual's vignette, and which frame of it.
fn ritual_pose(kind: RitualKind, index: usize) -> (ActionKind, u8) {
    match kind {
        RitualKind::GroupNap | RitualKind::QuietDayHuddle | RitualKind::LateNightSleepPile => {
            (ActionKind::Sleep, (index % 2) as u8)
        }
        RitualKind::Picnic => (ActionKind::Eat, (index % 4) as u8),
        RitualKind::Dance => (ActionKind::SocialPlay, (index % 4) as u8),
        RitualKind::FloorRace => (ActionKind::Sprint, (index * 2 % 6) as u8),
        RitualKind::Catch => (ActionKind::SoloPlay, (index % 4) as u8),
        RitualKind::ShelterGathering | RitualKind::GroupPresentation | RitualKind::HatchDay => {
            (ActionKind::Greet, (index % 4) as u8)
        }
    }
}

impl Vignette {
    /// The flat colours a vignette's backdrop is made of — its sky, its ground and the line along
    /// the ground's edge, its tufts of grass and, at night, its stars — as rectangles in art
    /// pixels, `(x, y, width, height, colour)`, so a page can paint them as fills rather than
    /// hold them as a picture.
    pub fn backdrop(self) -> Vec<(i32, i32, i32, i32, Rgba)> {
        let (width, height) = (VIGNETTE_WIDTH as i32, VIGNETTE_HEIGHT as i32);
        let night = self.night();
        let mut fills = vec![
            (0, 0, width, GROUND, if night { NIGHT_SKY } else { SKY }),
            (0, GROUND, width, height - GROUND, GROUND_FILL),
            (0, GROUND, width, 1, GROUND_EDGE),
        ];
        for x in [9, 31, 70, 118, 151, 167] {
            fills.push((x, GROUND - 1, 1, 1, TUFT));
            fills.push((x + 1, GROUND - 2, 1, 1, TUFT));
            fills.push((x + 2, GROUND - 1, 1, 1, TUFT));
        }
        if night {
            for (x, y) in [(14, 8), (40, 18), (77, 6), (122, 14), (160, 9), (101, 22)] {
                fills.push((x, y, 1, 1, STAR));
            }
        }
        fills
    }
}

/// The picture for `vignette`, with `cast` the companions it is about, first the one it is
/// mostly about: its backdrop with `render_vignette_figures` over it.
pub fn render_vignette(vignette: Vignette, cast: &[&AppearanceGenome]) -> Canvas {
    let mut canvas = Canvas::new(VIGNETTE_WIDTH, VIGNETTE_HEIGHT);
    for (x, y, width, height, colour) in vignette.backdrop() {
        canvas.fill_rect(x, y, width, height, colour);
    }
    let figures = render_vignette_figures(vignette, cast);
    for y in 0..VIGNETTE_HEIGHT as i32 {
        for x in 0..VIGNETTE_WIDTH as i32 {
            let pixel = figures.get(x, y);
            if pixel.a > 0 {
                canvas.set(x, y, pixel);
            }
        }
    }
    canvas
}

/// Everything in a vignette but its backdrop — the companions and the sparkles, heart, blanket
/// or shape that go with them — on a clear canvas the size of the whole picture. A vignette with
/// nobody to show is its sparkles alone.
pub fn render_vignette_figures(vignette: Vignette, cast: &[&AppearanceGenome]) -> Canvas {
    let width = VIGNETTE_WIDTH as i32;
    let mut canvas = Canvas::new(VIGNETTE_WIDTH, VIGNETTE_HEIGHT);
    let cast: Vec<&AppearanceGenome> = cast.iter().copied().take(vignette.cast_size()).collect();
    let middle = width / 2;
    match vignette {
        Vignette::Arrival => {
            if let Some(genome) = cast.first() {
                stand(
                    &mut canvas,
                    &CreatureRenderer::render_frame(genome, ActionKind::Greet, 1, true),
                    middle,
                );
            }
            for (x, y) in [
                (middle - 26, 14),
                (middle + 24, 10),
                (middle - 14, 4),
                (middle + 12, 22),
            ] {
                sparkle(&mut canvas, x, y, SPARKLE);
            }
        }
        Vignette::Friendship => {
            let place = [(middle - 26, true), (middle + 26, false)];
            for (genome, (x, facing_right)) in cast.iter().zip(place) {
                stand(
                    &mut canvas,
                    &CreatureRenderer::render_frame(genome, ActionKind::Greet, 1, facing_right),
                    x,
                );
            }
            heart(&mut canvas, middle - 3, 12);
        }
        Vignette::Find => {
            if let Some(genome) = cast.first() {
                stand(
                    &mut canvas,
                    &CreatureRenderer::render_frame(genome, ActionKind::PresentDiscovery, 1, true),
                    middle,
                );
            }
            for (x, y) in [(middle - 16, 6), (middle + 15, 10)] {
                sparkle(&mut canvas, x, y, SPARKLE);
            }
        }
        Vignette::Ritual(kind) => {
            if kind == RitualKind::Picnic {
                let across = (cast.len().max(1) as i32) * 38 + 12;
                let left = middle - across / 2;
                canvas.fill_rect(left, GROUND - 1, across, 4, BLANKET);
                for x in (left..left + across).step_by(4) {
                    canvas.set(x, GROUND, BLANKET_CHECK);
                    canvas.set(x + 2, GROUND + 2, BLANKET_CHECK);
                }
            }
            let count = cast.len() as i32;
            let spacing = if kind == RitualKind::LateNightSleepPile {
                26
            } else {
                38
            };
            let first = middle - (count - 1) * spacing / 2;
            for (index, genome) in cast.iter().enumerate() {
                let (action, frame) = ritual_pose(kind, index);
                let facing_right = index * 2 < cast.len() || kind == RitualKind::FloorRace;
                stand(
                    &mut canvas,
                    &CreatureRenderer::render_frame(genome, action, frame, facing_right),
                    first + index as i32 * spacing,
                );
            }
        }
        Vignette::Visit => {
            if let Some(genome) = cast.first() {
                stand(
                    &mut canvas,
                    &CreatureRenderer::render_frame(genome, ActionKind::Greet, 1, true),
                    middle - 30,
                );
                // Whoever came by is not anyone the colony keeps a picture of, so they stand as
                // a shape against the sky.
                let mut stranger =
                    CreatureRenderer::render_frame(genome, ActionKind::Idle, 0, false);
                for y in 0..stranger.height() as i32 {
                    for x in 0..stranger.width() as i32 {
                        if stranger.get(x, y).a > 0 {
                            stranger.set(x, y, STRANGER);
                        }
                    }
                }
                stand(&mut canvas, &stranger, middle + 32);
            }
        }
    }
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::{DesktopSnapshot, World};
    use time::OffsetDateTime;

    fn genome(seed: u8) -> AppearanceGenome {
        World::preview_adult(
            [seed; 32],
            OffsetDateTime::UNIX_EPOCH,
            &DesktopSnapshot::default(),
        )
        .appearance
    }

    #[test]
    fn every_vignette_draws_its_cast_on_the_ground_and_the_same_way_twice() {
        let cast = [genome(1), genome(2), genome(3), genome(4)];
        let cast: Vec<&AppearanceGenome> = cast.iter().collect();
        for vignette in Vignette::ALL {
            let picture = render_vignette(vignette, &cast);
            assert_eq!(picture.width(), VIGNETTE_WIDTH);
            assert_eq!(picture.height(), VIGNETTE_HEIGHT);
            assert_eq!(picture, render_vignette(vignette, &cast), "{vignette:?}");
            let backdrop = render_vignette(vignette, &[]);
            let drawn = picture
                .pixels()
                .iter()
                .zip(backdrop.pixels())
                .filter(|(a, b)| a != b)
                .count();
            assert!(drawn > 200, "{vignette:?} shows its companions: {drawn}");
        }
    }

    #[test]
    fn only_moments_worth_a_picture_have_one() {
        assert_eq!(
            Vignette::for_moment(&JournalMoment::Arrival),
            Some(Vignette::Arrival)
        );
        assert_eq!(
            Vignette::for_moment(&JournalMoment::Ritual(RitualKind::Dance)),
            Some(Vignette::Ritual(RitualKind::Dance))
        );
        assert_eq!(Vignette::for_moment(&JournalMoment::Trip), None);
        assert_eq!(Vignette::Find.find_spot(), Some((80, 14)));
    }
}
