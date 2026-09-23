//! Baking a creature's animation into its body, face and trinket atlases, and finding a frame's
//! slot in them.
use super::*;

pub(super) const ATLAS_COLUMNS: u32 = 10;
// There are exactly 27 eyelid/gaze combinations per expression. Keeping one expression per row
// avoids padding slots and leaves enough texture budget for additional pre-baked body actions.
pub(super) const FACE_ATLAS_COLUMNS: u32 = 27;

pub(super) struct AtlasPixels {
    pub(super) body_width: u32,
    pub(super) body_height: u32,
    pub(super) body_pixels: Vec<u8>,
    pub(super) face_width: u32,
    pub(super) face_height: u32,
    pub(super) face_pixels: Vec<u8>,
    pub(super) face_anchors: Vec<PixelPoint>,
    pub(super) silhouette: Vec<(u8, u8)>,
}

/// The first and just-past-the-last rows a frame actually draws on, in art pixels. Measured after
/// the optional outline, because that is the picture the overlay puts on the desktop.
pub(super) fn silhouette_rows(canvas: &formiga_art::Canvas) -> (u8, u8) {
    match canvas.alpha_bounds() {
        Some((_, top, _, bottom)) => (top as u8, (bottom + 1) as u8),
        None => (0, FRAME_SIZE as u8),
    }
}

pub(super) fn build_atlas_pixels(
    creature: &Creature,
    reduce_motion: bool,
    outline: bool,
    dress: Option<AccessoryArt>,
) -> AtlasPixels {
    let body_slots = total_animation_frames();
    let body_rows = body_slots.div_ceil(ATLAS_COLUMNS);
    let body_width = ATLAS_COLUMNS * FRAME_SIZE;
    let body_height = body_rows * FRAME_SIZE;
    let mut body_pixels = vec![0_u8; (body_width * body_height * 4) as usize];
    let mut face_anchors = vec![PixelPoint::default(); body_slots as usize];
    let mut silhouette = vec![(0_u8, FRAME_SIZE as u8); body_slots as usize];
    for clip in BodyClip::baked() {
        let spec = AnimationSpec::for_clip(clip);
        for frame in 0..spec.frames {
            let mut rendered = CreatureRenderer::render_dressed_body_frame(
                &creature.appearance,
                dress,
                clip,
                frame,
                reduce_motion,
            );
            // Baked once into the atlas, so the edge costs nothing per frame and no draw call.
            if outline {
                CreatureRenderer::outline_frame(&mut rendered.canvas);
            }
            let slot = atlas_slot(clip, frame);
            face_anchors[slot as usize] = rendered.face_anchor;
            silhouette[slot as usize] = silhouette_rows(&rendered.canvas);
            blit_atlas_frame(
                &mut body_pixels,
                body_width,
                slot % ATLAS_COLUMNS * FRAME_SIZE,
                slot / ATLAS_COLUMNS * FRAME_SIZE,
                FRAME_SIZE,
                &rendered.canvas.rgba_bytes(),
            );
        }
    }

    let face_slots = face_slot_count();
    let face_rows = (face_slots + 8).div_ceil(FACE_ATLAS_COLUMNS);
    let face_width = FACE_ATLAS_COLUMNS * FACE_FRAME_SIZE;
    let face_height = face_rows * FACE_FRAME_SIZE;
    let mut face_pixels = vec![0_u8; (face_width * face_height * 4) as usize];
    for expression in formiga_art::ExpressionKind::ALL {
        for eyelids in formiga_art::EyelidPose::ALL {
            for gaze_y in -1_i8..=1 {
                for gaze_x in -1_i8..=1 {
                    let state = FaceRenderState {
                        expression,
                        eyelids,
                        gaze: formiga_art::GazeDirection::new(gaze_x, gaze_y),
                    };
                    let face = CreatureRenderer::render_face_frame(&creature.appearance, state);
                    let slot = face_atlas_slot(state);
                    blit_atlas_frame(
                        &mut face_pixels,
                        face_width,
                        slot % FACE_ATLAS_COLUMNS * FACE_FRAME_SIZE,
                        slot / FACE_ATLAS_COLUMNS * FACE_FRAME_SIZE,
                        FACE_FRAME_SIZE,
                        &face.rgba_bytes(),
                    );
                }
            }
        }
    }
    for variant in 0..8_u8 {
        let trinket = CreatureRenderer::render_trinket(&creature.appearance, variant);
        let slot = trinket_atlas_slot(variant);
        blit_atlas_frame(
            &mut face_pixels,
            face_width,
            slot % FACE_ATLAS_COLUMNS * FACE_FRAME_SIZE,
            slot / FACE_ATLAS_COLUMNS * FACE_FRAME_SIZE,
            FACE_FRAME_SIZE,
            &trinket.rgba_bytes(),
        );
    }
    AtlasPixels {
        body_width,
        body_height,
        body_pixels,
        face_width,
        face_height,
        face_pixels,
        face_anchors,
        silhouette,
    }
}

pub(super) fn blit_atlas_frame(
    target: &mut [u8],
    width: u32,
    origin_x: u32,
    origin_y: u32,
    frame_size: u32,
    frame: &[u8],
) {
    for y in 0..frame_size {
        let source_start = (y * frame_size * 4) as usize;
        let target_start = ((origin_y + y) * width * 4 + origin_x * 4) as usize;
        target[target_start..target_start + (frame_size * 4) as usize]
            .copy_from_slice(&frame[source_start..source_start + (frame_size * 4) as usize]);
    }
}

pub(super) fn total_animation_frames() -> u32 {
    BodyClip::baked()
        .map(|clip| u32::from(AnimationSpec::for_clip(clip).frames))
        .sum()
}

pub(super) fn creature_horizontal_scale(action: ActionKind) -> f32 {
    if action == ActionKind::SqueezeWindow {
        0.72
    } else {
        1.0
    }
}

pub(super) fn atlas_slot(clip: impl Into<BodyClip>, frame: u8) -> u32 {
    let clip = clip.into().body();
    let clip_offset: u32 = BodyClip::baked()
        .take_while(|candidate| *candidate != clip)
        .map(|candidate| u32::from(AnimationSpec::for_clip(candidate).frames))
        .sum();
    clip_offset + u32::from(frame)
}

pub(super) fn face_slot_count() -> u32 {
    formiga_art::ExpressionKind::ALL.len() as u32 * formiga_art::EyelidPose::ALL.len() as u32 * 9
}

pub(super) fn face_atlas_slot(state: FaceRenderState) -> u32 {
    state.expression.index() * 27 + state.eyelids.index() * 9 + state.gaze.index()
}

/// Which frame of a trinket to show. Presentation is four frames at 2fps, so a keepsake rests for
/// half a second and twinkles for half a second without any timer of its own.
pub(super) fn trinket_frame(body_frame: u8) -> u8 {
    if body_frame % 4 >= 2 {
        TRINKET_FRAME_GLINT
    } else {
        TRINKET_FRAME_REST
    }
}

/// The eight slots kept in each creature's face texture. Nothing samples them any more — the
/// overlay and the scrapbook both draw from the colony atlas — but the layout, and the exact
/// per-creature texture budget it produces, are unchanged.
pub(super) fn trinket_atlas_slot(variant: u8) -> u32 {
    face_slot_count() + u32::from(variant % 8)
}
