use super::surfaces::supports_on;
use super::*;

const INSPECTION_RADIUS: f32 = 12.0;
pub(super) const MANTLE_LIFT_POINTS: f32 = 10.0;

/// How many frames a second a top-out onto a ledge is drawn at. The art plays its mantle at this
/// rate from the moment the climb tops out, so the haul and the step in below change over on the
/// frame the art does.
pub const MANTLE_FRAMES_PER_SECOND: u8 = 5;
/// How many of the mantle's frames the haul takes: its chest over the edge with a knee up, and the
/// push up on both paws. The step in onto the ledge is the third. The haul is the longer part: it
/// is the effort, and the step is the arrival.
pub const MANTLE_HAUL_FRAMES: u8 = 2;
/// How much of the haul's rise is made with the chest over the edge, before the push: a little,
/// as the knee comes up. The push makes the rest.
const HAUL_CHEST_RISE: f32 = 0.15;
/// How much of the push is made at once, as the paws plant and the pose changes to the push.
const HAUL_HEAVE: f32 = 0.6;

/// How many frames a second a climb up a window's side is drawn at. The art plays its climb at
/// this rate, so the rise below and the hands drawn keep step.
pub const CLIMB_FRAMES_PER_SECOND: u8 = 6;
/// A climb goes a hand at a time, and each hand's stroke is this many frames: a reach up, a pull
/// and a settle onto the new grip. The climb's clip is two strokes, one with each hand.
pub const CLIMB_STROKE_FRAMES: u8 = 3;
/// How much of a stroke's rise each of its frames takes: hardly any while the hand reaches up,
/// most of it on the pull, and a little as the body settles onto the new grip. Never none, so a
/// climber is never stood still on the wall — a journey that holds still is a pause, which a
/// scene may fill with a pose.
const CLIMB_STROKE_RISE: [f32; CLIMB_STROKE_FRAMES as usize] = [0.08, 1.0, 0.3];

/// How far a climber's paw on the window's side reaches past its frame's middle, in art pixels,
/// for the bodies that reach furthest: a long body's forepaw held out ahead of it, and a classic
/// hopper's arm. See [`climb_grip_reach`].
const CLIMB_REACH_LONG: f32 = 18.0;
/// The same for every other body: the paws go up beside the head.
const CLIMB_REACH: f32 = 16.0;
/// How far over a window's edge line a climber's paw on that side grips, in art pixels: its tip,
/// so the paw is drawn on the edge rather than beside it.
const CLIMB_GRIP_OVER_EDGE: f32 = 2.0;
/// How far inside a window's edge line the middle of a companion is once it has pushed itself up
/// onto the ledge, in art pixels: enough for its back paw, planted at the hip for the push, to be
/// on the ledge too.
const MANTLE_PUSH_OVER_EDGE: f32 = 10.0;
/// How far inside a window's edge line a climb's track ran before it went up the outside, in
/// points. Still where a companion climbs a window whose side has no room beside it: one against
/// the side of its display or of the habitat.
const CLIMB_INSIDE_TRACK: f32 = 5.0;

/// How far a companion wearing `appearance`, climbing a window's side facing it, holds its paw on
/// the window's side out past the middle of its frame, in art pixels. Every paw a climb draws on
/// that side goes no further than this and a little less than it — a climb's paws go up beside the
/// head, and a long body's forepaw out ahead of it — so a climb with the frame's middle this far
/// out from a window's edge line, less [`CLIMB_GRIP_OVER_EDGE`], grips the edge with its paw and
/// hangs beside the window rather than over it. This crate cannot see the art, so this
/// approximates it; `formiga_art`'s
/// `a_climbing_paw_reaches_as_far_as_the_simulation_climbs_by` holds it to the real frames.
pub fn climb_grip_reach(appearance: &AppearanceGenome) -> f32 {
    // A fox and a sculpted form draw in place of the body the design or family would.
    if appearance.echo_fox || appearance.sculpt.is_some() {
        return CLIMB_REACH;
    }
    match appearance.design.map(|design| design.body) {
        Some(BodyPlan::Long) => CLIMB_REACH_LONG,
        None if appearance.family == BodyFamily::Hopper => CLIMB_REACH_LONG,
        _ => CLIMB_REACH,
    }
}

#[derive(Clone)]
pub(super) struct HopJourney {
    pub(super) start: Point,
    pub(super) target: Point,
    pub(super) surface: SurfaceAttachment,
    pub(super) elapsed: f32,
    pub(super) duration: f32,
    /// How high above the straight line the middle of the hop goes, in points, besides the arc a
    /// hop onto a window ledge always has. Zero for every hop but getting down off a roof and a
    /// leap.
    pub(super) lift: f32,
    /// A leap off a ledge, which rises and falls the way anything thrown does: steadily across,
    /// slowing to the top and quickening all the way down, rather than easing into the landing.
    pub(super) ballistic: bool,
}

#[derive(Clone)]
pub(super) struct GapJourney {
    pub(super) source: WindowKey,
    pub(super) source_bounds: DesktopRect,
    pub(super) target_bounds: DesktopRect,
    pub(super) hop: HopJourney,
    pub(super) runup: Point,
    pub(super) preparation: f32,
    pub(super) elapsed: f32,
    pub(super) catch: bool,
    pub(super) helped: bool,
    pub(super) helper: Option<CreatureId>,
    pub(super) assistance_checked: bool,
    pub(super) assistance_slip: bool,
    pub(super) bridge: bool,
}

#[derive(Clone)]
pub(super) struct ClimbJourney {
    target_window: WindowKey,
    target_bounds: DesktopRect,
    pub(super) start: Point,
    pub(super) approach: Point,
    pub(super) climb_end: Point,
    /// Where across the companion's middle is once it has pushed itself up over the edge: past
    /// the edge line, with both its paws on the ledge, and no further in than `target`.
    pub(super) over_x: f32,
    pub(super) target: Point,
    surface: SurfaceAttachment,
    pub(super) elapsed: f32,
    pub(super) approach_duration: f32,
    pub(super) climb_duration: f32,
    pub(super) mantle_duration: f32,
}

#[derive(Clone)]
pub(super) struct SqueezeJourney {
    pub(super) from_window: WindowKey,
    pub(super) from_bounds: DesktopRect,
    pub(super) target_window: WindowKey,
    pub(super) target_bounds: DesktopRect,
    pub(super) start: Point,
    pub(super) target: Point,
    pub(super) surface: SurfaceAttachment,
    pub(super) elapsed: f32,
    pub(super) duration: f32,
}

#[derive(Clone)]
pub(super) struct WindowRoutePlan {
    pub(super) geometry_hash: u64,
    pub(super) remaining: VecDeque<TopologyRouteHop>,
    pub(super) repaired: bool,
}

#[derive(Clone)]
pub(super) enum WindowJourney {
    Gap(GapJourney),
    Hop(HopJourney),
    Climb(ClimbJourney),
    Squeeze(SqueezeJourney),
}

#[derive(Clone, Copy)]
pub(super) struct JourneyStep {
    pub(super) position: Point,
    pub(super) action: ActionKind,
    pub(super) complete: bool,
    /// A pose the journey strikes over its action, and where the creature looks while it does:
    /// the mantle at the top of a climb. None for every other step.
    pub(super) pose: Option<(Gesture, Point)>,
}

impl WindowJourney {
    pub(super) fn initial_action(&self) -> ActionKind {
        match self {
            Self::Gap(_) => ActionKind::InspectScreen,
            Self::Hop(_) => ActionKind::Landing,
            // A staircase step that pauses first looks at where it is going.
            Self::Climb(journey) if journey.elapsed < 0.0 => ActionKind::InspectScreen,
            Self::Climb(journey) if journey.approach_duration > 0.05 => ActionKind::Traverse,
            Self::Climb(_) => ActionKind::ClimbWindow,
            Self::Squeeze(_) => ActionKind::InspectScreen,
        }
    }

    pub(super) fn surface(&self) -> &SurfaceAttachment {
        match self {
            Self::Gap(journey) => &journey.hop.surface,
            Self::Hop(journey) => &journey.surface,
            Self::Climb(journey) => &journey.surface,
            Self::Squeeze(journey) => &journey.surface,
        }
    }

    pub(super) fn valid(&self, desktop: &DesktopSnapshot) -> bool {
        match self {
            Self::Gap(journey) => [
                (Some(journey.source), journey.source_bounds),
                (journey.hop.surface.window_key, journey.target_bounds),
            ]
            .into_iter()
            .filter(|(key, _)| {
                *key != Some(journey.source) || journey.elapsed < journey.preparation
            })
            .all(|(key, bounds)| {
                desktop
                    .windows
                    .iter()
                    .any(|w| Some(w.key) == key && w.visible && !w.minimized && w.bounds == bounds)
            }),
            Self::Hop(journey) => match journey.surface.window_key {
                Some(key) => desktop
                    .windows
                    .iter()
                    .any(|window| window.key == key && window.visible && !window.minimized),
                None => desktop.monitors.iter().any(|monitor| {
                    monitor.id == journey.surface.monitor_id
                        && monitor.usable_bounds.contains(journey.target)
                }),
            },
            Self::Climb(journey) => desktop.windows.iter().any(|window| {
                window.key == journey.target_window
                    && window.visible
                    && !window.minimized
                    && window.bounds == journey.target_bounds
            }),
            Self::Squeeze(journey) => {
                let from_valid = desktop.windows.iter().any(|window| {
                    window.key == journey.from_window
                        && window.visible
                        && !window.minimized
                        && window.bounds == journey.from_bounds
                });
                let target_valid = desktop.windows.iter().any(|window| {
                    window.key == journey.target_window
                        && window.visible
                        && !window.minimized
                        && window.bounds == journey.target_bounds
                });
                from_valid && target_valid
            }
        }
    }

    pub(super) fn advance(&mut self, dt: f32) -> JourneyStep {
        match self {
            Self::Gap(journey) => {
                journey.elapsed += dt;
                if journey.elapsed < journey.preparation {
                    if journey.bridge {
                        return JourneyStep {
                            position: journey.hop.start,
                            action: ActionKind::InspectScreen,
                            complete: false,
                            pose: None,
                        };
                    }
                    let progress = (journey.elapsed - 0.6) / (journey.preparation - 0.6);
                    let (position, action) = if progress < 0.0 {
                        (journey.hop.start, ActionKind::InspectScreen)
                    } else if progress < 0.65 {
                        (
                            lerp_point(
                                journey.hop.start,
                                journey.runup,
                                smoothstep(progress / 0.65),
                            ),
                            ActionKind::Traverse,
                        )
                    } else {
                        (
                            lerp_point(
                                journey.runup,
                                journey.hop.start,
                                smoothstep((progress - 0.65) / 0.35),
                            ),
                            ActionKind::Sprint,
                        )
                    };
                    JourneyStep {
                        position,
                        action,
                        complete: false,
                        pose: None,
                    }
                } else {
                    let progress = ((journey.elapsed - journey.preparation) / journey.hop.duration)
                        .clamp(0.0, 1.0);
                    let caught_for = journey.elapsed - journey.preparation - journey.hop.duration;
                    JourneyStep {
                        position: if journey.bridge {
                            lerp_point(journey.hop.start, journey.hop.target, progress)
                        } else {
                            gap_position(&journey.hop, progress)
                        },
                        action: if journey.catch && caught_for >= 0.0 {
                            if caught_for < 0.65 {
                                ActionKind::Dangle
                            } else {
                                ActionKind::ClimbWindow
                            }
                        } else if journey.bridge {
                            ActionKind::Traverse
                        } else {
                            ActionKind::Landing
                        },
                        complete: progress >= 1.0
                            && (!journey.catch
                                || caught_for >= if journey.helped { 1.7 } else { 2.4 }),
                        pose: None,
                    }
                }
            }
            Self::Hop(journey) => {
                journey.elapsed += dt;
                let progress = (journey.elapsed / journey.duration).clamp(0.0, 1.0);
                let arc = f32::from(journey.surface.kind == SurfaceKind::WindowLedge)
                    * (progress * std::f32::consts::PI).sin()
                    * journey
                        .start
                        .distance(journey.target)
                        .mul_add(0.12, 24.0)
                        .min(90.0);
                // A lifted hop eases its height in and out as the roof plan's own hops do; every
                // other hop keeps the straight line it always had. A leap's straight line and
                // its lift together are one parabola, which is the path of a throw.
                let (rise, lift) = if journey.ballistic {
                    (progress, journey.lift * 4.0 * progress * (1.0 - progress))
                } else if journey.lift > 0.0 {
                    (
                        progress * progress * (3.0 - 2.0 * progress),
                        journey.lift * 4.0 * progress * (1.0 - progress),
                    )
                } else {
                    (progress, 0.0)
                };
                JourneyStep {
                    position: Point {
                        x: lerp(journey.start.x, journey.target.x, progress),
                        y: lerp(journey.start.y, journey.target.y, rise) - arc - lift,
                    },
                    action: ActionKind::Landing,
                    complete: progress >= 1.0,
                    pose: None,
                }
            }
            Self::Climb(journey) => {
                journey.elapsed += dt;
                if journey.elapsed < 0.0 {
                    return JourneyStep {
                        position: journey.start,
                        action: ActionKind::InspectScreen,
                        complete: false,
                        pose: None,
                    };
                }
                let approach_end = journey.approach_duration;
                let climb_end = approach_end + journey.climb_duration;
                let total = climb_end + journey.mantle_duration;
                if journey.elapsed < approach_end {
                    let progress = (journey.elapsed / approach_end.max(0.001)).clamp(0.0, 1.0);
                    JourneyStep {
                        position: lerp_point(journey.start, journey.approach, smoothstep(progress)),
                        action: ActionKind::Traverse,
                        complete: false,
                        pose: None,
                    }
                } else if journey.elapsed < climb_end {
                    let progress =
                        climb_progress(journey.elapsed - approach_end, journey.climb_duration);
                    JourneyStep {
                        position: lerp_point(journey.approach, journey.climb_end, progress),
                        action: ActionKind::ClimbWindow,
                        complete: false,
                        pose: None,
                    }
                } else {
                    // Topping out is a mantle of its own, frame for frame with the art: the
                    // chest brought up to the edge with a knee up, straight up beside the window
                    // with the forearm laid over the edge; the push, which heaves the companion up
                    // and over the edge onto both paws on the ledge; and a stand with a little
                    // settle as it steps the rest of the way in. Rising and sliding in together
                    // all the way up used to drag a body hung over the window along its edge;
                    // now the body hangs beside the window and only the push crosses the edge,
                    // as a heave does. It is still a climb while it does.
                    let mantle = (journey.elapsed - climb_end).max(0.0);
                    let haul = mantle_haul_secs(journey.mantle_duration);
                    let over = Point {
                        x: journey.over_x,
                        y: journey.target.y,
                    };
                    let position = if mantle < haul {
                        let t = mantle / haul.max(0.001);
                        Point {
                            x: lerp(journey.climb_end.x, over.x, haul_across(t)),
                            y: lerp(journey.climb_end.y, over.y, haul_rise(t)),
                        }
                    } else {
                        let step = (journey.mantle_duration - haul).max(0.001);
                        // Carried in by the push, and coming to a stop on the ledge.
                        let progress = ((mantle - haul) / step).clamp(0.0, 1.0);
                        lerp_point(over, journey.target, 1.0 - (1.0 - progress).powi(2))
                    };
                    // Looking ahead along the ledge it is climbing onto.
                    let inward = if journey.target.x < journey.climb_end.x {
                        -1.0
                    } else {
                        1.0
                    };
                    let ahead = Point {
                        x: journey.target.x + inward * 48.0,
                        y: journey.target.y - 24.0,
                    };
                    JourneyStep {
                        position,
                        action: ActionKind::ClimbWindow,
                        complete: journey.elapsed >= total,
                        pose: Some((Gesture::Mantle, ahead)),
                    }
                }
            }
            Self::Squeeze(journey) => {
                journey.elapsed += dt;
                let progress =
                    ((journey.elapsed - 0.2) / (journey.duration - 0.4).max(0.1)).clamp(0.0, 1.0);
                JourneyStep {
                    position: lerp_point(journey.start, journey.target, smoothstep(progress)),
                    action: if journey.elapsed < 0.2 || journey.elapsed >= journey.duration - 0.2 {
                        ActionKind::InspectScreen
                    } else {
                        ActionKind::SqueezeWindow
                    },
                    complete: journey.elapsed >= journey.duration,
                    pose: None,
                }
            }
        }
    }

    /// Which way the journey has its creature face, where that is not simply the way it is
    /// going: up a window's side and over its edge, a climb faces the window, whichever way the
    /// walk to it went. A companion that started under the window walks out from under it to the
    /// side it climbs, and turns to the wall to climb it.
    pub(super) fn facing_right(&self) -> Option<bool> {
        match self {
            Self::Climb(journey) if journey.elapsed >= journey.approach_duration => {
                Some(journey.target.x >= journey.climb_end.x)
            }
            _ => None,
        }
    }

    /// How far into the action it is on now the journey is, in seconds, for an action it has
    /// just changed to. The climb counts from the moment it reached the wall, a tick's fraction
    /// before the tick that saw it, so the clip's frames (drawn from the action's own elapsed
    /// time) and the rise (shaped from the journey's) fall on the same instants; every other
    /// action starts from nothing.
    pub(super) fn action_elapsed(&self) -> f32 {
        match self {
            Self::Climb(journey) => {
                let into = journey.elapsed - journey.approach_duration;
                if (0.0..journey.climb_duration).contains(&into) {
                    into
                } else if into >= journey.climb_duration {
                    // The mantle counts from the moment the climb topped out.
                    into - journey.climb_duration
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }
}

/// How long the haul of a mantle lasting `mantle` seconds takes: its first
/// [`MANTLE_HAUL_FRAMES`] frames at [`MANTLE_FRAMES_PER_SECOND`], never more than the mantle.
fn mantle_haul_secs(mantle: f32) -> f32 {
    (f32::from(MANTLE_HAUL_FRAMES) / f32::from(MANTLE_FRAMES_PER_SECOND)).min(mantle)
}

/// How far across the edge the haul has carried a companion, from 0 to 1, `t` of the way through
/// it: not at all with its chest at the edge, its forearm already over it, and all of it on the
/// push, heaved over with the rise (see [`haul_rise`]), so the paws planted on the ledge for the
/// push stay where they were planted rather than sliding in with the body.
pub(super) fn haul_across(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        0.0
    } else {
        let push = (t - 0.5) / 0.5;
        HAUL_HEAVE + (1.0 - HAUL_HEAVE) * (1.0 - (1.0 - push).powi(2))
    }
}

/// How far up the haul a companion is, from 0 to 1, `t` of the way through it: a little with its
/// chest over the edge as a knee comes up, through the haul's first frame, and the rest on the
/// push through the second. The push is a heave: most of it is made the moment the paws plant
/// and the pose changes to it, and the rest eases up onto the ledge, so the paws drawn planted on
/// the ledge stay on it rather than the body sliding up past them for the length of the frame.
pub(super) fn haul_rise(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        HAUL_CHEST_RISE * smoothstep(t / 0.5)
    } else {
        let push = (t - 0.5) / 0.5;
        let heave = HAUL_HEAVE + (1.0 - HAUL_HEAVE) * (1.0 - (1.0 - push).powi(2));
        HAUL_CHEST_RISE + (1.0 - HAUL_CHEST_RISE) * heave
    }
}

/// How far up its climb a companion is, from 0 to 1, `into` seconds into a climb that lasts
/// `duration`: a hand at a time rather than a steady slide. Each stroke rises mostly on its pull
/// and holds almost still through the reach, frame for frame with the clip, which plays at
/// [`CLIMB_FRAMES_PER_SECOND`] from the start of the climb. The whole climb still takes
/// `duration` and ends where it did, so only the shape of the rise changes.
pub(super) fn climb_progress(into: f32, duration: f32) -> f32 {
    let total = climb_effort(duration);
    if total <= f32::EPSILON {
        return (into / duration.max(0.001)).clamp(0.0, 1.0);
    }
    (climb_effort(into.clamp(0.0, duration)) / total).clamp(0.0, 1.0)
}

/// The rise a climb has made `secs` into it, in strokes' worth of [`CLIMB_STROKE_RISE`].
fn climb_effort(secs: f32) -> f32 {
    let frames = secs.max(0.0) * f32::from(CLIMB_FRAMES_PER_SECOND);
    let per_stroke = f32::from(CLIMB_STROKE_FRAMES);
    let strokes = (frames / per_stroke).floor();
    let mut effort = strokes * CLIMB_STROKE_RISE.iter().sum::<f32>();
    let mut into = frames - strokes * per_stroke;
    for rise in CLIMB_STROKE_RISE {
        let part = into.clamp(0.0, 1.0);
        effort += rise * part;
        into -= part;
    }
    effort
}

/// Shows the pose a journey's step strikes, `into` seconds into it, for this tick: over a scene's
/// own pose if the creature is in one, and otherwise as a pose of its own, looking at `looking`.
/// The step stands on whatever it is climbing onto, so nothing of it hangs. Struck afresh on every
/// tick the journey carries it, and let go of at the start of the next (see
/// [`World::let_go_of_journey_poses`]), so a pose never outlasts the step that struck it.
pub(super) fn strike_journey_pose(
    creature: &mut Creature,
    gesture: Gesture,
    looking: Point,
    into: f32,
) {
    match &mut creature.state.attention {
        Some(pose) => {
            pose.gesture = Some(gesture);
            pose.hanging = 0.0;
        }
        None => {
            creature.state.attention = Some(AttentionPose {
                target: looking,
                emotion: AttentionEmotion::Curious,
                hanging: 0.0,
                gesture: Some(gesture),
            });
        }
    }
    // Drawn from the moment the pose began, as the journey counts it.
    creature.state.action_elapsed = into;
}

impl World {
    /// Lets go of every pose a journey struck last tick. A journey that carries its creature on
    /// strikes its pose again this tick; one that ended, was cut short or was taken over leaves
    /// its creature showing whatever it does next. A pose made for the journey alone goes with
    /// it, and a scene's own pose loses only the gesture.
    pub(super) fn let_go_of_journey_poses(&mut self) {
        for creature in &mut self.save.creatures {
            let Some(pose) = &mut creature.state.attention else {
                continue;
            };
            if pose.gesture != Some(Gesture::Mantle) {
                continue;
            }
            if self.attention.has_plan(creature.id) {
                pose.gesture = None;
            } else {
                creature.state.attention = None;
            }
        }
    }
}

pub(super) fn gap_position(hop: &HopJourney, progress: f32) -> Point {
    let mut point = lerp_point(hop.start, hop.target, progress);
    point.y -= (progress * std::f32::consts::PI).sin()
        * (hop.start.distance(hop.target) * 0.18 + 18.0).min(60.0);
    point
}

pub(super) fn gap_hanging(gap: &GapJourney) -> f32 {
    if !gap.catch {
        return 0.0;
    }
    let elapsed = gap.elapsed - gap.preparation - gap.hop.duration;
    if elapsed < 0.0 {
        0.0
    } else if elapsed < 0.2 {
        elapsed / 0.2
    } else if elapsed < 0.65 {
        1.0
    } else if gap.helped {
        (1.0 - (elapsed - 0.65) / 1.05).clamp(0.0, 1.0)
    } else if elapsed < 1.05 {
        1.0 - (elapsed - 0.65) * 0.7
    } else if elapsed < 1.35 {
        0.72 + (elapsed - 1.05) * 0.6
    } else {
        (0.9 * (1.0 - (elapsed - 1.35) / 1.05)).clamp(0.0, 1.0)
    }
}

pub(super) fn build_window_journey(
    creature: &Creature,
    target: Point,
    mut surface: SurfaceAttachment,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    display_scale: u8,
) -> WindowJourney {
    let start = creature.state.position;
    let upward = target.y < start.y;
    let Some(window) = upward
        .then_some(surface.window_key)
        .flatten()
        .and_then(|key| desktop.windows.iter().find(|window| window.key == key))
    else {
        let distance = start.distance(target);
        return WindowJourney::Hop(HopJourney {
            start,
            target,
            surface,
            elapsed: 0.0,
            duration: (distance / 280.0).clamp(1.0, 2.6),
            lift: 0.0,
            ballistic: false,
        });
    };

    // A climb goes up the outside of the window's side, the companion hung beside the window by
    // the paw gripping its edge, so that none of it is over the window until it tops out. Where
    // that is depends on how wide the companion is drawn, which is in art pixels, so it is
    // measured in them and turned into points for the display the window is on.
    let monitor = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.id == surface.monitor_id);
    let unit = spacing::frame_width(display_scale, monitor.map_or(1.0, |m| m.scale_factor))
        / CREATURE_ART_WIDTH;
    let regions = monitor.map(|monitor| accessible_regions(policy, monitor));
    let half = monitor.map_or(0.0, |monitor| body_half_width(monitor, display_scale));
    let hang = (climb_grip_reach(&creature.appearance) - CLIMB_GRIP_OVER_EDGE) * unit;
    // Stop with the body just below its final contact point, then haul it up and over in the
    // mantle. Previously the contact point reached the ledge before the horizontal pull began,
    // producing a visible hold-and-reposition hitch.
    let top_y = window.bounds.y + MANTLE_LIFT_POINTS;
    let track = |edge: f32, outward: f32| {
        climb_track(
            edge,
            outward,
            hang,
            [start.y, top_y],
            regions.as_deref().unwrap_or_default(),
            half,
        )
    };
    let left_track = track(window.bounds.x, -1.0);
    let right_track = track(window.bounds.right(), 1.0);
    let use_left = (start.x - left_track).abs() <= (start.x - right_track).abs();
    let (track_x, edge, inward) = if use_left {
        (left_track, window.bounds.x, 1.0)
    } else {
        (right_track, window.bounds.right(), -1.0)
    };
    let target_x = edge + inward * 18.0;
    // Over the edge far enough for both paws to be on the ledge for the push, but never past
    // where it is going to stand, nor back from where it climbed.
    let over_x = {
        let over = edge + inward * MANTLE_PUSH_OVER_EDGE * unit;
        if inward > 0.0 {
            over.min(target_x).max(track_x)
        } else {
            over.max(target_x).min(track_x)
        }
    };
    let approach = Point {
        x: track_x,
        y: start.y,
    };
    let climb_end = Point {
        x: track_x,
        y: top_y,
    };
    let target = Point {
        x: target_x,
        y: window.bounds.y,
    };
    surface.relative_x = ((target_x - window.bounds.x) / window.bounds.width).clamp(0.05, 0.95);
    let traverse_speed = 24.0 + creature.personality.activity * 34.0;
    let climb_speed = 44.0 + creature.personality.activity * 18.0;
    WindowJourney::Climb(ClimbJourney {
        target_window: window.key,
        target_bounds: window.bounds,
        start,
        approach,
        climb_end,
        over_x,
        target,
        surface,
        elapsed: 0.0,
        approach_duration: (start.distance(approach) / traverse_speed).clamp(0.0, 6.0),
        climb_duration: (approach.distance(climb_end) / climb_speed).max(0.35),
        mantle_duration: 0.7,
    })
}

/// Where across a climb up the side of a window whose edge line is at `edge` goes, `outward` -1
/// for its left side and 1 for its right: `hang` points out from the edge, beside the window, and
/// brought in as far as the whole companion needs, `half` its frame's half width, to be drawn on
/// the habitat's `regions` at each of the heights in `ys`. A side with no habitat beside it at all
/// — a window against the side of its display, or of the habitat — is climbed just inside the
/// edge line, as every climb once was.
fn climb_track(
    edge: f32,
    outward: f32,
    hang: f32,
    ys: [f32; 2],
    regions: &[DesktopRect],
    half: f32,
) -> f32 {
    let inside = edge - outward * CLIMB_INSIDE_TRACK;
    let outside = edge + outward * hang;
    let at = |y: f32| Point { x: outside, y };
    if !ys
        .iter()
        .all(|&y| regions.iter().any(|region| region.contains(at(y))))
    {
        return inside;
    }
    let kept = ys.map(|y| keep_whole_in(at(y), regions, half).x);
    if outward < 0.0 {
        kept[0].max(kept[1]).min(inside)
    } else {
        kept[0].min(kept[1]).max(inside)
    }
}

pub(super) fn build_route_hop_journey(
    creature: &Creature,
    hop: TopologyRouteHop,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    display_scale: u8,
) -> WindowJourney {
    let surface = SurfaceAttachment {
        kind: SurfaceKind::WindowLedge,
        monitor_id: hop.monitor_id,
        window_key: Some(hop.to_window),
        relative_x: ((hop.target.x - hop.to_bounds.x) / hop.to_bounds.width).clamp(0.05, 0.95),
    };
    match hop.kind {
        RouteHopKind::WindowTier => build_window_journey(
            creature,
            hop.target,
            surface,
            desktop,
            policy,
            display_scale,
        ),
        RouteHopKind::NarrowGap => WindowJourney::Squeeze(SqueezeJourney {
            from_window: hop.from_window,
            from_bounds: hop.from_bounds,
            target_window: hop.to_window,
            target_bounds: hop.to_bounds,
            start: creature.state.position,
            target: hop.target,
            surface,
            elapsed: 0.0,
            duration: (creature.state.position.distance(hop.target) / 52.0).clamp(0.7, 4.0),
        }),
    }
}

/// How much rise and drop one creature will take in a single staircase step, in logical points.
/// Temperament, energy, and learned climbing decide it; a mini reaches less far than an adult.
pub(super) fn traversal_ability(creature: &Creature) -> (f32, f32) {
    let ability = (creature.personality.boldness * 0.45
        + creature.personality.activity * 0.2
        + LearnedTendencies::utility(creature.tendencies.climbing).max(0.0) * 0.6
        + creature.state.drives.energy * 0.25
        - creature.state.drives.sleep_pressure * 0.2)
        .clamp(0.0, 1.0);
    let reach = if creature.role.is_adult() { 1.0 } else { 0.75 };
    (
        (160.0 + ability * 200.0) * reach,
        (240.0 + ability * 160.0) * reach,
    )
}

pub(super) fn planned_window_route(
    creature: &Creature,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    topology: &DesktopTopology,
    familiar: Option<Point>,
) -> Vec<TopologyRouteHop> {
    let Some(start_window) = creature.state.surface.window_key else {
        return Vec::new();
    };
    let preferred = creature
        .memory
        .preferred_region
        .filter(|p| p.confidence > 0)
        .and_then(|preferred| {
            let monitor = desktop
                .monitors
                .iter()
                .find(|monitor| monitor.display_key == preferred.display)?;
            let column = f32::from(preferred.cell.min(8) % 3);
            let row = f32::from(preferred.cell.min(8) / 3);
            Some(Point {
                x: monitor.usable_bounds.x + monitor.usable_bounds.width * ((column + 0.5) / 3.0),
                y: monitor.usable_bounds.y + monitor.usable_bounds.height * ((row + 0.5) / 3.0),
            })
        });
    let target_hint = topology
        .invitation()
        .filter(|invitation| cursor_invitation_eligible(creature, *invitation))
        .map(|invitation| invitation.point)
        .or(familiar)
        .or(preferred);
    let (max_rise, max_drop) = traversal_ability(creature);
    let route = topology.plan_route(
        start_window,
        RoutePreferences {
            climbing: creature.tendencies.climbing.round() as i8,
            exploration: creature.tendencies.exploration.round() as i8,
            cursor_trust: creature.tendencies.cursor_trust.round() as i8,
            target_hint,
            max_rise,
            max_drop,
        },
    );
    if route.iter().all(|hop| {
        desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == hop.monitor_id)
            .is_some_and(|monitor| habitat_contains(policy, monitor, hop.target))
    }) {
        route
    } else {
        Vec::new()
    }
}

pub(super) fn settle_interrupted_journey(
    creature: &mut Creature,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    display_scale: u8,
    events: &mut Vec<WorldEvent>,
) {
    let support = find_drop_support(
        creature.state.position,
        desktop,
        policy,
        true,
        display_scale,
    )
    .or_else(|| {
        nearest_habitat_point(
            policy,
            &desktop.monitors,
            creature.state.position,
            display_scale,
        )
        .map(|(monitor_id, position)| {
            (
                position,
                SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id,
                    window_key: None,
                    relative_x: 0.5,
                },
            )
        })
    });
    if let Some((position, surface)) = support {
        creature.state.position = position;
        creature.state.surface = surface.clone();
        creature.state.action = ActionKind::ReactToWindow;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 2.2;
        creature.state.velocity = Point::default();
        World::emit(
            events,
            WorldEvent::SurfaceChanged {
                creature_id: creature.id,
                kind: surface.kind,
            },
        );
        World::emit(
            events,
            WorldEvent::WindowReaction {
                creature_id: creature.id,
                action: ActionKind::ReactToWindow,
            },
        );
    }
}

pub(super) fn crossed_inspection_anchor(
    creature: &Creature,
    previous_x: f32,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
) -> bool {
    let path_min = previous_x.min(creature.state.position.x) - INSPECTION_RADIUS;
    let path_max = previous_x.max(creature.state.position.x) + INSPECTION_RADIUS;
    if let Some(key) = creature.state.surface.window_key
        && let Some(window) = desktop
            .windows
            .iter()
            .find(|window| window.key == key && window.visible && !window.minimized)
    {
        return [1.0_f32 / 3.0, 2.0 / 3.0]
            .into_iter()
            .map(|fraction| window.bounds.x + window.bounds.width * fraction)
            .any(|anchor| (path_min..=path_max).contains(&anchor));
    }

    let Some(monitor) = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.id == creature.state.surface.monitor_id)
    else {
        return false;
    };
    accessible_regions(policy, monitor)
        .into_iter()
        .filter(|region| {
            (creature.state.position.y - (region.bottom() - 4.0)).abs() <= INSPECTION_RADIUS
        })
        .flat_map(|region| {
            [1.0_f32 / 3.0, 2.0 / 3.0].map(move |fraction| region.x + region.width * fraction)
        })
        .any(|anchor| (path_min..=path_max).contains(&anchor))
}

/// The shortest climb worth making, in points.
const MIN_CLIMB: f32 = 36.0;
/// How far to either side a companion on the floor will go to climb a window, in points.
const FLOOR_REACH_ACROSS: f32 = 420.0;
/// How much of its own height a companion needs clear above a ledge, below the top of the
/// screen, to sit on it without its head going under the menu bar.
const LEDGE_HEADROOM_FRAMES: f32 = 0.85;

pub(super) fn find_nearby_ledge(
    creature: &Creature,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    topology: &DesktopTopology,
    display_scale: u8,
) -> Option<(Point, SurfaceAttachment)> {
    let current_window = creature.state.surface.window_key;
    let on_the_floor = creature.state.surface.kind == SurfaceKind::ScreenFloor;
    let candidate = topology
        .windows()
        .iter()
        .filter(|window| window.bounds.width >= 120.0)
        .filter(|window| Some(window.key) != current_window)
        .filter_map(|window| {
            let ledge_x = creature
                .state
                .position
                .x
                .clamp(window.bounds.x + 12.0, window.bounds.right() - 12.0);
            let dx = (creature.state.position.x - ledge_x).abs();
            let dy = (creature.state.position.y - window.bounds.y).abs();
            let monitor = desktop.monitors.iter().find(|monitor| {
                monitor.bounds.contains(Point {
                    x: ledge_x,
                    y: window.bounds.y,
                })
            })?;
            // A window can run past the side of its display or of the habitat: the spot on it
            // worth climbing to is one where the whole companion is still drawn, and still on
            // the window.
            let ledge_x = keep_whole_in(
                Point {
                    x: ledge_x,
                    y: window.bounds.y,
                },
                &accessible_regions(policy, monitor),
                body_half_width(monitor, display_scale),
            )
            .x;
            if !(window.bounds.x + 12.0..=window.bounds.right() - 12.0).contains(&ledge_x) {
                return None;
            }
            // From the floor, any window is worth climbing however tall it stands, so long as
            // a companion sitting on top of it still fits below the top of the screen: a big
            // window is most of what a desktop has to climb. From one ledge to another the reach
            // stays a single staircase step.
            let frame = spacing::frame_width(display_scale, monitor.scale_factor);
            let fits_below_the_top =
                window.bounds.y - frame * LEDGE_HEADROOM_FRAMES >= monitor.usable_bounds.y;
            let rise = creature.state.position.y - window.bounds.y;
            let within_reach = if on_the_floor {
                dx <= FLOOR_REACH_ACROSS && rise >= MIN_CLIMB
            } else {
                dx <= 360.0 && (MIN_CLIMB..=640.0).contains(&dy)
            };
            let reachable = within_reach
                && fits_below_the_top
                && habitat_contains(
                    policy,
                    monitor,
                    Point {
                        x: ledge_x,
                        y: window.bounds.y,
                    },
                );
            let island_bonus = if topology.island_windows().any(|key| key == window.key) {
                42.0 + creature.tendencies.exploration.max(0.0) * 0.2
            } else {
                0.0
            };
            // Nearby intermediate ledges remain easiest. Isolated window islands become slightly
            // more attractive to curious creatures without bypassing reachability or habitat.
            reachable.then_some((
                dx * 0.65 + dy * 0.12
                    - island_bonus
                    - if creature.personality.window_tolerance > 0.7
                        && topology.is_slow_platform(window.key)
                    {
                        24.0
                    } else {
                        0.0
                    },
                window,
                ledge_x,
                monitor.id,
            ))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    candidate.map(|(_, window, ledge_x, monitor_id)| {
        (
            Point {
                x: ledge_x,
                y: window.bounds.y,
            },
            SurfaceAttachment {
                kind: SurfaceKind::WindowLedge,
                monitor_id,
                window_key: Some(window.key),
                relative_x: ((ledge_x - window.bounds.x) / window.bounds.width).clamp(0.05, 0.95),
            },
        )
    })
}

pub(super) fn topology_ledge_at_target(
    topology: &DesktopTopology,
    target: Point,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
) -> Option<(Point, SurfaceAttachment)> {
    let window = topology
        .windows()
        .iter()
        .filter(|window| {
            target.x >= window.bounds.x + 12.0
                && target.x <= window.bounds.right() - 12.0
                && (target.y - window.bounds.y).abs() <= 24.0
        })
        .min_by(|a, b| {
            (target.y - a.bounds.y)
                .abs()
                .total_cmp(&(target.y - b.bounds.y).abs())
        })?;
    let point = Point {
        x: target
            .x
            .clamp(window.bounds.x + 12.0, window.bounds.right() - 12.0),
        y: window.bounds.y,
    };
    let monitor = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.id == window.monitor_id)?;
    habitat_contains(policy, monitor, point).then_some((
        point,
        SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: window.monitor_id,
            window_key: Some(window.key),
            relative_x: ((point.x - window.bounds.x) / window.bounds.width).clamp(0.05, 0.95),
        },
    ))
}

pub(super) fn cursor_invitation_eligible(
    creature: &Creature,
    invitation: CursorInvitation,
) -> bool {
    creature.state.surface.monitor_id == invitation.monitor_id
        && creature.state.cursor_cooldown <= 0.0
        && creature.tendencies.cursor_trust >= -20.0
        && (creature.tendencies.cursor_trust >= 10.0
            || creature.personality.cursor_interest + creature.personality.boldness >= 1.15)
}

pub(super) fn constrain_to_surface(
    creature: &mut Creature,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    display_scale: u8,
) -> bool {
    if let Some(key) = creature.state.surface.window_key
        && let Some(window) = desktop.windows.iter().find(|window| window.key == key)
    {
        let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == creature.state.surface.monitor_id)
        else {
            return false;
        };
        // A ledge ends at the window's own corners, and at the sides of the display or of the
        // habitat it runs past: there the whole companion has to stay inside.
        let half = body_half_width(monitor, display_scale);
        let intervals: Vec<_> = accessible_regions(policy, monitor)
            .into_iter()
            .filter(|region| window.bounds.y >= region.y && window.bounds.y <= region.bottom())
            .filter_map(|region| {
                let (low, high) = standing_span(region.x, region.right(), half);
                let min = (window.bounds.x + 12.0).max(low);
                let max = (window.bounds.right() - 12.0).min(high);
                (max > min).then_some((min, max))
            })
            .collect();
        let Some((min_x, max_x)) = intervals.iter().copied().min_by(|a, b| {
            distance_to_interval(creature.state.position.x, *a)
                .total_cmp(&distance_to_interval(creature.state.position.x, *b))
        }) else {
            return false;
        };
        // A creature held against either end of its ledge has gone as far as this surface goes.
        // Turning it round is not enough on its own: left still walking outward it would step
        // over the edge and be put back on every tick, which reads as a shiver rather than as a
        // creature meeting a wall. Whoever called us decides what it does instead.
        let mut held = false;
        if creature.state.position.x <= min_x {
            creature.state.position.x = min_x;
            creature.state.facing_right = true;
            held = true;
        } else if creature.state.position.x >= max_x {
            creature.state.position.x = max_x;
            creature.state.facing_right = false;
            held = true;
        }
        if held {
            creature.state.velocity.x = 0.0;
        }
        creature.state.position.y = window.bounds.y;
        creature.state.surface.relative_x =
            ((creature.state.position.x - window.bounds.x) / window.bounds.width).clamp(0.05, 0.95);
        return held;
    }
    if creature.state.surface.kind == SurfaceKind::ScreenFloor
        && let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == creature.state.surface.monitor_id)
    {
        let regions = accessible_regions(policy, monitor);
        let half = body_half_width(monitor, display_scale);
        let span = |region: &DesktopRect| standing_span(region.x, region.right(), half);
        if let Some(region) = regions.iter().min_by(|a, b| {
            distance_to_interval(creature.state.position.x, span(a))
                .total_cmp(&distance_to_interval(creature.state.position.x, span(b)))
        }) {
            // The same rule on the floor: the ends of the ground a creature is allowed on are
            // walls, half a body in so none of it is cut off, and walking into one is arriving,
            // not a reason to keep pushing.
            let (low, high) = span(region);
            let held = creature.state.position.x <= low || creature.state.position.x >= high;
            if held {
                creature.state.facing_right = creature.state.position.x <= low;
                creature.state.velocity.x = 0.0;
            }
            creature.state.position.x = creature.state.position.x.clamp(low, high);
            creature.state.position.y = region.bottom() - 4.0;
            creature.state.surface.relative_x =
                ((creature.state.position.x - region.x) / region.width).clamp(0.0, 1.0);
            return held;
        }
    }
    false
}

/// Where a creature let go at `cursor` comes to rest: the nearest ledge or habitat floor at or
/// below it, measured as the crow flies.
///
/// A drop is a straight fall, but not a narrow one: a creature released just past the end of a
/// ledge slides onto its corner rather than carrying on to the ground, so the whole monitor is
/// in play and plain distance decides. The monitor is the one under the cursor, because that is
/// the desk the person is pointing at.
pub(super) fn find_drop_support(
    cursor: Point,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    window_ledges: bool,
    display_scale: u8,
) -> Option<(Point, SurfaceAttachment)> {
    let monitor = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.bounds.contains(cursor))
        .or_else(|| desktop.monitors.iter().find(|monitor| monitor.primary))
        .or_else(|| desktop.monitors.first())?;
    let regions = accessible_regions(policy, monitor);
    let windows = if window_ledges {
        desktop.windows.as_slice()
    } else {
        &[]
    };
    let half = body_half_width(monitor, display_scale);
    supports_on(windows, &regions, monitor.id)
        .filter(|span| span.y >= cursor.y)
        .filter_map(|span| {
            let (point, _) = span.place(span.nearest_x(cursor.x));
            if !regions.iter().any(|region| region.contains(point)) {
                return None;
            }
            // Landing whole: a creature let go by the side of the display or of the habitat
            // comes down far enough in to be drawn in full, if this surface reaches that far.
            let x = keep_whole_in(point, &regions, half).x;
            span.holds(x).then(|| span.place(x))
        })
        .min_by(|a, b| cursor.distance(a.0).total_cmp(&cursor.distance(b.0)))
}

impl World {
    /// Starts one companion up onto the ledge of a window the way it climbs one by itself: a look
    /// at the window first, as a staircase step takes, a walk to the nearer of its sides, the
    /// climb, the haul and the step in, and then a perch. The climb sheet uses it to show a whole
    /// climb on demand; the colony itself never calls it. Returns whether the climb began, which
    /// needs the companion and the window both there and the window's top above the companion.
    pub fn begin_window_climb(
        &mut self,
        creature_id: CreatureId,
        window_key: WindowKey,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let Some(window) = desktop
            .windows
            .iter()
            .find(|window| window.key == window_key && window.visible && !window.minimized)
        else {
            return false;
        };
        let top_middle = Point {
            x: window.bounds.x + window.bounds.width / 2.0,
            y: window.bounds.y,
        };
        let Some(monitor_id) = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.bounds.contains(top_middle))
            .map(|monitor| monitor.id)
        else {
            return false;
        };
        let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == creature_id)
        else {
            return false;
        };
        if window.bounds.y >= creature.state.position.y {
            return false;
        }
        let surface = SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id,
            window_key: Some(window_key),
            relative_x: 0.5,
        };
        let target = Point {
            x: creature.state.position.x,
            y: window.bounds.y,
        };
        let mut journey = build_window_journey(
            creature,
            target,
            surface,
            desktop,
            &self.save.settings.habitat,
            self.save.settings.display_scale,
        );
        if let WindowJourney::Climb(climb) = &mut journey {
            climb.elapsed = -0.3;
        }
        let action = journey.initial_action();
        creature.state.action = action;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = f32::MAX;
        creature.state.velocity = Point::default();
        self.window_routes.remove(&creature_id);
        self.window_journeys.insert(creature_id, journey);
        Self::emit(
            &mut self.events,
            WorldEvent::ActionStarted {
                creature_id,
                action,
            },
        );
        true
    }
}
