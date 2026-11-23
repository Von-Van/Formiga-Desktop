//! Small marks drawn around a body as it acts, such as sleep, play, a greeting, a startle, a sprint
//! or a find held up, and the ones that go with a cheer, a gasp, a worry, a heave or a bop.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_effects(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    face: PixelPoint,
    pose: Pose,
    action: ActionKind,
    frame: u8,
    reduce_motion: bool,
) {
    let pulse = if reduce_motion {
        0
    } else {
        i32::from(frame % 2)
    };
    match action {
        ActionKind::Sleep => {
            let x = (face.x + 7).min(43);
            let y = (face.y - 8 - pulse).max(3);
            draw_z(canvas, x, y, palette.accent);
        }
        ActionKind::InvestigateCursor => {
            let x = (face.x + 7).min(44);
            let y = (face.y - 6 - pulse).max(3);
            canvas.set(x, y, palette.accent);
            canvas.set(x + 1, y - 1, palette.accent);
            canvas.set(x + 1, y + 1, palette.accent);
            canvas.set(x + 1, y + 3, palette.accent);
        }
        ActionKind::SoloPlay => draw_motif(
            canvas,
            genome.effect_motif,
            (face.x + 8).min(44),
            (face.y + pulse - 7).max(3),
            palette.accent,
        ),
        // A mouthful: crumbs knocked off by the bite falling to the ground, and a cheek filled
        // out by the chew. Each is drawn on frames of its own (see `CRUMB_FRAMES` and
        // `FULL_CHEEK_FRAME`), so a meal re-timed frame by frame only has to move those.
        ActionKind::Eat => {
            let hold = prop_hold(genome, pose, face, BodyClip::Action(action));
            draw_falling_crumbs(canvas, genome, palette, hold, frame);
            if frame % 4 == FULL_CHEEK_FRAME {
                draw_full_cheek(canvas, face);
            }
        }
        ActionKind::Drink => {
            if frame % 4 == 2 {
                canvas.set((face.x + 10).min(45), face.y + 5, palette.highlight);
            }
        }
        ActionKind::Sprint => {
            let y = (face.y + 5).min(43);
            canvas.line(face.x - 10, y - 3, face.x - 7, y - 3, 1, palette.accent);
            if !reduce_motion {
                canvas.line(face.x - 12, y, face.x - 8, y, 1, palette.highlight);
            }
        }
        ActionKind::Greet | ActionKind::SocialPlay => draw_drifting_motif(
            canvas,
            if genome.effect_motif == EffectMotif::None {
                EffectMotif::Spark
            } else {
                genome.effect_motif
            },
            (face.x + 8, face.y - 7),
            frame,
            palette.accent,
        ),
        ActionKind::AvoidCursor | ActionKind::ReactToWindow => {
            let y = (face.y - 7).max(3);
            canvas.line(face.x - 7, y + 2, face.x - 9, y, 1, palette.accent);
            canvas.line(face.x + 7, y + 2, face.x + 9, y, 1, palette.accent);
        }
        ActionKind::ClimbWindow => {
            if !reduce_motion && frame.is_multiple_of(2) {
                canvas.set((face.x + 8).min(45), (face.y - 5).max(2), palette.highlight);
            }
        }
        ActionKind::InspectScreen => {
            let x = (face.x + 7).min(43);
            let y = (face.y + 1).clamp(4, 42);
            canvas.fill_circle(x, y, 2, palette.outline);
            canvas.fill_circle(x, y, 1, Rgba::TRANSPARENT);
            canvas.line(x + 2, y + 2, x + 4, y + 4, 1, palette.accent);
        }
        ActionKind::PresentDiscovery => draw_motif(
            canvas,
            if genome.effect_motif == EffectMotif::None {
                EffectMotif::Star
            } else {
                genome.effect_motif
            },
            (face.x + 9).min(44),
            (face.y - 9 - pulse).max(3),
            palette.highlight,
        ),
        _ => {}
    }
}

/// Small marks that finish a gesture, kept clear of the face and the raised limbs: a burst over
/// a cheer, startle lines for a gasp, a bead of worry, a note for a dance, puffs of effort.
pub(super) fn draw_gesture_effects(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    face: PixelPoint,
    gesture: Gesture,
    frame: u8,
    reduce_motion: bool,
) {
    let pulse = if reduce_motion {
        0
    } else {
        i32::from(frame % 2)
    };
    match gesture {
        // A seat and a sled are drawn by the wonder itself; the body needs nothing more.
        Gesture::Sit | Gesture::Scoot => {}
        // Asleep however it lies, the same little z as a curled sleeper's.
        Gesture::Sprawl | Gesture::Loaf => {
            let x = (face.x + 7).min(43);
            let y = (face.y - 8 - pulse).max(3);
            draw_z(canvas, x, y, palette.accent);
        }
        // Flecks thrown off either side, on the side it has just shaken toward.
        Gesture::Shake => {
            if !reduce_motion {
                let side = if frame.is_multiple_of(2) { 1 } else { -1 };
                let x = (face.x + side * 13).clamp(2, 45);
                let y = (face.y + 2).min(43);
                canvas.set(x, y, palette.highlight);
                canvas.set((x + side * 2).clamp(2, 45), y - 2, palette.highlight);
                canvas.set((x + side).clamp(2, 45), y + 3, palette.highlight);
            }
        }
        // Two wisps of something on the air, drawn in toward the nose.
        Gesture::Sniff => {
            if frame % 4 >= 1 && frame % 4 <= 2 {
                let x = (face.x + 10).min(44);
                let y = (face.y - 1).max(3);
                canvas.set(x, y, palette.highlight);
                canvas.set((x + 2).min(45), y - 1, palette.highlight);
            }
        }
        Gesture::Groom
        | Gesture::Skip
        | Gesture::Trudge
        | Gesture::Pour
        | Gesture::Tend
        | Gesture::Nibble
        | Gesture::Chomp
        | Gesture::Chew
        | Gesture::Gulp => {}
        // A puff of earth where the tug comes away.
        Gesture::Pluck => {
            if frame % 4 == 2 && !reduce_motion {
                let x = (face.x + 9).min(44);
                let y = (face.y + 18).min(44);
                canvas.set(x, y, palette.shadow);
                canvas.set((x + 2).min(45), y - 1, palette.shadow);
                canvas.set((x - 1).max(2), y - 2, palette.highlight);
            }
        }
        // A little heart over a companion being petted, on the push up into the hand.
        Gesture::Bask => {
            if frame % 2 == 1 || reduce_motion {
                draw_motif(
                    canvas,
                    EffectMotif::Heart,
                    (face.x + 9).min(42),
                    (face.y - 11).max(4),
                    palette.highlight,
                );
            }
        }
        Gesture::Cheer => draw_drifting_motif(
            canvas,
            if genome.effect_motif == EffectMotif::None {
                EffectMotif::Spark
            } else {
                genome.effect_motif
            },
            (face.x, face.y - 15),
            frame,
            palette.highlight,
        ),
        Gesture::Gasp => {
            let y = (face.y - 13).max(4);
            canvas.line(face.x - 4, y, face.x - 5, y - 2, 1, palette.accent);
            canvas.line(face.x, y - 1, face.x, y - 3, 1, palette.accent);
            canvas.line(face.x + 4, y, face.x + 5, y - 2, 1, palette.accent);
        }
        Gesture::Worry => {
            let x = (face.x + 8).min(44);
            let y = (face.y - 6 + pulse).clamp(3, 41);
            canvas.set(x, y, palette.highlight);
            canvas.fill_rect(x - 1, y + 1, 3, 2, palette.highlight);
        }
        Gesture::Heave => {
            if frame % 4 >= 2 {
                canvas.fill_circle(
                    (face.x - 12).max(3),
                    (face.y + 8).min(43),
                    1,
                    palette.highlight,
                );
                canvas.set(
                    (face.x - 15).max(2),
                    (face.y + 6).min(43),
                    palette.highlight,
                );
            }
        }
        Gesture::Bop => {
            let x = if frame % 4 < 2 {
                (face.x - 11).max(4)
            } else {
                (face.x + 11).min(42)
            };
            let y = (face.y - 9 - pulse).clamp(5, 40);
            canvas.line(x + 1, y - 3, x + 1, y, 1, palette.accent);
            canvas.set(x + 2, y - 3, palette.accent);
            canvas.fill_circle(x, y + 1, 1, palette.accent);
        }
        // A puff of steam off the top of the head on the hmph.
        Gesture::Huff => {
            if pulse == 1 || reduce_motion {
                let y = (face.y - 12).max(3);
                for x in [(face.x - 6).max(3), (face.x + 6).min(44)] {
                    canvas.set(x, y, palette.highlight);
                    canvas.set(x, y - 1, palette.highlight);
                    canvas.set(x + 1, y - 2, palette.highlight);
                }
            }
        }
        // Stars going round over a swoon.
        Gesture::Swoon => {
            let y = (face.y - 13).max(4);
            let turn = if reduce_motion {
                0
            } else {
                i32::from(frame % 2) * 2
            };
            for dx in [-4 + turn, 3 - turn] {
                let x = (face.x + dx).clamp(3, 44);
                canvas.set(x, y, palette.accent);
                canvas.set(x - 1, y, palette.accent);
                canvas.set(x + 1, y, palette.accent);
                canvas.set(x, y - 1, palette.accent);
                canvas.set(x, y + 1, palette.accent);
            }
        }
        // A glint off the flourish.
        Gesture::Strut => {
            if pulse == 0 {
                draw_motif(
                    canvas,
                    EffectMotif::Spark,
                    (face.x + 12).min(42),
                    (face.y - 12).max(4),
                    palette.highlight,
                );
            }
        }
        // A little dust where the foot comes down.
        Gesture::Stomp => {
            if frame.is_multiple_of(2) && !reduce_motion {
                let y = (face.y + 20).min(44);
                canvas.set((face.x - 9).max(2), y, palette.highlight);
                canvas.set((face.x - 11).max(2), y - 1, palette.highlight);
            }
        }
        // Nothing floats over a watching creature. A mark here would be the creature telling the
        // viewer it is interested; the pose has to say that by itself.
        Gesture::Cover
        | Gesture::Crouch
        | Gesture::Balance
        | Gesture::Reach
        | Gesture::Watch
        | Gesture::Stretch
        | Gesture::Yawn
        | Gesture::Beg
        | Gesture::Peek => {}
    }
}

/// How far a greeting's, a game's or a cheer's motif has drifted on each frame of its four-frame
/// clip, up and out from where it starts beside the head, in art pixels: it rises off the head
/// and away from it as a spark does, rather than hovering in one place.
const MOTIF_DRIFT: [(i32, i32); 4] = [(0, 0), (1, -2), (2, -4), (3, -5)];

/// How opaque a drifting motif is on its last frame, where it fades out before the next one
/// starts beside the head again.
const MOTIF_FADE_ALPHA: u8 = 120;

/// A motif drifting up and out from `start` over the four frames of its clip and fading on the
/// last. Reduced motion holds the first frame, which is where the motif always sat.
pub(super) fn draw_drifting_motif(
    canvas: &mut Canvas,
    motif: EffectMotif,
    start: (i32, i32),
    frame: u8,
    color: Rgba,
) {
    let step = usize::from(frame % 4);
    let (dx, dy) = MOTIF_DRIFT[step];
    // Kept two pixels in from the frame's edges, where a motif's arms reach, so it never pushes
    // the body over in the frame.
    let x = (start.0 + dx).min(44);
    let y = (start.1 + dy).max(3);
    if step + 1 < MOTIF_DRIFT.len() {
        draw_motif(canvas, motif, x, y, color);
        return;
    }
    let mut faded = Canvas::new(canvas.width(), canvas.height());
    draw_motif(&mut faded, motif, x, y, color);
    for py in 0..faded.height() as i32 {
        for px in 0..faded.width() as i32 {
            let pixel = faded.get(px, py);
            if pixel.a > 0 {
                crate::paint::put(canvas, px, py, crate::paint::faded(pixel, MOTIF_FADE_ALPHA));
            }
        }
    }
}

/// The frames of `ActionKind::Eat` whose crumbs fall: knocked off by the bite on the first, and
/// on the ground on the second.
pub(super) const CRUMB_FRAMES: [u8; 2] = [2, 3];

/// The frame of `ActionKind::Eat` whose cheek is filled out by the chew.
pub(super) const FULL_CHEEK_FRAME: u8 = 3;

/// Two crumbs off a mouthful: just under the mouth on the first of [`CRUMB_FRAMES`] and down on
/// the ground row the body stands on, `hold.ground`, on the second. Coloured against the
/// creature, as the snack is, so they read on its coat.
pub(super) fn draw_falling_crumbs(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    hold: PropHold,
    frame: u8,
) {
    let Some(fallen) = CRUMB_FRAMES.iter().position(|at| *at == frame % 4) else {
        return;
    };
    let palette = crate::prop_palette(palette, prop_signature(genome));
    let mouth = hold.mouth;
    let ground = hold.ground.min(FRAME_SIZE as i32 - 2);
    // One falls a little behind the other, so the pair lands as two crumbs rather than a dash.
    let rows = if fallen == 0 {
        [mouth.y + 3, mouth.y + 4]
    } else {
        [ground, ground]
    };
    canvas.set(mouth.x - 2, rows[0].min(ground), palette.shadow);
    canvas.set(mouth.x + 1, rows[1].min(ground), palette.highlight);
}

/// A cheek filled out by a chew: the outline of the head pushed out a pixel beside the mouth, two
/// rows deep, on the side the face looks toward. Found by walking out from the face along the
/// cheek's rows to the edge of whatever is drawn there, so it sits on the head each body plan
/// actually drew. A row with no edge within reach is left as it is.
pub(super) fn draw_full_cheek(canvas: &mut Canvas, face: PixelPoint) {
    const REACH: i32 = 12;
    for y in [face.y + 2, face.y + 3] {
        if canvas.get(face.x, y).a == 0 {
            continue;
        }
        let Some(edge) = (face.x..face.x + REACH)
            .find(|x| canvas.get(x + 1, y).a == 0)
            .filter(|x| *x > face.x + 1 && *x + 1 < FRAME_SIZE as i32 - 2)
        else {
            continue;
        };
        let outline = canvas.get(edge, y);
        let fill = canvas.get(edge - 1, y);
        canvas.set(edge + 1, y, outline);
        canvas.set(edge, y, fill);
    }
}

pub(super) fn draw_motif(canvas: &mut Canvas, motif: EffectMotif, x: i32, y: i32, color: Rgba) {
    match motif {
        EffectMotif::None => {}
        EffectMotif::Dot => canvas.fill_circle(x, y, 1, color),
        EffectMotif::Star | EffectMotif::Spark => {
            canvas.line(x - 2, y, x + 2, y, 1, color);
            canvas.line(x, y - 2, x, y + 2, 1, color);
        }
        EffectMotif::Heart => {
            canvas.set(x - 1, y - 1, color);
            canvas.set(x + 1, y - 1, color);
            canvas.fill_rect(x - 1, y, 3, 2, color);
            canvas.set(x, y + 2, color);
        }
        EffectMotif::Leaf => {
            canvas.line(x - 1, y + 1, x + 1, y - 1, 1, color);
            canvas.set(x - 1, y, color);
            canvas.set(x, y - 1, color);
        }
    }
}

/// A sleeper's z, four pixels square so it reads as a Z: a bar across the top, a stroke from its
/// right end down to the left, and a bar across the bottom. Three square has only the one middle
/// pixel to join its bars, and reads as an I.
fn draw_z(canvas: &mut Canvas, x: i32, y: i32, colour: Rgba) {
    canvas.line(x, y, x + 3, y, 1, colour);
    canvas.set(x + 2, y + 1, colour);
    canvas.set(x + 1, y + 2, colour);
    canvas.line(x, y + 3, x + 3, y + 3, 1, colour);
}
