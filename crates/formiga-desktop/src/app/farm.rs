//! The app's side of a session in Formiga Farm: offering it only while Farm is installed, writing
//! the one companion (or stand-in) Farm is given, deciding on each design Farm proposes by the
//! contract's own rule, and answering Farm whatever happens. The states and the guarantees are
//! described in [`crate::farm`].

use super::*;
use crate::expansion::Slot;
use crate::farm::session::{FarmFiles, OpenSession, Purpose, refusal_text};
use crate::farm::{FARM, FarmEvent, FarmRequest, SessionState, Watcher};
use crate::settings::GenerationPreview;
use formiga_core::forms::Design;
use formiga_farm_contract::{
    Accepted, Current, FarmCapability, FarmProposal, FarmSnapshot, RecallReason, Rejection,
    Renderer, SessionId, Verdict, accept_proposal, apply_design, project_create, project_edit,
};

/// The title every reason for not opening Farm is shown under.
const STAYING_SHUT: &str = "Formiga Farm is staying shut";

/// What Desktop does with Farm's designs: keeps one for the companion a session opened on, takes
/// one into the studio for the owner to welcome, draws sculpted forms, and notes a kept look in
/// the journal. A capability is offered only once Desktop does what it says.
const OFFERED: [FarmCapability; 4] = [
    FarmCapability::EditExisting,
    FarmCapability::CreateNew,
    FarmCapability::SculptedForms,
    FarmCapability::JournalNote,
];

/// The session's own state and files, and Farm's slot, as the app keeps them.
pub(super) struct FarmLink {
    pub(super) session: SessionState,
    pub(super) files: FarmFiles,
    /// Whether Farm is installed, and when a development run opens a companion in it by itself.
    pub(super) slot: Slot,
}

impl FarmLink {
    pub(super) fn new(data_dir: &std::path::Path) -> Self {
        Self {
            session: SessionState::Idle,
            files: FarmFiles::new(data_dir),
            slot: Slot::new(&FARM),
        }
    }
}

impl FormigaApp {
    /// Look for Farm if it is time to. A session under way keeps the Farm it started with, and
    /// ends, telling Farm why, once the companion it was opened on has left the colony.
    pub(super) fn check_for_farm(&mut self, now: bool) {
        if let Some(open) = self.farm.session.open().cloned() {
            if let Purpose::Reshape { creature } = open.purpose
                && self.world.as_ref().is_some_and(|world| {
                    !world
                        .save
                        .creatures
                        .iter()
                        .any(|living| living.id == creature)
                })
            {
                tracing::info!("the companion open in Formiga Farm has left the colony");
                self.end_farm_session(RecallReason::CreatureGone);
            }
            return;
        }
        if self.farm.slot.start_due(self.recovery_pending) {
            let first = self
                .world
                .as_ref()
                .and_then(|world| world.save.creatures.first().map(|creature| creature.id));
            if let Some(first) = first {
                tracing::info!(
                    "opening the first companion in the stand-in Farm, as {} asks",
                    FARM.start_after_env
                );
                self.check_for_farm(true);
                self.open_farm(FarmRequest::Reshape(first));
                return;
            }
        }
        self.farm.slot.look(now);
    }

    /// Whether Farm can be offered just now: it is installed, nothing is open in it already, the
    /// colony is here rather than away on a trip, and nothing is being recovered.
    pub(super) fn can_open_farm(&self) -> bool {
        self.farm.slot.install.is_some()
            && self.farm.session.open().is_none()
            && !self.hill.trip.holds_world()
            && !self.recovery_pending
            && self.world.is_some()
    }

    /// What the notebook may offer about Farm this frame.
    pub(super) fn formiga_farm_view(&self) -> crate::clubhouse::FormigaFarmView {
        crate::clubhouse::FormigaFarmView {
            installed: self.farm.slot.install.is_some(),
            open: self.farm.session.open().map(|open| match open.purpose {
                Purpose::Reshape { creature } => FarmRequest::Reshape(creature),
                Purpose::Draw { .. } => FarmRequest::Draw,
            }),
            can_open: self.can_open_farm(),
        }
    }

    /// Open Farm on a companion, or on a stand-in for a new one. Every reason it cannot be is
    /// said, and nothing about the colony changes by opening it: the companion goes on living on
    /// the desktop the whole time.
    pub(super) fn open_farm(&mut self, request: FarmRequest) {
        if self.farm.session.open().is_some() || self.world.is_none() {
            return;
        }
        if self.recovery_pending {
            self.failure_dialog(
                STAYING_SHUT,
                "Finish recovering the colony in the notebook before opening Formiga Farm.",
            );
            return;
        }
        if let Some(reason) = self.farm_cannot_have(request) {
            self.failure_dialog(STAYING_SHUT, &reason);
            return;
        }
        // Asked again now: it may have been removed since the last look.
        self.farm.slot.look(true);
        let Some(install) = self.farm.slot.install.clone() else {
            self.failure_dialog(STAYING_SHUT, "Formiga Farm is no longer installed.");
            return;
        };
        let now = OffsetDateTime::now_utc();
        let desktop = self.snapshot();
        let prepared = {
            let Some(world) = &self.world else { return };
            SessionId::generate()
                .map_err(|error| error.to_string())
                .and_then(|session| match request {
                    FarmRequest::Reshape(creature) => project_edit(
                        &world.save,
                        creature,
                        true,
                        session,
                        now,
                        env!("CARGO_PKG_VERSION"),
                        OFFERED.to_vec(),
                        Renderer::this_build(),
                    )
                    .map(|snapshot| (snapshot, Purpose::Reshape { creature }))
                    .map_err(|error| error.to_string()),
                    FarmRequest::Draw => {
                        // The newcomer Desktop would welcome from this seed, before it is anyone:
                        // Farm draws over its size and gait, and the studio makes it from the
                        // same seed.
                        let stand_in_seed = new_colony_seed().map_err(|error| error.to_string())?;
                        let stand_in = World::preview_adult(stand_in_seed, now, &desktop);
                        project_create(
                            &world.save,
                            &stand_in.appearance,
                            session,
                            now,
                            env!("CARGO_PKG_VERSION"),
                            OFFERED.to_vec(),
                            Renderer::this_build(),
                        )
                        .map(|snapshot| (snapshot, Purpose::Draw { stand_in_seed }))
                        .map_err(|error| error.to_string())
                    }
                })
        };
        let (snapshot, purpose) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.failure_dialog(
                    STAYING_SHUT,
                    &format!("The companion could not be described for Formiga Farm: {error}"),
                );
                return;
            }
        };
        if let Some(reason) = FARM.incompatibility(&install, snapshot.min_reader_version) {
            self.failure_dialog(STAYING_SHUT, &reason);
            return;
        }
        let open = match self.farm.files.open(&snapshot, purpose) {
            Ok(open) => open,
            Err(error) => {
                self.failure_dialog(
                    STAYING_SHUT,
                    &format!("The session could not be written: {error}"),
                );
                return;
            }
        };
        let session = open.seal.session_id.clone();
        let exited = UserEvent::Farm(FarmEvent::Exited {
            session: session.clone(),
        });
        if let Err(error) = FARM.start(&install, &open.dir, exited, self.event_proxy.clone()) {
            self.farm.files.close(&open);
            self.failure_dialog(
                STAYING_SHUT,
                &format!("Formiga Farm could not be opened: {error}"),
            );
            return;
        }
        // Without a watcher every proposal is still decided, only later: when Farm closes.
        let watcher = Watcher::start(&open.dir, session, self.event_proxy.clone())
            .inspect_err(|error| {
                tracing::warn!(%error, "Formiga Farm's proposals will be decided when it closes");
            })
            .ok();
        tracing::info!(
            reshaping = matches!(request, FarmRequest::Reshape(_)),
            "Formiga Farm is open"
        );
        self.farm.session = SessionState::Open {
            open,
            _watcher: watcher,
        };
    }

    /// Why Farm cannot be given what `request` asks for just now: a companion out with another
    /// app is reshaped once it is back, and a new one is drawn while the colony is home.
    fn farm_cannot_have(&self, request: FarmRequest) -> Option<String> {
        match request {
            FarmRequest::Reshape(creature) => {
                let app = self.away_in(creature)?;
                let name = self
                    .world
                    .as_ref()
                    .and_then(|world| world.save.creatures.iter().find(|c| c.id == creature))
                    .map_or_else(|| "This companion".to_owned(), |c| c.name.clone());
                Some(format!(
                    "{name} is away in {} just now, and can be reshaped once back.",
                    app.name
                ))
            }
            FarmRequest::Draw => self.hill.trip.holds_world().then(|| {
                "The colony is away in Formiga Hill just now. Draw a new companion once it is home."
                    .to_owned()
            }),
        }
    }

    pub(super) fn handle_farm_event(&mut self, event_loop: &ActiveEventLoop, event: FarmEvent) {
        match event {
            FarmEvent::Proposed { session } => {
                if let Some(open) = self.farm_session(&session) {
                    self.decide_on_farm_proposal(Some(event_loop), &open);
                }
            }
            FarmEvent::Exited { session } => {
                let Some(open) = self.farm_session(&session) else {
                    // A session that is already over: whatever Farm left is not read.
                    return;
                };
                self.decide_on_farm_proposal(Some(event_loop), &open);
                let refusal = self.farm.files.refusal(&open);
                self.farm.files.close(&open);
                self.farm.session = SessionState::Idle;
                tracing::info!(refused = refusal.is_some(), "Formiga Farm has closed");
                if let Some(refusal) = refusal {
                    self.failure_dialog(STAYING_SHUT, &refusal_text(&refusal));
                }
            }
        }
    }

    /// The session open now, if it is `session`.
    fn farm_session(&self, session: &SessionId) -> Option<OpenSession> {
        self.farm
            .session
            .open()
            .filter(|open| open.seal.session_id == *session)
            .cloned()
    }

    /// Decide on the newest proposal Farm has not been answered on yet, and answer it. A new
    /// companion's design goes into the studio, which needs the event loop to open the notebook
    /// to it; without one, as when Desktop is closing, it is left unanswered and Farm keeps it as
    /// a draft.
    fn decide_on_farm_proposal(
        &mut self,
        event_loop: Option<&ActiveEventLoop>,
        open: &OpenSession,
    ) {
        let proposal = match self.farm.files.unanswered(open) {
            Ok(Some(proposal)) => proposal,
            Ok(None) => return,
            Err(problem) => {
                tracing::warn!(%problem, "a proposal from Formiga Farm was not weighed");
                return;
            }
        };
        let now = OffsetDateTime::now_utc();
        let verdict = match self.farm.files.snapshot(open) {
            Ok(snapshot) => self.weigh_farm_proposal(event_loop, open, &snapshot, &proposal, now),
            Err(problem) => {
                tracing::warn!(%problem, "a proposal from Formiga Farm was not weighed");
                Some(Verdict::Invalid)
            }
        };
        let Some(verdict) = verdict else {
            return;
        };
        if let Err(error) = self.farm.files.answer(open, proposal.serial, verdict, now) {
            tracing::warn!(%error, "could not answer Formiga Farm");
        }
    }

    /// What Desktop makes of one proposal: the contract's own rule first, every time, and then
    /// the change it allows, made the way Desktop makes any other. `None` when a new companion's
    /// design cannot be shown in the studio just now, and so is not answered.
    fn weigh_farm_proposal(
        &mut self,
        event_loop: Option<&ActiveEventLoop>,
        open: &OpenSession,
        snapshot: &FarmSnapshot,
        proposal: &FarmProposal,
        now: OffsetDateTime,
    ) -> Option<Verdict> {
        let accepted = {
            let current = match open.purpose {
                Purpose::Reshape { creature } => {
                    let available = !self.recovery_pending && self.away_in(creature).is_none();
                    match self.world.as_ref().and_then(|world| {
                        world
                            .save
                            .creatures
                            .iter()
                            .find(|living| living.id == creature)
                    }) {
                        Some(creature) => Current::Creature {
                            creature,
                            available,
                        },
                        None => Current::Gone,
                    }
                }
                Purpose::Draw { .. } => Current::Nobody,
            };
            accept_proposal(&open.seal, snapshot, proposal, current)
        };
        match (accepted, open.purpose) {
            (Ok(Accepted::Edit { target, design }), _) => {
                let note = snapshot.offers(FarmCapability::JournalNote);
                Some(self.keep_new_look(target, &design, note, now))
            }
            (Ok(Accepted::Create { design, .. }), Purpose::Draw { stand_in_seed }) => {
                self.offer_drawn_companion(event_loop?, stand_in_seed, design, now)
            }
            (Ok(Accepted::Create { .. }), Purpose::Reshape { .. }) => Some(Verdict::Unsupported),
            (Err(rejection), _) => {
                match &rejection {
                    Rejection::Invalid(problem) => {
                        tracing::warn!(%problem, "a design from Formiga Farm did not check out");
                    }
                    Rejection::Stale { .. } => {
                        tracing::info!("a design from Formiga Farm was made from an older look");
                    }
                    other => tracing::info!(?other, "a design from Formiga Farm was not kept"),
                }
                Some(rejection.verdict())
            }
        }
    }

    /// Give the companion its new look, note it in the journal if Farm was told it would be, and
    /// save at once: Farm is told the look is kept only once it is.
    fn keep_new_look(
        &mut self,
        target: CreatureId,
        design: &Design,
        note: bool,
        now: OffsetDateTime,
    ) -> Verdict {
        let Some(world) = &mut self.world else {
            return Verdict::Gone;
        };
        let Some(creature) = world
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == target)
        else {
            return Verdict::Gone;
        };
        apply_design(creature, design);
        let revision = Design::of(&creature.appearance).revision();
        if note {
            world.note_new_look(target, now);
        }
        if let Err(error) = self.save() {
            tracing::error!(%error, "could not save a companion's new look");
        }
        self.request_overlay_redraw();
        tracing::info!("a companion has a new look from Formiga Farm");
        Verdict::Kept { revision }
    }

    /// Show a new companion's design in the studio, on the stand-in it was drawn over, for the
    /// owner to welcome there as any sketch is welcomed. Nobody joins the colony until then.
    fn offer_drawn_companion(
        &mut self,
        event_loop: &ActiveEventLoop,
        source_seed: [u8; 32],
        design: Design,
        now: OffsetDateTime,
    ) -> Option<Verdict> {
        let desktop = self.snapshot();
        let mut creature = World::preview_adult(source_seed, now, &desktop);
        apply_design(&mut creature, &design);
        self.show_settings(event_loop);
        let Some(window) = &mut self.settings_window else {
            tracing::warn!("a companion drawn in Formiga Farm could not be shown in the studio");
            return None;
        };
        window.set_generation_preview(GenerationPreview {
            shared: None,
            creature,
            source_seed,
            similarity: None,
            summary: "Drawn in Formiga Farm. Adopt them to give them a name and a life here."
                .into(),
            drawn: Some(Box::new(design)),
        });
        tracing::info!("a companion drawn in Formiga Farm is waiting in the studio");
        Some(Verdict::Welcomed)
    }

    /// End the session while Farm may still be running, and tell Farm why. Whatever it proposed
    /// and was not answered on is not kept.
    fn end_farm_session(&mut self, reason: RecallReason) {
        let Some(open) = self.farm.session.open().cloned() else {
            return;
        };
        self.farm.files.call_home(&open, reason);
        self.farm.session = SessionState::Idle;
    }

    /// Desktop is quitting: keep the look Farm proposed last for the companion it has open, and
    /// tell Farm the session is over. A new companion's design stays in Farm as a draft, since
    /// the studio it would wait in is closing too.
    pub(super) fn close_farm_session(&mut self) {
        if let Some(open) = self.farm.session.open().cloned() {
            self.decide_on_farm_proposal(None, &open);
        }
        self.end_farm_session(RecallReason::Closing);
    }

    /// The colony is about to be replaced: tell Farm the session is over, and keep nothing of it.
    pub(super) fn abandon_farm_session(&mut self) {
        self.end_farm_session(RecallReason::Closing);
    }

    /// Finish a session Desktop stopped in the middle of: keep the look Farm last proposed for
    /// the companion it had open, if it can still be kept, and tell Farm the session is over.
    pub(super) fn resume_farm_session(&mut self) {
        // Whatever earlier sessions left behind goes first; the open one, if any, is kept.
        self.farm.files.sweep();
        let Some(open) = self.farm.files.open_session() else {
            return;
        };
        if !self.recovery_pending {
            self.decide_on_farm_proposal(None, &open);
        }
        tracing::info!("ending a session in Formiga Farm after a restart");
        self.farm.files.call_home(&open, RecallReason::Closing);
    }
}
