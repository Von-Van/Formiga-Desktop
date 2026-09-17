//! Constant-size cursor observations: one previous sample and one local movement aggregate.
use crate::{DesktopSnapshot, MonitorId, Point};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CursorInterest {
    Fast,
    Local,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CursorCue {
    pub origin: u64,
    pub kind: CursorInterest,
    pub point: Point,
    pub monitor: MonitorId,
    remaining: f32,
}

#[derive(Default)]
pub(crate) struct CursorObserver {
    previous: Option<(Point, f64, MonitorId)>,
    center: Point,
    direction: Point,
    age: f32,
    distance: f32,
    turns: u8,
    fast_seconds: f32,
    turn_age: f32,
    announced_local: bool,
    cue: Option<CursorCue>,
    next_origin: u64,
    clock: f64,
    pub safe: bool,
}

impl CursorObserver {
    pub fn cue(&self) -> Option<CursorCue> {
        self.cue
    }

    pub fn reset(&mut self) {
        self.previous = None;
        self.cue = None;
        self.safe = false;
        self.fast_seconds = 0.0;
        self.reset_local(Point::default());
    }

    fn reset_local(&mut self, point: Point) {
        self.center = point;
        self.direction = Point::default();
        self.age = 0.0;
        self.distance = 0.0;
        self.turns = 0;
        self.turn_age = 0.0;
        self.announced_local = false;
    }

    pub fn update(&mut self, desktop: &DesktopSnapshot, dt: f32, active: bool) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.clock += f64::from(dt);
        if let Some(cue) = &mut self.cue {
            cue.remaining -= dt;
            if cue.remaining <= 0.0 {
                self.cue = None;
            }
        }
        let point = desktop.cursor.position;
        let monitor = desktop.monitors.iter().find(|m| m.bounds.contains(point));
        if !active
            || !desktop.cursor.available
            || !point.x.is_finite()
            || !point.y.is_finite()
            || monitor.is_none()
        {
            self.reset();
            return;
        }
        let monitor = monitor.unwrap().id;
        let at = desktop
            .cursor_sample_millis
            .map_or(self.clock, |millis| millis as f64 / 1_000.0);
        if let Some((_, old_at, _)) = self.previous
            && at == old_at
        {
            return;
        }
        let previous = self.previous.replace((point, at, monitor));
        let Some((old, old_at, old_monitor)) = previous else {
            self.reset_local(point);
            self.safe = false;
            return;
        };
        let elapsed = (at - old_at) as f32;
        let delta = Point {
            x: point.x - old.x,
            y: point.y - old.y,
        };
        let distance = point.distance(old);
        let speed = distance / elapsed.max(0.001);
        if !(0.0..=0.75).contains(&elapsed)
            || elapsed == 0.0
            || old_monitor != monitor
            || distance > 240.0
            || speed > 4_500.0
        {
            self.reset_local(point);
            self.fast_seconds = 0.0;
            self.cue = None;
            self.safe = false;
            return;
        }
        self.safe = true;
        self.age += elapsed;
        self.turn_age += elapsed;
        if point.distance(self.center) > 64.0 || self.age > 4.0 {
            self.reset_local(point);
        }
        if distance >= 3.0 {
            let direction = Point {
                x: delta.x / distance,
                y: delta.y / distance,
            };
            if self.turn_age >= 0.08
                && self.direction.distance(Point::default()) > 0.5
                && direction.x * self.direction.x + direction.y * self.direction.y < 0.8
            {
                self.turns = self.turns.saturating_add(1).min(8);
                self.turn_age = 0.0;
            }
            self.direction = direction;
            self.distance = (self.distance + distance).min(512.0);
        }
        if speed >= 600.0 && distance >= 12.0 {
            self.fast_seconds = (self.fast_seconds + elapsed).min(0.25);
        } else {
            self.fast_seconds = 0.0;
        }
        let kind = if !self.announced_local
            && self.age >= 0.7
            && self.turns >= 3
            && self.distance >= 100.0
        {
            self.announced_local = true;
            Some(CursorInterest::Local)
        } else if self.fast_seconds >= 0.08 && self.cue.is_none() {
            Some(CursorInterest::Fast)
        } else {
            None
        };
        if let Some(kind) = kind {
            self.next_origin = self.next_origin.wrapping_add(1);
            self.cue = Some(CursorCue {
                origin: self.next_origin,
                kind,
                point: if kind == CursorInterest::Local {
                    self.center
                } else {
                    point
                },
                monitor,
                remaining: if kind == CursorInterest::Local {
                    3.5
                } else {
                    1.5
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attention::tests::snapshot;

    fn sample(observer: &mut CursorObserver, desktop: &mut DesktopSnapshot, at: u64, point: Point) {
        desktop.cursor.available = true;
        desktop.cursor.position = point;
        // Classification derives speed from normalized positions and real sample timestamps.
        desktop.cursor.velocity = Point {
            x: 99_999.0,
            y: 99_999.0,
        };
        desktop.cursor_sample_millis = Some(at);
        observer.update(desktop, 0.05, true);
    }

    #[test]
    fn fast_motion_requires_fresh_samples_and_ignores_warps() {
        let mut observer = CursorObserver::default();
        let mut desktop = snapshot(0, 0);
        sample(&mut observer, &mut desktop, 0, Point { x: 500.0, y: 800.0 });
        sample(
            &mut observer,
            &mut desktop,
            50,
            Point { x: 540.0, y: 800.0 },
        );
        assert!(observer.cue().is_none());
        for _ in 0..10 {
            observer.update(&desktop, 0.05, true);
        }
        assert!(observer.cue().is_none());
        sample(
            &mut observer,
            &mut desktop,
            100,
            Point { x: 580.0, y: 800.0 },
        );
        assert_eq!(observer.cue().unwrap().kind, CursorInterest::Fast);
        sample(
            &mut observer,
            &mut desktop,
            150,
            Point {
                x: 1_300.0,
                y: 800.0,
            },
        );
        assert!(!observer.safe && observer.cue().is_none());
        sample(
            &mut observer,
            &mut desktop,
            200,
            Point {
                x: 1_300.0,
                y: 800.0,
            },
        );
        assert!(observer.safe && observer.cue().is_none());
    }

    #[test]
    fn repeated_local_circles_invite_interest_but_rest_and_straight_motion_do_not() {
        for mode in 0..3 {
            let mut observer = CursorObserver::default();
            let mut desktop = snapshot(0, 0);
            for step in 0..=20 {
                let angle = step as f32 * std::f32::consts::FRAC_PI_4;
                let point = match mode {
                    0 => Point {
                        x: 600.0 + angle.cos() * 24.0,
                        y: 800.0 + angle.sin() * 24.0,
                    },
                    1 => Point { x: 600.0, y: 800.0 },
                    _ => Point {
                        x: 600.0 + step as f32 * 12.0,
                        y: 800.0,
                    },
                };
                sample(&mut observer, &mut desktop, step * 100, point);
            }
            assert_eq!(
                observer
                    .cue()
                    .is_some_and(|c| c.kind == CursorInterest::Local),
                mode == 0
            );
        }
    }

    #[test]
    fn disable_unavailable_stale_and_cross_display_samples_clear_the_aggregate() {
        for reason in 0..4 {
            let mut observer = CursorObserver::default();
            let mut desktop = snapshot(0, 0);
            sample(&mut observer, &mut desktop, 0, Point { x: 500.0, y: 800.0 });
            sample(
                &mut observer,
                &mut desktop,
                50,
                Point { x: 540.0, y: 800.0 },
            );
            sample(
                &mut observer,
                &mut desktop,
                100,
                Point { x: 580.0, y: 800.0 },
            );
            match reason {
                0 => {}
                1 => desktop.cursor.available = false,
                2 => desktop.cursor_sample_millis = Some(30_000),
                _ => {
                    desktop.monitors[0].id = 2;
                    desktop.cursor_sample_millis = Some(150);
                }
            }
            observer.update(&desktop, 0.05, reason != 0);
            assert!(!observer.safe && observer.cue().is_none());
        }
        assert!(std::mem::size_of::<CursorObserver>() <= 256);
    }
}
