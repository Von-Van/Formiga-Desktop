//! Short, habitat-safe approaches. These never change supporting surfaces or plan jumps.
use super::*;

pub(super) const MAX_APPROACH_SECONDS: f32 = 3.0;
/// A whole play scene may travel for longer than a single approach, but never indefinitely.
pub(super) const MAX_PLAY_TRAVEL: f32 = 12.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct ShortWalk {
    pub destination: Point,
    pub watched_bounds: Option<DesktopRect>,
    /// Multiplies the spacing kept from companions. Play comes closer than an audience does.
    pub spacing_factor: f32,
}

impl ShortWalk {
    pub fn new(destination: Point) -> Self {
        Self {
            destination,
            watched_bounds: None,
            spacing_factor: 1.0,
        }
    }

    pub fn watching(destination: Point, bounds: DesktopRect) -> Self {
        Self {
            destination,
            watched_bounds: Some(bounds),
            spacing_factor: 1.0,
        }
    }

    pub fn playing(destination: Point) -> Self {
        Self {
            destination,
            watched_bounds: None,
            spacing_factor: PLAY_SPACING,
        }
    }
}

/// Creature-scale contact spacing for play, as a fraction of ordinary personal space. Play is
/// allowed inside the face-clear distance because its contact is momentary — a tag, a vault, a
/// hand-off. Anything a game then *holds* is spaced by the face-clear distance where it is chosen.
pub(super) const PLAY_SPACING: f32 = 0.37;

pub(super) fn speed(creature: &Creature) -> f32 {
    24.0 + creature.personality.activity * 34.0
}

/// Ordinary personal space: shoulder to shoulder, so an approach that ends here leaves the two
/// faces clear of each other. A flat ceiling used to cap this below half a frame at the largest
/// display scale, which left a companion's face behind whoever walked up to it.
fn spacing(creature: &Creature, settings: &Settings, desktop: &DesktopSnapshot) -> f32 {
    super::super::spacing::face_clear_gap(creature, settings.display_scale, desktop)
}

fn same_surface(a: &Creature, b: &Creature) -> bool {
    a.state.surface.monitor_id == b.state.surface.monitor_id
        && a.state.surface.window_key == b.state.surface.window_key
        && (a.state.position.y - b.state.position.y).abs() < 8.0
}

fn corridor_allows(
    creature: &Creature,
    destination: Point,
    desktop: &DesktopSnapshot,
    settings: &Settings,
) -> bool {
    let Some(monitor) = desktop
        .monitors
        .iter()
        .find(|m| m.id == creature.state.surface.monitor_id)
    else {
        return false;
    };
    let start = creature.state.position;
    if let Some(key) = creature.state.surface.window_key {
        let Some(window) = desktop
            .windows
            .iter()
            .find(|w| w.key == key && w.visible && !w.minimized)
        else {
            return false;
        };
        if destination.x < window.bounds.x + 12.0 || destination.x > window.bounds.right() - 12.0 {
            return false;
        }
    }
    accessible_regions(&settings.habitat, monitor)
        .iter()
        .any(|region| {
            region.contains(start)
                && region.contains(destination)
                && destination.x >= region.x + 8.0
                && destination.x <= region.right() - 8.0
        })
}

pub(super) fn landing_point(journey: &WindowJourney) -> Point {
    match journey {
        WindowJourney::Gap(journey) => journey.hop.target,
        WindowJourney::Hop(journey) => journey.target,
        WindowJourney::Climb(journey) => journey.target,
        WindowJourney::Squeeze(journey) => journey.target,
    }
}

/// The shortest walk worth setting off on. A creature asked to travel less than this stays where
/// it is, so anything that steers by small steps has to ask for at least this much to move at all.
pub(super) const MIN_STRIDE: f32 = 8.0;

pub(super) fn safe_goal(
    world: &World,
    creature: &Creature,
    x: f32,
    desktop: &DesktopSnapshot,
) -> Option<Point> {
    goal_with_spacing(world, creature, x, desktop, 1.0)
}

/// A goal a playing creature may take, which allows closer approach than an audience spot.
pub(super) fn play_goal(
    world: &World,
    creature: &Creature,
    x: f32,
    desktop: &DesktopSnapshot,
) -> Option<Point> {
    goal_with_spacing(world, creature, x, desktop, PLAY_SPACING)
}

fn goal_with_spacing(
    world: &World,
    creature: &Creature,
    x: f32,
    desktop: &DesktopSnapshot,
    spacing_factor: f32,
) -> Option<Point> {
    let destination = Point {
        x,
        y: creature.state.position.y,
    };
    let distance = (x - creature.state.position.x).abs();
    if distance < MIN_STRIDE
        || distance > (speed(creature) * MAX_APPROACH_SECONDS).min(120.0)
        || !corridor_allows(creature, destination, desktop, &world.save.settings)
    {
        return None;
    }
    let mut head = head_point(creature, &world.save.settings, desktop);
    head.x = x;
    if !point_exposed(head, creature.state.surface.window_key, desktop) {
        return None;
    }
    let gap = spacing(creature, &world.save.settings, desktop) * spacing_factor;
    if world.window_journeys.values().any(|journey| {
        let surface = journey.surface();
        surface.monitor_id == creature.state.surface.monitor_id
            && surface.window_key == creature.state.surface.window_key
            && landing_point(journey).distance(destination) < gap
    }) {
        return None;
    }
    if world.save.creatures.iter().any(|other| {
        if other.id == creature.id
            || other.state.arrival_delay_secs > 0.0
            || !same_surface(creature, other)
        {
            return false;
        }
        let reserved = world
            .attention
            .plans
            .get(&other.id)
            .and_then(|p| p.walk)
            .map_or(other.state.position, |walk| walk.destination);
        (reserved.x - x).abs() < gap
    }) {
        return None;
    }
    Some(destination)
}

pub(super) fn actor_walk(
    world: &World,
    creature: &Creature,
    signal: GeometrySignal,
    desktop: &DesktopSnapshot,
) -> Option<ShortWalk> {
    if world.save.settings.reduce_motion || creature.state.surface.window_key == Some(signal.window)
    {
        return None;
    }
    let bounds = signal.bounds;
    let desired = match signal.kind {
        GeometryChange::Appeared | GeometryChange::Rearranged => {
            let gap = spacing(creature, &world.save.settings, desktop) * 0.5;
            let mut edges = [bounds.x - gap, bounds.right() + gap];
            edges.sort_by(|a, b| {
                (a - creature.state.position.x)
                    .abs()
                    .total_cmp(&(b - creature.state.position.x).abs())
            });
            edges
                .into_iter()
                .find_map(|x| safe_goal(world, creature, x, desktop))
        }
        GeometryChange::Expanded
            if actor_emotion(creature, signal) == AttentionEmotion::Startled =>
        {
            let direction = if creature.state.position.x < bounds.x + bounds.width * 0.5 {
                -1.0
            } else {
                1.0
            };
            safe_goal(
                world,
                creature,
                creature.state.position.x
                    + direction * (24.0 + (1.0 - creature.personality.boldness) * 24.0),
                desktop,
            )
        }
        _ => None,
    }?;
    Some(ShortWalk::watching(desired, bounds))
}

pub(super) fn observer_walk(
    world: &World,
    creature: &Creature,
    actor: &Creature,
    desktop: &DesktopSnapshot,
) -> Option<ShortWalk> {
    if world.save.settings.reduce_motion
        || !same_surface(creature, actor)
        || creature.personality.sociability < 0.5
    {
        return None;
    }
    let plan = world.attention.plans.get(&actor.id)?;
    // Moving riders are best watched from the current surface. For a short inspection, reserve
    // a nearby spot relative to where the investigator is going to stand.
    let destination = plan.walk?.destination;
    let gap = spacing(creature, &world.save.settings, desktop);
    let side = if creature.state.position.x < destination.x {
        -1.0
    } else {
        1.0
    };
    [side, side * 2.0, -side, -side * 2.0]
        .into_iter()
        .find_map(|slot| safe_goal(world, creature, destination.x + slot * gap, desktop))
        .map(ShortWalk::new)
}

/// A failed step ends the approach at its current safe point, rather than forcing a crossing.
pub(super) fn step(
    creature: &mut Creature,
    walk: ShortWalk,
    neighbors: &[(CreatureId, SurfaceAttachment, Point)],
    desktop: &DesktopSnapshot,
    settings: &Settings,
    dt: f32,
) -> bool {
    let dx = walk.destination.x - creature.state.position.x;
    let distance = (speed(creature) * dt).min(dx.abs());
    let next = Point {
        x: creature.state.position.x + dx.signum() * distance,
        y: creature.state.position.y,
    };
    if !corridor_allows(creature, next, desktop, settings) {
        return false;
    }
    let gap = spacing(creature, settings, desktop) * walk.spacing_factor;
    if neighbors.iter().any(|(id, surface, point)| {
        *id != creature.id
            && surface.monitor_id == creature.state.surface.monitor_id
            && surface.window_key == creature.state.surface.window_key
            && (point.y - next.y).abs() < 8.0
            && (point.x - next.x).abs() < gap
            && (point.x - next.x).abs() < (point.x - creature.state.position.x).abs()
    }) {
        return false;
    }
    let mut head = head_point(creature, settings, desktop);
    head.x = next.x;
    if !point_exposed(head, creature.state.surface.window_key, desktop) {
        return false;
    }
    creature.state.position = next;
    creature.state.velocity.x = dx.signum() * speed(creature);
    creature.state.facing_right = dx >= 0.0;
    (walk.destination.x - next.x).abs() > 0.5
}
