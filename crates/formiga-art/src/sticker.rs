//! Animated transparent stickers: a creature's own clip, exactly as the desktop shows it, on a
//! fully transparent background and small enough to post anywhere.
//!
//! A sticker is rendered on demand and holds nothing afterwards. It carries only pixels: no name,
//! no seed, no memories, and no metadata beyond the standard loop block a GIF needs to repeat.

use crate::{
    AnimationSpec, BodyClip, Canvas, CreatureRenderer, ExpressionKind, EyelidPose, FaceRenderState,
    GazeDirection, MotionSignature, PlaybackMode, Rgba,
};
use formiga_core::{ActionKind, Creature, Gesture};
use std::collections::BTreeMap;

/// The sticker scale the export offers by default: the 48px art box becomes a 384px one, which is
/// large enough for a chat window without asking anyone to zoom.
pub const DEFAULT_STICKER_SCALE: u32 = 8;

/// The two sizes offered. Both are whole numbers, so every art pixel stays a crisp square.
pub const STICKER_SCALES: [u32; 2] = [4, 8];

/// Transparent art pixels kept around the creature on every side, so the sticker has a little air
/// and its edge is never flush with the image boundary.
const STICKER_MARGIN: i32 = 2;

/// Alpha at or above this becomes an opaque sticker pixel; anything softer is dropped. GIF
/// transparency is one bit, so a sticker either keeps a pixel at full strength or leaves it out.
/// Creature art is authored with hard edges, which is what keeps this from showing.
const OPAQUE_AT: u8 = 128;

/// The animation runs for at least this long before it is allowed to stop on a whole loop, so a
/// short clip still reads as a loop rather than a twitch.
const MINIMUM_DURATION_CENTISECONDS: u32 = 140;

/// Whole body loops are never repeated more than this, whatever the clip's own length.
const MAXIMUM_LOOPS: u32 = 4;

/// The clips a sticker can be made from: the handful of moments that show who a creature is.
///
/// Every one of these is a clip the creature already plays on the desktop, with its own frames and
/// its own timing. Nothing is authored here that a companion does not already do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StickerClip {
    /// The walk cycle, the thing a companion does most.
    Walk,
    /// The greeting: the pose a creature meets a friend with.
    #[default]
    Wave,
    /// Both limbs thrown up after something went well.
    Cheer,
    /// Playing on its own with whatever it found.
    Play,
    /// A snack, held in front and eaten in small bites.
    Snack,
    /// Curled up asleep, breathing slowly.
    Sleep,
    /// A side-to-side dance.
    Dance,
}

impl StickerClip {
    pub const ALL: [Self; 7] = [
        Self::Walk,
        Self::Wave,
        Self::Cheer,
        Self::Play,
        Self::Snack,
        Self::Sleep,
        Self::Dance,
    ];

    /// The name shown in the clubhouse.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Walk => "Walk",
            Self::Wave => "Wave",
            Self::Cheer => "Cheer",
            Self::Play => "Play",
            Self::Snack => "Snack",
            Self::Sleep => "Sleep",
            Self::Dance => "Dance",
        }
    }

    /// The lowercase word used in filenames and on the command line.
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Walk => "walk",
            Self::Wave => "wave",
            Self::Cheer => "cheer",
            Self::Play => "play",
            Self::Snack => "snack",
            Self::Sleep => "sleep",
            Self::Dance => "dance",
        }
    }

    /// Parse a clip from its slug, for the command line.
    pub fn from_slug(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|clip| clip.slug().eq_ignore_ascii_case(value))
    }

    /// The body clip the creature actually plays, with the frames and timing it already has.
    pub const fn body_clip(self) -> BodyClip {
        match self {
            Self::Walk => BodyClip::Action(ActionKind::Traverse),
            Self::Wave => BodyClip::Action(ActionKind::Greet),
            Self::Cheer => BodyClip::Gesture(Gesture::Cheer),
            Self::Play => BodyClip::Action(ActionKind::SoloPlay),
            Self::Snack => BodyClip::Action(ActionKind::Eat),
            Self::Sleep => BodyClip::Action(ActionKind::Sleep),
            Self::Dance => BodyClip::Gesture(Gesture::Bop),
        }
    }

    /// The face the desktop wears during this clip.
    const fn expression(self) -> ExpressionKind {
        match self {
            Self::Walk => ExpressionKind::Focused,
            Self::Wave => ExpressionKind::Affectionate,
            Self::Cheer | Self::Play | Self::Dance => ExpressionKind::Joy,
            Self::Snack => ExpressionKind::Content,
            Self::Sleep => ExpressionKind::Sleepy,
        }
    }

    /// Eyes shut for the whole clip, so nothing blinks awake mid-nap.
    const fn eyes_shut(self) -> bool {
        matches!(self, Self::Sleep)
    }
}

/// One drawn sticker frame and how long it is held, in the centiseconds a GIF counts in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StickerFrame {
    canvas: Canvas,
    delay_centiseconds: u16,
}

impl StickerFrame {
    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    pub fn delay_centiseconds(&self) -> u16 {
        self.delay_centiseconds
    }
}

/// A finished sticker: one canvas size for every frame, so nothing jitters as it loops.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sticker {
    width: u32,
    height: u32,
    frames: Vec<StickerFrame>,
}

impl Sticker {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn frames(&self) -> &[StickerFrame] {
        &self.frames
    }

    /// The one GIF encoder the app and the tools both use.
    ///
    /// One global palette for the whole animation, index 0 reserved for transparency, every frame
    /// restored to the background so nothing smears into the next, an infinite loop, and each
    /// frame held for exactly as long as the creature holds it. Nothing is dithered: the palette
    /// either holds the art's own colours or the nearest of the quantised ones.
    pub fn encode_gif(&self) -> std::io::Result<Vec<u8>> {
        let quantiser = Quantiser::build(&self.frames);
        let mut bytes = Vec::new();
        {
            let mut encoder = gif::Encoder::new(
                &mut bytes,
                self.width as u16,
                self.height as u16,
                &quantiser.palette_bytes(),
            )
            .map_err(std::io::Error::other)?;
            encoder
                .set_repeat(gif::Repeat::Infinite)
                .map_err(std::io::Error::other)?;
            for frame in &self.frames {
                let mut written = gif::Frame {
                    width: self.width as u16,
                    height: self.height as u16,
                    delay: frame.delay_centiseconds,
                    // Every frame clears back to transparency first, so a raised limb in one
                    // frame can never be left behind in the next.
                    dispose: gif::DisposalMethod::Background,
                    transparent: Some(TRANSPARENT_INDEX),
                    ..Default::default()
                };
                written.buffer = std::borrow::Cow::Owned(quantiser.indices(&frame.canvas));
                encoder
                    .write_frame(&written)
                    .map_err(std::io::Error::other)?;
            }
        }
        Ok(bytes)
    }
}

/// Stateless by design: a sticker's frames are built, handed over, and forgotten.
pub struct StickerRenderer;

impl StickerRenderer {
    /// Render one creature's clip as a sticker at `scale`.
    ///
    /// The frames are the composited body and layered face the desktop draws, played at the
    /// creature's own cadence, cropped once to a box that holds every frame so the sticker sits
    /// still while it loops.
    pub fn render(creature: &Creature, clip: StickerClip, scale: u32) -> Sticker {
        let scale = scale.clamp(1, 16) as i32;
        let body = clip.body_clip();
        let spec = AnimationSpec::for_clip(body);
        let signature = MotionSignature::for_creature(creature);
        let timeline = timeline(signature, body, spec);

        let faces = face_states(clip, timeline.len());
        let drawn: Vec<(Canvas, u16)> = timeline
            .iter()
            .zip(faces)
            .map(|((body_frame, delay), face)| {
                (
                    CreatureRenderer::render_composited_frame(
                        &creature.appearance,
                        body,
                        *body_frame,
                        true,
                        false,
                        face,
                    ),
                    *delay,
                )
            })
            .collect();

        let (crop_x, crop_y, crop_width, crop_height) = shared_crop(&drawn);
        let frames = drawn
            .iter()
            .map(|(canvas, delay)| StickerFrame {
                canvas: upscale(canvas, crop_x, crop_y, crop_width, crop_height, scale),
                delay_centiseconds: *delay,
            })
            .collect();
        Sticker {
            width: (crop_width * scale) as u32,
            height: (crop_height * scale) as u32,
            frames,
        }
    }
}

/// The body frames this creature plays, in its own order and at its own speed.
///
/// The cadence is read from the creature's `MotionSignature` rather than assumed: its timeline is
/// walked one centisecond at a time — the unit a GIF delay is counted in — and a frame is closed
/// whenever the creature moves on to the next one. The dwell it accumulated becomes that frame's
/// delay, so the sticker plays at the tempo and starts on the phase the desktop would.
fn timeline(signature: MotionSignature, clip: BodyClip, spec: AnimationSpec) -> Vec<(u8, u16)> {
    let wanted = loops(signature, clip, spec) * u32::from(spec.frames.max(1));
    let mut frames: Vec<(u8, u16)> = Vec::with_capacity(wanted as usize);
    let mut current = signature.frame(clip, 0.0);
    let mut dwell: u16 = 0;
    // A generous ceiling on the walk: the slowest clip holds a frame for well under a second.
    for step in 1..=20_000_u32 {
        dwell = dwell.saturating_add(1);
        let next = signature.frame(clip, step as f32 / 100.0);
        if next == current {
            continue;
        }
        frames.push((current, dwell));
        current = next;
        dwell = 0;
        if frames.len() as u32 >= wanted {
            return frames;
        }
    }
    // A clip with a single frame, or one held still: one frame, shown for a beat.
    if frames.is_empty() {
        frames.push((current, 50));
    }
    frames
}

/// How many whole body loops the sticker plays: enough to read as a loop, never more than a few.
fn loops(signature: MotionSignature, clip: BodyClip, spec: AnimationSpec) -> u32 {
    if spec.playback == PlaybackMode::Hold || spec.frames <= 1 {
        return 1;
    }
    // One loop's length, measured the same way the timeline is walked.
    let mut seen = 0_u32;
    let mut current = signature.frame(clip, 0.0);
    let mut centiseconds = 0_u32;
    for step in 1..=20_000_u32 {
        let next = signature.frame(clip, step as f32 / 100.0);
        if next == current {
            continue;
        }
        current = next;
        seen += 1;
        if seen == u32::from(spec.frames) {
            centiseconds = step;
            break;
        }
    }
    if centiseconds == 0 {
        return 1;
    }
    MINIMUM_DURATION_CENTISECONDS
        .div_ceil(centiseconds)
        .clamp(1, MAXIMUM_LOOPS)
}

/// The face worn over each frame: the clip's own expression, plus one blink near the end so the
/// sticker looks alive rather than looped. A sleeping creature keeps its eyes shut throughout.
fn face_states(clip: StickerClip, frames: usize) -> Vec<FaceRenderState> {
    let expression = clip.expression();
    let shut = clip.eyes_shut();
    // Away from the seam, so the blink never lands half on each end of the loop.
    let blink = (frames >= 6).then(|| frames - 4);
    (0..frames)
        .map(|index| FaceRenderState {
            expression,
            eyelids: match blink {
                _ if shut => EyelidPose::Closed,
                Some(at) if index == at + 1 => EyelidPose::Closed,
                Some(at) if index == at || index == at + 2 => EyelidPose::Half,
                _ => EyelidPose::Open,
            },
            gaze: GazeDirection::new(0, 0),
        })
        .collect()
}

/// One crop box that holds every frame, with a small transparent margin on each side. Cropping
/// each frame to its own pixels would make the creature hop around inside the sticker.
fn shared_crop(frames: &[(Canvas, u16)]) -> (i32, i32, i32, i32) {
    let bounds = frames
        .iter()
        .filter_map(|(canvas, _)| opaque_bounds(canvas))
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)));
    let Some((min_x, min_y, max_x, max_y)) = bounds else {
        return (0, 0, 1, 1);
    };
    (
        min_x - STICKER_MARGIN,
        min_y - STICKER_MARGIN,
        max_x - min_x + 1 + STICKER_MARGIN * 2,
        max_y - min_y + 1 + STICKER_MARGIN * 2,
    )
}

/// Bounds of the pixels a sticker will actually keep, which is not quite the canvas's own alpha
/// bounds: a soft pixel that will be dropped must not stretch the crop box.
fn opaque_bounds(canvas: &Canvas) -> Option<(i32, i32, i32, i32)> {
    let mut bounds: Option<(i32, i32, i32, i32)> = None;
    for y in 0..canvas.height() as i32 {
        for x in 0..canvas.width() as i32 {
            if canvas.get(x, y).a < OPAQUE_AT {
                continue;
            }
            bounds = Some(match bounds {
                None => (x, y, x, y),
                Some((min_x, min_y, max_x, max_y)) => {
                    (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
                }
            });
        }
    }
    bounds
}

/// Nearest-neighbour upscale of one crop box. Every kept pixel becomes a square of flat colour at
/// full alpha, and everything else stays transparent, so the sticker has a hard pixel edge and no
/// halo of half-solid pixels around it.
fn upscale(source: &Canvas, x: i32, y: i32, width: i32, height: i32, scale: i32) -> Canvas {
    let mut canvas = Canvas::new((width * scale) as u32, (height * scale) as u32);
    for row in 0..height {
        for column in 0..width {
            let pixel = source.get(x + column, y + row);
            if pixel.a < OPAQUE_AT {
                continue;
            }
            canvas.fill_rect(
                column * scale,
                row * scale,
                scale,
                scale,
                Rgba::new(pixel.r, pixel.g, pixel.b, 255),
            );
        }
    }
    canvas
}

/// One opaque art colour, and one art colour with how often the animation uses it.
type Rgb = (u8, u8, u8);
type ColourCount = (Rgb, u64);

/// Index 0 of the global palette is always the transparent one.
const TRANSPARENT_INDEX: u8 = 0;

/// The most colours a GIF global palette can hold once transparency has taken index 0.
const MAXIMUM_OPAQUE_COLOURS: usize = 255;

/// The animation's one global palette, and the map from art colour to palette index.
///
/// Creature art uses very few colours, so almost every sticker is stored exactly. A sticker that
/// somehow carries more than the palette can hold is reduced by median cut rather than refused:
/// boxes are split along their widest channel until there is room, and each colour then takes the
/// nearest surviving entry. Colours are visited in a fixed order throughout, so the same creature
/// and clip always produce the same bytes.
struct Quantiser {
    entries: Vec<Rgb>,
    lookup: BTreeMap<Rgb, u8>,
}

impl Quantiser {
    fn build(frames: &[StickerFrame]) -> Self {
        let mut counts: BTreeMap<Rgb, u64> = BTreeMap::new();
        for frame in frames {
            for pixel in frame.canvas.pixels() {
                if pixel.a < OPAQUE_AT {
                    continue;
                }
                *counts.entry((pixel.r, pixel.g, pixel.b)).or_default() += 1;
            }
        }
        let colours: Vec<ColourCount> = counts.into_iter().collect();
        let entries: Vec<Rgb> = if colours.len() <= MAXIMUM_OPAQUE_COLOURS {
            colours.iter().map(|(colour, _)| *colour).collect()
        } else {
            median_cut(&colours, MAXIMUM_OPAQUE_COLOURS)
        };
        let lookup = colours
            .iter()
            .map(|(colour, _)| (*colour, nearest(&entries, *colour)))
            .collect();
        Self { entries, lookup }
    }

    /// The global colour table, transparent entry first.
    fn palette_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0, 0, 0];
        for (r, g, b) in &self.entries {
            bytes.extend_from_slice(&[*r, *g, *b]);
        }
        bytes
    }

    fn indices(&self, canvas: &Canvas) -> Vec<u8> {
        canvas
            .pixels()
            .iter()
            .map(|pixel| {
                if pixel.a < OPAQUE_AT {
                    return TRANSPARENT_INDEX;
                }
                self.lookup
                    .get(&(pixel.r, pixel.g, pixel.b))
                    .copied()
                    .unwrap_or(TRANSPARENT_INDEX)
            })
            .collect()
    }
}

fn nearest(entries: &[Rgb], colour: Rgb) -> u8 {
    let distance = |entry: &Rgb| {
        let d = |a: u8, b: u8| {
            let delta = i32::from(a) - i32::from(b);
            delta * delta
        };
        d(entry.0, colour.0) + d(entry.1, colour.1) + d(entry.2, colour.2)
    };
    let index = entries
        .iter()
        .enumerate()
        .min_by_key(|(index, entry)| (distance(entry), *index))
        .map_or(0, |(index, _)| index);
    // Index 0 belongs to transparency, so the opaque entries start at one.
    (index + 1).min(usize::from(u8::MAX)) as u8
}

fn median_cut(colours: &[ColourCount], target: usize) -> Vec<Rgb> {
    let mut boxes: Vec<Vec<ColourCount>> = vec![colours.to_vec()];
    while boxes.len() < target {
        let Some(index) = boxes
            .iter()
            .enumerate()
            .filter(|(_, group)| group.len() > 1)
            .max_by_key(|(index, group)| (spread(group), group.len(), std::cmp::Reverse(*index)))
            .map(|(index, _)| index)
        else {
            break;
        };
        let mut group = boxes.swap_remove(index);
        let channel = widest_channel(&group);
        group.sort_by_key(|(colour, _)| (channel_of(*colour, channel), *colour));
        let rest = group.split_off(group.len() / 2);
        boxes.push(group);
        boxes.push(rest);
    }
    let mut entries: Vec<Rgb> = boxes.iter().map(|group| average(group)).collect();
    entries.sort_unstable();
    entries.dedup();
    entries
}

fn channel_of(colour: Rgb, channel: usize) -> u8 {
    match channel {
        0 => colour.0,
        1 => colour.1,
        _ => colour.2,
    }
}

fn widest_channel(group: &[ColourCount]) -> usize {
    (0..3)
        .max_by_key(|channel| {
            let values = group
                .iter()
                .map(|(colour, _)| channel_of(*colour, *channel));
            let high = values.clone().max().unwrap_or(0);
            let low = values.min().unwrap_or(0);
            (
                u32::from(high) - u32::from(low),
                std::cmp::Reverse(*channel),
            )
        })
        .unwrap_or(0)
}

fn spread(group: &[ColourCount]) -> u32 {
    (0..3)
        .map(|channel| {
            let values = group.iter().map(|(colour, _)| channel_of(*colour, channel));
            let high = values.clone().max().unwrap_or(0);
            let low = values.min().unwrap_or(0);
            u32::from(high) - u32::from(low)
        })
        .max()
        .unwrap_or(0)
}

fn average(group: &[ColourCount]) -> Rgb {
    let weight: u64 = group.iter().map(|(_, count)| *count).sum::<u64>().max(1);
    let sum = |channel: usize| {
        group
            .iter()
            .map(|(colour, count)| u64::from(channel_of(*colour, channel)) * count)
            .sum::<u64>()
    };
    (
        (sum(0) / weight) as u8,
        (sum(1) / weight) as u8,
        (sum(2) / weight) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::{DesktopRect, DesktopSnapshot, DisplayKey, MonitorInfo, World};
    use time::macros::datetime;

    fn creature() -> Creature {
        let desktop = DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: DisplayKey([1; 16]),
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1440.0,
                    height: 900.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 1440.0,
                    height: 876.0,
                },
                scale_factor: 1.0,
                primary: true,
            }],
            ..DesktopSnapshot::default()
        };
        World::new([7; 32], datetime!(2026-08-14 12:30 UTC), &desktop)
            .save
            .creatures
            .remove(0)
    }

    /// Width, height, delay, and RGBA pixels of one decoded frame.
    type DecodedFrame = (u16, u16, u16, Vec<u8>);

    fn decode(bytes: &[u8]) -> (gif::Repeat, Vec<DecodedFrame>) {
        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);
        let mut decoder = options
            .read_info(std::io::Cursor::new(bytes.to_vec()))
            .unwrap();
        let repeat = decoder.repeat();
        let mut frames = Vec::new();
        while let Some(frame) = decoder.read_next_frame().unwrap() {
            frames.push((
                frame.width,
                frame.height,
                frame.delay,
                frame.buffer.to_vec(),
            ));
        }
        (repeat, frames)
    }

    #[test]
    fn every_clip_is_a_stable_transparent_loop_at_both_offered_scales() {
        let creature = creature();
        for clip in StickerClip::ALL {
            for scale in STICKER_SCALES {
                let sticker = StickerRenderer::render(&creature, clip, scale);
                assert!(
                    !sticker.frames().is_empty(),
                    "{clip:?} at {scale}x should animate"
                );
                let (width, height) = (sticker.width(), sticker.height());
                assert!(width > 0 && height > 0);
                assert_eq!(width % scale, 0);
                assert_eq!(height % scale, 0);
                for frame in sticker.frames() {
                    // One canvas for every frame: the sticker never jitters as it loops.
                    assert_eq!(
                        (frame.canvas().width(), frame.canvas().height()),
                        (width, height)
                    );
                    // A hard pixel edge, never a halo of part-solid pixels.
                    assert!(
                        frame
                            .canvas()
                            .pixels()
                            .iter()
                            .all(|p| p.a == 0 || p.a == 255)
                    );
                    // The margin: the corners are always clear.
                    assert_eq!(frame.canvas().get(0, 0).a, 0);
                    assert_eq!(frame.canvas().get(width as i32 - 1, height as i32 - 1).a, 0);
                    assert!(
                        frame.delay_centiseconds() >= 2,
                        "{clip:?} delay is too short"
                    );
                }
                assert!(
                    sticker.frames().iter().any(|frame| frame
                        .canvas()
                        .pixels()
                        .iter()
                        .any(|pixel| pixel.a == 255)),
                    "{clip:?} should draw a creature"
                );
            }
        }
    }

    #[test]
    fn encoding_is_byte_deterministic_and_decodes_as_a_transparent_infinite_loop() {
        let creature = creature();
        let sticker = StickerRenderer::render(&creature, StickerClip::Wave, DEFAULT_STICKER_SCALE);
        let first = sticker.encode_gif().unwrap();
        let second = StickerRenderer::render(&creature, StickerClip::Wave, DEFAULT_STICKER_SCALE)
            .encode_gif()
            .unwrap();
        assert_eq!(first, second);

        let (repeat, frames) = decode(&first);
        assert_eq!(repeat, gif::Repeat::Infinite);
        assert_eq!(frames.len(), sticker.frames().len());
        for (index, (width, height, delay, pixels)) in frames.iter().enumerate() {
            assert_eq!(
                (u32::from(*width), u32::from(*height)),
                (sticker.width(), sticker.height())
            );
            assert_eq!(*delay, sticker.frames()[index].delay_centiseconds());
            // The top-left corner is inside the sticker's own margin, so it has to be clear in
            // every frame: a frame that smeared into the next would show up right here.
            assert_eq!(pixels[3], 0, "frame {index} corner is not transparent");
        }
    }

    #[test]
    fn frames_carry_the_creatures_own_cadence_rather_than_one_shared_delay() {
        let creature = creature();
        let walk = StickerRenderer::render(&creature, StickerClip::Walk, 4);
        let sleep = StickerRenderer::render(&creature, StickerClip::Sleep, 4);
        let longest = |sticker: &Sticker| {
            sticker
                .frames()
                .iter()
                .map(StickerFrame::delay_centiseconds)
                .max()
                .unwrap()
        };
        // Sleeping is the slowest clip there is and walking among the fastest.
        assert!(longest(&sleep) > longest(&walk) * 2);
    }

    #[test]
    fn a_blink_happens_once_in_a_waking_clip_and_never_in_a_sleeping_one() {
        let waking = face_states(StickerClip::Wave, 12);
        assert_eq!(
            waking
                .iter()
                .filter(|state| state.eyelids == EyelidPose::Closed)
                .count(),
            1
        );
        assert!(
            face_states(StickerClip::Sleep, 4)
                .iter()
                .all(|state| state.eyelids == EyelidPose::Closed)
        );
    }

    #[test]
    fn more_colours_than_a_palette_holds_are_quantised_rather_than_refused() {
        let mut canvas = Canvas::new(64, 64);
        for y in 0..64 {
            for x in 0..64 {
                canvas.set(x, y, Rgba::new((x * 4) as u8, (y * 4) as u8, 200, 255));
            }
        }
        let sticker = Sticker {
            width: 64,
            height: 64,
            frames: vec![StickerFrame {
                canvas,
                delay_centiseconds: 10,
            }],
        };
        let bytes = sticker.encode_gif().unwrap();
        assert_eq!(bytes, sticker.encode_gif().unwrap());
        let (_, frames) = decode(&bytes);
        assert_eq!(frames.len(), 1);
        assert_eq!((frames[0].0, frames[0].1), (64, 64));
    }

    #[test]
    fn a_sticker_carries_only_pixels_and_the_loop_block_it_needs() {
        for clip in StickerClip::ALL {
            let bytes = StickerRenderer::render(&creature(), clip, 4)
                .encode_gif()
                .unwrap();
            assert!(bytes.starts_with(b"GIF89a"));
            // Walked block by block rather than scanned for byte pairs: compressed pixels happen
            // to contain every two-byte sequence sooner or later, so only the real structure can
            // say whether a sticker is carrying anything but its own frames.
            let blocks = extension_blocks(&bytes);
            assert_eq!(
                blocks
                    .iter()
                    .filter(|(label, _)| *label == 0xFF)
                    .map(|(_, body)| &body[..11.min(body.len())])
                    .collect::<Vec<_>>(),
                vec![&b"NETSCAPE2.0"[..]],
                "{clip:?} carries an application block that is not the loop block"
            );
            // No comment (0xFE), plain-text (0x01), or any other extension: only the loop block
            // and one graphic control block per frame, which is where a delay has to live.
            for (label, _) in &blocks {
                assert!(
                    matches!(label, 0xFF | 0xF9),
                    "{clip:?} carries extension block {label:#04x}"
                );
            }
        }
    }

    /// Every extension block in a GIF stream, as `(label, body)`, by walking the format's own
    /// block structure: header, logical screen descriptor, optional global colour table, then
    /// extensions and images until the trailer.
    fn extension_blocks(bytes: &[u8]) -> Vec<(u8, Vec<u8>)> {
        let packed = bytes[10];
        let mut at = 13;
        if packed & 0x80 != 0 {
            at += 3 * (1 << ((packed & 0x07) + 1));
        }
        let sub_blocks = |bytes: &[u8], at: &mut usize| {
            let mut body = Vec::new();
            while bytes[*at] != 0 {
                let length = bytes[*at] as usize;
                body.extend_from_slice(&bytes[*at + 1..*at + 1 + length]);
                *at += 1 + length;
            }
            *at += 1;
            body
        };
        let mut blocks = Vec::new();
        loop {
            match bytes[at] {
                0x3B => return blocks,
                0x21 => {
                    let label = bytes[at + 1];
                    at += 2;
                    blocks.push((label, sub_blocks(bytes, &mut at)));
                }
                0x2C => {
                    let packed = bytes[at + 9];
                    at += 10;
                    if packed & 0x80 != 0 {
                        at += 3 * (1 << ((packed & 0x07) + 1));
                    }
                    at += 1; // LZW minimum code size
                    sub_blocks(bytes, &mut at);
                }
                other => panic!("unexpected GIF block {other:#04x} at {at}"),
            }
        }
    }

    #[test]
    fn renderer_holds_no_persistent_export_state() {
        assert_eq!(std::mem::size_of::<StickerRenderer>(), 0);
    }

    #[test]
    fn clip_slugs_round_trip_and_stay_distinct() {
        for clip in StickerClip::ALL {
            assert_eq!(StickerClip::from_slug(clip.slug()), Some(clip));
            assert_eq!(
                StickerClip::from_slug(&clip.slug().to_uppercase()),
                Some(clip)
            );
        }
        assert_eq!(StickerClip::from_slug("nap"), None);
    }
}
