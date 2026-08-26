use crate::behavior::{BehaviorContext, choose_action, habit_key};
use crate::rng::SeedStream;
use crate::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
use std::collections::BTreeMap;
use time::OffsetDateTime;

const ARRIVAL_DAYS: [i64; 3] = [30, 90, 180];

pub struct World {
    pub save: SaveFile,
    rngs: BTreeMap<CreatureId, ChaCha12Rng>,
    events: Vec<WorldEvent>,
    last_windows: BTreeMap<WindowKey, DesktopRect>,
}

impl World {
    pub fn new(colony_seed: [u8; 32], now: OffsetDateTime, desktop: &DesktopSnapshot) -> Self {
        let streams = SeedStream::new(colony_seed);
        let creature = generate_creature(&streams, 0, desktop, None);
        let save = SaveFile {
            save_version: crate::SAVE_VERSION,
            colony_seed,
            created_at_utc: now,
            maximum_seen_utc: now,
            arrival_state: ArrivalState::default(),
            settings: Settings::default(),
            creatures: vec![creature],
        };
        Self::from_save(save)
    }

    pub fn from_save(mut save: SaveFile) -> Self {
        // Identity and durable drives survive relaunch, but interrupted locomotion and reactions do
        // not. Surface attachments are validated against the first desktop snapshot on the next
        // tick, while every creature resumes from a stable pose.
        for creature in &mut save.creatures {
            creature.state.action = ActionKind::Idle;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = 2.5;
            creature.state.velocity = Point::default();
        }
        let rngs = save
            .creatures
            .iter()
            .map(|creature| (creature.id, ChaCha12Rng::from_seed(creature.behavior_seed)))
            .collect();
        Self {
            save,
            rngs,
            events: Vec::new(),
            last_windows: BTreeMap::new(),
        }
    }

    pub fn tick(&mut self, now: OffsetDateTime, dt: f32, desktop: &DesktopSnapshot) {
        if now > self.save.maximum_seen_utc {
            self.save.maximum_seen_utc = now;
        }
        self.process_arrivals(desktop);
        if self.save.settings.paused {
            return;
        }

        let window_changed =
            window_change_near_creatures(&self.last_windows, desktop, &self.save.creatures);
        update_surface_attachments(
            &mut self.save.creatures,
            desktop,
            &self.last_windows,
            &mut self.events,
        );
        let creature_views = self.save.creatures.clone();
        let mut relationship_updates = Vec::new();
        for creature in &mut self.save.creatures {
            if creature.state.arrival_delay_secs > 0.0 {
                let previous_delay = creature.state.arrival_delay_secs;
                creature.state.arrival_delay_secs = (previous_delay - dt).max(0.0);
                if creature.state.arrival_delay_secs == 0.0 {
                    self.events.push(WorldEvent::CreatureSpawned {
                        creature_id: creature.id,
                    });
                }
                continue;
            }
            update_drives(creature, dt);
            creature.state.cursor_cooldown = (creature.state.cursor_cooldown - dt).max(0.0);
            creature.state.action_elapsed += dt;

            let nearest = creature_views
                .iter()
                .filter(|other| other.id != creature.id && other.state.arrival_delay_secs <= 0.0)
                .map(|other| {
                    (
                        creature.state.position.distance(other.state.position),
                        other.state.position,
                        other.id,
                    )
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let context = BehaviorContext {
                nearest_creature_distance: nearest.map(|item| item.0),
                nearest_creature_position: nearest.map(|item| item.1),
                nearest_creature_id: nearest.map(|item| item.2),
                on_window_ledge: creature.state.surface.kind == SurfaceKind::WindowLedge,
                window_changed_nearby: window_changed.contains(&creature.id),
                hour_utc: now.hour(),
            };

            if creature.state.action_elapsed >= creature.state.action_duration {
                let old = creature.state.action;
                if old == ActionKind::InvestigateCursor {
                    creature.state.cursor_cooldown = creature.state.cursor_cooldown.max(5.0);
                }
                self.events.push(WorldEvent::ActionCompleted {
                    creature_id: creature.id,
                    action: old,
                });
                let rng = self
                    .rngs
                    .get_mut(&creature.id)
                    .expect("creature RNG exists");
                let next = choose_action(creature, desktop, context, rng);
                creature.state.action = next;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = action_duration(next, rng);
                if next == ActionKind::Perch
                    && creature.state.surface.kind == SurfaceKind::ScreenFloor
                    && attach_to_nearby_ledge(creature, desktop)
                {
                    self.events.push(WorldEvent::SurfaceChanged {
                        creature_id: creature.id,
                        kind: SurfaceKind::WindowLedge,
                    });
                }
                reinforce_habit(creature, next, now.hour());
                self.events.push(WorldEvent::ActionStarted {
                    creature_id: creature.id,
                    action: next,
                });
                if old == ActionKind::Sleep && next != ActionKind::Sleep {
                    self.events.push(WorldEvent::CreatureWoke {
                        creature_id: creature.id,
                    });
                } else if old != ActionKind::Sleep && next == ActionKind::Sleep {
                    self.events.push(WorldEvent::CreatureSlept {
                        creature_id: creature.id,
                    });
                }
                if matches!(
                    next,
                    ActionKind::InvestigateCursor | ActionKind::AvoidCursor
                ) {
                    self.events.push(WorldEvent::CursorReaction {
                        creature_id: creature.id,
                        action: next,
                    });
                }
                if matches!(next, ActionKind::ReactToWindow | ActionKind::RideWindow) {
                    self.events.push(WorldEvent::WindowReaction {
                        creature_id: creature.id,
                        action: next,
                    });
                }
                if matches!(
                    next,
                    ActionKind::Greet | ActionKind::Follow | ActionKind::SocialPlay
                ) && let Some((_, _, other_id)) = nearest
                {
                    let gain = if next == ActionKind::Follow {
                        0.004
                    } else {
                        0.012
                    };
                    relationship_updates.push((creature.id, other_id, gain));
                    self.events.push(WorldEvent::SocialInteraction {
                        a: creature.id,
                        b: other_id,
                        action: next,
                    });
                }
            }

            execute_action(creature, desktop, context, dt, nearest);
            constrain_to_surface(creature, desktop);
        }
        for (a, b, gain) in relationship_updates {
            if let Some(creature) = self
                .save
                .creatures
                .iter_mut()
                .find(|creature| creature.id == a)
            {
                let affinity = creature.state.relationships.entry(b).or_insert(0.25);
                *affinity = (*affinity + gain).min(1.0);
            }
            if let Some(creature) = self
                .save
                .creatures
                .iter_mut()
                .find(|creature| creature.id == b)
            {
                let affinity = creature.state.relationships.entry(a).or_insert(0.25);
                *affinity = (*affinity + gain).min(1.0);
            }
        }
        keep_creatures_on_screens(&mut self.save.creatures, desktop);
        self.last_windows = desktop
            .windows
            .iter()
            .map(|window| (window.key, window.bounds))
            .collect();
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = WorldEvent> + '_ {
        self.events.drain(..)
    }

    pub fn reset(&mut self, colony_seed: [u8; 32], now: OffsetDateTime, desktop: &DesktopSnapshot) {
        *self = Self::new(colony_seed, now, desktop);
    }

    fn process_arrivals(&mut self, desktop: &DesktopSnapshot) {
        let streams = SeedStream::new(self.save.colony_seed);
        let mut arrivals_this_tick = 0_u8;
        for (index, days) in ARRIVAL_DAYS.into_iter().enumerate() {
            let due = self.save.created_at_utc + time::Duration::days(days);
            if !self.save.arrival_state.arrived[index] && self.save.maximum_seen_utc >= due {
                let primary = self.save.creatures.first().cloned();
                let mut creature =
                    generate_creature(&streams, index as u8 + 1, desktop, primary.as_ref());
                creature.state.arrival_delay_secs = f32::from(arrivals_this_tick) * 15.0;
                self.rngs
                    .insert(creature.id, ChaCha12Rng::from_seed(creature.behavior_seed));
                if creature.state.arrival_delay_secs == 0.0 {
                    self.events.push(WorldEvent::CreatureSpawned {
                        creature_id: creature.id,
                    });
                }
                self.save.creatures.push(creature);
                self.save.arrival_state.arrived[index] = true;
                arrivals_this_tick += 1;
            }
        }
    }
}

fn generate_creature(
    streams: &SeedStream,
    generation: u8,
    desktop: &DesktopSnapshot,
    parent: Option<&Creature>,
) -> Creature {
    let mut appearance_rng = streams.rng("appearance", generation as u64);
    let mut personality_rng = streams.rng("personality", generation as u64);
    let family =
        parent
            .map(|value| value.appearance.family)
            .unwrap_or_else(|| match appearance_rng.random_range(0..3) {
                0 => BodyFamily::Blob,
                1 => BodyFamily::Hopper,
                _ => BodyFamily::SoftQuadruped,
            });
    let scale_percent = [100, 70, 62, 55][generation.min(3) as usize];
    let face_signature = parent
        .map(|value| value.appearance.face_signature)
        .unwrap_or_else(|| appearance_rng.random());
    let palette_index = parent
        .map(|value| value.appearance.palette_index)
        .unwrap_or_else(|| appearance_rng.random_range(0..12));
    let appearance = AppearanceGenome {
        family,
        logical_size: ((appearance_rng.random_range(34..=40) as f32) * scale_percent as f32 / 100.0)
            .round() as u8,
        body_width: mutate_parent(
            parent.map(|p| p.appearance.body_width),
            &mut appearance_rng,
            16,
            27,
        ),
        body_height: mutate_parent(
            parent.map(|p| p.appearance.body_height),
            &mut appearance_rng,
            13,
            24,
        ),
        head_ratio: mutate_float(
            parent.map(|p| p.appearance.head_ratio),
            &mut appearance_rng,
            0.55,
            1.05,
        ),
        roundness: mutate_float(
            parent.map(|p| p.appearance.roundness),
            &mut appearance_rng,
            0.35,
            1.0,
        ),
        leg_length: mutate_parent(
            parent.map(|p| p.appearance.leg_length),
            &mut appearance_rng,
            2,
            8,
        ),
        foot_size: mutate_parent(
            parent.map(|p| p.appearance.foot_size),
            &mut appearance_rng,
            2,
            5,
        ),
        appendage_style: if let Some(parent) = parent {
            if appearance_rng.random_bool(0.75) {
                parent.appearance.appendage_style
            } else {
                random_appendage(&mut appearance_rng)
            }
        } else {
            random_appendage(&mut appearance_rng)
        },
        appendage_size: mutate_parent(
            parent.map(|p| p.appearance.appendage_size),
            &mut appearance_rng,
            2,
            7,
        ),
        tail_style: if let Some(parent) = parent {
            if appearance_rng.random_bool(0.75) {
                parent.appearance.tail_style
            } else {
                random_tail(&mut appearance_rng)
            }
        } else {
            random_tail(&mut appearance_rng)
        },
        tail_length: mutate_parent(
            parent.map(|p| p.appearance.tail_length),
            &mut appearance_rng,
            2,
            10,
        ),
        eye_size: parent
            .map(|p| p.appearance.eye_size)
            .unwrap_or_else(|| appearance_rng.random_range(1..=2)),
        eye_spacing: parent
            .map(|p| p.appearance.eye_spacing)
            .unwrap_or_else(|| appearance_rng.random_range(3..=7)),
        eye_height: parent
            .map(|p| p.appearance.eye_height)
            .unwrap_or_else(|| appearance_rng.random_range(-2..=2)),
        palette_index,
        pattern: if let Some(parent) = parent {
            if appearance_rng.random_bool(0.6) {
                parent.appearance.pattern
            } else {
                random_pattern(&mut appearance_rng)
            }
        } else {
            random_pattern(&mut appearance_rng)
        },
        pattern_density: mutate_float(
            parent.map(|p| p.appearance.pattern_density),
            &mut appearance_rng,
            0.12,
            0.75,
        ),
        marking_seed: appearance_rng.random(),
        gait_bob: mutate_float(
            parent.map(|p| p.appearance.gait_bob),
            &mut appearance_rng,
            0.15,
            0.9,
        ),
        face_signature,
    };
    let personality = PersonalityGenome {
        activity: personality_rng.random_range(0.2..0.95),
        curiosity: personality_rng.random_range(0.15..0.95),
        boldness: personality_rng.random_range(0.1..0.95),
        playfulness: personality_rng.random_range(0.15..0.95),
        sociability: personality_rng.random_range(0.25..0.95),
        routine_affinity: personality_rng.random_range(0.1..0.9),
        sleep_timing: personality_rng.random_range(0.2..0.9),
        window_tolerance: personality_rng.random_range(0.1..0.95),
        cursor_interest: personality_rng.random_range(0.1..0.95),
        decision_temperature: personality_rng.random_range(0.22..0.75),
    };
    let monitor = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.primary)
        .or_else(|| desktop.monitors.first());
    let (position, monitor_id) = monitor
        .map(|monitor| {
            (
                Point {
                    x: monitor.usable_bounds.x
                        + monitor.usable_bounds.width * 0.35
                        + generation as f32 * 42.0,
                    y: monitor.usable_bounds.bottom() - 4.0,
                },
                monitor.id,
            )
        })
        .unwrap_or((Point { x: 320.0, y: 700.0 }, 0));
    Creature {
        id: u64::from_le_bytes(
            streams.bytes("creature-id", generation as u64)[..8]
                .try_into()
                .unwrap(),
        ),
        generation,
        display_scale_percent: scale_percent,
        appearance,
        personality,
        behavior_seed: streams.bytes("behavior", generation as u64),
        state: CreatureState {
            position,
            velocity: Point::default(),
            facing_right: true,
            action: ActionKind::Idle,
            action_elapsed: 0.0,
            action_duration: 2.5,
            drives: Drives::default(),
            surface: SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id,
                window_key: None,
                relative_x: 0.35,
            },
            habits: BTreeMap::new(),
            relationships: parent
                .map(|parent| BTreeMap::from([(parent.id, 0.85)]))
                .unwrap_or_default(),
            cursor_cooldown: 0.0,
            arrival_delay_secs: 0.0,
        },
    }
}

fn mutate_parent<R: Rng + ?Sized>(parent: Option<u8>, rng: &mut R, min: u8, max: u8) -> u8 {
    parent
        .map(|value| (value as i16 + rng.random_range(-2..=2)).clamp(min as i16, max as i16) as u8)
        .unwrap_or_else(|| rng.random_range(min..=max))
}

fn mutate_float<R: Rng + ?Sized>(parent: Option<f32>, rng: &mut R, min: f32, max: f32) -> f32 {
    parent
        .map(|value| (value + rng.random_range(-0.12..0.12)).clamp(min, max))
        .unwrap_or_else(|| rng.random_range(min..max))
}

fn random_appendage<R: Rng + ?Sized>(rng: &mut R) -> AppendageStyle {
    match rng.random_range(0..6) {
        0 => AppendageStyle::None,
        1 => AppendageStyle::Round,
        2 => AppendageStyle::Pointed,
        3 => AppendageStyle::Leaf,
        4 => AppendageStyle::Droop,
        _ => AppendageStyle::Antenna,
    }
}

fn random_tail<R: Rng + ?Sized>(rng: &mut R) -> TailStyle {
    match rng.random_range(0..5) {
        0 => TailStyle::None,
        1 => TailStyle::Stub,
        2 => TailStyle::Taper,
        3 => TailStyle::Tuft,
        _ => TailStyle::Curl,
    }
}

fn random_pattern<R: Rng + ?Sized>(rng: &mut R) -> PatternKind {
    match rng.random_range(0..7) {
        0 => PatternKind::Solid,
        1 => PatternKind::Patches,
        2 => PatternKind::Spots,
        3 => PatternKind::Stripes,
        4 => PatternKind::Mask,
        5 => PatternKind::Socks,
        _ => PatternKind::Tips,
    }
}

fn update_drives(creature: &mut Creature, dt: f32) {
    let moving = matches!(
        creature.state.action,
        ActionKind::Traverse
            | ActionKind::InvestigateCursor
            | ActionKind::AvoidCursor
            | ActionKind::Follow
    );
    let sleeping = creature.state.action == ActionKind::Sleep;
    if sleeping {
        creature.state.drives.energy = (creature.state.drives.energy + dt * 0.035).min(1.0);
        creature.state.drives.sleep_pressure =
            (creature.state.drives.sleep_pressure - dt * 0.05).max(0.0);
        creature.state.drives.arousal = (creature.state.drives.arousal - dt * 0.08).max(0.0);
    } else {
        creature.state.drives.energy =
            (creature.state.drives.energy - dt * if moving { 0.008 } else { 0.002 }).max(0.0);
        creature.state.drives.sleep_pressure =
            (creature.state.drives.sleep_pressure + dt * 0.0025).min(1.0);
        creature.state.drives.boredom = (creature.state.drives.boredom + dt * 0.004).min(1.0);
        creature.state.drives.social_need =
            (creature.state.drives.social_need + dt * 0.0015).min(1.0);
        creature.state.drives.arousal = (creature.state.drives.arousal - dt * 0.025).max(0.0);
    }
}

fn execute_action(
    creature: &mut Creature,
    desktop: &DesktopSnapshot,
    context: BehaviorContext,
    dt: f32,
    nearest: Option<(f32, Point, CreatureId)>,
) {
    let speed = 24.0 + creature.personality.activity * 34.0;
    let mut target_x = None;
    match creature.state.action {
        ActionKind::Traverse => {
            let direction = if creature.state.facing_right {
                1.0
            } else {
                -1.0
            };
            creature.state.velocity.x = direction * speed;
        }
        ActionKind::InvestigateCursor if desktop.cursor.available => {
            target_x = Some(desktop.cursor.position.x)
        }
        ActionKind::AvoidCursor if desktop.cursor.available => {
            target_x = Some(
                creature.state.position.x
                    + (creature.state.position.x - desktop.cursor.position.x).signum() * 180.0,
            );
            creature.state.cursor_cooldown = 4.0;
            creature.state.drives.arousal = (creature.state.drives.arousal + dt * 0.7).min(1.0);
        }
        ActionKind::Follow | ActionKind::Greet | ActionKind::SocialPlay => {
            target_x = nearest.map(|item| item.1.x);
            creature.state.drives.social_need =
                (creature.state.drives.social_need - dt * 0.08).max(0.0);
        }
        ActionKind::SoloPlay => {
            creature.state.drives.boredom = (creature.state.drives.boredom - dt * 0.09).max(0.0);
        }
        ActionKind::ReactToWindow => {
            creature.state.facing_right = !creature.state.facing_right;
            creature.state.velocity.x = if creature.state.facing_right {
                speed * 1.4
            } else {
                -speed * 1.4
            };
            creature.state.drives.arousal = (creature.state.drives.arousal + dt * 0.5).min(1.0);
        }
        _ => creature.state.velocity.x *= (1.0 - dt * 8.0).max(0.0),
    }
    if let Some(target) = target_x {
        let dx = target - creature.state.position.x;
        creature.state.facing_right = dx >= 0.0;
        creature.state.velocity.x = dx.signum() * speed;
    }
    creature.state.position.x += creature.state.velocity.x * dt;
    if !context.on_window_ledge && creature.state.action == ActionKind::Perch {
        creature.state.velocity.x = 0.0;
    }
}

fn action_duration<R: Rng + ?Sized>(action: ActionKind, rng: &mut R) -> f32 {
    let range = match action {
        ActionKind::Sleep => 18.0..45.0,
        ActionKind::Traverse | ActionKind::Follow => 4.0..11.0,
        ActionKind::InvestigateCursor | ActionKind::AvoidCursor | ActionKind::ReactToWindow => {
            2.0..5.0
        }
        ActionKind::Greet | ActionKind::SocialPlay | ActionKind::SoloPlay => 3.0..8.0,
        _ => 3.0..10.0,
    };
    rng.random_range(range)
}

fn reinforce_habit(creature: &mut Creature, action: ActionKind, hour_utc: u8) {
    for value in creature.state.habits.values_mut() {
        *value *= 0.997;
    }
    let value = creature
        .state
        .habits
        .entry(habit_key(creature, action, hour_utc))
        .or_default();
    *value = (*value + 0.015).min(1.0);
}

fn keep_creatures_on_screens(creatures: &mut [Creature], desktop: &DesktopSnapshot) {
    let primary = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.primary)
        .or_else(|| desktop.monitors.first());
    for creature in creatures {
        let monitor = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == creature.state.surface.monitor_id)
            .or(primary);
        if let Some(monitor) = monitor {
            let bounds = monitor.usable_bounds;
            if creature.state.position.x <= bounds.x + 8.0 {
                creature.state.position.x = bounds.x + 8.0;
                creature.state.facing_right = true;
            } else if creature.state.position.x >= bounds.right() - 8.0 {
                creature.state.position.x = bounds.right() - 8.0;
                creature.state.facing_right = false;
            }
            if !bounds.contains(creature.state.position) {
                creature.state.position = Point {
                    x: creature
                        .state
                        .position
                        .x
                        .clamp(bounds.x + 8.0, bounds.right() - 8.0),
                    y: bounds.bottom() - 4.0,
                };
                creature.state.surface = SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id: monitor.id,
                    window_key: None,
                    relative_x: 0.5,
                };
            }
        }
    }
}

fn update_surface_attachments(
    creatures: &mut [Creature],
    desktop: &DesktopSnapshot,
    previous: &BTreeMap<WindowKey, DesktopRect>,
    events: &mut Vec<WorldEvent>,
) {
    for creature in creatures {
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
                if old.is_some_and(|old| old != window.bounds) {
                    let old = old.expect("changed window has previous bounds");
                    let movement = Point {
                        x: window.bounds.x - old.x,
                        y: window.bounds.y - old.y,
                    };
                    let moved_distance = movement.distance(Point::default());
                    let resized = (window.bounds.width - old.width).abs()
                        + (window.bounds.height - old.height).abs();
                    let rapid = moved_distance > 80.0 || resized > 90.0;
                    creature.state.action = if rapid && creature.personality.window_tolerance < 0.72
                    {
                        creature.state.drives.arousal =
                            (creature.state.drives.arousal + 0.35).min(1.0);
                        ActionKind::ReactToWindow
                    } else {
                        ActionKind::RideWindow
                    };
                    creature.state.action_elapsed = 0.0;
                    creature.state.action_duration = 2.5;
                    events.push(WorldEvent::WindowReaction {
                        creature_id: creature.id,
                        action: creature.state.action,
                    });
                }
            }
            None => {
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
                    events.push(WorldEvent::SurfaceChanged {
                        creature_id: creature.id,
                        kind: SurfaceKind::ScreenFloor,
                    });
                    events.push(WorldEvent::WindowReaction {
                        creature_id: creature.id,
                        action: ActionKind::ReactToWindow,
                    });
                }
            }
        }
    }
}

fn attach_to_nearby_ledge(creature: &mut Creature, desktop: &DesktopSnapshot) -> bool {
    let candidate = desktop
        .windows
        .iter()
        .filter(|window| window.visible && !window.minimized && window.bounds.width >= 120.0)
        .filter_map(|window| {
            let ledge_x = creature
                .state
                .position
                .x
                .clamp(window.bounds.x + 12.0, window.bounds.right() - 12.0);
            let dx = (creature.state.position.x - ledge_x).abs();
            let dy = (creature.state.position.y - window.bounds.y).abs();
            (dx <= 160.0 && dy <= 320.0).then_some((dx + dy * 0.45, window, ledge_x))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let Some((_, window, ledge_x)) = candidate else {
        return false;
    };
    creature.state.position = Point {
        x: ledge_x,
        y: window.bounds.y,
    };
    creature.state.surface = SurfaceAttachment {
        kind: SurfaceKind::WindowLedge,
        monitor_id: creature.state.surface.monitor_id,
        window_key: Some(window.key),
        relative_x: ((ledge_x - window.bounds.x) / window.bounds.width).clamp(0.05, 0.95),
    };
    true
}

fn constrain_to_surface(creature: &mut Creature, desktop: &DesktopSnapshot) {
    if let Some(key) = creature.state.surface.window_key
        && let Some(window) = desktop.windows.iter().find(|window| window.key == key)
    {
        let min_x = window.bounds.x + 12.0;
        let max_x = window.bounds.right() - 12.0;
        if creature.state.position.x <= min_x {
            creature.state.position.x = min_x;
            creature.state.facing_right = true;
        } else if creature.state.position.x >= max_x {
            creature.state.position.x = max_x;
            creature.state.facing_right = false;
        }
        creature.state.position.y = window.bounds.y;
        creature.state.surface.relative_x =
            ((creature.state.position.x - window.bounds.x) / window.bounds.width).clamp(0.05, 0.95);
    }
}

fn window_change_near_creatures(
    previous: &BTreeMap<WindowKey, DesktopRect>,
    desktop: &DesktopSnapshot,
    creatures: &[Creature],
) -> Vec<CreatureId> {
    let mut changed = Vec::new();
    for window in &desktop.windows {
        let moved = previous
            .get(&window.key)
            .is_none_or(|old| old != &window.bounds);
        if moved {
            for creature in creatures {
                let expanded = DesktopRect {
                    x: window.bounds.x - 80.0,
                    y: window.bounds.y - 80.0,
                    width: window.bounds.width + 160.0,
                    height: window.bounds.height + 160.0,
                };
                if expanded.contains(creature.state.position) {
                    changed.push(creature.id);
                }
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn desktop() -> DesktopSnapshot {
        DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1440.0,
                    height: 900.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 1440.0,
                    height: 826.0,
                },
                scale_factor: 2.0,
                primary: true,
            }],
            ..DesktopSnapshot::default()
        }
    }

    #[test]
    fn colony_arrives_on_calendar_thresholds() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut world = World::new([9; 32], created, &desktop());
        world.tick(created + time::Duration::days(29), 0.05, &desktop());
        assert_eq!(world.save.creatures.len(), 1);
        world.tick(created + time::Duration::days(30), 0.05, &desktop());
        assert_eq!(world.save.creatures.len(), 2);
        world.tick(created + time::Duration::days(90), 0.05, &desktop());
        assert_eq!(world.save.creatures.len(), 3);
        world.tick(created + time::Duration::days(180), 0.05, &desktop());
        assert_eq!(world.save.creatures.len(), 4);
    }

    #[test]
    fn clock_rollback_does_not_remove_arrivals() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut world = World::new([2; 32], created, &desktop());
        world.tick(created + time::Duration::days(180), 0.05, &desktop());
        world.tick(created + time::Duration::days(3), 0.05, &desktop());
        assert_eq!(world.save.creatures.len(), 4);
    }

    #[test]
    fn overdue_arrivals_are_present_but_revealed_fifteen_seconds_apart() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut world = World::new([12; 32], created, &desktop());
        world.tick(created + time::Duration::days(181), 0.05, &desktop());
        assert_eq!(world.save.creatures.len(), 4);
        assert_eq!(world.save.creatures[1].state.arrival_delay_secs, 0.0);
        assert!(world.save.creatures[2].state.arrival_delay_secs > 14.0);
        assert!(world.save.creatures[3].state.arrival_delay_secs > 29.0);
        world.tick(created + time::Duration::days(181), 15.0, &desktop());
        assert_eq!(world.save.creatures[2].state.arrival_delay_secs, 0.0);
        assert!(world.save.creatures[3].state.arrival_delay_secs > 14.0);
    }

    #[test]
    fn mini_is_related_but_not_identical() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut world = World::new([4; 32], created, &desktop());
        world.tick(created + time::Duration::days(30), 0.05, &desktop());
        let parent = &world.save.creatures[0];
        let mini = &world.save.creatures[1];
        assert_eq!(parent.appearance.family, mini.appearance.family);
        assert_eq!(
            parent.appearance.face_signature,
            mini.appearance.face_signature
        );
        assert_eq!(
            parent.appearance.palette_index,
            mini.appearance.palette_index
        );
        assert_ne!(parent.appearance.marking_seed, mini.appearance.marking_seed);
        assert_ne!(parent.personality, mini.personality);
    }

    #[test]
    fn perched_creature_rides_and_falls_from_window() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut desktop = desktop();
        desktop.windows.push(DesktopWindow {
            key: 44,
            bounds: DesktopRect {
                x: 200.0,
                y: 300.0,
                width: 600.0,
                height: 400.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
        });
        let mut world = World::new([8; 32], created, &desktop);
        let creature = &mut world.save.creatures[0];
        creature.state.surface = SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: 1,
            window_key: Some(44),
            relative_x: 0.5,
        };
        world.tick(created, 0.05, &desktop);
        assert_eq!(
            world.save.creatures[0].state.position,
            Point { x: 500.0, y: 300.0 }
        );
        desktop.windows[0].bounds.x = 240.0;
        desktop.windows[0].bounds.y = 280.0;
        world.tick(created, 0.05, &desktop);
        assert_eq!(world.save.creatures[0].state.position.y, 280.0);
        assert_eq!(world.save.creatures[0].state.action, ActionKind::RideWindow);
        world.save.creatures[0].personality.window_tolerance = 0.0;
        desktop.windows[0].bounds.x = 500.0;
        world.tick(created, 0.05, &desktop);
        assert_eq!(
            world.save.creatures[0].state.action,
            ActionKind::ReactToWindow
        );
        desktop.windows.clear();
        world.tick(created, 0.05, &desktop);
        assert_eq!(
            world.save.creatures[0].state.surface.kind,
            SurfaceKind::ScreenFloor
        );
        assert_eq!(
            world.save.creatures[0].state.action,
            ActionKind::ReactToWindow
        );
    }
}
