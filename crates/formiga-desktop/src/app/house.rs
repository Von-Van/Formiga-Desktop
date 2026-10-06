//! The app's side of a visit to a house in Formiga Home: offering it only while Home is
//! installed, letting the household go indoors while Home has the house, keeping what Home may
//! change, and letting everyone back out whatever happens. The states and the guarantees are
//! described in [`crate::house`].

use super::*;
use crate::expansion::Slot;
use crate::house::session::{self, HouseFiles, OpenVisit};
use crate::house::{HOME, HouseEvent, VisitState};
use crate::houses::{HouseProxy, PlacedHouse};
use crate::tray::VisitOffer;
use formiga_home_contract::{
    HomeCapability, RecallReason, SessionId, likely_visitors, project_household,
};
use visits::HOME_ITEM;

/// The title every reason for not opening a house is shown under.
const STAYING_SHUT: &str = "The house is staying shut";

/// What Desktop takes back from a visit: a line in the journal for it, and nothing more. A
/// capability is offered only once Desktop applies what it brings.
const OFFERED: [HomeCapability; 1] = [HomeCapability::VisitRecord];

/// The visit's own state and files, and Home's slot, as the app keeps them.
pub(super) struct HouseLink {
    pub(super) visit: VisitState,
    pub(super) files: HouseFiles,
    /// Whether Home is installed, and when a development run opens the colony house by itself.
    pub(super) slot: Slot,
}

impl HouseLink {
    pub(super) fn new(data_dir: &std::path::Path) -> Self {
        Self {
            visit: VisitState::Idle,
            files: HouseFiles::new(data_dir),
            slot: Slot::new(&HOME),
        }
    }
}

impl FormigaApp {
    /// Look for Home if it is time to. A house can be opened only while it is installed, and a
    /// visit under way keeps the Home it started with.
    pub(super) fn check_for_home(&mut self, now: bool) {
        if self.house.visit.open().is_some() {
            return;
        }
        if self.house.slot.start_due(self.recovery_pending) {
            let colony_house = self.world.as_ref().and_then(|world| {
                formiga_core::house_owners(&world.save.creatures, &world.save.home.cottage_order)
                    .as_slice()
                    .first()
                    .copied()
            });
            if let Some(keeper) = colony_house {
                tracing::info!(
                    "opening the colony house in the stand-in Home, as {} asks",
                    HOME.start_after_env
                );
                self.check_for_home(true);
                self.open_house(keeper);
                return;
            }
        }
        self.house.slot.look(now);
    }

    /// Whether a house can be offered to be opened just now: Home is installed, no house is open
    /// already, the colony is here rather than away on a trip, and nothing is being recovered.
    pub(super) fn can_open_house(&self) -> bool {
        self.house.slot.install.is_some()
            && self.house.visit.open().is_none()
            && !self.hill.trip.holds_world()
            && !self.recovery_pending
            && self.world.is_some()
    }

    /// What the notebook may offer about Home this frame.
    pub(super) fn formiga_home_view(&self) -> crate::clubhouse::FormigaHomeView {
        crate::clubhouse::FormigaHomeView {
            installed: self.house.slot.install.is_some(),
            open: self.house.visit.open().map(|open| open.keeper),
            can_open: self.can_open_house(),
        }
    }

    pub(super) fn sync_house_menu(&mut self) {
        let away = self.house.visit.open().map(|open| {
            self.world
                .as_ref()
                .and_then(|world| world.save.creatures.iter().find(|c| c.id == open.keeper))
                .map_or_else(|| "the household".to_owned(), |keeper| keeper.name.clone())
        });
        let offer = away.map(|keeper| VisitOffer {
            text: format!("Bring {keeper}'s household back"),
            enabled: true,
        });
        if let Some(tray) = &mut self.tray {
            tray.sync_visit(HOME_ITEM, offer);
        }
    }

    /// Open the house `keeper` keeps, in Home. Every reason it cannot be is said, and leaves the
    /// household exactly as it was.
    pub(super) fn open_house(&mut self, keeper: CreatureId) {
        if self.house.visit.open().is_some() || self.world.is_none() {
            return;
        }
        if let Some(reason) = self.busy_elsewhere(&HOME, Some(&[keeper])) {
            self.failure_dialog(STAYING_SHUT, reason);
            return;
        }
        if self.recovery_pending {
            self.failure_dialog(
                STAYING_SHUT,
                "Finish recovering the colony in the notebook before opening its houses.",
            );
            return;
        }
        if let Some(trouble) = self.save_trouble.clone() {
            self.failure_dialog(
                STAYING_SHUT,
                &format!("The colony cannot be saved just now, so no house is opened. {trouble}"),
            );
            return;
        }
        // Asked again now: it may have been removed since the last look.
        self.check_for_home(true);
        let Some(install) = self.house.slot.install.clone() else {
            self.failure_dialog(STAYING_SHUT, "Formiga Home is no longer installed.");
            return;
        };
        let now = OffsetDateTime::now_utc();
        let prepared = {
            let Some(world) = &self.world else { return };
            // Whoever lives there goes in, with whichever of its closest friends from other
            // houses are free to come: not in the owner's hand, nor away in another app.
            let visitors: Vec<CreatureId> = likely_visitors(&world.save, keeper)
                .into_iter()
                .filter(|id| {
                    world.free_to_go_in(*id) && self.busy_elsewhere(&HOME, Some(&[*id])).is_none()
                })
                .collect();
            SessionId::generate()
                .map_err(|error| error.to_string())
                .and_then(|session| {
                    project_household(
                        &world.save,
                        keeper,
                        &visitors,
                        &OFFERED,
                        session,
                        now,
                        env!("CARGO_PKG_VERSION"),
                    )
                    .map_err(|error| error.to_string())
                })
                .map(|snapshot| {
                    let away: Vec<CreatureId> = snapshot
                        .residents
                        .iter()
                        .chain(&snapshot.visitors)
                        .map(|traveler| traveler.id.0)
                        .collect();
                    // Someone in the owner's hand cannot go in.
                    let busy = away
                        .iter()
                        .find(|id| !world.free_to_go_in(**id))
                        .map(|busy| {
                            world
                                .save
                                .creatures
                                .iter()
                                .find(|creature| creature.id == *busy)
                                .map_or_else(
                                    || "Someone who lives there".to_owned(),
                                    |creature| creature.name.clone(),
                                )
                        });
                    (snapshot, away, busy)
                })
        };
        let (snapshot, away, busy) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.failure_dialog(
                    STAYING_SHUT,
                    &format!("The house could not be described for Formiga Home: {error}"),
                );
                return;
            }
        };
        if let Some(reason) = HOME.incompatibility(&install, snapshot.min_reader_version) {
            self.failure_dialog(STAYING_SHUT, &reason);
            return;
        }
        if let Some(name) = busy {
            self.failure_dialog(STAYING_SHUT, &format!("{name} is busy just now."));
            return;
        }
        // Asked of the keeper first, before the house was described; asked again of everyone
        // going in, since another app may have any one of them.
        if let Some(reason) = self.busy_elsewhere(&HOME, Some(&away)) {
            self.failure_dialog(STAYING_SHUT, reason);
            return;
        }
        self.finish_habitat_editor(false);
        self.close_creature_menu(MenuDismissal::Hidden);
        let desktop = self.snapshot();
        if let Some(world) = &mut self.world {
            world.begin_house_visit(&away, &desktop);
        }
        // The colony is written as the household goes in, so a crash anywhere after this loses
        // nothing.
        if let Err(error) = self.save() {
            self.let_household_out(None);
            self.failure_dialog(
                STAYING_SHUT,
                &format!("The colony could not be saved, so the house is staying shut: {error}"),
            );
            return;
        }
        let open = match self.house.files.open(&snapshot, &away) {
            Ok(open) => open,
            Err(error) => {
                self.let_household_out(None);
                self.failure_dialog(
                    STAYING_SHUT,
                    &format!("The visit could not be written: {error}"),
                );
                return;
            }
        };
        let exited = UserEvent::House(HouseEvent::Exited {
            session: open.seal.session_id.clone(),
        });
        if let Err(error) = HOME.start(&install, &open.dir, exited, self.event_proxy.clone()) {
            self.house.files.close(&open);
            self.let_household_out(None);
            self.failure_dialog(
                STAYING_SHUT,
                &format!("Formiga Home could not be opened: {error}"),
            );
            return;
        }
        tracing::info!(
            residents = snapshot.residents.len(),
            visitors = snapshot.visitors.len(),
            "a house is open in Formiga Home"
        );
        self.house.visit = VisitState::Open { open };
        self.sync_house_menu();
        self.request_overlay_redraw();
    }

    /// The owner asked for the household back before Home was done with the house. Whatever Home
    /// had arranged by then is kept, as it would have been had Home closed the house itself.
    pub(super) fn bring_household_back(&mut self) {
        let Some(open) = self.house.visit.open().cloned() else {
            return;
        };
        self.house
            .files
            .call_home(&open, RecallReason::OwnerAsked, OffsetDateTime::now_utc());
        let _ = self.read_house_answers(&open);
        tracing::info!("the household was called back out of Formiga Home");
        self.let_household_out(None);
    }

    /// The colony is about to be replaced: tell Home the visit is over, and keep nothing of it.
    pub(super) fn abandon_house_visit(&mut self) {
        let Some(open) = self.house.visit.open().cloned() else {
            return;
        };
        self.house
            .files
            .call_home(&open, RecallReason::OwnerAsked, OffsetDateTime::now_utc());
        self.let_household_out(None);
    }

    pub(super) fn handle_house_event(&mut self, event: HouseEvent) {
        let HouseEvent::Exited { session } = event;
        let Some(open) = self
            .house
            .visit
            .open()
            .filter(|open| open.seal.session_id == session)
            .cloned()
        else {
            // A visit that is already over: everyone is back out, and whatever Home left is not
            // read.
            return;
        };
        let note = self.read_house_answers(&open);
        self.house.files.close(&open);
        self.let_household_out(note);
    }

    /// What Home left for this visit, kept as far as Desktop allows, and what to tell the owner
    /// if Home would not open the house.
    fn read_house_answers(&mut self, open: &OpenVisit) -> Option<String> {
        let now = OffsetDateTime::now_utc();
        let answer = self.house.files.answer(open);
        for problem in &answer.problems {
            tracing::warn!(%problem, "an answer from Formiga Home was not used");
        }
        if let Some(result) = &answer.result {
            match self.house.files.keep_result(open, result) {
                Ok(set_aside) if set_aside.is_empty() => {}
                Ok(set_aside) => tracing::info!(?set_aside, "parts of the homes set aside"),
                Err(problem) => tracing::warn!(%problem, "the homes were not kept"),
            }
        }
        if let Some(receipt) = &answer.receipt {
            let welcome = session::welcome(open, receipt, now);
            if !welcome.set_aside.is_empty() {
                tracing::info!(set_aside = ?welcome.set_aside, "receipt effects set aside");
            }
            if welcome.visited
                && let Some(world) = &mut self.world
                && world.note_house_visit(open.keeper, now)
            {
                self.save_waiting = SaveUrgency::Prompt;
            }
            tracing::info!("the household came back out of Formiga Home");
            return None;
        }
        let refusal = answer.refusal.as_ref().map(session::refusal_text);
        tracing::info!(
            refused = refusal.is_some(),
            "the household came back out of Formiga Home without a receipt"
        );
        refusal
    }

    /// Everyone who went in comes back out where they went in, and the visit is over.
    fn let_household_out(&mut self, note: Option<String>) {
        if let Some(world) = &mut self.world {
            world.end_house_visit();
        }
        self.house.visit = VisitState::Idle;
        if let Err(error) = self.save() {
            tracing::error!(%error, "could not save the household coming back out");
        }
        self.sync_house_menu();
        self.request_overlay_redraw();
        if let Some(note) = note {
            self.failure_dialog(STAYING_SHUT, &note);
        }
    }

    /// Finish a visit Desktop stopped in the middle of. With Home's receipt the household comes
    /// back out with what it may keep; without one, Home is told the visit is over and whatever
    /// it had arranged is kept. Either way everyone comes back out now: Desktop never waits on
    /// Home to let them out.
    pub(super) fn resume_open_visit(&mut self) {
        // Whatever earlier visits left behind goes first; the open visit, if any, is kept.
        self.house.files.sweep();
        let Some(open) = self.house.files.open_visit() else {
            return;
        };
        let now = OffsetDateTime::now_utc();
        if self.recovery_pending {
            // The colony that opened the house could not be opened itself; recovery decides what
            // happens to it. Home may still be running, so it is told.
            self.house
                .files
                .call_home(&open, RecallReason::DesktopRestarted, now);
            return;
        }
        // Back indoors for as long as finishing takes, so the world leaves them be.
        let desktop = self.snapshot();
        if let Some(world) = &mut self.world {
            world.begin_house_visit(&open.away, &desktop);
        }
        if self.house.files.answer(&open).receipt.is_some() {
            let note = self.read_house_answers(&open);
            self.house.files.close(&open);
            self.let_household_out(note);
        } else {
            tracing::info!("finishing a visit after a restart, without a receipt");
            self.house
                .files
                .call_home(&open, RecallReason::DesktopRestarted, now);
            let _ = self.read_house_answers(&open);
            self.let_household_out(None);
        }
    }

    /// Whether the houses can be clicked just now: Home is installed, the colony is showing and
    /// can be handled, and nothing else has the desktop.
    fn houses_clickable(&self) -> bool {
        let Some(world) = &self.world else {
            return false;
        };
        self.house.slot.install.is_some()
            && world.save.settings.visible
            && world.save.settings.direct_manipulation
            && self.habitat_editor.is_none()
            && !self.hill.trip.holds_world()
            && !self.recovery_pending
    }

    /// The houses where they can be clicked now: none while they cannot be, or are not out.
    pub(super) fn placed_houses(&mut self) -> Vec<PlacedHouse> {
        if !self.houses_clickable() {
            return Vec::new();
        }
        let Some(world) = &self.world else {
            return Vec::new();
        };
        self.house_shapes.placed(&world.save, &self.monitors)
    }

    /// A window over each house while the houses can be clicked, and none otherwise. A house
    /// takes a click only on its own pixels, and never where a companion or the open menu is in
    /// front of it.
    pub(super) fn sync_house_proxies(&mut self, event_loop: &ActiveEventLoop) {
        let houses = self.placed_houses();
        self.house_proxies
            .retain(|_, proxy| houses.iter().any(|house| house.keeper == proxy.keeper));
        for house in &houses {
            if self
                .house_proxies
                .values()
                .any(|proxy| proxy.keeper == house.keeper)
            {
                continue;
            }
            match HouseProxy::new(event_loop, house.keeper) {
                Ok(proxy) => {
                    self.house_proxies.insert(proxy.id(), proxy);
                }
                Err(error) => tracing::error!(%error, "could not create a house proxy"),
            }
        }
        if houses.is_empty() {
            return;
        }
        let cursor = self.current_cursor;
        let in_front = cursor.available
            && (self
                .interaction_proxies
                .values()
                .any(|proxy| proxy.hit_test(cursor.position.x, cursor.position.y))
                || self
                    .creature_menu
                    .as_ref()
                    .is_some_and(|menu| menu.contains(cursor.position)));
        let Some(settings) = self.world.as_ref().map(|world| world.save.settings.clone()) else {
            return;
        };
        for proxy in self.house_proxies.values_mut() {
            let Some(house) = houses.iter().find(|house| house.keeper == proxy.keeper) else {
                continue;
            };
            let Some(monitor) = self
                .monitors
                .iter()
                .find(|monitor| monitor.id == house.monitor_id)
            else {
                proxy.hide();
                continue;
            };
            if settings.fullscreen_app_occlusion
                && monitor_has_fullscreen_window(monitor.bounds, &self.cached_windows)
            {
                proxy.hide();
                continue;
            }
            let origin = self
                .overlays
                .values()
                .find(|overlay| overlay.monitor.id == monitor.id)
                .and_then(|overlay| overlay.window.outer_position().ok())
                .unwrap_or(PhysicalPosition::new(0, 0));
            proxy.sync(
                house,
                monitor,
                origin,
                cursor,
                in_front,
                settings.display_scale,
            );
        }
    }

    /// A click on a house opens its menu, or closes it again. A companion in front of the house
    /// gets the click instead, whichever window it arrived at, and so does the rest of a drag
    /// begun on one.
    pub(super) fn handle_house_proxy_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: &WindowEvent,
    ) {
        let cursor = self.current_cursor.position;
        let creature_window = self
            .interaction_proxies
            .iter()
            .find(|(_, proxy)| proxy.hit_test(cursor.x, cursor.y))
            .map(|(id, _)| *id);
        let dragging = self.world.as_ref().is_some_and(World::is_dragging);
        match event {
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => {
                if let Some(creature_window) = creature_window {
                    self.handle_proxy_event(event_loop, creature_window, event);
                    return;
                }
                if !matches!(button, MouseButton::Left | MouseButton::Right) {
                    return;
                }
                let keeper = self
                    .house_proxies
                    .get(&window_id)
                    .filter(|proxy| proxy.hit_test(cursor))
                    .map(|proxy| proxy.keeper);
                if let Some(keeper) = keeper {
                    self.toggle_house_menu(event_loop, keeper);
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                ..
            } if dragging => {
                if let Some(any) =
                    creature_window.or_else(|| self.interaction_proxies.keys().next().copied())
                {
                    self.handle_proxy_event(event_loop, any, event);
                }
            }
            _ => {}
        }
    }

    /// Open the menu of the house `keeper` keeps, or close it if it is the one open.
    pub(super) fn toggle_house_menu(&mut self, event_loop: &ActiveEventLoop, keeper: CreatureId) {
        if self
            .creature_menu
            .as_ref()
            .is_some_and(|menu| menu.target() == MenuTarget::House && menu.creature_id() == keeper)
        {
            self.close_creature_menu(MenuDismissal::Answered);
            return;
        }
        let Some(house) = self
            .placed_houses()
            .into_iter()
            .find(|house| house.keeper == keeper)
        else {
            return;
        };
        let settings_focused = self
            .settings_window
            .as_ref()
            .is_some_and(|window| window.window.has_focus());
        let open_here = self
            .house
            .visit
            .open()
            .is_some_and(|open| open.keeper == keeper);
        let menu = CreatureMenu::for_house(keeper, open_here, house.monitor_id, settings_focused);
        if !self.ensure_menu_proxy(event_loop) {
            return;
        }
        self.creature_menu = Some(menu);
        self.menus_opened += 1;
        // Placed and shown at once, under the click that asked for it.
        self.sync_creature_menu(0.0);
    }

    /// Keep a house's menu over its roof, and close it when the house can no longer be clicked:
    /// the same rules as a creature's.
    pub(super) fn advance_house_menu(
        &mut self,
        menu: &mut CreatureMenu,
        dt: f32,
        settings_focused: bool,
    ) -> std::result::Result<bool, MenuDismissal> {
        let house = self
            .placed_houses()
            .into_iter()
            .find(|house| house.keeper == menu.creature_id())
            .ok_or(MenuDismissal::Gone)?;
        let world = self.world.as_ref().ok_or(MenuDismissal::Gone)?;
        let settings = &world.save.settings;
        let occluded = self
            .monitors
            .iter()
            .find(|monitor| monitor.id == house.monitor_id)
            .is_some_and(|monitor| {
                settings.fullscreen_app_occlusion
                    && monitor_has_fullscreen_window(monitor.bounds, &self.cached_windows)
            });
        if let Some(reason) = menu.interruption(MenuWorld {
            present: true,
            monitor_id: house.monitor_id,
            handled: false,
            hidden: !settings.visible || !settings.direct_manipulation,
            occluded,
            settings_focused,
        }) {
            return Err(reason);
        }
        let anchor = self
            .overlays
            .values()
            .find(|overlay| overlay.monitor.id == house.monitor_id)
            .map(|overlay| overlay.house_menu_anchor(&house, settings.display_scale));
        menu.attach(anchor);
        let cursor = self
            .current_cursor
            .available
            .then_some(self.current_cursor.position);
        let tick = menu.track(dt, cursor, house.centre());
        match tick.dismissal {
            Some(reason) => Err(reason),
            None => Ok(tick.redraw),
        }
    }

    /// What a house's menu does: look inside it, bring its household back out of it, or turn the
    /// notebook to it.
    pub(super) fn choose_house_item(
        &mut self,
        event_loop: &ActiveEventLoop,
        keeper: CreatureId,
        icon: MenuIcon,
    ) {
        match icon {
            MenuIcon::Inside => self.open_house(keeper),
            MenuIcon::Stop => self.bring_household_back(),
            MenuIcon::Profile => {
                self.show_settings(event_loop);
                if let Some(window) = &mut self.settings_window {
                    window.select_house(keeper);
                }
            }
            _ => {}
        }
    }
}
