//! The train, as the overlay shows it: the colony walking to it and getting on, or getting off and
//! walking home.
//!
//! A scene is presentation and nothing else. The world does not tick while one plays, and nothing
//! a scene does is kept: it reads where everyone stood, draws them walking in a copy of the colony
//! made for the frame, and when a homecoming ends everyone is standing exactly where they stood
//! before they left.
//!
//! The train stops at the colony's own display, at the middle of its widest stretch of ground. It
//! always comes in from the left and leaves to the right, as Formiga Hill's own train does, so the
//! journey runs one way from the desktop to the Hill and home again. Whoever is on that ground
//! walks to a door from where they are; whoever is somewhere else — up
//! on a ledge, on another display — comes along the ground from the edge nearest them. Anyone with
//! a long way to go hurries, so nobody keeps the train waiting more than a few seconds. Coming
//! home, each steps off in turn and walks back to its own spot, or off towards the display it
//! lives on. With reduced motion the train does not slide: it is simply there, and then it is not.

use formiga_art::{FRAME_SIZE, TRAIN_GROUND, TRAIN_WIDTH, door_centers};
use formiga_core::{
    ActionKind, CreatureId, DesktopRect, MonitorId, MonitorInfo, Point, SaveFile,
    SurfaceAttachment, SurfaceKind, accessible_regions,
};

/// How long the train takes to pull in, and to pull away.
const PULL_IN_SECS: f32 = 2.6;
const PULL_OUT_SECS: f32 = 2.4;
/// From the train stopping to the first companion stepping on or off, and between each after.
const FIRST_STEP_SECS: f32 = 0.35;
const STEP_SECS: f32 = 0.25;
/// How long the train stands once the last companion is on, or off.
const LINGER_SECS: f32 = 0.8;
/// The longest anyone takes to reach the train, or to walk home from it: anyone farther hurries.
const LONGEST_WALK_SECS: f32 = 5.0;
/// How far behind each other companions set off.
const SET_OFF_SECS: f32 = 0.15;
/// Wheels turning, frames a second, and the two standing frames.
const RUNNING_FPS: f32 = 10.0;
const STANDING_FRAME: u8 = formiga_art::RUNNING_FRAMES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leg {
    Departure,
    Arrival,
}

/// Where the train is this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrainPose {
    pub monitor_id: MonitorId,
    /// The train's left edge, in desktop points.
    pub left: f32,
    /// The ground its wheels stand on: the same line companions stand on.
    pub ground: f32,
    pub frame: u8,
    /// Which way it faces: always to the right, the way it runs.
    pub facing_right: bool,
}

/// What one companion is doing at a moment of the scene.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Shown {
    /// Not on the platform: aboard, or not yet arrived from somewhere else, or gone on home.
    Hidden,
    Standing {
        x: f32,
        facing_right: bool,
    },
    Walking {
        x: f32,
        facing_right: bool,
        for_secs: f32,
        hurrying: bool,
    },
    /// Home, exactly as it was before it left.
    AsItWas,
}

#[derive(Clone, Debug)]
struct Walker {
    id: CreatureId,
    from_x: f32,
    to_x: f32,
    /// Whether it is on the platform before it sets off; one coming from elsewhere is not.
    starts_here: bool,
    /// Whether its walk ends at its own spot; one going on to a ledge or a display is not.
    ends_home: bool,
    sets_off: f32,
    speed: f32,
    hurrying: bool,
    /// Leaving: when it steps aboard.
    aboard_at: f32,
}

impl Walker {
    fn arrives_at(&self) -> f32 {
        self.sets_off + (self.to_x - self.from_x).abs() / self.speed
    }

    fn shown(&self, leg: Leg, t: f32) -> Shown {
        let facing_right = self.to_x >= self.from_x;
        let walking = |t: f32| Shown::Walking {
            x: self.from_x + (self.to_x - self.from_x).signum() * self.speed * (t - self.sets_off),
            facing_right,
            for_secs: t - self.sets_off,
            hurrying: self.hurrying,
        };
        match leg {
            Leg::Departure if t >= self.aboard_at => Shown::Hidden,
            Leg::Departure if t >= self.arrives_at() => Shown::Standing {
                x: self.to_x,
                facing_right,
            },
            Leg::Departure if t >= self.sets_off => walking(t),
            Leg::Departure if self.starts_here => Shown::Standing {
                x: self.from_x,
                facing_right,
            },
            Leg::Departure => Shown::Hidden,
            Leg::Arrival if t < self.sets_off => Shown::Hidden,
            Leg::Arrival if t < self.arrives_at() => walking(t),
            Leg::Arrival if self.ends_home => Shown::AsItWas,
            Leg::Arrival => Shown::Hidden,
        }
    }
}

pub struct TrainScene {
    leg: Leg,
    monitor_id: MonitorId,
    ground: f32,
    facing_right: bool,
    enter_left: f32,
    stop_left: f32,
    exit_left: f32,
    walkers: Vec<Walker>,
    reduce_motion: bool,
    elapsed: f32,
    leave_at: f32,
    ends_at: f32,
}

/// The display the train stops at, and the stretch of ground on it: the village's display when the
/// colony has one, then the primary, then any other the colony may use.
fn station(save: &SaveFile, monitors: &[MonitorInfo]) -> Option<(MonitorInfo, DesktopRect)> {
    let preferred = save
        .home
        .display
        .and_then(|key| monitors.iter().position(|m| m.display_key == key))
        .or_else(|| monitors.iter().position(|m| m.primary))
        .unwrap_or(0);
    let order = std::iter::once(preferred).chain((0..monitors.len()).filter(|i| *i != preferred));
    order
        .filter_map(|index| monitors.get(index))
        .find_map(|monitor| {
            accessible_regions(&save.settings.habitat, monitor)
                .into_iter()
                .max_by(|a, b| a.width.total_cmp(&b.width))
                .map(|region| (monitor.clone(), region))
        })
}

fn ease_out(u: f32) -> f32 {
    1.0 - (1.0 - u.clamp(0.0, 1.0)).powi(3)
}

fn ease_in(u: f32) -> f32 {
    u.clamp(0.0, 1.0).powi(3)
}

impl TrainScene {
    /// The colony leaving, if there is anywhere on the desktop to show it.
    pub fn departure(save: &SaveFile, monitors: &[MonitorInfo]) -> Option<Self> {
        Self::new(Leg::Departure, save, monitors)
    }

    /// The colony coming home, if there is anywhere on the desktop to show it.
    pub fn arrival(save: &SaveFile, monitors: &[MonitorInfo]) -> Option<Self> {
        Self::new(Leg::Arrival, save, monitors)
    }

    fn new(leg: Leg, save: &SaveFile, monitors: &[MonitorInfo]) -> Option<Self> {
        if save.creatures.is_empty() || !save.settings.visible {
            return None;
        }
        let (monitor, region) = station(save, monitors)?;
        let reduce_motion = save.settings.reduce_motion;
        // Desktop points to one art pixel, as the overlay draws a creature on this display.
        let unit = f32::from(save.settings.display_scale.max(1)) / monitor.scale_factor.max(0.5);
        let width = TRAIN_WIDTH as f32 * unit;
        let half_sprite = FRAME_SIZE as f32 * unit / 2.0;
        let ground = region.bottom() - 4.0;
        let stop_left = region.x + region.width / 2.0 - width / 2.0;
        let (off_left, off_right) = (monitor.bounds.x - width - 4.0, monitor.bounds.right() + 4.0);
        // It always runs left to right, engine first, as Formiga Hill's own train does: out of the
        // desktop to the right, in at the Hill from the left, and the same way home again, so the
        // journey never turns round between the two apps.
        let facing_right = true;
        let (enter_left, exit_left) = (off_left, off_right);
        let art_width = TRAIN_WIDTH as f32;
        let mut doors: Vec<f32> = door_centers()
            .into_iter()
            .map(|door| {
                // The art faces right, as drawn.
                let along = if facing_right {
                    door as f32
                } else {
                    art_width - door as f32
                };
                stop_left + along * unit
            })
            .collect();
        doors.sort_by(f32::total_cmp);
        let (floor_left, floor_right) = (region.x + 8.0, region.right() - 8.0);
        // Where each companion is, on this ground, and whether it is really standing there.
        let spots: Vec<(CreatureId, f32, bool, bool, f32)> = save
            .creatures
            .iter()
            .map(|creature| {
                let state = &creature.state;
                let here = state.surface.monitor_id == monitor.id;
                let on_floor = here && state.surface.kind == SurfaceKind::ScreenFloor;
                let x = if here {
                    state.position.x.clamp(floor_left, floor_right)
                } else {
                    // From the edge nearest the display it lives on.
                    let elsewhere = monitors
                        .iter()
                        .find(|m| m.id == state.surface.monitor_id)
                        .map_or(monitor.bounds.right(), |m| {
                            m.bounds.x + m.bounds.width / 2.0
                        });
                    if elsewhere < monitor.bounds.x + monitor.bounds.width / 2.0 {
                        region.x - half_sprite
                    } else {
                        region.right() + half_sprite
                    }
                };
                let pace = 24.0 + creature.personality.activity.clamp(0.0, 1.0) * 34.0;
                (creature.id, x, here, on_floor, pace)
            })
            .collect();
        // Nobody crosses anybody: the leftmost companion takes the leftmost door.
        let mut order: Vec<usize> = (0..spots.len()).collect();
        order.sort_by(|a, b| spots[*a].1.total_cmp(&spots[*b].1));
        let mut walkers = Vec::with_capacity(spots.len());
        for (rank, &index) in order.iter().enumerate() {
            let (id, spot, here, on_floor, pace) = spots[index];
            let door = doors[(rank * doors.len() / spots.len()).min(doors.len() - 1)];
            // Two to a door, a step apart.
            let door = door + if rank % 2 == 0 { -3.0 } else { 3.0 } * unit;
            let (from_x, to_x) = match leg {
                Leg::Departure => (spot, door),
                Leg::Arrival => (door, spot),
            };
            let distance = (to_x - from_x).abs();
            let hurrying = distance / pace > LONGEST_WALK_SECS * 0.6;
            let speed =
                (pace * if hurrying { 2.35 } else { 1.0 }).max(distance / LONGEST_WALK_SECS);
            let sets_off = match leg {
                Leg::Departure => 0.2 + index as f32 * SET_OFF_SECS,
                Leg::Arrival => PULL_IN_SECS + FIRST_STEP_SECS + rank as f32 * STEP_SECS,
            };
            walkers.push(Walker {
                id,
                from_x,
                to_x,
                starts_here: here,
                ends_home: on_floor,
                sets_off,
                speed: speed.max(1.0),
                hurrying,
                aboard_at: 0.0,
            });
        }
        let (leave_at, ends_at) = match leg {
            Leg::Departure => {
                // Whoever was waiting on the platform steps aboard in turn once the train has
                // stopped; anyone who reaches the train after that steps straight in, rather than
                // waiting behind a carriage.
                let mut arrivals: Vec<(usize, f32)> = walkers
                    .iter()
                    .enumerate()
                    .map(|(index, walker)| (index, walker.arrives_at()))
                    .collect();
                arrivals.sort_by(|a, b| a.1.total_cmp(&b.1));
                let stopped = PULL_IN_SECS + FIRST_STEP_SECS;
                let mut queue = stopped - STEP_SECS;
                let mut last: f32 = stopped;
                for (index, arrives) in arrivals {
                    let aboard = if arrives < stopped {
                        queue += STEP_SECS;
                        queue
                    } else {
                        arrives
                    };
                    walkers[index].aboard_at = aboard;
                    last = last.max(aboard);
                }
                let leave_at = last + LINGER_SECS;
                (leave_at, leave_at + PULL_OUT_SECS)
            }
            Leg::Arrival => {
                let last_off = walkers.iter().map(|w| w.sets_off).fold(0.0, f32::max);
                let leave_at = last_off + LINGER_SECS;
                let home = walkers.iter().map(Walker::arrives_at).fold(0.0, f32::max);
                (leave_at, (leave_at + PULL_OUT_SECS).max(home))
            }
        };
        Some(Self {
            leg,
            monitor_id: monitor.id,
            ground,
            facing_right,
            enter_left,
            stop_left,
            exit_left,
            walkers,
            reduce_motion,
            elapsed: 0.0,
            leave_at,
            ends_at: if reduce_motion { leave_at } else { ends_at },
        })
    }

    /// Turn a departure round where it stands: the train, already in, lets everyone off again.
    pub fn turn_back(&self, save: &SaveFile, monitors: &[MonitorInfo]) -> Option<Self> {
        let mut scene = Self::new(Leg::Arrival, save, monitors)?;
        if self.leg == Leg::Departure && self.monitor_id == scene.monitor_id {
            // The same train at the same stop, facing the way it came in, leaving the way it was
            // going; and nobody waits for it to pull in again.
            scene.facing_right = self.facing_right;
            scene.stop_left = self.stop_left;
            scene.exit_left = self.exit_left;
            let skip = PULL_IN_SECS.min(self.elapsed);
            scene.elapsed = skip;
        }
        Some(scene)
    }

    #[cfg(test)]
    pub fn leg(&self) -> Leg {
        self.leg
    }

    pub fn monitor_id(&self) -> MonitorId {
        self.monitor_id
    }

    pub fn advance(&mut self, seconds: f32) {
        self.elapsed += seconds.max(0.0);
    }

    pub fn finished(&self) -> bool {
        self.elapsed >= self.ends_at
    }

    /// Whether everyone is aboard and the train is on its way.
    pub fn pulling_away(&self) -> bool {
        self.leg == Leg::Departure && self.elapsed >= self.leave_at
    }

    /// Seconds the whole scene takes.
    #[cfg(test)]
    pub fn length(&self) -> f32 {
        self.ends_at
    }

    /// Where the train is, while it is on the display.
    pub fn train(&self) -> Option<TrainPose> {
        let t = self.elapsed;
        let pose = |left: f32, frame: u8| TrainPose {
            monitor_id: self.monitor_id,
            left,
            ground: self.ground,
            frame,
            facing_right: self.facing_right,
        };
        if self.reduce_motion {
            return (t < self.leave_at).then(|| pose(self.stop_left, STANDING_FRAME));
        }
        let running = ((t * RUNNING_FPS) as u32 % u32::from(STANDING_FRAME)) as u8;
        if t < PULL_IN_SECS {
            let u = ease_out(t / PULL_IN_SECS);
            Some(pose(
                self.enter_left + (self.stop_left - self.enter_left) * u,
                running,
            ))
        } else if t < self.leave_at {
            Some(pose(self.stop_left, STANDING_FRAME + (t * 2.0) as u8 % 2))
        } else if t < self.leave_at + PULL_OUT_SECS {
            let u = ease_in((t - self.leave_at) / PULL_OUT_SECS);
            Some(pose(
                self.stop_left + (self.exit_left - self.stop_left) * u,
                running,
            ))
        } else {
            None
        }
    }

    /// The colony as it is drawn this frame: whoever is on the platform, where they are and what
    /// they are doing. Everyone else is left out of the copy, so nobody is drawn anywhere else.
    pub fn stage(&self, save: &SaveFile) -> SaveFile {
        let mut staged = save.clone();
        staged.visitors.settle_after_opening();
        let t = self.elapsed;
        staged.creatures.retain_mut(|creature| {
            let Some(walker) = self.walkers.iter().find(|walker| walker.id == creature.id) else {
                return false;
            };
            let (x, facing_right, action, elapsed) = match walker.shown(self.leg, t) {
                Shown::Hidden => return false,
                Shown::AsItWas => return true,
                Shown::Standing { x, facing_right } => (x, facing_right, ActionKind::Idle, t),
                Shown::Walking {
                    x,
                    facing_right,
                    for_secs,
                    hurrying,
                } => (
                    x,
                    facing_right,
                    if hurrying {
                        ActionKind::Sprint
                    } else {
                        ActionKind::Traverse
                    },
                    for_secs,
                ),
            };
            let state = &mut creature.state;
            state.position = Point { x, y: self.ground };
            state.surface = SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id: self.monitor_id,
                window_key: None,
                relative_x: 0.5,
            };
            state.facing_right = facing_right;
            state.action = action;
            state.action_elapsed = elapsed;
            state.velocity = Point::default();
            state.attention = None;
            state.flourish = None;
            state.nudge = None;
            state.beat = None;
            state.indoors = false;
            true
        });
        staged
    }
}

/// Where the train is drawn, in an art frame of [`TRAIN_GROUND`] rows: its top edge sits this many
/// art pixels above the ground line, as a creature's frame sits above its contact point.
pub const TRAIN_RISE: u32 = TRAIN_GROUND + 1;

#[cfg(test)]
mod review;
#[cfg(test)]
mod tests;
