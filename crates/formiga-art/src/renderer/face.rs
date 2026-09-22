//! The layered face: eyes, brows, cheeks and mouth, and which expression, eyelids and gaze a
//! creature shows.
use super::*;

pub(super) fn draw_face(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    center_x: i32,
    center_y: i32,
    state: FaceRenderState,
) {
    let mut face = genome.face;
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
        genome,
        palette,
        center_x,
        y,
        spacing,
        state.expression,
    );
    draw_cheeks(
        canvas,
        genome,
        palette,
        center_x,
        y,
        spacing,
        state.expression,
    );
    draw_mouth(
        canvas,
        genome,
        palette,
        center_x,
        y + eye_radius + 3,
        state.expression,
    );
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
}

pub(super) fn draw_brows(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
    palette: Palette,
    center_x: i32,
    y: i32,
    spacing: i32,
    expression: ExpressionKind,
) {
    let weight = match genome.face.brow_style {
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
    let (left_inner, right_inner) = match expression {
        ExpressionKind::Worried | ExpressionKind::Affectionate => (-1, -1),
        ExpressionKind::Focused | ExpressionKind::Determined => (1, 1),
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
    genome: &AppearanceGenome,
    palette: Palette,
    center_x: i32,
    y: i32,
    spacing: i32,
    expression: ExpressionKind,
) {
    if genome.face.cheek_style == CheekStyle::None
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
        if genome.face.cheek_style == CheekStyle::Blush {
            canvas.set(x + 1, cheek_y, palette.accent);
        }
    }
}

pub(super) fn draw_mouth(
    canvas: &mut Canvas,
    genome: &AppearanceGenome,
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
        _ => match genome.face.mouth_style {
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
    if let Some(pose) = creature.state.attention {
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
            if creature.tendencies.sociability >= 35 || creature.state.drives.comfort > 0.75 {
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

pub(super) fn resolve_eyelids(creature: &Creature) -> EyelidPose {
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
    if matches!(resolve_expression(creature), ExpressionKind::Startled) {
        return EyelidPose::Open;
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

pub(super) fn resolve_gaze(
    creature: &Creature,
    cursor: CursorSnapshot,
    cursor_reactions: bool,
) -> GazeDirection {
    if let Some(pose) = creature.state.attention {
        return GazeDirection::new(
            axis_direction(pose.target.x - creature.state.position.x, 10.0),
            axis_direction(pose.target.y - (creature.state.position.y - 28.0), 10.0),
        );
    }
    let forward = if creature.state.facing_right { 1 } else { -1 };
    // Looking the snack over: down at it, closer, then up and along it.
    if let Some((Habit::LooksFoodOver, _, into)) = flourish_shown(creature) {
        return match into {
            into if into < 0.55 => GazeDirection::new(forward, 1),
            into if into < 1.1 => GazeDirection::new(0, 1),
            _ => GazeDirection::new(forward, 0),
        };
    }
    match creature.state.action {
        ActionKind::InspectScreen => {
            return GazeDirection::new(
                if creature.state.facing_right { 1 } else { -1 },
                if creature.state.surface.kind == formiga_core::SurfaceKind::WindowLedge {
                    1
                } else {
                    -1
                },
            );
        }
        ActionKind::Dangle => return GazeDirection::new(0, 1),
        ActionKind::PresentDiscovery => return GazeDirection::new(0, -1),
        _ => {}
    }
    if !cursor_reactions
        || !cursor.available
        || creature.state.position.distance(cursor.position) > 240.0
    {
        return GazeDirection::default();
    }
    let face_position = formiga_core::Point {
        x: creature.state.position.x,
        y: creature.state.position.y - 28.0,
    };
    let dx = cursor.position.x - face_position.x;
    let dy = cursor.position.y - face_position.y;
    GazeDirection::new(axis_direction(dx, 10.0), axis_direction(dy, 10.0))
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
