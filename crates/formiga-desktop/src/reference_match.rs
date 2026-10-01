use anyhow::{Context as _, Result, bail};
use formiga_art::{Canvas, CreatureRenderer};
use formiga_core::to_hsl as hsl;
use formiga_core::{
    ActionKind, BodyArchetype, BodyPlan, Creature, CreatureDesign, DesktopSnapshot, DetailParts,
    EarStyle, SeedStream, World, apply_creature_design,
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
/// How many different takes on one picture a search offers.
pub const TAKES: usize = 4;
const ANALYSIS_SIZE: u32 = 64;

#[derive(Clone, Debug)]
pub struct ReferenceMatch {
    pub creature: Creature,
    pub source_seed: [u8; 32],
    pub similarity: u8,
    pub summary: &'static str,
}

/// Four takes on the picture at `path`: the closest, the cutest of the close ones, the closest on
/// another body, and a wildcard in the picture's colours.
pub fn match_reference_file(
    path: &Path,
    search_seed: [u8; 32],
    now: OffsetDateTime,
    desktop: &DesktopSnapshot,
) -> Result<Vec<ReferenceMatch>> {
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

/// One candidate from the search: how far it reads from the picture, and whether the picture's
/// cues shaped it or it was left to explore.
struct Candidate {
    distance: f32,
    seed: [u8; 32],
    creature: Creature,
    guided: bool,
}

fn match_reference_image(
    reference: &DynamicImage,
    search_seed: [u8; 32],
    now: OffsetDateTime,
    desktop: &DesktopSnapshot,
) -> Vec<ReferenceMatch> {
    let target = features_from_image(reference);
    let streams = SeedStream::new(search_seed);
    let mut candidates: Vec<Candidate> = (0..MATCH_CANDIDATES)
        .map(|ordinal| {
            let seed = streams.bytes("reference-candidate", ordinal);
            let mut creature = World::preview_adult(seed, now, desktop);
            // Reference cues guide safe parts, never trace arbitrary source geometry. A quarter
            // of candidates are left exploratory so an unusual picture still finds a friendly
            // shape, and so there is always a wildcard to offer.
            let guided = ordinal % 4 != 0;
            let design = if guided {
                guided_design(&target, seed, ordinal)
            } else {
                let mut design = CreatureDesign::generated(seed, 0, None);
                design.coat = target.coat;
                design.accent = target.accent;
                design.bounded()
            };
            apply_creature_design(&mut creature, Some(design));
            let frame =
                CreatureRenderer::render_frame(&creature.appearance, ActionKind::Idle, 0, true);
            let distance = feature_distance(&target, &features_from_canvas(&frame));
            Candidate {
                distance,
                seed,
                creature,
                guided,
            }
        })
        .collect();
    candidates.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    takes(&candidates)
}

/// The four takes, each a different candidate, in the order they are shown. The first three
/// come from candidates the picture shaped, so each carries what was read from it; the wildcard
/// comes from those left to explore.
fn takes(candidates: &[Candidate]) -> Vec<ReferenceMatch> {
    let design = |index: usize| {
        candidates[index]
            .creature
            .appearance
            .design
            .expect("every candidate has a recipe")
    };
    let guided: Vec<usize> = (0..candidates.len())
        .filter(|index| candidates[*index].guided)
        .collect();
    let mut chosen: Vec<(usize, &'static str)> = Vec::with_capacity(TAKES);
    let free = |chosen: &[(usize, &str)], index: usize| chosen.iter().all(|(c, _)| *c != index);
    if let Some(&closest) = guided.first() {
        chosen.push((
            closest,
            "Closest to your picture: its colors in the same places, and the parts that read \
             most like it.",
        ));
        // Of the close takes, the one the generator would find cutest.
        if let Some(&cutest) = guided
            .iter()
            .take(24)
            .filter(|index| free(&chosen, **index))
            .max_by(|a, b| design(**a).coherence().total_cmp(&design(**b).coherence()))
        {
            chosen.push((cutest, "The cutest of the close takes."));
        }
        let body = design(closest).body;
        if let Some(&other) = guided
            .iter()
            .find(|index| free(&chosen, **index) && design(**index).body != body)
        {
            chosen.push((other, "The same colors and details on a different body."));
        }
    }
    if let Some(wild) =
        (0..candidates.len()).find(|index| free(&chosen, *index) && !candidates[*index].guided)
    {
        chosen.push((
            wild,
            "A wildcard: your picture's colors on a companion all its own.",
        ));
    }
    // Only a search with almost no candidates leaves a place unfilled; fill it with the next best.
    for index in 0..candidates.len() {
        if chosen.len() >= TAKES {
            break;
        }
        if free(&chosen, index) {
            chosen.push((index, "Another close take."));
        }
    }
    chosen
        .into_iter()
        .map(|(index, summary)| {
            let candidate = &candidates[index];
            ReferenceMatch {
                creature: candidate.creature.clone(),
                source_seed: candidate.seed,
                similarity: ((1.0 - candidate.distance.clamp(0.0, 1.0)) * 100.0).round() as u8,
                summary,
            }
        })
        .collect()
}

/// A recipe shaped by the picture's cues: a body that suits its proportions, its colours where
/// the picture has them, and the details it reads as having. The parts are always the generator's
/// own; only which of them, and in what colours, comes from the picture.
fn guided_design(target: &ImageFeatures, seed: [u8; 32], ordinal: u64) -> CreatureDesign {
    let cues = target.cues;
    let mut design = CreatureDesign::generated(seed, 0, None);
    design.coat = target.coat;
    design.accent = target.accent;
    // Bodies the picture could be, tried in turn so each gets an even share of the search.
    let winged_picture = cues.wings.is_some() || target.side_extensions > 0.18;
    let bodies: &[BodyPlan] = if target.aspect > 1.3 {
        &[
            BodyPlan::Long,
            BodyPlan::Long,
            BodyPlan::Round,
            BodyPlan::Blob,
        ]
    } else if cues.wings.is_some() {
        &[
            BodyPlan::Upright,
            BodyPlan::Long,
            BodyPlan::Round,
            BodyPlan::Winged,
        ]
    } else if target.aspect < 0.8 && winged_picture {
        &[
            BodyPlan::Upright,
            BodyPlan::Upright,
            BodyPlan::Round,
            BodyPlan::Winged,
        ]
    } else if target.aspect < 0.8 {
        // A tall picture with nothing at its sides that reads as a wing has no winged body.
        &[BodyPlan::Upright, BodyPlan::Upright, BodyPlan::Round]
    } else if winged_picture {
        &[BodyPlan::Winged, BodyPlan::Round, BodyPlan::Upright]
    } else if target.upper_extensions < 0.05 && target.side_extensions < 0.08 {
        // Smooth, compact subjects with no read appendages become blobs.
        &[BodyPlan::Blob, BodyPlan::Round]
    } else {
        &[BodyPlan::Round, BodyPlan::Upright, BodyPlan::Long]
    };
    let turn = ordinal / 4;
    design.body = bodies[turn as usize % bodies.len()];
    let lean = ordinal.is_multiple_of(2);
    design.width = (target.aspect * 10.0).round().clamp(8.0, 12.0) as u8;
    design.height = (10.0 / target.aspect.max(0.1)).round().clamp(7.0, 11.0) as u8;
    // Narrow points over the head are pointed ears, unless the picture already reads as a dragon,
    // with wings or a tip; even then a picture cannot say which they are, so half the candidates
    // try ears.
    let horned = cues.horns && (cues.wings.is_some() || cues.tip.is_some()) && lean;
    if horned {
        // Horns read best on their own, with no ears beside them.
        design.ears = EarStyle::None;
    } else if cues.horns {
        design.ears = EarStyle::Pointed;
    } else if target.upper_extensions > 0.08 {
        design.ears = if lean {
            EarStyle::Pointed
        } else {
            EarStyle::Long
        };
    }
    if target.color_variation < 0.08 || cues.belly.is_some() {
        design.marking = 0;
    }
    design.classic.crown = 0;
    if cues.tip.is_some() {
        design.classic.tail = 0;
        design.tail = if lean { 2 } else { 3 };
    }
    // A body the picture chose belongs to an archetype of its own: a dragonish picture on four
    // legs is a whelp.
    let dragonish = cues.wings.is_some() || cues.horns || cues.tip.is_some();
    if let Some(archetype) = design.body_archetype()
        && !archetype.allows(design.body)
    {
        design.archetype = BodyArchetype::for_body(design.body, lean || dragonish).number();
    }
    design.details = DetailParts {
        wings: u8::from(cues.wings.is_some()) * if turn % 3 == 2 { 2 } else { 1 },
        horns: u8::from(horned) * if turn.is_multiple_of(2) { 1 } else { 2 },
        belly: u8::from(cues.belly.is_some()),
        tip: match cues.tip {
            // A warm tip reads as a flame, any other as a bobble.
            Some(color) if warm(color) => 1,
            Some(_) => 2,
            None => 0,
        },
        belly_color: cues.belly.unwrap_or_default(),
        // A tip is small, so it is drawn brighter than the picture has it to stand out from the
        // coat beside it.
        tip_color: cues.tip.map(vivid).unwrap_or_default(),
    };
    design.bounded()
}

fn vivid(rgb: [u8; 3]) -> [u8; 3] {
    let (hue, saturation, lightness) = hsl(rgb);
    formiga_core::hsl(hue, saturation.max(0.9), lightness.clamp(0.55, 0.62))
}

fn warm(rgb: [u8; 3]) -> bool {
    let (hue, saturation, _) = hsl(rgb);
    saturation > 0.45 && !(65.0..=330.0).contains(&hue)
}

/// What the picture's colours say about its parts, read only from a picture.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Cues {
    /// A lighter colour down the lower middle.
    belly: Option<[u8; 3]>,
    /// A colour of their own out at both sides.
    wings: Option<[u8; 3]>,
    /// A small, saturated patch of its own colour out at an end.
    tip: Option<[u8; 3]>,
    /// Narrow points above the head: horns, or pointed ears.
    horns: bool,
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
    /// The mean colour of each ninth of the subject's box, row by row, and the share of the
    /// subject in it, so colours are compared where they are and not only overall.
    grid: [[f32; 3]; 9],
    grid_share: [f32; 9],
    cues: Cues,
}

fn features_from_image(image: &DynamicImage) -> ImageFeatures {
    let resized = image.thumbnail(ANALYSIS_SIZE, ANALYSIS_SIZE).to_rgba8();
    let (width, height) = (resized.width(), resized.height());
    let bytes = resized.as_raw();
    let mask = subject_mask(bytes, width, height);
    let mut features = summarize_mask(bytes, width, height, &mask);
    features.cues = read_cues(bytes, width, height, &mask, &mut features);
    features
}

fn features_from_canvas(canvas: &Canvas) -> ImageFeatures {
    let bytes = canvas.rgba_bytes();
    let mask: Vec<bool> = bytes.chunks_exact(4).map(|pixel| pixel[3] > 24).collect();
    summarize_mask(&bytes, canvas.width(), canvas.height(), &mask)
}

fn distance(a: [u8; 3], b: [u8; 3]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(a, b)| (f32::from(*a) - f32::from(b)).powi(2))
        .sum::<f32>()
        .sqrt()
}

/// Which pixels are the subject. A picture with transparency says so itself. An opaque one is
/// read from its edges: the commonest colour along them is the background, and everything joined
/// to the edges in about that colour is background too. Taking the commonest edge colour rather
/// than the average keeps a subject that touches the edge — a wingtip, a tail — from tinting the
/// background into something the subject's own pale parts then match; filling in from the edges
/// keeps those pale parts, a white belly or the shine in an eye, inside the subject.
fn subject_mask(bytes: &[u8], width: u32, height: u32) -> Vec<bool> {
    let index = |x: u32, y: u32| (y * width + x) as usize;
    let pixel = |i: usize| -> [u8; 3] { [bytes[i * 4], bytes[i * 4 + 1], bytes[i * 4 + 2]] };
    let alpha = |i: usize| bytes[i * 4 + 3];
    let alpha_mask: Vec<bool> = (0..(width * height) as usize)
        .map(|i| alpha(i) > 24)
        .collect();
    if alpha_mask.iter().any(|opaque| !opaque) {
        return alpha_mask;
    }
    let border: Vec<usize> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .filter(|&(x, y)| x == 0 || y == 0 || x + 1 == width || y + 1 == height)
        .map(|(x, y)| index(x, y))
        .collect();
    let bin = |rgb: [u8; 3]| {
        (usize::from(rgb[0] / 32) << 6) | (usize::from(rgb[1] / 32) << 3) | usize::from(rgb[2] / 32)
    };
    let mut counts = [0_u32; 512];
    for &i in &border {
        counts[bin(pixel(i))] += 1;
    }
    let common = (0..512).max_by_key(|b| counts[*b]).unwrap_or(0);
    let mut sum = [0_u32; 3];
    let mut n = 0_u32;
    for &i in &border {
        if bin(pixel(i)) == common {
            for (total, channel) in sum.iter_mut().zip(pixel(i)) {
                *total += u32::from(channel);
            }
            n += 1;
        }
    }
    let background = sum.map(|total| (total / n.max(1)) as u8);
    let mut is_background = vec![false; (width * height) as usize];
    let mut stack: Vec<(u32, u32)> = Vec::new();
    let near = |i: usize| distance(pixel(i), background) < 48.0;
    for y in 0..height {
        for x in 0..width {
            if (x == 0 || y == 0 || x + 1 == width || y + 1 == height) && near(index(x, y)) {
                is_background[index(x, y)] = true;
                stack.push((x, y));
            }
        }
    }
    while let Some((x, y)) = stack.pop() {
        let neighbours = [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ];
        for (nx, ny) in neighbours {
            if nx >= width || ny >= height {
                continue;
            }
            let i = index(nx, ny);
            if !is_background[i] && near(i) {
                is_background[i] = true;
                stack.push((nx, ny));
            }
        }
    }
    let mask: Vec<bool> = is_background.iter().map(|background| !background).collect();
    // A picture that is all one colour has no subject to find; read the whole of it.
    if mask.iter().filter(|value| **value).count() < (width * height / 100) as usize {
        return alpha_mask;
    }
    mask
}

/// Read where the picture's colours are: a belly, wings, a tip, and horns. Near-black pixels are
/// outline ink and say nothing about colour.
fn read_cues(
    bytes: &[u8],
    width: u32,
    height: u32,
    mask: &[bool],
    features: &mut ImageFeatures,
) -> Cues {
    let Some((min_x, min_y, max_x, max_y)) = bounds(mask, width, height) else {
        return Cues::default();
    };
    let (box_w, box_h) = ((max_x - min_x + 1) as f32, (max_y - min_y + 1) as f32);
    let pixel = |x: u32, y: u32| {
        let o = ((y * width + x) * 4) as usize;
        [bytes[o], bytes[o + 1], bytes[o + 2]]
    };
    let ink = |rgb: [u8; 3]| hsl(rgb).2 < 0.14;
    // The dominant colour of the subject where `place` holds, leaving out anything near `besides`,
    // with its share of everything in that place.
    let dominant_besides =
        |place: &dyn Fn(f32, f32) -> bool, besides: Option<[u8; 3]>| -> Option<([u8; 3], f32)> {
            let mut bins = vec![(0_u32, [0_u32; 3]); 512];
            let mut total = 0_u32;
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    if !mask[(y * width + x) as usize] {
                        continue;
                    }
                    let (rx, ry) = ((x - min_x) as f32 / box_w, (y - min_y) as f32 / box_h);
                    let rgb = pixel(x, y);
                    if !place(rx, ry) || ink(rgb) {
                        continue;
                    }
                    total += 1;
                    if besides.is_some_and(|other| distance(rgb, other) < 45.0) {
                        continue;
                    }
                    // A second colour is read in coarser bins: a shaded wing or belly spreads over
                    // several fine ones, none of which would stand out alone.
                    let step = if besides.is_some() { 64 } else { 32 };
                    let b = (usize::from(rgb[0] / step) << 6)
                        | (usize::from(rgb[1] / step) << 3)
                        | usize::from(rgb[2] / step);
                    bins[b].0 += 1;
                    for (sum, channel) in bins[b].1.iter_mut().zip(rgb) {
                        *sum += u32::from(channel);
                    }
                }
            }
            let (count, sum) = bins.into_iter().max_by_key(|bin| bin.0)?;
            (count > 0).then(|| {
                (
                    sum.map(|channel| (channel / count) as u8),
                    count as f32 / total.max(1) as f32,
                )
            })
        };
    let dominant = |place: &dyn Fn(f32, f32) -> bool| dominant_besides(place, None);
    // The coat is the commonest colour down the middle, where the body is; wings or a busy
    // background prop out at the sides can cover more of the picture than the body does.
    let Some((coat, _)) =
        dominant(&|x, _| (0.3..0.7).contains(&x)).or_else(|| dominant(&|_, _| true))
    else {
        return Cues::default();
    };
    features.coat = coat;
    let lightness = |rgb: [u8; 3]| hsl(rgb).2;
    // A belly is the commonest colour other than the coat down the lower middle, and lighter.
    let belly = dominant_besides(
        &|x, y| (0.3..0.7).contains(&x) && (0.45..0.95).contains(&y),
        Some(coat),
    )
    .filter(|(color, share)| {
        *share > 0.15 && distance(*color, coat) > 50.0 && lightness(*color) > lightness(coat) + 0.06
    })
    .map(|(color, _)| color);
    let wing_place = |x: f32, y: f32| !(0.25..=0.75).contains(&x) && y < 0.75;
    let mut wing_pixels = 0_u32;
    let mut subject_pixels = 0_u32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if mask[(y * width + x) as usize] {
                subject_pixels += 1;
                wing_pixels += u32::from(wing_place(
                    (x - min_x) as f32 / box_w,
                    (y - min_y) as f32 / box_h,
                ));
            }
        }
    }
    // Wings are the same colour high up on both sides; legs and a head out to one side are not.
    let high_on_both_sides = |color: [u8; 3]| {
        let mut sides = [(0_u32, 0.0_f32); 2];
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let (rx, ry) = ((x - min_x) as f32 / box_w, (y - min_y) as f32 / box_h);
                if !mask[(y * width + x) as usize]
                    || !wing_place(rx, ry)
                    || distance(pixel(x, y), color) > 60.0
                {
                    continue;
                }
                let side = &mut sides[usize::from(rx > 0.5)];
                side.0 += 1;
                side.1 += ry;
            }
        }
        sides
            .iter()
            .all(|(count, rows)| *count >= 3 && rows / (*count as f32) < 0.6)
    };
    let wings = dominant_besides(&wing_place, Some(coat))
        .filter(|(color, share)| {
            *share > 0.2
                && wing_pixels as f32 > subject_pixels as f32 * 0.08
                && distance(*color, coat) > 60.0
                && belly.is_none_or(|belly| distance(*color, belly) > 50.0)
                && high_on_both_sides(*color)
        })
        .map(|(color, _)| color);
    // A tip: a small patch of one saturated colour of its own, far out from the middle.
    let mut tip = None;
    {
        let mut bins = vec![(0_u32, [0_u32; 3], 0.0_f32, 0.0_f32); 512];
        let (cx, cy) = ((min_x + max_x) as f32 / 2.0, (min_y + max_y) as f32 / 2.0);
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let rgb = pixel(x, y);
                if !mask[(y * width + x) as usize] || ink(rgb) {
                    continue;
                }
                let b = (usize::from(rgb[0] / 32) << 6)
                    | (usize::from(rgb[1] / 32) << 3)
                    | usize::from(rgb[2] / 32);
                let bin = &mut bins[b];
                bin.0 += 1;
                for (sum, channel) in bin.1.iter_mut().zip(rgb) {
                    *sum += u32::from(channel);
                }
                bin.2 += (x as f32 - cx) / (box_w / 2.0);
                bin.3 += (y as f32 - cy) / (box_h / 2.0);
            }
        }
        let low = (subject_pixels as f32 * 0.003).max(2.0);
        let high = subject_pixels as f32 * 0.08;
        let mut best = 0;
        for (count, sum, dx, dy) in bins {
            if count < best || (count as f32) < low || count as f32 > high {
                continue;
            }
            let color = sum.map(|channel| (channel / count) as u8);
            let (mx, my) = (dx / count as f32, dy / count as f32);
            let out = (mx * mx + my * my).sqrt();
            let own = distance(color, coat) > 45.0
                && wings.is_none_or(|wings| distance(color, wings) > 45.0)
                && belly.is_none_or(|belly| distance(color, belly) > 45.0);
            // A tail goes out to a side or up; a patch below the middle, like a pair of feet,
            // is not a tip.
            let tail_like = mx.abs() > 0.45 || my < -0.45;
            if hsl(color).1 > 0.55 && own && out > 0.55 && tail_like {
                best = count;
                tip = Some(color);
            }
        }
    }
    // Horns: two or more narrow points across the top of the middle of the subject.
    let top = min_y + ((box_h * 0.18) as u32).max(1);
    let mut points = 0;
    for y in min_y..top {
        let mut run = 0;
        let mut row_points = 0;
        for x in min_x..=max_x + 1 {
            let inside = x <= max_x
                && mask[(y * width + x) as usize]
                && (0.2..0.8).contains(&((x - min_x) as f32 / box_w));
            if inside {
                run += 1;
            } else {
                if (1..=3).contains(&run) {
                    row_points += 1;
                }
                run = 0;
            }
        }
        points = points.max(row_points);
    }
    let horns = points >= 2;
    if let Some(wings) = wings {
        features.accent = wings;
    }
    Cues {
        belly,
        wings,
        tip,
        horns,
    }
}

fn bounds(mask: &[bool], width: u32, height: u32) -> Option<(u32, u32, u32, u32)> {
    let mut found = None;
    for y in 0..height {
        for x in 0..width {
            if mask[(y * width + x) as usize] {
                let (a, b, c, d) = found.unwrap_or((x, y, x, y));
                found = Some((a.min(x), b.min(y), c.max(x), d.max(y)));
            }
        }
    }
    found
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
    let mut grid_sum = [[0_u64; 3]; 9];
    let mut grid_count = [0_u32; 9];
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
            let cell =
                ((relative_y * 3.0) as usize).min(2) * 3 + ((relative_x * 3.0) as usize).min(2);
            let offset = ((y * width + x) * 4) as usize;
            for channel in 0..3 {
                grid_sum[cell][channel] += u64::from(bytes[offset + channel]);
            }
            grid_count[cell] += 1;
        }
    }
    let grid = std::array::from_fn(|cell| {
        grid_sum[cell].map(|sum| sum as f32 / grid_count[cell].max(1) as f32 / 255.0)
    });
    let grid_share = grid_count.map(|n| n as f32 / count as f32);
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
        grid,
        grid_share,
        cues: Cues::default(),
    }
}

fn feature_distance(target: &ImageFeatures, candidate: &ImageFeatures) -> f32 {
    let color = ((target.color[0] - candidate.color[0]).powi(2)
        + (target.color[1] - candidate.color[1]).powi(2)
        + (target.color[2] - candidate.color[2]).powi(2))
    .sqrt()
        / 3.0_f32.sqrt();
    // Colours where they are: each ninth of the box compared where both have the subject, and a
    // ninth only one of them fills counted by how much it holds.
    let mut grid = 0.0;
    let mut weight = 0.0;
    for cell in 0..9 {
        let (a, b) = (target.grid_share[cell], candidate.grid_share[cell]);
        let w = a.max(b);
        if w < 0.01 {
            continue;
        }
        let d = if a > 0.02 && b > 0.02 {
            (0..3)
                .map(|channel| (target.grid[cell][channel] - candidate.grid[cell][channel]).powi(2))
                .sum::<f32>()
                .sqrt()
                / 3.0_f32.sqrt()
        } else {
            0.6
        };
        grid += d * w;
        weight += w;
    }
    let grid = if weight > 0.0 { grid / weight } else { 0.0 };
    0.2 * color
        + 0.26 * grid
        + 0.15 * ((target.aspect - candidate.aspect).abs() / 2.0).min(1.0)
        + 0.1 * (target.occupancy - candidate.occupancy).abs()
        + 0.08 * (target.symmetry - candidate.symmetry).abs()
        + 0.08 * (target.upper_extensions - candidate.upper_extensions).abs()
        + 0.06 * (target.lower_extensions - candidate.lower_extensions).abs()
        + 0.07 * (target.side_extensions - candidate.side_extensions).abs()
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
        assert_eq!(first.len(), TAKES);
        let seeds: std::collections::HashSet<_> = first.iter().map(|t| t.source_seed).collect();
        assert_eq!(seeds.len(), TAKES, "four different takes");
        for (a, b) in first.iter().zip(&second) {
            assert_eq!(a.source_seed, b.source_seed);
            assert_eq!(a.creature.appearance, b.creature.appearance);
            assert_eq!(a.creature.display_scale_percent, 100);
            assert!(a.creature.role.is_adult());
            assert!(a.similarity <= 100);
        }
        assert_ne!(
            first[2].creature.appearance.design.unwrap().body,
            first[0].creature.appearance.design.unwrap().body,
            "the third take is on another body"
        );
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
            let takes =
                match_reference_image(&input, [21; 32], OffsetDateTime::UNIX_EPOCH, &desktop());
            assert_eq!(takes.len(), TAKES);
            for matched in takes {
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
                let shared = formiga_core::decode_creature_seed(
                    &formiga_core::encode_creature_seed(matched.creature.origin),
                )
                .unwrap();
                let imported =
                    World::from_shared_creature(shared, OffsetDateTime::UNIX_EPOCH, &desktop());
                assert_eq!(
                    imported.save.creatures[0].appearance,
                    matched.creature.appearance
                );
            }
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
        let takes = match_reference_image(
            &decoded,
            [10; 32],
            datetime!(2026-09-02 1:00 UTC),
            &desktop(),
        );
        assert_eq!(takes.len(), TAKES);
        for matched in takes {
            assert!(matched.creature.role.is_adult());
            assert_eq!(matched.creature.display_scale_percent, 100);
            assert!(matched.similarity <= 100);
        }
    }

    /// An orange dragonish figure on white, its wings out to both edges of the picture: a cream
    /// belly, teal wings, and a yellow flame at the end of its tail.
    fn dragon_on_white() -> DynamicImage {
        let mut image = ImageBuffer::from_pixel(120, 110, Rgba([255, 255, 255, 255]));
        let mut put = |x: i32, y: i32, color: [u8; 3]| {
            if (0..120).contains(&x) && (0..110).contains(&y) {
                image.put_pixel(
                    x as u32,
                    y as u32,
                    Rgba([color[0], color[1], color[2], 255]),
                );
            }
        };
        let ellipse = |x: i32, y: i32, cx: i32, cy: i32, rx: i32, ry: i32| {
            (x - cx).pow(2) * ry.pow(2) + (y - cy).pow(2) * rx.pow(2) <= rx.pow(2) * ry.pow(2)
        };
        for y in 0..110 {
            for x in 0..120 {
                // Wings out to the very edges, behind everything.
                if ellipse(x, y, 14, 38, 16, 26) || ellipse(x, y, 105, 38, 16, 26) {
                    put(x, y, [60, 140, 150]);
                }
                // Horns, two narrow points over the head.
                if (2..14).contains(&y) && ((x - 50).abs() <= 1 || (x - 70).abs() <= 1) {
                    put(x, y, [240, 230, 200]);
                }
                if ellipse(x, y, 60, 28, 16, 15) || ellipse(x, y, 60, 70, 26, 32) {
                    put(x, y, [237, 132, 44]);
                }
                if ellipse(x, y, 60, 76, 15, 22) {
                    put(x, y, [250, 228, 160]);
                }
                if ellipse(x, y, 98, 92, 5, 7) {
                    put(x, y, [255, 214, 40]);
                }
            }
        }
        DynamicImage::ImageRgba8(image)
    }

    #[test]
    fn a_subject_touching_the_edges_keeps_the_background_out_of_its_colors() {
        let features = features_from_image(&dragon_on_white());
        assert!(
            distance(features.coat, [237, 132, 44]) < 30.0,
            "coat {:?}",
            features.coat
        );
        assert!(features.occupancy < 0.9, "white is not the subject");
    }

    #[test]
    fn a_picture_with_a_belly_wings_horns_and_a_flame_reads_as_having_them() {
        let image = dragon_on_white();
        let cues = features_from_image(&image).cues;
        assert!(
            cues.belly
                .is_some_and(|c| distance(c, [250, 228, 160]) < 30.0),
            "{cues:?}"
        );
        assert!(
            cues.wings
                .is_some_and(|c| distance(c, [60, 140, 150]) < 30.0),
            "{cues:?}"
        );
        assert!(
            cues.tip.is_some_and(|c| distance(c, [255, 214, 40]) < 40.0),
            "{cues:?}"
        );
        assert!(cues.horns, "{cues:?}");
        let takes =
            match_reference_image(&image, [3; 32], datetime!(2026-10-01 1:00 UTC), &desktop());
        let closest = takes[0].creature.appearance.design.unwrap();
        let details = closest.details;
        assert!(
            [
                details.wings > 0 || closest.body == BodyPlan::Winged,
                details.horns > 0,
                details.belly > 0,
                details.tip > 0
            ]
            .iter()
            .filter(|read| **read)
            .count()
                >= 3,
            "the closest take carries most of what the picture has: {closest:?}"
        );
    }
}
