//! Baking a creature's animation into its body, face and trinket atlases, and finding a frame's
//! slot in them.
use super::*;

pub(super) const ATLAS_COLUMNS: u32 = 10;
/// Face cells to a row of the face texture: as many as there are eyelid and gaze combinations for
/// one expression.
pub(super) const FACE_ATLAS_COLUMNS: u32 = 27;

pub(super) struct AtlasPixels {
    /// How many texture pixels each art pixel is baked as, across and down: the body's cells are
    /// this many times [`FRAME_SIZE`] square, and the face's this many times [`FACE_FRAME_SIZE`].
    /// Face anchors and silhouettes stay in art pixels.
    pub(super) detail: u32,
    pub(super) body_width: u32,
    pub(super) body_height: u32,
    pub(super) body_pixels: Vec<u8>,
    /// The cell of the body texture each baked frame is drawn from, by slot. Frames that come out
    /// identical — a pose held across frames, a whole clip under reduced motion — share one cell.
    pub(super) body_cells: Vec<u16>,
    pub(super) face_width: u32,
    pub(super) face_height: u32,
    pub(super) face_pixels: Vec<u8>,
    /// The same for faces, by face slot: shut eyes, for one, look the same whichever way they
    /// would be looking.
    pub(super) face_cells: Vec<u16>,
    pub(super) face_anchors: Vec<PixelPoint>,
    pub(super) silhouette: Vec<(u8, u8)>,
}

/// Lays frames of `frame_size` pixels square into a texture `columns` cells wide, each distinct
/// frame once, in the order they first appear. Returns the texture's width, height and pixels, and
/// the cell each frame landed in.
fn pack(frames: Vec<Vec<u8>>, frame_size: u32, columns: u32) -> (u32, u32, Vec<u8>, Vec<u16>) {
    let mut distinct: Vec<Vec<u8>> = Vec::new();
    let mut seen: std::collections::HashMap<Vec<u8>, u16> = std::collections::HashMap::new();
    let cells = frames
        .into_iter()
        .map(|frame| {
            *seen.entry(frame).or_insert_with_key(|frame| {
                distinct.push(frame.clone());
                (distinct.len() - 1) as u16
            })
        })
        .collect();
    let rows = (distinct.len() as u32).div_ceil(columns).max(1);
    let (width, height) = (columns * frame_size, rows * frame_size);
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for (cell, frame) in (0_u32..).zip(&distinct) {
        blit_atlas_frame(
            &mut pixels,
            width,
            cell % columns * frame_size,
            cell / columns * frame_size,
            frame_size,
            frame,
        );
    }
    (width, height, pixels, cells)
}

/// The first and just-past-the-last rows a frame actually draws on, in art pixels. Measured after
/// the optional outline, because that is the picture the overlay puts on the desktop.
pub(super) fn silhouette_rows(canvas: &formiga_art::Canvas) -> (u8, u8) {
    match canvas.alpha_bounds() {
        Some((_, top, _, bottom)) => (top as u8, (bottom + 1) as u8),
        None => (0, FRAME_SIZE as u8),
    }
}

/// How many texture pixels each art pixel of `appearance` is baked as, on a display that shows it
/// `display_scale` screen pixels to the art pixel, drawn by an overlay `render_divisor` screen
/// pixels to each of its own. A companion is pixel art at one pixel to the unit, and is baked at
/// one. A sculpted form is painted finer, as Formiga Farm draws it, and is baked as finely as the
/// overlay draws it and no finer, so it is drawn at the same size, never shrunk and never
/// blurred. On a Retina display at an even size the overlay draws at half resolution, and a bake
/// finer than that would only be sampled away, a pixel here and there.
pub(super) fn baked_detail(
    appearance: &AppearanceGenome,
    display_scale: u8,
    render_divisor: u32,
) -> u32 {
    if appearance.sculpt.is_none() {
        return 1;
    }
    formiga_art::forms::FINEST_DETAIL
        .min(u32::from(display_scale) / render_divisor.max(1))
        .max(1)
}

pub(super) fn build_atlas_pixels(
    creature: &Creature,
    reduce_motion: bool,
    outline: bool,
    dress: Option<AccessoryArt>,
    detail: u32,
) -> AtlasPixels {
    let body_slots = total_animation_frames();
    let mut body_frames = vec![Vec::new(); body_slots as usize];
    let mut face_anchors = vec![PixelPoint::default(); body_slots as usize];
    let mut silhouette = vec![(0_u8, FRAME_SIZE as u8); body_slots as usize];
    for clip in BodyClip::baked() {
        let spec = AnimationSpec::for_clip(clip);
        for frame in 0..spec.frames {
            // The frame itself, for its anchors and silhouette, and the picture baked from it.
            let (mut rendered, mut fine) = if detail > 1 {
                let (rendered, fine) = CreatureRenderer::render_dressed_body_frame_in_detail(
                    &creature.appearance,
                    dress,
                    clip,
                    frame,
                    reduce_motion,
                    detail,
                );
                (rendered, Some(fine))
            } else {
                let rendered = CreatureRenderer::render_dressed_body_frame(
                    &creature.appearance,
                    dress,
                    clip,
                    frame,
                    reduce_motion,
                );
                (rendered, None)
            };
            // Baked once into the atlas, so the edge costs nothing per frame and no draw call.
            if outline {
                CreatureRenderer::outline_frame(&mut rendered.canvas);
                if let Some(fine) = &mut fine {
                    CreatureRenderer::outline_frame_in_detail(fine, detail);
                }
            }
            let slot = atlas_slot(clip, frame) as usize;
            face_anchors[slot] = rendered.face_anchor;
            silhouette[slot] = silhouette_rows(&rendered.canvas);
            body_frames[slot] = fine.unwrap_or(rendered.canvas).rgba_bytes();
        }
    }
    let (body_width, body_height, body_pixels, body_cells) =
        pack(body_frames, FRAME_SIZE * detail, ATLAS_COLUMNS);

    let mut face_frames = vec![Vec::new(); face_slot_count() as usize];
    for expression in formiga_art::ExpressionKind::ALL {
        for eyelids in formiga_art::EyelidPose::ALL {
            for gaze_y in -1_i8..=1 {
                for gaze_x in -1_i8..=1 {
                    let state = FaceRenderState {
                        expression,
                        eyelids,
                        gaze: formiga_art::GazeDirection::new(gaze_x, gaze_y),
                    };
                    let face = CreatureRenderer::render_face_frame_in_detail(
                        &creature.appearance,
                        state,
                        detail,
                    );
                    face_frames[face_atlas_slot(state) as usize] = face.rgba_bytes();
                }
            }
        }
    }
    let (face_width, face_height, face_pixels, face_cells) =
        pack(face_frames, FACE_FRAME_SIZE * detail, FACE_ATLAS_COLUMNS);
    AtlasPixels {
        detail,
        body_width,
        body_height,
        body_pixels,
        body_cells,
        face_width,
        face_height,
        face_pixels,
        face_cells,
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

/// Where cell `cell` of an atlas `columns` cells wide, of cells `cell_size` texture pixels square,
/// lies in a texture `width` by `height`, as left, right, top and bottom texture coordinates. A
/// finer atlas is the same layout at more pixels, so a cell lies in the same place in it.
pub(super) fn cell_uv(
    cell: u32,
    columns: u32,
    cell_size: u32,
    width: u32,
    height: u32,
) -> [f32; 4] {
    let (column, row) = (cell % columns, cell / columns);
    [
        (column * cell_size) as f32 / width as f32,
        ((column + 1) * cell_size) as f32 / width as f32,
        (row * cell_size) as f32 / height as f32,
        ((row + 1) * cell_size) as f32 / height as f32,
    ]
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
