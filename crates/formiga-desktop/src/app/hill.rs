//! The app's side of a trip to Formiga Hill: offering it in the tray only while Hill is installed,
//! sending the colony, holding the world still while it is away, and bringing it home whatever
//! happens. The states and the guarantees are described in [`crate::hill`].

use super::*;
use crate::expansion::Slot;
use crate::hill::session::{self, Answer, TravelFiles};
use crate::hill::{HILL, HillEvent, TripState, scene::TrainScene};
use crate::platform::companion_app::AppInstall;
use crate::tray::VisitOffer;
use crate::trip::{HomecomingNote, Step};
use formiga_travel::{RecallReason, SessionId, project_colony};
use visits::HILL_ITEM;

/// The title every reason for not leaving is shown under.
const STAYING_HOME: &str = "The colony is staying home";

/// The trip's own state and files, and Hill's slot, as the app keeps them.
pub(super) struct HillLink {
    pub(super) trip: TripState,
    pub(super) files: TravelFiles,
    /// Whether Hill is installed, and when a development run sends the colony by itself.
    pub(super) slot: Slot,
}

impl HillLink {
    pub(super) fn new(data_dir: &std::path::Path) -> Self {
        Self {
            trip: TripState::Idle,
            files: TravelFiles::new(data_dir),
            slot: Slot::new(&HILL),
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
        if self.hill.slot.start_due(self.recovery_pending) {
            tracing::info!(
                "sending the colony to the stand-in Hill, as {} asks",
                HILL.start_after_env
            );
            self.go_to_hill();
            return;
        }
        if self.hill.slot.look(now) {
            self.sync_hill_menu();
        }
    }

    /// Offer the trip to Formiga Hill in the tray, or the way home from it, or nothing at all.
    /// While the colony is away the actions that move it about the desktop wait for it to come
    /// back.
    pub(super) fn sync_hill_menu(&mut self) {
        let offer = match &self.hill.trip {
            TripState::Idle if self.hill.slot.install.is_some() => {
                Some(("Go to Formiga Hill…", true))
            }
            TripState::Idle => None,
            TripState::Departing { .. } | TripState::Away { .. } => {
                Some(("Bring the colony home", true))
            }
            TripState::Returning { .. } => Some(("Coming home…", false)),
        }
        .map(|(text, enabled)| VisitOffer {
            text: text.to_owned(),
            enabled,
        });
        let away = self.colony_away_in().map(|app| app.name);
        if let Some(tray) = &mut self.tray {
            tray.sync_visit(HILL_ITEM, offer);
            tray.sync_colony_away(away);
        }
    }

    /// What Hill's tray item offers: the trip while the colony is home, the way back while it is
    /// leaving or away.
    pub(super) fn choose_hill_item(&mut self) {
        match self.hill.trip {
            TripState::Idle => self.go_to_hill(),
            TripState::Departing { .. } | TripState::Away { .. } => self.bring_colony_home(),
            TripState::Returning { .. } => {}
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
        if let Some(reason) = self.busy_elsewhere(&HILL, None) {
            self.failure_dialog(STAYING_HOME, reason);
            return;
        }
        // Asked again now: it may have been removed since the last look.
        self.check_for_hill(true);
        let Some(install) = self.hill.slot.install.clone() else {
            self.failure_dialog(STAYING_HOME, "Formiga Hill is no longer installed.");
            return;
        };
        // The travel version this colony needs: newer only if someone in it is drawn in a way an
        // older Hill cannot draw.
        let needs = self
            .world
            .as_ref()
            .map_or(1, |world| formiga_travel::reader_for_colony(&world.save));
        if let Some(reason) = HILL.incompatibility(&install, needs) {
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
        let scene = TrainScene::departure(&world.save, &self.monitors);
        let (trip, start_now) = TripState::leave(open.clone(), scene, install);
        if let Some(install) = start_now
            && let Err(error) = self.launch_hill(&install, &open)
        {
            self.hill.files.close(&open);
            self.failure_dialog(STAYING_HOME, &error);
            return;
        }
        self.hill.trip = trip;
        self.enter_trip();
    }

    /// Start Hill for this trip, with a thread waiting for it to exit.
    fn launch_hill(&self, install: &AppInstall, open: &session::OpenTrip) -> Result<(), String> {
        let exited = UserEvent::Hill(HillEvent::Exited {
            session: open.seal.session_id.clone(),
        });
        HILL.start(install, &open.dir, exited, self.event_proxy.clone())
            .map_err(|error| format!("Formiga Hill could not be opened: {error}"))?;
        tracing::info!("the colony left for Formiga Hill");
        Ok(())
    }

    /// The owner asked for the colony back before Hill was done with it.
    pub(super) fn bring_colony_home(&mut self) {
        let Some(open) = self.hill.trip.open().cloned() else {
            return;
        };
        if !self.hill.trip.boarding() {
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
        match self.hill.trip.come_home(scene, note) {
            None => self.enter_trip(),
            Some(note) => self.finish_trip(note),
        }
    }

    /// The colony is back to living on the desktop.
    fn finish_trip(&mut self, note: Option<HomecomingNote>) {
        self.back_on_desktop();
        if let Some(note) = note {
            if note.asked_for {
                self.failure_dialog(STAYING_HOME, &note.text);
            } else {
                self.settings_notice(note.text);
            }
        }
    }

    /// One tick of a trip under way, in place of the world's own: the train moves on, and a
    /// scene that has played out moves the trip on.
    pub(super) fn tick_trip(&mut self, dt: f32) {
        if self.hill.trip.scene().is_none() {
            return;
        }
        let step = self.hill.trip.advance(dt);
        self.redraw_train();
        match step {
            Step::Nothing => {}
            // Everyone is aboard and the train is pulling away: now Hill opens. If it cannot, the
            // train stops where it is and everyone gets off again.
            Step::Start { app, open } => {
                if let Err(error) = self.launch_hill(&app, &open) {
                    tracing::warn!(%error, "Formiga Hill could not be started");
                    self.hill.files.close(&open);
                    self.come_home(Some(HomecomingNote {
                        text: error,
                        asked_for: true,
                    }));
                }
            }
            Step::Gone => {
                self.sync_overlay_visibility();
                self.sync_hill_menu();
            }
            Step::Home(note) => self.finish_trip(note),
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
}
