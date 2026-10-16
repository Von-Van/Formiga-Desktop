//! What every companion app's visit has in common on the app's side: the item each one offers in
//! the tray, and the rule that a creature is away in one app at a time.

use super::*;
use crate::battle::BATTLE;
use crate::expansion::{Expansion, Holding};
use crate::hill::HILL;
use crate::house::HOME;

/// Each companion app's place in the tray, in the order the tray lists them.
pub(super) const HILL_ITEM: usize = 0;
pub(super) const HOME_ITEM: usize = 1;
pub(super) const BATTLE_ITEM: usize = 2;
/// How many companion apps the tray has items for.
pub(super) const VISIT_ITEMS: usize = 3;

impl FormigaApp {
    /// Who each companion app has just now.
    fn holdings(&self) -> [(&'static Expansion, Holding); VISIT_ITEMS] {
        let everyone = |away: bool| {
            if away {
                Holding::Everyone
            } else {
                Holding::Nobody
            }
        };
        let hill = everyone(self.hill.trip.holds_world());
        let home = self
            .house
            .visit
            .open()
            .map_or(Holding::Nobody, |open| Holding::These(open.away.clone()));
        // The whole colony rides to Battle, as to Hill.
        let battle = everyone(self.battle.session.holds_world());
        [(&HILL, hill), (&HOME, home), (&BATTLE, battle)]
    }

    /// Why a visit to `asking` cannot have `wants` (everyone, when `None`) just now, in the words of
    /// the app that has some of them.
    pub(super) fn busy_elsewhere(
        &self,
        asking: &Expansion,
        wants: Option<&[CreatureId]>,
    ) -> Option<&'static str> {
        crate::expansion::busy_elsewhere(asking, wants, &self.holdings())
    }

    /// The app `id` is away in just now, if any.
    pub(super) fn away_in(&self, id: CreatureId) -> Option<&'static Expansion> {
        self.holdings()
            .into_iter()
            .find(|(_, holding)| holding.holds_any(Some(&[id])))
            .map(|(app, _)| app)
    }

    /// Do what a companion app's tray item offers.
    pub(super) fn choose_visit_item(&mut self, item: usize) {
        match item {
            HILL_ITEM => self.choose_hill_item(),
            HOME_ITEM => self.bring_household_back(),
            BATTLE_ITEM => self.play_battle(),
            _ => {}
        }
    }
}
