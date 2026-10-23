//! The app's side of a match in Formiga Battle: offering it in the tray only while Battle is
//! installed, writing the colony's roster and its Battle record for it, taking everyone there on
//! the train that goes to Formiga Hill, keeping what its matches earned once it closes, bringing
//! everyone home, and telling the owner in Battle's own words if it turned the roster away. The
//! states and the guarantees are described in [`crate::battle`].

use super::companions::Companion;
use super::*;
use crate::battle::record;
use crate::battle::session::{BattleFiles, OpenSession, refusal_text};
use crate::battle::{BATTLE, BattleEvent, SessionState};
use crate::expansion::{Slot, Takes};
use crate::hill::scene::TrainScene;
use crate::platform::companion_app::AppInstall;
use crate::tray::VisitOffer;
use crate::trip::{HomecomingNote, Step};
use formiga_battle_contract::{SessionId, project_roster};
use visits::BATTLE_ITEM;

/// The title every reason for not opening Battle is shown under.
const STAYING_SHUT: &str = BATTLE.words.staying;

/// The session's own state and files, and Battle's slot, as the app keeps them.
pub(super) struct BattleLink {
    pub(super) session: SessionState,
    pub(super) files: BattleFiles,
    /// Whether Battle is installed, and when a development run opens a match in it by itself.
    pub(super) slot: Slot,
}

impl BattleLink {
    pub(super) fn new(data_dir: &std::path::Path) -> Self {
        Self {
            session: SessionState::Idle,
            files: BattleFiles::new(data_dir),
            slot: Slot::new(&BATTLE),
        }
    }
}

impl FormigaApp {
    /// Look for Battle if it is time to. A session under way keeps the Battle it started with.
    pub(super) fn check_for_battle(&mut self, now: bool) {
        if self.battle.session.holds_world() {
            return;
        }
        if self.battle.slot.start_due(self.recovery_pending) {
            tracing::info!(
                "opening the colony in the stand-in Battle, as {} asks",
                BATTLE.start_after_env
            );
            self.check_for_battle(true);
            self.play_battle();
            return;
        }
        if self.battle.slot.look(now) {
            self.sync_battle_menu();
        }
    }

    /// Offer a match in the tray while Battle is installed, say so while one is open or the
    /// colony is coming home from it, and say nothing at all otherwise.
    pub(super) fn sync_battle_menu(&mut self) {
        let offer = if self.battle.session.open().is_some() {
            Some(("Formiga Battle is open", false))
        } else if self.battle.session.holds_world() {
            Some(("Coming home…", false))
        } else if self.battle.slot.install.is_some() {
            Some(("Play Formiga Battle…", true))
        } else {
            None
        }
        .map(|(text, enabled)| VisitOffer {
            text: text.to_owned(),
            enabled,
        });
        let away = self.colony_away_in().map(|app| app.name);
        if let Some(tray) = &mut self.tray {
            tray.sync_visit(BATTLE_ITEM, offer);
            tray.sync_colony_away(away);
        }
    }

    /// Take the colony to Battle on its roster: the train that goes to Formiga Hill pulls in,
    /// everyone gets on, and Battle opens as it pulls away. Every reason it cannot go is said, and
    /// leaves the colony home as it was.
    pub(super) fn play_battle(&mut self) {
        if self.battle.session.holds_world() || self.world.is_none() {
            return;
        }
        let Some(install) = self.ready_for(Companion::Battle, Takes::Everyone) else {
            return;
        };
        let snapshot = {
            let Some(world) = &mut self.world else { return };
            // The colony's anonymous Handler key is made the first time it goes to Battle, and
            // written with the colony as it leaves. Without one Battle still opens; a friend's
            // Battle just cannot know this Handler again.
            if let Err(error) = world.save.battle.record.give_handler_key() {
                tracing::warn!(%error, "no Handler key could be made for Formiga Battle");
            }
            SessionId::generate()
                .map_err(|error| error.to_string())
                .and_then(|session| {
                    project_roster(
                        &world.save,
                        session,
                        OffsetDateTime::now_utc(),
                        env!("CARGO_PKG_VERSION"),
                    )
                    .map_err(|error| error.to_string())
                })
                .map(|roster| with_record(roster, &world.save.battle.record))
        };
        let snapshot = match snapshot {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.failure_dialog(
                    STAYING_SHUT,
                    &format!("The colony could not be described for Formiga Battle: {error}"),
                );
                return;
            }
        };
        if let Some(reason) = BATTLE.incompatibility(&install, snapshot.min_reader_version) {
            self.failure_dialog(STAYING_SHUT, &reason);
            return;
        }
        let open = match self.battle.files.open(&snapshot) {
            Ok(open) => open,
            Err(error) => {
                self.failure_dialog(
                    STAYING_SHUT,
                    &format!("The session could not be written: {error}"),
                );
                return;
            }
        };
        tracing::info!(
            contenders = snapshot.contenders.len(),
            "the colony is leaving for Formiga Battle"
        );
        // Everyone stops what they are doing and stands still out of doors to wait for the train,
        // as for a trip to Hill, and the colony is written as it leaves.
        self.finish_habitat_editor(false);
        self.close_creature_menu(MenuDismissal::Hidden);
        let scene = {
            let Some(world) = &mut self.world else { return };
            world.prepare_for_trip(OffsetDateTime::now_utc());
            TrainScene::departure(&world.save, &self.monitors)
        };
        // A colony that cannot be written stays home: Battle's results are kept against the
        // colony as it left, and a restart would otherwise find an older one on disk.
        if let Err(error) = self.save() {
            tracing::error!(%error, "could not save the colony leaving for Formiga Battle");
            self.battle.files.close(&open);
            self.failure_dialog(
                STAYING_SHUT,
                &format!("The colony could not be saved, so it is not leaving: {error}"),
            );
            return;
        }
        // With the train to watch, Battle opens as it pulls away; with nothing on the desktop to
        // see, at once.
        let (trip, start_now) = SessionState::leave(open.clone(), scene, install);
        if let Some(install) = start_now
            && let Err(error) = self.launch_battle(&install, &open)
        {
            self.battle.files.close(&open);
            self.failure_dialog(STAYING_SHUT, &error);
            return;
        }
        self.battle.session = trip;
        self.enter_trip();
    }

    /// Start Battle for this session, with a thread waiting for it to exit.
    fn launch_battle(&self, install: &AppInstall, open: &OpenSession) -> Result<(), String> {
        let exited = UserEvent::Battle(BattleEvent::Exited {
            session: open.session_id.clone(),
        });
        BATTLE
            .start(install, &open.dir, exited, self.event_proxy.clone())
            .map_err(|error| format!("Formiga Battle could not be opened: {error}"))?;
        tracing::info!("Formiga Battle is open");
        Ok(())
    }

    /// One tick of the train to or from Battle, in place of the world's own.
    pub(super) fn tick_battle_trip(&mut self, dt: f32) {
        if self.battle.session.scene().is_none() {
            return;
        }
        let step = self.battle.session.advance(dt);
        self.redraw_train();
        match step {
            Step::Nothing => {}
            // Everyone is aboard and the train is pulling away: now Battle opens. If it cannot,
            // the train stops where it is and everyone gets off again.
            Step::Start { app, open } => {
                if let Err(error) = self.launch_battle(&app, &open) {
                    tracing::warn!(%error, "Formiga Battle could not be started");
                    self.battle.files.close(&open);
                    self.battle_trip_home(Some(error));
                }
            }
            Step::Gone => {
                self.sync_overlay_visibility();
                self.sync_battle_menu();
            }
            Step::Home(note) => self.finish_battle_trip(note),
        }
    }

    /// The session is over and the colony comes home: by train if anyone can see it, at once if
    /// not. `refusal` is said once everyone is home.
    fn battle_trip_home(&mut self, refusal: Option<String>) {
        let note = refusal.map(|text| HomecomingNote {
            text,
            asked_for: true,
        });
        let scene = self.train_home(self.battle.session.scene());
        match self.battle.session.come_home(scene, note) {
            None => self.enter_trip(),
            Some(note) => self.finish_battle_trip(note),
        }
    }

    /// The colony is back from Battle and living on the desktop again.
    fn finish_battle_trip(&mut self, note: Option<HomecomingNote>) {
        self.back_on_desktop();
        tracing::info!("the colony is home from Formiga Battle");
        if let Some(note) = note {
            self.failure_dialog(STAYING_SHUT, &note.text);
        }
    }

    pub(super) fn handle_battle_event(&mut self, event: BattleEvent) {
        match event {
            BattleEvent::Exited { session } => {
                let Some(open) = self.battle_session(&session) else {
                    // A session that is already over: whatever Battle left is not read.
                    return;
                };
                let refusal = self.battle.files.refusal(&open);
                self.keep_battle_result(&open);
                self.battle.files.close(&open);
                tracing::info!(refused = refusal.is_some(), "Formiga Battle has closed");
                self.battle_trip_home(refusal.as_ref().map(refusal_text));
            }
        }
    }

    /// The session open now, if it is `session`.
    fn battle_session(&self, session: &SessionId) -> Option<OpenSession> {
        self.battle
            .session
            .open()
            .filter(|open| open.session_id == *session)
            .cloned()
    }

    /// Clear a session Desktop stopped in the middle of, and bring the colony home by train.
    /// Battle, if it is still open, keeps the roster it already read; nothing it might say now is
    /// waited for.
    pub(super) fn resume_battle_session(&mut self) {
        // Whatever earlier sessions left behind goes first; the open one, if any, is kept.
        self.battle.files.sweep();
        let Some(open) = self.battle.files.open_session() else {
            return;
        };
        tracing::info!("keeping what a session in Formiga Battle earned, after a restart");
        self.keep_battle_result(&open);
        self.battle.files.close(&open);
        if !self.recovery_pending && !self.colony_away() {
            self.battle_trip_home(None);
        }
    }

    /// The colony is about to be replaced: whatever Battle goes on to say belongs to the colony
    /// it was given, so nothing more of it is read, and the new colony is on the desktop at once.
    pub(super) fn abandon_battle_session(&mut self) {
        if !self.battle.session.holds_world() {
            return;
        }
        if let Some(open) = self.battle.session.open().cloned() {
            self.battle.files.close(&open);
        }
        self.battle.session = SessionState::Idle;
        self.back_on_desktop();
    }

    /// Keep what the session's matches earned: each match Battle names is applied to the colony's
    /// record by the contract's own rule, once, by its id, and only a match applied for the first
    /// time adds its lines to the journal and its keepsakes to the colony's. The colony is saved
    /// at once. A result that does not check out, or one that comes while the colony is being
    /// recovered, keeps nothing.
    fn keep_battle_result(&mut self, open: &OpenSession) {
        let result = match self.battle.files.result(open) {
            Ok(Some(result)) => result,
            Ok(None) => return,
            Err(problem) => {
                tracing::warn!(%problem, "what Formiga Battle said a session earned was not kept");
                return;
            }
        };
        if self.recovery_pending {
            tracing::warn!("what Formiga Battle earned was not kept while the colony is recovered");
            return;
        }
        let Some(world) = &mut self.world else { return };
        match record::keep_result(world, &result, OffsetDateTime::now_utc()) {
            Ok(kept) if kept.matches == 0 && !kept.registry => return,
            Ok(kept) => tracing::info!(
                matches = kept.matches,
                keepsakes = kept.keepsakes,
                registry = kept.registry,
                "kept what Formiga Battle's matches earned"
            ),
            Err(problem) => {
                tracing::warn!(%problem, "what Formiga Battle earned was not kept");
                return;
            }
        }
        if let Err(error) = self.save() {
            tracing::error!(%error, "could not save what Formiga Battle's matches earned");
        }
    }
}

/// The roster, carrying the colony's Battle record once it has one. A record that cannot be sent
/// as Battle reads it is left out, and Battle opens as for a colony that has never played; what
/// it earns is still kept.
fn with_record(
    roster: formiga_battle_contract::BattleSnapshot,
    kept: &formiga_core::BattleRecord,
) -> formiga_battle_contract::BattleSnapshot {
    if kept.is_empty() {
        return roster;
    }
    let sent = record::to_contract(kept).map(|record| roster.clone().with_record(record));
    match sent {
        Ok(sent) if sent.validate().is_ok() => sent,
        Ok(_) => {
            tracing::warn!("the colony's Battle record is outside what Battle reads; not sent");
            roster
        }
        Err(problem) => {
            tracing::warn!(%problem, "the colony's Battle record was not sent");
            roster
        }
    }
}
