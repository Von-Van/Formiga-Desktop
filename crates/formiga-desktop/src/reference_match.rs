use anyhow::{Context as _, Result, bail};
use formiga_art::{Canvas, CreatureRenderer};
use formiga_core::{
    ActionKind, BodyPlan, Creature, CreatureDesign, DesktopSnapshot, EarStyle, SeedStream, World,
    apply_creature_design,
};
use image::{DynamicImage, GenericImageView as _, ImageFormat, ImageReader, Limits};
use std::fs;
use std::io::Cursor;
use std::path::Path;
use time::OffsetDateTime;

pub const MAX_REFERENCE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_REFERENCE_DIMENSION: u32 = 4_096;
pub const MAX_REFERENCE_PIXELS: u64 = 16_000_000;
pub const MATCH_CANDIDATES: u64 = 512;
const ANALYSIS_SIZE: u32 = 64;

#[derive(Clone, Debug)]
pub struct ReferenceMatch {
    pub creature: Creature,
    pub source_seed: [u8; 32],
    pub similarity: u8,
    pub summary: &'static str,
}

pub fn match_reference_file(
    path: &Path,
    search_seed: [u8; 32],
    now: OffsetDateTime,
    desktop: &DesktopSnapshot,
) -> Result<ReferenceMatch> {
    let metadata = fs::metadata(path).context("read reference image information")?;
    if metadata.len() > MAX_REFERENCE_BYTES {
        bail!("reference image is larger than 16 MB");
    }
    let bytes = fs::read(path).context("read reference image")?;
    let reference = decode_reference_bytes(&bytes)?;
    Ok(match_reference_image(&reference, search_seed, now, desktop))
}

fn decode_reference_bytes(bytes: &[u8]) -> Result<DynamicImage> {
    if bytes.len() as u64 > MAX_REFERENCE_BYTES {
        bail!("reference image is larger than 16 MB");
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .context("recognize reference image")?;
    if !matches!(reader.format(), Some(ImageFormat::Png | ImageFormat::Jpeg)) {
        bail!("choose a PNG or JPEG image");
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_REFERENCE_DIMENSION);
    limits.max_image_height = Some(MAX_REFERENCE_DIMENSION);
    limits.max_alloc = Some(MAX_REFERENCE_PIXELS * 4);
    reader.limits(limits);
    let image = reader.decode().context("decode reference image")?;
    let (width, height) = image.dimensions();
    if u64::from(width) * u64::from(height) > MAX_REFERENCE_PIXELS {
        bail!("reference image contains too many pixels");
    }
    Ok(image)
}

fn match_reference_image(
    reference: &DynamicImage,
    search_seed: [u8; 32],
    now: OffsetDateTime,
    desktop: &DesktopSnapshot,
) -> ReferenceMatch {
    let target = features_from_image(reference);
    let streams = SeedStream::new(search_seed);
    let mut best: Option<(f32, [u8; 32], Creature)> = None;
    for ordinal in 0..MATCH_CANDIDATES {
        let seed = streams.bytes("reference-candidate", ordinal);
        let mut creature = World::preview_adult(seed, now, desktop);
        let mut design = CreatureDesign::generated(seed, 0, None);
        design.coat = target.coat;
        design.accent = target.accent;
        // Reference cues guide safe parts, never trace arbitrary source geometry. Leave a
        // quarter of candidates exploratory so unusual images still find a friendly shape.
        if ordinal % 4 != 0 {
            design.body = if target.aspect > 1.3 {
                BodyPlan::Long
            } else if target.aspect < 0.8 {
                BodyPlan::Upright
            } else if target.side_extensions > 0.18 {
                BodyPlan::Winged
            } else {
                BodyPlan::Round
            };
            design.width = (target.aspect * 10.0).round().clamp(8.0, 12.0) as u8;
            design.height = (10.0 / target.aspect.max(0.1)).round().clamp(7.0, 11.0) as u8;
            if target.upper_extensions > 0.08 {
                design.ears = if ordinal % 2 == 0 {
                    EarStyle::Pointed
                } else {
                    EarStyle::Long
                };
            }
            if target.color_variation < 0.08 {
                design.marking = 0;
            }
        }
        apply_creature_design(&mut creature, Some(design));
        let frame = CreatureRenderer::render_frame(&creature.appearance, ActionKind::Idle, 0, true);
        let candidate = features_from_canvas(&frame);
        let score = feature_distance(target, candidate);
        if best
            .as_ref()
            .is_none_or(|(best_score, _, _)| score < *best_score)
        {
            best = Some((score, seed, creature));
        }
    }
    let (distance, source_seed, creature) = best.expect("the bounded search is non-empty");
    let similarity = ((1.0 - distance.clamp(0.0, 1.0)) * 100.0).round() as u8;
    ReferenceMatch {
        creature,
        source_seed,
        similarity,
        summary: "Cute reinterpretation of colors, proportions, and appendage cues—not object recognition. Try another preview for a different interpretation.",
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ImageFeatures {
    color: [f32; 3],
    coat: [u8; 3],
    accent: [u8; 3],
    color_variation: f32,
    aspect: f32,
    occupancy: f32,
    symmetry: f32,
    upper_extensions: f32,
    lower_extensions: f32,
    side_extensions: f32,
}

fn features_from_image(image: &DynamicImage) -> ImageFeatures {
    let resized = image.thumbnail(ANALYSIS_SIZE, ANALYSIS_SIZE).to_rgba8();
    let has_transparency = resized.pixels().any(|p| p[3] < 24);
    features_from_rgba(
        resized.as_raw(),
        resized.width(),
        resized.height(),
        has_transparency,
    )
}

fn features_from_canvas(canvas: &Canvas) -> ImageFeatures {
    features_from_rgba(&canvas.rgba_bytes(), canvas.width(), canvas.height(), true)
}

fn features_from_rgba(
    bytes: &[u8],
    width: u32,
    height: u32,
    transparent_is_background: bool,
) -> ImageFeatures {
    let mut border = [0_u64; 3];
    let mut border_count = 0_u64;
    for y in 0..height {
        for x in 0..width {
            if x != 0 && y != 0 && x + 1 != width && y + 1 != height {
                continue;
            }
            let offset = ((y * width + x) * 4) as usize;
            if bytes[offset + 3] < 24 {
                continue;
            }
            border[0] += u64::from(bytes[offset]);
            border[1] += u64::from(bytes[offset + 1]);
            border[2] += u64::from(bytes[offset + 2]);
            border_count += 1;
        }
    }
    let background = if border_count == 0 {
        [255.0; 3]
    } else {
        [
            border[0] as f32 / border_count as f32,
            border[1] as f32 / border_count as f32,
            border[2] as f32 / border_count as f32,
        ]
    };
    let mut mask = vec![false; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let offset = ((y * width + x) * 4) as usize;
            let alpha = bytes[offset + 3];
            let dr = f32::from(bytes[offset]) - background[0];
            let dg = f32::from(bytes[offset + 1]) - background[1];
            let db = f32::from(bytes[offset + 2]) - background[2];
            let background_distance = (dr * dr + dg * dg + db * db).sqrt();
            mask[(y * width + x) as usize] = if transparent_is_background {
                alpha > 24
            } else {
                alpha > 32 && background_distance > 34.0
            };
        }
    }
    if mask.iter().filter(|value| **value).count() < (width * height / 100) as usize {
        for y in 0..height {
            for x in 0..width {
                let offset = ((y * width + x) * 4) as usize;
                mask[(y * width + x) as usize] = bytes[offset + 3] > 32;
            }
        }
    }
    summarize_mask(bytes, width, height, &mask)
}

fn summarize_mask(bytes: &[u8], width: u32, height: u32, mask: &[bool]) -> ImageFeatures {
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut count = 0_u32;
    let mut color = [0_u64; 3];
    // 512 fixed bins retain recognizable dominant/accent colors instead of averaging a
    // multicolored subject into gray. Analysis is at most 64 by 64 pixels.
    let mut bins = [(0_u32, [0_u64; 3]); 512];
    for y in 0..height {
        for x in 0..width {
            if !mask[(y * width + x) as usize] {
                continue;
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            count += 1;
            let offset = ((y * width + x) * 4) as usize;
            color[0] += u64::from(bytes[offset]);
            color[1] += u64::from(bytes[offset + 1]);
            color[2] += u64::from(bytes[offset + 2]);
            let bin = ((bytes[offset] as usize / 32) << 6)
                | ((bytes[offset + 1] as usize / 32) << 3)
                | (bytes[offset + 2] as usize / 32);
            bins[bin].0 += 1;
            for channel in 0..3 {
                bins[bin].1[channel] += u64::from(bytes[offset + channel]);
            }
        }
    }
    if count == 0 {
        return ImageFeatures::default();
    }
    let box_width = max_x.saturating_sub(min_x).saturating_add(1).max(1);
    let box_height = max_y.saturating_sub(min_y).saturating_add(1).max(1);
    let center_x = (min_x + max_x) / 2;
    let mut mirrored = 0_u32;
    let mut considered = 0_u32;
    let mut upper = 0_u32;
    let mut lower = 0_u32;
    let mut sides = 0_u32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if !mask[(y * width + x) as usize] {
                continue;
            }
            let mirror_x = center_x.saturating_mul(2).saturating_sub(x);
            if mirror_x < width {
                considered += 1;
                mirrored += u32::from(mask[(y * width + mirror_x) as usize]);
            }
            let relative_x = (x - min_x) as f32 / box_width as f32;
            let relative_y = (y - min_y) as f32 / box_height as f32;
            upper += u32::from(relative_y < 0.25 && !(0.25..=0.75).contains(&relative_x));
            lower += u32::from(relative_y > 0.72);
            sides += u32::from(!(0.12..=0.88).contains(&relative_x));
        }
    }
    bins.sort_by_key(|bin| std::cmp::Reverse(bin.0));
    let bin_color = |bin: &(u32, [u64; 3])| bin.1.map(|v| (v / u64::from(bin.0.max(1))) as u8);
    let coat = bin_color(&bins[0]);
    let accent_bin = bins
        .iter()
        .filter(|bin| bin.0 >= (count / 50).max(1))
        .find(|bin| {
            bin_color(bin)
                .iter()
                .zip(coat)
                .map(|(a, b)| (f32::from(*a) - f32::from(b)).powi(2))
                .sum::<f32>()
                > 3600.0
        });
    let accent = accent_bin.map_or(coat.map(|v| v.saturating_add(35)), bin_color);
    ImageFeatures {
        coat,
        accent,
        color_variation: 1.0 - bins[0].0 as f32 / count as f32,
        color: [
            color[0] as f32 / count as f32 / 255.0,
            color[1] as f32 / count as f32 / 255.0,
            color[2] as f32 / count as f32 / 255.0,
        ],
        aspect: box_width as f32 / box_height as f32,
        occupancy: count as f32 / (box_width * box_height) as f32,
        symmetry: mirrored as f32 / considered.max(1) as f32,
        upper_extensions: upper as f32 / count as f32,
        lower_extensions: lower as f32 / count as f32,
        side_extensions: sides as f32 / count as f32,
    }
}

fn feature_distance(target: ImageFeatures, candidate: ImageFeatures) -> f32 {
    let color = ((target.color[0] - candidate.color[0]).powi(2)
        + (target.color[1] - candidate.color[1]).powi(2)
        + (target.color[2] - candidate.color[2]).powi(2))
    .sqrt()
        / 3.0_f32.sqrt();
    0.34 * color
        + 0.19 * ((target.aspect - candidate.aspect).abs() / 2.0).min(1.0)
        + 0.14 * (target.occupancy - candidate.occupancy).abs()
        + 0.11 * (target.symmetry - candidate.symmetry).abs()
        + 0.09 * (target.upper_extensions - candidate.upper_extensions).abs()
        + 0.07 * (target.lower_extensions - candidate.lower_extensions).abs()
        + 0.06 * (target.side_extensions - candidate.side_extensions).abs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::{DesktopRect, DisplayKey, MonitorInfo};
    use image::{ImageBuffer, Rgba};
    use time::macros::datetime;

    fn desktop() -> DesktopSnapshot {
        DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: DisplayKey([1; 16]),
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1280.0,
                    height: 800.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 1280.0,
                    height: 776.0,
                },
                scale_factor: 1.0,
                primary: true,
            }],
            ..DesktopSnapshot::default()
        }
    }

    fn reference_png() -> Vec<u8> {
        let mut image = ImageBuffer::from_pixel(96, 96, Rgba([248, 248, 248, 255]));
        for y in 22..76 {
            for x in 15..82 {
                if ((x as i32 - 48).pow(2) / 34_i32.pow(2) + (y as i32 - 49).pow(2) / 27_i32.pow(2))
                    <= 1
                {
                    image.put_pixel(x, y, Rgba([82, 164, 117, 255]));
                }
            }
        }
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    #[test]
    fn png_reference_match_is_bounded_deterministic_and_full_size() {
        let decoded = decode_reference_bytes(&reference_png()).unwrap();
        let first = match_reference_image(
            &decoded,
            [9; 32],
            datetime!(2026-09-02 1:00 UTC),
            &desktop(),
        );
        let second = match_reference_image(
            &decoded,
            [9; 32],
            datetime!(2026-09-02 1:00 UTC),
            &desktop(),
        );
        assert_eq!(first.source_seed, second.source_seed);
        assert_eq!(first.creature.appearance, second.creature.appearance);
        assert_eq!(first.creature.display_scale_percent, 100);
        assert!(first.creature.role.is_adult());
        assert!(first.similarity <= 100);
    }

    #[test]
    fn analysis_preserves_aspect_transparency_and_two_subject_colors() {
        let mut image = ImageBuffer::from_pixel(160, 80, Rgba([0, 0, 0, 0]));
        for y in 20..60 {
            for x in 20..140 {
                image.put_pixel(
                    x,
                    y,
                    if x < 110 {
                        Rgba([220, 70, 40, 255])
                    } else {
                        Rgba([30, 90, 210, 255])
                    },
                );
            }
        }
        let features = features_from_image(&DynamicImage::ImageRgba8(image));
        assert!((features.aspect - 3.0).abs() < 0.3);
        assert!(features.coat[0] > 200 && features.coat[2] < 60);
        assert!(features.accent[2] > 190 && features.accent[0] < 50);
    }

    #[test]
    fn blank_dark_and_unusual_images_still_produce_bounded_cute_designs() {
        for (width, height, color) in [
            (1, 1, [0, 0, 0, 0]),
            (320, 12, [2, 2, 2, 255]),
            (12, 320, [255, 255, 255, 255]),
        ] {
            let input =
                DynamicImage::ImageRgba8(ImageBuffer::from_pixel(width, height, Rgba(color)));
            let matched =
                match_reference_image(&input, [21; 32], OffsetDateTime::UNIX_EPOCH, &desktop());
            let d = matched.creature.appearance.design.unwrap();
            assert_eq!(d, d.bounded());
            assert_eq!(Some(d), matched.creature.origin.design);
            assert!(
                CreatureRenderer::render_frame(
                    &matched.creature.appearance,
                    ActionKind::Idle,
                    0,
                    true
                )
                .alpha_bounds()
                .is_some()
            );
            let shared = formiga_core::decode_creature_seed(&formiga_core::encode_creature_seed(
                matched.creature.origin,
            ))
            .unwrap();
            let imported =
                World::from_shared_creature(shared, OffsetDateTime::UNIX_EPOCH, &desktop());
            assert_eq!(
                imported.save.creatures[0].appearance,
                matched.creature.appearance
            );
        }
    }

    #[test]
    fn unsupported_and_oversized_inputs_are_rejected_before_matching() {
        assert!(decode_reference_bytes(b"not an image").is_err());
        let oversized = vec![0; MAX_REFERENCE_BYTES as usize + 1];
        assert!(decode_reference_bytes(&oversized).is_err());
    }

    #[test]
    fn jpeg_reference_is_accepted_without_retaining_source_pixels() {
        let png = decode_reference_bytes(&reference_png()).unwrap();
        let mut bytes = Cursor::new(Vec::new());
        png.write_to(&mut bytes, ImageFormat::Jpeg).unwrap();
        let decoded = decode_reference_bytes(&bytes.into_inner()).unwrap();
        let matched = match_reference_image(
            &decoded,
            [10; 32],
            datetime!(2026-09-02 1:00 UTC),
            &desktop(),
        );
        assert!(matched.creature.role.is_adult());
        assert_eq!(matched.creature.display_scale_percent, 100);
        assert!(matched.similarity <= 100);
    }
}
