//! The app's side of a trip to Formiga Hill: offering it in the tray only while Hill is installed,
//! sending the colony, holding the world still while it is away, and bringing it home whatever
//! happens. The states and the guarantees are described in [`crate::hill`].

use super::*;
use crate::hill::session::{self, Answer, TravelFiles};
use crate::hill::{HillEvent, HomecomingNote, TripState, scene::TrainScene};
use crate::platform::hill::{HillInstall, find_formiga_hill, launch_formiga_hill};
use crate::tray::HillMenu;
use formiga_travel::{RecallReason, SessionId, project_colony};

/// How often Desktop looks again for Hill having been installed or removed.
const HILL_RECHECK: Duration = Duration::from_secs(10 * 60);

/// The title every reason for not leaving is shown under.
const STAYING_HOME: &str = "The colony is staying home";

/// For development only: with [`formiga_travel::discovery::PATH_OVERRIDE_ENV`] naming a stand-in
/// Hill, the colony leaves this many seconds after Desktop starts, once, as if the tray had been
/// used. Ignored without the override, so it can never send a colony to an installed Hill.
const TRIP_AFTER_ENV: &str = "FORMIGA_HILL_TRIP_AFTER";

/// The trip's own state, files and Hill, as the app keeps them.
pub(super) struct HillLink {
    pub(super) trip: TripState,
    pub(super) files: TravelFiles,
    pub(super) install: Option<HillInstall>,
    checked: Option<Instant>,
    /// When a development run sends the colony by itself; see [`TRIP_AFTER_ENV`].
    trip_at: Option<Instant>,
}

impl HillLink {
    pub(super) fn new(data_dir: &std::path::Path) -> Self {
        let trip_at = std::env::var_os(formiga_travel::discovery::PATH_OVERRIDE_ENV)
            .and(std::env::var(TRIP_AFTER_ENV).ok())
            .and_then(|seconds| seconds.parse::<f32>().ok())
            .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
            .map(|seconds| Instant::now() + Duration::from_secs_f32(seconds));
        Self {
            trip: TripState::Idle,
            files: TravelFiles::new(data_dir),
            install: None,
            checked: None,
            trip_at,
        }
    }
}

impl FormigaApp {
    /// Look for Hill if it is time to, and offer or withdraw the trip in the tray. Only while the
    /// colony is home: a trip under way keeps the Hill it started with.
    pub(super) fn check_for_hill(&mut self, now: bool) {
        if self.hill.trip.holds_world() {
            return;
        }
        if self
            .hill
            .trip_at
            .is_some_and(|at| Instant::now() >= at && !self.recovery_pending)
        {
            self.hill.trip_at = None;
            tracing::info!("sending the colony to the stand-in Hill, as {TRIP_AFTER_ENV} asks");
            self.go_to_hill();
            return;
        }
        if !now
            && self
                .hill
                .checked
                .is_some_and(|checked| checked.elapsed() < HILL_RECHECK)
        {
            return;
        }
        let found = find_formiga_hill();
        if found != self.hill.install {
            match &found {
                Some(install) => tracing::info!(
                    version = install.version.as_deref().unwrap_or("unknown"),
                    "Formiga Hill is installed"
                ),
                None => tracing::info!("Formiga Hill is not installed"),
            }
        }
        self.hill.install = found;
        self.hill.checked = Some(Instant::now());
        self.sync_hill_menu();
    }

    pub(super) fn sync_hill_menu(&mut self) {
        let mode = match &self.hill.trip {
            TripState::Idle if self.hill.install.is_some() => HillMenu::Go,
            TripState::Idle => HillMenu::Hidden,
            TripState::Departing { .. } | TripState::Away { .. } => HillMenu::Away,
            TripState::Returning { .. } => HillMenu::Returning,
        };
        if let Some(tray) = &mut self.tray {
            tray.sync_hill(mode);
        }
    }

    /// Send the colony to Hill. Every reason it cannot go is said, and leaves the colony home
    /// exactly as it was.
    pub(super) fn go_to_hill(&mut self) {
        if self.hill.trip.holds_world() || self.world.is_none() {
            return;
        }
        if self.recovery_pending {
            self.failure_dialog(
                STAYING_HOME,
                "Finish recovering the colony in the notebook before it travels.",
            );
            return;
        }
        if let Some(trouble) = self.save_trouble.clone() {
            self.failure_dialog(
                STAYING_HOME,
                &format!("The colony cannot be saved just now, so it is not leaving. {trouble}"),
            );
            return;
        }
        if self.house.visit.open().is_some() {
            self.failure_dialog(
                STAYING_HOME,
                "A house is open in Formiga Home. Bring its household back first, and the whole \
                 colony can travel.",
            );
            return;
        }
        // Asked again now: it may have been removed since the last look.
        self.check_for_hill(true);
        let Some(install) = self.hill.install.clone() else {
            self.failure_dialog(STAYING_HOME, "Formiga Hill is no longer installed.");
            return;
        };
        // The travel version this colony needs: newer only if someone in it is drawn in a way an
        // older Hill cannot draw.
        let needs = self
            .world
            .as_ref()
            .map_or(1, |world| formiga_travel::reader_for_colony(&world.save));
        if let Some(reason) = install.incompatibility(needs) {
            self.failure_dialog(STAYING_HOME, &reason);
            return;
        }
        self.finish_habitat_editor(false);
        self.close_creature_menu(MenuDismissal::Hidden);
        let now = OffsetDateTime::now_utc();
        if let Some(world) = &mut self.world {
            world.prepare_for_trip(now);
        }
        // The colony is written as it leaves, so a crash anywhere after this loses nothing.
        if let Err(error) = self.save() {
            self.failure_dialog(
                STAYING_HOME,
                &format!("The colony could not be saved, so it is not leaving: {error}"),
            );
            return;
        }
        let Some(world) = &self.world else { return };
        let snapshot = SessionId::generate()
            .map_err(|error| error.to_string())
            .and_then(|session| {
                project_colony(&world.save, session, now, env!("CARGO_PKG_VERSION"))
                    .map_err(|error| error.to_string())
            });
        let snapshot = match snapshot {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.failure_dialog(STAYING_HOME, &format!("The colony could not pack: {error}"));
                return;
            }
        };
        let open = match self.hill.files.open(&snapshot) {
            Ok(open) => open,
            Err(error) => {
                self.failure_dialog(
                    STAYING_HOME,
                    &format!("The colony's ticket could not be written: {error}"),
                );
                return;
            }
        };
        tracing::info!(
            travelers = snapshot.travelers.len(),
            "the colony is leaving for Formiga Hill"
        );
        // With the train to watch, Hill opens as it pulls away; with nothing on the desktop to
        // see, at once.
        match TrainScene::departure(&world.save, &self.monitors) {
            Some(scene) => {
                self.hill.trip = TripState::Departing {
                    open,
                    scene,
                    hill: Some(install),
                };
            }
            None => {
                if let Err(error) = self.launch_hill(&install, &open) {
                    self.hill.files.close(&open);
                    self.failure_dialog(STAYING_HOME, &error);
                    return;
                }
                self.hill.trip = TripState::Away { open };
            }
        }
        self.enter_trip();
    }

    /// Start Hill for this trip, with a thread waiting for it to exit.
    fn launch_hill(&self, install: &HillInstall, open: &session::OpenTrip) -> Result<(), String> {
        launch_formiga_hill(install, &open.dir)
            .and_then(|child| {
                crate::hill::watch(
                    child,
                    open.seal.session_id.clone(),
                    self.event_proxy.clone(),
                )
            })
            .map_err(|error| format!("Formiga Hill could not be opened: {error}"))?;
        tracing::info!("the colony left for Formiga Hill");
        Ok(())
    }

    /// The owner asked for the colony back before Hill was done with it.
    pub(super) fn bring_colony_home(&mut self) {
        let Some(open) = self.hill.trip.open().cloned() else {
            return;
        };
        let started = !matches!(self.hill.trip, TripState::Departing { hill: Some(_), .. });
        if started {
            self.hill
                .files
                .call_home(&open, RecallReason::OwnerAsked, OffsetDateTime::now_utc());
        } else {
            // Still boarding, and Hill not yet started: there is nobody to tell.
            self.hill.files.close(&open);
        }
        tracing::info!("the colony was called home from Formiga Hill");
        self.come_home(None);
    }

    pub(super) fn handle_hill_event(&mut self, event: HillEvent) {
        let HillEvent::Exited { session } = event;
        let Some(open) = self
            .hill
            .trip
            .open()
            .filter(|open| open.seal.session_id == session)
            .cloned()
        else {
            // A trip that is already over: the colony is home, and whatever Hill left is not read.
            return;
        };
        let departing = matches!(self.hill.trip, TripState::Departing { .. });
        let note = self.read_homecoming(&open);
        self.hill.files.close(&open);
        self.come_home(note.map(|text| HomecomingNote {
            text,
            asked_for: departing,
        }));
    }

    /// What Hill left for this trip, applied as far as Desktop allows, and what to tell the owner
    /// if the colony came home without it.
    fn read_homecoming(&mut self, open: &session::OpenTrip) -> Option<String> {
        let now = OffsetDateTime::now_utc();
        match self.hill.files.answer(open) {
            Answer::Receipt(receipt) => {
                let welcome = session::welcome(open, &receipt, now);
                if !welcome.set_aside.is_empty() {
                    tracing::info!(set_aside = ?welcome.set_aside, "receipt effects set aside");
                }
                if let Some(world) = &mut self.world {
                    let counted = welcome
                        .trip
                        .is_some_and(|trip| world.welcome_home(trip, now));
                    let kept = world.keep_souvenirs(&welcome.souvenirs, now);
                    if counted || kept {
                        self.save_waiting = SaveUrgency::Prompt;
                    }
                }
                tracing::info!(
                    souvenirs = welcome.souvenirs.len(),
                    "the colony came home from Formiga Hill"
                );
                None
            }
            Answer::Silent { refusal, problem } => {
                if let Some(problem) = &problem {
                    tracing::warn!(%problem, "Formiga Hill's receipt was not used");
                }
                tracing::info!(
                    refused = refusal.is_some(),
                    "the colony came home from Formiga Hill without a receipt"
                );
                Some(match refusal {
                    Some(refusal) => session::refusal_text(&refusal),
                    None => "Formiga Hill closed before the trip was over, so everyone came \
                             straight home."
                        .to_owned(),
                })
            }
        }
    }

    /// The colony is home: by train if anyone can see it, at once if not.
    fn come_home(&mut self, note: Option<HomecomingNote>) {
        let scene = self
            .world
            .as_ref()
            .and_then(|world| match self.hill.trip.scene() {
                Some(scene) => scene.turn_back(&world.save, &self.monitors),
                None => TrainScene::arrival(&world.save, &self.monitors),
            });
        if let Err(error) = self.save() {
            tracing::error!(%error, "could not save the colony coming home");
        }
        match scene {
            Some(scene) => {
                self.hill.trip = TripState::Returning { scene, note };
                self.enter_trip();
            }
            None => self.finish_trip(note),
        }
    }

    /// The colony is back to living on the desktop.
    fn finish_trip(&mut self, note: Option<HomecomingNote>) {
        self.hill.trip = TripState::Idle;
        // The world picks up where it left off, as it does on opening, rather than with one long
        // tick for the time it was away.
        self.last_tick = Instant::now();
        self.sync_overlay_visibility();
        self.request_overlay_redraw();
        self.sync_hill_menu();
        if let Some(note) = note {
            if note.asked_for {
                self.failure_dialog(STAYING_HOME, &note.text);
            } else {
                self.settings_notice(note.text);
            }
        }
    }

    /// Whatever is shown of the colony is now the trip's: the hit-test windows go, the overlays
    /// show only the station, and the tray says where everyone is.
    fn enter_trip(&mut self) {
        for proxy in self.interaction_proxies.values_mut() {
            proxy.hide();
        }
        for proxy in self.house_proxies.values_mut() {
            proxy.hide();
        }
        self.sync_overlay_visibility();
        self.request_overlay_redraw();
        self.sync_hill_menu();
    }

    /// One tick of a trip under way, in place of the world's own: the train moves on, and a
    /// scene that has played out moves the trip on.
    pub(super) fn tick_trip(&mut self, dt: f32) {
        let Some(scene) = self.hill.trip.scene_mut() else {
            return;
        };
        scene.advance(dt);
        let finished = scene.finished();
        for overlay in self.overlays.values() {
            if overlay.is_visible() {
                overlay.window.request_redraw();
            }
        }
        // Everyone is aboard and the train is pulling away: now Hill opens. If it cannot, the train
        // stops where it is and everyone gets off again.
        if let TripState::Departing {
            open,
            scene,
            hill: hill @ Some(_),
        } = &mut self.hill.trip
            && scene.pulling_away()
        {
            let (install, open) = (hill.take().expect("matched Some"), open.clone());
            if let Err(error) = self.launch_hill(&install, &open) {
                tracing::warn!(%error, "Formiga Hill could not be started");
                self.hill.files.close(&open);
                self.come_home(Some(HomecomingNote {
                    text: error,
                    asked_for: true,
                }));
                return;
            }
        }
        if !finished {
            return;
        }
        match std::mem::take(&mut self.hill.trip) {
            TripState::Departing { open, .. } => {
                self.hill.trip = TripState::Away { open };
                self.sync_overlay_visibility();
                self.sync_hill_menu();
            }
            TripState::Returning { note, .. } => self.finish_trip(note),
            other => self.hill.trip = other,
        }
    }

    /// Finish a trip Desktop stopped in the middle of. With Hill's receipt the colony comes home
    /// with what it may keep; without one, Hill is told the trip is over and the colony comes home
    /// as it left. Either way it comes home now: Desktop never waits on Hill to show the colony.
    pub(super) fn resume_open_trip(&mut self) {
        // Whatever earlier trips left behind goes first; the open trip, if any, is kept.
        self.hill.files.sweep();
        let Some(open) = self.hill.files.open_trip() else {
            return;
        };
        let now = OffsetDateTime::now_utc();
        if self.recovery_pending {
            // The colony that left could not be opened, so there is nothing to bring it home to;
            // recovery decides what happens to it. Hill may still be running, so it is told.
            self.hill
                .files
                .call_home(&open, RecallReason::DesktopRestarted, now);
            return;
        }
        if self.read_homecoming(&open).is_some() {
            tracing::info!("finishing a trip after a restart, without a receipt");
            self.hill
                .files
                .call_home(&open, RecallReason::DesktopRestarted, now);
        } else {
            self.hill.files.close(&open);
        }
        self.come_home(None);
    }

    /// The colony as the overlay draws it this frame, while a trip has it: the platform's copy
    /// during a scene, or nobody while it is away.
    pub(super) fn trip_stage(&self, save: &SaveFile) -> Option<(SaveFile, Option<TrainView>)> {
        if !self.hill.trip.holds_world() {
            return None;
        }
        Some(match self.hill.trip.scene() {
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
        self.hill
            .trip
            .holds_world()
            .then(|| self.hill.trip.scene().map(TrainScene::monitor_id))
    }
}
