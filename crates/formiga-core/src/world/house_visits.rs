//! A house open in Formiga Home.
//!
//! While a house is open, the household that lives there and any friend lent for the visit are
//! away from the desktop: indoors, out of every plan, and left alone by the world, which goes on
//! as usual for everybody else. When the house closes they come back out where they went in. What
//! went on inside, and what of it is kept, is the desktop host's business: nothing here reads
//! another app's words.

use super::*;

impl World {
    /// Lend `ids` to a house opened elsewhere. Each lets go of whatever it was in the middle of — a
    /// plan about the village, a wonder, a climb, a ride, a toss — and goes indoors, where the world
    /// leaves it until [`World::end_house_visit`]. Anyone caught between two places is set down on
    /// the floor first, so it comes back out somewhere it can stand. Only colony members who have
    /// arrived and are not being held can go, and there is only ever one visit: one still open is
    /// closed first. Returns who went.
    pub fn begin_house_visit(
        &mut self,
        ids: &[CreatureId],
        desktop: &DesktopSnapshot,
    ) -> Vec<CreatureId> {
        self.end_house_visit();
        let policy = self.save.settings.habitat.clone();
        let display_scale = self.save.settings.display_scale;
        let mut lent = Vec::new();
        for &id in ids {
            let held = self
                .interaction
                .as_ref()
                .is_some_and(|interaction| interaction.creature_id == id);
            let Some(index) =
                self.save.creatures.iter().position(|creature| {
                    creature.id == id && creature.state.arrival_delay_secs <= 0.0
                })
            else {
                continue;
            };
            if held || lent.contains(&id) {
                continue;
            }
            self.drop_village_activity(id);
            let at_x = self.save.creatures[index].state.position.x;
            let off_a_wonder = self.wonders.grab(id, at_x);
            let mid_journey = self.window_journeys.remove(&id).is_some();
            let toss = self.tosses.remove(&id);
            self.cancel_creature_attention(id);
            self.window_routes.remove(&id);
            self.action_choices.remove(&id);
            self.bond_plans.remove(&id);
            self.home_moments.remove(&id);
            self.home_roam.remove(&id);
            self.pending_home_greetings.remove(&id);
            self.forget_tows_of(id);
            self.bubbles.retain(|bubble| bubble.creature_id != id);
            let creature = &mut self.save.creatures[index];
            if let Some((ground_y, surface)) = off_a_wonder {
                creature.state.position.y = ground_y;
                creature.state.surface = surface;
            }
            if let Some(toss) = toss {
                creature.state.position = toss.last_safe_position;
                creature.state.surface = toss.last_safe_surface;
            } else if mid_journey
                && let Some((monitor_id, position)) = crate::nearest_habitat_point(
                    &policy,
                    &desktop.monitors,
                    creature.state.position,
                    display_scale,
                )
            {
                creature.state.position = position;
                creature.state.surface = SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id,
                    window_key: None,
                    relative_x: 0.5,
                };
            }
            let state = &mut creature.state;
            state.action = ActionKind::Idle;
            state.action_elapsed = 0.0;
            state.action_duration = 2.5;
            state.velocity = Point::default();
            state.activity_variant = 0;
            state.attention = None;
            state.flourish = None;
            state.nudge = None;
            state.beat = None;
            state.indoors = true;
            lent.push(id);
        }
        self.house_visit.extend(lent.iter().copied());
        lent
    }

    /// Whether this companion is away in a house open elsewhere.
    pub fn away_in_a_house(&self, id: CreatureId) -> bool {
        self.house_visit.contains(&id)
    }

    /// Whether this colony member could go into a house just now: it has arrived, is not in the
    /// owner's hand, and is not away in one already. The same test [`World::begin_house_visit`]
    /// makes, asked before anyone is written into a visit.
    pub fn free_to_go_in(&self, id: CreatureId) -> bool {
        !self.house_visit.contains(&id)
            && !self
                .interaction
                .as_ref()
                .is_some_and(|interaction| interaction.creature_id == id)
            && self
                .save
                .creatures
                .iter()
                .any(|creature| creature.id == id && creature.state.arrival_delay_secs <= 0.0)
    }

    /// The house has closed: everyone lent to it comes back out where it went in and carries on
    /// from standing still. Anyone who has left the colony meanwhile is simply gone. Returns
    /// whether anybody was away.
    pub fn end_house_visit(&mut self) -> bool {
        if self.house_visit.is_empty() {
            return false;
        }
        for id in std::mem::take(&mut self.house_visit) {
            if let Some(creature) = creature_mut(&mut self.save.creatures, id) {
                let state = &mut creature.state;
                state.indoors = false;
                state.action = ActionKind::Idle;
                state.action_elapsed = 0.0;
                state.action_duration = 2.5;
                state.velocity = Point::default();
            }
        }
        true
    }

    /// The owner spent a while inside `keeper`'s house, and the host has checked it: written in
    /// the journal in Desktop's own words, once however often the same visit is read again, and
    /// once for a few visits to the same house close together. Returns whether it was written.
    pub fn note_house_visit(&mut self, keeper: CreatureId, now: OffsetDateTime) -> bool {
        if !self
            .save
            .creatures
            .iter()
            .any(|creature| creature.id == keeper)
        {
            return false;
        }
        let before = self.save.companion.journal.last().cloned();
        self.save
            .companion
            .remember(Some(keeper), crate::JournalMoment::HouseVisit, now);
        self.save.companion.journal.last() != before.as_ref()
    }
}
