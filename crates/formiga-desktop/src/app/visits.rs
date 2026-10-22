//! What every companion app's visit has in common on the app's side: the item each one offers in
//! the tray, and the rule that a creature is away in one app at a time.

use super::*;
use crate::expansion::{Expansion, Holding};
use companions::Companion;

/// Each companion app's place in the tray, in the order the tray lists them.
pub(super) const HILL_ITEM: usize = 0;
pub(super) const HOME_ITEM: usize = 1;
pub(super) const BATTLE_ITEM: usize = 2;
/// How many companion apps the tray has items for.
pub(super) const VISIT_ITEMS: usize = 3;

impl FormigaApp {
    /// Who each companion app has just now.
    fn holdings(&self) -> Vec<(&'static Expansion, Holding)> {
        Companion::ALL
            .into_iter()
            .map(|app| (app.expansion(), self.holding(app)))
            .collect()
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
