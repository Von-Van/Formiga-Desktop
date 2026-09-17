//! Four remembered hangouts per creature, sampled once a second. Nothing here is serialized.
use super::*;

#[derive(Clone, Copy)]
struct Hangout {
    creature: CreatureId,
    window: WindowKey,
    monitor: MonitorId,
    relative_x: f32,
    seconds: f32,
}

pub(super) struct SurfaceMemory {
    places: [Option<Hangout>; 16],
    sample_in: f32,
    pub inspect_in: f32,
    pub next_origin: u64,
}

impl Default for SurfaceMemory {
    fn default() -> Self {
        Self {
            places: [None; 16],
            sample_in: 1.0,
            inspect_in: 18.0,
            next_origin: 0,
        }
    }
}

impl SurfaceMemory {
    pub fn update(
        &mut self,
        creatures: &[Creature],
        desktop: &DesktopSnapshot,
        dt: f32,
        active: bool,
    ) {
        if !active {
            // Do not turn time spent paused or a wake gap into evidence of a favorite place.
            self.sample_in = 1.0;
            self.inspect_in = self.inspect_in.max(18.0);
            return;
        }
        self.inspect_in = (self.inspect_in - dt).max(0.0);
        self.sample_in -= dt;
        if self.sample_in > 0.0 {
            return;
        }
        self.sample_in = 1.0;
        for slot in &mut self.places {
            let Some(place) = slot else {
                continue;
            };
            let available = desktop
                .windows
                .iter()
                .take(MAX_TOPOLOGY_WINDOWS)
                .any(|w| w.key == place.window && w.visible && !w.minimized);
            place.seconds -= if available { 0.05 } else { 3.0 };
            if place.seconds <= 0.0 || !creatures.iter().any(|c| c.id == place.creature) {
                *slot = None;
            }
        }
        for creature in creatures.iter().take(4) {
            let Some(window) = creature.state.surface.window_key else {
                continue;
            };
            if creature.state.arrival_delay_secs > 0.0
                || !super::attention::point_exposed(
                    Point {
                        x: creature.state.position.x,
                        y: creature.state.position.y - 1.0,
                    },
                    Some(window),
                    desktop,
                )
                || !matches!(
                    creature.state.action,
                    ActionKind::Idle | ActionKind::Perch | ActionKind::Sleep
                )
                || !desktop.windows.iter().take(MAX_TOPOLOGY_WINDOWS).any(|w| {
                    w.key == window
                        && w.visible
                        && !w.minimized
                        && (w.bounds.y - creature.state.position.y).abs() < 1.0
                })
            {
                continue;
            }
            let existing = self.places.iter().position(|p| {
                p.is_some_and(|p| {
                    p.creature == creature.id
                        && p.window == window
                        && p.monitor == creature.state.surface.monitor_id
                })
            });
            let slot = existing.or_else(|| {
                let count = self
                    .places
                    .iter()
                    .flatten()
                    .filter(|p| p.creature == creature.id)
                    .count();
                if count < 4 {
                    self.places.iter().position(Option::is_none)
                } else {
                    self.places
                        .iter()
                        .enumerate()
                        .filter_map(|(i, p)| {
                            p.filter(|p| p.creature == creature.id)
                                .map(|p| (i, p.seconds))
                        })
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(i, _)| i)
                }
            });
            let Some(index) = slot else {
                continue;
            };
            let mut place = existing.and_then(|i| self.places[i]).unwrap_or(Hangout {
                creature: creature.id,
                window,
                monitor: creature.state.surface.monitor_id,
                relative_x: creature.state.surface.relative_x,
                seconds: 0.0,
            });
            // Gentle averaging avoids recording a path across the surface.
            place.relative_x += (creature.state.surface.relative_x - place.relative_x) * 0.02;
            place.seconds = (place.seconds + 1.05).min(300.0);
            self.places[index] = Some(place);
        }
    }

    pub fn familiar(&self, creature: CreatureId, window: Option<WindowKey>) -> bool {
        self.places
            .iter()
            .flatten()
            .any(|p| p.creature == creature && Some(p.window) == window && p.seconds >= 60.0)
    }

    pub fn destination(&self, creature: &Creature, desktop: &DesktopSnapshot) -> Option<Point> {
        self.preferred(creature, desktop, false)
    }

    pub fn nearby(&self, creature: &Creature, desktop: &DesktopSnapshot) -> Option<Point> {
        self.preferred(creature, desktop, true)
    }

    fn preferred(
        &self,
        creature: &Creature,
        desktop: &DesktopSnapshot,
        current: bool,
    ) -> Option<Point> {
        self.places
            .iter()
            .flatten()
            .filter(|p| {
                p.creature == creature.id
                    && p.seconds >= 60.0
                    && p.monitor == creature.state.surface.monitor_id
                    && (Some(p.window) == creature.state.surface.window_key) == current
            })
            .filter_map(|p| {
                let window = desktop
                    .windows
                    .iter()
                    .take(MAX_TOPOLOGY_WINDOWS)
                    .find(|w| w.key == p.window && w.visible && !w.minimized)?;
                let point = Point {
                    x: window.bounds.x + window.bounds.width * p.relative_x.clamp(0.05, 0.95),
                    y: window.bounds.y,
                };
                Some((p.seconds, point))
            })
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, point)| point)
    }
}

/// Distance below an exposed edge to the next usable support, in logical desktop points.
/// Absolute monitor origins never enter the risk score.
pub(super) fn drop_below(
    creature: &Creature,
    x: f32,
    desktop: &DesktopSnapshot,
    settings: &Settings,
) -> f32 {
    support_below(creature, x, desktop, settings).map_or(0.0, |(point, _)| {
        (point.y - creature.state.position.y).max(0.0)
    })
}

/// The nearest exposed ledge or habitat floor directly below `x`, never the current support.
pub(super) fn support_below(
    creature: &Creature,
    x: f32,
    desktop: &DesktopSnapshot,
    settings: &Settings,
) -> Option<(Point, SurfaceAttachment)> {
    let y = creature.state.position.y;
    let monitor = desktop
        .monitors
        .iter()
        .find(|m| m.id == creature.state.surface.monitor_id)?;
    let floor = accessible_regions(&settings.habitat, monitor)
        .into_iter()
        .filter(|r| x >= r.x && x <= r.right() && r.bottom() > y)
        .map(|r| {
            (
                Point {
                    x,
                    y: r.bottom() - 4.0,
                },
                SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id: monitor.id,
                    window_key: None,
                    relative_x: ((x - r.x) / r.width).clamp(0.0, 1.0),
                },
            )
        });
    desktop
        .windows
        .iter()
        .take(MAX_TOPOLOGY_WINDOWS)
        .filter(|w| {
            settings.window_ledges
                && w.visible
                && !w.minimized
                && Some(w.key) != creature.state.surface.window_key
                && w.bounds.y > y + 1.0
                && x >= w.bounds.x + 12.0
                && x <= w.bounds.right() - 12.0
        })
        .filter(|w| {
            let point = Point {
                x,
                y: w.bounds.y - 1.0,
            };
            habitat_contains(&settings.habitat, monitor, point)
                && super::attention::point_exposed(point, Some(w.key), desktop)
        })
        .map(|w| {
            (
                Point { x, y: w.bounds.y },
                SurfaceAttachment {
                    kind: SurfaceKind::WindowLedge,
                    monitor_id: monitor.id,
                    window_key: Some(w.key),
                    relative_x: ((x - w.bounds.x) / w.bounds.width).clamp(0.05, 0.95),
                },
            )
        })
        .chain(floor)
        .min_by(|a, b| a.0.y.total_cmp(&b.0.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn fixture() -> (World, DesktopSnapshot) {
        let mut desktop = super::super::tests::desktop();
        desktop.windows.push(DesktopWindow {
            key: 321,
            bounds: DesktopRect {
                x: 200.0,
                y: 400.0,
                width: 400.0,
                height: 200.0,
            },
            visible: true,
            minimized: false,
            z_order: 0,
            application: None,
            application_name: None,
        });
        let mut world = World::new([54; 32], datetime!(2026-01-01 0:00 UTC), &desktop);
        let c = &mut world.save.creatures[0];
        c.state.surface = SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: 1,
            window_key: Some(321),
            relative_x: 0.25,
        };
        c.state.position = Point { x: 300.0, y: 400.0 };
        c.state.action = ActionKind::Perch;
        (world, desktop)
    }

    #[test]
    fn hangouts_need_dwell_decay_when_missing_and_are_never_serialized() {
        let (mut world, mut desktop) = fixture();
        let id = world.save.creatures[0].id;
        let before = serde_json::to_value(&world.save).unwrap();
        for _ in 0..59 {
            world
                .surface_memory
                .update(&world.save.creatures, &desktop, 1.0, true);
        }
        assert!(!world.surface_memory.familiar(id, Some(321)));
        for _ in 0..10 {
            world
                .surface_memory
                .update(&world.save.creatures, &desktop, 1.0, false);
        }
        assert!(!world.surface_memory.familiar(id, Some(321)));
        world
            .surface_memory
            .update(&world.save.creatures, &desktop, 1.0, true);
        assert!(world.surface_memory.familiar(id, Some(321)));
        assert_eq!(serde_json::to_value(&world.save).unwrap(), before);
        desktop.windows.clear();
        for _ in 0..25 {
            world
                .surface_memory
                .update(&world.save.creatures, &desktop, 1.0, true);
        }
        assert!(world.surface_memory.places.iter().all(Option::is_none));
        assert!(std::mem::size_of::<SurfaceMemory>() <= 1024);
    }

    #[test]
    fn exact_hangouts_follow_the_surface_but_only_coarse_regions_reach_saves() {
        let (mut world, mut desktop) = fixture();
        for _ in 0..65 {
            world
                .surface_memory
                .update(&world.save.creatures, &desktop, 1.0, true);
        }
        let c = &world.save.creatures[0];
        desktop.windows[0].bounds.x += 50.0;
        assert_eq!(
            world.surface_memory.nearby(c, &desktop),
            Some(Point { x: 350.0, y: 400.0 })
        );
        world.events.push(WorldEvent::ObservationElapsed {
            creature_id: c.id,
            display: desktop.monitors[0].display_key,
            region: 4,
            on_ledge: true,
            riding_window: false,
            nearby_creature: None,
            active_seconds: 60,
        });
        world.project_events(datetime!(2026-01-01 0:01 UTC));
        assert_eq!(
            world.save.creatures[0]
                .memory
                .preferred_region
                .unwrap()
                .cell,
            4
        );
        assert_eq!(
            world.save.creatures[0]
                .memory
                .preferred_region
                .unwrap()
                .confidence,
            1
        );
        let reloaded = World::from_save(world.save.clone());
        assert!(
            !reloaded
                .surface_memory
                .familiar(world.save.creatures[0].id, Some(321))
        );
        for key in 322..342 {
            desktop.windows[0].key = key;
            world.save.creatures[0].state.surface.window_key = Some(key);
            world
                .surface_memory
                .update(&world.save.creatures, &desktop, 1.0, true);
        }
        assert!(world.surface_memory.places.iter().flatten().count() <= 4);
    }

    #[test]
    fn height_uses_the_next_available_support_and_is_translation_invariant() {
        let (mut world, mut desktop) = fixture();
        let base = drop_below(
            &world.save.creatures[0],
            615.0,
            &desktop,
            &world.save.settings,
        );
        let mut lower = desktop.windows[0].clone();
        lower.key = 322;
        lower.bounds = DesktopRect {
            x: 560.0,
            y: 480.0,
            width: 300.0,
            height: 200.0,
        };
        lower.z_order = 1;
        desktop.windows.push(lower);
        assert!(base > 80.0);
        assert_eq!(
            drop_below(
                &world.save.creatures[0],
                615.0,
                &desktop,
                &world.save.settings
            ),
            80.0
        );
        for w in &mut desktop.windows {
            w.bounds.x -= 1800.0;
            w.bounds.y -= 1200.0;
        }
        for m in &mut desktop.monitors {
            m.bounds.x -= 1800.0;
            m.bounds.y -= 1200.0;
            m.usable_bounds.x -= 1800.0;
            m.usable_bounds.y -= 1200.0;
        }
        world.save.creatures[0].state.position.x -= 1800.0;
        world.save.creatures[0].state.position.y -= 1200.0;
        assert_eq!(
            drop_below(
                &world.save.creatures[0],
                615.0 - 1800.0,
                &desktop,
                &world.save.settings
            ),
            80.0
        );
        desktop.windows[1].minimized = true;
        assert_eq!(
            drop_below(
                &world.save.creatures[0],
                615.0 - 1800.0,
                &desktop,
                &world.save.settings
            ),
            base
        );
    }
}
