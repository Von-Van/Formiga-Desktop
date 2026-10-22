//! The companion apps as one list: everything the app does for each of them at the same moments
//! (as it starts, on a tick, before a visit, when the colony is replaced, and when the tray is
//! brought up to date) goes through [`Companion`], whose every `match` the compiler holds to the
//! whole list. A fifth app is added here, and the compiler names each place it needs a word.

use super::*;
use crate::battle::BATTLE;
use crate::expansion::{Expansion, Holding, Readiness, Slot, Takes};
use crate::farm::FARM;
use crate::hill::HILL;
use crate::house::HOME;
use crate::platform::companion_app::AppInstall;

/// One of the companion apps Desktop visits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Companion {
    Hill,
    Home,
    Farm,
    Battle,
}

impl Companion {
    /// Every companion app, in the order Desktop looks for them and finishes their visits.
    pub(super) const ALL: [Self; 4] = [Self::Hill, Self::Home, Self::Farm, Self::Battle];

    pub(super) fn expansion(self) -> &'static Expansion {
        match self {
            Self::Hill => &HILL,
            Self::Home => &HOME,
            Self::Farm => &FARM,
            Self::Battle => &BATTLE,
        }
    }
}

impl FormigaApp {
    fn slot_mut(&mut self, app: Companion) -> &mut Slot {
        match app {
            Companion::Hill => &mut self.hill.slot,
            Companion::Home => &mut self.house.slot,
            Companion::Farm => &mut self.farm.slot,
            Companion::Battle => &mut self.battle.slot,
        }
    }

    /// Whom `app` has just now, as far as any other visit is concerned.
    pub(super) fn holding(&self, app: Companion) -> Holding {
        let everyone = |away: bool| {
            if away {
                Holding::Everyone
            } else {
                Holding::Nobody
            }
        };
        match app {
            Companion::Hill => everyone(self.hill.trip.holds_world()),
            Companion::Home => self
                .house
                .visit
                .open()
                .map_or(Holding::Nobody, |open| Holding::These(open.away.clone())),
            // Farm has a copy of a companion's look, never the companion.
            Companion::Farm => Holding::Nobody,
            // The whole colony rides to Battle, as to Hill.
            Companion::Battle => everyone(self.battle.session.holds_world()),
        }
    }

    /// Look for `app` if it is time to (or now), and start a development run's visit when due.
    fn check_for(&mut self, app: Companion, now: bool) {
        match app {
            Companion::Hill => self.check_for_hill(now),
            Companion::Home => self.check_for_home(now),
            Companion::Farm => self.check_for_farm(now),
            Companion::Battle => self.check_for_battle(now),
        }
    }

    /// Look for every companion app whose time it is.
    pub(super) fn check_for_companions(&mut self) {
        for app in Companion::ALL {
            self.check_for(app, false);
        }
    }

    /// As Desktop starts: look for each app now, and finish whatever visit to it a stop in the
    /// middle left open.
    pub(super) fn start_companions(&mut self) {
        for app in Companion::ALL {
            self.check_for(app, true);
            match app {
                Companion::Hill => self.resume_open_trip(),
                Companion::Home => self.resume_open_visit(),
                Companion::Farm => self.resume_farm_session(),
                Companion::Battle => self.resume_battle_session(),
            }
        }
    }

    /// The colony is about to be replaced, by a reset or a restored backup: whatever an app goes
    /// on to say belongs to the colony it was given, so none of it is read.
    pub(super) fn abandon_visits(&mut self) {
        for app in Companion::ALL {
            match app {
                // A trip holds the colony, and the colony is never replaced while it is away.
                Companion::Hill => {}
                Companion::Home => self.abandon_house_visit(),
                Companion::Farm => self.abandon_farm_session(),
                Companion::Battle => self.abandon_battle_session(),
            }
        }
    }

    /// Bring each app's tray item, and the tray's word on where the colony is, up to date.
    pub(super) fn sync_visit_menus(&mut self) {
        for app in Companion::ALL {
            match app {
                Companion::Hill => self.sync_hill_menu(),
                Companion::Home => self.sync_house_menu(),
                // Farm is offered in the notebook, not the tray.
                Companion::Farm => {}
                Companion::Battle => self.sync_battle_menu(),
            }
        }
    }

    /// The installed `app`, once nothing stands in the way of a visit that takes `takes` to it;
    /// otherwise the reason is said under the app's own title and nothing is returned. The app is
    /// looked for again, since it may have been removed since the last look.
    pub(super) fn ready_for(&mut self, app: Companion, takes: Takes) -> Option<AppInstall> {
        let expansion = app.expansion();
        let busy = takes
            .wants()
            .and_then(|wants| self.busy_elsewhere(expansion, wants));
        let readiness = Readiness {
            recovering: self.recovery_pending,
            save_trouble: self.save_trouble.as_deref(),
            busy,
        };
        if let Some(reason) = expansion.blocked(&readiness) {
            self.failure_dialog(expansion.words.staying, &reason);
            return None;
        }
        let slot = self.slot_mut(app);
        let had = slot.install.clone();
        slot.look(true);
        let install = slot.install.clone();
        if install != had {
            self.sync_visit_menus();
        }
        if install.is_none() {
            self.failure_dialog(expansion.words.staying, &expansion.gone());
        }
        install
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_app_is_on_the_list_once_and_speaks_for_itself() {
        let names: Vec<&str> = Companion::ALL
            .iter()
            .map(|app| app.expansion().name)
            .collect();
        assert_eq!(
            names,
            [
                "Formiga Hill",
                "Formiga Home",
                "Formiga Farm",
                "Formiga Battle"
            ]
        );
        for app in Companion::ALL {
            let words = &app.expansion().words;
            assert!(!words.staying.is_empty() && !words.recover_first.is_empty());
            assert!(words.recover_first.ends_with('.'));
        }
        // Only Farm takes nobody away, so only Farm opens while the colony cannot be saved.
        let without_save: Vec<_> = Companion::ALL
            .into_iter()
            .filter(|app| app.expansion().words.cannot_save.is_none())
            .collect();
        assert_eq!(without_save, [Companion::Farm]);
    }
}
