use super::*;

pub(super) fn update_drives(creature: &mut Creature, dt: f32) {
    let moving = matches!(
        creature.state.action,
        ActionKind::Traverse
            | ActionKind::Sprint
            | ActionKind::InvestigateCursor
            | ActionKind::AvoidCursor
            | ActionKind::Follow
            | ActionKind::ClimbWindow
    );
    let sleeping = creature.state.action == ActionKind::Sleep;
    if sleeping {
        creature.state.drives.energy = (creature.state.drives.energy + dt * 0.035).min(1.0);
        creature.state.drives.sleep_pressure =
            (creature.state.drives.sleep_pressure - dt * 0.05).max(0.0);
        creature.state.drives.arousal = (creature.state.drives.arousal - dt * 0.08).max(0.0);
    } else {
        let movement_cost = if creature.state.action == ActionKind::Sprint {
            0.02
        } else if moving {
            0.008
        } else {
            0.002
        };
        creature.state.drives.energy = (creature.state.drives.energy - dt * movement_cost).max(0.0);
        creature.state.drives.sleep_pressure =
            (creature.state.drives.sleep_pressure + dt * 0.0025).min(1.0);
        creature.state.drives.boredom = (creature.state.drives.boredom + dt * 0.004).min(1.0);
        creature.state.drives.social_need =
            (creature.state.drives.social_need + dt * 0.0015).min(1.0);
        creature.state.drives.arousal = (creature.state.drives.arousal - dt * 0.025).max(0.0);
    }
}

/// A creature doing something where it stands comes to a stop from whatever pace it arrived at,
/// in about an eighth of a second, rather than carrying a walk's speed through a snack or a game.
fn coast(creature: &mut Creature, dt: f32) {
    creature.state.velocity.x *= (1.0 - dt * 8.0).max(0.0);
}

pub(super) fn execute_action(
    creature: &mut Creature,
    desktop: &DesktopSnapshot,
    context: BehaviorContext,
    dt: f32,
    nearest: Option<(f32, Point, CreatureId)>,
    selected_target: Option<Point>,
) {
    if matches!(
        creature.state.action,
        ActionKind::InvestigateCursor | ActionKind::AvoidCursor
    ) && (!desktop.cursor.available || !context.cursor_safe)
    {
        creature.state.action = ActionKind::Idle;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 2.5;
        creature.state.velocity = Point::default();
        return;
    }
    // A companion doing one of its little habits does it where it stands. The action carries on
    // as usual once it has finished, walking on to wherever it was going if it has somewhere.
    if flourishing(creature) {
        creature.state.velocity.x = 0.0;
        return;
    }
    let speed = 24.0 + creature.personality.activity * 34.0;
    let mut target_x = None;
    let mut target_stop_distance = 0.0_f32;
    let mut target_speed_multiplier = 1.0;
    match creature.state.action {
        ActionKind::Traverse if selected_target.is_some() => {
            target_x = selected_target.map(|target| target.x);
        }
        ActionKind::Sprint if selected_target.is_some() => {
            target_x = selected_target.map(|target| target.x);
            target_speed_multiplier = 2.35;
        }
        ActionKind::Traverse | ActionKind::Sprint => {
            let direction = if creature.state.facing_right {
                1.0
            } else {
                -1.0
            };
            let multiplier = if creature.state.action == ActionKind::Sprint {
                2.35
            } else {
                1.0
            };
            creature.state.velocity.x = direction * speed * multiplier;
        }
        ActionKind::InvestigateCursor if desktop.cursor.available && context.cursor_safe => {
            target_x = Some(desktop.cursor.position.x)
        }
        ActionKind::AvoidCursor if desktop.cursor.available && context.cursor_safe => {
            target_x = Some(
                creature.state.position.x
                    + (creature.state.position.x - desktop.cursor.position.x).signum() * 180.0,
            );
            creature.state.cursor_cooldown = 4.0;
            creature.state.drives.arousal = (creature.state.drives.arousal + dt * 0.7).min(1.0);
        }
        ActionKind::Follow | ActionKind::Greet | ActionKind::SocialPlay => {
            target_x = selected_target
                .map(|target| target.x)
                .or_else(|| nearest.map(|item| item.1.x));
            // A bond mark already carries the distance the two should keep, so walking onto it is
            // the whole approach. Stopping well short of it used to let a creature that was
            // already too close simply stay there for the length of the greeting. Only the
            // fallback, which steers at a companion's own contact point, needs room of its own.
            target_stop_distance = if selected_target.is_some() {
                2.0
            } else if creature.state.action == ActionKind::Follow {
                42.0
            } else {
                30.0
            };
            creature.state.drives.social_need =
                (creature.state.drives.social_need - dt * 0.08).max(0.0);
        }
        ActionKind::Sleep if selected_target.is_some() => {
            target_x = selected_target.map(|target| target.x);
            target_stop_distance = 5.0;
        }
        ActionKind::SoloPlay => {
            coast(creature, dt);
            creature.state.drives.boredom = (creature.state.drives.boredom - dt * 0.09).max(0.0);
        }
        ActionKind::Eat => {
            coast(creature, dt);
            creature.state.drives.energy = (creature.state.drives.energy + dt * 0.025).min(1.0);
            creature.state.drives.comfort = (creature.state.drives.comfort + dt * 0.018).min(1.0);
            creature.state.drives.boredom = (creature.state.drives.boredom - dt * 0.018).max(0.0);
        }
        ActionKind::Drink => {
            coast(creature, dt);
            creature.state.drives.comfort = (creature.state.drives.comfort + dt * 0.024).min(1.0);
            creature.state.drives.arousal = (creature.state.drives.arousal - dt * 0.04).max(0.0);
            creature.state.drives.curiosity_satisfaction =
                (creature.state.drives.curiosity_satisfaction + dt * 0.012).min(1.0);
        }
        ActionKind::Dangle => {
            coast(creature, dt);
            creature.state.drives.comfort = (creature.state.drives.comfort + dt * 0.012).min(1.0);
            creature.state.drives.boredom = (creature.state.drives.boredom - dt * 0.025).max(0.0);
        }
        ActionKind::InspectScreen => {
            coast(creature, dt);
            if let Some(target) = selected_target {
                creature.state.facing_right = target.x >= creature.state.position.x;
            }
            creature.state.drives.curiosity_satisfaction =
                (creature.state.drives.curiosity_satisfaction + dt * 0.055).min(1.0);
            creature.state.drives.boredom = (creature.state.drives.boredom - dt * 0.035).max(0.0);
        }
        ActionKind::PresentDiscovery => {
            coast(creature, dt);
            creature.state.drives.curiosity_satisfaction =
                (creature.state.drives.curiosity_satisfaction + dt * 0.035).min(1.0);
            creature.state.drives.comfort = (creature.state.drives.comfort + dt * 0.012).min(1.0);
            if let Some(target) = selected_target {
                creature.state.facing_right = target.x >= creature.state.position.x;
            }
        }
        ActionKind::ReactToWindow => {
            if let Some(target) = selected_target {
                creature.state.facing_right = target.x >= creature.state.position.x;
                creature.state.velocity.x = 0.0;
            } else {
                creature.state.velocity.x = if creature.state.facing_right {
                    speed * 1.4
                } else {
                    -speed * 1.4
                };
            }
            creature.state.drives.arousal = (creature.state.drives.arousal + dt * 0.5).min(1.0);
        }
        _ => coast(creature, dt),
    }
    if let Some(target) = target_x {
        let dx = target - creature.state.position.x;
        // Arriving means landing on the mark, not stepping over it. A walk covers one to three
        // points per tick, so a creature aimed at a spot with no room around it would overshoot,
        // turn, overshoot coming back, and shiver there for the rest of the action — close to a
        // companion, where the marks are, that reads as a creature having a fit. A step is never
        // longer than what is left of the walk, and a creature that has arrived stands still.
        let step = speed * target_speed_multiplier * dt;
        if dx.abs() <= target_stop_distance.max(step) {
            if target_stop_distance <= 0.0 {
                creature.state.position.x = target;
            }
            creature.state.velocity.x = 0.0;
            // Standing on its mark, a creature keeps the way it was walking rather than turning
            // on the last fraction of a point. Beside a companion it turns to face them, which
            // is what it walked over for.
            if matches!(
                creature.state.action,
                ActionKind::Follow | ActionKind::Greet | ActionKind::SocialPlay
            ) && let Some((_, companion, _)) = nearest
            {
                creature.state.facing_right = companion.x >= creature.state.position.x;
            }
        } else {
            creature.state.facing_right = dx >= 0.0;
            creature.state.velocity.x = dx.signum() * speed * target_speed_multiplier;
        }
    }
    creature.state.position.x += creature.state.velocity.x * dt;
    if !context.on_window_ledge && creature.state.action == ActionKind::Perch {
        creature.state.velocity.x = 0.0;
    }
}

pub(super) fn action_duration<R: Rng + ?Sized>(action: ActionKind, rng: &mut R) -> f32 {
    let range = match action {
        ActionKind::Sleep => 18.0..45.0,
        ActionKind::Traverse | ActionKind::Follow => 4.0..11.0,
        ActionKind::Sprint => 2.5..5.5,
        ActionKind::InvestigateCursor | ActionKind::AvoidCursor | ActionKind::ReactToWindow => {
            2.0..5.0
        }
        ActionKind::Greet | ActionKind::SocialPlay | ActionKind::SoloPlay => 3.0..8.0,
        ActionKind::Eat | ActionKind::Drink => 5.0..9.0,
        ActionKind::Dangle => 4.0..7.0,
        ActionKind::InspectScreen => 3.0..5.0,
        ActionKind::PresentDiscovery => 3.8..4.2,
        _ => 3.0..10.0,
    };
    rng.random_range(range)
}

pub(super) fn reinforce_habit(creature: &mut Creature, action: ActionKind, hour_utc: u8) {
    let key = routine_key(
        creature.state.surface.kind,
        creature.state.surface.relative_x,
        action,
        hour_utc,
    );
    creature.routines.reinforce(key);
}

pub(super) fn keep_creatures_in_habitat(
    creatures: &mut [Creature],
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    unconstrained: &[CreatureId],
) {
    let primary = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.primary)
        .or_else(|| desktop.monitors.first());
    for creature in creatures {
        if unconstrained.contains(&creature.id) {
            continue;
        }
        let monitor = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == creature.state.surface.monitor_id)
            .or_else(|| {
                desktop
                    .monitors
                    .iter()
                    .find(|monitor| monitor.bounds.contains(creature.state.position))
            })
            .or(primary);
        if let Some(monitor) = monitor {
            // Native display identifiers can change after sleep, hot-plugging, or a display-mode
            // transition. Rendering filters by the current identifier, so retaining a stale ID
            // leaves an otherwise valid creature alive in the simulation but absent from every
            // overlay until restart. Rebind it to the monitor that actually contains its point.
            creature.state.surface.monitor_id = monitor.id;
            let regions = accessible_regions(policy, monitor);
            if regions.is_empty() {
                if let Some((monitor_id, position)) =
                    nearest_habitat_point(policy, &desktop.monitors, creature.state.position)
                {
                    creature.state.position = position;
                    creature.state.surface = SurfaceAttachment {
                        kind: SurfaceKind::ScreenFloor,
                        monitor_id,
                        window_key: None,
                        relative_x: 0.5,
                    };
                }
                continue;
            }
            let bounds = monitor.usable_bounds;
            if creature.state.position.x <= bounds.x + 8.0 {
                creature.state.position.x = bounds.x + 8.0;
                creature.state.facing_right = true;
            } else if creature.state.position.x >= bounds.right() - 8.0 {
                creature.state.position.x = bounds.right() - 8.0;
                creature.state.facing_right = false;
            }
            if !regions
                .iter()
                .any(|region| region.contains(creature.state.position))
                && let Some((monitor_id, position)) =
                    nearest_habitat_point(policy, &desktop.monitors, creature.state.position)
            {
                creature.state.position = position;
                creature.state.surface = SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id,
                    window_key: None,
                    relative_x: 0.5,
                };
            }
        }
    }
}

pub(super) fn update_surface_attachments(
    creatures: &mut [Creature],
    desktop: &DesktopSnapshot,
    previous: &BTreeMap<WindowKey, DesktopRect>,
    topology: &DesktopTopology,
    react_to_motion: bool,
    journeys: &BTreeMap<CreatureId, WindowJourney>,
    events: &mut Vec<WorldEvent>,
) {
    for creature in creatures {
        // In-flight contact belongs to the journey. Reattaching first would snap the position
        // back to the old surface before an observer could see the actual movement.
        if journeys.contains_key(&creature.id)
            || matches!(
                creature.state.action,
                ActionKind::Tossed | ActionKind::Dragged
            )
        {
            continue;
        }
        let Some(key) = creature.state.surface.window_key else {
            continue;
        };
        let current = desktop
            .windows
            .iter()
            .find(|window| window.key == key && window.visible && !window.minimized);
        match current {
            Some(window) => {
                let old = previous.get(&key).copied();
                let relative = creature.state.surface.relative_x.clamp(0.05, 0.95);
                creature.state.position.x = window.bounds.x + window.bounds.width * relative;
                creature.state.position.y = window.bounds.y;
                if let Some(monitor) = desktop
                    .monitors
                    .iter()
                    .find(|m| m.bounds.contains(creature.state.position))
                {
                    creature.state.surface.monitor_id = monitor.id;
                }
                if react_to_motion
                    && creature.state.attention.is_none()
                    && old.is_some_and(|old| old != window.bounds)
                {
                    let old = old.expect("changed window has previous bounds");
                    let movement = Point {
                        x: window.bounds.x - old.x,
                        y: window.bounds.y - old.y,
                    };
                    let moved_distance = movement.distance(Point::default());
                    let resized = (window.bounds.width - old.width).abs()
                        + (window.bounds.height - old.height).abs();
                    let rapid = moved_distance > 80.0 || resized > 90.0;
                    let calm_platform = topology.is_slow_platform(key);
                    let next = if !calm_platform
                        && rapid
                        && creature.personality.window_tolerance < 0.72
                    {
                        ActionKind::ReactToWindow
                    } else {
                        ActionKind::RideWindow
                    };
                    if creature.state.action != next {
                        creature.state.action = next;
                        creature.state.action_elapsed = 0.0;
                        creature.state.action_duration = 2.5;
                        if next == ActionKind::ReactToWindow {
                            creature.state.drives.arousal =
                                (creature.state.drives.arousal + 0.35).min(1.0);
                        }
                        World::emit(
                            events,
                            WorldEvent::WindowReaction {
                                creature_id: creature.id,
                                action: next,
                            },
                        );
                    }
                }
            }
            None => {
                // A native identifier change with an unchanged frame is the same visible ledge.
                let mut renamed = desktop.windows.iter().filter(|window| {
                    window.visible
                        && !window.minimized
                        && Some(window.bounds) == previous.get(&key).copied()
                        && !previous.contains_key(&window.key)
                });
                if let (Some(window), None) = (renamed.next(), renamed.next()) {
                    creature.state.surface.window_key = Some(window.key);
                    continue;
                }
                let monitor = desktop
                    .monitors
                    .iter()
                    .find(|monitor| monitor.id == creature.state.surface.monitor_id)
                    .or_else(|| desktop.monitors.iter().find(|monitor| monitor.primary))
                    .or_else(|| desktop.monitors.first());
                if let Some(monitor) = monitor {
                    creature.state.position.y = monitor.usable_bounds.bottom() - 4.0;
                    creature.state.position.x = creature.state.position.x.clamp(
                        monitor.usable_bounds.x + 8.0,
                        monitor.usable_bounds.right() - 8.0,
                    );
                    creature.state.surface = SurfaceAttachment {
                        kind: SurfaceKind::ScreenFloor,
                        monitor_id: monitor.id,
                        window_key: None,
                        relative_x: (creature.state.position.x - monitor.usable_bounds.x)
                            / monitor.usable_bounds.width,
                    };
                    creature.state.action = ActionKind::ReactToWindow;
                    creature.state.action_elapsed = 0.0;
                    creature.state.action_duration = 3.0;
                    World::emit(
                        events,
                        WorldEvent::SurfaceChanged {
                            creature_id: creature.id,
                            kind: SurfaceKind::ScreenFloor,
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
        }
    }
}
