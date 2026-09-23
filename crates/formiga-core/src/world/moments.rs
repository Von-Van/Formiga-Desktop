//! Moments the person at the desk invites the whole village to share while the houses are out: a
//! picnic, a dance, or a nap together on the ground between the houses.
//!
//! A scheduled ritual gathers the colony out on the desktop and never starts while the houses are
//! out. This is the village's own path: it runs inside the homebound tick, puts everyone who wants
//! to join in a line on the commons, and hands them back to their afternoon when it is over, so
//! roaming, quiet moments at the door, and a visiting guest all carry on around it.

use super::*;

/// Fewest companions a moment is shared between.
const MOMENT_MIN_COMPANIONS: usize = 2;
/// How long the village has to gather before a moment starts without whoever is slowest.
const MOMENT_GATHER_SECS: f32 = 16.0;
/// The last seconds of a dance, when every dancer finishes with its own celebration.
const DANCE_FINALE_SECS: f32 = 1.6;

/// How long a moment lasts once everyone has gathered.
fn moment_length(moment: VillageMoment) -> f32 {
    match moment {
        VillageMoment::Picnic => 14.0,
        VillageMoment::Dance => 12.0,
        VillageMoment::Nap => 30.0,
    }
}

/// One companion's part in a moment: where it does it, and what it does there.
#[derive(Clone, Copy, Debug)]
pub(super) struct MomentPlace {
    pub(super) creature_id: CreatureId,
    pub(super) spot: Point,
    pub(super) action: ActionKind,
}

/// A moment the village was asked to share and is sharing. Runtime only, like everything else a
/// village does: a relaunch finds everyone simply at home again.
#[derive(Clone, Debug)]
pub(super) struct VillageMomentPlan {
    pub(super) moment: VillageMoment,
    /// Whether everyone has gathered, or the wait ran out, and the moment itself is under way.
    pub(super) together: bool,
    /// Seconds left of the gathering, and then of the moment itself.
    remaining: f32,
    /// The middle of the line, which everyone in it faces.
    pub(super) centre: Point,
    pub(super) places: Vec<MomentPlace>,
}

impl VillageMomentPlan {
    pub(super) fn place(&self, creature_id: CreatureId) -> Option<MomentPlace> {
        self.places
            .iter()
            .copied()
            .find(|place| place.creature_id == creature_id)
    }

    /// The pose a companion that has reached its place in the moment shows over its action. Only
    /// a dance has one: each dancer on its own beat, finishing with its own celebration.
    pub(super) fn pose(&self) -> Option<AttentionPose> {
        (self.together && self.moment == VillageMoment::Dance).then_some(AttentionPose {
            target: Point {
                x: self.centre.x,
                y: self.centre.y - 28.0,
            },
            emotion: AttentionEmotion::Enjoying,
            hanging: 0.0,
            gesture: Some(if self.remaining <= DANCE_FINALE_SECS {
                Gesture::Cheer
            } else {
                Gesture::Bop
            }),
        })
    }
}

/// Whether a companion who was asked joins in, or the bubble it answers no with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MomentAnswer {
    Joins,
    Declines(BubbleIcon),
}

/// How one companion answers being asked. Its temperament and how it feels right now make it more
/// or less willing, and the companion that was asked by name is twice as willing as the rest.
fn moment_answer(
    creature: &Creature,
    moment: VillageMoment,
    asked: bool,
    rng: &mut ChaCha12Rng,
) -> MomentAnswer {
    // Already asleep: nothing is worth waking up for, a nap included.
    if creature.state.action == ActionKind::Sleep {
        return MomentAnswer::Declines(BubbleIcon::Sleepy);
    }
    let drives = &creature.state.drives;
    let p = &creature.personality;
    let (mut reluctance, excuse) = match moment {
        VillageMoment::Picnic => (
            0.25 * (1.0 - p.sociability) + if drives.energy > 0.9 { 0.25 } else { 0.0 },
            BubbleIcon::Decline,
        ),
        VillageMoment::Dance if drives.energy < 0.25 => {
            (0.5 + 0.3 * (1.0 - p.playfulness), BubbleIcon::Sleepy)
        }
        VillageMoment::Dance => (0.35 * (1.0 - p.playfulness), BubbleIcon::Decline),
        VillageMoment::Nap => (
            if drives.sleep_pressure < 0.2 && drives.energy > 0.7 {
                0.5
            } else {
                0.1
            },
            BubbleIcon::Decline,
        ),
    };
    if asked {
        reluctance *= 0.5;
    }
    if rng.random_bool(f64::from(reluctance.clamp(0.0, 0.9))) {
        MomentAnswer::Declines(excuse)
    } else {
        MomentAnswer::Joins
    }
}

impl World {
    /// The moments the village could be asked to share right now, in the order a menu offers
    /// them. Empty unless the houses are out and in sight, the colony is neither paused nor
    /// hidden, nobody is being carried, no ritual is under way, and at least two companions are
    /// home and free to join in. A moment already under way is not offered again, and reduced
    /// motion leaves out the dance, since a dance is nothing but motion.
    pub fn available_village_moments(&self) -> Vec<VillageMoment> {
        if !self.save.home.is_active()
            || !self.save.settings.visible
            || self.save.settings.paused
            || self.interaction.is_some()
            || self.colony_plan.is_some()
            || self.moment_candidates().len() < MOMENT_MIN_COMPANIONS
        {
            return Vec::new();
        }
        let under_way = self.village_moment();
        VillageMoment::ALL
            .into_iter()
            .filter(|moment| *moment != VillageMoment::Dance || !self.save.settings.reduce_motion)
            .filter(|moment| Some(*moment) != under_way)
            .collect()
    }

    /// The moment the village is sharing right now, if any.
    pub fn village_moment(&self) -> Option<VillageMoment> {
        self.village_moment.as_ref().map(|plan| plan.moment)
    }

    /// Whether this companion is taking part in the moment under way.
    pub(super) fn in_village_moment(&self, creature_id: CreatureId) -> bool {
        self.village_moment
            .as_ref()
            .is_some_and(|plan| plan.place(creature_id).is_some())
    }

    /// Everyone home and free to be asked: arrived, on their own feet on the floor, and not in
    /// the middle of coming down from somewhere.
    fn moment_candidates(&self) -> Vec<CreatureId> {
        self.save
            .creatures
            .iter()
            .filter(|creature| {
                creature.state.arrival_delay_secs <= 0.0
                    && creature.state.surface.kind == SurfaceKind::ScreenFloor
                    && !matches!(
                        creature.state.action,
                        ActionKind::Dragged | ActionKind::Tossed | ActionKind::Landing
                    )
                    && !self.window_journeys.contains_key(&creature.id)
                    && self
                        .interaction
                        .as_ref()
                        .is_none_or(|interaction| interaction.creature_id != creature.id)
            })
            .map(|creature| creature.id)
            .collect()
    }

    /// The person at the desk asks the village to share a moment. Everyone home answers for
    /// themselves with a bubble, and those who want to join walk to a place in one line on the
    /// ground between the houses. Asking for one while another is under way ends that one first.
    /// Returns whether the moment is going ahead.
    pub(super) fn invite_village_moment(
        &mut self,
        host: CreatureId,
        moment: VillageMoment,
        desktop: &DesktopSnapshot,
    ) -> bool {
        // Checked again now, whatever was true when the menu was drawn.
        if !self.available_village_moments().contains(&moment) {
            return false;
        }
        let cottages = colony_cottage_list(&self.save.creatures);
        let Some(commons) = home_commons(
            &self.save.home,
            cottages.as_slice(),
            &desktop.monitors,
            &self.save.settings.habitat,
            self.save.settings.display_scale,
        ) else {
            return false;
        };
        self.end_village_moment(false);

        let mut answers = Vec::new();
        for creature_id in self.moment_candidates() {
            let Some(creature) = self.save.creatures.iter().find(|c| c.id == creature_id) else {
                continue;
            };
            let answer = moment_answer(creature, moment, creature_id == host, &mut self.moment_rng);
            answers.push((creature_id, creature.state.position, answer));
        }
        let mut joining: Vec<(CreatureId, Point)> = answers
            .iter()
            .filter(|(.., answer)| *answer == MomentAnswer::Joins)
            .map(|(creature_id, position, _)| (*creature_id, *position))
            .collect();
        for (creature_id, _, answer) in &answers {
            if let MomentAnswer::Declines(icon) = answer {
                self.show_bubble(*creature_id, *icon);
            }
        }
        if joining.len() < MOMENT_MIN_COMPANIONS {
            // Willing, but on its own: it looks round for somebody to share it with.
            for (creature_id, _) in &joining {
                self.show_bubble(*creature_id, BubbleIcon::Question);
            }
            return false;
        }

        // One line across the ground, shoulder to shoulder like every other arrangement the
        // colony holds still in, centred on the picnic blanket or the nap cushion if one has been
        // put down, and otherwise on whoever was asked. A line longer than the ground keeps the
        // companions nearest that middle.
        let gathers_at = match moment {
            VillageMoment::Picnic => Some(HangoutKind::Blanket),
            VillageMoment::Nap => Some(HangoutKind::Cushion),
            VillageMoment::Dance => None,
        };
        let spot = gathers_at.and_then(|kind| {
            home_hangout_positions(
                &self.save.home,
                cottages.as_slice(),
                &desktop.monitors,
                &self.save.settings.habitat,
                self.save.settings.display_scale,
            )
            .into_iter()
            .find(|(placed, monitor_id, _)| *placed == kind && *monitor_id == commons.monitor_id)
            .map(|(_, _, at)| at.x)
        });
        let host_x = spot.unwrap_or_else(|| {
            joining
                .iter()
                .find(|(creature_id, _)| *creature_id == host)
                .map_or_else(
                    || joining.iter().map(|(_, at)| at.x).sum::<f32>() / joining.len() as f32,
                    |(_, at)| at.x,
                )
        });
        let (low, high) = commons.standing_span();
        let clear = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * commons.scale;
        let fits = (((high - low).max(0.0) / clear).floor() as usize + 1).max(1);
        if joining.len() > fits {
            joining.sort_by(|a, b| (a.1.x - host_x).abs().total_cmp(&(b.1.x - host_x).abs()));
            for (creature_id, _) in joining.drain(fits..) {
                self.show_bubble(creature_id, BubbleIcon::Ellipsis);
            }
        }
        if joining.len() < MOMENT_MIN_COMPANIONS {
            return false;
        }
        // In the order they stand, so nobody walks through anybody else to reach their place.
        joining.sort_by(|a, b| a.1.x.total_cmp(&b.1.x).then(a.0.cmp(&b.0)));
        let reduce_motion = self.save.settings.reduce_motion;
        let span = clear * (joining.len() - 1) as f32;
        let centre_x = if high - low >= span {
            host_x.clamp(low + span / 2.0, high - span / 2.0)
        } else {
            (low + high) / 2.0
        };
        let places: Vec<MomentPlace> = joining
            .iter()
            .enumerate()
            .map(|(index, (creature_id, at))| MomentPlace {
                creature_id: *creature_id,
                // Reduced motion keeps the colony where it stands: the moment happens in place.
                spot: if reduce_motion {
                    *at
                } else {
                    Point {
                        x: centre_x - span / 2.0 + index as f32 * clear,
                        y: commons.ground_y,
                    }
                },
                action: match moment {
                    VillageMoment::Picnic if index % 2 == 0 => ActionKind::Eat,
                    VillageMoment::Picnic => ActionKind::Drink,
                    VillageMoment::Dance => ActionKind::SocialPlay,
                    VillageMoment::Nap => ActionKind::Sleep,
                },
            })
            .collect();
        let centre = if reduce_motion {
            Point {
                x: places.iter().map(|place| place.spot.x).sum::<f32>() / places.len() as f32,
                y: commons.ground_y,
            }
        } else {
            Point {
                x: centre_x,
                y: commons.ground_y,
            }
        };

        // Whatever small thing each was doing at its door gives way, and each will pick a fresh
        // place to wander to once the moment is over.
        let accepted = match moment {
            VillageMoment::Picnic => BubbleIcon::Snack,
            VillageMoment::Dance => BubbleIcon::Music,
            VillageMoment::Nap => BubbleIcon::Sleepy,
        };
        for place in &places {
            self.end_village_activity(place.creature_id);
            self.home_moments.remove(&place.creature_id);
            self.home_moment_timers.remove(&place.creature_id);
            self.home_roam.remove(&place.creature_id);
            self.show_bubble(place.creature_id, accepted);
        }
        self.village_moment = Some(VillageMomentPlan {
            moment,
            together: false,
            remaining: MOMENT_GATHER_SECS,
            centre,
            places,
        });
        Self::emit(
            &mut self.events,
            WorldEvent::RitualStarted {
                kind: moment.ritual(),
            },
        );
        true
    }

    /// Moves a moment along: lets go of anyone who has been picked up or has gone, starts the
    /// moment once everyone has gathered or the wait is over, and ends it when its time is up.
    pub(super) fn advance_village_moment(&mut self, dt: f32) {
        let Some(plan) = &mut self.village_moment else {
            return;
        };
        // Picked up, not merely petted: a pet is answered where the companion stands, and it
        // carries on with the moment afterwards.
        let carried = self
            .interaction
            .as_ref()
            .filter(|interaction| interaction.dragging)
            .map(|interaction| interaction.creature_id);
        let creatures = &self.save.creatures;
        plan.places.retain(|place| {
            Some(place.creature_id) != carried
                && creatures.iter().any(|creature| {
                    creature.id == place.creature_id
                        && !matches!(
                            creature.state.action,
                            ActionKind::Dragged | ActionKind::Tossed
                        )
                })
        });
        let ended = if plan.places.len() < MOMENT_MIN_COMPANIONS {
            Some(false)
        } else {
            plan.remaining -= dt.max(0.0);
            if !plan.together {
                let gathered = plan.places.iter().all(|place| {
                    creatures.iter().any(|creature| {
                        creature.id == place.creature_id
                            && creature.state.position.distance(place.spot) <= 0.5
                    })
                });
                if gathered || plan.remaining <= 0.0 {
                    plan.together = true;
                    plan.remaining = moment_length(plan.moment);
                }
                None
            } else {
                (plan.remaining <= 0.0).then_some(true)
            }
        };
        if let Some(completed) = ended {
            self.end_village_moment(completed);
        }
    }

    /// Ends the moment under way, if there is one. Everyone in it goes back to their own
    /// afternoon from wherever they are. A moment that ran its course is written down and brings
    /// everyone who shared it a little closer; one cut short is not.
    pub(super) fn end_village_moment(&mut self, completed: bool) {
        let Some(plan) = self.village_moment.take() else {
            return;
        };
        let kind = plan.moment.ritual();
        for place in &plan.places {
            self.home_roam.remove(&place.creature_id);
            let Some(creature) = creature_mut(&mut self.save.creatures, place.creature_id) else {
                continue;
            };
            creature.state.attention = None;
            creature.state.flourish = None;
            if creature.state.action != place.action {
                continue;
            }
            let elapsed = creature.state.action_elapsed;
            Self::emit(
                &mut self.events,
                WorldEvent::ActionCompleted {
                    creature_id: creature.id,
                    action: place.action,
                },
            );
            if place.action == ActionKind::Sleep {
                let uninterrupted_seconds = self
                    .sleep_elapsed
                    .remove(&creature.id)
                    .unwrap_or(elapsed)
                    .max(0.0) as u32;
                Self::emit(
                    &mut self.events,
                    WorldEvent::CreatureRested {
                        creature_id: creature.id,
                        uninterrupted_seconds,
                    },
                );
                Self::emit(
                    &mut self.events,
                    WorldEvent::CreatureWoke {
                        creature_id: creature.id,
                    },
                );
            }
            creature.state.action = ActionKind::Homebound;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = home::HOME_DURATION.whole_seconds() as f32;
            creature.state.velocity = Point::default();
            Self::emit(
                &mut self.events,
                WorldEvent::ActionStarted {
                    creature_id: creature.id,
                    action: ActionKind::Homebound,
                },
            );
        }
        if !completed {
            Self::emit(&mut self.events, WorldEvent::RitualInterrupted { kind });
            return;
        }
        let experience = match plan.moment {
            VillageMoment::Picnic => RelationshipExperience::Greeting,
            VillageMoment::Dance => RelationshipExperience::PositivePlay,
            VillageMoment::Nap => RelationshipExperience::SharedRest,
        };
        for (index, first) in plan.places.iter().enumerate() {
            for second in plan.places.iter().skip(index + 1) {
                Self::emit(
                    &mut self.events,
                    WorldEvent::BondInteraction {
                        a: first.creature_id,
                        b: second.creature_id,
                        experience,
                    },
                );
            }
        }
        Self::emit(&mut self.events, WorldEvent::RitualCompleted { kind });
    }
}
