//! Riders carried along between window reads. The window list is read four times a second while a
//! window could be moving under somebody, and the world puts a rider wherever its window was at the
//! last read, so a rider on a window being dragged at a few hundred points a second jumps a body's
//! length or more four times a second while the window glides under it. Between reads the overlay
//! carries each rider on by the motion the last two reads measured (`World::ride_drift`), for as
//! long as the pointer dragging it keeps moving, and the next read puts it back exactly on its
//! window. Only where it is drawn changes: the world, its save and the grab windows are untouched,
//! and nothing is read but the window rectangles and the pointer it already reads.

use super::*;

/// The furthest past a window read a rider is carried: one read's interval at the quickest rate
/// windows are read, so a late read never lets a rider run on by itself.
pub(super) const CARRY_LIMIT_SECS: f32 = 0.25;

/// A pointer moving slower than this, in points a second, has stopped dragging for now, and so has
/// the window it drags: a rider is held where it has been carried to until the next read.
const POINTER_STILL: f32 = 20.0;

/// How far past the last window read riders are being carried.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct RideCarry {
    /// Seconds of dragging since the last read: the time a dragged window has moved for since.
    since_read: f32,
}

impl RideCarry {
    /// A fresh window list was read: everyone stands exactly where the world puts them.
    pub(super) fn read(&mut self) {
        self.since_read = 0.0;
    }

    /// `dt` seconds passed with no fresh read. A dragged window moves only while the button is
    /// held and the pointer moves, so only that time counts.
    pub(super) fn advance(&mut self, dt: f32, button_down: bool, pointer: Point) {
        if button_down && pointer.distance(Point::default()) >= POINTER_STILL {
            self.since_read = (self.since_read + dt.max(0.0)).min(CARRY_LIMIT_SECS);
        }
    }

    /// How far to carry each rider whose window the last reads found moving, in desktop points.
    pub(super) fn offsets(&self, world: &World) -> Vec<(CreatureId, Point)> {
        if self.since_read <= 0.0 {
            return Vec::new();
        }
        world
            .save
            .creatures
            .iter()
            .filter_map(|creature| {
                let velocity = world.ride_drift(creature.id)?;
                Some((
                    creature.id,
                    Point {
                        x: velocity.x * self.since_read,
                        y: velocity.y * self.since_read,
                    },
                ))
            })
            .collect()
    }
}

/// Whether anybody is standing on a window the last reads found moving: the overlay then draws at
/// its quickest pace, so there are frames between reads for a rider to be carried along in.
pub(super) fn anyone_riding_a_moving_window(world: &World) -> bool {
    world
        .save
        .creatures
        .iter()
        .any(|creature| world.ride_drift(creature.id).is_some())
}

#[cfg(test)]
mod tests {
    use super::super::cadence::{
        FREQUENT_WINDOW_READ, frame_due, tick_interval_for_reads, world_needs_frequent_window_scan,
        world_redraw_interval, world_tick_interval,
    };
    use super::*;
    use time::macros::datetime;

    const WINDOW: WindowKey = 701;

    fn desktop() -> DesktopSnapshot {
        DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: DisplayKey([1; 16]),
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 2560.0,
                    height: 1440.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 2560.0,
                    height: 1366.0,
                },
                scale_factor: 2.0,
                primary: true,
            }],
            windows: vec![DesktopWindow {
                key: WINDOW,
                bounds: DesktopRect {
                    x: 300.0,
                    y: 700.0,
                    width: 600.0,
                    height: 400.0,
                },
                z_order: 0,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            }],
            window_sample: Some(WindowSample {
                monotonic_millis: 0,
                reliable: true,
            }),
            ..DesktopSnapshot::default()
        }
    }

    /// One companion standing on the window's top, settled, the village put away.
    fn rider(desktop: &DesktopSnapshot) -> (World, time::OffsetDateTime) {
        let start = datetime!(2026-05-02 10:00 UTC);
        let mut world = World::new([23; 32], start, desktop);
        world.tick(start, 0.05, desktop);
        world.save.home.active_since_utc = None;
        world.save.home.last_disappeared_utc = Some(start);
        let creature = &mut world.save.creatures[0];
        creature.state.arrival_delay_secs = 0.0;
        creature.state.action = ActionKind::Idle;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 100.0;
        creature.personality.window_tolerance = 1.0;
        creature.state.surface = SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: 1,
            window_key: Some(WINDOW),
            relative_x: 0.5,
        };
        creature.state.position = Point { x: 600.0, y: 700.0 };
        world.tick(start, 0.05, desktop);
        world.drain_events().for_each(drop);
        (world, start)
    }

    /// When the drag starts letting go, a little after a read rather than on one.
    const STOP: f64 = 2.1;

    /// Where the dragged window's left edge is `seconds` into a drag: still for half a second,
    /// then dragged right at `speed` points a second until `STOP`, then let stop over `stopping`
    /// seconds and held still, the button still down, for the rest.
    fn dragged_x(seconds: f64, speed: f64, stopping: f64) -> f64 {
        let moving = (seconds - 0.5).clamp(0.0, STOP - 0.5);
        let slowing = (seconds - STOP).clamp(0.0, stopping);
        let slowed = if stopping > 0.0 {
            speed * (slowing - slowing * slowing / (2.0 * stopping))
        } else {
            0.0
        };
        300.0 + speed * moving + slowed
    }

    #[derive(Debug, Default)]
    struct Ride {
        /// The biggest move a rider made between two presented frames in the drag's first half
        /// second, before two reads have measured how fast its window is going.
        starting_step: f32,
        /// The biggest move a rider made between two presented frames once its window's motion
        /// has been measured, until the window stops.
        largest_step: f32,
        /// The furthest a rider was drawn from its window's real contact point over that time.
        largest_error: f32,
        /// How far a rider was drawn past where the window stopped.
        overshoot: f32,
        /// Frames presented while the window moved.
        frames_while_moving: u32,
    }

    /// A rider on a window dragged as `dragged_x` describes, ticked, read and drawn exactly as
    /// the app schedules it: a fresh window list on a tick at least a quarter second after the
    /// last, each frame placed with `RideCarry`, or not when `carry` is off. Drawn positions are
    /// compared with where the window really was at that moment.
    fn ride(speed: f64, stopping: f64, carry: bool, pointer_gate: bool) -> Ride {
        let mut desktop = desktop();
        let (mut world, start) = rider(&desktop);
        let id = world.save.creatures[0].id;
        let mut clock = 0.0_f64;
        let mut last_read = -10.0_f64;
        let mut carried = RideCarry::default();
        let mut previous_x = dragged_x(0.0, speed, stopping);
        let base = Instant::now();
        let (mut redraw_due, mut last_frame) = (base, None);
        let mut shown: Option<f32> = None;
        let mut result = Ride::default();
        while clock < 3.0 {
            // Ticked as `FormigaApp::tick_interval` ticks: the colony's own pace, brought forward
            // to the next window read while one is due every quarter second.
            let base_now = base + Duration::from_secs_f64(clock);
            let frequent = world_needs_frequent_window_scan(&world, true);
            let dt = tick_interval_for_reads(
                world_tick_interval(&world),
                frequent.then(|| base + Duration::from_secs_f64(last_read.max(0.0))),
                base_now,
            )
            .as_secs_f64();
            clock += dt;
            let now = base + Duration::from_secs_f64(clock);
            let window_x = dragged_x(clock, speed, stopping);
            let pointer = Point {
                x: ((window_x - previous_x) / dt) as f32,
                y: 0.0,
            };
            previous_x = window_x;
            let interval = if frequent {
                FREQUENT_WINDOW_READ.as_secs_f64()
            } else {
                1.0
            };
            if clock - last_read >= interval - 1e-9 {
                last_read = clock;
                desktop.windows[0].bounds.x = window_x as f32;
                desktop.window_sample = Some(WindowSample {
                    monotonic_millis: (clock * 1000.0) as u64,
                    reliable: true,
                });
                carried.read();
            } else {
                let pointer = if pointer_gate {
                    pointer
                } else {
                    Point { x: 1.0e6, y: 0.0 }
                };
                carried.advance(dt as f32, true, pointer);
            }
            world.tick(
                start + time::Duration::seconds_f64(clock),
                dt as f32,
                &desktop,
            );
            world.drain_events().for_each(drop);
            let creature = &world.save.creatures[0];
            assert_eq!(creature.state.surface.window_key, Some(WINDOW));
            let interval = world_redraw_interval(&world);
            redraw_due = frame_due(redraw_due, last_frame, interval);
            if now < redraw_due {
                continue;
            }
            last_frame = Some(now);
            let phased = redraw_due + interval;
            redraw_due = if phased > now { phased } else { now + interval };
            let offset = if carry {
                carried
                    .offsets(&world)
                    .iter()
                    .find(|(who, _)| *who == id)
                    .map_or(0.0, |(_, offset)| offset.x)
            } else {
                0.0
            };
            let drawn = creature.state.position.x + offset;
            let truth = (window_x + 600.0 * f64::from(creature.state.surface.relative_x)) as f32;
            let step = shown.map_or(0.0, |before| (drawn - before).abs());
            if (0.5..STOP + stopping).contains(&clock) {
                result.frames_while_moving += 1;
            }
            if (0.5..1.0).contains(&clock) {
                result.starting_step = result.starting_step.max(step);
            } else if (1.0..STOP).contains(&clock) {
                result.largest_step = result.largest_step.max(step);
                result.largest_error = result.largest_error.max((drawn - truth).abs());
            }
            if clock >= STOP + stopping {
                let stopped = (dragged_x(10.0, speed, stopping)
                    + 600.0 * f64::from(creature.state.surface.relative_x))
                    as f32;
                result.overshoot = result.overshoot.max(drawn - stopped);
            }
            shown = Some(drawn);
        }
        result
    }

    /// A rider on a window dragged steadily at an everyday speed is carried along with it
    /// between window reads, rather than left behind and jumping a body's length or more four
    /// times a second, and the read after the window stops puts it back exactly on its ledge.
    #[test]
    fn a_rider_is_carried_between_window_reads_and_set_right_by_the_next() {
        for speed in [300.0, 600.0, 1200.0] {
            let before = ride(speed, 0.0, false, true);
            let after = ride(speed, 0.0, true, true);
            let ungated = ride(speed, 0.0, true, false);
            let gentle = ride(speed, 0.3, true, true);
            eprintln!(
                "{speed} pt/s: before {before:?}\n  after {after:?}\n  \
                 without the pointer check {ungated:?}\n  stopping over 0.3 s {gentle:?}"
            );
            // Frames come at the moving rate, so there is somewhere to carry a rider to.
            assert!(after.frames_while_moving >= 28, "{after:?}");
            // Between frames a rider moves no further than the window does in one frame.
            assert!(
                after.largest_step <= speed as f32 * 0.05 * 1.05 + 1.0,
                "{speed}: {after:?}"
            );
            assert!(after.largest_step < before.largest_step / 2.0, "{speed}");
            // ...and it stays with its window, give or take what a stop it could not see coming
            // carries it past, which the next read corrects.
            assert!(after.largest_error < before.largest_error, "{speed}");
            assert!(
                after.overshoot <= speed as f32 * 0.05 + 1.0,
                "{speed}: {after:?}"
            );
            assert!(
                gentle.overshoot <= speed as f32 * 0.1 + 1.0,
                "{speed}: {gentle:?}"
            );
        }
    }

    /// The carry itself: nothing straight after a read, the measured velocity times the time
    /// dragged since, held while the pointer rests, capped at one read's interval, and nothing
    /// again once the next read is in.
    #[test]
    fn the_carry_is_the_measured_velocity_times_the_time_dragged_since_the_read() {
        let mut desktop = desktop();
        let (mut world, start) = rider(&desktop);
        let id = world.save.creatures[0].id;
        let mut carry = RideCarry::default();
        // Two reads a quarter second apart, the window 150 points further right: 600 pt/s.
        for (millis, x) in [(250_u64, 300.0), (500, 450.0)] {
            desktop.windows[0].bounds.x = x;
            desktop.window_sample = Some(WindowSample {
                monotonic_millis: millis,
                reliable: true,
            });
            carry.read();
            world.tick(
                start + time::Duration::milliseconds(millis as i64),
                0.25,
                &desktop,
            );
        }
        let velocity = world
            .ride_drift(id)
            .expect("a moving window under the rider");
        assert!((velocity.x - 600.0).abs() < 0.5 && velocity.y.abs() < 0.5);
        assert!(
            carry.offsets(&world).is_empty(),
            "nothing straight after a read"
        );
        let dragging = Point { x: 600.0, y: 0.0 };
        carry.advance(0.05, true, dragging);
        carry.advance(0.05, true, dragging);
        let offset = carry.offsets(&world)[0];
        assert_eq!(offset.0, id);
        assert!((offset.1.x - 60.0).abs() < 0.5, "{offset:?}");
        // The pointer rests, or the button comes up: the rider stays where it was carried.
        carry.advance(0.05, true, Point::default());
        carry.advance(0.05, false, dragging);
        assert!((carry.offsets(&world)[0].1.x - 60.0).abs() < 0.5);
        // Never further than one read's interval, however late the read is.
        for _ in 0..20 {
            carry.advance(0.05, true, dragging);
        }
        assert!((carry.offsets(&world)[0].1.x - 600.0 * CARRY_LIMIT_SECS).abs() < 0.5);
        // The next read finds the window where it stopped: back exactly on the window, and
        // nothing to carry it by any more.
        desktop.window_sample = Some(WindowSample {
            monotonic_millis: 750,
            reliable: true,
        });
        carry.read();
        world.tick(start + time::Duration::milliseconds(750), 0.25, &desktop);
        assert!(carry.offsets(&world).is_empty());
        assert!(world.ride_drift(id).is_none());
        let creature = &world.save.creatures[0];
        assert_eq!(
            creature.state.position.x,
            450.0 + 600.0 * creature.state.surface.relative_x
        );
        carry.advance(0.05, true, dragging);
        assert!(
            carry.offsets(&world).is_empty(),
            "a still window carries no one"
        );
    }
}
