//! A whole climb up onto a window, as the desktop shows it: the colony's own simulation walks a
//! companion over, up the window's side and onto its ledge, and every tenth of a second of it is
//! drawn where the overlay would draw it, against the window it is climbing.

use crate::{fill_rect, reference_creatures, write_png};
use anyhow::Result;
use formiga_art::{BodyPresentation, Canvas, CreatureRenderer, FRAME_SIZE, FramePlacement};
use formiga_core::*;
use std::path::PathBuf;
use time::{Duration, OffsetDateTime};

/// Sheet pixels to a point, and to an art pixel: a companion at display scale 2 on a 2x display,
/// which draws one art pixel to a point.
const UNIT: u32 = 2;
/// How far the window's top is above the floor, in points.
const RISE: f32 = 120.0;
const WINDOW_KEY: WindowKey = 1201;
const WINDOW_LEFT: f32 = 600.0;
/// Where the climber starts, in points left of the window: near enough to see the walk over.
const START_BEFORE: f32 = 26.0;
const TICK: f32 = 0.05;
/// One sample every this many ticks: a tenth of a second.
const TICKS_PER_SAMPLE: u32 = 2;
/// How long a perch is shown once it has begun, in seconds.
const PERCH_SHOWN: f32 = 0.6;
/// Each sample's view, in points from the window's top-left corner.
const VIEW_LEFT: f32 = -50.0;
const VIEW_RIGHT: f32 = 46.0;
const VIEW_ABOVE: f32 = 56.0;
const VIEW_BELOW_FLOOR: f32 = 6.0;
const SAMPLES_PER_ROW: u32 = 20;
const LABEL: u32 = 150;
const CAPTION: u32 = 20;
const ROW_GAP: u32 = 6;
const HANGING: [f32; 3] = [0.0, 0.5, 1.0];
/// A dangle's view, in points from the ledge it hangs from.
const DANGLE_WIDE: f32 = 64.0;
const DANGLE_ABOVE: f32 = 50.0;
const DANGLE_BELOW: f32 = 54.0;

/// What a body's two climbs are told apart by on the sheet.
const SIDE_LABEL: &str = ", SIDE-ON";
const BACK_LABEL: &str = ", BACK TO US";

const WALLPAPER: [u8; 4] = [236, 234, 228, 255];
const WINDOW_FACE: [u8; 4] = [200, 210, 218, 255];
const WINDOW_TITLE: [u8; 4] = [168, 180, 192, 255];
const WINDOW_EDGE: [u8; 4] = [64, 74, 86, 255];
const FLOOR: [u8; 4] = [178, 170, 156, 255];
const INK: [u8; 4] = [60, 64, 62, 255];

/// The bodies the sheet climbs with: a modular body from each original family, and one body plan
/// of each kind the families' bodies do not already show.
fn climbers() -> Vec<(String, AppearanceGenome)> {
    let mut named: Vec<(String, AppearanceGenome)> = reference_creatures()
        .into_iter()
        .map(|creature| {
            let genome = creature.appearance;
            let plan = genome.design.map(|design| design.body);
            (
                format!("{:?} {:?}", genome.family, plan.unwrap_or(BodyPlan::Round)).to_uppercase(),
                genome,
            )
        })
        .collect();
    let shown: Vec<_> = named
        .iter()
        .filter_map(|(_, genome)| genome.design.map(|design| design.body))
        .collect();
    let preview = World::preview_adult(
        [29; 32],
        OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    for plan in BodyPlan::ALL
        .into_iter()
        .filter(|plan| !shown.contains(plan))
    {
        let mut appearance = preview.appearance.clone();
        let mut design = CreatureDesign::modular([29; 32], 0, None);
        design.body = plan;
        appearance.design = Some(design);
        named.push((format!("PLAN {plan:?}").to_uppercase(), appearance));
    }
    // The looks from before recipes, which are drawn side on by their own renderers, and the fox.
    for (name, genome) in named.clone().into_iter().take(3) {
        let mut classic = genome;
        classic.design = None;
        let family = name.split(' ').next().unwrap_or_default();
        named.push((format!("{family} BEFORE RECIPES"), classic));
    }
    let mut fox = preview.appearance.clone();
    fox.echo_fox = true;
    named.push(("FOX".into(), fox));
    named
}

/// The companions named with `--colony` and `--cast`, or the sheet's own bodies when none is.
fn cast_or_climbers(args: &[String]) -> Result<Vec<(String, AppearanceGenome)>> {
    let mut cast = crate::cast::colony_argument(args)?;
    cast.extend(crate::cast::cast_argument(args)?);
    if cast.is_empty() {
        return Ok(climbers());
    }
    Ok(cast
        .into_iter()
        .map(|creature| (creature.name.to_uppercase(), creature.appearance))
        .collect())
}

/// Every climb the sheet shows: each body side-on, up the outside of the window's side, and
/// then, where it can be drawn so, with its back to the screen up the window's face.
fn climbs_shown(args: &[String], every: u32) -> Result<Vec<(String, AppearanceGenome, Climb)>> {
    let mut shown = Vec::new();
    for (name, genome) in cast_or_climbers(args)? {
        let both = climbs_both_ways(&genome);
        let side = climb_sampled(&genome, every, false);
        let label = if both {
            format!("{name}{SIDE_LABEL}")
        } else {
            name.clone()
        };
        shown.push((label, genome.clone(), side));
        if both {
            let back = climb_sampled(&genome, every, true);
            shown.push((format!("{name}{BACK_LABEL}"), genome, back));
        }
    }
    Ok(shown)
}

/// `climb-sheet --animate`: the same climbs moving, side by side, as a looping GIF at the
/// simulation's own twenty pictures a second, each placed as the overlay places it: a row of
/// climbs made side-on, and under it the climbs made with their backs to the screen.
pub fn animate(path: PathBuf, args: &[String]) -> Result<()> {
    const SCALE: u32 = 2;
    // A row of everybody side-on, and under it a row of those that climb with their backs to us.
    let (back, side): (Vec<_>, Vec<_>) = climbs_shown(args, 1)?
        .into_iter()
        .map(|(name, _, climb)| (name, climb))
        .partition(|(name, _)| name.ends_with(BACK_LABEL));
    let across = side.len().max(1) as u32;
    let rows = 1 + u32::from(!back.is_empty());
    let climbs: Vec<_> = side.into_iter().chain(back).collect();
    let view_w = ((VIEW_RIGHT - VIEW_LEFT) * UNIT as f32) as u32;
    let view_h = ((VIEW_ABOVE + RISE + VIEW_BELOW_FLOOR) * UNIT as f32) as u32;
    let (width, height) = (view_w * across, (view_h + CAPTION) * rows);
    let longest = climbs
        .iter()
        .map(|(_, climb)| climb.samples.len())
        .max()
        .unwrap_or(0);
    let file = std::fs::File::create(&path)?;
    let mut encoder = gif::Encoder::new(
        std::io::BufWriter::new(file),
        (width * SCALE) as u16,
        (height * SCALE) as u16,
        &[],
    )?;
    encoder.set_repeat(gif::Repeat::Infinite)?;
    // Held on the perch at the end for a second before it comes round.
    for index in 0..longest + 20 {
        let mut sheet = Sheet {
            pixels: vec![0; (width * height * 4) as usize],
            width,
            height,
        };
        for chunk in sheet.pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&WALLPAPER);
        }
        for (column, (name, climb)) in climbs.iter().enumerate() {
            let (_, creature) = &climb.samples[index.min(climb.samples.len() - 1)];
            let bounds = window_bounds(climb.floor);
            let origin = Point {
                x: bounds.x + VIEW_LEFT,
                y: bounds.y - VIEW_ABOVE,
            };
            let top = column as u32 / across * (view_h + CAPTION);
            let view = (column as u32 % across * view_w, top, view_w, view_h);
            let floor_y = top as i32 + ((climb.floor - origin.y) * UNIT as f32) as i32;
            sheet.rect_in(
                view,
                view.0 as i32,
                floor_y,
                view_w as i32,
                view_h as i32,
                FLOOR,
            );
            sheet.window(view, origin, bounds);
            sheet.companion(view, origin, creature);
            sheet.text(view.0 + 4, top + view_h + 6, name);
        }
        let mut scaled = vec![0; (width * SCALE * height * SCALE * 4) as usize];
        for y in 0..height * SCALE {
            for x in 0..width * SCALE {
                let from = ((y / SCALE * width + x / SCALE) * 4) as usize;
                let to = ((y * width * SCALE + x) * 4) as usize;
                scaled[to..to + 4].copy_from_slice(&sheet.pixels[from..from + 4]);
            }
        }
        let mut frame = gif::Frame::from_rgba_speed(
            (width * SCALE) as u16,
            (height * SCALE) as u16,
            scaled.as_mut_slice(),
            20,
        );
        frame.delay = 5;
        encoder.write_frame(&frame)?;
    }
    println!("wrote {}", path.display());
    Ok(())
}

/// One display with one window on it whose top is [`RISE`] points above the floor a companion
/// stands on.
fn desktop_with_window(floor: f32) -> DesktopSnapshot {
    let mut desktop = bare_desktop();
    desktop.windows.push(DesktopWindow {
        key: WINDOW_KEY,
        bounds: window_bounds(floor),
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    desktop
}

fn window_bounds(floor: f32) -> DesktopRect {
    DesktopRect {
        x: WINDOW_LEFT,
        y: floor - RISE,
        width: 320.0,
        height: RISE,
    }
}

fn bare_desktop() -> DesktopSnapshot {
    DesktopSnapshot {
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
                height: 826.0,
            },
            scale_factor: 2.0,
            primary: true,
        }],
        ..DesktopSnapshot::default()
    }
}

/// A climb, a tenth of a second at a time: how long in, and the companion then.
pub(crate) struct Climb {
    pub(crate) floor: f32,
    pub(crate) samples: Vec<(f32, Creature)>,
}

/// Whether a companion wearing `appearance` can make a climb with its back to the screen, up the
/// window's face, as well as side-on up the outside of its side.
fn climbs_both_ways(appearance: &AppearanceGenome) -> bool {
    formiga_core::climbs_turned(appearance)
}

/// One companion wearing `appearance` climbs onto the window from the floor beside it, in the
/// colony's own simulation, until it has perched for [`PERCH_SHOWN`] seconds: sampled every
/// `every` ticks, and made with its `back` to the screen where the body can be (see
/// [`climbs_both_ways`]). A companion picks one way or the other for each climb from how many it
/// has made; the sheet gives it a count that picks the way asked for.
fn climb_sampled(appearance: &AppearanceGenome, every: u32, back: bool) -> Climb {
    let created = OffsetDateTime::UNIX_EPOCH + Duration::days(400);
    let floor = World::new([47; 32], created, &bare_desktop())
        .save
        .creatures[0]
        .state
        .position
        .y;
    let desktop = desktop_with_window(floor);
    let mut world = World::new([47; 32], created, &desktop);
    world.save.settings.display_scale = 2;
    let mut now = created;
    // Settled in first, the colony out on the desktop rather than home, and one companion alone,
    // so nobody else's doings get in the way of the climb.
    for _ in 0..60 {
        now += Duration::milliseconds(50);
        world.tick(now, TICK, &desktop);
    }
    world.save.home.active_since_utc = None;
    world.save.home.last_disappeared_utc = Some(now);
    world.save.creatures.truncate(1);
    let id = world.save.creatures[0].id;
    {
        let creature = &mut world.save.creatures[0];
        creature.appearance = appearance.clone();
        creature.state.position = Point {
            x: WINDOW_LEFT - START_BEFORE,
            y: floor,
        };
        creature.state.surface = SurfaceAttachment {
            kind: SurfaceKind::ScreenFloor,
            monitor_id: 1,
            window_key: None,
            relative_x: 0.5,
        };
        creature.state.depth = 0.0;
        creature.state.facing_right = true;
        creature.state.attention = None;
        creature.state.beat = None;
        creature.state.flourish = None;
        creature.state.arrival_delay_secs = 0.0;
        let back = back && climbs_both_ways(appearance);
        creature.memory.window_climbs = (0..u32::MAX)
            .find(|climbs| {
                creature.memory.window_climbs = *climbs;
                formiga_core::climbs_away(creature, WINDOW_KEY) == back
            })
            .unwrap_or(0);
    }
    let began = world.begin_window_climb(id, WINDOW_KEY, &desktop);
    assert!(began, "the climb sheet's companion starts its climb");
    let mut samples = vec![(0.0, world.save.creatures[0].clone())];
    let mut perched_at = None;
    for tick in 1..=600_u32 {
        now += Duration::milliseconds(50);
        world.tick(now, TICK, &desktop);
        let at = tick as f32 * TICK;
        let creature = &world.save.creatures[0];
        if creature.state.action == ActionKind::Perch && perched_at.is_none() {
            perched_at = Some(at);
        }
        if tick % every == 0 {
            samples.push((at, creature.clone()));
        }
        if perched_at.is_some_and(|perched| at >= perched + PERCH_SHOWN) {
            break;
        }
    }
    Climb { floor, samples }
}

/// What a sample is doing, in a word or two: a mantle by its frame, as its haul and its step in.
fn doing(creature: &Creature) -> String {
    let body = BodyPresentation::for_creature(creature);
    if body.clip == formiga_art::BodyClip::Gesture(Gesture::Crest) {
        return match body.frame {
            0 => "CREST REACH".into(),
            1 => "CREST PULL UP".into(),
            2 => "CREST KNEE".into(),
            3 => "CREST TURN".into(),
            _ => "CREST STAND".into(),
        };
    }
    if body.clip == formiga_art::BodyClip::Gesture(Gesture::Mantle) {
        return match body.frame {
            0 => "MANTLE HANG".into(),
            1 => "MANTLE PULL UP".into(),
            2 => "MANTLE CHEST".into(),
            3 => "MANTLE PUSH".into(),
            _ => "MANTLE STAND".into(),
        };
    }
    match creature.state.action {
        ActionKind::InspectScreen => "LOOK",
        ActionKind::Traverse => "WALK",
        ActionKind::ClimbWindow => "CLIMB",
        ActionKind::Perch => "PERCH",
        ActionKind::Dangle => "DANGLE",
        ActionKind::Idle => "IDLE",
        _ => "OTHER",
    }
    .into()
}

/// A sheet being drawn: its pixels and size.
struct Sheet {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
}

impl Sheet {
    fn text(&mut self, x: u32, y: u32, words: &str) {
        crate::social_preview::draw_text(
            &mut self.pixels,
            self.width as i32,
            self.height as i32,
            x as i32,
            y as i32,
            words,
            1,
            INK,
        );
    }

    /// A rectangle in sheet pixels, cut to the view `(left, top, width, height)`.
    fn rect_in(
        &mut self,
        view: (u32, u32, u32, u32),
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        color: [u8; 4],
    ) {
        let (left, top, width, height) = view;
        let x0 = x.max(left as i32);
        let y0 = y.max(top as i32);
        let x1 = (x + w).min((left + width) as i32);
        let y1 = (y + h).min((top + height) as i32);
        if x1 > x0 && y1 > y0 {
            fill_rect(
                &mut self.pixels,
                self.width,
                x0 as u32,
                y0 as u32,
                (x1 - x0) as u32,
                (y1 - y0) as u32,
                color,
            );
        }
    }

    /// A window drawn in a view whose top-left is the point `origin`: its face, a title bar, and
    /// a one-point edge.
    fn window(&mut self, view: (u32, u32, u32, u32), origin: Point, bounds: DesktopRect) {
        let at = |x: f32, y: f32| {
            (
                view.0 as i32 + ((x - origin.x) * UNIT as f32).round() as i32,
                view.1 as i32 + ((y - origin.y) * UNIT as f32).round() as i32,
            )
        };
        let (x0, y0) = at(bounds.x, bounds.y);
        let (x1, y1) = at(bounds.right(), bounds.y + bounds.height);
        let unit = UNIT as i32;
        self.rect_in(view, x0, y0, x1 - x0, y1 - y0, WINDOW_EDGE);
        self.rect_in(
            view,
            x0 + unit,
            y0 + unit,
            x1 - x0 - 2 * unit,
            y1 - y0 - 2 * unit,
            WINDOW_FACE,
        );
        self.rect_in(
            view,
            x0 + unit,
            y0 + unit,
            x1 - x0 - 2 * unit,
            10 * unit,
            WINDOW_TITLE,
        );
    }

    /// A companion drawn as the overlay draws it: its frame placed from its drawn contact point by
    /// [`FramePlacement`], in a view whose top-left is the point `origin`. The contact point is
    /// snapped to whole art pixels, the drawable grid of a half-resolution overlay.
    fn companion(&mut self, view: (u32, u32, u32, u32), origin: Point, creature: &Creature) {
        let body = BodyPresentation::for_creature(creature);
        let face = CreatureRenderer::resolve_face_state(creature, CursorSnapshot::default(), false);
        let picture = CreatureRenderer::render_composited_frame(
            &creature.appearance,
            body.clip,
            body.frame,
            body.facing_right,
            false,
            face,
        );
        let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
        let contact = creature.state.drawn_position(1.0);
        let drawn = FramePlacement::for_creature(creature, baseline).frame_rect(
            view.0 as f32 + (contact.x - origin.x).round() * UNIT as f32,
            view.1 as f32 + (contact.y - origin.y).round() * UNIT as f32,
            UNIT as f32,
        );
        self.frame(view, &picture, drawn);
    }

    /// A frame stretched over the rectangle it is drawn in, nearest neighbour, as the overlay's
    /// sampler draws it.
    fn frame(
        &mut self,
        view: (u32, u32, u32, u32),
        picture: &Canvas,
        drawn: formiga_art::FrameRect,
    ) {
        let left = drawn.centre_x - drawn.width / 2.0;
        let (x0, y0) = (left.round() as i32, drawn.top.round() as i32);
        let (w, h) = (drawn.width.round() as i32, drawn.height.round() as i32);
        for y in 0..h {
            for x in 0..w {
                let (px, py) = (x0 + x, y0 + y);
                if px < view.0 as i32
                    || py < view.1 as i32
                    || px >= (view.0 + view.2) as i32
                    || py >= (view.1 + view.3) as i32
                {
                    continue;
                }
                let pixel = picture.get(
                    x * FRAME_SIZE as i32 / w.max(1),
                    y * FRAME_SIZE as i32 / h.max(1),
                );
                if pixel.a > 0 {
                    crate::blend_pixel(
                        &mut self.pixels,
                        self.width,
                        px as u32,
                        py as u32,
                        [pixel.r, pixel.g, pixel.b, pixel.a],
                    );
                }
            }
        }
    }
}

/// A whole climb onto a window 120 points above the floor, for each body: the look, the walk to
/// the window's side, the climb, the haul, the step in and two seconds of perching, a tenth of a
/// second a picture at 2x, placed exactly as the overlay places a companion and with the window
/// behind it, the time and what it is doing under each. Under those, each body dangling from a
/// ledge, every frame, as the hang takes its weight at nothing, half and all of it.
pub fn climb_sheet(path: PathBuf, args: &[String]) -> Result<()> {
    let bodies = climbs_shown(args, TICKS_PER_SAMPLE)?;
    let view_w = ((VIEW_RIGHT - VIEW_LEFT) * UNIT as f32) as u32;
    let view_h = ((VIEW_ABOVE + RISE + VIEW_BELOW_FLOOR) * UNIT as f32) as u32;
    let rows_of = |climb: &Climb| (climb.samples.len() as u32).div_ceil(SAMPLES_PER_ROW);
    let climb_rows: u32 = bodies.iter().map(|(_, _, climb)| rows_of(climb)).sum();
    // A dangle is drawn one way: once for each body.
    let dangling: Vec<_> = bodies
        .iter()
        .filter(|(name, _, _)| !name.ends_with(BACK_LABEL))
        .collect();
    let dangle_frames =
        u32::from(formiga_art::AnimationSpec::for_action(ActionKind::Dangle).frames);
    let dangle_w = (DANGLE_WIDE * UNIT as f32) as u32;
    let dangle_h = ((DANGLE_ABOVE + DANGLE_BELOW) * UNIT as f32) as u32;
    let width = (LABEL + SAMPLES_PER_ROW * view_w)
        .max(LABEL + HANGING.len() as u32 * dangle_frames * dangle_w);
    let height = 20
        + climb_rows * (view_h + CAPTION + ROW_GAP)
        + 20
        + dangling.len() as u32 * (dangle_h + CAPTION + ROW_GAP);
    let mut sheet = Sheet {
        pixels: vec![0; (width * height * 4) as usize],
        width,
        height,
    };
    for chunk in sheet.pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&WALLPAPER);
    }
    sheet.text(
        6,
        6,
        "A CLIMB ONTO A WINDOW 120 POINTS UP, A TENTH OF A SECOND A PICTURE, PLACED AS THE OVERLAY PLACES IT",
    );
    let mut top = 20;
    for (name, _, climb) in &bodies {
        sheet.text(6, top + 6, name);
        let bounds = window_bounds(climb.floor);
        let origin = Point {
            x: bounds.x + VIEW_LEFT,
            y: bounds.y - VIEW_ABOVE,
        };
        for (index, (at, creature)) in climb.samples.iter().enumerate() {
            let column = index as u32 % SAMPLES_PER_ROW;
            let row = index as u32 / SAMPLES_PER_ROW;
            let view = (
                LABEL + column * view_w,
                top + row * (view_h + CAPTION + ROW_GAP),
                view_w,
                view_h,
            );
            // The floor, the window, and the companion over them.
            let floor_y = view.1 as i32 + ((climb.floor - origin.y) * UNIT as f32) as i32;
            sheet.rect_in(
                view,
                view.0 as i32,
                floor_y,
                view_w as i32,
                view_h as i32,
                FLOOR,
            );
            sheet.window(view, origin, bounds);
            sheet.companion(view, origin, creature);
            // A hairline between samples, so each reads as its own picture.
            sheet.rect_in(
                (view.0, view.1, view_w, view_h + CAPTION),
                (view.0 + view_w - 1) as i32,
                view.1 as i32,
                1,
                (view_h + CAPTION) as i32,
                [255, 255, 255, 255],
            );
            sheet.text(view.0 + 4, view.1 + view_h + 3, &format!("{at:.1}"));
            sheet.text(view.0 + 4, view.1 + view_h + 12, &doing(creature));
        }
        top += rows_of(climb) * (view_h + CAPTION + ROW_GAP);
    }
    sheet.text(
        6,
        top + 6,
        "DANGLING FROM A LEDGE, EVERY FRAME, AS THE HANG TAKES NONE, HALF AND ALL OF ITS WEIGHT",
    );
    top += 20;
    for (name, genome, climb) in dangling {
        sheet.text(6, top + 6, name.trim_end_matches(SIDE_LABEL));
        // Hanging from the window's top, partway along it.
        let ledge = Point {
            x: WINDOW_LEFT + 40.0,
            y: climb.floor - RISE,
        };
        let origin = Point {
            x: ledge.x - DANGLE_WIDE / 2.0,
            y: ledge.y - DANGLE_ABOVE,
        };
        let mut creature = climb.samples[0].1.clone();
        creature.appearance = genome.clone();
        creature.state.action = ActionKind::Dangle;
        creature.state.position = ledge;
        creature.state.depth = 0.0;
        creature.state.facing_right = true;
        creature.state.beat = None;
        creature.state.flourish = None;
        // Unmeasured, so its eyes open and shut on the strip's own clock rather than staying
        // wherever its blink was when the climb began.
        creature.state.presence = Presence::default();
        let frame_secs =
            1.0 / f32::from(formiga_art::AnimationSpec::for_action(ActionKind::Dangle).fps);
        let mut column = 0;
        for hanging in HANGING {
            creature.state.attention = Some(AttentionPose {
                // Looking along the ledge, as a companion that caught it looks to where it was
                // going, so a glance down at the drop shows as one.
                target: Point {
                    x: ledge.x + 48.0,
                    y: ledge.y - 28.0,
                },
                emotion: AttentionEmotion::Concerned,
                hanging,
                gesture: None,
            });
            for frame in 0..dangle_frames {
                creature.state.action_elapsed = (frame as f32 + 0.5) * frame_secs;
                let view = (LABEL + column * dangle_w, top, dangle_w, dangle_h);
                sheet.window(
                    view,
                    origin,
                    DesktopRect {
                        x: WINDOW_LEFT,
                        y: ledge.y,
                        width: 320.0,
                        height: RISE,
                    },
                );
                sheet.companion(view, origin, &creature);
                sheet.rect_in(
                    (view.0, view.1, dangle_w, dangle_h + CAPTION),
                    (view.0 + dangle_w - 1) as i32,
                    view.1 as i32,
                    1,
                    (dangle_h + CAPTION) as i32,
                    [255, 255, 255, 255],
                );
                let shown = BodyPresentation::for_creature(&creature);
                sheet.text(
                    view.0 + 4,
                    view.1 + dangle_h + 3,
                    &format!("HANG {hanging:.1} F{}", shown.frame),
                );
                column += 1;
            }
        }
        top += dangle_h + CAPTION + ROW_GAP;
    }
    write_png(&path, width, height, &sheet.pixels)?;
    println!("wrote {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every body on the sheet makes the whole climb, so no row of it is ever a companion
    /// standing about: it looks, walks, climbs, tops out — every frame of its mantle shown — and
    /// perches on the window's top. A body that can be drawn turned does it both ways, side-on
    /// in the climb's own clips and with its back to the screen in the two that show it so.
    #[test]
    fn every_climber_on_the_sheet_reaches_its_perch() {
        for (name, genome) in climbers() {
            let ways: &[bool] = if climbs_both_ways(&genome) {
                &[false, true]
            } else {
                &[false]
            };
            for &back in ways {
                let climb = climb_sampled(&genome, TICKS_PER_SAMPLE, back);
                let mantle_clip = formiga_art::BodyClip::Gesture(if back {
                    Gesture::Crest
                } else {
                    Gesture::Mantle
                });
                let climb_clip = if back {
                    formiga_art::BodyClip::Gesture(Gesture::Scale)
                } else {
                    formiga_art::BodyClip::Action(ActionKind::ClimbWindow)
                };
                let mut stages: Vec<ActionKind> = Vec::new();
                let mut mantle = Vec::new();
                let mut climbing = 0;
                for (_, creature) in &climb.samples {
                    if stages.last() != Some(&creature.state.action) {
                        stages.push(creature.state.action);
                    }
                    let body = BodyPresentation::for_creature(creature);
                    if body.clip == mantle_clip && mantle.last() != Some(&body.frame) {
                        mantle.push(body.frame);
                    }
                    climbing += u32::from(body.clip == climb_clip);
                    if creature.state.action == ActionKind::ClimbWindow {
                        assert!(
                            body.clip == climb_clip || body.clip == mantle_clip,
                            "{name} back {back}: {:?} on the wall",
                            body.clip
                        );
                    }
                }
                assert!(
                    climbing > 6,
                    "{name} back {back}: {climbing} samples of climbing"
                );
                assert_eq!(mantle, [0, 1, 2, 3, 4], "{name} back {back}");
                assert_eq!(
                    stages,
                    [
                        ActionKind::InspectScreen,
                        ActionKind::Traverse,
                        ActionKind::ClimbWindow,
                        ActionKind::Perch,
                    ],
                    "{name} back {back}"
                );
                let (_, last) = climb.samples.last().expect("samples");
                assert_eq!(
                    last.state.position.y,
                    climb.floor - RISE,
                    "{name} back {back}"
                );
            }
        }
    }
}
