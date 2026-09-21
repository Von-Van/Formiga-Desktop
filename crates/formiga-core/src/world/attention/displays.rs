//! Display identity tracking and short exploration routes through connected habitat rectangles.
use super::*;

const MAX_DISPLAYS: usize = 8;
const MAX_DISCOVERY_TRAVEL: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct DisplayShape {
    id: MonitorId,
    key: DisplayKey,
    bounds: DesktopRect,
    usable: DesktopRect,
    scale: f32,
}

#[derive(Clone, Copy)]
struct Discovery {
    key: DisplayKey,
    origin: u64,
    age: f32,
}

#[derive(Default)]
pub(crate) struct DisplayAttention {
    previous: [Option<DisplayShape>; MAX_DISPLAYS],
    initialized: bool,
    discovery: Option<Discovery>,
    reorient: [Option<CreatureId>; MAX_COLONY_CREATURES],
    recovery_age: f32,
    next_origin: u64,
    sample_at: Option<u64>,
    retry_in: f32,
}

impl DisplayAttention {
    pub(super) fn has_pending(&self) -> bool {
        self.discovery.is_some() || self.reorient.iter().any(Option::is_some)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DisplayWalk {
    from: DisplayKey,
    to: DisplayKey,
    source_region: DesktopRect,
    target_region: DesktopRect,
    source_bounds: DesktopRect,
    target_bounds: DesktopRect,
    seam: Point,
    destination: Point,
    crossed_seam: bool,
}

impl DisplayWalk {
    pub(super) fn settle(self, creature: &mut Creature, desktop: &DesktopSnapshot) {
        let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|m| m.id == creature.state.surface.monitor_id)
        else {
            return;
        };
        let region = if monitor.display_key == self.from {
            self.source_region
        } else {
            self.target_region
        };
        creature.state.position.x = creature
            .state
            .position
            .x
            .clamp(region.x + 8.0, region.right() - 8.0);
        creature.state.position.y = region.bottom() - 4.0;
        creature.state.velocity = Point::default();
    }
    pub(super) fn valid(self, desktop: &DesktopSnapshot, settings: &Settings) -> bool {
        [
            (self.from, self.source_bounds, self.source_region),
            (self.to, self.target_bounds, self.target_region),
        ]
        .into_iter()
        .all(|(key, bounds, region)| {
            desktop
                .monitors
                .iter()
                .find(|m| m.display_key == key)
                .is_some_and(|m| {
                    m.bounds == bounds && accessible_regions(&settings.habitat, m).contains(&region)
                })
        })
    }

    fn plan(
        world: &World,
        creature: &Creature,
        to: &MonitorInfo,
        desktop: &DesktopSnapshot,
    ) -> Option<(f32, Self)> {
        if creature.state.surface.kind != SurfaceKind::ScreenFloor {
            return None;
        }
        let from = desktop
            .monitors
            .iter()
            .find(|m| m.id == creature.state.surface.monitor_id)?;
        if from.display_key == to.display_key {
            return None;
        }
        let mut best = None;
        let targets = accessible_regions(&world.save.settings.habitat, to);
        for source in accessible_regions(&world.save.settings.habitat, from)
            .into_iter()
            .take(64)
            .filter(|r| r.contains(creature.state.position))
        {
            for target in targets.iter().copied().take(64) {
                let (x, direction) = if (source.right() - target.x).abs() <= 0.5 {
                    (source.right(), 1.0)
                } else if (source.x - target.right()).abs() <= 0.5 {
                    (source.x, -1.0)
                } else {
                    continue;
                };
                let min_y = source.y.max(target.y) + 8.0;
                let max_y = source.bottom().min(target.bottom()) - 4.0;
                if max_y <= min_y {
                    continue;
                }
                let seam = Point {
                    x,
                    y: creature.state.position.y.clamp(min_y, max_y),
                };
                let destination = Point {
                    x: x + direction * (target.width * 0.25).clamp(24.0, 84.0),
                    y: target.bottom() - 4.0,
                };
                let length = creature.state.position.distance(seam) + seam.distance(destination);
                if length > (motion::speed(creature) * MAX_DISCOVERY_TRAVEL).min(360.0) {
                    continue;
                }
                if best.as_ref().is_none_or(|(distance, _)| length < *distance) {
                    best = Some((
                        length,
                        Self {
                            from: from.display_key,
                            to: to.display_key,
                            source_region: source,
                            target_region: target,
                            source_bounds: from.bounds,
                            target_bounds: to.bounds,
                            seam,
                            destination,
                            crossed_seam: false,
                        },
                    ));
                }
            }
        }
        best
    }

    pub(super) fn step(
        &mut self,
        creature: &mut Creature,
        neighbors: &[(CreatureId, SurfaceAttachment, Point)],
        desktop: &DesktopSnapshot,
        settings: &Settings,
        dt: f32,
    ) -> bool {
        let target = if self.crossed_seam {
            self.destination
        } else {
            self.seam
        };
        let previous = creature.state.position;
        let distance = previous.distance(target);
        let next = lerp_point(
            previous,
            target,
            (motion::speed(creature) * dt / distance.max(0.001)).min(1.0),
        );
        let monitor = desktop
            .monitors
            .iter()
            .find(|m| m.id == creature.state.surface.monitor_id && m.bounds.contains(next))
            .or_else(|| desktop.monitors.iter().find(|m| m.bounds.contains(next)));
        let Some(monitor) = monitor else {
            return false;
        };
        let region = if monitor.display_key == self.from {
            self.source_region
        } else {
            self.target_region
        };
        if !region.contains(next) {
            return false;
        }
        let mut head = head_point(creature, settings, desktop);
        head.x += next.x - previous.x;
        head.y += next.y - previous.y;
        let gap = (48.0 * f32::from(settings.display_scale) / monitor.scale_factor * 0.65)
            .clamp(24.0, 96.0);
        if !point_exposed(head, None, desktop)
            || neighbors.iter().any(|(id, _, point)| {
                *id != creature.id
                    && point.distance(next) < gap
                    && point.distance(next) < point.distance(previous)
            })
        {
            return false;
        }
        creature.state.position = next;
        creature.state.velocity = Point {
            x: (next.x - previous.x) / dt.max(0.001),
            y: (next.y - previous.y) / dt.max(0.001),
        };
        creature.state.facing_right = target.x >= previous.x;
        creature.state.surface.monitor_id = monitor.id;
        creature.state.surface.relative_x = ((next.x - region.x) / region.width).clamp(0.0, 1.0);
        if next.distance(target) <= 0.1 {
            if self.crossed_seam {
                return false;
            }
            self.crossed_seam = true;
        }
        true
    }
}

impl World {
    pub(in crate::world) fn prepare_display_attention(
        &mut self,
        desktop: &DesktopSnapshot,
        dt: f32,
        active: bool,
    ) {
        let at = desktop
            .cursor_sample_millis
            .or(desktop.window_sample.map(|s| s.monotonic_millis));
        let continuous = match (self.display_attention.sample_at, at) {
            (Some(old), Some(now)) => now >= old && now - old <= 3_000,
            _ => true,
        };
        self.display_attention.sample_at = at;
        let active = active && continuous;
        self.display_attention.retry_in = (self.display_attention.retry_in - dt).max(0.0);
        if let Some(discovery) = &mut self.display_attention.discovery {
            discovery.age += dt;
            if discovery.age > 25.0 {
                self.display_attention.discovery = None;
            }
        }
        self.display_attention.recovery_age += dt;
        if self.display_attention.recovery_age > 3.0 {
            self.display_attention.reorient.fill(None);
        }
        let valid = !desktop.monitors.is_empty()
            && desktop.monitors.len() <= MAX_DISPLAYS
            && desktop.monitors.iter().enumerate().all(|(index, m)| {
                crate::attention::valid_rect(m.bounds)
                    && crate::attention::valid_rect(m.usable_bounds)
                    && m.scale_factor.is_finite()
                    && m.scale_factor > 0.0
                    && !desktop.monitors[..index]
                        .iter()
                        .any(|other| other.display_key == m.display_key)
            });
        if !valid {
            self.display_attention = DisplayAttention::default();
            return;
        }
        let mut shapes: [Option<DisplayShape>; MAX_DISPLAYS] = std::array::from_fn(|index| {
            desktop.monitors.get(index).map(|m| DisplayShape {
                id: m.id,
                key: m.display_key,
                bounds: m.bounds,
                usable: m.usable_bounds,
                scale: m.scale_factor,
            })
        });
        shapes.sort_by_key(|shape| shape.map(|s| s.key));
        let old = self.display_attention.previous;
        let initialized = self.display_attention.initialized;
        self.display_attention.initialized = true;
        self.display_attention.previous = shapes;
        if !active {
            self.display_attention.discovery = None;
            self.display_attention.reorient.fill(None);
        }
        if !initialized || old == shapes {
            return;
        }
        self.clear_attention();
        self.cursor_observer.reset();
        // Stable display keys distinguish real removal from native identifier/DPI churn.
        let mut affected = [None; MAX_COLONY_CREATURES];
        for (index, creature) in self
            .save
            .creatures
            .iter_mut()
            .take(MAX_COLONY_CREATURES)
            .enumerate()
        {
            let Some(previous) = old
                .iter()
                .flatten()
                .find(|m| m.id == creature.state.surface.monitor_id)
            else {
                continue;
            };
            let current = shapes
                .iter()
                .flatten()
                .find(|m| m.key == previous.key)
                .or_else(|| {
                    // Native keys can also change on reconnect. An unchanged occupied rectangle
                    // provides no new territory and should not manufacture a loss/discovery pair.
                    shapes.iter().flatten().find(|m| {
                        m.bounds == previous.bounds
                            && m.usable == previous.usable
                            && !old.iter().flatten().any(|old| old.key == m.key)
                    })
                });
            if let Some(current) = current {
                creature.state.surface.monitor_id = current.id;
            }
            if current.is_none_or(|m| m.bounds != previous.bounds || m.usable != previous.usable) {
                affected[index] = Some(creature.id);
            }
        }
        for id in affected.into_iter().flatten() {
            if self
                .interaction
                .as_ref()
                .is_some_and(|i| i.creature_id == id)
            {
                self.handle_command(WorldCommand::CancelInteraction, desktop);
            }
            self.window_journeys.remove(&id);
            self.window_routes.remove(&id);
            self.tosses.remove(&id);
            self.bond_plans.remove(&id);
            self.action_choices.remove(&id);
            let creature = self.save.creatures.iter().find(|c| c.id == id).unwrap();
            // Resolution/layout changes can leave a resting contact perfectly valid. Preserve it;
            // only displaced or airborne creatures need a new floor position.
            let supported = desktop
                .monitors
                .iter()
                .find(|m| m.id == creature.state.surface.monitor_id)
                .is_some_and(|monitor| {
                    habitat_contains(
                        &self.save.settings.habitat,
                        monitor,
                        creature.state.position,
                    ) && match creature.state.surface.window_key {
                        Some(key) => desktop.windows.iter().any(|w| {
                            w.key == key
                                && w.visible
                                && !w.minimized
                                && (w.bounds.y - creature.state.position.y).abs() <= 0.5
                                && creature.state.position.x >= w.bounds.x + 12.0
                                && creature.state.position.x <= w.bounds.right() - 12.0
                        }),
                        None => accessible_regions(&self.save.settings.habitat, monitor)
                            .iter()
                            .any(|r| {
                                r.contains(creature.state.position)
                                    && (r.bottom() - 4.0 - creature.state.position.y).abs() <= 0.5
                            }),
                    }
                });
            if supported {
                continue;
            }
            let previous = creature.state.position;
            if let Some((monitor_id, mut position)) =
                nearest_habitat_point(&self.save.settings.habitat, &desktop.monitors, previous)
            {
                let monitor = desktop
                    .monitors
                    .iter()
                    .find(|m| m.id == monitor_id)
                    .unwrap();
                let gap = (48.0 * f32::from(self.save.settings.display_scale)
                    / monitor.scale_factor
                    * 0.65)
                    .clamp(24.0, 96.0);
                // Enough slots for the whole colony on one side, since a recovery against a
                // screen edge can only spread the other way.
                if let Some(spaced) = (0..=2 * MAX_COLONY_CREATURES)
                    .filter_map(|slot| {
                        let direction = if slot % 2 == 0 { 1.0 } else { -1.0 };
                        let candidate = Point {
                            x: position.x + direction * slot.div_ceil(2) as f32 * gap,
                            y: position.y,
                        };
                        (habitat_contains(&self.save.settings.habitat, monitor, candidate)
                            && candidate.x >= monitor.usable_bounds.x + 8.0
                            && candidate.x <= monitor.usable_bounds.right() - 8.0
                            && !self.save.creatures.iter().any(|other| {
                                other.id != id
                                    && other.state.surface.monitor_id == monitor_id
                                    && other.state.position.distance(candidate) < gap - 0.1
                            }))
                        .then_some(candidate)
                    })
                    .next()
                {
                    position = spaced;
                }
                let creature = creature_mut(&mut self.save.creatures, id).unwrap();
                creature.state.position = position;
                creature.state.surface = SurfaceAttachment {
                    monitor_id,
                    window_key: None,
                    kind: SurfaceKind::ScreenFloor,
                    relative_x: 0.5,
                };
                creature.state.velocity = Point::default();
                if creature.state.action != ActionKind::Sleep {
                    creature.state.action = ActionKind::Idle;
                    creature.state.action_duration = 2.5;
                    creature.state.action_elapsed = 0.0;
                }
                Self::emit(
                    &mut self.events,
                    WorldEvent::SurfaceChanged {
                        creature_id: id,
                        kind: SurfaceKind::ScreenFloor,
                    },
                );
            }
        }
        if !active {
            return;
        }
        self.display_attention.next_origin = self.display_attention.next_origin.wrapping_add(1);
        self.display_attention.reorient = affected;
        self.display_attention.recovery_age = 0.0;
        self.display_attention.retry_in = 0.0;
        // Retain one nearby discovery opportunity, not a queue of monitor events.
        self.display_attention.discovery = shapes
            .iter()
            .flatten()
            .find(|m| {
                !old.iter().flatten().any(|old| {
                    old.key == m.key || (old.bounds == m.bounds && old.usable == m.usable)
                })
            })
            .map(|m| Discovery {
                key: m.key,
                origin: self.display_attention.next_origin,
                age: 0.0,
            });
    }

    pub(super) fn try_display_attention(&mut self, desktop: &DesktopSnapshot) -> bool {
        let recovery = self
            .display_attention
            .reorient
            .into_iter()
            .flatten()
            .find(|id| {
                self.save
                    .creatures
                    .iter()
                    .find(|c| c.id == *id)
                    .is_some_and(|c| self.attention_eligible(c, desktop))
            });
        if let Some(id) = recovery {
            self.display_attention
                .reorient
                .iter_mut()
                .filter(|slot| **slot == Some(id))
                .for_each(|slot| *slot = None);
            let creature = self.save.creatures.iter().find(|c| c.id == id).unwrap();
            let monitor = desktop
                .monitors
                .iter()
                .find(|m| m.id == creature.state.surface.monitor_id)
                .unwrap();
            let center = monitor.usable_bounds.x + monitor.usable_bounds.width * 0.5;
            let target = Point {
                x: creature.state.position.x + (center - creature.state.position.x).signum() * 60.0,
                y: creature.state.position.y - 48.0,
            };
            let walk = if self.save.settings.reduce_motion {
                None
            } else {
                motion::safe_goal(self, creature, target.x, desktop).map(ShortWalk::new)
            };
            let reaction = Reaction {
                ride: None,
                cue: None,
                origin: Origin::Display(self.display_attention.next_origin),
                role: Role::Display { discovery: false },
                target,
                emotion: AttentionEmotion::Concerned,
                elapsed: 0.0,
                seconds: REACTION_SECONDS,
                delay: 0.0,
                travel_elapsed: 0.0,
                walk,
                display_walk: None,
                surface: (
                    creature.state.surface.monitor_id,
                    creature.state.surface.window_key,
                ),
                action: ActionKind::InspectScreen,
            };
            self.begin_attention(id, reaction);
            self.recruit_attention_observers(id, reaction.origin, desktop);
            self.attention.colony_cooldown = 7.0;
            return true;
        }
        let Some(discovery) = self.display_attention.discovery else {
            return false;
        };
        if discovery.age < 0.3
            || self.attention.colony_cooldown > 0.0
            || self.display_attention.retry_in > 0.0
        {
            return false;
        }
        let Some(target) = desktop
            .monitors
            .iter()
            .find(|m| m.display_key == discovery.key)
        else {
            return false;
        };
        self.display_attention.retry_in = 1.0;
        let candidate = self
            .save
            .creatures
            .iter()
            .filter(|c| {
                self.attention_eligible(c, desktop)
                    && c.personality.curiosity >= 0.45
                    && c.state.drives.energy > 0.3
            })
            .filter_map(|c| {
                DisplayWalk::plan(self, c, target, desktop)
                    .map(|(distance, walk)| (distance - c.personality.curiosity * 60.0, c.id, walk))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, id, walk)) = candidate else {
            return false;
        };
        self.display_attention.discovery = None;
        let creature = self.save.creatures.iter().find(|c| c.id == id).unwrap();
        let reaction = Reaction {
            ride: None,
            cue: None,
            origin: Origin::Display(discovery.origin),
            role: Role::Display { discovery: true },
            target: walk.destination,
            emotion: AttentionEmotion::Curious,
            elapsed: 0.0,
            seconds: REACTION_SECONDS,
            delay: 0.0,
            travel_elapsed: 0.0,
            walk: None,
            display_walk: (!self.save.settings.reduce_motion).then_some(walk),
            surface: (
                creature.state.surface.monitor_id,
                creature.state.surface.window_key,
            ),
            action: ActionKind::InspectScreen,
        };
        self.begin_attention(id, reaction);
        self.recruit_attention_observers(id, reaction.origin, desktop);
        self.attention.colony_cooldown = 12.0;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> (World, DesktopSnapshot, OffsetDateTime) {
        let (mut world, desktop, now) = super::super::tests::open_scene();
        world.save.creatures.truncate(1);
        world.save.creatures[0].state.position.x = 1_360.0;
        (world, desktop, now)
    }

    fn attach(desktop: &mut DesktopSnapshot) {
        let mut monitor = desktop.monitors[0].clone();
        monitor.id = 2;
        monitor.display_key = DisplayKey([2; 16]);
        monitor.primary = false;
        monitor.bounds.x += 1_440.0;
        monitor.usable_bounds.x += 1_440.0;
        desktop.monitors.push(monitor);
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 250;
    }

    fn ticks(world: &mut World, desktop: &DesktopSnapshot, now: OffsetDateTime, count: i64) {
        for step in 0..count {
            let mut sample = desktop.clone();
            sample.cursor_sample_millis = Some(desktop.cursor_sample_millis.unwrap_or_else(|| {
                desktop.window_sample.unwrap().monotonic_millis + step as u64 * 50
            }));
            world.tick(now + Duration::milliseconds(step * 50), 0.05, &sample);
        }
    }

    #[test]
    fn discovery_crosses_the_seam_continuously_and_finishes_in_new_territory() {
        let (mut world, mut desktop, now) = scene();
        attach(&mut desktop);
        let id = world.save.creatures[0].id;
        let mut previous = world.save.creatures[0].state.position;
        let mut crossed = false;
        for step in 0..180 {
            world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
            let c = &world.save.creatures[0];
            assert!(
                previous.distance(c.state.position) <= 3.0,
                "discovery teleported at {step}"
            );
            if c.state.position.x < 1_440.0 {
                assert_eq!(c.state.surface.monitor_id, 1);
            }
            if c.state.position.x > 1_440.0 {
                assert_eq!(c.state.surface.monitor_id, 2);
                crossed = true;
            }
            assert!(desktop.monitors.iter().any(|m| habitat_contains(
                &world.save.settings.habitat,
                m,
                c.state.position
            )));
            previous = c.state.position;
        }
        assert!(crossed);
        assert!(!world.attention.owns(id));
        assert!(world.save.creatures[0].state.position.x > 1_500.0);
        assert!(world.display_attention.discovery.is_none());
    }

    /// Exploring newly available space, and finding your feet again when it is taken away, are
    /// both things a companion watches its friend do rather than working out for itself.
    #[test]
    fn companions_watch_a_new_display_being_explored_and_the_recovery_when_it_goes() {
        let (mut world, mut desktop, now) = super::super::tests::open_scene();
        for (index, creature) in world.save.creatures.iter_mut().enumerate() {
            creature.state.position.x = 1_360.0 - index as f32 * 70.0;
        }
        attach(&mut desktop);
        let explorer = world.save.creatures[0].id;
        let watching = |world: &World, desktop: &DesktopSnapshot| {
            let head = head_point(&world.save.creatures[0], &world.save.settings, desktop);
            world.save.creatures.iter().skip(1).any(|other| {
                world.attention.plans.get(&other.id).is_some_and(
                    |p| matches!(p.role, Role::Observer { actor } if actor == explorer),
                ) && other
                    .state
                    .attention
                    .is_some_and(|pose| (pose.target.x - head.x).abs() < 4.0)
            })
        };
        let mut watched_the_crossing = false;
        for step in 0..70 {
            let mut sample = desktop.clone();
            sample.cursor_sample_millis = Some(250 + step as u64 * 50);
            world.tick(now + Duration::milliseconds(step * 50), 0.05, &sample);
            watched_the_crossing |= watching(&world, &desktop);
        }
        assert!(watched_the_crossing);
        assert_eq!(world.save.creatures[0].state.surface.monitor_id, 2);
        for creature in world.save.creatures.iter_mut().skip(1) {
            creature.state.action = ActionKind::Idle;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = 100.0;
        }
        desktop.monitors.pop();
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 4_000;
        let mut watched_the_recovery = false;
        for step in 0..60 {
            let mut sample = desktop.clone();
            sample.cursor_sample_millis = Some(4_000 + step as u64 * 50);
            world.tick(
                now + Duration::milliseconds(4_000 + step * 50),
                0.05,
                &sample,
            );
            watched_the_recovery |= watching(&world, &desktop);
        }
        assert!(watched_the_recovery);
        assert_eq!(world.save.creatures[0].state.surface.monitor_id, 1);
    }

    #[test]
    fn disconnected_disallowed_or_excluded_routes_never_cross() {
        for reason in 0..3 {
            let (mut world, mut desktop, now) = scene();
            attach(&mut desktop);
            match reason {
                0 => {
                    desktop.monitors[1].bounds.x += 100.0;
                    desktop.monitors[1].usable_bounds.x += 100.0;
                }
                1 => world.save.settings.habitat.preset = HabitatPreset::PrimaryDisplay,
                _ => world.save.settings.habitat.zones.push(HabitatZone {
                    id: 77,
                    display: desktop.monitors[0].display_key,
                    kind: HabitatZoneKind::Excluded,
                    enabled: true,
                    normalized_bounds: DesktopRect {
                        x: 1_400.0 / 1_440.0,
                        y: 0.0,
                        width: 40.0 / 1_440.0,
                        height: 1.0,
                    },
                }),
            }
            ticks(&mut world, &desktop, now, 40);
            assert!(world.attention.plans.is_empty());
            assert_eq!(world.save.creatures[0].state.surface.monitor_id, 1);
            assert_eq!(world.save.creatures[0].state.position.x, 1_360.0);
        }
    }

    #[test]
    fn removal_recovers_before_reorientation_and_discards_the_crossing() {
        let (mut world, mut desktop, now) = scene();
        attach(&mut desktop);
        ticks(&mut world, &desktop, now, 65);
        let id = world.save.creatures[0].id;
        assert_eq!(world.save.creatures[0].state.surface.monitor_id, 2);
        desktop.monitors.pop();
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 3_500;
        world.tick(now + Duration::milliseconds(3_500), 0.05, &desktop);
        assert_eq!(world.save.creatures[0].state.surface.monitor_id, 1);
        assert!(habitat_contains(
            &world.save.settings.habitat,
            &desktop.monitors[0],
            world.save.creatures[0].state.position
        ));
        assert!(!world.attention.crosses_displays(id));
        ticks(
            &mut world,
            &desktop,
            now + Duration::milliseconds(3_500),
            15,
        );
        assert!(matches!(
            world.attention.plans[&id].role,
            Role::Display { discovery: false }
        ));
        assert!(world.save.creatures[0].state.position.x < 1_432.0);
    }

    #[test]
    fn identifier_and_scale_churn_rebind_silently_without_discovery_or_loss() {
        for native_key_changed in [false, true] {
            let (mut world, mut desktop, now) = scene();
            desktop.monitors[0].id = 91;
            desktop.monitors[0].scale_factor = 1.5;
            if native_key_changed {
                desktop.monitors[0].display_key = DisplayKey([99; 16]);
            }
            desktop.window_sample.as_mut().unwrap().monotonic_millis = 250;
            ticks(&mut world, &desktop, now, 20);
            assert_eq!(world.save.creatures[0].state.surface.monitor_id, 91);
            assert_eq!(world.save.creatures[0].state.position.x, 1_360.0);
            assert!(world.attention.plans.is_empty());
            assert!(!world.display_attention.has_pending());
        }
    }

    #[test]
    fn pause_wake_and_reduced_motion_do_not_replay_or_teleport_exploration() {
        for reason in 0..3 {
            let (mut world, mut desktop, now) = scene();
            match reason {
                0 => world.save.settings.paused = true,
                1 => desktop.cursor_sample_millis = Some(30_000),
                _ => world.save.settings.reduce_motion = true,
            }
            attach(&mut desktop);
            ticks(&mut world, &desktop, now, 25);
            assert_eq!(world.save.creatures[0].state.position.x, 1_360.0);
            assert_eq!(world.save.creatures[0].state.surface.monitor_id, 1);
            if reason == 2 {
                assert!(world.save.creatures[0].state.attention.is_some());
            } else {
                assert!(!world.display_attention.has_pending());
            }
            world.save.settings.paused = false;
            ticks(&mut world, &desktop, now + Duration::seconds(2), 5);
            assert_eq!(world.save.creatures[0].state.surface.monitor_id, 1);
        }
        assert!(std::mem::size_of::<DisplayAttention>() <= 1_024);
    }
    #[test]
    fn removal_recovers_a_whole_colony_into_separate_safe_places_even_while_paused() {
        let (mut world, mut desktop, now) = super::super::tests::open_scene();
        // A full colony, so the fifth and sixth are carried back as surely as the first four.
        super::super::tests::fill_colony(&mut world, &desktop, now);
        assert_eq!(world.save.creatures.len(), MAX_COLONY_CREATURES);
        attach(&mut desktop);
        ticks(&mut world, &desktop, now, 1);
        for (index, creature) in world.save.creatures.iter_mut().enumerate() {
            creature.state.surface.monitor_id = 2;
            creature.state.position.x = 1_800.0 + index as f32 * 90.0;
        }
        world.save.settings.paused = true;
        desktop.monitors.pop();
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 500;
        world.tick(now + Duration::milliseconds(500), 0.05, &desktop);
        for (index, creature) in world.save.creatures.iter().enumerate() {
            assert_eq!(creature.state.surface.monitor_id, 1);
            assert!(habitat_contains(
                &world.save.settings.habitat,
                &desktop.monitors[0],
                creature.state.position
            ));
            for other in &world.save.creatures[..index] {
                assert!(creature.state.position.distance(other.state.position) >= 24.0);
            }
        }
        assert!(!world.display_attention.has_pending());
    }

    #[test]
    fn changing_habitat_mid_discovery_cancels_before_crossing_an_exclusion() {
        let (mut world, mut desktop, now) = scene();
        attach(&mut desktop);
        ticks(&mut world, &desktop, now, 22);
        let id = world.save.creatures[0].id;
        assert!(world.attention.crosses_displays(id));
        let before = world.save.creatures[0].state.position;
        world.save.settings.habitat.preset = HabitatPreset::PrimaryDisplay;
        ticks(&mut world, &desktop, now + Duration::seconds(2), 1);
        assert!(!world.attention.crosses_displays(id));
        assert_eq!(world.save.creatures[0].state.position, before);
        assert_eq!(world.save.creatures[0].state.surface.monitor_id, 1);
    }
    #[test]
    fn a_resolution_change_preserves_a_still_valid_window_contact() {
        let (mut world, mut desktop, now) = super::super::tests::scene();
        let position = world.save.creatures[0].state.position;
        desktop.monitors[0].id = 99;
        desktop.monitors[0].bounds.width += 100.0;
        desktop.monitors[0].usable_bounds.width += 100.0;
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 250;
        ticks(&mut world, &desktop, now, 2);
        assert_eq!(world.save.creatures[0].state.position, position);
        assert_eq!(world.save.creatures[0].state.surface.monitor_id, 99);
        assert_eq!(world.save.creatures[0].state.surface.window_key, Some(701));
        assert!(world.display_attention.discovery.is_none());
    }
    #[test]
    fn discovery_handles_a_display_to_the_left_with_different_scaling() {
        let (mut world, mut desktop, now) = scene();
        world.save.creatures[0].state.position.x = 80.0;
        attach(&mut desktop);
        desktop.monitors[1].bounds.x = -1_440.0;
        desktop.monitors[1].usable_bounds.x = -1_440.0;
        desktop.monitors[1].scale_factor = 1.0;
        ticks(&mut world, &desktop, now, 140);
        assert_eq!(world.save.creatures[0].state.surface.monitor_id, 2);
        assert!(world.save.creatures[0].state.position.x < -70.0);
        assert!(habitat_contains(
            &world.save.settings.habitat,
            &desktop.monitors[1],
            world.save.creatures[0].state.position
        ));
    }

    #[test]
    fn display_loss_during_a_window_route_keeps_the_reorientation_cause() {
        let (mut world, mut desktop, now) = scene();
        attach(&mut desktop);
        ticks(&mut world, &desktop, now, 1);
        let creature = &mut world.save.creatures[0];
        let id = creature.id;
        creature.state.surface.monitor_id = 2;
        creature.state.position = Point {
            x: 1_800.0,
            y: 600.0,
        };
        creature.state.action = ActionKind::Landing;
        world.window_journeys.insert(
            id,
            WindowJourney::Hop(HopJourney {
                start: creature.state.position,
                target: Point {
                    x: 1_850.0,
                    y: 846.0,
                },
                surface: creature.state.surface.clone(),
                elapsed: 0.5,
                duration: 2.0,
            }),
        );
        world.window_routes.insert(
            id,
            WindowRoutePlan {
                repaired: false,
                geometry_hash: 0,
                remaining: VecDeque::new(),
            },
        );
        desktop.monitors.pop();
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 500;
        ticks(&mut world, &desktop, now + Duration::milliseconds(500), 2);
        assert!(!world.window_journeys.contains_key(&id));
        assert!(!world.window_routes.contains_key(&id));
        assert_eq!(world.save.creatures[0].state.surface.monitor_id, 1);
        assert!(matches!(
            world.attention.plans[&id].role,
            Role::Display { discovery: false }
        ));
    }
}
