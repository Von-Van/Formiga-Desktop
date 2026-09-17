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
    pub(super) fn steer_play(
        &mut self,
        id: CreatureId,
        destination: Option<Point>,
        gesture: ActionKind,
    ) {
        let Some(plan) = self.attention.plans.get_mut(&id) else {
            return;
        };
        if let Role::Play {
            gesture: current, ..
        } = &mut plan.role
        {
            *current = gesture;
        }
        plan.walk = destination.map(ShortWalk::playing);
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
            // A moment of delight at the hand-over, then the roles reverse.
            self.steer_play(lead_id, None, ActionKind::Greet);
            self.steer_play(catcher_id, None, ActionKind::Greet);
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
            return false;
        };
        let heading = (goal.x - lead.x).signum();
        self.steer_play(lead_id, Some(goal), ActionKind::Traverse);
        self.look_at(lead_id, goal);
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
        let unit = self.play_unit(lead_id, desktop);
        self.steer_play(lead_id, None, ActionKind::SoloPlay);
        self.look_at(lead_id, lead);
        let partners: Vec<_> = s.with_role(PlayRole::Partner).collect();
        for (order, (_, id)) in partners.iter().enumerate() {
            let Some(position) = self.play_position(*id) else {
                continue;
            };
            // Gather to either side of the initiator, an arc flattened onto the surface.
            let side = if position.x >= lead.x { 1.0 } else { -1.0 };
            let spot = lead.x + side * CONTACT * (1.0 + order as f32) * unit;
            let arrived = (position.x - spot).abs() <= CONTACT * 0.5 * unit;
            let goal = (!arrived)
                .then(|| self.play_goal_for(*id, spot, desktop))
                .flatten();
            // Staggered responses: each dancer joins in on its own beat.
            let beat = s.elapsed - 0.6 - order as f32 * 0.45;
            let gesture = if goal.is_some() {
                ActionKind::Traverse
            } else if beat > 0.0 && (beat % 1.2) < 0.7 {
                ActionKind::Greet
            } else {
                ActionKind::SoloPlay
            };
            self.steer_play(*id, goal, gesture);
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
        let unit = self.play_unit(anchor_id, desktop);
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
            let spot = anchor.x + side * CONTACT * (1.0 + order as f32) * unit;
            let arrived = (position.x - spot).abs() <= CONTACT * 0.5 * unit;
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
        let unit = self.play_unit(front_id, desktop);
        if s.swaps >= 4 {
            return false;
        }
        let heading = if front.x >= back.x { 1.0 } else { -1.0 };
        // The one in front crouches; everyone else waits their turn where they stand.
        self.steer_play(front_id, None, ActionKind::Perch);
        self.look_at(front_id, back);
        for (_, id) in jumpers.iter().filter(|(_, id)| *id != back_id) {
            self.steer_play(*id, None, ActionKind::InspectScreen);
            self.look_at(*id, front);
        }
        let gap = (front.x - back.x).abs();
        if gap > CONTACT * 1.3 * unit {
            // Close up behind the crouching companion first.
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
        let Some(landing) =
            self.play_goal_for(back_id, front.x + heading * CONTACT * 1.6 * unit, desktop)
        else {
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
        if prop.remaining <= 0.0 || prop.handoffs >= 3 {
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
        if gap > CONTACT * 1.4 * unit {
            let midpoint = (a.x + b.x) * 0.5;
            for (id, position) in [(first, a), (second, b)] {
                let side = if position.x >= midpoint { 1.0 } else { -1.0 };
                let goal = self.play_goal_for(id, midpoint + side * CONTACT * 0.6 * unit, desktop);
                self.steer_play(id, goal, ActionKind::Traverse);
            }
            self.look_at(first, b);
            self.look_at(second, a);
            return true;
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
        for (id, position) in [(first, a), (second, b)] {
            let shift = if id == puller {
                0.0
            } else {
                pulling * 6.0 * unit
            };
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
                .filter(|goal| (goal.x - point.x).abs() > CONTACT * unit);
            self.steer_play(
                id,
                goal,
                if goal.is_some() {
                    ActionKind::Traverse
                } else {
                    ActionKind::Perch
                },
            );
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
            return false;
        }
        let (Some(point), Some(position)) =
            (self.play_position(sleeper), self.play_position(prankster))
        else {
            return false;
        };
        let unit = self.play_unit(sleeper, desktop);
        if (position.x - point.x).abs() > CONTACT * 1.4 * unit {
            let side = if position.x >= point.x { 1.0 } else { -1.0 };
            let goal = self.play_goal_for(prankster, point.x + side * CONTACT * unit, desktop);
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
                self.steer_play(prankster, None, ActionKind::Greet);
                return true;
            }
        }
        self.steer_play(
            prankster,
            None,
            if (s.elapsed % 1.4) < 0.7 {
                ActionKind::SocialPlay
            } else {
                ActionKind::InspectScreen
            },
        );
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
        following || s.elapsed < 3.0
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
        let claimant = challengers.iter().find(|(_, id)| {
            self.play_position(*id)
                .is_some_and(|p| (p.x - king.x).abs() <= CONTACT * 1.6 * unit)
        });
        if let Some(&(claim_index, claim_id)) = claimant {
            if s.swaps >= 2 {
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
            self.look_at(king_id, spot);
            self.look_at(claim_id, spot);
            return true;
        }
        // Holding the spot, or heading for it.
        let goal = ((king.x - spot.x).abs() > 12.0 * unit)
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
        self.look_at(king_id, spot);
        for (_, id) in challengers {
            let Some(position) = self.play_position(id) else {
                continue;
            };
            // Come up beside whoever is on it, close enough to make the claim.
            let side = if position.x >= spot.x { 1.0 } else { -1.0 };
            let goal = self
                .play_goal_for(id, spot.x + side * CONTACT * unit, desktop)
                .or_else(|| self.play_goal_for(id, spot.x + side * CONTACT * 1.6 * unit, desktop));
            self.steer_play(id, goal, ActionKind::Traverse);
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
    fn release_player(&mut self, id: CreatureId) {
        if let Some(plan) = self.attention.plans.remove(&id)
            && let Some(creature) = creature_mut(&mut self.save.creatures, id)
        {
            release(creature, plan);
        }
        self.attention.cooldowns.insert(id, 20.0);
    }
}

#[cfg(test)]
mod tests {
    use super::super::play::Kind;
    use super::super::play::tests::{Audience, everyone_settled, scene, tick};
    use super::*;

    /// Creature 0 walks after creature 1, the ordinary encounter these games grow from.
    fn following_scene(playful: bool) -> (World, DesktopSnapshot, OffsetDateTime) {
        let (mut w, d, now) = scene(false);
        let (follower, target) = (w.save.creatures[0].id, w.save.creatures[1].id);
        for c in w.save.creatures.iter_mut() {
            if !playful {
                c.personality.playfulness = 0.2;
            }
        }
        w.bond_plans.insert(
            follower,
            BondPlan {
                target,
                final_action: ActionKind::SocialPlay,
                experience: RelationshipExperience::PositivePlay,
                approaching: true,
            },
        );
        let c = &mut w.save.creatures[0];
        c.state.action = ActionKind::Follow;
        c.state.action_duration = 100.0;
        c.state.action_elapsed = 0.0;
        (w, d, now)
    }

    fn session_kind(w: &World) -> Option<Kind> {
        w.attention.play.session.map(|s| s.kind)
    }

    fn window_bounds(d: &DesktopSnapshot) -> (f32, f32) {
        let w = d.windows.iter().find(|w| w.key == 701).unwrap();
        (w.bounds.x, w.bounds.right())
    }

    #[test]
    fn lively_pursuit_becomes_a_chase_that_swaps_roles_and_ends_on_its_own() {
        let (mut w, mut d, now) = following_scene(true);
        let (left, right) = window_bounds(&d);
        // The last companion is far too unplayful to be drawn into a chase, so it watches.
        let watcher = w.save.creatures[3].id;
        let mut audience = Audience::default();
        let mut started = false;
        let mut ran = false;
        let mut swapped = false;
        let mut finished = false;
        let energy_before = w.save.creatures[0].state.drives.energy;
        for step in 1..300 {
            tick(&mut w, &mut d, now, step);
            audience.note(&w);
            if let Some(s) = w.attention.play.session {
                assert_eq!(s.kind, Kind::Chase);
                started = true;
                swapped |= s.swaps > 0;
                ran |= w
                    .save
                    .creatures
                    .iter()
                    .any(|c| c.state.action == ActionKind::Sprint);
                for c in w.save.creatures.iter().take(2) {
                    assert!(c.state.position.x > left && c.state.position.x < right);
                    assert_eq!(c.state.surface.window_key, Some(701));
                }
            } else if started {
                finished = true;
                assert!(everyone_settled(&w), "the audience goes with the chase");
                break;
            }
        }
        assert!(
            started && ran && swapped && finished,
            "{started} {ran} {swapped} {finished}"
        );
        // Somebody is watching the running, not sitting through it: the gaze stays on a runner
        // the length of the ledge, and the face changes when the running stops.
        assert!(audience.coherent(), "{audience:?}");
        let looked = audience.looked_at(watcher);
        assert!(looked.len() >= 6, "a watcher follows the chase: {looked:?}");
        let felt = audience.felt_by(watcher);
        assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
        assert!(felt.len() >= 2, "and feels the end of it: {felt:?}");
        assert!(w.save.creatures[0].state.drives.energy < energy_before);
        assert!(w.attention.plans.is_empty());
        assert!(w.bond_plans.is_empty(), "the follow plan became the game");
        assert!(!w.drain_events().any(|e| matches!(
            e,
            WorldEvent::BondInteraction { .. } | WorldEvent::ActionCompleted { .. }
        )));
    }

    #[test]
    fn a_calm_follow_becomes_a_spaced_procession_that_releases_a_lost_follower() {
        let (mut w, mut d, now) = following_scene(false);
        let watcher = w.save.creatures[3].id;
        let mut audience = Audience::default();
        let mut started = false;
        let mut spaced = false;
        for step in 1..200 {
            tick(&mut w, &mut d, now, step);
            audience.note(&w);
            let Some(s) = w.attention.play.session else {
                if started {
                    break;
                }
                continue;
            };
            assert_eq!(s.kind, Kind::Parade);
            started = true;
            let (lead, follower) = (
                w.save.creatures[1].state.position,
                w.save.creatures[0].state.position,
            );
            if s.elapsed > 2.0 {
                let gap = (lead.x - follower.x).abs();
                spaced |= (20.0..140.0).contains(&gap);
            }
            // Separate the follower: the procession lets it go instead of dragging it along.
            // The ledge attachment has to move with it, or the next scan pulls it back.
            if s.elapsed > 3.0 {
                let far = if lead.x > 500.0 { 0.05 } else { 0.95 };
                let c = &mut w.save.creatures[0];
                c.state.surface.relative_x = far;
                c.state.position.x = 200.0 + 600.0 * far;
                tick(&mut w, &mut d, now, step + 1);
                assert!(w.attention.play.session.is_none());
                assert!(!w.attention.owns(w.save.creatures[0].id));
                // A procession that falls apart takes its audience with it: the watchers are
                // let go in the same moment, rather than left staring at an empty ledge.
                assert!(everyone_settled(&w), "{audience:?}");
                break;
            }
        }
        assert!(started && spaced);
        // Watching a procession is watching the front of it walk away. This one is cut short on
        // purpose, so the audience's whole part in it is the looking.
        assert!(audience.coherent(), "{audience:?}");
        let looked = audience.looked_at(watcher);
        assert!(looked.len() >= 4, "the gaze travels with it: {looked:?}");
        assert_eq!(
            audience.felt_by(watcher).first(),
            Some(&AttentionEmotion::Curious)
        );
    }

    #[test]
    fn a_solo_flourish_gathers_a_dance_circle_with_staggered_responses() {
        let (mut w, mut d, now) = scene(false);
        for (i, c) in w.save.creatures.iter_mut().enumerate() {
            c.state.action = if i == 0 {
                ActionKind::SoloPlay
            } else {
                ActionKind::Perch
            };
            c.state.action_duration = 100.0;
        }
        // The second companion is playful enough to be pulled into the circle; the last is not.
        let (joiner, watcher) = (w.save.creatures[2].id, w.save.creatures[3].id);
        let mut audience = Audience::default();
        let mut members = BTreeSet::new();
        let mut gestured = BTreeSet::new();
        let mut approached = false;
        let mut started = false;
        for step in 1..260 {
            tick(&mut w, &mut d, now, step);
            audience.note(&w);
            let Some(s) = w.attention.play.session else {
                if started {
                    assert!(everyone_settled(&w), "{audience:?}");
                    break;
                }
                continue;
            };
            assert_eq!(s.kind, Kind::Dance);
            started = true;
            for (&id, p) in &w.attention.plans {
                if let Role::Play { gesture, .. } = p.role {
                    members.insert(id);
                    if gesture == ActionKind::Greet {
                        gestured.insert(id);
                    }
                    approached |= gesture == ActionKind::Traverse;
                }
            }
        }
        assert!(started && approached);
        assert!(members.len() >= 3, "the circle gathers companions");
        assert!(!gestured.is_empty());
        assert!(w.attention.plans.is_empty());
        // The circle grows out of its own audience, and openly: the companion it invites in
        // takes a dancer's part, while the one with no appetite for it watches the whole thing
        // through, feeling one way about the dancing and another about its ending.
        assert!(audience.coherent(), "{audience:?}");
        assert!(audience.joined.contains(&joiner), "{audience:?}");
        assert_eq!(audience.watchers(), vec![watcher], "{audience:?}");
        let felt = audience.felt_by(watcher);
        assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
        assert!(
            felt.len() >= 2,
            "the dance ends on the watcher too: {felt:?}"
        );
    }

    #[test]
    fn resting_company_forms_a_pile_without_commandeering_the_anchor() {
        let (mut w, mut d, now) = scene(false);
        for (i, c) in w.save.creatures.iter_mut().enumerate() {
            c.state.action = ActionKind::Perch;
            c.state.action_duration = 100.0;
            c.state.action_elapsed = 8.0;
            c.state.drives.social_need = 0.9;
            c.personality.sociability = 0.9;
            // Calm company: nobody is staring, vaulting, or dancing.
            c.state.facing_right = true;
            c.personality.curiosity = if i == 0 { 0.8 } else { 0.2 };
            c.personality.playfulness = 0.2;
        }
        let anchor_start = w.save.creatures[1].state.position;
        let mut audience = Audience::default();
        let mut started = false;
        let mut settled_close = false;
        for step in 1..240 {
            tick(&mut w, &mut d, now, step);
            audience.note(&w);
            let Some(s) = w.attention.play.session else {
                if started {
                    assert!(everyone_settled(&w), "{audience:?}");
                    break;
                }
                continue;
            };
            assert_eq!(s.kind, Kind::Pile);
            started = true;
            // The anchor keeps resting exactly where it was, from the first full tick on.
            assert_eq!(w.save.creatures[1].state.position, anchor_start);
            if s.elapsed > 0.2 {
                assert_eq!(w.save.creatures[1].state.action, ActionKind::Perch);
            }
            let gap = (w.save.creatures[0].state.position.x - anchor_start.x).abs();
            settled_close |= s.elapsed > 2.0 && gap < 60.0;
        }
        assert!(started && settled_close);
        assert!(w.attention.plans.is_empty());
        // Even settling down has an audience. The two companions who do not join keep their eyes
        // on the creature the pile is forming around — it never moves, so neither does the gaze —
        // and they are calm about the whole thing by the time it is over.
        assert!(audience.coherent(), "{audience:?}");
        let watchers = audience.watchers();
        assert_eq!(watchers.len(), 2, "{audience:?}");
        for id in watchers {
            let looked = audience.looked_at(id);
            assert!(
                looked
                    .iter()
                    .all(|x| (x - anchor_start.x as i32).abs() <= 12),
                "a watcher's gaze rests on the anchor: {looked:?}"
            );
            let felt = audience.felt_by(id);
            assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
            assert!(felt.len() >= 2, "and settles with the pile: {felt:?}");
        }
    }

    #[test]
    fn playful_companions_take_turns_vaulting_over_one_another() {
        let (mut w, mut d, now) = scene(false);
        for (i, c) in w.save.creatures.iter_mut().enumerate() {
            c.state.action = ActionKind::Perch;
            c.state.action_duration = 100.0;
            c.state.action_elapsed = 0.0;
            // The last companion has no appetite for vaulting, so it never joins the rotation.
            c.personality.playfulness = if i == 3 { 0.2 } else { 0.9 };
            c.personality.curiosity = 0.2; // Not a staring contest.
            c.state.facing_right = true;
        }
        let (first, second) = (w.save.creatures[0].id, w.save.creatures[1].id);
        let watcher = w.save.creatures[3].id;
        let start = w.save.creatures[0].state.position.x;
        let mut audience = Audience::default();
        let mut started = false;
        let mut airborne = BTreeSet::new();
        let mut progressed = false;
        let mut vaulted = false;
        for step in 1..300 {
            tick(&mut w, &mut d, now, step);
            audience.note(&w);
            let Some(s) = w.attention.play.session else {
                if started {
                    assert!(everyone_settled(&w), "{audience:?}");
                    break;
                }
                continue;
            };
            assert_eq!(s.kind, Kind::Leapfrog);
            started = true;
            assert!(s.swaps <= 4, "turns are bounded");
            for id in [first, second] {
                if w.window_journeys.contains_key(&id) {
                    airborne.insert(id);
                    let c = w.save.creatures.iter().find(|c| c.id == id).unwrap();
                    assert_eq!(c.state.action, ActionKind::Landing);
                }
            }
            progressed |= w.save.creatures[0].state.position.x > start + 40.0;
            for c in w.save.creatures.iter().take(2) {
                assert_eq!(c.state.surface.window_key, Some(701));
                // Only a leap in progress leaves the ledge, and only upward.
                if w.window_journeys.contains_key(&c.id) {
                    assert!(c.state.position.y <= 600.0);
                    vaulted |= c.state.position.y < 597.0;
                } else {
                    assert_eq!(c.state.position.y, 600.0);
                }
            }
        }
        assert!(
            started && progressed && vaulted,
            "{started} {progressed} {vaulted}"
        );
        assert_eq!(airborne.len(), 2, "both take a turn");
        assert!(w.window_journeys.is_empty());
        assert!(w.attention.plans.is_empty());
        for c in w.save.creatures.iter().take(2) {
            assert!(matches!(
                c.state.action,
                ActionKind::Perch | ActionKind::Idle | ActionKind::Traverse
            ));
        }
        // The companion sitting this one out watches the vaulting travel down the ledge, and is
        // never quietly counted as one of the vaulters.
        assert!(audience.coherent(), "{audience:?}");
        assert!(audience.watchers().contains(&watcher), "{audience:?}");
        let looked = audience.looked_at(watcher);
        assert!(looked.len() >= 4, "the gaze follows the turns: {looked:?}");
        let felt = audience.felt_by(watcher);
        assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
        assert!(felt.len() >= 2, "and changes at the end: {felt:?}");
    }

    #[test]
    fn a_toy_changes_hands_and_disappears_with_the_scene() {
        let (mut w, mut d, now) = scene(false);
        for (i, c) in w.save.creatures.iter_mut().enumerate() {
            c.state.action = if i == 0 {
                ActionKind::SocialPlay
            } else {
                ActionKind::Perch
            };
            c.state.action_duration = 100.0;
            c.personality.playfulness = 0.9;
            c.personality.curiosity = 0.2;
        }
        let first = w.save.creatures[0].id;
        let mut audience = Audience::default();
        let mut holders = BTreeSet::new();
        let mut shown = BTreeSet::new();
        let mut chased = false;
        let mut started = false;
        for step in 1..300 {
            tick(&mut w, &mut d, now, step);
            audience.note(&w);
            let Some(s) = w.attention.play.session else {
                if started {
                    assert!(everyone_settled(&w), "{audience:?}");
                    break;
                }
                continue;
            };
            assert_eq!(s.kind, Kind::KeepAway);
            started = true;
            let prop = s.prop.expect("keep-away always has its toy");
            assert!(prop.handoffs <= 3, "hand-offs are bounded");
            holders.insert(prop.holder);
            for c in &w.save.creatures {
                if c.state.action == ActionKind::PresentDiscovery {
                    // Only whoever is holding it shows the toy.
                    assert_eq!(c.id, prop.holder);
                    shown.insert(c.id);
                }
                chased |= c.id != prop.holder && c.state.action == ActionKind::Sprint;
            }
        }
        assert!(started && chased);
        assert!(holders.len() >= 2, "the toy changes hands");
        assert!(shown.contains(&first));
        // The toy is runtime-only and goes with the scene.
        assert!(w.attention.play.session.is_none());
        assert!(
            w.save
                .creatures
                .iter()
                .all(|c| c.state.action != ActionKind::PresentDiscovery)
        );
        assert!(!w.drain_events().any(|e| matches!(
            e,
            WorldEvent::ActionCompleted {
                action: ActionKind::PresentDiscovery,
                ..
            }
        )));
        assert!(
            w.save
                .creatures
                .iter()
                .all(|c| c.memory.discoveries_found == 0),
            "a plaything is never a discovery"
        );
        // The tussle is watched. Both companions who are not in it track it up and down the
        // ledge, and both are playful enough to cheer it out at the end rather than just being
        // glad it is over.
        assert!(audience.coherent(), "{audience:?}");
        let watchers = audience.watchers();
        assert_eq!(watchers.len(), 2, "{audience:?}");
        for id in watchers {
            let looked = audience.looked_at(id);
            assert!(looked.len() >= 6, "a watcher tracks the toy: {looked:?}");
            let felt = audience.felt_by(id);
            assert!(felt.contains(&AttentionEmotion::Enjoying), "{felt:?}");
            assert!(audience.cheered.contains(&id), "{audience:?}");
            // The toy is what the scene is about, so the gaze goes with it: every time it
            // changes hands the audience is handed the new holder.
            assert!(
                audience.followed[&id].len() > 2,
                "the audience follows the toy: {audience:?}"
            );
        }
    }

    #[test]
    fn a_tug_of_war_pulls_both_ways_and_ends_with_one_holder() {
        let (mut w, mut d, now) = scene(false);
        for (i, c) in w.save.creatures.iter_mut().enumerate() {
            c.state.action = if i == 0 {
                ActionKind::SoloPlay
            } else {
                ActionKind::Perch
            };
            c.state.action_duration = 100.0;
            // Exactly one playful companion, so this is a tussle and not a dance.
            c.personality.playfulness = if i < 2 { 0.9 } else { 0.1 };
            c.personality.curiosity = 0.2;
            c.state.facing_right = i == 0;
        }
        let (first, second) = (w.save.creatures[0].id, w.save.creatures[1].id);
        let mut audience = Audience::default();
        let mut started = false;
        let mut closed_in = false;
        let mut pulled_both_ways = BTreeSet::new();
        let mut settled = false;
        for step in 1..300 {
            tick(&mut w, &mut d, now, step);
            audience.note(&w);
            let Some(s) = w.attention.play.session else {
                if started {
                    assert!(everyone_settled(&w), "{audience:?}");
                    break;
                }
                continue;
            };
            assert_eq!(s.kind, Kind::Tug);
            started = true;
            let (a, b) = (
                w.save.creatures[0].state.position.x,
                w.save.creatures[1].state.position.x,
            );
            closed_in |= (a - b).abs() < 60.0;
            for c in w.save.creatures.iter().take(2) {
                if c.state.velocity.x.abs() > 0.5 {
                    pulled_both_ways.insert(c.state.velocity.x > 0.0);
                }
            }
            settled |= s.elapsed >= s.ends_at - 2.2
                && w.save
                    .creatures
                    .iter()
                    .filter(|c| c.state.action == ActionKind::PresentDiscovery)
                    .count()
                    == 1;
        }
        assert!(started && closed_in, "{started} {closed_in}");
        assert_eq!(pulled_both_ways.len(), 2, "the toy moves both ways");
        assert!(settled, "one of them ends up with it");
        assert!(w.attention.plans.is_empty());
        let _ = (first, second);
        // The two companions with no appetite for a tussle still watch one: their eyes stay on a
        // puller, and when it is decided they are relieved rather than delighted, which is not
        // how the pair who wanted the toy take it.
        assert!(audience.coherent(), "{audience:?}");
        let watchers = audience.watchers();
        assert_eq!(watchers.len(), 2, "{audience:?}");
        for id in watchers {
            let felt = audience.felt_by(id);
            assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
            assert!(felt.contains(&AttentionEmotion::Relieved), "{felt:?}");
            assert!(!felt.contains(&AttentionEmotion::Enjoying), "{felt:?}");
            assert!(!audience.cheered.contains(&id), "{audience:?}");
        }
    }

    #[test]
    fn tag_passes_the_role_on_contact_and_cannot_bounce_straight_back() {
        let (mut w, mut d, now) = scene(false);
        for (i, c) in w.save.creatures.iter_mut().enumerate() {
            c.state.action = if i == 0 {
                ActionKind::Sprint
            } else {
                ActionKind::Perch
            };
            c.state.action_duration = 100.0;
            // Two watchers of different temper: one who would love a game, one who would not.
            c.personality.playfulness = if i == 3 { 0.2 } else { 0.9 };
            c.personality.curiosity = 0.2;
        }
        // Whoever ends up being chased is decided by where the sprinter has got to, so the lively
        // watcher is whichever of the playful companions is left on the sidelines.
        let quiet = w.save.creatures[3].id;
        let mut audience = Audience::default();
        let mut started = false;
        let mut its = Vec::new();
        let mut swap_times = Vec::new();
        for step in 1..300 {
            tick(&mut w, &mut d, now, step);
            audience.note(&w);
            let Some(s) = w.attention.play.session else {
                if started {
                    assert!(everyone_settled(&w), "{audience:?}");
                    break;
                }
                continue;
            };
            assert_eq!(s.kind, Kind::Tag);
            started = true;
            assert!(s.swaps <= 3, "the role passes a bounded number of times");
            let it = s
                .with_role(PlayRole::Lead)
                .next()
                .map(|(_, id)| id)
                .expect("somebody is it");
            if its.last() != Some(&it) {
                its.push(it);
                swap_times.push(s.elapsed);
            }
        }
        assert!(started);
        assert!(its.len() >= 2, "the role is passed on: {its:?}");
        for pair in swap_times.windows(2) {
            assert!(
                pair[1] - pair[0] >= TAG_IMMUNITY,
                "no instant tag back: {swap_times:?}"
            );
        }
        assert!(w.attention.plans.is_empty());
        // Nobody on the sidelines is quietly made "it". Both watch the running, and the same
        // ending reads differently to each of them: the lively one cheers the last tag out loud,
        // the quiet one is only glad it is over.
        assert!(audience.coherent(), "{audience:?}");
        let watchers = audience.watchers();
        assert_eq!(watchers.len(), 2, "{audience:?}");
        let lively = watchers
            .into_iter()
            .find(|id| *id != quiet)
            .expect("one of the two watchers has an appetite for games");
        let looked = audience.looked_at(lively);
        assert!(looked.len() >= 6, "the gaze follows a runner: {looked:?}");
        assert!(
            audience
                .felt_by(lively)
                .contains(&AttentionEmotion::Enjoying)
                && audience.cheered.contains(&lively),
            "{audience:?}"
        );
        let felt = audience.felt_by(quiet);
        assert!(felt.contains(&AttentionEmotion::Relieved), "{felt:?}");
        assert!(!felt.contains(&AttentionEmotion::Enjoying), "{felt:?}");
        assert!(!audience.cheered.contains(&quiet), "{audience:?}");
    }

    #[test]
    fn a_jump_contest_gives_each_competitor_a_turn_with_the_others_watching() {
        let (mut world, mut desktop, now) = super::super::ledges::tests::edge_scene(true, true);
        world.clear_attention();
        world.window_journeys.clear();
        for (index, c) in world.save.creatures.iter_mut().enumerate() {
            c.state.action = ActionKind::Perch;
            c.state.action_duration = 100.0;
            c.personality.playfulness = 0.9;
            c.personality.curiosity = 0.5;
            c.state.drives.energy = 0.85;
            // Two competitors beside the same gap, the rest watching from further along.
            // Two competitors beside the gap, and two companions settled close enough behind
            // them to watch rather than wander off after each other.
            c.state.position.x = [770.0, 690.0, 620.0, 560.0][index];
            c.state.surface.relative_x = (c.state.position.x - 200.0) / 600.0;
            c.state.facing_right = true;
            if index >= 2 {
                c.personality.sociability = 0.2;
                c.state.drives.social_need = 0.0;
            }
        }
        world.attention.play.cooldown = 0.0;
        world.surface_memory.inspect_in = 100.0;
        let mut started = false;
        let mut performers = BTreeSet::new();
        let mut watched = false;
        for step in 3..500 {
            tick(&mut world, &mut desktop, now, step);
            let Some(s) = world.attention.play.session else {
                if started {
                    break;
                }
                continue;
            };
            assert_eq!(s.kind, Kind::JumpContest);
            started = true;
            assert!(s.swaps <= 2, "turns are bounded");
            for id in s.members.iter().flatten() {
                if world.window_journeys.contains_key(id) {
                    performers.insert(*id);
                    // Whoever is not in the air is watching the one who is. That is the
                    // competitor waiting its turn here: the two companions further along the
                    // ledge drift into errands of their own within a second or two of the
                    // contest starting, and a creature busy elsewhere is not recruited. Each
                    // turn is an ordinary gap attempt, so it gathers whatever audience is free
                    // — which is covered where that is what the scene is about, in
                    // `bold_gap_has_anticipation_airtime_and_an_outcome_audience`.
                    watched |= world.save.creatures.iter().any(|c| {
                        c.id != *id
                            && c.state.attention.is_some()
                            && c.state.action == ActionKind::InspectScreen
                    });
                }
            }
        }
        assert!(started && watched, "{started} {watched}");
        assert_eq!(performers.len(), 2, "both competitors take a turn");
        // A turn still in the air when the contest ends is allowed to land before everything is
        // expected to be put away; it is the landing that clears it, not the scene ending.
        for step in 500..900 {
            tick(&mut world, &mut desktop, now, step);
            if world.window_journeys.is_empty() {
                break;
            }
        }
        assert!(world.attention.plans.is_empty());
        assert!(world.window_journeys.is_empty());
    }

    /// One creature asleep, the rest awake beside it.
    fn sleeper_scene(playful: bool) -> (World, DesktopSnapshot, OffsetDateTime) {
        let (mut w, d, now) = scene(false);
        for (i, c) in w.save.creatures.iter_mut().enumerate() {
            c.personality.playfulness = if playful { 0.9 } else { 0.2 };
            c.personality.sociability = 0.9;
            c.personality.curiosity = 0.2;
            c.state.action_duration = 100.0;
            if i == 1 {
                c.state.action = ActionKind::Sleep;
                c.state.drives.sleep_pressure = if playful { 0.2 } else { 0.65 };
            } else {
                c.state.action = ActionKind::Perch;
            }
        }
        (w, d, now)
    }

    #[test]
    fn company_creeps_around_a_sleeping_companion_without_disturbing_it() {
        let (mut w, mut d, now) = sleeper_scene(false);
        let sleeper = w.save.creatures[1].id;
        let asleep_at = w.save.creatures[1].state.position;
        let mut started = false;
        let mut crept = false;
        let mut froze = false;
        for step in 1..300 {
            tick(&mut w, &mut d, now, step);
            let Some(s) = w.attention.play.session else {
                if started {
                    break;
                }
                continue;
            };
            assert_eq!(s.kind, Kind::Hush);
            started = true;
            assert_eq!(s.focus, Some(sleeper));
            assert!(
                !s.members.contains(&Some(sleeper)),
                "the sleeper is left alone"
            );
            // The sleeper keeps sleeping, exactly where it was.
            assert_eq!(w.save.creatures[1].state.action, ActionKind::Sleep);
            assert_eq!(w.save.creatures[1].state.position, asleep_at);
            crept |= w.save.creatures[0].state.action == ActionKind::Traverse;
            froze |= w.save.creatures[0].state.action == ActionKind::Perch
                && w.save.creatures[0].state.attention.is_some();
        }
        assert!(started && crept && froze, "{started} {crept} {froze}");
        assert!(!w.drain_events().any(|e| matches!(
            e,
            WorldEvent::SleepInterrupted { .. } | WorldEvent::CreatureWoke { .. }
        )));
    }

    #[test]
    fn a_playful_creature_can_pester_a_rested_sleeper_awake_but_not_an_exhausted_one() {
        for playful in [false, true] {
            let (mut w, mut d, now) = sleeper_scene(playful);
            let sleeper = w.save.creatures[1].id;
            let mut audience = Audience::default();
            let mut woke = false;
            let mut played = false;
            let mut attempts = 0;
            for step in 1..320 {
                tick(&mut w, &mut d, now, step);
                audience.note(&w);
                if let Some(s) = w.attention.play.session {
                    played = true;
                    attempts = attempts.max(s.swaps);
                    assert!(s.swaps <= 4, "attempts are capped");
                    // The companion this is all about is asleep, so it is neither a player nor
                    // part of the audience: nothing of it is posed while it sleeps.
                    let resting = w.save.creatures.iter().find(|c| c.id == sleeper).unwrap();
                    if resting.state.action == ActionKind::Sleep {
                        assert!(resting.state.attention.is_none());
                        assert!(!s.members.contains(&Some(sleeper)));
                    }
                } else if played {
                    played = false;
                    assert!(everyone_settled(&w), "{audience:?}");
                }
                woke |= w
                    .save
                    .creatures
                    .iter()
                    .any(|c| c.id == sleeper && c.state.action != ActionKind::Sleep);
            }
            assert_eq!(woke, playful, "only a rested sleeper is roused: {playful}");
            // Creeping around a sleeper, or pestering one awake, is a performance with an
            // audience: the two companions who keep out of it watch the one making the attempt,
            // and feel differently about it once it is over.
            assert!(audience.coherent(), "{audience:?}");
            let watchers = audience.watchers();
            assert_eq!(watchers.len(), 2, "{audience:?}");
            for id in watchers {
                assert!(!audience.looked_at(id).is_empty());
                let felt = audience.felt_by(id);
                assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
                assert!(felt.len() >= 2, "the attempt ends on them too: {felt:?}");
            }
            if playful {
                assert!(attempts >= 1);
                let events: Vec<_> = w.drain_events().collect();
                assert!(
                    events
                        .iter()
                        .any(|e| matches!(e, WorldEvent::SleepInterrupted { .. }))
                );
                assert!(
                    events
                        .iter()
                        .any(|e| matches!(e, WorldEvent::CreatureWoke { .. }))
                );
            }
        }
    }

    #[test]
    fn followers_walk_the_leaders_own_route_and_hesitate_where_they_cannot() {
        let (mut w, mut d, now) = following_scene(true);
        // Playful but not energetic: they copy the route instead of racing.
        for c in w.save.creatures.iter_mut() {
            c.state.drives.energy = 0.45;
        }
        let (follower, leader) = (w.save.creatures[0].id, w.save.creatures[1].id);
        let watcher = w.save.creatures[3].id;
        let mut audience = Audience::default();
        let mut started = false;
        let mut trailed = false;
        let mut followed = false;
        for step in 1..300 {
            tick(&mut w, &mut d, now, step);
            audience.note(&w);
            let Some(s) = w.attention.play.session else {
                if started {
                    assert!(everyone_settled(&w), "{audience:?}");
                    break;
                }
                continue;
            };
            assert_eq!(s.kind, Kind::FollowLeader);
            started = true;
            trailed |= s.trail.iter().flatten().count() >= 2;
            let (lead, follower) = (
                w.save.creatures[1].state.position.x,
                w.save.creatures[0].state.position.x,
            );
            // The follower walks the way the leader went, staying behind it.
            followed |= w.save.creatures[0].state.action == ActionKind::Traverse
                && (lead - follower).abs() < 200.0;
            // A route is only ever positions; nothing about it is written down.
            let saved = serde_json::to_string(&w.save).unwrap();
            assert!(!saved.contains("trail"));
        }
        assert!(
            started && trailed && followed,
            "{started} {trailed} {followed}"
        );
        // Watching this one means watching the front of it: after the opening tick the audience
        // is given the leader, and keeps its eyes on them the whole way along the ledge.
        assert!(audience.coherent(), "{audience:?}");
        assert_eq!(
            audience.followed[&watcher],
            vec![follower, leader],
            "{audience:?}"
        );
        let looked = audience.looked_at(watcher);
        assert!(looked.len() >= 4, "the gaze walks the route: {looked:?}");
        let felt = audience.felt_by(watcher);
        assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
        assert!(felt.len() >= 2, "and the route ends on them: {felt:?}");
    }

    #[test]
    fn the_best_spot_on_a_ledge_changes_hands_by_stepping_aside() {
        let (mut w, mut d, now) = scene(false);
        // Window 701 spans 200..800, so its middle is 500.
        for (i, c) in w.save.creatures.iter_mut().enumerate() {
            c.state.action = ActionKind::Perch;
            c.state.action_duration = 100.0;
            c.personality.playfulness = 0.8;
            c.personality.boldness = 0.8;
            c.personality.curiosity = 0.2;
            c.state.position.x = [640.0, 500.0, 340.0, 280.0][i];
            c.state.surface.relative_x = (c.state.position.x - 200.0) / 600.0;
        }
        let holder = w.save.creatures[1].id;
        let mut audience = Audience::default();
        let mut started = false;
        let mut kings = Vec::new();
        let mut stepped_aside = false;
        for step in 1..320 {
            tick(&mut w, &mut d, now, step);
            audience.note(&w);
            let Some(s) = w.attention.play.session else {
                if started {
                    assert!(everyone_settled(&w), "{audience:?}");
                    break;
                }
                continue;
            };
            assert_eq!(s.kind, Kind::Hill);
            started = true;
            assert!(
                s.swaps <= 2,
                "the spot changes hands a bounded number of times"
            );
            let king = s.with_role(PlayRole::Lead).next().map(|(_, id)| id);
            if kings.last() != king.as_ref() {
                kings.extend(king);
            }
            // Yielding is walking aside, never shoving: nobody is pushed off the ledge.
            for c in w.save.creatures.iter().take(2) {
                assert_eq!(c.state.surface.window_key, Some(701));
                assert_eq!(c.state.position.y, 600.0);
            }
            stepped_aside |= kings.len() > 1
                && w.save
                    .creatures
                    .iter()
                    .take(2)
                    .any(|c| c.state.action == ActionKind::Traverse && Some(c.id) != king);
        }
        assert!(started, "a contest begins");
        assert_eq!(
            kings.first().copied(),
            Some(holder),
            "the occupant starts on it"
        );
        assert!(kings.len() >= 2, "the spot is taken: {kings:?}");
        assert!(stepped_aside, "the previous occupant walks aside");
        // The contest is played to the two companions further down the ledge. Their eyes stay on
        // the companion standing in the middle of it — the one with something to lose, who barely
        // moves while holding the spot — and they are pleased enough with the business of it
        // changing hands to say so at the end.
        assert!(audience.coherent(), "{audience:?}");
        let watchers = audience.watchers();
        assert_eq!(watchers.len(), 2, "{audience:?}");
        for id in watchers {
            let looked = audience.looked_at(id);
            // The spot is in the middle of the ledge, so the gaze stays about there — but it
            // goes with whoever is standing on it, including the moment they step aside.
            assert!(
                looked.iter().all(|x| (x - 500).abs() <= 60),
                "the gaze stays about the middle of the ledge: {looked:?}"
            );
            assert!(
                looked.iter().any(|x| (x - 500).abs() > 12),
                "and moves when the spot changes hands: {looked:?}"
            );
            let felt = audience.felt_by(id);
            assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
            assert!(felt.contains(&AttentionEmotion::Enjoying), "{felt:?}");
            assert!(audience.cheered.contains(&id), "{audience:?}");
        }
    }

    #[test]
    fn reduced_motion_never_starts_a_travelling_game() {
        for reduced in [false, true] {
            let (mut w, mut d, now) = following_scene(true);
            w.save.settings.reduce_motion = reduced;
            let mut travelled = false;
            for step in 1..120 {
                tick(&mut w, &mut d, now, step);
                travelled |= session_kind(&w).is_some_and(Kind::travels);
            }
            assert_eq!(travelled, !reduced);
        }
    }
}
