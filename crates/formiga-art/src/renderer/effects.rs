//! Small marks drawn around a body as it acts, such as sleep, play, a greeting, a startle, a sprint
//! or a find held up, and the ones that go with a cheer, a gasp, a worry, a heave or a bop.
use super::*;

pub(super) fn draw_effects(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    face: PixelPoint,
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
            let x = (face.x + 7).min(44);
            let y = (face.y - 7 - pulse).max(3);
            canvas.line(x, y + 2, x + 2, y + 2, 1, palette.accent);
            canvas.line(x + 2, y + 2, x, y, 1, palette.accent);
            canvas.line(x, y, x + 2, y, 1, palette.accent);
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
        ActionKind::Eat => {
            if frame % 4 == 3 {
                canvas.set((face.x + 6).min(44), face.y + 2, palette.accent);
                canvas.set((face.x + 8).min(45), face.y + 4, palette.highlight);
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
        ActionKind::Greet | ActionKind::SocialPlay => {
            draw_motif(
                canvas,
                if genome.effect_motif == EffectMotif::None {
                    EffectMotif::Spark
                } else {
                    genome.effect_motif
                },
                (face.x + 8).min(44),
                (face.y - 7 - pulse).max(3),
                palette.accent,
            );
        }
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
        Gesture::Cheer => draw_motif(
            canvas,
            if genome.effect_motif == EffectMotif::None {
                EffectMotif::Spark
            } else {
                genome.effect_motif
            },
            face.x,
            (face.y - 15 - pulse).max(3),
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
        // Nothing floats over a watching creature. A mark here would be the creature telling the
        // viewer it is interested; the pose has to say that by itself.
        Gesture::Cover
        | Gesture::Crouch
        | Gesture::Balance
        | Gesture::Reach
        | Gesture::Watch
        | Gesture::Stretch
        | Gesture::Yawn => {}
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
