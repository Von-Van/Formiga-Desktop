//! Drawing into RGBA pixel buffers, blending, and writing them out as PNGs.

use anyhow::{Context, Result};
use formiga_art::FRAME_SIZE;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

#[allow(clippy::too_many_arguments)]
/// A canvas of any size drawn `scale` times over at `(origin_x, origin_y)`, blended over what is
/// there.
pub(crate) fn blit_canvas_scaled(
    target: &mut [u8],
    target_width: u32,
    origin_x: u32,
    origin_y: u32,
    canvas: &formiga_art::Canvas,
    scale: u32,
) {
    let target_height = (target.len() / 4) as u32 / target_width;
    for sy in 0..canvas.height() {
        for sx in 0..canvas.width() {
            let pixel = canvas.get(sx as i32, sy as i32);
            if pixel.a == 0 {
                continue;
            }
            for oy in 0..scale {
                for ox in 0..scale {
                    let (x, y) = (origin_x + sx * scale + ox, origin_y + sy * scale + oy);
                    if x < target_width && y < target_height {
                        blend_pixel(
                            target,
                            target_width,
                            x,
                            y,
                            [pixel.r, pixel.g, pixel.b, pixel.a],
                        );
                    }
                }
            }
        }
    }
}

/// The size of a cell that holds a companion beside the reference tree, in art pixels: the tree's
/// width past the frame, and its height with a little room under the ground.
pub(crate) const SCENE_CELL: (u32, u32) = (102, 66);

pub(crate) fn blit_scaled_square_alpha(
    target: &mut [u8],
    target_width: u32,
    origin_x: u32,
    origin_y: u32,
    source: &[u8],
    source_size: u32,
    scale: u32,
) {
    for sy in 0..source_size {
        for sx in 0..source_size {
            let source_index = ((sy * source_size + sx) * 4) as usize;
            let color = [
                source[source_index],
                source[source_index + 1],
                source[source_index + 2],
                source[source_index + 3],
            ];
            if color[3] == 0 {
                continue;
            }
            for oy in 0..scale {
                for ox in 0..scale {
                    blend_pixel(
                        target,
                        target_width,
                        origin_x + sx * scale + ox,
                        origin_y + sy * scale + oy,
                        color,
                    );
                }
            }
        }
    }
}

pub(crate) fn blit_scaled(
    target: &mut [u8],
    target_width: u32,
    origin_x: u32,
    origin_y: u32,
    source: &[u8],
    scale: u32,
) {
    blit_scaled_square(
        target,
        target_width,
        origin_x,
        origin_y,
        source,
        FRAME_SIZE,
        scale,
    );
}

#[allow(clippy::too_many_arguments)]
fn blit_scaled_square(
    target: &mut [u8],
    target_width: u32,
    origin_x: u32,
    origin_y: u32,
    source: &[u8],
    source_size: u32,
    scale: u32,
) {
    for sy in 0..source_size {
        for sx in 0..source_size {
            let source_index = ((sy * source_size + sx) * 4) as usize;
            for oy in 0..scale {
                for ox in 0..scale {
                    let x = origin_x + sx * scale + ox;
                    let y = origin_y + sy * scale + oy;
                    let target_index = ((y * target_width + x) * 4) as usize;
                    target[target_index..target_index + 4]
                        .copy_from_slice(&source[source_index..source_index + 4]);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn blit_scaled_anchor(
    target: &mut [u8],
    target_width: u32,
    target_height: u32,
    anchor_x: u32,
    anchor_y: u32,
    source: &[u8],
    scale: u32,
    clip: Option<(u32, u32, u32, u32)>,
) {
    let size = FRAME_SIZE * scale;
    let origin_x = anchor_x as i32 - size as i32 / 2;
    let origin_y = anchor_y as i32 - size as i32;
    for sy in 0..FRAME_SIZE {
        for sx in 0..FRAME_SIZE {
            let source_index = ((sy * FRAME_SIZE + sx) * 4) as usize;
            let color = [
                source[source_index],
                source[source_index + 1],
                source[source_index + 2],
                source[source_index + 3],
            ];
            if color[3] == 0 {
                continue;
            }
            for oy in 0..scale {
                for ox in 0..scale {
                    let x = origin_x + (sx * scale + ox) as i32;
                    let y = origin_y + (sy * scale + oy) as i32;
                    if x < 0 || y < 0 || x >= target_width as i32 || y >= target_height as i32 {
                        continue;
                    }
                    let x = x as u32;
                    let y = y as u32;
                    if clip.is_some_and(|(cx, cy, width, height)| {
                        x >= cx && x < cx + width && y >= cy && y < cy + height
                    }) {
                        continue;
                    }
                    blend_pixel(target, target_width, x, y, color);
                }
            }
        }
    }
}

pub(crate) fn fill_gradient(
    target: &mut [u8],
    width: u32,
    height: u32,
    top: [u8; 4],
    bottom: [u8; 4],
) {
    for y in 0..height {
        let mix = y as f32 / height.max(1) as f32;
        let color = [
            (top[0] as f32 * (1.0 - mix) + bottom[0] as f32 * mix) as u8,
            (top[1] as f32 * (1.0 - mix) + bottom[1] as f32 * mix) as u8,
            (top[2] as f32 * (1.0 - mix) + bottom[2] as f32 * mix) as u8,
            255,
        ];
        for x in 0..width {
            let index = ((y * width + x) * 4) as usize;
            target[index..index + 4].copy_from_slice(&color);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_rect_alpha(
    target: &mut [u8],
    target_width: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 4],
) {
    let target_height = (target.len() as u32 / 4) / target_width;
    for py in y..y.saturating_add(height).min(target_height) {
        for px in x..x.saturating_add(width).min(target_width) {
            blend_pixel(target, target_width, px, py, color);
        }
    }
}

pub(crate) fn fill_rect(
    target: &mut [u8],
    target_width: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 4],
) {
    for row in y..y + height {
        for column in x..x + width {
            if column >= target_width {
                continue;
            }
            blend_pixel(target, target_width, column, row, color);
        }
    }
}

pub(crate) fn blend_pixel(target: &mut [u8], width: u32, x: u32, y: u32, source: [u8; 4]) {
    let index = ((y * width + x) * 4) as usize;
    if index + 4 > target.len() {
        return;
    }
    let alpha = source[3] as f32 / 255.0;
    for channel in 0..3 {
        target[index + channel] =
            (source[channel] as f32 * alpha + target[index + channel] as f32 * (1.0 - alpha)) as u8;
    }
    target[index + 3] = 255;
}

pub(crate) fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    formiga_art::write_png(BufWriter::new(file), width, height, pixels)?;
    Ok(())
}
