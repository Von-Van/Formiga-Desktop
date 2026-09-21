use super::surfaces::supports_on;
use super::*;

const INSPECTION_RADIUS: f32 = 12.0;
pub(super) const MANTLE_LIFT_POINTS: f32 = 10.0;

#[derive(Clone)]
pub(super) struct HopJourney {
    pub(super) start: Point,
    pub(super) target: Point,
    pub(super) surface: SurfaceAttachment,
    pub(super) elapsed: f32,
    pub(super) duration: f32,
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
                JourneyStep {
                    position: Point {
                        x: lerp(journey.start.x, journey.target.x, progress),
                        y: lerp(journey.start.y, journey.target.y, progress) - arc,
                    },
                    action: ActionKind::Landing,
                    complete: progress >= 1.0,
                }
            }
            Self::Climb(journey) => {
                journey.elapsed += dt;
                if journey.elapsed < 0.0 {
                    return JourneyStep {
                        position: journey.start,
                        action: ActionKind::InspectScreen,
                        complete: false,
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
                    }
                } else if journey.elapsed < climb_end {
                    let progress = ((journey.elapsed - approach_end)
                        / journey.climb_duration.max(0.001))
                    .clamp(0.0, 1.0);
                    JourneyStep {
                        position: lerp_point(journey.approach, journey.climb_end, progress),
                        action: ActionKind::ClimbWindow,
                        complete: false,
                    }
                } else {
                    let progress = ((journey.elapsed - climb_end)
                        / journey.mantle_duration.max(0.001))
                    .clamp(0.0, 1.0);
                    JourneyStep {
                        position: lerp_point(
                            journey.climb_end,
                            journey.target,
                            smoothstep(progress),
                        ),
                        // Keep the climbing pose attached through the whole pull-up. Switching to
                        // the landing pose at the start of this short segment made the body appear
                        // to pause and then snap above the ledge before settling.
                        action: ActionKind::ClimbWindow,
                        complete: journey.elapsed >= total,
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
                }
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
        });
    };

    let left_track = window.bounds.x + 5.0;
    let right_track = window.bounds.right() - 5.0;
    let use_left = (start.x - left_track).abs() <= (start.x - right_track).abs();
    let track_x = if use_left { left_track } else { right_track };
    let target_x = if use_left {
        window.bounds.x + 18.0
    } else {
        window.bounds.right() - 18.0
    };
    let approach = Point {
        x: track_x,
        y: start.y,
    };
    let climb_end = Point {
        x: track_x,
        // Stop with the body just below its final contact point, then lift and move inward in one
        // smooth mantle. Previously the contact point reached the ledge before the horizontal
        // pull began, producing a visible hold-and-reposition hitch.
        y: window.bounds.y + MANTLE_LIFT_POINTS,
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
        target,
        surface,
        elapsed: 0.0,
        approach_duration: (start.distance(approach) / traverse_speed).clamp(0.0, 6.0),
        climb_duration: (approach.distance(climb_end) / climb_speed).max(0.35),
        mantle_duration: 0.7,
    })
}

pub(super) fn build_route_hop_journey(
    creature: &Creature,
    hop: TopologyRouteHop,
    desktop: &DesktopSnapshot,
) -> WindowJourney {
    let surface = SurfaceAttachment {
        kind: SurfaceKind::WindowLedge,
        monitor_id: hop.monitor_id,
        window_key: Some(hop.to_window),
        relative_x: ((hop.target.x - hop.to_bounds.x) / hop.to_bounds.width).clamp(0.05, 0.95),
    };
    match hop.kind {
        RouteHopKind::WindowTier => build_window_journey(creature, hop.target, surface, desktop),
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
            climbing: creature.tendencies.climbing,
            exploration: creature.tendencies.exploration,
            cursor_trust: creature.tendencies.cursor_trust,
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
    events: &mut Vec<WorldEvent>,
) {
    let support = find_drop_support(creature.state.position, desktop, policy, true).or_else(|| {
        nearest_habitat_point(policy, &desktop.monitors, creature.state.position).map(
            |(monitor_id, position)| {
                (
                    position,
                    SurfaceAttachment {
                        kind: SurfaceKind::ScreenFloor,
                        monitor_id,
                        window_key: None,
                        relative_x: 0.5,
                    },
                )
            },
        )
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

pub(super) fn find_nearby_ledge(
    creature: &Creature,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    topology: &DesktopTopology,
) -> Option<(Point, SurfaceAttachment)> {
    let current_window = creature.state.surface.window_key;
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
            let reachable = dx <= 360.0
                && (36.0..=640.0).contains(&dy)
                && habitat_contains(
                    policy,
                    monitor,
                    Point {
                        x: ledge_x,
                        y: window.bounds.y,
                    },
                );
            let island_bonus = if topology.island_windows().any(|key| key == window.key) {
                42.0 + f32::from(creature.tendencies.exploration.max(0)) * 0.2
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
        && creature.tendencies.cursor_trust >= -20
        && (creature.tendencies.cursor_trust >= 10
            || creature.personality.cursor_interest + creature.personality.boldness >= 1.15)
}

pub(super) fn constrain_to_surface(
    creature: &mut Creature,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
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
        let intervals: Vec<_> = accessible_regions(policy, monitor)
            .into_iter()
            .filter(|region| window.bounds.y >= region.y && window.bounds.y <= region.bottom())
            .filter_map(|region| {
                let min = (window.bounds.x + 12.0).max(region.x + 8.0);
                let max = (window.bounds.right() - 12.0).min(region.right() - 8.0);
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
        if let Some(region) = regions.iter().min_by(|a, b| {
            distance_to_interval(creature.state.position.x, (a.x + 8.0, a.right() - 8.0)).total_cmp(
                &distance_to_interval(creature.state.position.x, (b.x + 8.0, b.right() - 8.0)),
            )
        }) {
            // The same rule on the floor: the ends of the ground a creature is allowed on are
            // walls, and walking into one is arriving, not a reason to keep pushing.
            let (low, high) = (region.x + 8.0, region.right() - 8.0);
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
    supports_on(windows, &regions, monitor.id)
        .filter(|span| span.y >= cursor.y)
        .map(|span| span.place(span.nearest_x(cursor.x)))
        .filter(|(point, _)| regions.iter().any(|region| region.contains(*point)))
        .min_by(|a, b| cursor.distance(a.0).total_cmp(&cursor.distance(b.0)))
}
