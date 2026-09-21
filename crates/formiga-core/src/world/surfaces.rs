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

/// A landing keeps clear of the corners of whatever holds it: this much of a window's own
/// edges, and this much of a habitat region's. A creature on the ground stands this far above
/// the region's bottom line.
const LEDGE_CLEARANCE: f32 = 12.0;
const FLOOR_CLEARANCE: f32 = 8.0;
const FLOOR_STANDING_LIFT: f32 = 4.0;

/// One surface a creature could come to rest on: the top edge of a window, or the floor of a
/// single habitat region.
///
/// Three searches ask what lies below a point and none of them ask it the same way — a toss is
/// swept along its arc, a drop falls straight down, and a hangout only wants to know how far the
/// ground is. This type is what they share: the geometry of a landing, and nothing about which
/// landing wins. Each search keeps its own rule for what counts and what it prefers.
///
/// The clearances above are applied once, here, so a surface too narrow to stand on yields no
/// span at all rather than a backwards one. That is why [`SupportSpan::nearest_x`] can clamp
/// without ever being handed a minimum above its maximum.
pub(super) struct SupportSpan {
    /// Where a creature's feet end up once this surface holds it.
    pub(super) y: f32,
    /// The window this is the top of, or `None` for habitat floor.
    pub(super) window: Option<WindowKey>,
    monitor_id: MonitorId,
    rect_x: f32,
    rect_width: f32,
    min_x: f32,
    max_x: f32,
}

impl SupportSpan {
    /// The standable part of a window's top edge, or nothing when the window is too narrow.
    fn ledge(window: &DesktopWindow, monitor_id: MonitorId) -> Option<Self> {
        Self::new(
            window.bounds.y,
            window.bounds,
            LEDGE_CLEARANCE,
            Some(window.key),
            monitor_id,
        )
    }

    /// The standable part of one accessible habitat region's floor.
    fn floor(region: DesktopRect, monitor_id: MonitorId) -> Option<Self> {
        Self::new(
            region.bottom() - FLOOR_STANDING_LIFT,
            region,
            FLOOR_CLEARANCE,
            None,
            monitor_id,
        )
    }

    fn new(
        y: f32,
        rect: DesktopRect,
        clearance: f32,
        window: Option<WindowKey>,
        monitor_id: MonitorId,
    ) -> Option<Self> {
        let min_x = rect.x + clearance;
        let max_x = rect.right() - clearance;
        (min_x <= max_x).then_some(Self {
            y,
            window,
            monitor_id,
            rect_x: rect.x,
            rect_width: rect.width,
            min_x,
            max_x,
        })
    }

    /// `x` brought onto the span: a fall that misses a ledge sideways catches its corner.
    pub(super) fn nearest_x(&self, x: f32) -> f32 {
        x.clamp(self.min_x, self.max_x)
    }

    /// Whether a creature could stand at `x` on this surface.
    pub(super) fn holds(&self, x: f32) -> bool {
        (self.min_x..=self.max_x).contains(&x)
    }

    /// Whether the surface reaches under `x` at all, corners included. Measuring a drop is not
    /// the same question as finding somewhere to stand: the last few points of ground before the
    /// screen edge are still ground, and a creature peering over should read a short fall there
    /// rather than an empty one.
    pub(super) fn covers(&self, x: f32) -> bool {
        (self.rect_x..=self.rect_x + self.rect_width).contains(&x)
    }

    /// Standing at `x` on this surface: the exact point, and what the creature is attached to.
    pub(super) fn place(&self, x: f32) -> (Point, SurfaceAttachment) {
        let relative = (x - self.rect_x) / self.rect_width;
        (
            Point { x, y: self.y },
            SurfaceAttachment {
                kind: if self.window.is_some() {
                    SurfaceKind::WindowLedge
                } else {
                    SurfaceKind::ScreenFloor
                },
                monitor_id: self.monitor_id,
                window_key: self.window,
                // A ledge never claims a creature is standing on its very corner.
                relative_x: if self.window.is_some() {
                    relative.clamp(0.05, 0.95)
                } else {
                    relative.clamp(0.0, 1.0)
                },
            },
        )
    }
}

/// Everything on one monitor that could hold a creature: the given windows first, in their own
/// order, and then the monitor's habitat floor. Ledges come first so that a search settling a
/// tie by taking the first answer lands on the ledge rather than the ground beneath it.
///
/// The caller passes only the windows its own question allows to catch anything — an empty slice
/// when the user has turned window ledges off — and the regions it has already worked out, so
/// that it can test its own candidates against them without asking for them twice.
pub(super) fn supports_on<'a>(
    windows: &'a [DesktopWindow],
    regions: &'a [DesktopRect],
    monitor_id: MonitorId,
) -> impl Iterator<Item = SupportSpan> + 'a {
    windows
        .iter()
        .filter(|window| window.visible && !window.minimized)
        .filter_map(move |window| SupportSpan::ledge(window, monitor_id))
        .chain(
            regions
                .iter()
                .filter_map(move |region| SupportSpan::floor(*region, monitor_id)),
        )
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
///
/// This one only wants relative height, so it looks straight down a single column on the
/// creature's own monitor and the highest surface under it wins. It sees no further into the
/// window list than the topology does, because a creature cannot fall onto something the rest
/// of the simulation has already stopped looking at.
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
    let regions = accessible_regions(&settings.habitat, monitor);
    let windows = if settings.window_ledges {
        &desktop.windows[..desktop.windows.len().min(MAX_TOPOLOGY_WINDOWS)]
    } else {
        &[]
    };
    supports_on(windows, &regions, monitor.id)
        .filter(|span| match span.window {
            // Nobody falls onto the ledge they are already standing on, and a ledge buried
            // under another window is not somewhere to land: the creature cannot see it.
            Some(key) => {
                let above = Point { x, y: span.y - 1.0 };
                Some(key) != creature.state.surface.window_key
                    && span.y > y + 1.0
                    && span.holds(x)
                    && habitat_contains(&settings.habitat, monitor, above)
                    && super::attention::point_exposed(above, Some(key), desktop)
            }
            // The floor a creature is already standing on still counts, so that standing on the
            // ground reads as no drop at all rather than as whatever lies further down.
            None => span.y > y - FLOOR_STANDING_LIFT && span.covers(x),
        })
        .map(|span| span.place(x))
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
