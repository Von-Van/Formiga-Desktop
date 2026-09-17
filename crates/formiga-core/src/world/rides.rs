//! Motion is sampled only for the four actual riders, using native scan timestamps.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RideKind {
    Balance,
    Stumble,
    Grip,
    Elevator,
    Dizzy,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RidePose {
    pub kind: RideKind,
    pub velocity: Point,
}

#[derive(Clone, Copy)]
struct Rider {
    id: CreatureId,
    window: WindowKey,
    bounds: DesktopRect,
    velocity: Point,
    intensity: f32,
    still_for: f32,
    pending_dizzy: bool,
    pose: RidePose,
}

#[derive(Default)]
pub(super) struct RideMemory {
    riders: [Option<Rider>; 4],
    sample_at: Option<f64>,
    clock: f64,
}

impl RideMemory {
    pub fn update(
        &mut self,
        creatures: &[Creature],
        desktop: &DesktopSnapshot,
        dt: f32,
        active: bool,
    ) {
        self.clock += f64::from(dt.max(0.0));
        if !active {
            self.riders.fill(None);
            self.sample_at = None;
            return;
        }
        let at = desktop
            .window_sample
            .map_or(self.clock, |s| s.monotonic_millis as f64 / 1000.0);
        if self.sample_at == Some(at) {
            return;
        }
        let elapsed = self.sample_at.map_or(0.0, |old| at - old) as f32;
        self.sample_at = Some(at);
        let previous = self.riders;
        self.riders.fill(None);
        for (index, c) in creatures.iter().take(4).enumerate() {
            let Some(window) =
                desktop.windows.iter().take(MAX_TOPOLOGY_WINDOWS).find(|w| {
                    Some(w.key) == c.state.surface.window_key && w.visible && !w.minimized
                })
            else {
                continue;
            };
            let old = previous
                .iter()
                .flatten()
                .find(|r| r.id == c.id && r.window == window.key)
                .copied();
            let mut rider = old.unwrap_or(Rider {
                id: c.id,
                window: window.key,
                bounds: window.bounds,
                velocity: Point::default(),
                intensity: 0.0,
                still_for: 0.0,
                pending_dizzy: false,
                pose: RidePose {
                    kind: RideKind::Balance,
                    velocity: Point::default(),
                },
            });
            if old.is_some() && (0.0..=3.0).contains(&elapsed) && elapsed > 0.0 {
                // Platform motion under the rider's own contact, so dragging the far edge of a
                // window is not felt as a ride. Walking along the ledge is excluded as well.
                let relative = c.state.surface.relative_x.clamp(0.05, 0.95);
                let contact = |bounds: DesktopRect| Point {
                    x: bounds.x + bounds.width * relative,
                    y: bounds.y,
                };
                let velocity = Point {
                    x: (contact(window.bounds).x - contact(rider.bounds).x) / elapsed,
                    y: (contact(window.bounds).y - contact(rider.bounds).y) / elapsed,
                };
                let speed = velocity.distance(Point::default());
                let old_speed = rider.velocity.distance(Point::default());
                let reversal = speed > 80.0
                    && old_speed > 80.0
                    && velocity.x * rider.velocity.x + velocity.y * rider.velocity.y < 0.0;
                let acceleration = velocity.distance(rider.velocity) / elapsed;
                let moving = speed > 35.0 && window.bounds != rider.bounds;
                if moving {
                    rider.intensity = (rider.intensity
                        + (speed / 700.0).min(2.0) * elapsed
                        + f32::from(reversal) * 0.6)
                        .min(4.0);
                    rider.still_for = 0.0;
                    rider.pending_dizzy = false;
                    let kind = if speed > 650.0 && c.personality.window_tolerance < 0.6 {
                        RideKind::Grip
                    } else if (reversal || acceleration > 2400.0)
                        && c.personality.window_tolerance < 0.85
                    {
                        RideKind::Stumble
                    } else if velocity.y.abs() > velocity.x.abs() * 1.5 {
                        RideKind::Elevator
                    } else {
                        RideKind::Balance
                    };
                    rider.pose = RidePose { kind, velocity };
                    rider.velocity = velocity;
                } else {
                    rider.still_for += elapsed;
                    if rider.still_for >= 0.6 && rider.intensity > 1.2 {
                        rider.pending_dizzy = true;
                    }
                    rider.intensity = (rider.intensity - elapsed * 0.08).max(0.0);
                    if rider.still_for > 5.0 {
                        rider.pending_dizzy = false;
                        rider.intensity = 0.0;
                    }
                }
            }
            rider.bounds = window.bounds;
            self.riders[index] = Some(rider);
        }
    }
    pub fn pose(&self, id: CreatureId) -> Option<RidePose> {
        self.riders
            .iter()
            .flatten()
            .find(|r| r.id == id && r.still_for < 0.6)
            .map(|r| r.pose)
    }
    pub fn dizzy(&self, id: CreatureId) -> bool {
        self.riders
            .iter()
            .flatten()
            .any(|r| r.id == id && r.pending_dizzy)
    }
    pub fn take_dizzy(&mut self, id: CreatureId) -> bool {
        let Some(r) = self
            .riders
            .iter_mut()
            .flatten()
            .find(|r| r.id == id && r.pending_dizzy)
        else {
            return false;
        };
        r.pending_dizzy = false;
        r.intensity = 0.0;
        true
    }
}
