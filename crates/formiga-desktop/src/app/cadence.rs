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
