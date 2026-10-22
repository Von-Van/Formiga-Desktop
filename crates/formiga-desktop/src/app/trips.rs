//! What the app shows and holds while the whole colony is away by train, whichever app it left
//! for: a trip to Formiga Hill or a match in Formiga Battle. The train is the same for both
//! ([`crate::trip`]); each app's own side is in `hill.rs` and `battle.rs`.

use super::*;
use crate::battle::BATTLE;
use crate::expansion::Expansion;
use crate::hill::{HILL, scene::TrainScene};

impl FormigaApp {
    /// The app the whole colony is in, or leaving for or coming home from, if any.
    pub(super) fn colony_away_in(&self) -> Option<&'static Expansion> {
        if self.hill.trip.holds_world() {
            Some(&HILL)
        } else if self.battle.session.holds_world() {
            Some(&BATTLE)
        } else {
            None
        }
    }

    /// Whether the colony is not simply living on the desktop: the world is held still and the
    /// overlay shows only the train, or nothing.
    pub(super) fn colony_away(&self) -> bool {
        self.colony_away_in().is_some()
    }

    /// The train on the desktop just now, if one is.
    pub(super) fn train_scene(&self) -> Option<&TrainScene> {
        self.hill
            .trip
            .scene()
            .or_else(|| self.battle.session.scene())
    }

    /// One tick of whichever trip has the colony, in place of the world's own.
    pub(super) fn tick_trips(&mut self, dt: f32) {
        self.tick_trip(dt);
        self.tick_battle_trip(dt);
    }

    /// The train that brings the colony home: the one on the desktop now, turned back, or a new
    /// one pulling in. `None` when there is nobody to see it, and the colony is simply home.
    pub(super) fn train_home(&self, current: Option<&TrainScene>) -> Option<TrainScene> {
        let world = self.world.as_ref()?;
        match current {
            Some(scene) => scene.turn_back(&world.save, &self.monitors),
            None => TrainScene::arrival(&world.save, &self.monitors),
        }
    }

    /// The train has moved: every overlay showing it draws again.
    pub(super) fn redraw_train(&self) {
        for overlay in self.overlays.values() {
            if overlay.is_visible() {
                overlay.window.request_redraw();
            }
        }
    }

    /// Whatever is shown of the colony is now the trip's: the hit-test windows go, the overlays
    /// show only the station, and the tray says where everyone is.
    pub(super) fn enter_trip(&mut self) {
        for proxy in self.interaction_proxies.values_mut() {
            proxy.hide();
        }
        for proxy in self.house_proxies.values_mut() {
            proxy.hide();
        }
        self.sync_overlay_visibility();
        self.request_overlay_redraw();
        self.sync_visit_menus();
    }

    /// The colony is back to living on the desktop.
    pub(super) fn back_on_desktop(&mut self) {
        // The world picks up where it left off, as it does on opening, rather than with one long
        // tick for the time it was away.
        self.last_tick = Instant::now();
        self.sync_overlay_visibility();
        self.request_overlay_redraw();
        self.sync_visit_menus();
    }

    /// The colony as the overlay draws it this frame, while a trip has it: the platform's copy
    /// during a scene, or nobody while it is away.
    pub(super) fn trip_stage(&self, save: &SaveFile) -> Option<(SaveFile, Option<TrainView>)> {
        if !self.colony_away() {
            return None;
        }
        Some(match self.train_scene() {
            Some(scene) => (
                scene.stage(save),
                scene.train().map(|pose| TrainView {
                    pose,
                    look: formiga_art::TrainLook { lit: self.night },
                }),
            ),
            None => {
                let mut empty = save.clone();
                empty.creatures.clear();
                (empty, None)
            }
        })
    }

    /// Which displays show anything while a trip has the colony: only the one the train stops at,
    /// during a scene, and none while the colony is away.
    pub(super) fn trip_display(&self) -> Option<Option<formiga_core::MonitorId>> {
        self.colony_away()
            .then(|| self.train_scene().map(TrainScene::monitor_id))
    }
}
