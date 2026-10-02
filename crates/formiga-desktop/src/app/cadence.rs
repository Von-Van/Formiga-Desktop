//! How often the colony is ticked and drawn: twenty times a second while anything moves — a stroll
//! round the village included — and as little as the poses on show allow when it is still.
use super::*;

pub(super) fn world_needs_frequent_window_scan(world: &World, button_down: bool) -> bool {
    if world.save.settings.paused {
        return false;
    }
    // A fresh window list every quarter second matters only while a window could actually be
    // moving under a creature. A creature already riding, climbing, landing, squeezing, or being
    // carried needs it throughout. One simply standing on a ledge needs it only while the mouse
    // button is held, because that is the only way a window gets dragged out from under it; the
    // ordinary once-a-second scan notices everything else. A creature on the floor is carried by
    // no window at all. Scanning four times a second whenever anyone stood on a ledge meant
    // scanning four times a second nearly always, and the window list is the most expensive thing
    // this app asks the system for.
    world.save.creatures.iter().any(|creature| {
        (button_down && creature.state.surface.window_key.is_some())
            || matches!(
                creature.state.action,
                ActionKind::SqueezeWindow
                    | ActionKind::RideWindow
                    | ActionKind::Dragged
                    | ActionKind::Landing
                    | ActionKind::ClimbWindow
                    | ActionKind::Tossed
            )
    })
}

pub(super) fn world_has_spatial_motion(world: &World) -> bool {
    if world.save.settings.paused {
        return false;
    }
    // A wonder is out for half a minute at most, every ten minutes or more, and everything on it
    // moves: a seesaw tipping, a hammock swaying, its players hopping on and off.
    if world.wonder().is_some() {
        return true;
    }
    world.save.creatures.iter().any(|creature| {
        creature.state.velocity.x.abs() > 0.1
            || creature.state.velocity.y.abs() > 0.1
            || matches!(
                creature.state.action,
                ActionKind::Traverse
                    | ActionKind::SqueezeWindow
                    | ActionKind::Sprint
                    | ActionKind::InvestigateCursor
                    | ActionKind::AvoidCursor
                    | ActionKind::ReactToWindow
                    | ActionKind::Follow
                    | ActionKind::Dragged
                    | ActionKind::Landing
                    | ActionKind::ClimbWindow
                    | ActionKind::Tossed
            )
    })
}

/// A colony that is moving is ticked and drawn at 20 Hz, a stroll round the village as much as
/// anything else. From 0.59.2 until 0.62.0 a stroll at home ran at 10 Hz to save frames, which
/// left the village visibly steppier than the desktop around it.
const MOVING_INTERVAL: Duration = Duration::from_millis(50);

pub(super) fn world_redraw_interval(world: &World) -> Duration {
    if world.is_interacting() {
        return Duration::from_millis(50);
    }
    if world.save.settings.paused {
        return Duration::from_secs(1);
    }
    // The simulation itself advances at 20 Hz, so presenting faster would only repeat identical
    // positions. Pose-only activities follow their authored atlas frame rate instead.
    if world_has_spatial_motion(world) {
        return MOVING_INTERVAL;
    }
    let fps = world
        .save
        .creatures
        .iter()
        .filter(|creature| creature.state.arrival_delay_secs <= 0.0)
        .map(|creature| AnimationSpec::for_clip(BodyPresentation::for_creature(creature).clip).fps)
        .max()
        .unwrap_or(2)
        .max(1);
    Duration::from_secs_f32(1.0 / f32::from(fps))
}

/// When the next frame is due, given when it was due and when the last one was asked for, now
/// that the colony wants one every `interval`.
///
/// A colony at rest is drawn only as often as its slowest-moving poses need — twice a second for
/// residents resting at home. When one of them sets off, its ticks speed up to twenty a second at
/// once, and so must its frames: left on the resting schedule, the next frame could be up to half
/// a second away, and the walk would happen unseen in between — a companion that looks frozen and
/// then reappears a body's width along. So a frame is never further than one interval, at the
/// colony's current rate, from the last.
pub(super) fn frame_due(due: Instant, last: Option<Instant>, interval: Duration) -> Instant {
    last.map_or(due, |last| due.min(last + interval))
}

pub(super) fn world_tick_interval(world: &World) -> Duration {
    if world.is_interacting() {
        return Duration::from_millis(50);
    }
    if world.save.settings.paused {
        return Duration::from_millis(250);
    }
    if world_has_spatial_motion(world) {
        return MOVING_INTERVAL;
    }
    let has_expressive_action = world.save.creatures.iter().any(|creature| {
        creature.state.arrival_delay_secs <= 0.0
            && AnimationSpec::for_clip(BodyPresentation::for_creature(creature).clip).fps >= 8
    });
    let needs_responsive_gaze =
        world.save.settings.cursor_reactions
            && world.save.creatures.iter().any(|creature| {
                matches!(creature.state.action, ActionKind::Idle | ActionKind::Perch)
            });
    // Interaction proxies only refresh their native hit region on a tick, so a resting creature
    // still needs a responsive cadence to feel grabbable. Homebound creatures belong here too:
    // they are the stillest state in the simulation and the one users reach for at the shelter.
    let needs_responsive_grab = world.save.settings.direct_manipulation
        && world.save.creatures.iter().any(|creature| {
            matches!(
                creature.state.action,
                ActionKind::Idle | ActionKind::Perch | ActionKind::Homebound
            )
        });
    if has_expressive_action || needs_responsive_gaze || needs_responsive_grab {
        Duration::from_millis(100)
    } else {
        Duration::from_millis(200)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::{DesktopRect, DesktopSnapshot, DisplayKey, MonitorInfo, Point};
    use time::macros::datetime;

    /// The biggest move any companion makes between two frames the app would present, over
    /// `minutes` of a colony of six living at home on `desktop`, ticked and drawn exactly as the app
    /// schedules it. Hops and tosses travel in arcs and are left out.
    fn largest_step_between_frames(seed: u8, desktop: &DesktopSnapshot, minutes: f64) -> f32 {
        let start = datetime!(2026-05-02 10:00 UTC);
        let mut world = World::new([seed; 32], start, desktop);
        for extra in 0..5u8 {
            let _ =
                world.add_designed_adult([seed.wrapping_add(40 + extra); 32], None, start, desktop);
        }
        let base = Instant::now();
        let mut last_tick = base;
        let mut redraw_due = base;
        let mut last_frame = None;
        let mut shown: Option<Vec<(Point, ActionKind)>> = None;
        let mut largest = 0.0f32;
        while last_tick.duration_since(base).as_secs_f64() < minutes * 60.0 {
            let now = last_tick + world_tick_interval(&world);
            let dt = now.duration_since(last_tick).as_secs_f32().min(0.2);
            last_tick = now;
            if !world.save.home.is_active() {
                world.handle_command(WorldCommand::SendHome, desktop);
            }
            world.tick(
                start + time::Duration::seconds_f64(now.duration_since(base).as_secs_f64()),
                dt,
                desktop,
            );
            world.drain_events().for_each(drop);
            let interval = world_redraw_interval(&world);
            redraw_due = frame_due(redraw_due, last_frame, interval);
            if now >= redraw_due {
                last_frame = Some(now);
                let frame: Vec<(Point, ActionKind)> = world
                    .save
                    .creatures
                    .iter()
                    .map(|creature| (creature.state.position, creature.state.action))
                    .collect();
                if let Some(before) = &shown
                    && before.len() == frame.len()
                {
                    let arc = |action: ActionKind| {
                        matches!(
                            action,
                            ActionKind::Landing | ActionKind::Tossed | ActionKind::Dragged
                        )
                    };
                    for (creature, ((was, was_doing), (is, doing))) in
                        world.save.creatures.iter().zip(before.iter().zip(&frame))
                    {
                        if !arc(*was_doing) && !arc(*doing) && !creature.state.indoors {
                            largest = largest.max(was.distance(*is));
                        }
                    }
                }
                shown = Some(frame);
                let phased = redraw_due + interval;
                redraw_due = if phased > now { phased } else { now + interval };
            }
        }
        largest
    }

    /// A resident setting off from rest at home is drawn from its first step, not half a second
    /// later a body's width along: between any two frames nobody walks further than a few points.
    /// Before 0.66.3 the frame after a resting spell could be half a second late, and a companion
    /// stepping aside looked frozen and then jumped up to 26 points.
    #[test]
    fn a_resident_setting_off_from_rest_is_drawn_from_its_first_step() {
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
                    height: 826.0,
                },
                scale_factor: 2.0,
                primary: true,
            }],
            ..DesktopSnapshot::default()
        };
        let largest = largest_step_between_frames(1, &desktop, 20.0);
        assert!(
            largest <= 10.0,
            "a companion moved {largest:.1} points between two frames"
        );
    }
}
