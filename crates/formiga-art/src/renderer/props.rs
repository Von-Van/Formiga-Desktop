//! Belongings a body holds: which toy, snack and cup it owns, where it holds them, and how each is
//! drawn. The things themselves are drawn apart from the body; see [`super::held`].
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

/// Where a body holds what it is using, and how low it may go. Props are placed from the frame's
/// own two points, read from the paw it drew and the mouth its face is drawn with (see
/// [`HoldPoints`]), so a toy lands in the paw that is actually drawn and a snack arrives at the
/// mouth that is eating it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PropHold {
    pub(crate) hands: PixelPoint,
    pub(crate) mouth: PixelPoint,
    pub(crate) floor: i32,
    /// The lowest row this body's feet reach. Nothing it drops lands below it.
    pub(crate) ground: i32,
    /// Which way a toy is knocked from the paw: `1`, the way the frame faces, or `-1`, back along
    /// a body whose head leads out over its paws.
    pub(crate) forward: i32,
    /// The face anchor, which a toy in the air keeps clear of.
    pub(crate) face: PixelPoint,
    /// The top of the head, which a body with no paw free keeps its toy up off.
    pub(crate) crown: PixelPoint,
    /// Whether `hands` is on a paw the frame drew, rather than under the mouth.
    pub(crate) paw: bool,
}

/// What a frame holds from: its own `hold`, the face and paw of its `figure`, and the floor and
/// ground the body stands on.
pub(super) fn prop_hold(
    genome: &AppearanceGenome,
    pose: Pose,
    clip: BodyClip,
    hold: HoldPoints,
    figure: Figure,
) -> PropHold {
    let (floor, ground) = if genome.echo_fox {
        fox::footing(pose, clip)
    } else if let Some(design) = genome.design {
        modular::footing(design, pose, scale(genome))
    } else {
        // A blob's mass is pinned to row 38 and its feet hang two rows below it; the legged
        // families stand on a ground of their own at row 45.
        (
            42,
            match genome.family {
                BodyFamily::Blob => 40,
                BodyFamily::Hopper | BodyFamily::SoftQuadruped => 45,
            },
        )
    };
    // A body on four legs carries its head out ahead over the paws it plays with, so a toy is
    // knocked back along it, over its shoulders and away from the face; every other body knocks
    // it forward.
    let forward = if genome.sculpt.is_none() && !genome.echo_fox && stretches_long(genome) {
        -1
    } else {
        1
    };
    PropHold {
        hands: hold.hands,
        mouth: hold.mouth,
        floor,
        ground,
        forward,
        face: figure.face,
        crown: figure.crown,
        paw: figure.hand.is_some(),
    }
}

/// What a body is holding in one frame of play, a mouthful or a sip, and where: the thing itself
/// is drawn apart from the body (see [`super::held`]), so this only says which picture and where
/// its middle goes.
pub(super) fn activity_held(
    genome: &AppearanceGenome,
    hold: PropHold,
    clip: BodyClip,
    frame: u8,
    reduce_motion: bool,
) -> Option<HeldPlacement> {
    let phase = if reduce_motion {
        AnimationSpec::still_frame(clip)
    } else {
        frame
    } % 4;
    let (toy, snack, drink) = prop_variants(genome);
    let snack_at = |bite: u8, x: i32, y: i32| {
        Some(HeldPlacement::centred(
            HeldThing::Snack { kind: snack, bite },
            x,
            y,
        ))
    };
    let action = match clip {
        BodyClip::Action(action) => action,
        // The eating gestures carry their food at the mouth: nibbled at in little dips, pushed in
        // for a big bite, lowered while the mouth works, and up over the face to drop in.
        // Each is held in front of the mouth with its near edge at it, so what is eaten is seen
        // going into the mouth rather than laid over the face.
        BodyClip::Gesture(Gesture::Nibble) => {
            let dip = [0, 1, 0, 1][usize::from(phase)];
            return snack_at(1, hold.mouth.x + 4, hold.mouth.y + 3 - dip);
        }
        BodyClip::Gesture(Gesture::Chomp) => {
            let push = [0, 1, 2, 0][usize::from(phase)];
            return snack_at(1, hold.mouth.x + 5 - push, hold.mouth.y + 4 - push);
        }
        // Lowered in the paw while the mouth works, or held under the mouth by a body with no
        // paw free.
        BodyClip::Gesture(Gesture::Chew) => {
            return if hold.paw {
                snack_at(2, hold.hands.x, hold.hands.y.min(hold.floor - 3))
            } else {
                snack_at(2, hold.mouth.x + 3, (hold.mouth.y + 4).min(hold.floor - 3))
            };
        }
        // Up over the face in the paw and dropped into the mouth; a body with no paw free
        // tosses it up off the mouth instead.
        BodyClip::Gesture(Gesture::Gulp) => {
            return match (hold.paw, phase) {
                (true, 0..=2) => snack_at(0, hold.hands.x, hold.hands.y),
                (true, _) => snack_at(0, hold.mouth.x + 4, hold.mouth.y + 2),
                (false, _) => {
                    let up = [1, 3, 2, 0][usize::from(phase)];
                    snack_at(0, hold.mouth.x + 5, hold.mouth.y + 2 - up)
                }
            };
        }
        // Poses a meal borrows between bites, holding the food where the paws are.
        BodyClip::Gesture(Gesture::Gasp | Gesture::Loaf | Gesture::Worry) => {
            return snack_at(1, hold.hands.x, hold.hands.y.min(hold.floor - 3));
        }
        // Every other pose holds things where its paws are, for a game that needs it; nothing is
        // drawn there unless a meal or a game says so.
        BodyClip::Gesture(_) => {
            return snack_at(1, hold.hands.x, hold.hands.y.min(hold.floor - 3));
        }
    };
    match action {
        ActionKind::SoloPlay => {
            // One round of keep-up in time with the body (see [`KEEP_UP_TOSS`]): caught against
            // the paw on the crouch, knocked up off it, at the top of its arc on the stretch and
            // dropping back, each beat measured from where that frame holds it.
            let (x, y, spin) = play_path(genome, hold, toy, phase);
            let thing = HeldThing::Toy {
                kind: toy,
                colourway: toy_colourway(genome),
                spin,
            };
            Some(HeldPlacement::centred(thing, x, y))
        }
        ActionKind::Eat => {
            // A mouthful in time with the head (see [`MOUTHFUL`]): held up in front of the mouth,
            // then carried up into it as the head dips to meet it, so it goes with the mouth
            // rather than staying where it was held, and gone while it is chewed. It is never
            // held higher than just under the mouth, so it never covers the eyes over it.
            let (bite, dx, dy) = match MOUTHFUL[usize::from(phase)] {
                Mouthful::Raised => (0, 5, 3),
                Mouthful::Bitten => (1, 4, 2),
                Mouthful::Chewing => return None,
            };
            let thing = HeldThing::Snack { kind: snack, bite };
            Some(HeldPlacement::centred(
                thing,
                hold.mouth.x + dx,
                hold.mouth.y + dy,
            ))
        }
        ActionKind::Drink => {
            // Tipped up with its rim at the mouth for the sip, and lowered a little out in front
            // between sips, its rim still under the mouth and the steam off a mug rising clear
            // of the face. Held from the mouth, it goes back with the head as the head tips back
            // to drink (see [`SIP_TILT`]).
            let tipped = SIP_TILT[usize::from(phase)] > 0;
            let (dx, dy) = cup_hold(drink, tipped);
            let thing = HeldThing::Cup {
                kind: drink,
                tipped,
                sip: phase % 2,
            };
            Some(HeldPlacement::centred(
                thing,
                hold.mouth.x + dx,
                hold.mouth.y + dy,
            ))
        }
        // Down off a ledge, under an umbrella if the colony has one.
        ActionKind::Landing => snack_at(1, hold.hands.x, hold.hands.y.min(hold.floor - 3)),
        _ => None,
    }
}

/// Where a cup's middle goes from the mouth, tipped up to it or lowered between sips. Each kind
/// is held by its rim: a mug's is five rows above its middle and reaches five columns back, a
/// shallow bowl's two rows up and wider still, and a bottle's cap six rows up and narrow, so a
/// bottle is lowered under the mouth rather than out in front of it, and lower, so the straw
/// standing out of it stays clear of the eyes.
fn cup_hold(kind: u8, tipped: bool) -> (i32, i32) {
    match (kind % DRINK_KINDS, tipped) {
        (0, true) => (3, 4),
        (0, false) => (5, 6),
        (1, true) => (3, 1),
        (1, false) => (5, 3),
        (_, true) => (3, 5),
        (_, false) => (1, 8),
    }
}

/// The marks play leaves on the body's own frame: the dashes behind a toy in the air and the knock
/// where it meets the paw. They are the creature's own doing, so they stay in its frame while the
/// thing it holds is drawn apart. `held` is what the frame holds, and `previous` where its middle
/// was on the frame before, which the dashes run from. A mouthful's crumbs are drawn with the
/// meal's other marks (see `effects::draw_falling_crumbs`).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_activity_marks(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    hold: PropHold,
    action: ActionKind,
    frame: u8,
    reduce_motion: bool,
    held: Option<HeldPlacement>,
    previous: Option<PixelPoint>,
) {
    let phase = if reduce_motion {
        AnimationSpec::still_frame(action)
    } else {
        frame
    } % 4;
    // Marks are coloured against the creature, not from it, so they stay readable on its coat.
    let palette = crate::prop_palette(palette, prop_signature(genome));
    if action != ActionKind::SoloPlay {
        return;
    }
    let Some(PixelPoint { x, y }) = held.map(HeldPlacement::middle) else {
        return;
    };
    if !reduce_motion && let Some(from) = previous {
        draw_prop_trail(canvas, palette, (from.x, from.y), (x, y));
    }
    if phase == 0 {
        // The knock itself, on the beat it is caught: two ticks where the toy meets the paw that
        // is keeping it up.
        for (dx, dy) in [(-1, 3), (1, 4)] {
            canvas.set(x + dx * hold.forward, y + dy, palette.highlight);
        }
    }
}

/// The pixels of a face, and those `margin` pixels round it, as the overlay draws it over a frame:
/// what a toy in the air keeps off. Read from the face itself, in the expressions play wears, so
/// a wide face or a low one is kept clear of exactly as it is drawn.
struct FaceMask {
    left: i32,
    top: i32,
    rows: [u32; FACE_MASK_SIZE],
}

/// How wide and tall a [`FaceMask`] is: a face frame and a margin of up to two pixels round it.
const FACE_MASK_SIZE: usize = FACE_FRAME_SIZE as usize + 4;

impl FaceMask {
    fn of(genome: &AppearanceGenome, face: PixelPoint, margin: i32) -> Self {
        let half = FACE_FRAME_SIZE as i32 / 2;
        let (left, top) = (face.x - half - 2, face.y - half - 2);
        let mut rows = [0_u32; FACE_MASK_SIZE];
        for expression in [ExpressionKind::Joy, ExpressionKind::Neutral] {
            let drawn = CreatureRenderer::render_face_frame(
                genome,
                FaceRenderState {
                    expression,
                    eyelids: EyelidPose::Open,
                    gaze: GazeDirection::default(),
                },
            );
            for y in 0..FACE_FRAME_SIZE as i32 {
                for x in 0..FACE_FRAME_SIZE as i32 {
                    if drawn.get(x, y).a == 0 {
                        continue;
                    }
                    for dy in -margin..=margin {
                        for dx in -margin..=margin {
                            let (row, column) = (y + 2 + dy, x + 2 + dx);
                            if (0..FACE_MASK_SIZE as i32).contains(&row)
                                && (0..FACE_MASK_SIZE as i32).contains(&column)
                            {
                                rows[row as usize] |= 1 << column;
                            }
                        }
                    }
                }
            }
        }
        Self { left, top, rows }
    }

    fn hits(&self, x: i32, y: i32) -> bool {
        let (row, column) = (y - self.top, x - self.left);
        (0..FACE_MASK_SIZE as i32).contains(&row)
            && (0..FACE_MASK_SIZE as i32).contains(&column)
            && self.rows[row as usize] & (1 << column) != 0
    }
}

/// Where toy `kind` is on each beat of the play loop, and how far it has turned by then: its
/// middle, in frame pixels facing right.
///
/// Each beat is measured from where that frame holds the toy (see [`KEEP_UP_TOSS`]). A paw keeps
/// it up from beside itself, the side away from the body; a body with no paw free keeps it up off
/// the top of its head. On the beat it is caught it touches the paw, as near to resting against
/// it as the face allows. It never goes across the face: where the beat would put it there, it
/// goes on ahead or higher, the least it takes, and only failing those lower or back.
pub(super) fn play_path(
    genome: &AppearanceGenome,
    hold: PropHold,
    kind: u8,
    phase: u8,
) -> (i32, i32, u8) {
    let beat = phase % 4;
    let spin = beat;
    let toy = HeldThing::Toy {
        kind,
        colourway: 0,
        spin,
    }
    .drawn();
    let (left, top, right, bottom) = toy.extent();
    let dots = toy.pixels();
    let forward = hold.forward;
    let size = FRAME_SIZE as i32;
    // Nothing sinks into the surface underfoot, and the whole toy stays a pixel inside the frame
    // for the atlas's margin.
    let (highest, lowest) = (1 - top, (hold.floor - 1 - bottom).min(size - 2 - bottom));
    let fit_y = |y: i32| y.min(lowest).max(highest);
    let fit_x = |x: i32| x.clamp(1 - left, size - 2 - right);
    let fits = |(x, y): (i32, i32)| fit_x(x) == x && fit_y(y) == y;
    // Clear of the face and a pixel round it, so a glance or a blink never brushes it; failing
    // that, clear of the face itself.
    let masks = [
        FaceMask::of(genome, hold.face, 1),
        FaceMask::of(genome, hold.face, 0),
    ];
    let clear = |mask: &FaceMask, (x, y): (i32, i32)| {
        !dots.iter().any(|&(dx, dy)| mask.hits(x + dx, y + dy))
    };
    let (rest_x, rest_y) = if hold.paw {
        // Against the paw on the side away from the body, its near edge on the paw's own row.
        let y = fit_y(hold.hands.y - 1);
        (hold.hands.x - toy.edge(hold.hands.y - y, -forward), y)
    } else {
        // Resting on the top of the head.
        (hold.crown.x, fit_y(hold.crown.y - 1 - bottom))
    };
    // Every place the toy could go within `reach` of `from`, that keeps it whole in the frame and
    // off the face.
    let places = |mask: &FaceMask, from: (i32, i32), reach: i32| {
        (-reach..=reach)
            .flat_map(move |dy| (-reach..=reach).map(move |dx| (from.0 + dx, from.1 + dy)))
            .filter(move |&at| fits(at) && clear(mask, at))
            .collect::<Vec<_>>()
    };
    if beat == 0 && hold.paw {
        // Caught: on the side of the paw away from the body, so it is not across the paw from
        // where it is knocked to; as near the paw as the face lets it be, touching it along a row
        // or a column if it can; then as near to resting against it as it can be.
        let (hx, hy) = (hold.hands.x, hold.hands.y);
        let gap = |(x, y): (i32, i32)| {
            dots.iter()
                .map(|&(dx, dy)| ((x + dx - hx).abs() + (y + dy - hy).abs() - 1).max(0))
                .min()
                .unwrap_or(i32::MAX)
        };
        let caught = masks.iter().find_map(|mask| {
            places(mask, (rest_x, rest_y), 12)
                .into_iter()
                .min_by_key(|&(x, y)| {
                    (
                        (x - rest_x) * forward < -2,
                        gap((x, y)),
                        2 * ((x - rest_x).abs() + (y - rest_y).abs())
                            + i32::from((x - rest_x) * forward < 0),
                    )
                })
        });
        if let Some((x, y)) = caught {
            return (x, y, spin);
        }
    }
    let (ahead, lift) = KEEP_UP_TOSS[usize::from(beat)];
    let from = (fit_x(rest_x + ahead * forward), fit_y(rest_y - lift));
    // On ahead or up is cheapest, lower dearer, and back toward the body dearest of all, so the
    // toy keeps to one side of the face through the whole round.
    let cost = |(x, y): (i32, i32)| {
        let on = (x - from.0) * forward;
        let up = from.1 - y;
        2 * on.max(0) + 5 * (-on).max(0) + 2 * up.max(0) + 3 * (-up).max(0)
    };
    let (x, y) = masks
        .iter()
        .find_map(|mask| {
            places(mask, from, size / 2)
                .into_iter()
                .min_by_key(|&at| cost(at))
        })
        .unwrap_or(from);
    (x, y, spin)
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
