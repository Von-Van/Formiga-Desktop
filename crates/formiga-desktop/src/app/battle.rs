//! The app's side of a match in Formiga Battle: offering it in the tray only while Battle is
//! installed, writing the colony's roster for it, and telling the owner in Battle's own words if
//! it turned the roster away. The states and the guarantees are described in [`crate::battle`].

use super::*;
use crate::battle::session::{BattleFiles, OpenSession, refusal_text};
use crate::battle::{BATTLE, BattleEvent, SessionState};
use crate::expansion::Slot;
use crate::tray::VisitOffer;
use formiga_battle_contract::{SessionId, project_roster};
use visits::BATTLE_ITEM;

/// The title every reason for not opening Battle is shown under.
const STAYING_SHUT: &str = "Formiga Battle is staying shut";

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
        if self.battle.session.open().is_some() {
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

    /// Offer a match in the tray while Battle is installed, say so while one is open, and say
    /// nothing at all otherwise.
    pub(super) fn sync_battle_menu(&mut self) {
        let offer = if self.battle.session.open().is_some() {
            Some(("Formiga Battle is open", false))
        } else if self.battle.slot.install.is_some() {
            Some(("Play Formiga Battle…", true))
        } else {
            None
        }
        .map(|(text, enabled)| VisitOffer {
            text: text.to_owned(),
            enabled,
        });
        if let Some(tray) = &mut self.tray {
            tray.sync_visit(BATTLE_ITEM, offer);
        }
    }

    /// Open Battle on the colony's roster. Every reason it cannot be is said, and nothing about
    /// the colony changes by opening it: every companion goes on living on the desktop the whole
    /// time.
    pub(super) fn play_battle(&mut self) {
        if self.battle.session.open().is_some() || self.world.is_none() {
            return;
        }
        if self.recovery_pending {
            self.failure_dialog(
                STAYING_SHUT,
                "Finish recovering the colony in the notebook before opening Formiga Battle.",
            );
            return;
        }
        if let Some(reason) = self.busy_elsewhere(&BATTLE, None) {
            self.failure_dialog(STAYING_SHUT, reason);
            return;
        }
        // Asked again now: it may have been removed since the last look.
        self.battle.slot.look(true);
        let Some(install) = self.battle.slot.install.clone() else {
            self.sync_battle_menu();
            self.failure_dialog(STAYING_SHUT, "Formiga Battle is no longer installed.");
            return;
        };
        let snapshot = {
            let Some(world) = &self.world else { return };
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
        let exited = UserEvent::Battle(BattleEvent::Exited {
            session: open.session_id.clone(),
        });
        if let Err(error) = BATTLE.start(&install, &open.dir, exited, self.event_proxy.clone()) {
            self.battle.files.close(&open);
            self.failure_dialog(
                STAYING_SHUT,
                &format!("Formiga Battle could not be opened: {error}"),
            );
            return;
        }
        tracing::info!(
            contenders = snapshot.contenders.len(),
            "Formiga Battle is open"
        );
        self.battle.session = SessionState::Open(open);
        self.sync_battle_menu();
    }

    pub(super) fn handle_battle_event(&mut self, event: BattleEvent) {
        match event {
            BattleEvent::Exited { session } => {
                let Some(open) = self.battle_session(&session) else {
                    // A session that is already over: whatever Battle left is not read.
                    return;
                };
                let refusal = self.battle.files.refusal(&open);
                self.battle.files.close(&open);
                self.battle.session = SessionState::Idle;
                self.sync_battle_menu();
                tracing::info!(refused = refusal.is_some(), "Formiga Battle has closed");
                if let Some(refusal) = refusal {
                    self.failure_dialog(STAYING_SHUT, &refusal_text(&refusal));
                }
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

    /// Clear a session Desktop stopped in the middle of. Battle, if it is still open, keeps the
    /// roster it already read; nothing it might say now is waited for.
    pub(super) fn resume_battle_session(&mut self) {
        // Whatever earlier sessions left behind goes first; the open one, if any, is kept.
        self.battle.files.sweep();
        let Some(open) = self.battle.files.open_session() else {
            return;
        };
        tracing::info!("clearing a session in Formiga Battle after a restart");
        self.battle.files.close(&open);
    }
}
