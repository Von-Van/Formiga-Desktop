//! How often the colony is ticked and drawn: twenty times a second while anything moves, ten while
//! the only movement is a stroll at home, and as little as the poses on show allow when it is
//! still.
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

/// Below this, in points a second, movement at home is a stroll round the village. A stroll
/// steps under two and a half points a tick even at 10 Hz, and its walk cycle runs at less than
/// half speed to match, so the colony is ticked and drawn at 10 Hz while nothing moves faster.
///
/// It is the stroll's own ceiling — the briskest walk any companion could have, at a stroll's
/// pace — plus a whisker of slack, because velocity is measured from the step a companion
/// actually took and the briskest stroll therefore lands on the ceiling rather than under it. A
/// round number here quietly excluded the liveliest companions: at 24 points a second, a colony
/// with one member of high spirits strolled at 24.8 and the whole village was drawn twice as
/// often as it needed to be, which is most of what a strolling village cost.
pub(super) const STROLL_TICK_SPEED: f32 = MAX_STROLL_SPEED + 0.5;

/// The colony is at home and everyone moving is only strolling round the commons.
pub(super) fn world_moves_only_at_a_stroll(world: &World) -> bool {
    world.save.home.is_active()
        && world.save.creatures.iter().all(|creature| {
            creature.state.velocity.x.abs() < STROLL_TICK_SPEED
                && creature.state.velocity.y.abs() <= 0.1
        })
}

/// How often a colony that is moving needs a tick and a frame: 20 Hz, or 10 while the only thing
/// moving is a stroll at home.
pub(super) fn moving_interval(world: &World) -> Duration {
    if world_moves_only_at_a_stroll(world) {
        Duration::from_millis(100)
    } else {
        Duration::from_millis(50)
    }
}

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
        return moving_interval(world);
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

pub(super) fn world_tick_interval(world: &World) -> Duration {
    if world.is_interacting() {
        return Duration::from_millis(50);
    }
    if world.save.settings.paused {
        return Duration::from_millis(250);
    }
    if world_has_spatial_motion(world) {
        return moving_interval(world);
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
