//! The whole-colony portrait: one illustrated keepsake of the village and everyone who lives in
//! it, in the same family as the single-creature card.
//!
//! It shows only what a creature card may already show — rendered pixels, names, counts, and
//! arrival months. No seed codes, no memories, no relationship scores, no journal text, no display
//! keys, no screen information, no visitors, and no guest book.

use crate::card::{
    CREAM, CardRng, CardText, INK, MUTED_INK, NIGHT_LIGHT, PAPER, PAPER_SHADOW, blend, blit_scaled,
    draw_background, draw_chip, draw_motif, month_label, stepped_panel,
};
use crate::{
    COLONY_OBJECT_SIZE, Canvas, ColonyObjectRenderer, CreatureRenderer, ExpressionKind, EyelidPose,
    FRAME_SIZE, FaceRenderState, GazeDirection, PALETTES, Palette, Rgba, SHELTER_SIZE,
    ShelterRenderer,
};
use formiga_core::{
    ActionKind, ColonyObjectKind, Creature, CreatureRole, DesktopRect, DisplayKey, DwellingKind,
    HabitatPolicy, MonitorInfo, SaveFile, TreeEnd,
};

pub const COLONY_CARD_WIDTH: u32 = 960;
pub const COLONY_CARD_HEIGHT: u32 = 600;

/// Where the scene's own pixels live, inside the stepped frame.
const SCENE_LEFT: i32 = 72;
const SCENE_RIGHT: i32 = 888;
const SCENE_TOP: i32 = 115;
const SCENE_BOTTOM: i32 = 473;

/// Where the sky stops and the ground begins.
const HORIZON: i32 = 236;

/// The ground line the village stands on, at the back of the scene.
const VILLAGE_GROUND: i32 = 296;

/// The near floor the colony stands on, at the front.
const COLONY_GROUND: i32 = 424;

/// The paper strip along the bottom of the scene that the names are written on.
const NAME_PLAQUE_TOP: i32 = 436;

/// Air kept clear at each end of the row so nobody is pressed against the frame.
const ROW_PADDING: i32 = 20;

/// Whitespace between two neighbours in the row.
const ROW_GAP: i32 = 20;

/// The tallest a member may be drawn. A colony standing any taller than this would rise past the
/// village's rooflines and hide the home it is standing in front of.
const MEMBER_CEILING: i32 = 176;

/// The tallest the colony house may be drawn: roughly half a member, which is what reads as a
/// village standing back behind the colony rather than crowding in beside it.
const VILLAGE_HEIGHT_CEILING: i32 = 90;

/// The furthest the village may slide from centre to clear the colony house of a member.
const VILLAGE_NUDGE: i32 = 110;

/// Stateless by design: every allocation a portrait makes is dropped when the export finishes.
pub struct ColonyCardRenderer;

impl ColonyCardRenderer {
    /// Render the colony portrait: the village, everyone who lives in it, and two small facts.
    ///
    /// Nothing about the reader's desktop reaches the card. The village is laid out by the same
    /// shared functions the desktop lays it out with, on a notional one-to-one desktop.
    pub fn render(save: &SaveFile) -> Canvas {
        let mut canvas = Canvas::new(COLONY_CARD_WIDTH, COLONY_CARD_HEIGHT);
        let palette = colony_palette(save);
        let mut rng = CardRng::new(save.home.shelter.detail_seed);

        draw_background(&mut canvas, &mut rng, palette.accent);
        stepped_panel(&mut canvas, 24, 22, 912, 556, PAPER_SHADOW);
        stepped_panel(&mut canvas, 31, 29, 898, 542, PAPER);

        let mut text = CardText::new();
        text.draw_centered(
            &mut canvas,
            480,
            44,
            "A FORMIGA COLONY",
            26.0,
            palette.shadow,
        );
        text.draw_centered(
            &mut canvas,
            480,
            78,
            &format!("SINCE {}", since_label(save).to_uppercase()),
            14.0,
            MUTED_INK,
        );

        stepped_panel(&mut canvas, 57, 100, 846, 388, palette.outline);
        stepped_panel(&mut canvas, 64, 107, 832, 374, NIGHT_LIGHT);
        draw_scene(&mut canvas, &mut rng, palette);
        // The row is placed before anything is drawn, so the village can aim its colony house at
        // one of the spaces the colony leaves.
        let row = member_row(save);
        draw_village(&mut canvas, save, &row_gaps(&row));
        draw_floor(&mut canvas, palette);
        draw_members(&mut canvas, &mut text, &row);

        canvas.fill_rect(57, 502, 846, 2, PAPER_SHADOW);
        draw_stats(&mut canvas, &mut text, save, palette);
        canvas
    }
}

/// The colony's own colours: the home it shares, not any one creature's coat.
fn colony_palette(save: &SaveFile) -> Palette {
    let base = PALETTES[save.home.shelter.palette_index as usize % PALETTES.len()];
    let accent = PALETTES[save.home.shelter.accent_index as usize % PALETTES.len()];
    Palette {
        accent: accent.accent,
        highlight: accent.highlight,
        ..base
    }
}

/// The moonlit desktop the colony lives on: a sky, a far ground for the village, and a near floor
/// for the colony, each a step darker as it comes forward. The bands are what give the card its
/// depth — the village is genuinely behind, not merely smaller.
fn draw_scene(canvas: &mut Canvas, rng: &mut CardRng, palette: Palette) {
    let width = SCENE_RIGHT - SCENE_LEFT;
    for (top, bottom, color) in [
        (SCENE_TOP, HORIZON, Rgba::new(42, 87, 79, 255)),
        (HORIZON, VILLAGE_GROUND, Rgba::new(33, 71, 67, 255)),
        (VILLAGE_GROUND, COLONY_GROUND, Rgba::new(26, 59, 57, 255)),
        (COLONY_GROUND, SCENE_BOTTOM, Rgba::new(20, 46, 46, 255)),
    ] {
        canvas.fill_rect(SCENE_LEFT, top, width, bottom - top, color);
    }

    canvas.fill_circle(800, 172, 34, CREAM);
    canvas.fill_circle(790, 163, 28, Rgba::new(255, 226, 145, 255));
    // Stars over the sky, fireflies drifting low over the village's own ground.
    for _ in 0..120 {
        let x = SCENE_LEFT + 10 + rng.range(width - 20);
        let y = SCENE_TOP + 8 + rng.range(VILLAGE_GROUND - SCENE_TOP - 16);
        let size = if rng.range(5) == 0 { 3 } else { 2 };
        canvas.fill_rect(x, y, size, size, Rgba::new(174, 222, 176, 255));
    }
    // Tufts along the far ground line, so the village stands on something that runs the width of
    // the card instead of floating on a bare band.
    for _ in 0..52 {
        let x = SCENE_LEFT + 4 + rng.range(width - 8);
        let height = 4 + rng.range(7);
        let tint = Rgba::new(48, 101, 86, 255);
        canvas.fill_rect(x, VILLAGE_GROUND - height, 2, height, tint);
        canvas.fill_rect(x + 3, VILLAGE_GROUND - height / 2, 2, height / 2 + 1, tint);
    }
    // A scatter of little stones across the near ground, so the colony is standing on a floor
    // rather than in front of a flat panel of colour.
    for _ in 0..34 {
        let x = SCENE_LEFT + 6 + rng.range(width - 12);
        let y = VILLAGE_GROUND + 10 + rng.range(COLONY_GROUND - VILLAGE_GROUND - 18);
        canvas.fill_rect(x, y, 2 + rng.range(3), 2, Rgba::new(34, 75, 71, 255));
    }

    draw_vine(canvas, SCENE_LEFT + 11, SCENE_TOP + 4, true, palette.accent);
    draw_vine(
        canvas,
        SCENE_RIGHT - 11,
        SCENE_TOP + 3,
        false,
        palette.accent,
    );
}

/// A vine curling in from the frame, the same one the creature card hangs beside its portrait.
fn draw_vine(canvas: &mut Canvas, x: i32, y: i32, right: bool, color: Rgba) {
    let direction = if right { 1 } else { -1 };
    canvas.line(x, y, x + direction * 18, y + 90, 2, color);
    for offset in [18, 42, 67] {
        canvas.fill_ellipse(
            x + direction * offset / 5 + direction * 7,
            y + offset,
            8,
            4,
            color,
        );
    }
}

/// The lip of the near floor, and the paper plaque the names are written on. The creature card
/// finishes its portrait the same way — an accent rule, then a strip of the card's own paper — so
/// the bottom of the panel lifts instead of ending in a dark band.
fn draw_floor(canvas: &mut Canvas, palette: Palette) {
    let width = SCENE_RIGHT - SCENE_LEFT;
    canvas.fill_rect(SCENE_LEFT, COLONY_GROUND - 2, width, 2, palette.shadow);
    canvas.fill_rect(
        SCENE_LEFT + 24,
        COLONY_GROUND,
        width - 48,
        2,
        palette.accent,
    );
    canvas.fill_rect(
        SCENE_LEFT,
        NAME_PLAQUE_TOP,
        width,
        SCENE_BOTTOM - NAME_PLAQUE_TOP,
        Rgba::new(241, 220, 179, 255),
    );
    canvas.fill_rect(SCENE_LEFT, NAME_PLAQUE_TOP, width, 4, palette.accent);
}

/// What one piece of the village strip is drawn from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LotArt {
    /// A dwelling quadrant of the village atlas.
    Dwelling(DwellingKind),
    /// The village atlas's tree cell. The inward bookend samples it mirrored, exactly as the
    /// desktop does, so the card's two trees are not one drawing twice.
    Tree { mirrored: bool },
    /// One cell of the colony object sheet.
    Object(ColonyObjectKind),
}

/// One piece of the village strip, and where the desktop puts it.
struct Lot {
    art: LotArt,
    /// Centre and ground line, in the desktop's own coordinates.
    center_x: f32,
    ground_y: f32,
}

impl Lot {
    /// Side of the square art cell this piece is sampled from, in desktop units.
    fn cell(&self) -> f32 {
        match self.art {
            LotArt::Dwelling(_) | LotArt::Tree { .. } => SHELTER_SIZE as f32,
            LotArt::Object(_) => COLONY_OBJECT_SIZE as f32,
        }
    }

    /// Whether this piece is drawn in front of the houses rather than among them.
    fn in_front(&self) -> bool {
        matches!(self.art, LotArt::Object(_))
    }
}

/// The village exactly as the colony has built it: the colony house with the decorations it has
/// earned, a cottage per companion in colony order, the two trees bookending them, and the loose
/// belongings scattered in the yards at their feet. Every position comes from the shared
/// placement functions rather than from numbers written down here, so the strip on the card keeps
/// matching the strip on the desktop when cottages grow or the walk changes.
///
/// The layout is resolved on a notional desktop rather than the reader's own, for two reasons: a
/// card must carry no screen information, and a habitat too narrow for the whole strip would
/// otherwise drop cottages out of the colony's own portrait. Only the home corner, which is the
/// colony's own choice, decides which way the village runs.
fn village_lots(save: &SaveFile) -> Vec<Lot> {
    let cottages = formiga_core::colony_cottages(&save.creatures);
    let objects = save
        .objects
        .objects
        .len()
        .min(formiga_core::MAX_COLONY_OBJECTS);
    let monitors = [notional_monitor()];
    let policy = HabitatPolicy::default();
    let mut lots = Vec::with_capacity(3 + cottages.len() + objects);
    for slot in 0..=cottages.len() {
        let Some((_, point)) = formiga_core::home_dwelling_position(
            &save.home,
            slot,
            &cottages,
            &monitors,
            &policy,
            NOTIONAL_DISPLAY_SCALE,
        ) else {
            continue;
        };
        lots.push(Lot {
            art: LotArt::Dwelling(if slot == 0 {
                DwellingKind::Main
            } else {
                cottages[slot - 1]
            }),
            center_x: point.x,
            ground_y: point.y,
        });
    }
    for end in TreeEnd::BOTH {
        let Some((_, point)) = formiga_core::home_tree_position(
            &save.home,
            end,
            &cottages,
            &monitors,
            &policy,
            NOTIONAL_DISPLAY_SCALE,
        ) else {
            continue;
        };
        lots.push(Lot {
            art: LotArt::Tree {
                mirrored: end == TreeEnd::Inward,
            },
            center_x: point.x,
            ground_y: point.y,
        });
    }
    for slot in 0..objects {
        let Some((_, point)) = formiga_core::home_object_position(
            &save.home,
            slot,
            &cottages,
            &monitors,
            &policy,
            NOTIONAL_DISPLAY_SCALE,
        ) else {
            continue;
        };
        lots.push(Lot {
            art: LotArt::Object(save.objects.objects[slot].kind),
            center_x: point.x,
            ground_y: point.y,
        });
    }
    lots
}

/// One desktop unit per art pixel, so a house's spacing and a house's art agree exactly.
const NOTIONAL_DISPLAY_SCALE: u8 = 1;

/// A desktop wide enough that no village is ever clipped out of its own portrait. Pure geometry
/// at one-to-one scale: it borrows nothing from the reader's screen, not even a display key.
fn notional_monitor() -> MonitorInfo {
    let bounds = DesktopRect {
        x: 0.0,
        y: 0.0,
        width: 4096.0,
        height: 1024.0,
    };
    MonitorInfo {
        id: 1,
        display_key: DisplayKey([0; 16]),
        bounds,
        usable_bounds: bounds,
        scale_factor: 1.0,
        primary: true,
    }
}

fn draw_village(canvas: &mut Canvas, save: &SaveFile, gaps: &[i32]) {
    let lots = village_lots(save);
    if lots.is_empty() {
        return;
    }
    let decorations: Vec<_> = save
        .home
        .decorations
        .decorations
        .iter()
        .copied()
        .filter(|kind| save.home.hidden_decorations & (1 << kind.index()) == 0)
        .collect();
    let village = ShelterRenderer::render_village(&save.home.shelter, &decorations);
    let objects = ColonyObjectRenderer::render_atlas(save.colony_seed);

    let left = lots
        .iter()
        .map(|lot| lot.center_x - lot.cell() / 2.0)
        .fold(f32::MAX, f32::min);
    let right = lots
        .iter()
        .map(|lot| lot.center_x + lot.cell() / 2.0)
        .fold(f32::MIN, f32::max);
    let span = (right - left).max(1.0);
    let room = (SCENE_RIGHT - SCENE_LEFT - ROW_PADDING * 2) as f32;
    // Whole steps only: a half-scaled house would stop being pixel art. The height is measured
    // off the colony house as it is actually drawn rather than off its atlas cell, so a village
    // whose houses grow keeps its roofline where the card wants it.
    let house_height = cell_height(&village, 0, 0, SHELTER_SIZE as i32).unwrap_or(36);
    let scale = (1..=3)
        .rev()
        .find(|scale| {
            span * *scale as f32 <= room && house_height * scale <= VILLAGE_HEIGHT_CEILING
        })
        .unwrap_or(1);
    let drawn = (span * scale as f32).round() as i32;
    // Centred across the back of the scene. The corner the colony chose is already in the walk
    // itself — it decides which way the strip runs out from the house — so pinning the strip to
    // one side as well would only leave the other half of the sky empty.
    let centred = (SCENE_LEFT + SCENE_RIGHT) / 2 - drawn / 2;
    let origin_x = aim_house_at_a_gap(&lots, centred, left, scale, drawn, gaps);
    // Every lot shares the village's own ground line; only a belonging lifts off it.
    let base_ground = lots.iter().map(|lot| lot.ground_y).fold(f32::MIN, f32::max);
    // Houses and trees behind, belongings in front, exactly as the desktop layers them.
    let mut ordered: Vec<&Lot> = lots.iter().collect();
    ordered.sort_by_key(|lot| lot.in_front());
    for lot in ordered {
        let x = origin_x + ((lot.center_x - lot.cell() / 2.0 - left) * scale as f32).round() as i32;
        let y = VILLAGE_GROUND - (lot.cell() * scale as f32).round() as i32
            + ((lot.ground_y - base_ground) * scale as f32).round() as i32;
        match lot.art {
            // The same quadrants the desktop samples from the same atlas.
            LotArt::Dwelling(kind) => {
                let (u, v) = match kind {
                    DwellingKind::Main => (0, 0),
                    DwellingKind::Cottage => (SHELTER_SIZE as i32, 0),
                    DwellingKind::MiniCottage => (0, SHELTER_SIZE as i32),
                };
                blit_cell(
                    canvas,
                    &village,
                    u,
                    v,
                    SHELTER_SIZE as i32,
                    SHELTER_SIZE as i32,
                    x,
                    y,
                    scale,
                    false,
                );
            }
            LotArt::Tree { mirrored } => blit_cell(
                canvas,
                &village,
                SHELTER_SIZE as i32,
                SHELTER_SIZE as i32,
                SHELTER_SIZE as i32,
                SHELTER_SIZE as i32,
                x,
                y,
                scale,
                mirrored,
            ),
            LotArt::Object(kind) => blit_cell(
                canvas,
                &objects,
                i32::from(kind.index()) * COLONY_OBJECT_SIZE as i32,
                0,
                COLONY_OBJECT_SIZE as i32,
                COLONY_OBJECT_SIZE as i32,
                x,
                y,
                scale,
                false,
            ),
        }
    }
}

/// How tall one atlas cell's art actually is, as opposed to the cell it is drawn in.
fn cell_height(atlas: &Canvas, x: i32, y: i32, size: i32) -> Option<i32> {
    let mut bounds: Option<(i32, i32)> = None;
    for row in 0..size {
        for column in 0..size {
            if atlas.get(x + column, y + row).a == 0 {
                continue;
            }
            bounds =
                Some(bounds.map_or((row, row), |(top, bottom)| (top.min(row), bottom.max(row))));
        }
    }
    bounds.map(|(top, bottom)| bottom - top + 1)
}

/// Slide the whole village sideways, by as little as possible, until the colony house lands in
/// one of the spaces the row of members leaves. The strip keeps its own order and spacing; only
/// where it sits on the card changes, and never far enough to push an end off the scene.
fn aim_house_at_a_gap(
    lots: &[Lot],
    centred: i32,
    left: f32,
    scale: i32,
    drawn: i32,
    gaps: &[i32],
) -> i32 {
    let Some(house) = lots
        .iter()
        .find(|lot| lot.art == LotArt::Dwelling(DwellingKind::Main))
    else {
        return centred;
    };
    let house_x = centred + ((house.center_x - left) * scale as f32).round() as i32;
    let Some(nearest) = gaps
        .iter()
        .copied()
        .min_by_key(|gap| ((gap - house_x).abs(), *gap))
    else {
        return centred;
    };
    // Only a nudge: the strip stays a centred backdrop, it does not chase the gap across the card.
    let nudge = (nearest - house_x).clamp(-VILLAGE_NUDGE, VILLAGE_NUDGE);
    let limits = (
        SCENE_LEFT + 6,
        (SCENE_RIGHT - 6 - drawn).max(SCENE_LEFT + 6),
    );
    (centred + nudge).clamp(limits.0, limits.1)
}

/// Blit one atlas cell, scaled, with a touch of night mixed in so the village reads as standing
/// behind the colony rather than beside it.
#[allow(clippy::too_many_arguments)]
fn blit_cell(
    canvas: &mut Canvas,
    atlas: &Canvas,
    source_x: i32,
    source_y: i32,
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    scale: i32,
    mirrored: bool,
) {
    const HAZE: Rgba = Rgba::new(33, 71, 67, 255);
    for row in 0..height {
        for column in 0..width {
            let read = if mirrored { width - 1 - column } else { column };
            let pixel = atlas.get(source_x + read, source_y + row);
            if pixel.a == 0 {
                continue;
            }
            let hazed = blend(Rgba::new(pixel.r, pixel.g, pixel.b, 255), HAZE, 36);
            for dy in 0..scale {
                for dx in 0..scale {
                    let (px, py) = (x + column * scale + dx, y + row * scale + dy);
                    canvas.set(px, py, blend(canvas.get(px, py), hazed, pixel.a));
                }
            }
        }
    }
}

/// One colony member, measured before anything is placed.
struct Member<'a> {
    creature: &'a Creature,
    canvas: Canvas,
    /// The drawn pixels inside the 48x48 frame.
    left: i32,
    width: i32,
    height: i32,
}

/// One member, measured and placed: the row is worked out before anything is drawn, because the
/// village behind wants to know where the spaces between the colony are.
struct Placed<'a> {
    member: Member<'a>,
    /// Left edge of the drawn pixels on the card, and their width.
    x: i32,
    width: i32,
    scale: i32,
    /// The clear space that follows this member.
    gap: i32,
}

/// Everyone who lives here, standing in front of their village in a greeting pose, at the largest
/// whole scale the row can hold. The art already carries each creature's own size, so a mini
/// comes out visibly smaller without being singled out here.
fn member_row(save: &SaveFile) -> Vec<Placed<'_>> {
    let mut ordered: Vec<&Creature> = save.creatures.iter().collect();
    ordered.sort_by_key(|creature| (creature.colony_order, creature.id));
    if ordered.is_empty() {
        return Vec::new();
    }

    let count = ordered.len() as i32;
    let members: Vec<Member<'_>> = ordered
        .iter()
        .copied()
        .enumerate()
        .map(|(index, creature)| {
            // Everyone turns a little toward the middle, so the row reads as a group portrait.
            let facing_right = (index as i32) * 2 < count;
            let drawn = CreatureRenderer::render_composited_frame(
                &creature.appearance,
                ActionKind::Greet,
                greeting_frame(creature),
                facing_right,
                false,
                FaceRenderState {
                    expression: ExpressionKind::Affectionate,
                    eyelids: EyelidPose::Open,
                    gaze: GazeDirection::new(if facing_right { 1 } else { -1 }, 0),
                },
            );
            let (left, top, right, bottom) = drawn
                .alpha_bounds()
                .map_or((0, 0, FRAME_SIZE - 1, FRAME_SIZE - 1), |bounds| bounds);
            Member {
                creature,
                canvas: drawn,
                left: left as i32,
                width: (right - left + 1) as i32,
                height: (bottom - top + 1) as i32,
            }
        })
        .collect();

    let room = SCENE_RIGHT - SCENE_LEFT - ROW_PADDING * 2;
    let art_width: i32 = members.iter().map(|member| member.width).sum();
    let tallest = members
        .iter()
        .map(|member| member.height)
        .max()
        .unwrap_or(1);
    // Room is counted against the gaps a row needs on both sides as well as between neighbours,
    // so the largest scale that fits is one the row can actually breathe at.
    let scale = (2..=7)
        .rev()
        .find(|scale| {
            art_width * scale + ROW_GAP * (count + 1) <= room && tallest * scale <= MEMBER_CEILING
        })
        .unwrap_or(2);

    // Whatever room is left over is shared out evenly, before the first member, between each
    // pair, and after the last. Packing by silhouette alone leaves a narrow companion looking
    // stranded beside a wide one; sharing the slack keeps the rhythm even whoever is in the row.
    let slack = (room - art_width * scale).max(0);
    let gap = slack / (count + 1);
    let mut x = SCENE_LEFT + ROW_PADDING + gap + (slack - gap * (count + 1)) / 2;
    members
        .into_iter()
        .map(|member| {
            let width = member.width * scale;
            let placed = Placed {
                member,
                x,
                width,
                scale,
                gap,
            };
            x += width + gap;
            placed
        })
        .collect()
}

fn draw_members(canvas: &mut Canvas, text: &mut CardText, row: &[Placed<'_>]) {
    if row.is_empty() {
        text.draw_centered(
            canvas,
            480,
            COLONY_GROUND - 60,
            "The colony is out exploring",
            22.0,
            CREAM,
        );
        return;
    }
    for placed in row {
        // The 48x48 frame is placed so the creature's own resting feet meet the floor, the same
        // seating the desktop gives it.
        let baseline =
            CreatureRenderer::resting_baseline(&placed.member.creature.appearance, false) as i32;
        let frame_top = COLONY_GROUND + (baseline - FRAME_SIZE as i32) * placed.scale;
        contact_shadow(
            canvas,
            placed.x + placed.width / 2,
            COLONY_GROUND + 1,
            (placed.width / 2).max(6),
            5,
        );
        blit_scaled(
            canvas,
            &placed.member.canvas,
            placed.x - placed.member.left * placed.scale,
            frame_top,
            placed.scale,
        );
        draw_name(
            canvas,
            text,
            placed.member.creature,
            placed.x + placed.width / 2,
            placed.width + placed.gap,
        );
    }
}

/// The middle of each clear space in the row: before the first member, between each pair, and
/// after the last. The village behind aims its colony house at one of these, so the home the card
/// is about is framed by the colony rather than hidden behind it.
fn row_gaps(row: &[Placed<'_>]) -> Vec<i32> {
    if row.is_empty() {
        return vec![(SCENE_LEFT + SCENE_RIGHT) / 2];
    }
    let mut centres = vec![(SCENE_LEFT + row[0].x) / 2];
    for pair in row.windows(2) {
        centres.push((pair[0].x + pair[0].width + pair[1].x) / 2);
    }
    let last = &row[row.len() - 1];
    centres.push((last.x + last.width + SCENE_RIGHT) / 2);
    centres
}

/// A soft contact shadow under a member, mixed into the ledge rather than laid over it, so the
/// card stays fully opaque.
fn contact_shadow(canvas: &mut Canvas, center_x: i32, center_y: i32, radius_x: i32, radius_y: i32) {
    const SHADOW: Rgba = Rgba::new(16, 38, 38, 255);
    let (rx2, ry2) = ((radius_x * radius_x) as i64, (radius_y * radius_y) as i64);
    for y in -radius_y..=radius_y {
        for x in -radius_x..=radius_x {
            if x as i64 * x as i64 * ry2 + y as i64 * y as i64 * rx2 > rx2 * ry2 {
                continue;
            }
            let (px, py) = (center_x + x, center_y + y);
            canvas.set(px, py, blend(canvas.get(px, py), SHADOW, 120));
        }
    }
}

/// Which frame of the greeting each member is caught on. Stable per creature, and different
/// enough between neighbours that a row never looks stamped out.
fn greeting_frame(creature: &Creature) -> u8 {
    let frames = crate::AnimationSpec::for_action(ActionKind::Greet)
        .frames
        .max(1);
    (creature.colony_order.wrapping_add(creature.generation)) % frames
}

/// A name under each member, shrunk and then shortened until it fits the space that member
/// actually occupies, so a long or wide name never runs into a neighbour.
fn draw_name(
    canvas: &mut Canvas,
    text: &mut CardText,
    creature: &Creature,
    center_x: i32,
    width: i32,
) {
    let room = (width - 8).max(56) as f32;
    let size = text.fit_size(&creature.name, room, 21.0, 12.0);
    let label = text.truncate_to_width(&creature.name, size, room);
    text.draw_centered(canvas, center_x, NAME_PLAQUE_TOP + 9, &label, size, INK);
}

/// Two small facts, and only from the well a creature card already draws on: how many live here,
/// how many family lines they make, and the month somebody arrived.
fn draw_stats(canvas: &mut Canvas, text: &mut CardText, save: &SaveFile, palette: Palette) {
    let members = save.creatures.len();
    let families = family_count(&save.creatures);
    let newest = newest_arrival(save);
    let chips = [
        match members {
            0 => "An empty colony".to_owned(),
            1 => "One companion, one home".to_owned(),
            other => format!("{} companions sharing one home", spelled(other)),
        },
        // Family lines are only worth saying when they say something a headcount does not.
        if families > 1 && families < members {
            format!("{} family lines under one roof", spelled(families))
        } else if newest
            .as_deref()
            .is_some_and(|month| month != since_label(save))
        {
            format!("Newest arrived in {}", newest.unwrap_or_default())
        } else {
            "Here since the very first day".to_owned()
        },
    ];
    let widths: Vec<i32> = chips
        .iter()
        .map(|label| (text.measure(label, 17.0).ceil() as i32 + 26).clamp(80, 394))
        .collect();
    let total: i32 = widths.iter().sum::<i32>() + 14;
    let mut x = 480 - total / 2;
    for (label, width) in chips.iter().zip(&widths) {
        draw_chip(canvas, text, x, 512, label, palette.highlight);
        x += width + 14;
    }
    // The colony's own mark, bookending the row the way the creature card signs its corner.
    if let Some(motif) = save
        .creatures
        .first()
        .map(|creature| creature.appearance.effect_motif)
    {
        for x in [x + 22, 480 - total / 2 - 22] {
            draw_motif(canvas, x, 528, motif, 1, palette.accent);
        }
    }
    text.draw_centered(
        canvas,
        480,
        552,
        "A QUIET DESKTOP COLONY, ALL TOGETHER",
        13.0,
        MUTED_INK,
    );
}

/// Distinct family lines: an adult and the minis that came from it count as one.
fn family_count(creatures: &[Creature]) -> usize {
    let mut roots: Vec<u64> = creatures
        .iter()
        .map(|creature| match creature.role {
            CreatureRole::Mini { parent_id }
                if creatures.iter().any(|other| other.id == parent_id) =>
            {
                parent_id
            }
            _ => creature.id,
        })
        .collect();
    roots.sort_unstable();
    roots.dedup();
    roots.len()
}

/// The month the colony began, or — for a save that predates the field — the month its earliest
/// member arrived. Month and year only, exactly what a creature card is allowed to show.
fn since_label(save: &SaveFile) -> String {
    let started = save
        .creatures
        .iter()
        .map(|creature| creature.born_at_utc)
        .min()
        .map_or(save.created_at_utc, |earliest| {
            earliest.min(save.created_at_utc)
        });
    format!("{} {}", month_label(started.month()), started.year())
}

/// The month the colony's newest member arrived. Month and year only, the same grain as the
/// arrival line on a creature card.
fn newest_arrival(save: &SaveFile) -> Option<String> {
    save.creatures
        .iter()
        .map(|creature| creature.born_at_utc)
        .max()
        .map(|latest| format!("{} {}", month_label(latest.month()), latest.year()))
}

fn spelled(value: usize) -> String {
    const WORDS: [&str; 9] = [
        "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine",
    ];
    WORDS
        .get(value.saturating_sub(1))
        .map_or_else(|| value.to_string(), |word| (*word).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::{
        ColonyObject, CreatureRelationship, DesktopSnapshot, HabitatPreset, HabitatZone,
        HabitatZoneKind, ShelterDecorationKind, World, encode_creature_seed,
    };
    use time::macros::datetime;

    fn monitors() -> Vec<MonitorInfo> {
        vec![MonitorInfo {
            id: 1,
            display_key: DisplayKey([1; 16]),
            bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 1440.0,
                height: 900.0,
            },
            usable_bounds: DesktopRect {
                x: 0.0,
                y: 24.0,
                width: 1440.0,
                height: 826.0,
            },
            scale_factor: 2.0,
            primary: true,
        }]
    }

    fn colony(members: usize) -> SaveFile {
        let monitors = monitors();
        let desktop = DesktopSnapshot {
            monitors: monitors.clone(),
            ..DesktopSnapshot::default()
        };
        let created = datetime!(2026-03-08 09:15 UTC);
        let mut world = World::new([25; 32], created, &desktop);
        world.tick(created + time::Duration::days(60), 0.05, &desktop);
        world.save.creatures.truncate(members.max(1));
        world.save.home.decorations.decorations = ShelterDecorationKind::ALL.to_vec();
        world.save.objects.objects = ColonyObjectKind::ALL
            .iter()
            .enumerate()
            .map(|(index, kind)| ColonyObject {
                id: index as u64,
                kind: *kind,
                role: kind.default_role(),
                ..Default::default()
            })
            .collect();
        world.save
    }

    #[test]
    fn portrait_is_fixed_size_opaque_and_pixel_deterministic() {
        let save = colony(4);
        let first = ColonyCardRenderer::render(&save);
        let second = ColonyCardRenderer::render(&save);
        assert_eq!(
            (first.width(), first.height()),
            (COLONY_CARD_WIDTH, COLONY_CARD_HEIGHT)
        );
        assert_eq!(first, second);
        assert!(first.pixels().iter().all(|pixel| pixel.a == 255));
    }

    /// Where the colony actually ended up on the card, read back from the pixels rather than from
    /// the layout: the band between the village and the floor holds only ground, its scattered
    /// stones, and whoever is standing there.
    fn drawn_row(card: &Canvas) -> (i32, i32) {
        let ground = [Rgba::new(26, 59, 57, 255), Rgba::new(34, 75, 71, 255)];
        let mut bounds = (SCENE_RIGHT, SCENE_LEFT);
        for y in VILLAGE_GROUND + 20..COLONY_GROUND - 8 {
            for x in SCENE_LEFT..SCENE_RIGHT {
                if !ground.contains(&card.get(x, y)) {
                    bounds = (bounds.0.min(x), bounds.1.max(x));
                }
            }
        }
        bounds
    }

    #[test]
    fn every_colony_size_composes_centred_without_leaving_a_hole() {
        for members in 1..=4 {
            let save = colony(members);
            let row = member_row(&save);
            assert_eq!(row.len(), members);
            // Nobody is laid on top of a neighbour, and the spaces between are all the same.
            for pair in row.windows(2) {
                assert_eq!(pair[1].x - (pair[0].x + pair[0].width), pair[0].gap);
                assert!(pair[0].gap > 0);
            }
            let left = row[0].x;
            let right = row[members - 1].x + row[members - 1].width;
            assert!(
                right - left > 140,
                "{members} members were drawn too small to read"
            );
            // A row hanging off one side is the failure that reads as a hole on the other, and a
            // colony of one has to end up in the middle rather than at one end.
            let lean = (left - SCENE_LEFT) - (SCENE_RIGHT - right);
            assert!(
                lean.abs() <= 4,
                "{members} members sat off centre by {lean}px"
            );

            // And the drawing really did follow the layout.
            let card = ColonyCardRenderer::render(&save);
            let (drawn_left, drawn_right) = drawn_row(&card);
            // The band skips the lowest rows, where a wide foot can sit, so a few pixels of slack.
            assert!(
                (drawn_left - left).abs() <= 12 && (drawn_right - right).abs() <= 12,
                "{members} members were drawn at {drawn_left}..{drawn_right}, laid out at {left}..{right}"
            );
        }
    }

    #[test]
    fn the_whole_village_reaches_the_card_whatever_the_habitat_allows() {
        for members in 1..=4 {
            let mut save = colony(members);
            // A habitat with no room at all, which on the desktop would hide most of the strip.
            save.settings.habitat.preset = HabitatPreset::PrimaryDisplay;
            save.settings.habitat.zones = vec![HabitatZone {
                id: 1,
                display: DisplayKey([1; 16]),
                kind: HabitatZoneKind::Allowed,
                normalized_bounds: DesktopRect {
                    x: 0.4,
                    y: 0.4,
                    width: 0.06,
                    height: 0.06,
                },
                enabled: true,
            }];
            let expected = 1
                + formiga_core::colony_cottages(&save.creatures).len()
                + formiga_core::TreeEnd::BOTH.len()
                + save
                    .objects
                    .objects
                    .len()
                    .min(formiga_core::MAX_COLONY_OBJECTS);
            assert_eq!(village_lots(&save).len(), expected);
        }
    }

    #[test]
    fn the_village_comes_from_the_shared_layout_rather_than_numbers_written_here() {
        let save = colony(4);
        let lots = village_lots(&save);
        let cottages = formiga_core::colony_cottages(&save.creatures);
        let expected_monitors = [notional_monitor()];
        for (slot, lot) in lots.iter().take(1 + cottages.len()).enumerate() {
            let (_, point) = formiga_core::home_dwelling_position(
                &save.home,
                slot,
                &cottages,
                &expected_monitors,
                &HabitatPolicy::default(),
                NOTIONAL_DISPLAY_SCALE,
            )
            .unwrap();
            assert_eq!((lot.center_x, lot.ground_y), (point.x, point.y));
        }
        // Spacing is the shared walk's, so a wider cottage really does move its neighbour.
        let gap = |cottages: &[DwellingKind]| {
            let at = |slot| {
                formiga_core::home_dwelling_position(
                    &save.home,
                    slot,
                    cottages,
                    &expected_monitors,
                    &HabitatPolicy::default(),
                    NOTIONAL_DISPLAY_SCALE,
                )
                .unwrap()
                .1
                .x
            };
            (at(1) - at(0)).abs()
        };
        assert!(gap(&[DwellingKind::Cottage]) > gap(&[DwellingKind::MiniCottage]));
    }

    #[test]
    fn long_and_unicode_names_stay_inside_the_scene() {
        let mut save = colony(4);
        for (index, creature) in save.creatures.iter_mut().enumerate() {
            creature.name = match index {
                0 => "WWWWWWWWWWWWWWWWWWWWWWWW".into(),
                1 => "Mochi 雪 ✨ très doux".into(),
                2 => "Пушистик-Облачко".into(),
                _ => "🌙".into(),
            };
        }
        let card = ColonyCardRenderer::render(&save);
        assert!(card.pixels().iter().all(|pixel| pixel.a == 255));
        // Name rows stay on the scene's dark ground, never spilling onto the paper frame.
        for y in COLONY_GROUND + 20..SCENE_BOTTOM {
            for x in [SCENE_LEFT - 3, SCENE_RIGHT + 2] {
                assert_eq!(
                    card.get(x, y),
                    card.get(x, SCENE_TOP + 4),
                    "a name reached the frame at ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn nothing_private_can_change_a_single_pixel_of_the_portrait() {
        let save = colony(4);
        let before = ColonyCardRenderer::render(&save);
        let mut loud = save.clone();
        // Everything the privacy contract keeps off a card, turned up as far as it goes.
        for creature in &mut loud.creatures {
            creature.memory.times_petted = 9_999;
            creature.memory.window_climbs = 9_999;
            creature.memory.discoveries_found = 99;
            creature.memory.descriptor_flags = u16::MAX;
            creature.tendencies.climbing = 127;
            creature.tendencies.sociability = 127;
        }
        loud.relationships = save
            .creatures
            .windows(2)
            .map(|pair| CreatureRelationship {
                a: pair[0].id,
                b: pair[1].id,
                affinity: 255,
                avoidance: 255,
                playfulness: 255,
                ..Default::default()
            })
            .collect();
        loud.companion.scrapbook.clear();
        loud.visitors = Default::default();
        assert_eq!(ColonyCardRenderer::render(&loud), before);
        // The stand-in desktop the fallback uses carries a placeholder key, not the colony's.
        assert_eq!(notional_monitor().display_key, DisplayKey([0; 16]));
        // The full share code exists, and no part of the card is built from it.
        let code = encode_creature_seed(save.creatures[0].origin);
        assert!(!code.is_empty());
        assert!(!since_label(&save).contains(&code));
    }

    #[test]
    fn family_lines_fold_minis_into_the_adult_they_came_from() {
        let mut save = colony(3);
        let parent = save.creatures[0].id;
        save.creatures[1].role = CreatureRole::Mini { parent_id: parent };
        save.creatures[2].role = CreatureRole::Mini {
            parent_id: 0xdead_beef,
        };
        assert_eq!(family_count(&save.creatures), 2);
    }

    #[test]
    fn renderer_holds_no_persistent_export_state() {
        assert_eq!(std::mem::size_of::<ColonyCardRenderer>(), 0);
    }
}
