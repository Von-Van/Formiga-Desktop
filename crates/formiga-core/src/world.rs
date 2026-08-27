use crate::behavior::{BehaviorContext, choose_action, habit_key};
use crate::rng::SeedStream;
use crate::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
use std::collections::BTreeMap;
use time::OffsetDateTime;

const ARRIVAL_DAYS: [i64; 3] = [30, 90, 180];
/// Width of a creature's art frame, matching `formiga_art::FRAME_SIZE`. The simulation crate
/// cannot depend on the art crate, so shelter layout mirrors the constant the way `home_anchor`
/// already mirrors the shelter's own half-width.
const CREATURE_ART_WIDTH: f32 = 48.0;
/// How far apart homebound creatures sit, as a fraction of their drawn width. Below roughly half
/// they overlap enough to hide each other; much above it they stop reading as a colony at home.
const HOME_SPACING_RATIO: f32 = 0.55;
const HOME_DURATION: time::Duration = time::Duration::minutes(15);
const HOME_COOLDOWN: time::Duration = time::Duration::minutes(15);

pub struct World {
    pub save: SaveFile,
    rngs: BTreeMap<CreatureId, ChaCha12Rng>,
    events: Vec<WorldEvent>,
    last_windows: BTreeMap<WindowKey, DesktopRect>,
    drag: Option<DragSession>,
    ledge_journeys: BTreeMap<CreatureId, LedgeJourney>,
}

#[derive(Clone)]
struct DragSession {
    creature_id: CreatureId,
    grab_offset: Point,
    original_position: Point,
    original_surface: SurfaceAttachment,
    original_action: ActionKind,
}

#[derive(Clone)]
struct LedgeJourney {
    start: Point,
    target: Point,
    surface: SurfaceAttachment,
    elapsed: f32,
    duration: f32,
}

impl World {
    pub fn new(colony_seed: [u8; 32], now: OffsetDateTime, desktop: &DesktopSnapshot) -> Self {
        let streams = SeedStream::new(colony_seed);
        let creature = generate_creature(&streams, 0, desktop, None);
        let home_display = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.primary)
            .or_else(|| desktop.monitors.first())
            .map(|monitor| monitor.display_key);
        let save = SaveFile {
            save_version: crate::SAVE_VERSION,
            colony_seed,
            created_at_utc: now,
            maximum_seen_utc: now,
            arrival_state: ArrivalState::default(),
            home: ColonyHome::from_seed(colony_seed, home_display, Some(now), None),
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
            drag: None,
            ledge_journeys: BTreeMap::new(),
        }
    }

    pub fn tick(&mut self, now: OffsetDateTime, dt: f32, desktop: &DesktopSnapshot) {
        if now > self.save.maximum_seen_utc {
            self.save.maximum_seen_utc = now;
        }
        self.process_arrivals(desktop);
        let home_active = self.update_home_cycle(desktop);
        if self.save.settings.paused {
            return;
        }
        if home_active {
            self.tick_homebound_creatures(dt);
            self.last_windows = desktop
                .windows
                .iter()
                .map(|window| (window.key, window.bounds))
                .collect();
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
            if self
                .drag
                .as_ref()
                .is_some_and(|drag| drag.creature_id == creature.id)
            {
                continue;
            }
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

            if let Some((position, complete, surface)) =
                self.ledge_journeys.get_mut(&creature.id).map(|journey| {
                    journey.elapsed += dt;
                    let progress = (journey.elapsed / journey.duration).clamp(0.0, 1.0);
                    let arc = (progress * std::f32::consts::PI).sin()
                        * journey
                            .start
                            .distance(journey.target)
                            .mul_add(0.12, 24.0)
                            .min(90.0);
                    (
                        Point {
                            x: journey.start.x + (journey.target.x - journey.start.x) * progress,
                            y: journey.start.y + (journey.target.y - journey.start.y) * progress
                                - arc,
                        },
                        progress >= 1.0,
                        journey.surface.clone(),
                    )
                })
            {
                creature.state.facing_right = position.x >= creature.state.position.x;
                creature.state.position = position;
                if complete {
                    self.ledge_journeys.remove(&creature.id);
                    creature.state.surface = surface;
                    creature.state.action = ActionKind::Perch;
                    creature.state.action_elapsed = 0.0;
                    creature.state.action_duration = 3.5;
                    creature.state.velocity = Point::default();
                    self.events.push(WorldEvent::ActionCompleted {
                        creature_id: creature.id,
                        action: ActionKind::Landing,
                    });
                    self.events.push(WorldEvent::SurfaceChanged {
                        creature_id: creature.id,
                        kind: SurfaceKind::WindowLedge,
                    });
                    self.events.push(WorldEvent::ActionStarted {
                        creature_id: creature.id,
                        action: ActionKind::Perch,
                    });
                }
                continue;
            }

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
                // A ledge is a destination, not a one-time upgrade from the desktop floor.
                // Continuing to search while perched lets creatures climb between stacked
                // application windows and later descend when the desktop arrangement changes.
                reachable_window_ledge: find_nearby_ledge(
                    creature,
                    desktop,
                    &self.save.settings.habitat,
                )
                .is_some(),
                window_changed_nearby: window_changed.contains(&creature.id),
                hour_utc: now.hour(),
            };

            if context.window_changed_nearby
                && creature.state.action != ActionKind::ReactToWindow
                && creature.state.action_elapsed >= 0.25
            {
                creature.state.action = ActionKind::ReactToWindow;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = 2.2;
                creature.state.drives.arousal = (creature.state.drives.arousal + 0.3).min(1.0);
                self.events.push(WorldEvent::WindowReaction {
                    creature_id: creature.id,
                    action: ActionKind::ReactToWindow,
                });
                self.events.push(WorldEvent::ActionStarted {
                    creature_id: creature.id,
                    action: ActionKind::ReactToWindow,
                });
            }

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
                let selected = choose_action(creature, desktop, context, rng);
                let mut next = selected;
                if selected == ActionKind::Perch
                    && let Some((target, surface)) =
                        find_nearby_ledge(creature, desktop, &self.save.settings.habitat)
                {
                    let distance = creature.state.position.distance(target);
                    self.ledge_journeys.insert(
                        creature.id,
                        LedgeJourney {
                            start: creature.state.position,
                            target,
                            surface,
                            elapsed: 0.0,
                            duration: (distance / 280.0).clamp(1.0, 2.6),
                        },
                    );
                    next = ActionKind::Landing;
                }
                creature.state.action = next;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = action_duration(next, rng);
                reinforce_habit(creature, selected, now.hour());
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
            constrain_to_surface(creature, desktop, &self.save.settings.habitat);
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
        let unconstrained: Vec<_> = self
            .drag
            .as_ref()
            .map(|drag| drag.creature_id)
            .into_iter()
            .chain(self.ledge_journeys.keys().copied())
            .collect();
        keep_creatures_in_habitat(
            &mut self.save.creatures,
            desktop,
            &self.save.settings.habitat,
            &unconstrained,
        );
        self.last_windows = desktop
            .windows
            .iter()
            .map(|window| (window.key, window.bounds))
            .collect();
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = WorldEvent> + '_ {
        self.events.drain(..)
    }

    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    fn update_home_cycle(&mut self, desktop: &DesktopSnapshot) -> bool {
        let timeline_now = self.save.maximum_seen_utc;
        if self
            .save
            .home
            .active_since_utc
            .is_some_and(|started| timeline_now - started >= HOME_DURATION)
        {
            self.dismiss_home(timeline_now, false);
        }

        let due = !self.save.home.is_active()
            && self
                .save
                .home
                .last_disappeared_utc
                .is_none_or(|ended| timeline_now - ended >= HOME_COOLDOWN);
        if due && self.drag.is_none() && self.resolve_home_monitor(desktop).is_some() {
            self.save.home.active_since_utc = Some(timeline_now);
            self.ledge_journeys.clear();
            self.events.push(WorldEvent::HomeAppeared);
        }

        if self.save.home.is_active() {
            self.place_colony_at_home(desktop);
        }
        self.save.home.is_active()
    }

    fn resolve_home_monitor(&mut self, desktop: &DesktopSnapshot) -> Option<MonitorInfo> {
        let preferred = self.save.home.display;
        let monitor = preferred
            .and_then(|display| {
                desktop.monitors.iter().find(|monitor| {
                    monitor.display_key == display
                        && !accessible_regions(&self.save.settings.habitat, monitor).is_empty()
                })
            })
            .or_else(|| {
                desktop.monitors.iter().find(|monitor| {
                    monitor.primary
                        && !accessible_regions(&self.save.settings.habitat, monitor).is_empty()
                })
            })
            .or_else(|| {
                desktop.monitors.iter().find(|monitor| {
                    !accessible_regions(&self.save.settings.habitat, monitor).is_empty()
                })
            })?
            .clone();
        self.save.home.display = Some(monitor.display_key);
        Some(monitor)
    }

    fn place_colony_at_home(&mut self, desktop: &DesktopSnapshot) {
        let Some(monitor) = self.resolve_home_monitor(desktop) else {
            return;
        };
        let Some(anchor) = resolved_home_anchor(
            &self.save.home,
            &monitor,
            self.save.settings.display_scale,
            &self.save.settings.habitat,
        ) else {
            return;
        };
        let inward = match self.save.home.corner {
            HomeCorner::BottomLeft => 1.0,
            HomeCorner::BottomRight => -1.0,
        };
        // Space the colony by a fraction of how wide a creature actually draws. A flat 18 points
        // was narrower than a single creature at every supported scale, so the whole colony piled
        // onto one point at the shelter and only the last creature drawn stayed visible.
        let creature_width = CREATURE_ART_WIDTH * f32::from(self.save.settings.display_scale)
            / monitor.scale_factor.max(1.0);
        let spacing = creature_width * HOME_SPACING_RATIO;
        for creature in &mut self.save.creatures {
            let offset = inward * f32::from(creature.generation) * spacing;
            let mut position = Point {
                x: anchor.x + offset,
                y: anchor.y,
            };
            if !habitat_contains(&self.save.settings.habitat, &monitor, position) {
                position = anchor;
            }
            creature.state.position = position;
            creature.state.velocity = Point::default();
            creature.state.facing_right = inward > 0.0;
            creature.state.surface = SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id: monitor.id,
                window_key: None,
                relative_x: ((position.x - monitor.usable_bounds.x) / monitor.usable_bounds.width)
                    .clamp(0.0, 1.0),
            };
            if creature.state.action != ActionKind::Homebound {
                creature.state.action = ActionKind::Homebound;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = HOME_DURATION.whole_seconds() as f32;
            }
        }
    }

    fn tick_homebound_creatures(&mut self, dt: f32) {
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
            creature.state.action_elapsed += dt;
            creature.state.drives.comfort = (creature.state.drives.comfort + dt * 0.01).min(1.0);
            creature.state.drives.arousal = (creature.state.drives.arousal - dt * 0.04).max(0.0);
        }
    }

    fn dismiss_home(&mut self, now: OffsetDateTime, interrupted: bool) {
        if !self.save.home.is_active() {
            return;
        }
        self.save.home.active_since_utc = None;
        self.save.home.last_disappeared_utc = Some(now);
        for creature in &mut self.save.creatures {
            if creature.state.action == ActionKind::Homebound {
                creature.state.action = ActionKind::Idle;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = 2.5;
                creature.state.velocity = Point::default();
            }
        }
        self.events
            .push(WorldEvent::HomeDisappeared { interrupted });
    }

    pub fn handle_command(&mut self, command: WorldCommand, desktop: &DesktopSnapshot) -> bool {
        match command {
            WorldCommand::BeginDrag {
                creature_id,
                cursor,
            } => self.begin_drag(creature_id, cursor),
            WorldCommand::UpdateDrag { cursor } => self.update_drag(cursor, desktop),
            WorldCommand::EndDrag { cursor } => self.end_drag(cursor, desktop),
            WorldCommand::CancelDrag => self.cancel_drag(),
            WorldCommand::GatherCreatures => {
                self.gather_creatures(desktop);
                true
            }
        }
    }

    fn begin_drag(&mut self, creature_id: CreatureId, cursor: Point) -> bool {
        if self.drag.is_some() || !self.save.settings.direct_manipulation {
            return false;
        }
        let Some(creature_index) = self.save.creatures.iter().position(|creature| {
            creature.id == creature_id && creature.state.arrival_delay_secs <= 0.0
        }) else {
            return false;
        };
        if self.save.home.is_active() {
            self.dismiss_home(self.save.maximum_seen_utc, true);
        }
        let creature = &mut self.save.creatures[creature_index];
        self.drag = Some(DragSession {
            creature_id,
            grab_offset: Point {
                x: cursor.x - creature.state.position.x,
                y: cursor.y - creature.state.position.y,
            },
            original_position: creature.state.position,
            original_surface: creature.state.surface.clone(),
            original_action: creature.state.action,
        });
        creature.state.action = ActionKind::Dragged;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = f32::MAX;
        creature.state.velocity = Point::default();
        creature.state.surface.window_key = None;
        self.events.push(WorldEvent::DragStarted { creature_id });
        true
    }

    fn update_drag(&mut self, cursor: Point, desktop: &DesktopSnapshot) -> bool {
        let Some(drag) = &self.drag else { return false };
        let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == drag.creature_id)
        else {
            self.drag = None;
            return false;
        };
        creature.state.position = Point {
            x: cursor.x - drag.grab_offset.x,
            y: cursor.y - drag.grab_offset.y,
        };
        if let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.bounds.contains(cursor))
        {
            creature.state.surface.monitor_id = monitor.id;
        }
        true
    }

    fn end_drag(&mut self, cursor: Point, desktop: &DesktopSnapshot) -> bool {
        let Some(drag) = self.drag.take() else {
            return false;
        };
        let policy = self.save.settings.habitat.clone();
        let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == drag.creature_id)
        else {
            return false;
        };
        let support = find_drop_support(cursor, desktop, &policy).or_else(|| {
            nearest_habitat_point(&policy, &desktop.monitors, cursor).map(
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
        let Some((position, surface)) = support else {
            creature.state.position = drag.original_position;
            creature.state.surface = drag.original_surface;
            creature.state.action = drag.original_action;
            return false;
        };
        creature.state.position = position;
        creature.state.surface = surface.clone();
        creature.state.velocity = Point::default();
        creature.state.action_elapsed = 0.0;
        if self.save.settings.paused || self.save.settings.reduce_motion {
            creature.state.action = ActionKind::Idle;
            creature.state.action_duration = 3.0;
        } else {
            creature.state.action = ActionKind::Landing;
            creature.state.action_duration = 0.55;
        }
        creature.state.drives.arousal = (creature.state.drives.arousal + 0.08).min(1.0);
        self.events.push(WorldEvent::DragEnded {
            creature_id: creature.id,
            surface: surface.kind,
        });
        self.events.push(WorldEvent::SurfaceChanged {
            creature_id: creature.id,
            kind: surface.kind,
        });
        true
    }

    fn cancel_drag(&mut self) -> bool {
        let Some(drag) = self.drag.take() else {
            return false;
        };
        if let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == drag.creature_id)
        {
            creature.state.position = drag.original_position;
            creature.state.surface = drag.original_surface;
            creature.state.action = drag.original_action;
            creature.state.action_elapsed = 0.0;
            creature.state.velocity = Point::default();
        }
        true
    }

    fn gather_creatures(&mut self, desktop: &DesktopSnapshot) {
        let policy = self.save.settings.habitat.clone();
        for creature in &mut self.save.creatures {
            if let Some((monitor_id, position)) =
                nearest_habitat_point(&policy, &desktop.monitors, creature.state.position)
            {
                creature.state.position = position;
                creature.state.surface = SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id,
                    window_key: None,
                    relative_x: 0.5,
                };
                creature.state.action = ActionKind::Idle;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = 3.0;
                creature.state.velocity = Point::default();
            }
        }
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
        head_appendages: HeadAppendageGenome {
            style: if let Some(parent) = parent {
                if appearance_rng.random_bool(0.75) {
                    parent.appearance.head_appendages.style
                } else {
                    random_head_appendage(&mut appearance_rng)
                }
            } else {
                random_head_appendage(&mut appearance_rng)
            },
            size: mutate_parent(
                parent.map(|p| p.appearance.head_appendages.size),
                &mut appearance_rng,
                2,
                7,
            ),
        },
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
        face: generate_face(parent.map(|p| p.appearance.face), &mut appearance_rng),
        forelimbs: generate_forelimbs(
            family,
            parent.map(|p| p.appearance.forelimbs),
            &mut appearance_rng,
        ),
        effect_motif: if let Some(parent) = parent {
            if appearance_rng.random_bool(0.65) {
                parent.appearance.effect_motif
            } else {
                random_effect_motif(&mut appearance_rng)
            }
        } else {
            random_effect_motif(&mut appearance_rng)
        },
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

fn random_head_appendage<R: Rng + ?Sized>(rng: &mut R) -> HeadAppendageStyle {
    match rng.random_range(0..6) {
        0 => HeadAppendageStyle::None,
        1 => HeadAppendageStyle::Round,
        2 => HeadAppendageStyle::Pointed,
        3 => HeadAppendageStyle::Leaf,
        4 => HeadAppendageStyle::Droop,
        _ => HeadAppendageStyle::Antenna,
    }
}

fn generate_face<R: Rng + ?Sized>(parent: Option<FaceGenome>, rng: &mut R) -> FaceGenome {
    if let Some(parent) = parent {
        return FaceGenome {
            // These genes form the inherited face signature shared by the colony.
            eye_shape: parent.eye_shape,
            eye_size: parent.eye_size,
            eye_spacing: parent.eye_spacing,
            vertical_offset: parent.vertical_offset,
            pupil_style: parent.pupil_style,
            highlight_style: parent.highlight_style,
            brow_style: if rng.random_bool(0.28) {
                random_brow(rng)
            } else {
                parent.brow_style
            },
            mouth_style: if rng.random_bool(0.2) {
                random_mouth(rng)
            } else {
                parent.mouth_style
            },
            cheek_style: if rng.random_bool(0.35) {
                random_cheek(rng)
            } else {
                parent.cheek_style
            },
        };
    }
    FaceGenome {
        eye_shape: match rng.random_range(0..3) {
            0 => EyeShape::Round,
            1 => EyeShape::Tall,
            _ => EyeShape::SoftSquare,
        },
        eye_size: rng.random_range(1..=2),
        eye_spacing: rng.random_range(4..=7),
        vertical_offset: rng.random_range(-1..=1),
        pupil_style: match rng.random_range(0..3) {
            0 => PupilStyle::Dot,
            1 => PupilStyle::Wide,
            _ => PupilStyle::Spark,
        },
        highlight_style: match rng.random_range(0..3) {
            0 => HighlightStyle::Single,
            1 => HighlightStyle::Double,
            _ => HighlightStyle::Diagonal,
        },
        brow_style: random_brow(rng),
        mouth_style: random_mouth(rng),
        cheek_style: random_cheek(rng),
    }
}

fn generate_forelimbs<R: Rng + ?Sized>(
    family: BodyFamily,
    parent: Option<ForelimbGenome>,
    rng: &mut R,
) -> ForelimbGenome {
    let (style, tip_style) = match family {
        BodyFamily::Blob => (
            if rng.random_bool(0.5) {
                ForelimbStyle::SoftNub
            } else {
                ForelimbStyle::Pseudopod
            },
            LimbTipStyle::Round,
        ),
        BodyFamily::Hopper => (ForelimbStyle::MittenArm, LimbTipStyle::Mitten),
        BodyFamily::SoftQuadruped => (ForelimbStyle::FrontPaw, LimbTipStyle::Paw),
    };
    ForelimbGenome {
        style: parent.map_or(style, |value| value.style),
        length: mutate_parent(parent.map(|value| value.length), rng, 3, 7),
        thickness: mutate_parent(parent.map(|value| value.thickness), rng, 1, 2),
        tip_style: parent.map_or(tip_style, |value| value.tip_style),
        rest_pose: if let Some(parent) = parent {
            if rng.random_bool(0.3) {
                random_rest_pose(rng)
            } else {
                parent.rest_pose
            }
        } else {
            random_rest_pose(rng)
        },
    }
}

fn random_brow<R: Rng + ?Sized>(rng: &mut R) -> BrowStyle {
    match rng.random_range(0..3) {
        0 => BrowStyle::None,
        1 => BrowStyle::Soft,
        _ => BrowStyle::Bold,
    }
}

fn random_mouth<R: Rng + ?Sized>(rng: &mut R) -> MouthStyle {
    match rng.random_range(0..4) {
        0 => MouthStyle::Tiny,
        1 => MouthStyle::Smile,
        2 => MouthStyle::Cat,
        _ => MouthStyle::Beak,
    }
}

fn random_cheek<R: Rng + ?Sized>(rng: &mut R) -> CheekStyle {
    match rng.random_range(0..3) {
        0 => CheekStyle::None,
        1 => CheekStyle::Dots,
        _ => CheekStyle::Blush,
    }
}

fn random_rest_pose<R: Rng + ?Sized>(rng: &mut R) -> RestPose {
    match rng.random_range(0..3) {
        0 => RestPose::AtSides,
        1 => RestPose::Folded,
        _ => RestPose::Together,
    }
}

fn random_effect_motif<R: Rng + ?Sized>(rng: &mut R) -> EffectMotif {
    match rng.random_range(0..6) {
        0 => EffectMotif::None,
        1 => EffectMotif::Dot,
        2 => EffectMotif::Star,
        3 => EffectMotif::Heart,
        4 => EffectMotif::Leaf,
        _ => EffectMotif::Spark,
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
            | ActionKind::Sprint
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
        ActionKind::Eat => {
            creature.state.drives.energy = (creature.state.drives.energy + dt * 0.025).min(1.0);
            creature.state.drives.comfort = (creature.state.drives.comfort + dt * 0.018).min(1.0);
            creature.state.drives.boredom = (creature.state.drives.boredom - dt * 0.018).max(0.0);
        }
        ActionKind::Drink => {
            creature.state.drives.comfort = (creature.state.drives.comfort + dt * 0.024).min(1.0);
            creature.state.drives.arousal = (creature.state.drives.arousal - dt * 0.04).max(0.0);
            creature.state.drives.curiosity_satisfaction =
                (creature.state.drives.curiosity_satisfaction + dt * 0.012).min(1.0);
        }
        ActionKind::ReactToWindow => {
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
        ActionKind::Sprint => 2.5..5.5,
        ActionKind::InvestigateCursor | ActionKind::AvoidCursor | ActionKind::ReactToWindow => {
            2.0..5.0
        }
        ActionKind::Greet | ActionKind::SocialPlay | ActionKind::SoloPlay => 3.0..8.0,
        ActionKind::Eat | ActionKind::Drink => 5.0..9.0,
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

fn keep_creatures_in_habitat(
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
            .or(primary);
        if let Some(monitor) = monitor {
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

fn find_nearby_ledge(
    creature: &Creature,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
) -> Option<(Point, SurfaceAttachment)> {
    let current_window = creature.state.surface.window_key;
    let candidate = desktop
        .windows
        .iter()
        .filter(|window| window.visible && !window.minimized && window.bounds.width >= 120.0)
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
            (dx <= 360.0
                && (36.0..=640.0).contains(&dy)
                && habitat_contains(
                    policy,
                    monitor,
                    Point {
                        x: ledge_x,
                        y: window.bounds.y,
                    },
                ))
            // Nearby intermediate ledges remain easiest, while the vertical-progress bonus makes
            // a visibly different elevation preferable to another almost-level window.
            .then_some((dx * 0.65 + dy * 0.12, window, ledge_x, monitor.id))
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

fn constrain_to_surface(
    creature: &mut Creature,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
) {
    if let Some(key) = creature.state.surface.window_key
        && let Some(window) = desktop.windows.iter().find(|window| window.key == key)
    {
        let Some(monitor) = desktop
            .monitors
            .iter()
            .find(|monitor| monitor.id == creature.state.surface.monitor_id)
        else {
            return;
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
            return;
        };
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

fn distance_to_interval(value: f32, interval: (f32, f32)) -> f32 {
    if value < interval.0 {
        interval.0 - value
    } else if value > interval.1 {
        value - interval.1
    } else {
        0.0
    }
}

fn find_drop_support(
    cursor: Point,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
) -> Option<(Point, SurfaceAttachment)> {
    let monitor = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.bounds.contains(cursor))
        .or_else(|| desktop.monitors.iter().find(|monitor| monitor.primary))
        .or_else(|| desktop.monitors.first())?;
    let regions = accessible_regions(policy, monitor);
    let mut candidates = Vec::new();
    for window in desktop
        .windows
        .iter()
        .filter(|window| window.visible && !window.minimized)
    {
        let x = cursor
            .x
            .clamp(window.bounds.x + 12.0, window.bounds.right() - 12.0);
        let point = Point {
            x,
            y: window.bounds.y,
        };
        if point.y >= cursor.y && regions.iter().any(|region| region.contains(point)) {
            candidates.push((
                cursor.distance(point),
                point,
                SurfaceAttachment {
                    kind: SurfaceKind::WindowLedge,
                    monitor_id: monitor.id,
                    window_key: Some(window.key),
                    relative_x: ((x - window.bounds.x) / window.bounds.width).clamp(0.05, 0.95),
                },
            ));
        }
    }
    for region in regions {
        let point = Point {
            x: cursor.x.clamp(region.x + 8.0, region.right() - 8.0),
            y: region.bottom() - 4.0,
        };
        if point.y >= cursor.y {
            candidates.push((
                cursor.distance(point),
                point,
                SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id: monitor.id,
                    window_key: None,
                    relative_x: ((point.x - region.x) / region.width).clamp(0.0, 1.0),
                },
            ));
        }
    }
    candidates
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, point, surface)| (point, surface))
}

fn window_change_near_creatures(
    previous: &BTreeMap<WindowKey, DesktopRect>,
    desktop: &DesktopSnapshot,
    creatures: &[Creature],
) -> Vec<CreatureId> {
    let mut changed = std::collections::BTreeSet::new();
    for window in &desktop.windows {
        let moved = previous
            .get(&window.key)
            .is_none_or(|old| old != &window.bounds);
        if moved {
            for creature in creatures {
                if distance_to_rect(creature.state.position, window.bounds) <= 260.0 {
                    changed.insert(creature.id);
                }
            }
        }
    }
    for (key, bounds) in previous {
        if !desktop.windows.iter().any(|window| window.key == *key) {
            for creature in creatures {
                if distance_to_rect(creature.state.position, *bounds) <= 260.0 {
                    changed.insert(creature.id);
                }
            }
        }
    }
    changed.into_iter().collect()
}

fn distance_to_rect(point: Point, rect: DesktopRect) -> f32 {
    let dx = if point.x < rect.x {
        rect.x - point.x
    } else if point.x > rect.right() {
        point.x - rect.right()
    } else {
        0.0
    };
    let dy = if point.y < rect.y {
        rect.y - point.y
    } else if point.y > rect.bottom() {
        point.y - rect.bottom()
    } else {
        0.0
    };
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn desktop() -> DesktopSnapshot {
        DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: DisplayKey([1; 16]),
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

    fn let_colony_wander(world: &mut World, now: OffsetDateTime) {
        world.save.home.active_since_utc = None;
        world.save.home.last_disappeared_utc = Some(now);
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
        assert_eq!(
            parent.appearance.face.eye_shape,
            mini.appearance.face.eye_shape
        );
        assert_eq!(
            parent.appearance.face.eye_size,
            mini.appearance.face.eye_size
        );
        assert_eq!(
            parent.appearance.face.eye_spacing,
            mini.appearance.face.eye_spacing
        );
        assert_eq!(
            parent.appearance.face.pupil_style,
            mini.appearance.face.pupil_style
        );
        assert_eq!(
            parent.appearance.face.highlight_style,
            mini.appearance.face.highlight_style
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
            application: None,
            application_name: None,
        });
        let mut world = World::new([8; 32], created, &desktop);
        let_colony_wander(&mut world, created);
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

    #[test]
    fn drag_release_lands_inside_the_desktop_habitat() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([21; 32], created, &desktop);
        let creature_id = world.save.creatures[0].id;
        let original = world.save.creatures[0].state.position;
        assert!(world.handle_command(
            WorldCommand::BeginDrag {
                creature_id,
                cursor: original,
            },
            &desktop,
        ));
        assert_eq!(world.save.creatures[0].state.action, ActionKind::Dragged);
        assert!(world.handle_command(
            WorldCommand::UpdateDrag {
                cursor: Point { x: 900.0, y: 300.0 },
            },
            &desktop,
        ));
        assert!(world.handle_command(
            WorldCommand::EndDrag {
                cursor: Point { x: 900.0, y: 300.0 },
            },
            &desktop,
        ));
        let creature = &world.save.creatures[0];
        assert_eq!(creature.state.action, ActionKind::Landing);
        assert!(
            desktop.monitors[0]
                .usable_bounds
                .contains(creature.state.position)
        );
        assert_eq!(creature.state.surface.kind, SurfaceKind::ScreenFloor);
    }

    #[test]
    fn cancelled_drag_restores_the_last_safe_state() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([22; 32], created, &desktop);
        let creature_id = world.save.creatures[0].id;
        let original_position = world.save.creatures[0].state.position;
        let original_surface = world.save.creatures[0].state.surface.clone();
        let original_action = world.save.creatures[0].state.action;
        assert!(world.handle_command(
            WorldCommand::BeginDrag {
                creature_id,
                cursor: original_position,
            },
            &desktop,
        ));
        world.handle_command(
            WorldCommand::UpdateDrag {
                cursor: Point {
                    x: -500.0,
                    y: -500.0,
                },
            },
            &desktop,
        );
        assert!(world.handle_command(WorldCommand::CancelDrag, &desktop));
        let creature = &world.save.creatures[0];
        assert_eq!(creature.state.position, original_position);
        assert_eq!(creature.state.surface, original_surface);
        assert_eq!(creature.state.action, original_action);
    }

    #[test]
    fn nearby_window_change_interrupts_a_long_running_action() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut desktop = desktop();
        let mut world = World::new([31; 32], created, &desktop);
        let_colony_wander(&mut world, created);
        world.tick(created, 0.05, &desktop);
        let position = world.save.creatures[0].state.position;
        world.save.creatures[0].state.action = ActionKind::Sleep;
        world.save.creatures[0].state.action_elapsed = 5.0;
        world.save.creatures[0].state.action_duration = 100.0;
        desktop.windows.push(DesktopWindow {
            key: 91,
            bounds: DesktopRect {
                x: position.x - 180.0,
                y: position.y - 260.0,
                width: 360.0,
                height: 120.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
        world.tick(created, 0.05, &desktop);
        assert_eq!(
            world.save.creatures[0].state.action,
            ActionKind::ReactToWindow
        );
        assert!(world.drain_events().any(|event| matches!(
            event,
            WorldEvent::WindowReaction {
                action: ActionKind::ReactToWindow,
                ..
            }
        )));
    }

    #[test]
    fn ledge_journey_visibly_moves_before_attaching() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut desktop = desktop();
        let mut world = World::new([32; 32], created, &desktop);
        let_colony_wander(&mut world, created);
        let start = world.save.creatures[0].state.position;
        desktop.windows.push(DesktopWindow {
            key: 92,
            bounds: DesktopRect {
                x: start.x - 100.0,
                y: start.y - 300.0,
                width: 300.0,
                height: 220.0,
            },
            z_order: 0,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
        let (target, surface) = find_nearby_ledge(
            &world.save.creatures[0],
            &desktop,
            &world.save.settings.habitat,
        )
        .expect("test window should expose a reachable ledge");
        let creature_id = world.save.creatures[0].id;
        world.save.creatures[0].state.action = ActionKind::Landing;
        world.ledge_journeys.insert(
            creature_id,
            LedgeJourney {
                start,
                target,
                surface,
                elapsed: 0.0,
                duration: 1.0,
            },
        );
        world.tick(created, 0.4, &desktop);
        let midway = world.save.creatures[0].state.position;
        assert_ne!(midway, start);
        assert_ne!(midway, target);
        world.tick(created, 0.7, &desktop);
        let creature = &world.save.creatures[0];
        assert_eq!(creature.state.position, target);
        assert_eq!(creature.state.surface.kind, SurfaceKind::WindowLedge);
        assert_eq!(creature.state.action, ActionKind::Perch);
    }

    #[test]
    fn most_creatures_discover_a_reachable_window_within_one_minute() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut discovered = 0;
        for seed_byte in 0_u8..20 {
            let mut desktop = desktop();
            let mut world = World::new([seed_byte; 32], created, &desktop);
            let_colony_wander(&mut world, created);
            let start = world.save.creatures[0].state.position;
            desktop.windows.push(DesktopWindow {
                key: 100 + u64::from(seed_byte),
                bounds: DesktopRect {
                    x: start.x - 180.0,
                    y: start.y - 320.0,
                    width: 420.0,
                    height: 250.0,
                },
                z_order: 0,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            });
            for _ in 0..1_200 {
                world.tick(created, 0.05, &desktop);
                if world.save.creatures[0].state.surface.kind == SurfaceKind::WindowLedge {
                    discovered += 1;
                    break;
                }
            }
        }
        assert!(
            discovered >= 16,
            "only {discovered}/20 creatures found the ledge"
        );
    }

    #[test]
    fn perched_creature_can_choose_a_different_window_height() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut desktop = desktop();
        desktop.windows.extend([
            DesktopWindow {
                key: 201,
                bounds: DesktopRect {
                    x: 360.0,
                    y: 610.0,
                    width: 460.0,
                    height: 210.0,
                },
                z_order: 1,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            },
            DesktopWindow {
                key: 202,
                bounds: DesktopRect {
                    x: 470.0,
                    y: 330.0,
                    width: 520.0,
                    height: 300.0,
                },
                z_order: 0,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            },
        ]);
        let mut world = World::new([41; 32], created, &desktop);
        let_colony_wander(&mut world, created);
        let creature = &mut world.save.creatures[0];
        creature.state.position = Point { x: 590.0, y: 610.0 };
        creature.state.surface = SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: 1,
            window_key: Some(201),
            relative_x: 0.5,
        };

        let (target, surface) = find_nearby_ledge(creature, &desktop, &world.save.settings.habitat)
            .expect("the upper window should be a reachable transfer");
        assert_eq!(surface.window_key, Some(202));
        assert_eq!(target.y, 330.0);
    }

    #[test]
    fn creatures_explore_multiple_window_levels() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let mut explorers = 0;
        for seed_byte in 0_u8..20 {
            let mut desktop = desktop();
            let mut world = World::new([seed_byte; 32], created, &desktop);
            let_colony_wander(&mut world, created);
            let start = world.save.creatures[0].state.position;
            desktop.windows.extend([
                DesktopWindow {
                    key: 300 + u64::from(seed_byte) * 3,
                    bounds: DesktopRect {
                        x: start.x - 180.0,
                        y: start.y - 190.0,
                        width: 410.0,
                        height: 150.0,
                    },
                    z_order: 2,
                    visible: true,
                    minimized: false,
                    application: None,
                    application_name: None,
                },
                DesktopWindow {
                    key: 301 + u64::from(seed_byte) * 3,
                    bounds: DesktopRect {
                        x: start.x - 90.0,
                        y: start.y - 390.0,
                        width: 440.0,
                        height: 180.0,
                    },
                    z_order: 1,
                    visible: true,
                    minimized: false,
                    application: None,
                    application_name: None,
                },
                DesktopWindow {
                    key: 302 + u64::from(seed_byte) * 3,
                    bounds: DesktopRect {
                        x: start.x - 210.0,
                        y: start.y - 590.0,
                        width: 520.0,
                        height: 190.0,
                    },
                    z_order: 0,
                    visible: true,
                    minimized: false,
                    application: None,
                    application_name: None,
                },
            ]);
            let mut visited = std::collections::BTreeSet::new();
            for _ in 0..2_400 {
                world.tick(created, 0.05, &desktop);
                if let Some(key) = world.save.creatures[0].state.surface.window_key {
                    visited.insert(key);
                }
                if visited.len() >= 2 {
                    explorers += 1;
                    break;
                }
            }
        }
        assert!(
            explorers >= 16,
            "only {explorers}/20 creatures explored more than one window level"
        );
    }

    #[test]
    fn home_appears_for_fifteen_minutes_then_observes_its_cooldown() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([52; 32], created, &desktop);
        world.tick(created, 0.05, &desktop);
        assert!(world.save.home.is_active());
        assert_eq!(world.save.creatures[0].state.action, ActionKind::Homebound);
        assert_eq!(world.save.home.display, Some(DisplayKey([1; 16])));

        world.tick(
            created + time::Duration::minutes(14) + time::Duration::seconds(59),
            0.05,
            &desktop,
        );
        assert!(world.save.home.is_active());
        world.tick(created + time::Duration::minutes(15), 0.05, &desktop);
        assert!(!world.save.home.is_active());
        assert_eq!(
            world.save.home.last_disappeared_utc,
            Some(created + time::Duration::minutes(15))
        );

        world.tick(
            created + time::Duration::minutes(29) + time::Duration::seconds(59),
            0.05,
            &desktop,
        );
        assert!(!world.save.home.is_active());
        world.tick(created + time::Duration::minutes(30), 0.05, &desktop);
        assert!(world.save.home.is_active());
    }

    #[test]
    fn homebound_colony_is_spaced_by_how_wide_a_creature_draws() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let monitor = &desktop.monitors[0];

        for display_scale in [1_u8, 3, 5] {
            let mut world = World::new([53; 32], created, &desktop);
            world.save.settings.display_scale = display_scale;
            // Give the colony four generations without waiting out the real arrival schedule.
            while world.save.creatures.len() < 4 {
                let generation = world.save.creatures.len() as u8;
                let mut grown = world.save.creatures[0].clone();
                grown.generation = generation;
                grown.id = u64::from(generation) + 100;
                world.save.creatures.push(grown);
            }
            world.tick(created, 0.05, &desktop);
            assert!(world.save.home.is_active());

            let creature_width =
                CREATURE_ART_WIDTH * f32::from(display_scale) / monitor.scale_factor.max(1.0);
            let mut xs: Vec<_> = world
                .save
                .creatures
                .iter()
                .map(|creature| creature.state.position.x)
                .collect();
            xs.sort_by(f32::total_cmp);
            for pair in xs.windows(2) {
                let gap = pair[1] - pair[0];
                assert!(
                    gap >= creature_width * 0.5,
                    "scale {display_scale}: creatures {gap} apart but draw {creature_width} wide",
                );
            }
        }
    }

    #[test]
    fn dragging_a_homebound_creature_dismisses_the_shelter_and_starts_cooldown() {
        let created = datetime!(2026-01-01 0:00 UTC);
        let desktop = desktop();
        let mut world = World::new([53; 32], created, &desktop);
        world.tick(created, 0.05, &desktop);
        let creature_id = world.save.creatures[0].id;
        let position = world.save.creatures[0].state.position;
        assert!(world.handle_command(
            WorldCommand::BeginDrag {
                creature_id,
                cursor: position,
            },
            &desktop,
        ));
        assert!(!world.save.home.is_active());
        assert_eq!(world.save.home.last_disappeared_utc, Some(created));
        assert!(
            world
                .drain_events()
                .any(|event| matches!(event, WorldEvent::HomeDisappeared { interrupted: true }))
        );

        assert!(world.handle_command(
            WorldCommand::EndDrag {
                cursor: Point { x: 720.0, y: 500.0 },
            },
            &desktop,
        ));
        world.tick(created + time::Duration::minutes(14), 0.05, &desktop);
        assert!(!world.save.home.is_active());
        world.tick(created + time::Duration::minutes(15), 0.05, &desktop);
        assert!(world.save.home.is_active());
    }

    #[test]
    fn passive_activity_actions_have_distinct_state_outcomes() {
        let desktop = desktop();
        let creature = World::new([61; 32], datetime!(2026-01-01 0:00 UTC), &desktop)
            .save
            .creatures
            .remove(0);
        let context = BehaviorContext {
            nearest_creature_distance: None,
            nearest_creature_position: None,
            nearest_creature_id: None,
            on_window_ledge: false,
            reachable_window_ledge: false,
            window_changed_nearby: false,
            hour_utc: 12,
        };

        let mut eating = creature.clone();
        eating.state.action = ActionKind::Eat;
        eating.state.drives.energy = 0.25;
        let energy_before = eating.state.drives.energy;
        update_drives(&mut eating, 1.0);
        execute_action(&mut eating, &desktop, context, 1.0, None);
        assert!(eating.state.drives.energy > energy_before);

        let mut drinking = creature.clone();
        drinking.state.action = ActionKind::Drink;
        drinking.state.drives.comfort = 0.2;
        drinking.state.drives.arousal = 0.8;
        execute_action(&mut drinking, &desktop, context, 1.0, None);
        assert!(drinking.state.drives.comfort > 0.2);
        assert!(drinking.state.drives.arousal < 0.8);

        let mut sprinting = creature;
        sprinting.state.action = ActionKind::Sprint;
        sprinting.state.facing_right = true;
        let start_x = sprinting.state.position.x;
        let walking_speed = 24.0 + sprinting.personality.activity * 34.0;
        execute_action(&mut sprinting, &desktop, context, 0.5, None);
        assert!(sprinting.state.position.x - start_x > walking_speed * 0.5 * 2.0);
    }
}
