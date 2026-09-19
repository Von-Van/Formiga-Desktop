use super::*;

impl World {
    /// Move to the routine the schedule intends, if that is not already where we are. Only the
    /// current intended state is applied: a machine that slept through a week of transitions
    /// wakes into today's routine, not through every one it missed. Visibility, pause, and the
    /// separate quiet expiry are never touched.
    pub(super) fn apply_routine_schedule(&mut self, now: OffsetDateTime) {
        if !self.save.companion.schedule.enabled {
            return;
        }
        let local = local_time_or_utc(now);
        let Some(intended) = self.save.companion.schedule.intended(local) else {
            return;
        };
        if self.save.companion.schedule.applied == Some(intended) {
            return;
        }
        let Some(preset) = self
            .save
            .companion
            .modes
            .get(usize::from(intended))
            .and_then(Option::as_ref)
            .filter(|preset| habitat_usable(&preset.habitat))
            .cloned()
        else {
            // The routine it wants has never been saved, or the habitat it saved no longer leaves
            // anywhere to stand: leave the settings exactly as they are and try again at the next
            // transition rather than reporting a change nobody asked for.
            return;
        };
        // A scheduled transition is the moment a manual override stops applying.
        self.save.companion.schedule.overridden = false;
        self.save.companion.schedule.applied = Some(intended);
        preset.apply(&mut self.save.settings);
        self.save.settings.habitat.zones.truncate(MAX_HABITAT_ZONES);
    }

    /// The user has chosen a routine by hand. It holds until the next scheduled transition.
    pub fn override_routine(&mut self) {
        self.save.companion.schedule.overridden = true;
    }

    /// Hand the routine back to the schedule, which takes effect at once.
    pub fn resume_routine(&mut self, now: OffsetDateTime) {
        self.save.companion.schedule.overridden = false;
        self.save.companion.schedule.applied = None;
        self.apply_routine_schedule(now);
    }

    /// Uses the existing home path and tick; never alters the user's behavior settings.
    pub fn set_quiet_mode(&mut self, minutes: u16, now: OffsetDateTime) {
        if minutes == 0 {
            self.save.companion.quiet_until = None;
            self.dismiss_home(now, false);
            return;
        }
        self.interrupt_colony_plan(now);
        self.save.companion.quiet_until =
            Some(now + Duration::minutes(i64::from(minutes.min(120))));
        self.save.home.active_since_utc = Some(now);
        self.window_journeys.clear();
        self.window_routes.clear();
        self.bond_plans.clear();
        self.action_choices.clear();
    }
}

/// A saved habitat is usable when it is a preset, or when it still names at least one place a
/// creature is allowed to be. An empty custom policy would strand the colony.
fn habitat_usable(policy: &HabitatPolicy) -> bool {
    policy.preset != HabitatPreset::Custom
        || policy
            .zones
            .iter()
            .any(|zone| zone.enabled && zone.kind == HabitatZoneKind::Allowed)
}
