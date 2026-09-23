//! Postcards: the colony together in a little scene — a nap, a picnic, a game, or the village at
//! dusk — drawn from its own art on an illustrated background, with a caption if one is given.
//!
//! A postcard shows only what the colony portrait may: rendered pixels, the scene's name, and a
//! month. It carries no names, seeds, memories, scores, journal text, visitors, or anything about
//! the desktop. The caption is the one thing on it the sender wrote, and it is written as typed,
//! less control characters and runs of spaces.

use crate::card::{
    CREAM, CardRng, CardText, INK, MUTED_INK, PAPER, PAPER_SHADOW, blend, draw_background,
    month_label, stepped_panel,
};
use crate::colony_card::{LotArt, cell_height, colony_palette, village_lots};
use crate::{
    AnimationSpec, BodyClip, COLONY_OBJECT_SIZE, Canvas, ColonyObjectRenderer, CreatureRenderer,
    ExpressionKind, EyelidPose, FaceRenderState, GazeDirection, Palette, Rgba, SHELTER_SIZE,
    ShelterRenderer, VillageCell,
};
use formiga_core::{ActionKind, Creature, Gesture, SaveFile};

pub const POSTCARD_WIDTH: u32 = 960;
pub const POSTCARD_HEIGHT: u32 = 600;

/// The longest caption a postcard carries, in characters: one line in the card's hand.
pub const POSTCARD_CAPTION_LIMIT: usize = 60;

/// Where the picture sits on the card, inside its frame.
const PICTURE_X: i32 = 64;
const PICTURE_Y: i32 = 57;
const PICTURE_WIDTH: i32 = 832;
const PICTURE_HEIGHT: i32 = 396;

/// Air kept clear at each end of the colony so nobody is pressed against the frame.
const PADDING: i32 = 36;

/// The least clear space between two neighbours.
const GAP: i32 = 14;

/// A scene the colony can be sent in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PostcardScene {
    /// Everyone asleep together on a quilt, one golden afternoon.
    #[default]
    Nap,
    /// Everyone round a picnic blanket on a bright day, with a snack or a cup.
    Picnic,
    /// A game out on the grass, a ball in the air and a kite overhead.
    Play,
    /// The houses lit up at sundown, and everyone waving goodnight.
    Dusk,
}

impl PostcardScene {
    pub const ALL: [Self; 4] = [Self::Nap, Self::Picnic, Self::Play, Self::Dusk];

    /// The name the Home page offers it by.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Nap => "A nap",
            Self::Picnic => "A picnic",
            Self::Play => "Playtime",
            Self::Dusk => "The village at dusk",
        }
    }

    /// The lowercase word used in filenames and on the command line.
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Nap => "nap",
            Self::Picnic => "picnic",
            Self::Play => "play",
            Self::Dusk => "dusk",
        }
    }

    /// Parse a scene from its slug, for the command line.
    pub fn from_slug(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|scene| scene.slug().eq_ignore_ascii_case(value))
    }

    /// What the card says about the picture, under it.
    const fn title(self) -> &'static str {
        match self {
            Self::Nap => "A NAP AT HOME",
            Self::Picnic => "A PICNIC BETWEEN THE HOUSES",
            Self::Play => "PLAYTIME ON THE GRASS",
            Self::Dusk => "THE VILLAGE AT DUSK",
        }
    }

    fn light(self) -> Light {
        match self {
            Self::Nap => GOLDEN_AFTERNOON,
            Self::Picnic | Self::Play => BRIGHT_DAY,
            Self::Dusk => SUNDOWN,
        }
    }
}

const GOLDEN_AFTERNOON: Light = Light {
    sky: &[
        Rgba::new(236, 184, 150, 255),
        Rgba::new(244, 204, 162, 255),
        Rgba::new(250, 222, 180, 255),
        Rgba::new(253, 236, 202, 255),
    ],
    horizon: 176,
    far_hills: Rgba::new(184, 196, 132, 255),
    near_hills: Rgba::new(156, 182, 112, 255),
    ground: &[
        Rgba::new(142, 178, 100, 255),
        Rgba::new(128, 166, 92, 255),
        Rgba::new(114, 152, 84, 255),
    ],
    tuft: Rgba::new(98, 138, 74, 255),
    haze: Rgba::new(222, 214, 170, 255),
    haze_amount: 96,
};

const BRIGHT_DAY: Light = Light {
    sky: &[
        Rgba::new(118, 184, 228, 255),
        Rgba::new(146, 200, 234, 255),
        Rgba::new(176, 216, 238, 255),
        Rgba::new(206, 232, 242, 255),
    ],
    horizon: 184,
    far_hills: Rgba::new(142, 198, 150, 255),
    near_hills: Rgba::new(114, 180, 124, 255),
    ground: &[
        Rgba::new(120, 186, 106, 255),
        Rgba::new(106, 172, 96, 255),
        Rgba::new(94, 158, 88, 255),
    ],
    tuft: Rgba::new(78, 142, 78, 255),
    haze: Rgba::new(190, 222, 226, 255),
    haze_amount: 92,
};

const SUNDOWN: Light = Light {
    sky: &[
        Rgba::new(46, 42, 94, 255),
        Rgba::new(84, 60, 128, 255),
        Rgba::new(150, 82, 136, 255),
        Rgba::new(218, 118, 120, 255),
        Rgba::new(248, 168, 112, 255),
    ],
    horizon: 214,
    far_hills: Rgba::new(92, 64, 110, 255),
    near_hills: Rgba::new(66, 52, 92, 255),
    ground: &[
        Rgba::new(58, 58, 86, 255),
        Rgba::new(48, 50, 76, 255),
        Rgba::new(40, 42, 66, 255),
    ],
    tuft: Rgba::new(34, 38, 60, 255),
    haze: Rgba::new(46, 38, 78, 255),
    haze_amount: 120,
};

/// How a scene is lit: its sky from the top down to the horizon, the hills along it, its ground
/// from there to the bottom, and what the far village fades toward.
struct Light {
    sky: &'static [Rgba],
    horizon: i32,
    far_hills: Rgba,
    near_hills: Rgba,
    ground: &'static [Rgba],
    tuft: Rgba,
    haze: Rgba,
    haze_amount: u8,
}

/// A caption made safe to write on a card: control characters dropped, every run of whitespace
/// folded to one space, the ends trimmed, and the rest cut to `POSTCARD_CAPTION_LIMIT` characters.
pub fn postcard_caption(raw: &str) -> String {
    let folded = raw
        .split_whitespace()
        .map(|word| word.chars().filter(|c| !c.is_control()).collect::<String>())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    folded.chars().take(POSTCARD_CAPTION_LIMIT).collect()
}

/// Stateless by design: every allocation a postcard makes is dropped when the export finishes.
pub struct PostcardRenderer;

impl PostcardRenderer {
    /// Render one postcard of the whole colony in `scene`, with `caption` written under it if it
    /// says anything.
    pub fn render(save: &SaveFile, scene: PostcardScene, caption: &str) -> Canvas {
        let mut canvas = Canvas::new(POSTCARD_WIDTH, POSTCARD_HEIGHT);
        let palette = colony_palette(save);
        let mut rng = CardRng::new(save.home.shelter.detail_seed ^ scene_salt(scene));
        draw_background(&mut canvas, &mut rng, palette.accent);
        stepped_panel(&mut canvas, 24, 22, 912, 556, PAPER_SHADOW);
        stepped_panel(&mut canvas, 31, 29, 898, 542, PAPER);
        stepped_panel(
            &mut canvas,
            PICTURE_X - 7,
            PICTURE_Y - 7,
            PICTURE_WIDTH + 14,
            PICTURE_HEIGHT + 14,
            palette.outline,
        );

        let picture = paint_picture(save, scene, palette, &mut rng);
        for y in 0..PICTURE_HEIGHT {
            for x in 0..PICTURE_WIDTH {
                canvas.set(PICTURE_X + x, PICTURE_Y + y, picture.get(x, y));
            }
        }

        let mut text = CardText::new();
        let month = format!(
            "{} {}",
            month_label(save.maximum_seen_utc.month()),
            save.maximum_seen_utc.year()
        );
        draw_stamp(&mut canvas, &mut text, save, palette, &month, scene);
        let caption = postcard_caption(caption);
        let line = format!("{}  ·  {}", scene.title(), month.to_uppercase());
        if caption.is_empty() {
            text.draw(&mut canvas, 80, 478, scene.title(), 24.0, INK);
            text.draw(&mut canvas, 82, 515, &month.to_uppercase(), 14.0, MUTED_INK);
        } else {
            let room = (POSTCARD_WIDTH as i32 - 160) as f32;
            let size = text.fit_size(&caption, room, 30.0, 18.0);
            let caption = text.truncate_to_width(&caption, size, room);
            text.draw(&mut canvas, 80, 472, &caption, size, INK);
            text.draw(&mut canvas, 82, 518, &line, 14.0, MUTED_INK);
        }
        canvas
    }
}

/// Each scene scatters its own stars, tufts, and fireflies, so four postcards of one colony are
/// four pictures rather than one sky with different people in front of it.
const fn scene_salt(scene: PostcardScene) -> u64 {
    match scene {
        PostcardScene::Nap => 0x4E41_5000,
        PostcardScene::Picnic => 0x5049_4300,
        PostcardScene::Play => 0x504C_4100,
        PostcardScene::Dusk => 0x4455_5300,
    }
}

/// The picture itself, drawn on a canvas of its own so nothing in it can run over the frame.
fn paint_picture(
    save: &SaveFile,
    scene: PostcardScene,
    palette: Palette,
    rng: &mut CardRng,
) -> Canvas {
    let light = scene.light();
    let mut picture = Canvas::new(PICTURE_WIDTH as u32, PICTURE_HEIGHT as u32);
    paint_sky(&mut picture, &light);
    match scene {
        PostcardScene::Nap => {
            paint_sun(&mut picture, 606, 92, 34, Rgba::new(255, 244, 214, 255));
            paint_clouds(&mut picture, rng, 3, Rgba::new(255, 240, 222, 255));
        }
        PostcardScene::Picnic | PostcardScene::Play => {
            paint_sun(&mut picture, 118, 70, 26, Rgba::new(255, 240, 170, 255));
            paint_clouds(&mut picture, rng, 4, Rgba::new(250, 252, 255, 255));
        }
        PostcardScene::Dusk => {
            paint_stars(&mut picture, rng, light.horizon - 90);
            paint_moon(&mut picture, 150, 62);
            paint_sun(
                &mut picture,
                560,
                light.horizon,
                42,
                Rgba::new(255, 206, 128, 255),
            );
        }
    }
    paint_hills(&mut picture, rng, &light);
    paint_ground(&mut picture, rng, &light);
    let village_ground = light.horizon + 26;
    paint_village(
        &mut picture,
        save,
        village_ground,
        if scene == PostcardScene::Dusk {
            118
        } else {
            72
        },
        scene == PostcardScene::Dusk,
        &light,
    );
    let members = pose_members(save, scene);
    match scene {
        PostcardScene::Nap => paint_nap(&mut picture, &mut CardText::new(), palette, &members),
        PostcardScene::Picnic => paint_picnic(&mut picture, palette, &members),
        PostcardScene::Play => paint_play(&mut picture, palette, &members),
        PostcardScene::Dusk => {
            paint_fireflies(&mut picture, rng, village_ground - 60, PICTURE_HEIGHT - 20);
            let row = row(&members, 350);
            for (member, place) in members.iter().zip(row) {
                draw_member(&mut picture, member, place);
            }
        }
    }
    picture
}

/// Bands of colour from the top of the sky down to the horizon, each meeting the next in a few
/// rows of checkerboard, the way pixel art blends without a gradient.
fn paint_sky(picture: &mut Canvas, light: &Light) {
    let bands = light.sky.len() as i32;
    let band = (light.horizon / bands).max(1);
    for y in 0..light.horizon + 40 {
        let index = (y / band).min(bands - 1);
        let within = y - index * band;
        let mut color = light.sky[index as usize];
        if within >= band - 4 && index + 1 < bands {
            let next = light.sky[index as usize + 1];
            for x in 0..PICTURE_WIDTH {
                let dither = (x + y) % 2 == 0 && within >= band - 2 || (x + y) % 4 == 0;
                picture.set(x, y, if dither { next } else { color });
            }
            continue;
        }
        if index + 1 == bands {
            color = light.sky[bands as usize - 1];
        }
        picture.fill_rect(0, y, PICTURE_WIDTH, 1, color);
    }
}

fn paint_sun(picture: &mut Canvas, x: i32, y: i32, radius: i32, color: Rgba) {
    let halo = Rgba::new(color.r, color.g, color.b, 70);
    for (grow, alpha) in [(14, 40_u8), (7, 70)] {
        soft_circle(picture, x, y, radius + grow, Rgba { a: alpha, ..halo });
    }
    picture.fill_circle(x, y, radius, color);
}

/// A circle mixed into what is already there, for glows and halos.
fn soft_circle(picture: &mut Canvas, x: i32, y: i32, radius: i32, color: Rgba) {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy > radius * radius {
                continue;
            }
            let (px, py) = (x + dx, y + dy);
            let under = picture.get(px, py);
            if under.a == 0 {
                continue;
            }
            picture.set(px, py, blend(under, color, 255));
        }
    }
}

fn paint_clouds(picture: &mut Canvas, rng: &mut CardRng, count: i32, color: Rgba) {
    let shade = blend(color, Rgba::new(160, 170, 200, 255), 40);
    for index in 0..count {
        let x = 180 + index * (PICTURE_WIDTH - 300) / count.max(1) + rng.range(60);
        let y = 34 + rng.range(60);
        let width = 44 + rng.range(40);
        picture.fill_ellipse(x, y + 6, width, 10, shade);
        picture.fill_ellipse(x, y + 4, width, 9, color);
        picture.fill_circle(x - width / 3, y - 2, 12 + rng.range(4), color);
        picture.fill_circle(x + width / 5, y - 6, 16 + rng.range(5), color);
    }
}

fn paint_stars(picture: &mut Canvas, rng: &mut CardRng, lowest: i32) {
    for _ in 0..70 {
        let x = rng.range(PICTURE_WIDTH);
        let y = 4 + rng.range(lowest.max(8));
        let size = if rng.range(6) == 0 { 2 } else { 1 };
        picture.fill_rect(x, y, size, size, Rgba::new(255, 240, 214, 255));
    }
}

/// A crescent: a full moon with the sky taken back out of one side.
fn paint_moon(picture: &mut Canvas, x: i32, y: i32) {
    picture.fill_circle(x, y, 20, Rgba::new(255, 240, 206, 255));
    // The bite is filled with the sky beside it, row by row, so it matches whatever band and
    // dither the moon was hung in.
    let (bite_x, bite_y, radius) = (x + 9, y - 5, 18);
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= radius * radius {
                let sky = picture.get(bite_x + dx + 64, bite_y + dy);
                picture.set(bite_x + dx, bite_y + dy, sky);
            }
        }
    }
}

/// Two rows of rolling hills along the horizon, the far one paler.
fn paint_hills(picture: &mut Canvas, rng: &mut CardRng, light: &Light) {
    for (lift, height, color) in [(30, 26, light.far_hills), (12, 18, light.near_hills)] {
        let phase = rng.range(360) as f32 / 57.3;
        let wavelength = 120.0 + rng.range(80) as f32;
        for x in 0..PICTURE_WIDTH {
            let t = x as f32 / wavelength + phase;
            let rise = (t.sin() * 0.6 + (t * 2.3).sin() * 0.4 + 1.0) / 2.0;
            let top = light.horizon - lift + ((1.0 - rise) * height as f32) as i32 / 2 * 2;
            picture.fill_rect(x, top, 1, light.horizon + 40 - top, color);
        }
    }
}

/// The ground from the horizon down, in bands that darken toward the front, with tufts of grass
/// and a few flowers so the colony is standing on a meadow rather than a panel of colour.
fn paint_ground(picture: &mut Canvas, rng: &mut CardRng, light: &Light) {
    let bands = light.ground.len() as i32;
    let depth = PICTURE_HEIGHT - light.horizon;
    for (index, color) in light.ground.iter().enumerate() {
        let top = light.horizon + depth * index as i32 / bands;
        picture.fill_rect(0, top, PICTURE_WIDTH, PICTURE_HEIGHT - top, *color);
    }
    for _ in 0..90 {
        let x = rng.range(PICTURE_WIDTH);
        let y = light.horizon + 8 + rng.range(depth - 8);
        let height = 3 + rng.range(4) + (y - light.horizon) / 40;
        picture.fill_rect(x, y - height, 2, height, light.tuft);
        picture.fill_rect(x + 3, y - height / 2, 2, height / 2, light.tuft);
    }
    let petals = [
        Rgba::new(255, 250, 236, 255),
        Rgba::new(255, 214, 110, 255),
        Rgba::new(244, 150, 170, 255),
    ];
    for _ in 0..26 {
        let x = rng.range(PICTURE_WIDTH);
        let y = light.horizon + 20 + rng.range(depth - 24);
        let petal = petals[rng.range(petals.len() as i32) as usize];
        picture.fill_rect(x, y, 2, 2, blend(petal, light.tuft, 60));
    }
}

/// Fireflies drifting low over the ground: a warm dot in a soft glow.
fn paint_fireflies(picture: &mut Canvas, rng: &mut CardRng, top: i32, bottom: i32) {
    for _ in 0..34 {
        let x = rng.range(PICTURE_WIDTH);
        let y = top + rng.range((bottom - top).max(1));
        soft_circle(picture, x, y, 4, Rgba::new(255, 226, 120, 60));
        picture.fill_rect(x, y, 2, 2, Rgba::new(255, 240, 160, 255));
    }
}

/// The colony's own village across the back of the picture, laid out by the very functions the
/// desktop uses, faded toward the sky so it stands behind the colony: the houses, both trees, the
/// belongings, the spots, and the gardens. After dark, the houses are the lit ones.
fn paint_village(
    picture: &mut Canvas,
    save: &SaveFile,
    ground: i32,
    ceiling: i32,
    after_dark: bool,
    light: &Light,
) {
    let lots = village_lots(save);
    if lots.is_empty() {
        return;
    }
    let village = ShelterRenderer::render_look(
        &crate::VillageLook::of(&save.home, &save.creatures),
        after_dark,
    );
    let objects = ColonyObjectRenderer::render_atlas(save.colony_seed);
    let left = lots
        .iter()
        .map(|lot| lot.center_x - lot.cell() / 2.0)
        .fold(f32::MAX, f32::min);
    let right = lots
        .iter()
        .map(|lot| lot.center_x + lot.cell() / 2.0)
        .fold(f32::MIN, f32::max);
    let span = (right - left).max(1.0);
    let room = (PICTURE_WIDTH - 24) as f32;
    let house_height = cell_height(&village, 0, 0, SHELTER_SIZE as i32).unwrap_or(36);
    let scale = (1..=4)
        .rev()
        .find(|scale| span * *scale as f32 <= room && house_height * scale <= ceiling)
        .unwrap_or(1);
    let origin = PICTURE_WIDTH / 2 - (span * scale as f32).round() as i32 / 2;
    let base = lots.iter().map(|lot| lot.ground_y).fold(f32::MIN, f32::max);
    let mut ordered: Vec<_> = lots.iter().collect();
    ordered.sort_by_key(|lot| lot.in_front());
    let haze = |pixel: Rgba| blend(pixel, light.haze, light.haze_amount);
    for lot in ordered {
        let x = origin + ((lot.center_x - lot.cell() / 2.0 - left) * scale as f32).round() as i32;
        let y = ground - (lot.cell() * scale as f32).round() as i32
            + ((lot.ground_y - base) * scale as f32).round() as i32;
        let (atlas, source_x, source_y, size, mirrored) = match lot.art {
            LotArt::Dwelling { slot } => {
                let (u, v) = ShelterRenderer::village_cell(VillageCell::House {
                    slot,
                    lit: after_dark,
                    occupied: false,
                });
                (&village, u as i32, v as i32, SHELTER_SIZE as i32, false)
            }
            LotArt::Tree { mirrored } => {
                let (u, v) = ShelterRenderer::village_cell(VillageCell::Tree);
                (&village, u as i32, v as i32, SHELTER_SIZE as i32, mirrored)
            }
            LotArt::Object(kind) => {
                let (u, v) =
                    ColonyObjectRenderer::cell_origin(ColonyObjectRenderer::object_cell(kind));
                (
                    &objects,
                    u as i32,
                    v as i32,
                    COLONY_OBJECT_SIZE as i32,
                    false,
                )
            }
            LotArt::Ground { cell, mirrored } => {
                let (u, v) = ColonyObjectRenderer::cell_origin(cell);
                (
                    &objects,
                    u as i32,
                    v as i32,
                    COLONY_OBJECT_SIZE as i32,
                    mirrored,
                )
            }
        };
        for row in 0..size {
            for column in 0..size {
                let read = if mirrored { size - 1 - column } else { column };
                let pixel = atlas.get(source_x + read, source_y + row);
                if pixel.a == 0 {
                    continue;
                }
                // Lamplight carries through the haze: a lit window stays lit from far off.
                let lamp = after_dark && pixel.r > 230 && pixel.g > 180 && pixel.b < 190;
                let color = if lamp {
                    Rgba::new(pixel.r, pixel.g, pixel.b, 255)
                } else {
                    haze(Rgba::new(pixel.r, pixel.g, pixel.b, 255))
                };
                for dy in 0..scale {
                    for dx in 0..scale {
                        let (px, py) = (x + column * scale + dx, y + row * scale + dy);
                        let under = picture.get(px, py);
                        picture.set(px, py, blend(under, color, pixel.a));
                    }
                }
            }
        }
    }
}

/// One member of the colony, posed for the scene and measured before anything is placed.
struct Member {
    canvas: Canvas,
    /// The drawn pixels inside the 48x48 frame.
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

/// Where one member goes: the middle of its feet, and the scale it is drawn at.
#[derive(Clone, Copy)]
struct Place {
    center_x: i32,
    feet_y: i32,
    scale: i32,
}

/// Everyone who lives here, in colony order, each in the pose the scene gives it, turned toward
/// the middle of the group.
fn pose_members(save: &SaveFile, scene: PostcardScene) -> Vec<Member> {
    let mut ordered: Vec<&Creature> = save.creatures.iter().collect();
    ordered.sort_by_key(|creature| (creature.colony_order, creature.id));
    let count = ordered.len();
    ordered
        .into_iter()
        .enumerate()
        .map(|(index, creature)| {
            let facing_right = match scene {
                // Two by two, facing one another across the game.
                PostcardScene::Play => index % 2 == 0 && index + 1 < count,
                _ => index * 2 < count,
            };
            let clip: BodyClip = match scene {
                PostcardScene::Nap => ActionKind::Sleep.into(),
                PostcardScene::Picnic if index % 2 == 0 => ActionKind::Eat.into(),
                PostcardScene::Picnic => ActionKind::Drink.into(),
                PostcardScene::Play => match index % 4 {
                    0 => ActionKind::SoloPlay.into(),
                    1 => BodyClip::Gesture(Gesture::Cheer),
                    2 => ActionKind::SocialPlay.into(),
                    _ => BodyClip::Gesture(Gesture::Bop),
                },
                PostcardScene::Dusk => ActionKind::Greet.into(),
            };
            let gaze = GazeDirection::new(if facing_right { 1 } else { -1 }, 0);
            let face = match scene {
                PostcardScene::Nap => FaceRenderState {
                    expression: ExpressionKind::Sleepy,
                    eyelids: EyelidPose::Closed,
                    gaze: GazeDirection::new(0, 0),
                },
                PostcardScene::Picnic => FaceRenderState {
                    expression: ExpressionKind::Content,
                    eyelids: EyelidPose::Open,
                    gaze,
                },
                PostcardScene::Play => FaceRenderState {
                    expression: ExpressionKind::Joy,
                    eyelids: EyelidPose::Open,
                    gaze,
                },
                PostcardScene::Dusk => FaceRenderState {
                    expression: ExpressionKind::Affectionate,
                    eyelids: EyelidPose::Open,
                    gaze,
                },
            };
            let frames = AnimationSpec::for_clip(clip).frames.max(1);
            let frame = creature.colony_order.wrapping_add(creature.generation) % frames;
            let canvas = CreatureRenderer::render_composited_frame(
                &creature.appearance,
                clip,
                frame,
                facing_right,
                false,
                face,
            );
            let (left, top, right, bottom) = canvas.alpha_bounds().unwrap_or((0, 0, 47, 47));
            Member {
                canvas,
                left: left as i32,
                top: top as i32,
                width: (right - left + 1) as i32,
                height: (bottom - top + 1) as i32,
            }
        })
        .collect()
}

/// The colony in one row at the largest whole scale it fits at, the spare room shared out before,
/// between, and after them, feet on `feet_y`.
fn row(members: &[Member], feet_y: i32) -> Vec<Place> {
    let count = members.len() as i32;
    if count == 0 {
        return Vec::new();
    }
    let room = PICTURE_WIDTH - PADDING * 2;
    let art: i32 = members.iter().map(|member| member.width).sum();
    let tallest = members
        .iter()
        .map(|member| member.height)
        .max()
        .unwrap_or(1);
    let scale = (2..=5)
        .rev()
        .find(|scale| art * scale + GAP * (count + 1) <= room && tallest * scale <= 150)
        .unwrap_or(2);
    let slack = (room - art * scale).max(0);
    let gap = slack / (count + 1);
    let mut x = PADDING + gap + (slack - gap * (count + 1)) / 2;
    members
        .iter()
        .map(|member| {
            let width = member.width * scale;
            let place = Place {
                center_x: x + width / 2,
                feet_y,
                scale,
            };
            x += width + gap;
            place
        })
        .collect()
}

/// One member drawn with its lowest pixel on its feet line, over a soft contact shadow.
fn draw_member(picture: &mut Canvas, member: &Member, place: Place) {
    let scale = place.scale;
    let width = member.width * scale;
    let origin_x = place.center_x - width / 2 - member.left * scale;
    let origin_y = place.feet_y - (member.top + member.height) * scale;
    contact_shadow(
        picture,
        place.center_x,
        place.feet_y,
        width / 2 + 4,
        4 + scale,
    );
    for y in 0..member.canvas.height() as i32 {
        for x in 0..member.canvas.width() as i32 {
            let pixel = member.canvas.get(x, y);
            if pixel.a == 0 {
                continue;
            }
            let solid = Rgba::new(pixel.r, pixel.g, pixel.b, 255);
            for dy in 0..scale {
                for dx in 0..scale {
                    let (px, py) = (origin_x + x * scale + dx, origin_y + y * scale + dy);
                    let under = picture.get(px, py);
                    picture.set(px, py, blend(under, solid, pixel.a));
                }
            }
        }
    }
}

fn contact_shadow(picture: &mut Canvas, x: i32, y: i32, radius_x: i32, radius_y: i32) {
    let (rx2, ry2) = ((radius_x * radius_x) as i64, (radius_y * radius_y) as i64);
    for dy in -radius_y..=radius_y {
        for dx in -radius_x..=radius_x {
            if dx as i64 * dx as i64 * ry2 + dy as i64 * dy as i64 * rx2 > rx2 * ry2 {
                continue;
            }
            let (px, py) = (x + dx, y + dy);
            let under = picture.get(px, py);
            picture.set(px, py, blend(under, Rgba::new(20, 30, 36, 255), 70));
        }
    }
}

/// Everyone asleep in a heap on a patchwork quilt, close enough to touch and a little up and down,
/// with a "z" or three rising off every other sleeper.
fn paint_nap(picture: &mut Canvas, text: &mut CardText, palette: Palette, members: &[Member]) {
    let feet = 318;
    let mut places = huddle(members, feet, -6);
    for (index, place) in places.iter_mut().enumerate() {
        place.feet_y += [0, 6, -3, 5, -2, 7][index % 6];
    }
    if let (Some(first), Some(last)) = (places.first(), places.last()) {
        let reach = |member: &Member, place: &Place| member.width * place.scale / 2 + 34;
        let left = first.center_x - reach(&members[0], first);
        let right = last.center_x + reach(&members[members.len() - 1], last);
        paint_quilt(picture, palette, left, right, feet - 24, feet + 40);
    }
    let mut order: Vec<usize> = (0..members.len()).collect();
    order.sort_by_key(|index| places[*index].feet_y);
    for index in order {
        draw_member(picture, &members[index], places[index]);
    }
    let ink = Rgba::new(116, 84, 112, 255);
    for (index, (member, place)) in members.iter().zip(&places).enumerate() {
        if index % 2 == 1 {
            continue;
        }
        let top = place.feet_y - member.height * place.scale;
        let x = place.center_x + member.width * place.scale / 4;
        for (step, size) in [(0, 16.0), (1, 21.0), (2, 27.0)] {
            text.draw(picture, x + step * 13, top - 12 - step * 19, "z", size, ink);
        }
    }
}

/// The colony close together in the middle of the picture, `gap` apart (less than nothing leans
/// them on one another), at the scale a row of them would be drawn at.
fn huddle(members: &[Member], feet_y: i32, gap: i32) -> Vec<Place> {
    let spread = row(members, feet_y);
    let Some(scale) = spread.first().map(|place| place.scale) else {
        return spread;
    };
    let total: i32 = members
        .iter()
        .map(|member| member.width * scale)
        .sum::<i32>()
        + gap * (members.len() as i32 - 1);
    let mut x = PICTURE_WIDTH / 2 - total / 2;
    members
        .iter()
        .map(|member| {
            let width = member.width * scale;
            let place = Place {
                center_x: x + width / 2,
                feet_y,
                scale,
            };
            x += width + gap;
            place
        })
        .collect()
}

/// A patchwork quilt spread flat on the grass: squares in the colony's own colours and cream,
/// stitched along every seam, with a binding round the edge.
fn paint_quilt(
    picture: &mut Canvas,
    palette: Palette,
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
) {
    let binding = palette.shadow;
    picture.fill_rect(left + 4, top, right - left - 8, bottom - top, binding);
    picture.fill_rect(left, top + 4, right - left, bottom - top - 8, binding);
    let patches = [palette.coat, CREAM, palette.accent, palette.highlight];
    let size = 22;
    for y in (top + 4..bottom - 4).step_by(size as usize) {
        for x in (left + 4..right - 4).step_by(size as usize) {
            let index = (((x - left) / size) + ((y - top) / size) * 3) as usize % patches.len();
            let width = size.min(right - 4 - x);
            let height = size.min(bottom - 4 - y);
            picture.fill_rect(x, y, width, height, patches[index]);
            for stitch in (x + 2..x + width).step_by(4) {
                picture.set(stitch, y + height - 1, blend(patches[index], binding, 140));
            }
            for stitch in (y + 2..y + height).step_by(4) {
                picture.set(x + width - 1, stitch, blend(patches[index], binding, 140));
            }
        }
    }
}

/// Everyone round a gingham blanket with a snack or a cup: the middle of the group at the far
/// side of the blanket, the ends coming round its sides, and a basket in the middle.
fn paint_picnic(picture: &mut Canvas, palette: Palette, members: &[Member]) {
    let far = 292;
    let mut places = row(members, far);
    let count = places.len() as i32;
    let middle = (count - 1) as f32 / 2.0;
    for (index, place) in places.iter_mut().enumerate() {
        let out = if count > 1 {
            (index as f32 - middle).abs() / middle.max(1.0)
        } else {
            0.0
        };
        place.feet_y = far + (out * out * 56.0) as i32;
    }
    let span = match (places.first(), places.last()) {
        (Some(first), Some(last)) => (last.center_x - first.center_x).max(160),
        _ => 160,
    };
    let center = PICTURE_WIDTH / 2;
    paint_blanket(
        picture,
        palette,
        center,
        (span as f32 * 0.42) as i32 + 60,
        far - 18,
        far + 52,
    );
    paint_basket(picture, center, far + 30);
    let mut order: Vec<usize> = (0..members.len()).collect();
    order.sort_by_key(|index| places[*index].feet_y);
    for index in order {
        draw_member(picture, &members[index], places[index]);
    }
}

/// A gingham blanket spread on the grass, narrower at its far edge than its near one.
fn paint_blanket(
    picture: &mut Canvas,
    palette: Palette,
    center: i32,
    half_width: i32,
    top: i32,
    bottom: i32,
) {
    let dark = palette.accent;
    let light = blend(palette.accent, CREAM, 150);
    let cream = CREAM;
    for y in top..bottom {
        let depth = (y - top) as f32 / (bottom - top).max(1) as f32;
        let half = (half_width as f32 * (0.82 + 0.18 * depth)) as i32;
        for x in center - half..center + half {
            let band_x = ((x - center + 400) / 14) % 2 == 0;
            let band_y = ((y - top) / 10) % 2 == 0;
            let color = match (band_x, band_y) {
                (true, true) => dark,
                (true, false) | (false, true) => light,
                (false, false) => cream,
            };
            let edge = x == center - half || x == center + half - 1 || y == top || y == bottom - 1;
            picture.set(x, y, if edge { palette.outline } else { color });
        }
    }
}

/// A woven basket with a handle, an apple beside it.
fn paint_basket(picture: &mut Canvas, x: i32, y: i32) {
    let outline = Rgba::new(70, 44, 30, 255);
    let wicker = Rgba::new(188, 132, 76, 255);
    let weave = Rgba::new(150, 100, 56, 255);
    for step in 0..=24 {
        let angle = std::f32::consts::PI * step as f32 / 24.0;
        let hx = x + (angle.cos() * 20.0) as i32;
        let hy = y - 14 - (angle.sin() * 16.0) as i32;
        picture.fill_rect(hx - 1, hy - 1, 3, 3, outline);
        picture.set(hx, hy, weave);
    }
    picture.fill_rect(x - 24, y - 16, 48, 22, outline);
    picture.fill_rect(x - 22, y - 14, 44, 18, wicker);
    for row in 0..3 {
        for column in 0..6 {
            if (row + column) % 2 == 0 {
                picture.fill_rect(x - 20 + column * 7, y - 12 + row * 6, 5, 3, weave);
            }
        }
    }
    picture.fill_circle(x + 36, y + 2, 7, outline);
    picture.fill_circle(x + 36, y + 2, 6, Rgba::new(220, 64, 70, 255));
    picture.fill_rect(x + 33, y - 1, 2, 2, Rgba::new(255, 170, 170, 255));
    picture.fill_rect(x + 36, y - 8, 2, 4, outline);
    picture.fill_rect(x + 38, y - 9, 4, 2, Rgba::new(92, 158, 80, 255));
}

/// The colony out on the grass in pairs, one of each pair further back, a ball in the air between
/// the first two, and a kite flying over everyone.
fn paint_play(picture: &mut Canvas, palette: Palette, members: &[Member]) {
    paint_kite(picture, palette, 540, 70);
    let ground = 326;
    let mut places = row(members, ground);
    for (index, place) in places.iter_mut().enumerate() {
        place.feet_y += if (index / 2) % 2 == 0 { -10 } else { 16 };
    }
    let mut order: Vec<usize> = (0..members.len()).collect();
    order.sort_by_key(|index| places[*index].feet_y);
    for index in order {
        draw_member(picture, &members[index], places[index]);
    }
    let (Some(first), Some(member)) = (places.first(), members.first()) else {
        return;
    };
    let top = first.feet_y - member.height * first.scale;
    let toward = places
        .get(1)
        .map_or(first.center_x + 120, |second| second.center_x);
    let (from, to) = (first.center_x, toward);
    let ball_x = from + (to - from) / 2;
    let ball_y = top - 4;
    // A dotted arc from the thrower up to where the ball is now.
    for step in 1..7 {
        let t = step as f32 / 8.0;
        let x = from + ((ball_x - from) as f32 * t) as i32;
        let y = top + 30 + ((ball_y - top - 30) as f32 * t) as i32 - (t * (1.0 - t) * 70.0) as i32;
        picture.fill_rect(x, y, 4, 4, blend(CREAM, palette.outline, 40));
    }
    picture.fill_circle(ball_x, ball_y, 15, palette.outline);
    picture.fill_circle(ball_x, ball_y, 13, palette.accent);
    picture.fill_rect(ball_x - 13, ball_y - 2, 26, 4, CREAM);
    picture.fill_rect(ball_x - 7, ball_y - 9, 4, 4, Rgba::new(255, 255, 255, 255));
}

/// A diamond kite in the colony's colours with a bowed tail trailing off to the side.
fn paint_kite(picture: &mut Canvas, palette: Palette, x: i32, y: i32) {
    for row in -26_i32..=26 {
        let half = 18 - (row.abs() * 18 / 26);
        let color = if row < 0 {
            palette.accent
        } else {
            palette.coat
        };
        picture.fill_rect(x - half - 1, y + row, half * 2 + 3, 1, palette.outline);
        if half > 0 {
            picture.fill_rect(x - half, y + row, half * 2 + 1, 1, color);
        }
    }
    picture.fill_rect(x - 1, y - 26, 2, 52, palette.outline);
    let mut tail = (x, y + 26);
    for step in 0..12 {
        let next = (tail.0 - 6, tail.1 + 7 + if step % 2 == 0 { 3 } else { -3 });
        picture.line(tail.0, tail.1, next.0, next.1, 1, palette.outline);
        if step % 3 == 1 {
            picture.fill_rect(next.0 - 3, next.1 - 2, 6, 4, palette.highlight);
        }
        tail = next;
    }
}

/// A postage stamp stuck over the picture's top corner — the colony house on the colony's own
/// colour, with scalloped edges — cancelled by a round postmark with the month.
fn draw_stamp(
    canvas: &mut Canvas,
    text: &mut CardText,
    save: &SaveFile,
    palette: Palette,
    month: &str,
    scene: PostcardScene,
) {
    let (x, y, width, height) = (792, 34, 84, 100);
    let paper = Rgba::new(252, 248, 238, 255);
    canvas.fill_rect(x + 3, y + 3, width, height, PAPER_SHADOW);
    canvas.fill_rect(x, y, width, height, paper);
    for step in (x + 4..x + width - 2).step_by(8) {
        canvas.fill_circle(step, y, 2, PAPER_SHADOW);
        canvas.fill_circle(step, y + height - 1, 2, PAPER_SHADOW);
    }
    for step in (y + 4..y + height - 2).step_by(8) {
        canvas.fill_circle(x, step, 2, PAPER_SHADOW);
        canvas.fill_circle(x + width - 1, step, 2, PAPER_SHADOW);
    }
    let inner = blend(palette.highlight, paper, 60);
    canvas.fill_rect(x + 8, y + 8, width - 16, height - 16, inner);
    let village = ShelterRenderer::render_village(
        &save.home.drawn_shelter(),
        &[],
        &[],
        &save.home.house_style_list(&save.creatures)[..1],
        false,
    );
    let (u, v) = ShelterRenderer::village_cell(VillageCell::House {
        slot: 0,
        lit: false,
        occupied: false,
    });
    let origin = (x + width / 2 - 32, y + 14);
    for row in 0..SHELTER_SIZE as i32 {
        for column in 0..SHELTER_SIZE as i32 {
            let pixel = village.get(u as i32 + column, v as i32 + row);
            if pixel.a == 0 {
                continue;
            }
            let (px, py) = (origin.0 + column, origin.1 + row);
            if px < x + 8 || px >= x + width - 8 || py < y + 8 || py >= y + height - 16 {
                continue;
            }
            canvas.set(px, py, blend(canvas.get(px, py), pixel, 255));
        }
    }
    canvas.fill_rect(x + 8, y + height - 20, width - 16, 12, palette.shadow);
    text.draw_centered(
        canvas,
        x + width / 2,
        y + height - 21,
        "FORMIGA",
        10.0,
        CREAM,
    );

    // The postmark: two rings with the month between them, and wavy cancellation lines running
    // out to the left over the picture.
    let (cx, cy) = (x - 2, y + 64);
    // Dark ink on a light sky; after sundown the postmark is struck in pale ink instead, or it
    // would vanish into the night.
    let ink = if scene == PostcardScene::Dusk {
        Rgba::new(255, 236, 214, 150)
    } else {
        Rgba::new(64, 58, 84, 150)
    };
    for radius in [30, 26] {
        for step in 0..180 {
            let angle = step as f32 / 180.0 * std::f32::consts::TAU;
            let px = cx + (angle.cos() * radius as f32).round() as i32;
            let py = cy + (angle.sin() * radius as f32).round() as i32;
            canvas.set(px, py, blend(canvas.get(px, py), ink, 255));
        }
    }
    let short = match month.split_once(' ') {
        Some((name, year)) => format!("{} {year}", name.chars().take(3).collect::<String>()),
        None => month.to_owned(),
    }
    .to_uppercase();
    text.draw_centered(canvas, cx, cy - 14, "FORMIGA", 9.0, ink);
    text.draw_centered(canvas, cx, cy + 2, &short, 9.0, ink);
    for line in 0..4 {
        let base = cy - 18 + line * 12;
        for step in 0..70 {
            let px = cx - 36 - step;
            let py = base + ((step as f32 / 6.0).sin() * 3.0).round() as i32;
            canvas.set(px, py, blend(canvas.get(px, py), ink, 255));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::{DesktopSnapshot, World};

    fn colony(members: usize) -> SaveFile {
        let created = time::macros::datetime!(2026-03-08 09:15 UTC);
        let mut world = World::new([31; 32], created, &DesktopSnapshot::default());
        while world.save.creatures.len() < members {
            let generation = world.save.creatures.len() as u8;
            let mut next =
                World::preview_adult([generation + 40; 32], created, &DesktopSnapshot::default());
            next.id = u64::from(generation) + 100;
            next.colony_order = generation;
            world.save.creatures.push(next);
        }
        world.save
    }

    /// Every scene, for a colony of one and a colony of six, with and without a caption: the same
    /// colony always makes the same card, each scene makes a different one, and the caption only
    /// changes what is written under the picture.
    #[test]
    fn every_scene_draws_the_colony_the_same_way_every_time() {
        for members in [1, 6] {
            let save = colony(members);
            let mut seen = Vec::new();
            for scene in PostcardScene::ALL {
                let card = PostcardRenderer::render(&save, scene, "");
                assert_eq!(card.width(), POSTCARD_WIDTH);
                assert_eq!(card.height(), POSTCARD_HEIGHT);
                assert_eq!(card, PostcardRenderer::render(&save, scene, ""));
                assert!(card.pixels().iter().all(|pixel| pixel.a == 255), "opaque");
                assert!(!seen.contains(&card), "{scene:?} looks like another scene");
                let captioned = PostcardRenderer::render(&save, scene, "Wish you were here");
                let changed: Vec<i32> = (0..POSTCARD_HEIGHT as i32)
                    .filter(|y| {
                        (0..POSTCARD_WIDTH as i32).any(|x| card.get(x, *y) != captioned.get(x, *y))
                    })
                    .collect();
                assert!(!changed.is_empty(), "{scene:?} writes the caption");
                assert!(
                    changed.iter().all(|y| *y > PICTURE_Y + PICTURE_HEIGHT),
                    "{scene:?}: the caption reaches the picture"
                );
                seen.push(card);
            }
        }
    }

    /// Everyone in the colony is in the picture: a colony of six leaves far more of the picture
    /// covered than a colony of one, whatever the scene.
    #[test]
    fn the_whole_colony_is_in_every_scene() {
        let (one, six) = (colony(1), colony(6));
        for scene in PostcardScene::ALL {
            let alone = pose_members(&one, scene);
            let together = pose_members(&six, scene);
            assert_eq!(alone.len(), 1);
            assert_eq!(together.len(), 6);
            let places = row(&together, 300);
            for pair in places.windows(2) {
                assert!(
                    pair[0].center_x < pair[1].center_x,
                    "{scene:?} keeps its order"
                );
            }
            for (member, place) in together.iter().zip(&places) {
                let half = member.width * place.scale / 2;
                assert!(place.center_x - half >= PADDING / 2, "{scene:?}");
                assert!(
                    place.center_x + half <= PICTURE_WIDTH - PADDING / 2,
                    "{scene:?}"
                );
            }
        }
    }

    #[test]
    fn a_caption_is_written_as_typed_less_what_cannot_be() {
        assert_eq!(
            postcard_caption("  Hello \n  from\tthe  village!  "),
            "Hello from the village!"
        );
        assert_eq!(postcard_caption("\u{7}\u{1b}[2J"), "[2J");
        assert_eq!(postcard_caption(""), "");
        let long = "ü".repeat(POSTCARD_CAPTION_LIMIT + 10);
        assert_eq!(
            postcard_caption(&long).chars().count(),
            POSTCARD_CAPTION_LIMIT
        );
    }

    #[test]
    fn scenes_are_named_by_their_slugs() {
        for scene in PostcardScene::ALL {
            assert_eq!(PostcardScene::from_slug(scene.slug()), Some(scene));
        }
        assert_eq!(PostcardScene::from_slug("DUSK"), Some(PostcardScene::Dusk));
        assert_eq!(PostcardScene::from_slug("party"), None);
    }
}
