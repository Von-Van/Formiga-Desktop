//! Belongings a body holds: which toy, snack and cup it owns, where it holds them, and how each is
//! drawn.
use super::*;

/// How many kinds of each belonging the generator can produce. Every one is resolved from genes
/// the appearance genome already stores, so nothing is added to a save: an existing companion
/// simply reaches for a clearer toy from a larger shelf.
pub const TOY_KINDS: u8 = 8;
pub const SNACK_KINDS: u8 = 4;
pub const DRINK_KINDS: u8 = 3;

/// The one number every belonging is resolved from.
pub(super) fn prop_signature(genome: &AppearanceGenome) -> u64 {
    genome.marking_seed ^ u64::from(genome.face_signature)
}

/// Which toy, snack, and cup one appearance owns. Stable for the life of a creature.
pub fn prop_variants(genome: &AppearanceGenome) -> (u8, u8, u8) {
    let signature = prop_signature(genome);
    (
        (signature % u64::from(TOY_KINDS)) as u8,
        ((signature >> 7) % u64::from(SNACK_KINDS)) as u8,
        ((signature >> 13) % u64::from(DRINK_KINDS)) as u8,
    )
}

/// Where a body holds what it is using. Props are placed from these two points rather than from an
/// offset guessed against the face, so a toy lands in the paw that is actually drawn and a snack
/// arrives at the mouth that is eating it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PropHold {
    pub(crate) hands: PixelPoint,
    pub(crate) mouth: PixelPoint,
    pub(crate) floor: i32,
    /// The lowest row this body's feet reach. Nothing it drops lands below it.
    pub(crate) ground: i32,
    /// Which way is away from this body's own face: `1` for a body whose paws are in front of its
    /// head, `-1` for one whose head leads and whose chest trails behind it.
    pub(crate) forward: i32,
}

pub(super) fn prop_hold(genome: &AppearanceGenome, pose: Pose, face: PixelPoint) -> PropHold {
    if let Some(design) = genome.design {
        return modular::prop_hold(design, pose, scale(genome));
    }
    // The original families carry their face on the body itself, so a belonging is placed out to
    // the side of the mass rather than in front of a separate head.
    let (dx, dy) = match genome.family {
        BodyFamily::Blob => (12, 4),
        BodyFamily::Hopper => (12, 5),
        // A soft quadruped's head is the anchor and its chest is behind it.
        BodyFamily::SoftQuadruped => (-12, 7),
    };
    // A blob's mass is pinned to row 38 and its feet hang two rows below it; the legged families
    // stand on a ground of their own at row 45.
    let ground = match genome.family {
        BodyFamily::Blob => 40,
        BodyFamily::Hopper | BodyFamily::SoftQuadruped => 45,
    };
    PropHold {
        hands: PixelPoint {
            x: face.x + dx,
            y: face.y + dy,
        },
        mouth: PixelPoint {
            x: face.x + 6,
            y: face.y + 5,
        },
        floor: 42,
        ground,
        forward: dx.signum(),
    }
}

/// One round of play, one mouthful, or one sip — placed where the body can reach it.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_activity_prop(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    face: PixelPoint,
    pose: Pose,
    action: ActionKind,
    frame: u8,
    reduce_motion: bool,
) {
    let phase = if reduce_motion { 0 } else { frame % 4 };
    let (toy, snack, drink) = prop_variants(genome);
    // Belongings are coloured against the creature, not from it, so a held toy stays readable.
    let palette = crate::prop_palette(palette, prop_signature(genome));
    let hold = prop_hold(genome, pose, face);
    match action {
        ActionKind::SoloPlay => {
            // One round of keep-up, read straight off the paw: knocked up off the paw, over the
            // head, caught coming down, and back onto the paw. The toy is the thing that moves,
            // and it passes through the paw twice a loop, so play reads as play even on a body
            // whose forelimbs stay folded.
            let at = |phase: u8| play_path(hold, phase);
            let (x, y, spin) = at(phase);
            if !reduce_motion {
                let (px, py, _) = at((phase + 3) % 4);
                draw_prop_trail(canvas, palette, (px, py), (x, y));
            }
            draw_generated_toy(canvas, palette, genome.effect_motif, toy, x, y, spin);
            if phase == 0 || phase == 3 {
                // The knock itself: two ticks where the toy meets the paw that is keeping it up.
                for (dx, dy) in [(-1, 3), (1, 4)] {
                    canvas.set(x + dx * hold.forward, y + dy, palette.highlight);
                }
            }
        }
        ActionKind::Eat => {
            // Carried up to the mouth and taken down to a crumb.
            let lift = [0, 2, 3, 1][usize::from(phase)];
            let x = hold.mouth.x - 1 + i32::from(phase == 0) * 2;
            let y = hold.mouth.y + 2 - lift;
            draw_generated_snack(canvas, palette, snack, x, y, phase);
            if phase == 3 {
                // Crumbs, so the last frame is the end of a mouthful rather than an empty hand.
                // They fall no further than the ground.
                canvas.set(
                    hold.mouth.x - 2,
                    (hold.mouth.y + 5).min(hold.ground),
                    palette.shadow,
                );
                canvas.set(
                    hold.mouth.x + 1,
                    (hold.mouth.y + 6).min(hold.ground),
                    palette.highlight,
                );
            }
        }
        ActionKind::Drink => {
            let tipped = phase == 1 || phase == 2;
            let x = hold.mouth.x - 1;
            let y = hold.mouth.y + 4 - i32::from(tipped) * 3;
            draw_generated_drinkware(canvas, palette, drink, x, y, tipped, phase);
        }
        _ => {}
    }
}

/// Where the toy is on each beat of the play loop, and how far it has turned by then.
pub(super) fn play_path(hold: PropHold, phase: u8) -> (i32, i32, u8) {
    let (hx, hy) = (hold.hands.x, hold.hands.y);
    let (dx, dy, spin) = match phase {
        // Resting on the paw.
        0 => (0, -1, 0),
        // Knocked up and away from the body.
        1 => (3, -5, 1),
        // The top of its arc: clear of the creature, never over its face, and never so high that
        // the paw it came off has visibly let go of it.
        2 => (5, -8, 2),
        // Coming back down onto the paw.
        _ => (1, -4, 3),
    };
    let dx = dx * hold.forward;
    // Nothing sinks into the surface underfoot, and the widest toy still keeps the one-pixel
    // margin the atlas needs without the whole sprite being shifted to make room for it.
    (
        (hx + dx).clamp(8, FRAME_SIZE as i32 - 9),
        (hy + dy)
            .min(hold.floor - 4)
            .clamp(8, FRAME_SIZE as i32 - 9),
        spin,
    )
}

/// Two short dashes along the way the toy has just come. Nothing here is animated on its own: the
/// marks are simply drawn between this beat's position and the last one's.
pub(super) fn draw_prop_trail(
    canvas: &mut Canvas,
    palette: Palette,
    from: (i32, i32),
    to: (i32, i32),
) {
    for step in [2, 4, 6] {
        let x = from.0 + (to.0 - from.0) * step / 8;
        let y = from.1 + (to.1 - from.1) * step / 8;
        canvas.set(x, y, palette.outline);
        canvas.set(x + 1, y, palette.highlight);
    }
}

/// A shape with the shared dark edge every belonging carries, so nothing melts into a coat.
pub(super) fn prop_blob(
    canvas: &mut Canvas,
    palette: Palette,
    x: i32,
    y: i32,
    rx: i32,
    ry: i32,
    fill: Rgba,
) {
    canvas.fill_ellipse(x, y, rx + 1, ry + 1, palette.outline);
    canvas.fill_ellipse(x, y, rx, ry, fill);
}

pub(super) fn prop_box(
    canvas: &mut Canvas,
    palette: Palette,
    x: i32,
    y: i32,
    rx: i32,
    ry: i32,
    fill: Rgba,
) {
    canvas.fill_rect(
        x - rx - 1,
        y - ry - 1,
        rx * 2 + 3,
        ry * 2 + 3,
        palette.outline,
    );
    canvas.fill_rect(x - rx, y - ry, rx * 2 + 1, ry * 2 + 1, fill);
}

/// Eight playthings, each with a silhouette that survives being shrunk to 2x on a busy desktop:
/// a ball, a spinning top, a plush, a block, a yo-yo, a rattle, a pinwheel, and a hoop.
pub(super) fn draw_generated_toy(
    canvas: &mut Canvas,
    palette: Palette,
    motif: EffectMotif,
    variant: u8,
    x: i32,
    y: i32,
    spin: u8,
) {
    let tilt = [0, 1, 0, -1][usize::from(spin % 4)];
    match variant % TOY_KINDS {
        // Ball, with a seam that turns as it flies.
        0 => {
            prop_blob(canvas, palette, x, y, 4, 4, palette.accent);
            match spin % 4 {
                0 => canvas.line(x, y - 3, x, y + 3, 1, palette.shadow),
                1 => canvas.line(x - 3, y - 2, x + 3, y + 2, 1, palette.shadow),
                2 => canvas.line(x - 3, y, x + 3, y, 1, palette.shadow),
                _ => canvas.line(x - 3, y + 2, x + 3, y - 2, 1, palette.shadow),
            }
            canvas.set(x - 2, y - 2, palette.highlight);
            canvas.set(x - 1, y - 3, palette.highlight);
        }
        // Spinning top: a broad disc on a point, leaning further over the faster it goes.
        1 => {
            canvas.fill_ellipse(x + tilt, y - 1, 6, 4, palette.outline);
            canvas.fill_ellipse(x + tilt, y - 1, 5, 3, palette.accent);
            canvas.line(x + tilt, y + 1, x + tilt * 2, y + 5, 3, palette.outline);
            canvas.line(x + tilt, y + 1, x + tilt * 2, y + 5, 1, palette.highlight);
            canvas.fill_rect(x + tilt - 2, y - 6, 5, 3, palette.outline);
            canvas.fill_rect(x + tilt - 1, y - 5, 3, 2, palette.highlight);
            canvas.line(x + tilt - 4, y - 1, x + tilt + 4, y - 2, 1, palette.shadow);
        }
        // Plush, with its own two ears and two stitched eyes.
        2 => {
            for side in [-1, 1] {
                prop_blob(canvas, palette, x + side * 3, y - 4, 2, 2, palette.accent);
            }
            prop_blob(canvas, palette, x, y + 1, 4, 4, palette.accent);
            canvas.set(x - 2, y - 1, palette.outline);
            canvas.set(x + 2, y - 1, palette.outline);
            canvas.line(x - 1, y + 2, x + 1, y + 2, 1, palette.shadow);
            canvas.set(x - 3, y - 1, palette.highlight);
        }
        // Building block, with a mark cut into its face.
        3 => {
            prop_box(canvas, palette, x, y, 4, 4, palette.accent);
            canvas.fill_rect(x - 4, y - 4, 9, 1, palette.highlight);
            canvas.fill_rect(x - 4, y - 4, 1, 9, palette.highlight);
            canvas.fill_rect(x - 2, y - 2, 5, 5, palette.shadow);
            canvas.fill_rect(x - 1, y - 1, 3, 3, palette.accent);
        }
        // Yo-yo, on a string from the paw above it.
        4 => {
            canvas.line(x, y - 10, x, y - 4, 1, palette.outline);
            prop_blob(canvas, palette, x, y, 4, 4, palette.accent);
            canvas.fill_rect(x - 4, y - 1, 9, 2, palette.shadow);
            canvas.set(x - 2, y - 2, palette.highlight);
        }
        // Rattle: a bulb on a handle, shaking the way it is carried.
        5 => {
            canvas.line(x - tilt, y + 1, x - tilt * 2, y + 7, 3, palette.outline);
            canvas.line(x - tilt, y + 1, x - tilt * 2, y + 7, 1, palette.highlight);
            prop_blob(canvas, palette, x, y - 3, 5, 5, palette.accent);
            canvas.fill_rect(x - 2, y - 4, 4, 2, palette.shadow);
            canvas.set(x - 3, y - 5, palette.highlight);
            canvas.set(x - 2, y - 6, palette.highlight);
        }
        // Pinwheel: four sails on a stick, turning with the beat.
        6 => {
            canvas.line(x, y, x, y + 7, 3, palette.outline);
            canvas.line(x, y, x, y + 7, 1, palette.highlight);
            let mut sail = [
                (0, -1),
                (0, -2),
                (0, -3),
                (0, -4),
                (1, -2),
                (1, -3),
                (1, -4),
                (2, -3),
                (2, -4),
                (3, -4),
            ];
            for turn in 0..4 {
                let fill = if (turn + spin).is_multiple_of(2) {
                    palette.accent
                } else {
                    palette.shadow
                };
                for (dx, dy) in sail {
                    canvas.fill_circle(x + dx, y + dy, 1, palette.outline);
                }
                for (dx, dy) in sail {
                    canvas.set(x + dx, y + dy, fill);
                }
                sail = sail.map(|(dx, dy)| (-dy, dx));
            }
            canvas.fill_circle(x, y, 1, palette.outline);
            canvas.set(x, y, palette.highlight);
        }
        // Hoop, wide enough to be a hoop rather than a bead.
        7 => {
            canvas.fill_ellipse(x, y, 5, 5, palette.outline);
            canvas.fill_ellipse(x, y, 4, 4, palette.accent);
            canvas.fill_ellipse(x, y, 2, 2, palette.shadow);
            canvas.set(x - 3, y - 2, palette.highlight);
            canvas.set(x - 2, y - 3, palette.highlight);
        }
        // Unreachable while TOY_KINDS is 8; kept so a larger shelf never draws nothing.
        _ => draw_motif(
            canvas,
            if motif == EffectMotif::None {
                EffectMotif::Star
            } else {
                motif
            },
            x,
            y,
            palette.accent,
        ),
    }
}

/// Four things worth eating, each one visibly smaller by the mouthful.
pub(super) fn draw_generated_snack(
    canvas: &mut Canvas,
    palette: Palette,
    variant: u8,
    x: i32,
    y: i32,
    phase: u8,
) {
    // Whole, bitten, half gone, and down to the last of it.
    let left = [3, 3, 2, 1][usize::from(phase % 4)];
    match variant % SNACK_KINDS {
        // A berry on its stem.
        0 => {
            canvas.line(x, y - left - 1, x + 2, y - left - 3, 1, palette.shadow);
            prop_blob(canvas, palette, x, y, left, left, palette.accent);
            canvas.set(x - 1, y - 1, palette.highlight);
        }
        // A biscuit with seeds in it.
        1 => {
            prop_box(canvas, palette, x, y, left, left - 1, palette.accent);
            for (dx, dy) in [(-1, -1), (1, 0), (0, 1)] {
                canvas.set(x + dx, y + dy, palette.shadow);
            }
            canvas.fill_rect(x - left, y - left + 1, left, 1, palette.highlight);
        }
        // A seed, pointed at one end.
        2 => {
            prop_blob(canvas, palette, x, y, left - 1, left, palette.accent);
            canvas.line(x, y - left, x + 1, y - left - 2, 1, palette.outline);
            canvas.set(x - 1, y, palette.highlight);
        }
        // A slice, rind side down.
        _ => {
            canvas.fill_ellipse(x, y, left + 1, left, palette.outline);
            canvas.fill_ellipse(x, y, left, left - 1, palette.accent);
            canvas.fill_rect(x - left, y + left - 1, left * 2 + 1, 1, palette.shadow);
            canvas.set(x - 1, y - 1, palette.highlight);
        }
    }
    if phase > 0 {
        // The bite taken out of it, on the side the mouth is.
        canvas.fill_circle(x + left, y - 1, 1, Rgba::TRANSPARENT);
    }
}

/// Three things to drink from, each tipped toward the mouth on the swallow.
pub(super) fn draw_generated_drinkware(
    canvas: &mut Canvas,
    palette: Palette,
    variant: u8,
    x: i32,
    y: i32,
    tipped: bool,
    phase: u8,
) {
    let lean = i32::from(tipped);
    let x = x + lean;
    match variant % DRINK_KINDS {
        // A tall mug with a C handle on the side, steaming while it rests.
        0 => {
            prop_box(canvas, palette, x - 1, y, 3, 4, palette.highlight);
            canvas.fill_rect(x - 4, y - 4 + lean * 2, 7, 2, palette.accent);
            canvas.fill_ellipse(x + 4, y + 1, 3, 3, palette.outline);
            canvas.fill_ellipse(x + 4, y + 1, 2, 2, Rgba::TRANSPARENT);
            canvas.fill_rect(x + 1, y - 1, 3, 5, palette.highlight);
            if !tipped {
                for (dx, dy) in [(-1, -7), (0, -9), (1, -11)] {
                    canvas.set(x + dx, y + dy, palette.highlight);
                }
            }
        }
        // A wide shallow bowl, with the surface tilting as it is lifted.
        1 => {
            canvas.fill_ellipse(x, y, 6, 4, palette.outline);
            canvas.fill_rect(x - 6, y - 4, 13, 4, Rgba::TRANSPARENT);
            canvas.fill_ellipse(x, y, 5, 3, palette.highlight);
            canvas.fill_rect(x - 5, y - 3, 11, 3, Rgba::TRANSPARENT);
            canvas.fill_rect(x - 5, y - 1 - lean, 11, 2, palette.accent);
            canvas.fill_rect(x - 6, y - 2, 13, 1, palette.outline);
            canvas.fill_rect(x - 5, y - 2, 4, 1, palette.highlight);
        }
        // A narrow bottle with a cap and a bent straw standing out of it.
        _ => {
            prop_box(canvas, palette, x, y + 1, 2, 3, palette.highlight);
            canvas.fill_rect(x - 2, y + 1, 5, 4, palette.accent);
            canvas.fill_rect(x - 2, y - 4, 5, 2, palette.outline);
            canvas.fill_rect(x - 1, y - 3, 3, 1, palette.highlight);
            canvas.fill_rect(x - 1, y - 6, 3, 2, palette.outline);
            // The straw leans a little further out on alternate beats, so a sip reads as a sip.
            let bend = 2 + i32::from(phase % 2);
            canvas.line(x + 1, y - 5, x + bend, y - 10, 2, palette.outline);
            canvas.line(x + 1, y - 5, x + bend, y - 10, 1, palette.accent);
        }
    }
}

/// One found thing, drawn from the shared catalogue so a keepsake looks the same wherever it is
/// shown — the scrapbook, a review sheet, or a companion holding it up.
pub(super) fn draw_generated_trinket(
    canvas: &mut Canvas,
    palette: Palette,
    variant: u8,
    detail_seed: u64,
) {
    let ink = crate::trinkets::ink_against(palette, detail_seed ^ u64::from(variant));
    crate::draw_trinket(canvas, ink, variant, crate::TRINKET_FRAME_REST, 0, 0);
}
