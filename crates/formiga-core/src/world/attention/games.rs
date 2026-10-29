//! Games that move: a chase, a procession, a dance circle, and a pile-up. Each one reuses the
//! shared play plan, the bounded approach steps, and the same one-origin audience contract.
//! Returning `false` means the scene has run out of room, energy, or partners and should wind down.
use super::play::{PlayRole, Session, composure};

/// Whoever holds on more steadily keeps the toy.
fn composure_of(world: &World, id: CreatureId) -> f32 {
    world
        .save
        .creatures
        .iter()
        .find(|c| c.id == id)
        .map_or(0.0, composure)
}
use super::*;

/// Creature-scale contact distance for a tag or a settled arrangement.
const CONTACT: f32 = 26.0;
/// How far a runner tries to open up, and how far behind a procession follows.
const CHASE_LEAD: f32 = 90.0;
const PARADE_GAP: f32 = 52.0;
/// A follower this far behind has lost the procession and is released.
const PARADE_LOST: f32 = 220.0;
/// After being tagged, a creature cannot tag straight back for this long.
const TAG_IMMUNITY: f32 = 1.2;

/// How close to its goal a player counts as having arrived, for the purpose of being allowed to
/// change its mind about which way to go. Wider than the two points a stride covers, so the last
/// fraction of a walk cannot read as still being under way.
const COMMITTED_WALK: f32 = 4.0;

impl World {
    pub(super) fn play_unit(&self, id: CreatureId, desktop: &DesktopSnapshot) -> f32 {
        self.save
            .creatures
            .iter()
            .find(|c| c.id == id)
            .map_or(1.0, |c| creature_unit(c, &self.save.settings, desktop))
    }

    pub(super) fn play_position(&self, id: CreatureId) -> Option<Point> {
        self.save
            .creatures
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.state.position)
    }

    /// Point a member at a destination and give it the gesture it plays with while it travels.
    /// Steering is a fresh instruction, so it clears any pose struck over the previous one.
    pub(super) fn steer_play(
        &mut self,
        id: CreatureId,
        destination: Option<Point>,
        gesture: ActionKind,
    ) {
        let here = self.play_position(id).map(|point| point.x);
        let Some(plan) = self.attention.plans.get_mut(&id) else {
            return;
        };
        if let Role::Play {
            gesture: current,
            pose,
            ..
        } = &mut plan.role
        {
            *current = gesture;
            *pose = None;
        }
        // A player commits to the way it is going until it gets there. Every scene works out its
        // goals afresh each tick, and a runner with no room ahead is sent back past its chaser,
        // then away again the moment that opens up: alternating twice a second is a decision,
        // twenty times a second is a creature having a fit. Only a reversal is refused — a
        // chaser steering at a lead that keeps running the same way still tracks it every tick,
        // and a goal that turns out to be unreachable clears the walk and frees the next one.
        if let (Some(here), Some(new), Some(current)) = (here, destination, plan.walk) {
            let under_way = (current.destination.x - here).abs() > COMMITTED_WALK;
            let reversing = (new.x - here).signum() != (current.destination.x - here).signum();
            if under_way && reversing {
                return;
            }
        }
        plan.walk = destination.map(ShortWalk::playing);
    }

    /// Put the scene's own toy in someone's hands. A game borrows `PresentDiscovery` to show a
    /// plaything off, so the toy is chosen here, from the scene's own seed, and is always an
    /// everyday trinket: a game is not a find, and what is held up must never be a keepsake that
    /// only turns up after dark, high up, mid-ride, or beside a close friend — nor be whatever
    /// this creature last actually found. Called for whoever holds the prop, every tick a toy
    /// scene runs, so the toy is right even on the paths that only wind the scene down.
    fn hold_plaything(&mut self, id: CreatureId, seed: u64) {
        let variant = discovery::plaything_variant(seed);
        if let Some(creature) = self.save.creatures.iter_mut().find(|c| c.id == id) {
            creature.state.activity_variant = variant;
        }
    }

    /// Strike a body pose over the gesture a member was just steered into. The pose lasts until
    /// the member is steered again or the next tick, and shows only while the member stands
    /// still in a pose the body can give over; travel and a toy being shown off keep the body.
    pub(super) fn pose_play(&mut self, id: CreatureId, gesture: Gesture) {
        if let Some(plan) = self.attention.plans.get_mut(&id)
            && let Role::Play { pose, .. } = &mut plan.role
        {
            *pose = Some(gesture);
        }
    }

    /// How near a settling companion has to get before a game counts it as in place.
    pub(super) fn held_slack(&self, id: CreatureId, desktop: &DesktopSnapshot) -> f32 {
        CONTACT * 0.5 * self.play_unit(id, desktop)
    }

    /// Where a game stands companions it is holding in one formation: shoulder to shoulder, with
    /// room for the slack above at either end, so that for as long as the formation is held
    /// nobody's face is behind anybody's body. Contact a game only passes through — a tag, a
    /// vault, a hand-off — is not spaced by this, because it is over in a moment.
    pub(super) fn held_gap(&self, id: CreatureId, desktop: &DesktopSnapshot) -> f32 {
        let clear = self
            .save
            .creatures
            .iter()
            .find(|c| c.id == id)
            .map_or(CONTACT * 2.0, |c| {
                super::super::spacing::face_clear_gap(c, self.save.settings.display_scale, desktop)
            });
        clear + self.held_slack(id, desktop) * 2.0
    }

    /// A goal on the member's own surface, closer than an audience would stand.
    pub(super) fn play_goal_for(
        &self,
        id: CreatureId,
        x: f32,
        desktop: &DesktopSnapshot,
    ) -> Option<Point> {
        let creature = self.save.creatures.iter().find(|c| c.id == id)?;
        motion::play_goal(self, creature, x, desktop)
    }

    /// Room to travel in `direction`, settling for a shorter stride when companions or the edge
    /// of the surface are in the way. `None` means that direction is closed.
    pub(super) fn travel_goal(
        &self,
        id: CreatureId,
        from: Point,
        direction: f32,
        unit: f32,
        desktop: &DesktopSnapshot,
    ) -> Option<Point> {
        [1.0, 0.65, 0.4].into_iter().find_map(|fraction| {
            self.play_goal_for(
                id,
                from.x + direction * CHASE_LEAD * fraction * unit,
                desktop,
            )
        })
    }

    /// A step toward `x`, shortened until it is one this creature can take in a single stride.
    /// `None` means it cannot set off that way at all.
    pub(super) fn step_toward(
        &self,
        id: CreatureId,
        x: f32,
        desktop: &DesktopSnapshot,
    ) -> Option<Point> {
        let from = self.play_position(id)?.x;
        // One stride is as far as this creature walks in a single approach; a distant corner is
        // reached by taking several.
        let reach = self
            .save
            .creatures
            .iter()
            .find(|c| c.id == id)
            .map_or(0.0, |c| {
                (motion::speed(c) * MAX_APPROACH_SECONDS).min(120.0) * 0.98
            });
        let stride = from + (x - from).clamp(-reach, reach);
        [1.0, 0.7, 0.45, 0.25]
            .into_iter()
            .find_map(|fraction| self.play_goal_for(id, from + (stride - from) * fraction, desktop))
    }

    pub(super) fn look_at(&mut self, id: CreatureId, target: Point) {
        if let Some(plan) = self.attention.plans.get_mut(&id) {
            plan.target = target;
        }
    }

    /// One companion may join a scene already in progress, taking `role`.
    fn recruit_player(
        &mut self,
        s: &mut Session,
        anchor: CreatureId,
        role: PlayRole,
        playfulness: f32,
        desktop: &DesktopSnapshot,
    ) {
        if s.count >= MAX_PARTICIPANTS {
            return;
        }
        let Some(actor) = self.save.creatures.iter().find(|c| c.id == anchor) else {
            return;
        };
        let candidate = self.attention.plans.iter().find_map(|(&id, p)| {
            let c = self.save.creatures.iter().find(|c| c.id == id)?;
            (matches!(p.role, Role::Observer { .. })
                && p.origin == s.origin
                && c.personality.playfulness > playfulness
                && self.play_available(c, desktop)
                && self.play_pair_spaced(actor, c, desktop, 24.0)
                && !s.members.contains(&Some(id)))
            .then_some(id)
        });
        if let Some(id) = candidate {
            self.join_play(id, anchor, s.origin, desktop);
            self.signal_playing(id);
            s.members[s.count] = Some(id);
            s.roles[s.count] = role;
            s.count += 1;
        }
    }

    pub(super) fn advance_chase(&mut self, s: &mut Session, desktop: &DesktopSnapshot) -> bool {
        let Some((lead_index, lead_id)) = s.with_role(PlayRole::Lead).next() else {
            return false;
        };
        let Some(lead) = self.play_position(lead_id) else {
            return false;
        };
        let unit = self.play_unit(lead_id, desktop);
        let chasers: Vec<_> = s.with_role(PlayRole::Follow).collect();
        if chasers.is_empty() {
            return false;
        }
        // Reaching the runner hands the role over, a bounded number of times.
        let caught = chasers.iter().find(|(_, id)| {
            self.play_position(*id)
                .is_some_and(|p| (p.x - lead.x).abs() <= CONTACT * unit)
        });
        if let Some(&(catcher_index, catcher_id)) = caught {
            if s.swaps >= 2 {
                return false;
            }
            s.swaps += 1;
            s.roles[lead_index] = PlayRole::Follow;
            s.roles[catcher_index] = PlayRole::Lead;
            s.turn_started = s.elapsed;
            // A moment of delight at the hand-over, then the roles reverse. The one caught
            // gasps as the hand lands.
            self.steer_play(lead_id, None, ActionKind::Greet);
            self.steer_play(catcher_id, None, ActionKind::Greet);
            self.pose_play(lead_id, Gesture::Gasp);
            self.look_at(lead_id, self.play_position(catcher_id).unwrap_or(lead));
            self.look_at(catcher_id, lead);
            return true;
        }
        let nearest = chasers
            .iter()
            .filter_map(|(_, id)| self.play_position(*id))
            .min_by(|a, b| (a.x - lead.x).abs().total_cmp(&(b.x - lead.x).abs()));
        let away = nearest.map_or(1.0, |p| if lead.x >= p.x { 1.0 } else { -1.0 });
        // The runner opens up a gap; when neither direction has room, the chase is over.
        let Some(goal) = [away, -away]
            .into_iter()
            .find_map(|direction| self.travel_goal(lead_id, lead, direction, unit, desktop))
        else {
            return false;
        };
        self.steer_play(lead_id, Some(goal), ActionKind::Sprint);
        self.look_at(lead_id, goal);
        for (_, id) in chasers {
            let goal = self
                .play_goal_for(id, lead.x, desktop)
                .or_else(|| self.play_goal_for(id, lead.x - away * CONTACT * unit, desktop));
            self.steer_play(id, goal, ActionKind::Sprint);
            self.look_at(id, lead);
        }
        if s.elapsed >= 2.5 {
            self.recruit_player(s, lead_id, PlayRole::Follow, 0.6, desktop);
        }
        true
    }

    pub(super) fn advance_parade(&mut self, s: &mut Session, desktop: &DesktopSnapshot) -> bool {
        let Some((_, lead_id)) = s.with_role(PlayRole::Lead).next() else {
            return false;
        };
        let Some(lead) = self.play_position(lead_id) else {
            return false;
        };
        let unit = self.play_unit(lead_id, desktop);
        // The front of a procession heads away from the creatures queued behind it, so it never
        // has to walk through its own followers. The seed only breaks a tie.
        let behind = s
            .with_role(PlayRole::Follow)
            .filter_map(|(_, id)| self.play_position(id))
            .map(|p| p.x)
            .sum::<f32>()
            / s.with_role(PlayRole::Follow).count().max(1) as f32;
        let heading = if (lead.x - behind).abs() < 1.0 {
            if s.seed.is_multiple_of(2) { 1.0 } else { -1.0 }
        } else if lead.x > behind {
            1.0
        } else {
            -1.0
        };
        let Some(goal) = [heading, -heading]
            .into_iter()
            .find_map(|direction| self.travel_goal(lead_id, lead, direction, unit, desktop))
        else {
            // Out of room: the front of the procession ends it with a flourish where it stands.
            self.pose_play(lead_id, Gesture::Cheer);
            return false;
        };
        let heading = (goal.x - lead.x).signum();
        self.steer_play(lead_id, Some(goal), ActionKind::Traverse);
        self.look_at(lead_id, goal);
        if s.elapsed >= s.ends_at - 1.4 {
            // The procession is winding down and stops where it is: the front of it cheers.
            self.pose_play(lead_id, Gesture::Cheer);
        }
        let followers: Vec<_> = s.with_role(PlayRole::Follow).collect();
        let mut ahead = lead;
        for (index, id) in followers {
            let Some(position) = self.play_position(id) else {
                continue;
            };
            // A follower left far behind is released rather than dragged along.
            if (position.x - lead.x).abs() > PARADE_LOST * unit {
                s.members[index] = None;
                self.release_player(id);
                continue;
            }
            // Keep station behind the creature ahead, with a little give when space is tight.
            let goal = [1.0, 0.8, 1.25].into_iter().find_map(|fraction| {
                self.play_goal_for(
                    id,
                    ahead.x - heading * PARADE_GAP * fraction * unit,
                    desktop,
                )
            });
            self.steer_play(id, goal, ActionKind::Traverse);
            self.look_at(id, ahead);
            ahead = position;
        }
        if s.members.iter().flatten().count() < 2 {
            return false;
        }
        if s.elapsed >= 2.0 {
            let tail = s
                .with_role(PlayRole::Follow)
                .last()
                .map_or(lead_id, |(_, id)| id);
            self.recruit_player(s, tail, PlayRole::Follow, 0.3, desktop);
        }
        true
    }

    pub(super) fn advance_dance(&mut self, s: &mut Session, desktop: &DesktopSnapshot) -> bool {
        let Some((_, lead_id)) = s.with_role(PlayRole::Lead).next() else {
            return false;
        };
        let Some(lead) = self.play_position(lead_id) else {
            return false;
        };
        self.steer_play(lead_id, None, ActionKind::SoloPlay);
        self.look_at(lead_id, lead);
        let partners: Vec<_> = s.with_role(PlayRole::Partner).collect();
        for (order, (_, id)) in partners.iter().enumerate() {
            let Some(position) = self.play_position(*id) else {
                continue;
            };
            // Gather to either side of the initiator, an arc flattened onto the surface.
            let side = if position.x >= lead.x { 1.0 } else { -1.0 };
            let spot = lead.x + side * self.held_gap(lead_id, desktop) * (1.0 + order as f32);
            let arrived = (position.x - spot).abs() <= self.held_slack(lead_id, desktop);
            let goal = (!arrived)
                .then(|| self.play_goal_for(*id, spot, desktop))
                .flatten();
            // Staggered responses: each dancer joins in on its own beat.
            let beat = s.elapsed - 0.6 - order as f32 * 0.45;
            let on_beat = beat > 0.0 && (beat % 1.2) < 0.7;
            let gesture = if goal.is_some() {
                ActionKind::Traverse
            } else if on_beat {
                ActionKind::Greet
            } else {
                ActionKind::SoloPlay
            };
            self.steer_play(*id, goal, gesture);
            if goal.is_none() && on_beat {
                self.pose_play(*id, Gesture::Bop);
            }
            self.look_at(*id, lead);
        }
        if s.elapsed >= 1.5 {
            self.recruit_player(s, lead_id, PlayRole::Partner, 0.5, desktop);
        }
        true
    }

    pub(super) fn advance_pile(&mut self, s: &mut Session, desktop: &DesktopSnapshot) -> bool {
        let Some((_, anchor_id)) = s.with_role(PlayRole::Anchor).next() else {
            return false;
        };
        let Some(anchor) = self.play_position(anchor_id) else {
            return false;
        };
        let resting = self
            .save
            .creatures
            .iter()
            .find(|c| c.id == anchor_id)
            .map_or(ActionKind::Idle, |c| {
                if c.state.surface.window_key.is_some() {
                    ActionKind::Perch
                } else {
                    ActionKind::Idle
                }
            });
        // The anchor keeps resting exactly as it was; nobody commandeers it.
        self.steer_play(anchor_id, None, resting);
        let arrivals: Vec<_> = s.with_role(PlayRole::Partner).collect();
        let mut settled = 0;
        for (order, (_, id)) in arrivals.iter().enumerate() {
            let Some(position) = self.play_position(*id) else {
                continue;
            };
            let side = if position.x >= anchor.x { 1.0 } else { -1.0 };
            let spot = anchor.x + side * self.held_gap(anchor_id, desktop) * (1.0 + order as f32);
            let arrived = (position.x - spot).abs() <= self.held_slack(anchor_id, desktop);
            settled += usize::from(arrived);
            let goal = (!arrived)
                .then(|| self.play_goal_for(*id, spot, desktop))
                .flatten();
            self.steer_play(
                *id,
                goal,
                if goal.is_some() {
                    ActionKind::Traverse
                } else {
                    resting
                },
            );
            self.look_at(*id, anchor);
        }
        self.look_at(anchor_id, anchor);
        if settled == arrivals.len() && s.elapsed >= 2.0 {
            self.recruit_player(s, anchor_id, PlayRole::Partner, 0.2, desktop);
        }
        true
    }

    /// Two companions take turns vaulting over one another along a surface. Each leap is an
    /// ordinary hop journey with a validated landing; the roles swap as it takes off.
    pub(super) fn advance_leapfrog(&mut self, s: &mut Session, desktop: &DesktopSnapshot) -> bool {
        let Some((front_index, front_id)) = s.with_role(PlayRole::Lead).next() else {
            return false;
        };
        let Some(front) = self.play_position(front_id) else {
            return false;
        };
        // Whoever is furthest behind takes the next turn, so a third companion joins the rotation.
        let jumpers: Vec<_> = s.with_role(PlayRole::Follow).collect();
        let Some(&(back_index, back_id)) = jumpers.iter().min_by(|a, b| {
            let distance = |id: CreatureId| {
                self.play_position(id)
                    .map_or(f32::MAX, |p| (p.x - front.x).abs())
            };
            distance(b.1).total_cmp(&distance(a.1))
        }) else {
            return false;
        };
        if self.window_journeys.contains_key(&back_id) {
            // Mid-leap: the journey owns the movement until it lands.
            self.look_at(back_id, front);
            return true;
        }
        let Some(back) = self.play_position(back_id) else {
            return false;
        };
        if s.swaps >= 4 {
            return false;
        }
        let heading = if front.x >= back.x { 1.0 } else { -1.0 };
        // The one in front crouches; everyone else waits their turn where they stand.
        self.steer_play(front_id, None, ActionKind::Perch);
        self.pose_play(front_id, Gesture::Crouch);
        self.look_at(front_id, back);
        for (_, id) in jumpers.iter().filter(|(_, id)| *id != back_id) {
            self.steer_play(*id, None, ActionKind::InspectScreen);
            self.look_at(*id, front);
        }
        let gap = (front.x - back.x).abs();
        let unit = self.play_unit(front_id, desktop);
        let held = self.held_gap(front_id, desktop);
        if gap > CONTACT * 1.3 * unit {
            // Close up behind the crouching companion first. The run-up is part of the vault, so
            // it comes to creature-scale contact and is over in a moment.
            let Some(goal) =
                self.play_goal_for(back_id, front.x - heading * CONTACT * unit, desktop)
            else {
                return false;
            };
            self.steer_play(back_id, Some(goal), ActionKind::Traverse);
            self.look_at(back_id, front);
            return true;
        }
        // Vault over, landing clear of the crouching companion on its far side.
        // Landing clear of the crouching companion, on its far side, where it then stands.
        let Some(landing) = self.play_goal_for(back_id, front.x + heading * held, desktop) else {
            return false;
        };
        let Some(creature) = self.save.creatures.iter().find(|c| c.id == back_id) else {
            return false;
        };
        let mut surface = creature.state.surface.clone();
        if let Some(window) = desktop
            .windows
            .iter()
            .find(|w| Some(w.key) == surface.window_key)
        {
            surface.relative_x =
                ((landing.x - window.bounds.x) / window.bounds.width).clamp(0.05, 0.95);
        }
        self.window_journeys.insert(
            back_id,
            WindowJourney::Hop(HopJourney {
                start: back,
                target: landing,
                surface,
                elapsed: 0.0,
                duration: 0.55,
                lift: 0.0,
                ballistic: false,
            }),
        );
        if let Some(plan) = self.attention.plans.get_mut(&back_id)
            && let Role::Play {
                hopping, gesture, ..
            } = &mut plan.role
        {
            *hopping = true;
            *gesture = ActionKind::Landing;
            plan.walk = None;
        }
        // Landing ahead makes this one the next to crouch.
        s.roles[front_index] = PlayRole::Follow;
        s.roles[back_index] = PlayRole::Lead;
        s.swaps += 1;
        s.turn_started = s.elapsed;
        if s.swaps == 2 {
            self.recruit_player(s, front_id, PlayRole::Follow, 0.6, desktop);
        }
        true
    }

    /// One toy, one holder. Chasers pursue it, contact hands it over, and the holder sometimes
    /// passes it on instead of being caught. The toy exists only while the scene does.
    pub(super) fn advance_keep_away(&mut self, s: &mut Session, desktop: &DesktopSnapshot) -> bool {
        let Some(prop) = s.prop else {
            return false;
        };
        self.hold_plaything(prop.holder, s.seed);
        if prop.remaining <= 0.0 || prop.handoffs >= 3 {
            // The tussle is over, but whoever ended up without the toy still reaches after it
            // while the scene winds down around them.
            for id in s.members.iter().flatten().copied() {
                if id != prop.holder {
                    self.pose_play(id, Gesture::Reach);
                }
            }
            return false;
        }
        let Some(holder) = self.play_position(prop.holder) else {
            return false;
        };
        let unit = self.play_unit(prop.holder, desktop);
        let others: Vec<_> = s
            .members
            .iter()
            .flatten()
            .copied()
            .filter(|id| *id != prop.holder)
            .collect();
        if others.is_empty() {
            return false;
        }
        // Close enough to take it: the toy changes hands and the roles reverse.
        let taker = others.iter().copied().find(|id| {
            self.play_position(*id)
                .is_some_and(|p| (p.x - holder.x).abs() <= CONTACT * unit)
        });
        if let Some(taker) = taker {
            self.hand_over_prop(s, taker, holder);
            return true;
        }
        // Keeping it: run from whoever is nearest, and taunt when there is room.
        let nearest = others
            .iter()
            .filter_map(|id| self.play_position(*id))
            .min_by(|a, b| (a.x - holder.x).abs().total_cmp(&(b.x - holder.x).abs()));
        let clear = nearest.is_none_or(|p| (p.x - holder.x).abs() > CHASE_LEAD * 0.8 * unit);
        let away = nearest.map_or(1.0, |p| if holder.x >= p.x { 1.0 } else { -1.0 });
        if clear {
            // Far enough ahead to show it off, which is also how a pass begins.
            self.steer_play(prop.holder, None, ActionKind::PresentDiscovery);
            self.look_at(prop.holder, nearest.unwrap_or(holder));
            if s.elapsed > 2.0 && s.seed.is_multiple_of(2) {
                let receiver = others[(s.seed as usize / 2) % others.len()];
                self.hand_over_prop(s, receiver, holder);
            }
        } else {
            let Some(goal) = [away, -away].into_iter().find_map(|direction| {
                self.travel_goal(prop.holder, holder, direction, unit, desktop)
            }) else {
                return false;
            };
            self.steer_play(prop.holder, Some(goal), ActionKind::Sprint);
            self.look_at(prop.holder, goal);
        }
        for id in others {
            let goal = self
                .play_goal_for(id, holder.x, desktop)
                .or_else(|| self.play_goal_for(id, holder.x - away * CONTACT * unit, desktop));
            self.steer_play(id, goal, ActionKind::Sprint);
            // Grabbing for the toy. A sprint keeps the body, so this shows once the scene winds
            // down and the chasers stop where they are, still reaching for whoever has it.
            self.pose_play(id, Gesture::Reach);
            self.look_at(id, holder);
        }
        true
    }

    /// Give the toy to someone else, with a moment of delight on both sides.
    fn hand_over_prop(&mut self, s: &mut Session, receiver: CreatureId, from: Point) {
        let Some(prop) = &mut s.prop else {
            return;
        };
        let previous = prop.holder;
        prop.holder = receiver;
        prop.handoffs += 1;
        s.turn_started = s.elapsed;
        if let Some(index) = s.members.iter().position(|m| *m == Some(previous)) {
            s.roles[index] = PlayRole::Follow;
        }
        if let Some(index) = s.members.iter().position(|m| *m == Some(receiver)) {
            s.roles[index] = PlayRole::Lead;
        }
        self.hold_plaything(receiver, s.seed);
        self.steer_play(receiver, None, ActionKind::PresentDiscovery);
        self.steer_play(previous, None, ActionKind::Greet);
        self.look_at(previous, self.play_position(receiver).unwrap_or(from));
        self.look_at(receiver, from);
    }

    /// Two creatures hold one toy between them and take turns pulling, until one of them has it.
    pub(super) fn advance_tug(
        &mut self,
        s: &mut Session,
        desktop: &DesktopSnapshot,
        dt: f32,
    ) -> bool {
        let Some(prop) = s.prop else {
            return false;
        };
        self.hold_plaything(prop.holder, s.seed);
        let mut members = s.members.iter().flatten().copied();
        let (Some(first), Some(second)) = (members.next(), members.next()) else {
            return false;
        };
        let (Some(a), Some(b)) = (self.play_position(first), self.play_position(second)) else {
            return false;
        };
        let unit = self.play_unit(first, desktop);
        // Settle in first: close enough to hold the same toy, facing each other.
        let gap = (a.x - b.x).abs();
        let held = self.held_gap(first, desktop);
        if gap > held * 1.1 {
            let midpoint = (a.x + b.x) * 0.5;
            let mut closing = false;
            for (id, position) in [(first, a), (second, b)] {
                let side = if position.x >= midpoint { 1.0 } else { -1.0 };
                let goal = self.play_goal_for(id, midpoint + side * held * 0.5, desktop);
                closing |= goal.is_some();
                self.steer_play(id, goal, ActionKind::Traverse);
            }
            self.look_at(first, b);
            self.look_at(second, a);
            // The walk stops a hand's width short, because what is left is shorter than a stride
            // either of them will take. That is as close as they get, and the tussle starts from
            // where they stand rather than waiting for a step neither can make.
            if closing {
                return true;
            }
        }
        // Alternate pulls: each turn the toy and both creatures shift a little.
        let turn = ((s.elapsed - s.turn_started) / 0.9) as u32;
        let puller = if turn.is_multiple_of(2) {
            first
        } else {
            second
        };
        let holder = prop.holder;
        let pulling = if puller == first { -1.0 } else { 1.0 };
        // Whoever is not digging in gets hauled a step. It has to be a step the walk will
        // actually take, or the pair would strain against each other without ever moving.
        let haul = (6.0 * unit).max(motion::MIN_STRIDE + 1.0);
        for (id, position) in [(first, a), (second, b)] {
            let shift = if id == puller { 0.0 } else { pulling * haul };
            let goal = (shift != 0.0)
                .then(|| self.play_goal_for(id, position.x + shift, desktop))
                .flatten();
            self.steer_play(
                id,
                goal,
                if id == holder {
                    ActionKind::PresentDiscovery
                } else {
                    ActionKind::SocialPlay
                },
            );
            // Both haul on the toy. Whoever holds it keeps showing it off, and whoever is being
            // dragged a step is travelling, so the pull shows on the one digging in this turn.
            self.pose_play(id, Gesture::Heave);
        }
        self.look_at(first, b);
        self.look_at(second, a);
        // The last pull decides it: one keeps the toy, the other sits back.
        if s.elapsed >= s.ends_at - 2.2 {
            let winner = if composure_of(self, first) >= composure_of(self, second) {
                first
            } else {
                second
            };
            let loser = if winner == first { second } else { first };
            if let Some(prop) = &mut s.prop {
                prop.holder = winner;
            }
            self.hold_plaything(winner, s.seed);
            self.steer_play(winner, None, ActionKind::PresentDiscovery);
            self.steer_play(loser, None, ActionKind::ReactToWindow);
        }
        let _ = dt;
        true
    }

    /// One creature is "it" and pursues the others. Contact passes the role on, with a short
    /// immunity so it cannot bounce straight back.
    pub(super) fn advance_tag(&mut self, s: &mut Session, desktop: &DesktopSnapshot) -> bool {
        let Some((it_index, it_id)) = s.with_role(PlayRole::Lead).next() else {
            return false;
        };
        let Some(it) = self.play_position(it_id) else {
            return false;
        };
        let unit = self.play_unit(it_id, desktop);
        let runners: Vec<_> = s.with_role(PlayRole::Follow).collect();
        if runners.is_empty() {
            return false;
        }
        let immune = s.elapsed - s.turn_started < TAG_IMMUNITY;
        let caught = (!immune && s.swaps < 3)
            .then(|| {
                runners.iter().find(|(_, id)| {
                    self.play_position(*id)
                        .is_some_and(|p| (p.x - it.x).abs() <= CONTACT * unit)
                })
            })
            .flatten();
        if let Some(&(caught_index, caught_id)) = caught {
            s.swaps += 1;
            s.roles[it_index] = PlayRole::Follow;
            s.roles[caught_index] = PlayRole::Lead;
            s.turn_started = s.elapsed;
            self.steer_play(it_id, None, ActionKind::Greet);
            self.steer_play(caught_id, None, ActionKind::ReactToWindow);
            self.pose_play(caught_id, Gesture::Gasp);
            self.look_at(caught_id, it);
            return true;
        }
        if s.swaps >= 3 {
            return false;
        }
        // "It" goes after the nearest; everyone else keeps away from it.
        let target = runners
            .iter()
            .filter_map(|(_, id)| self.play_position(*id).map(|p| (*id, p)))
            .min_by(|a, b| (a.1.x - it.x).abs().total_cmp(&(b.1.x - it.x).abs()));
        let Some((_, chased)) = target else {
            return false;
        };
        let goal = self
            .play_goal_for(it_id, chased.x, desktop)
            .or_else(|| self.travel_goal(it_id, it, (chased.x - it.x).signum(), unit, desktop));
        self.steer_play(it_id, goal, ActionKind::Sprint);
        self.look_at(it_id, chased);
        let mut room = false;
        for (_, id) in runners {
            let Some(position) = self.play_position(id) else {
                continue;
            };
            let away = if position.x >= it.x { 1.0 } else { -1.0 };
            let goal = [away, -away]
                .into_iter()
                .find_map(|direction| self.travel_goal(id, position, direction, unit, desktop));
            room |= goal.is_some();
            self.steer_play(
                id,
                goal,
                if immune {
                    ActionKind::Traverse
                } else {
                    ActionKind::Sprint
                },
            );
            self.look_at(id, it);
        }
        room
    }

    /// Turns at one gap. Each competitor walks to the edge and answers it in its own way, using
    /// the ordinary attempt machinery, while the others watch from where they stand.
    pub(super) fn advance_jump_contest(
        &mut self,
        s: &mut Session,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let taking_turn = s.members.iter().flatten().any(|id| {
            self.attention
                .plans
                .get(id)
                .is_some_and(|p| !matches!(p.role, Role::Play { .. }))
                || self.window_journeys.contains_key(id)
        });
        if taking_turn {
            // Watch whoever is up; their own scene supplies the stages.
            let Some(performer) = s.members.iter().flatten().copied().find(|id| {
                self.attention
                    .plans
                    .get(id)
                    .is_some_and(|p| !matches!(p.role, Role::Play { .. }))
            }) else {
                return true;
            };
            let Some(point) = self.play_position(performer) else {
                return true;
            };
            for id in s.members.iter().flatten().copied() {
                if id != performer
                    && self
                        .attention
                        .plans
                        .get(&id)
                        .is_some_and(|p| matches!(p.role, Role::Play { .. }))
                {
                    self.steer_play(id, None, ActionKind::InspectScreen);
                    self.look_at(id, point);
                }
            }
            return true;
        }
        // Whoever just finished their go rejoins the watchers before the next one steps up.
        let waiting: Vec<_> = s
            .members
            .iter()
            .flatten()
            .copied()
            .filter(|id| !self.attention.plans.contains_key(id))
            .collect();
        for id in waiting {
            let other = s
                .members
                .iter()
                .flatten()
                .copied()
                .find(|other| *other != id)
                .unwrap_or(id);
            self.join_play(id, other, s.origin, desktop);
        }
        if s.swaps as usize >= s.count.min(2) {
            return false;
        }
        // Next competitor: whoever has not been up yet, in member order.
        let Some(next) = s
            .members
            .iter()
            .flatten()
            .copied()
            .nth(s.swaps as usize)
            .filter(|id| {
                self.save
                    .creatures
                    .iter()
                    .any(|c| c.id == *id && c.state.surface.window_key.is_some())
            })
        else {
            return false;
        };
        let Some(creature) = self.save.creatures.iter().find(|c| c.id == next) else {
            return false;
        };
        let attempt = self.gap_candidate(creature, desktop);
        let Some((journey, decision, drop)) = attempt else {
            // Not at the edge yet: walk over to it, then judge the gap from there.
            let Some((edge, direction)) = self.gap_edge_for(creature, desktop) else {
                return false;
            };
            let unit = self.play_unit(next, desktop);
            let Some(goal) = self.play_goal_for(next, edge.x - direction * 20.0 * unit, desktop)
            else {
                return false;
            };
            self.steer_play(next, Some(goal), ActionKind::Traverse);
            self.look_at(next, edge);
            return true;
        };
        s.swaps += 1;
        s.turn_started = s.elapsed;
        self.attention.plans.remove(&next);
        match decision {
            gaps::GapDecision::Commit => {
                self.begin_gap_attention(next, journey, true, drop, desktop)
            }
            gaps::GapDecision::Hesitate => self.begin_hesitation(next, journey, drop, desktop),
            gaps::GapDecision::Refuse => {
                self.begin_gap_attention(next, journey, false, drop, desktop)
            }
        }
        true
    }

    /// Creeping around a sleeping companion: short steps, long freezes, and a lot of looking.
    /// The sleeper is the subject of the scene, never a member of it.
    pub(super) fn advance_hush(&mut self, s: &mut Session, desktop: &DesktopSnapshot) -> bool {
        let Some(sleeper) = s.focus else {
            return false;
        };
        if !self
            .save
            .creatures
            .iter()
            .any(|c| c.id == sleeper && c.state.action == ActionKind::Sleep)
        {
            return false;
        }
        let Some(point) = self.play_position(sleeper) else {
            return false;
        };
        let unit = self.play_unit(sleeper, desktop);
        for (index, id) in s.members.iter().flatten().copied().enumerate() {
            let Some(position) = self.play_position(id) else {
                continue;
            };
            // Each creeper has its own rhythm of tiny steps and held breath.
            let creeping = ((s.elapsed + index as f32 * 0.7) % 2.4) < 1.1;
            let side = if position.x >= point.x { 1.0 } else { -1.0 };
            let goal = creeping
                .then(|| self.play_goal_for(id, position.x - side * 12.0 * unit, desktop))
                .flatten()
                .filter(|goal| (goal.x - point.x).abs() > self.held_gap(sleeper, desktop));
            self.steer_play(
                id,
                goal,
                if goal.is_some() {
                    ActionKind::Traverse
                } else {
                    ActionKind::Perch
                },
            );
            if goal.is_none() {
                // Frozen mid-creep, low and holding its breath.
                self.pose_play(id, Gesture::Crouch);
            }
            self.look_at(id, point);
        }
        if s.elapsed >= 2.0 {
            self.recruit_player(s, sleeper, PlayRole::Partner, 0.2, desktop);
        }
        true
    }

    /// Trying to annoy a sleeping friend awake: a bounded number of attempts, and the sleeper
    /// only stirs if it has slept enough.
    pub(super) fn advance_prank(
        &mut self,
        s: &mut Session,
        desktop: &DesktopSnapshot,
        dt: f32,
    ) -> bool {
        let Some(sleeper) = s.focus else {
            return false;
        };
        let Some(&prankster) = s.members.iter().flatten().next() else {
            return false;
        };
        let asleep = self
            .save
            .creatures
            .iter()
            .any(|c| c.id == sleeper && c.state.action == ActionKind::Sleep);
        if !asleep {
            // Already awake: a moment of delight, then the scene is over.
            self.steer_play(prankster, None, ActionKind::Greet);
            self.pose_play(prankster, Gesture::Bop);
            return false;
        }
        let (Some(point), Some(position)) =
            (self.play_position(sleeper), self.play_position(prankster))
        else {
            return false;
        };
        let held = self.held_gap(sleeper, desktop);
        if (position.x - point.x).abs() > held * 1.15 {
            let side = if position.x >= point.x { 1.0 } else { -1.0 };
            let goal = self.play_goal_for(prankster, point.x + side * held, desktop);
            if goal.is_none() {
                return false;
            }
            self.steer_play(prankster, goal, ActionKind::Traverse);
            self.look_at(prankster, point);
            return true;
        }
        // In reach: pester in bouts, capped.
        let bout = ((s.elapsed - s.turn_started) / 1.4) as u8;
        if bout > s.swaps {
            s.swaps = bout;
            if s.swaps > 3 {
                return false;
            }
            let sleepy = self
                .save
                .creatures
                .iter()
                .find(|c| c.id == sleeper)
                .map_or(1.0, |c| c.state.drives.sleep_pressure);
            // A creature that still needs the rest simply sleeps through it.
            if sleepy < 0.4 && self.ambient_rng.random_ratio(1, 2) {
                self.wake_sleeper(sleeper, dt);
                // It worked: a little victory dance over the companion it woke.
                self.steer_play(prankster, None, ActionKind::Greet);
                self.pose_play(prankster, Gesture::Bop);
                return true;
            }
        }
        let poking = (s.elapsed % 1.4) < 0.7;
        self.steer_play(
            prankster,
            None,
            if poking {
                ActionKind::SocialPlay
            } else {
                ActionKind::InspectScreen
            },
        );
        if !poking {
            // Between pokes it keeps low and still, watching whether the sleeper stirs.
            self.pose_play(prankster, Gesture::Crouch);
        }
        self.look_at(prankster, point);
        true
    }

    /// Wake a creature the way an interruption elsewhere would, with the same events.
    fn wake_sleeper(&mut self, id: CreatureId, dt: f32) {
        let elapsed = self.sleep_elapsed.remove(&id).unwrap_or(0.0).max(0.0) as u32;
        let Some(creature) = creature_mut(&mut self.save.creatures, id) else {
            return;
        };
        creature.state.action = ActionKind::ReactToWindow;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 2.2;
        creature.state.drives.arousal = (creature.state.drives.arousal + 0.3).min(1.0);
        let _ = dt;
        World::emit(
            &mut self.events,
            WorldEvent::SleepInterrupted {
                creature_id: id,
                elapsed_seconds: elapsed,
            },
        );
        World::emit(
            &mut self.events,
            WorldEvent::CreatureWoke { creature_id: id },
        );
    }

    /// The middle of a creature's own ledge: the spot worth having.
    pub(super) fn hill_spot(&self, c: &Creature, desktop: &DesktopSnapshot) -> Option<Point> {
        let window = desktop
            .windows
            .iter()
            .take(MAX_TOPOLOGY_WINDOWS)
            .find(|w| Some(w.key) == c.state.surface.window_key && w.visible && !w.minimized)?;
        (window.bounds.width >= 160.0).then_some(Point {
            x: window.bounds.x + window.bounds.width * 0.5,
            y: window.bounds.y,
        })
    }

    /// Followers walk the leader's own recent path, each as far as it is willing to go.
    pub(super) fn advance_follow_leader(
        &mut self,
        s: &mut Session,
        desktop: &DesktopSnapshot,
        dt: f32,
    ) -> bool {
        let Some((_, lead_id)) = s.with_role(PlayRole::Lead).next() else {
            return false;
        };
        let Some(lead) = self.play_position(lead_id) else {
            return false;
        };
        let unit = self.play_unit(lead_id, desktop);
        // Remember a short trail of where the leader has been, nothing more.
        if s.elapsed - s.turn_started >= 0.55 {
            s.turn_started = s.elapsed;
            s.trail.rotate_right(1);
            s.trail[0] = Some(lead);
        }
        let heading = if s.seed.is_multiple_of(2) { 1.0 } else { -1.0 };
        let Some(goal) = [heading, -heading]
            .into_iter()
            .find_map(|direction| self.travel_goal(lead_id, lead, direction, unit, desktop))
        else {
            // Nowhere left to lead: a flourish from the front, and the game is over.
            self.pose_play(lead_id, Gesture::Cheer);
            return false;
        };
        self.steer_play(lead_id, Some(goal), ActionKind::Traverse);
        self.look_at(lead_id, goal);
        let mut following = false;
        for (_, id) in s.with_role(PlayRole::Follow).collect::<Vec<_>>() {
            let Some(position) = self.play_position(id) else {
                continue;
            };
            // Take the oldest remembered point still ahead of this follower.
            let next = s
                .trail
                .iter()
                .flatten()
                .rev()
                .find(|point| (point.x - position.x).abs() > 18.0 * unit)
                .copied();
            let goal = next.and_then(|point| self.play_goal_for(id, point.x, desktop));
            match goal {
                Some(goal) => {
                    following = true;
                    self.steer_play(id, Some(goal), ActionKind::Traverse);
                    self.look_at(id, lead);
                }
                // Unwilling or unable to copy this step: hesitate in place and keep watching.
                None => {
                    self.steer_play(id, None, ActionKind::InspectScreen);
                    self.look_at(id, next.unwrap_or(lead));
                }
            }
        }
        let _ = dt;
        let continuing = following || s.elapsed < 3.0;
        if !continuing || s.elapsed >= s.ends_at - 1.4 {
            // The route is over and the line stops where it is: the leader finishes with a
            // flourish.
            self.pose_play(lead_id, Gesture::Cheer);
        }
        continuing
    }

    /// A friendly contest for the middle of a ledge, settled by stepping aside rather than shoving.
    pub(super) fn advance_hill(&mut self, s: &mut Session, desktop: &DesktopSnapshot) -> bool {
        let Some((king_index, king_id)) = s.with_role(PlayRole::Lead).next() else {
            return false;
        };
        let Some(king) = self.play_position(king_id) else {
            return false;
        };
        let Some(spot) = self
            .save
            .creatures
            .iter()
            .find(|c| c.id == king_id)
            .and_then(|c| self.hill_spot(c, desktop))
        else {
            return false;
        };
        let unit = self.play_unit(king_id, desktop);
        let challengers: Vec<_> = s.with_role(PlayRole::Follow).collect();
        if challengers.is_empty() {
            return false;
        }
        // Close enough to make the claim: the occupant yields the spot and steps aside.
        let claim = self.held_gap(king_id, desktop) * 1.3;
        let claimant = challengers.iter().find(|(_, id)| {
            self.play_position(*id)
                .is_some_and(|p| (p.x - king.x).abs() <= claim)
        });
        if let Some(&(claim_index, claim_id)) = claimant {
            if s.swaps >= 2 {
                // The spot has changed hands as often as it is going to: whoever is standing on
                // it keeps it and says so, and the rest are left straining at it from below.
                if (king.x - spot.x).abs() <= 12.0 * unit {
                    self.pose_play(king_id, Gesture::Cheer);
                }
                for (_, id) in &challengers {
                    self.pose_play(*id, Gesture::Heave);
                }
                return false;
            }
            s.swaps += 1;
            s.roles[king_index] = PlayRole::Follow;
            s.roles[claim_index] = PlayRole::Lead;
            s.turn_started = s.elapsed;
            let aside = if king.x >= spot.x { 1.0 } else { -1.0 };
            let goal = self.play_goal_for(king_id, king.x + aside * CONTACT * 1.6 * unit, desktop);
            self.steer_play(king_id, goal, ActionKind::Traverse);
            self.steer_play(claim_id, None, ActionKind::Greet);
            // The new holder of the spot celebrates taking it.
            self.pose_play(claim_id, Gesture::Cheer);
            self.look_at(king_id, spot);
            self.look_at(claim_id, spot);
            return true;
        }
        // Holding the spot, or heading for it.
        let on_top = (king.x - spot.x).abs() <= 12.0 * unit;
        let goal = (!on_top)
            .then(|| self.play_goal_for(king_id, spot.x, desktop))
            .flatten();
        self.steer_play(
            king_id,
            goal,
            if goal.is_some() {
                ActionKind::Traverse
            } else {
                ActionKind::Perch
            },
        );
        if on_top {
            self.pose_play(king_id, Gesture::Cheer);
        }
        self.look_at(king_id, spot);
        for (_, id) in challengers {
            let Some(position) = self.play_position(id) else {
                continue;
            };
            // Come up beside whoever is on it, close enough to make the claim.
            let side = if position.x >= spot.x { 1.0 } else { -1.0 };
            let held = self.held_gap(id, desktop);
            let goal = self
                .play_goal_for(id, spot.x + side * held, desktop)
                .or_else(|| self.play_goal_for(id, spot.x + side * held * 1.4, desktop));
            self.steer_play(id, goal, ActionKind::Traverse);
            // Straining to get up there. Walking keeps the body, so this shows once a challenger
            // is left standing below the spot as the contest winds down.
            self.pose_play(id, Gesture::Heave);
            self.look_at(id, spot);
        }
        true
    }

    /// A member whose own hop has finished rejoins the scene before the next stretch.
    pub(super) fn rejoin_players(&mut self, s: &mut Session, desktop: &DesktopSnapshot) {
        let waiting: Vec<_> = s
            .members
            .iter()
            .flatten()
            .copied()
            .filter(|id| {
                !self.attention.plans.contains_key(id) && !self.window_journeys.contains_key(id)
            })
            .collect();
        for id in waiting {
            let other = s
                .members
                .iter()
                .flatten()
                .copied()
                .find(|other| *other != id)
                .unwrap_or(id);
            self.join_play(id, other, s.origin, desktop);
        }
    }

    /// Clear a finished leap so the next turn can begin.
    pub(super) fn settle_leaps(&mut self) {
        for (&id, plan) in &mut self.attention.plans {
            if let Role::Play {
                hopping, gesture, ..
            } = &mut plan.role
                && *hopping
                && !self.window_journeys.contains_key(&id)
            {
                *hopping = false;
                *gesture = ActionKind::Perch;
                plan.action = ActionKind::Perch;
            }
        }
    }

    /// Let one member go without ending the scene for everyone else.
    pub(super) fn release_player(&mut self, id: CreatureId) {
        if let Some(plan) = self.attention.plans.remove(&id)
            && let Some(creature) = creature_mut(&mut self.save.creatures, id)
        {
            release(creature, plan);
        }
        self.attention.cooldowns.insert(id, 20.0);
    }
}

#[cfg(test)]
pub(in super::super) mod tests;
