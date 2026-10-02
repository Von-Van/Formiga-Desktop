//! The colony going away on the train and coming home.
//!
//! The world knows only the two ends of a trip. Before the colony leaves, everything anybody is in
//! the middle of is let go, so the colony leaves as it would on quitting and comes back as it would
//! on opening. When it is home again, the trip is counted and the journal says so, once. Where
//! the colony went, how it was started, and what was checked on its way home are the desktop
//! host's business: nothing here reads another app's words.

use super::*;

impl World {
    /// Settle the colony to leave on a trip. A drag or a toss is let go, every plan, scene, game,
    /// offer and bubble is dropped, a guest who is visiting goes on its way (signing the guest
    /// book if it got as far as saying hello), and everybody is out of doors, standing still
    /// where they are. Nothing is moved: the trip's own presentation does that, and none of it is
    /// kept.
    pub fn prepare_for_trip(&mut self, now: OffsetDateTime) {
        if self.interaction.is_some() {
            self.handle_command(WorldCommand::CancelInteraction, &DesktopSnapshot::default());
        }
        self.clear_runtime_plans();
        self.pending_home_greetings.clear();
        self.offers.clear();
        self.bubbles.clear();
        self.sign_if_visited(now);
        self.close_visit();
        for creature in &mut self.save.creatures {
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
            state.indoors = false;
        }
    }

    /// Whether the trip with this identifier has already been counted.
    pub fn trip_counted(&self, session: &str) -> bool {
        self.save
            .trips
            .last
            .as_ref()
            .is_some_and(|trip| trip.session == session)
    }

    /// The colony is home from a trip the host has checked: count it, keep it as the last trip,
    /// and write it in the journal. A trip already counted, or one that is not written as a trip,
    /// changes nothing. Returns whether anything changed.
    pub fn welcome_home(&mut self, trip: Trip, now: OffsetDateTime) -> bool {
        if !Trip::is_session(&trip.session)
            || trip.arrived_at_utc > trip.left_at_utc
            || self.trip_counted(&trip.session)
        {
            return false;
        }
        self.save.trips.count = self.save.trips.count.saturating_add(1);
        self.save.trips.last = Some(trip);
        self.save
            .companion
            .remember(None, crate::JournalMoment::Trip, now);
        true
    }
}
