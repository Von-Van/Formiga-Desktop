use super::surfaces::supports_on;
use super::*;

const TOSS_SPEED_THRESHOLD: f32 = 220.0;
const TOSS_VELOCITY_SCALE: f32 = 0.65;
pub(super) const TOSS_MAX_SPEED: f32 = 900.0;
const TOSS_GRAVITY: f32 = 1_200.0;
const TOSS_HORIZONTAL_DRAG: f32 = 1.6;
const TOSS_BOUNCE_RESTITUTION: f32 = 0.28;
const TOSS_BOUNCE_HORIZONTAL_RETENTION: f32 = 0.65;
const TOSS_MIN_BOUNCE_SPEED: f32 = 140.0;
pub(super) const TOSS_MAX_DURATION: f32 = 3.0;
pub(super) const DRAG_THRESHOLD: f32 = 6.0;

#[derive(Clone)]
pub(super) struct InteractionSession {
    pub(super) creature_id: CreatureId,
    /// A guest can be petted, but never picked up: it is not ours to carry.
    pub(super) guest: bool,
    pub(super) press_cursor: Point,
    pub(super) max_excursion: f32,
    pub(super) dragging: bool,
    pub(super) grab_offset: Point,
    pub(super) original_position: Point,
    pub(super) original_surface: SurfaceAttachment,
    pub(super) original_action: ActionKind,
    /// The last three cursor speeds, oldest overwritten first, and where the next one goes.
    /// The cursor is wrapped as it is advanced, so it always names a slot that exists.
    pub(super) velocity_samples: [Point; 3],
    pub(super) velocity_sample_count: u8,
    pub(super) next_velocity_sample: u8,
}

impl InteractionSession {
    pub(super) fn record_velocity(&mut self, velocity: Point) {
        let index = usize::from(self.next_velocity_sample);
        self.velocity_samples[index] = velocity;
        self.next_velocity_sample = (self.next_velocity_sample + 1) % 3;
        self.velocity_sample_count = self.velocity_sample_count.saturating_add(1).min(3);
    }

    pub(super) fn release_velocity(&self) -> Point {
        let count = usize::from(self.velocity_sample_count);
        if count == 0 {
            return Point::default();
        }
        let sum = self
            .velocity_samples
            .iter()
            .take(count)
            .fold(Point::default(), |sum, sample| Point {
                x: sum.x + sample.x,
                y: sum.y + sample.y,
            });
        Point {
            x: sum.x / count as f32,
            y: sum.y / count as f32,
        }
    }
}

#[derive(Clone)]
pub(super) struct TossState {
    pub(super) elapsed: f32,
    pub(super) bounces: u8,
    pub(super) last_safe_position: Point,
    pub(super) last_safe_surface: SurfaceAttachment,
}

impl World {
    pub(super) fn settle_active_tosses(&mut self, desktop: &DesktopSnapshot) {
        let tossed: Vec<_> = self.tosses.keys().copied().collect();
        for creature_id in tossed {
            let Some(toss) = self.tosses.remove(&creature_id) else {
                continue;
            };
            let Some(creature) = self
                .save
                .creatures
                .iter_mut()
                .find(|creature| creature.id == creature_id)
            else {
                continue;
            };
            if let Some((surface, bounced)) = settle_toss(
                creature,
                &toss,
                desktop,
                &self.save.settings.habitat,
                self.save.settings.window_ledges,
            ) {
                Self::emit(
                    &mut self.events,
                    WorldEvent::TossLanded {
                        creature_id,
                        surface: surface.kind,
                        bounced,
                    },
                );
                Self::emit(
                    &mut self.events,
                    WorldEvent::SurfaceChanged {
                        creature_id,
                        kind: surface.kind,
                    },
                );
            }
        }
    }

    pub fn handle_command(&mut self, command: WorldCommand, desktop: &DesktopSnapshot) -> bool {
        let handled = match command {
            WorldCommand::BeginInteraction {
                creature_id,
                cursor,
            } => self.begin_interaction(creature_id, cursor),
            WorldCommand::UpdateInteraction { cursor, velocity } => {
                self.update_interaction(cursor, velocity, desktop)
            }
            WorldCommand::EndInteraction { cursor, velocity } => {
                self.end_interaction(cursor, velocity, desktop)
            }
            WorldCommand::CancelInteraction => self.cancel_interaction(),
            WorldCommand::GatherCreatures => {
                self.interrupt_colony_plan(self.save.maximum_seen_utc);
                self.gather_creatures(desktop);
                true
            }
            WorldCommand::OfferSnack { creature_id } => self.offer_snack(creature_id, desktop),
            WorldCommand::OfferToy { creature_id } => self.offer_toy(creature_id, desktop),
            WorldCommand::SendHome => self.send_home(desktop),
            WorldCommand::InviteVillageMoment {
                creature_id,
                moment,
            } => self.invite_village_moment(creature_id, moment, desktop),
            WorldCommand::StopVillageMoment => {
                let under_way = self.village_moment.is_some();
                self.end_village_moment(false);
                under_way
            }
        };
        self.project_events(self.save.maximum_seen_utc);
        handled
    }

    fn begin_interaction(&mut self, creature_id: CreatureId, cursor: Point) -> bool {
        if self.interaction.is_some() || !self.save.settings.direct_manipulation {
            return false;
        }
        // Whoever is visiting can be greeted with a pet like anyone else, and nothing more: a
        // drag on a guest stays a pet, and never sends the houses away.
        if let Some(guest) = self
            .save
            .visitors
            .on_stage()
            .filter(|g| g.id == creature_id)
        {
            let position = guest.state.position;
            self.interaction = Some(InteractionSession {
                creature_id,
                guest: true,
                press_cursor: cursor,
                max_excursion: 0.0,
                dragging: false,
                grab_offset: Point {
                    x: cursor.x - position.x,
                    y: cursor.y - position.y,
                },
                original_position: position,
                original_surface: guest.state.surface.clone(),
                original_action: guest.state.action,
                velocity_samples: [Point::default(); 3],
                velocity_sample_count: 0,
                next_velocity_sample: 0,
            });
            return true;
        }
        let Some(creature_index) = self.save.creatures.iter().position(|creature| {
            creature.id == creature_id && creature.state.arrival_delay_secs <= 0.0
        }) else {
            return false;
        };
        let interrupted_journey = self.window_journeys.remove(&creature_id).is_some();
        self.cancel_creature_attention(creature_id);
        self.window_routes.remove(&creature_id);
        let interrupted_toss = self.tosses.remove(&creature_id);
        self.action_choices.remove(&creature_id);
        self.bond_plans.remove(&creature_id);
        let creature = &mut self.save.creatures[creature_index];
        let original_position = interrupted_toss
            .as_ref()
            .map_or(creature.state.position, |toss| toss.last_safe_position);
        let original_surface = interrupted_toss.as_ref().map_or_else(
            || creature.state.surface.clone(),
            |toss| toss.last_safe_surface.clone(),
        );
        let original_action = if interrupted_journey || interrupted_toss.is_some() {
            ActionKind::Idle
        } else {
            creature.state.action
        };
        if interrupted_journey || interrupted_toss.is_some() {
            creature.state.position = original_position;
            creature.state.surface = original_surface.clone();
            creature.state.action = ActionKind::Idle;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = 2.5;
            creature.state.velocity = Point::default();
        }
        self.interaction = Some(InteractionSession {
            creature_id,
            guest: false,
            press_cursor: cursor,
            max_excursion: 0.0,
            dragging: false,
            grab_offset: Point {
                x: cursor.x - creature.state.position.x,
                y: cursor.y - creature.state.position.y,
            },
            original_position,
            original_surface,
            original_action,
            velocity_samples: [Point::default(); 3],
            velocity_sample_count: 0,
            next_velocity_sample: 0,
        });
        true
    }

    fn start_drag_motion(&mut self) {
        let Some(interaction) = self.interaction.as_mut() else {
            return;
        };
        if interaction.dragging || interaction.guest {
            return;
        }
        interaction.dragging = true;
        let creature_id = interaction.creature_id;
        // A doze at home is a passive moment, not a night's sleep: interrupting one
        // is not something the creature should learn from.
        let interrupted_sleep =
            interaction.original_action == ActionKind::Sleep && !self.save.home.is_active();
        if self.colony_plan.is_some() {
            self.interrupt_colony_plan(self.save.maximum_seen_utc);
        }
        if self.save.home.is_active() {
            self.dismiss_home(self.save.maximum_seen_utc, true);
        }
        let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == creature_id)
        else {
            return;
        };
        if interrupted_sleep {
            let elapsed_seconds = self
                .sleep_elapsed
                .remove(&creature_id)
                .unwrap_or(creature.state.action_elapsed)
                .max(0.0) as u32;
            Self::emit(
                &mut self.events,
                WorldEvent::SleepInterrupted {
                    creature_id,
                    elapsed_seconds,
                },
            );
        }
        creature.state.action = ActionKind::Dragged;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = f32::MAX;
        creature.state.velocity = Point::default();
        creature.state.surface.window_key = None;
        bubbles::show(&mut self.bubbles, creature_id, BubbleIcon::Surprise);
        Self::emit(&mut self.events, WorldEvent::DragStarted { creature_id });
    }

    fn update_interaction(
        &mut self,
        cursor: Point,
        velocity: Point,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let Some(interaction) = &mut self.interaction else {
            return false;
        };
        interaction.max_excursion = interaction
            .max_excursion
            .max(interaction.press_cursor.distance(cursor));
        if interaction.max_excursion > DRAG_THRESHOLD && !interaction.dragging {
            self.start_drag_motion();
        }
        let Some(interaction) = &mut self.interaction else {
            return false;
        };
        if !interaction.dragging {
            return true;
        }
        interaction.record_velocity(velocity);
        let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == interaction.creature_id)
        else {
            self.interaction = None;
            return false;
        };
        creature.state.position = Point {
            x: cursor.x - interaction.grab_offset.x,
            y: cursor.y - interaction.grab_offset.y,
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

    fn end_interaction(
        &mut self,
        cursor: Point,
        velocity: Point,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let Some(interaction) = &mut self.interaction else {
            return false;
        };
        interaction.max_excursion = interaction
            .max_excursion
            .max(interaction.press_cursor.distance(cursor));
        if interaction.max_excursion > DRAG_THRESHOLD && !interaction.dragging {
            self.start_drag_motion();
        }
        let Some(mut interaction) = self.interaction.take() else {
            return false;
        };
        if !interaction.dragging {
            // As above: a doze at the village door teaches the creature nothing when it ends.
            let interrupted_seconds = (interaction.original_action == ActionKind::Sleep
                && !self.save.home.is_active())
            .then(|| {
                self.sleep_elapsed
                    .remove(&interaction.creature_id)
                    .unwrap_or_default()
                    .max(0.0) as u32
            });
            let creature_id = interaction.creature_id;
            let Some(creature) = self.member_or_guest_mut(creature_id) else {
                return false;
            };
            let elapsed = creature.state.action_elapsed as u32;
            creature.state.action = ActionKind::PetReaction;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = 1.4;
            creature.state.velocity = Point::default();
            creature.state.drives.comfort = (creature.state.drives.comfort + 0.12).min(1.0);
            creature.state.drives.arousal = (creature.state.drives.arousal - 0.08).max(0.0);
            if let Some(elapsed_seconds) = interrupted_seconds {
                Self::emit(
                    &mut self.events,
                    WorldEvent::SleepInterrupted {
                        creature_id,
                        elapsed_seconds: elapsed_seconds.max(elapsed),
                    },
                );
            }
            bubbles::show(&mut self.bubbles, creature_id, BubbleIcon::Heart);
            Self::emit(&mut self.events, WorldEvent::CreaturePetted { creature_id });
            Self::emit(
                &mut self.events,
                WorldEvent::ActionStarted {
                    creature_id,
                    action: ActionKind::PetReaction,
                },
            );
            return true;
        }
        interaction.record_velocity(velocity);
        let release_velocity = interaction.release_velocity();
        let policy = self.save.settings.habitat.clone();
        let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == interaction.creature_id)
        else {
            return false;
        };
        creature.state.position = Point {
            x: cursor.x - interaction.grab_offset.x,
            y: cursor.y - interaction.grab_offset.y,
        };
        let release_speed = release_velocity.distance(Point::default());
        if release_speed >= TOSS_SPEED_THRESHOLD
            && !self.save.settings.paused
            && !self.save.settings.reduce_motion
        {
            let scaled_speed = (release_speed * TOSS_VELOCITY_SCALE).min(TOSS_MAX_SPEED);
            let scale = scaled_speed / release_speed.max(0.001);
            let initial_velocity = Point {
                x: release_velocity.x * scale,
                y: release_velocity.y * scale,
            };
            creature.state.velocity = initial_velocity;
            creature.state.action = ActionKind::Tossed;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = f32::MAX;
            creature.state.surface.window_key = None;
            if let Some(monitor) = desktop
                .monitors
                .iter()
                .find(|monitor| monitor.bounds.contains(creature.state.position))
            {
                creature.state.surface.monitor_id = monitor.id;
            }
            creature.state.surface.kind = SurfaceKind::ScreenFloor;
            self.tosses.insert(
                creature.id,
                TossState {
                    elapsed: 0.0,
                    bounces: 0,
                    last_safe_position: interaction.original_position,
                    last_safe_surface: interaction.original_surface,
                },
            );
            creature.state.drives.arousal = (creature.state.drives.arousal + 0.16).min(1.0);
            Self::emit(
                &mut self.events,
                WorldEvent::DragEnded {
                    creature_id: creature.id,
                    outcome: DragReleaseKind::Tossed {
                        velocity: initial_velocity,
                    },
                },
            );
            return true;
        }
        let support = find_drop_support(cursor, desktop, &policy, self.save.settings.window_ledges)
            .or_else(|| {
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
            creature.state.position = interaction.original_position;
            creature.state.surface = interaction.original_surface;
            creature.state.action = interaction.original_action;
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
        Self::emit(
            &mut self.events,
            WorldEvent::DragEnded {
                creature_id: creature.id,
                outcome: DragReleaseKind::Placed(surface.kind),
            },
        );
        Self::emit(
            &mut self.events,
            WorldEvent::SurfaceChanged {
                creature_id: creature.id,
                kind: surface.kind,
            },
        );
        if let Some((display, region)) = display_region(desktop, surface.monitor_id, position) {
            Self::emit(
                &mut self.events,
                WorldEvent::CreaturePlaced {
                    creature_id: creature.id,
                    display,
                    region,
                },
            );
        }
        true
    }

    fn cancel_interaction(&mut self) -> bool {
        let Some(interaction) = self.interaction.take() else {
            return false;
        };
        if let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == interaction.creature_id)
        {
            creature.state.position = interaction.original_position;
            creature.state.surface = interaction.original_surface;
            creature.state.action = interaction.original_action;
            creature.state.action_elapsed = 0.0;
            creature.state.velocity = Point::default();
        }
        true
    }

    fn gather_creatures(&mut self, desktop: &DesktopSnapshot) {
        self.clear_runtime_plans();
        // Everyone is about to be stood side by side on the floor, so there is no homecoming
        // left to greet anybody about.
        self.pending_home_greetings.clear();
        let policy = self.save.settings.habitat.clone();
        let display_scale = self.save.settings.display_scale;
        // Gather is called on creatures anywhere, including two perched on ledges above one
        // another, who would otherwise come down onto the same point and stay there.
        let mut taken: Vec<Point> = Vec::new();
        for creature in &mut self.save.creatures {
            if let Some((monitor_id, mut position)) =
                nearest_habitat_point(&policy, &desktop.monitors, creature.state.position)
            {
                let clear = spacing::creature_frame_width(creature, display_scale, desktop)
                    * spacing::FULL_CLEAR_RATIO;
                for _ in 0..taken.len() {
                    let Some(occupied) = taken.iter().copied().find(|other| {
                        (other.x - position.x).abs() < clear && (other.y - position.y).abs() < clear
                    }) else {
                        break;
                    };
                    position.x = occupied.x + clear;
                }
                if let Some((_, settled)) =
                    nearest_habitat_point(&policy, &desktop.monitors, position)
                {
                    position = settled;
                }
                taken.push(position);
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
}

pub(super) fn advance_toss(
    creature: &mut Creature,
    toss: &mut TossState,
    dt: f32,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    reduce_motion: bool,
    window_ledges: bool,
) -> Option<(SurfaceAttachment, bool)> {
    toss.elapsed += dt;
    if reduce_motion || toss.elapsed >= TOSS_MAX_DURATION {
        return settle_toss(creature, toss, desktop, policy, window_ledges);
    }

    let previous = creature.state.position;
    creature.state.velocity.y += TOSS_GRAVITY * dt;
    creature.state.velocity.x *= (-TOSS_HORIZONTAL_DRAG * dt).exp();
    let next = Point {
        x: previous.x + creature.state.velocity.x * dt,
        y: previous.y + creature.state.velocity.y * dt,
    };

    if creature.state.velocity.y > 0.0
        && let Some((impact, surface)) =
            find_swept_support(previous, next, desktop, policy, window_ledges)
    {
        creature.state.position = impact;
        if toss.bounces == 0 && creature.state.velocity.y >= TOSS_MIN_BOUNCE_SPEED {
            toss.bounces = 1;
            creature.state.velocity.x *= TOSS_BOUNCE_HORIZONTAL_RETENTION;
            creature.state.velocity.y *= -TOSS_BOUNCE_RESTITUTION;
            return None;
        }
        creature.state.surface = surface.clone();
        creature.state.velocity = Point::default();
        creature.state.action = ActionKind::Landing;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 0.55;
        return Some((surface, toss.bounces > 0));
    }

    creature.state.position = next;
    if let Some(monitor) = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.bounds.contains(next))
    {
        creature.state.surface.monitor_id = monitor.id;
        creature.state.facing_right = creature.state.velocity.x >= 0.0;
        None
    } else {
        settle_toss(creature, toss, desktop, policy, window_ledges)
    }
}

pub(super) fn settle_toss(
    creature: &mut Creature,
    toss: &TossState,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    window_ledges: bool,
) -> Option<(SurfaceAttachment, bool)> {
    let support = find_drop_support(creature.state.position, desktop, policy, window_ledges)
        .or_else(|| {
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
    let (position, surface) =
        support.unwrap_or_else(|| (toss.last_safe_position, toss.last_safe_surface.clone()));
    creature.state.position = position;
    creature.state.surface = surface.clone();
    creature.state.velocity = Point::default();
    creature.state.action = ActionKind::Landing;
    creature.state.action_elapsed = 0.0;
    creature.state.action_duration = 0.55;
    Some((surface, toss.bounces > 0))
}

/// What catches a creature thrown from `previous` to `next` this frame.
///
/// A toss is swept along an arc, so a surface only counts where the path actually crosses it —
/// no sliding sideways onto a corner — and the first one crossed wins however far away it is.
/// The throw can carry across displays, so every monitor is searched, and a landing belongs to
/// whichever monitor's habitat holds the point it lands on.
pub(super) fn find_swept_support(
    previous: Point,
    next: Point,
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    window_ledges: bool,
) -> Option<(Point, SurfaceAttachment)> {
    let dy = next.y - previous.y;
    if dy <= 0.0 {
        return None;
    }
    let windows = if window_ledges {
        desktop.windows.as_slice()
    } else {
        &[]
    };
    let mut candidates = Vec::new();
    for monitor in &desktop.monitors {
        let regions = accessible_regions(policy, monitor);
        for span in supports_on(windows, &regions, monitor.id) {
            let crossed = (span.y - previous.y) / dy;
            if !(0.0..=1.0).contains(&crossed) {
                continue;
            }
            let x = lerp(previous.x, next.x, crossed);
            if !span.holds(x) {
                continue;
            }
            let (point, surface) = span.place(x);
            if regions.iter().any(|region| region.contains(point)) {
                candidates.push((crossed, point, surface));
            }
        }
    }
    candidates
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, point, surface)| (point, surface))
}
