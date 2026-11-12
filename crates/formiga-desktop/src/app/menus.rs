//! The creature menu: opening it over a creature, following that creature, the small native window
//! that takes its clicks, and what each item does.
use super::*;

impl FormigaApp {
    /// A secondary click on a creature: open its menu, or close the one it already has. Only one
    /// menu is ever open, so asking for another creature's simply replaces it.
    pub(super) fn toggle_creature_menu(&mut self, event_loop: &ActiveEventLoop) {
        let cursor = self.current_cursor.position;
        let hits: Vec<_> = self
            .interaction_proxies
            .iter()
            .filter(|(_, proxy)| proxy.hit_test(cursor.x, cursor.y))
            .map(|(id, proxy)| (*id, proxy.creature_id))
            .collect();
        let draw_order = self.draw_order();
        let Some((_, creature_id)) = resolve_press_target(&hits, &draw_order) else {
            return;
        };
        if self
            .creature_menu
            .as_ref()
            .is_some_and(|menu| menu.creature_id() == creature_id)
        {
            self.close_creature_menu(MenuDismissal::Answered);
            return;
        }
        self.open_creature_menu(event_loop, creature_id);
    }

    pub(super) fn open_creature_menu(
        &mut self,
        event_loop: &ActiveEventLoop,
        creature_id: CreatureId,
    ) {
        let Some(world) = &self.world else { return };
        let guest = world
            .save
            .visitors
            .on_stage()
            .is_some_and(|guest| guest.id == creature_id);
        let Some(creature) = world
            .save
            .creatures
            .iter()
            .chain(world.save.visitors.on_stage())
            .find(|creature| creature.id == creature_id)
        else {
            return;
        };
        // A creature that is being carried, or that cannot be seen, has nothing to offer a menu.
        if matches!(
            creature.state.action,
            ActionKind::Dragged | ActionKind::Tossed
        ) || menu_hidden(
            creature,
            &world.save.settings,
            self.habitat_editor.is_some(),
        ) {
            return;
        }
        let monitor_id = creature.state.surface.monitor_id;
        let target = if guest {
            MenuTarget::Guest
        } else {
            MenuTarget::Member
        };
        let can_stay = world.visitor_can_stay();
        // Whether the settings window has focus *now* is remembered, so only it taking focus
        // later closes the menu. Opening one while the settings window happens to be up is fine.
        let settings_focused = self
            .settings_window
            .as_ref()
            .is_some_and(|window| window.window.has_focus());
        // While the houses are out, the cell that would send everyone home offers the moments the
        // village could share instead.
        let offer_moments = !guest
            && (world.village_moment().is_some() || !world.available_village_moments().is_empty());
        let mut menu =
            CreatureMenu::new(creature_id, target, can_stay, monitor_id, settings_focused);
        if offer_moments {
            menu = menu.offering_moments();
        } else if !guest && sends_out(world) {
            menu = menu.offering_out();
        }
        if !self.ensure_menu_proxy(event_loop) {
            return;
        }
        self.creature_menu = Some(menu);
        self.menus_opened += 1;
        // Place and show it now rather than on the next tick, so the strip appears under the
        // click that asked for it instead of up to a frame later.
        self.sync_creature_menu(0.0);
    }

    /// The small native window that takes clicks on whichever menu is open, made the first time a
    /// menu is. Says whether there is one.
    pub(super) fn ensure_menu_proxy(&mut self, event_loop: &ActiveEventLoop) -> bool {
        if self.menu_proxy.is_none() {
            match MenuProxy::new(event_loop) {
                Ok(proxy) => self.menu_proxy = Some(proxy),
                Err(error) => {
                    tracing::error!(%error, "could not create the creature menu proxy");
                    return false;
                }
            }
        }
        true
    }

    pub(super) fn close_creature_menu(&mut self, reason: MenuDismissal) {
        if self.creature_menu.take().is_none() {
            return;
        }
        tracing::debug!(?reason, "creature menu closed");
        if let Some(proxy) = &mut self.menu_proxy {
            proxy.hide();
        }
        self.request_overlay_redraw();
    }

    /// Keep the open menu attached to its creature, follow the cursor across its cells, and close
    /// it when any of the dismissal rules fires. `creature_menu` owns every rule; this only feeds
    /// it the world and acts on the answer.
    pub(super) fn sync_creature_menu(&mut self, dt: f32) {
        let Some(mut menu) = self.creature_menu.take() else {
            return;
        };
        let outcome = self.advance_creature_menu(&mut menu, dt);
        match outcome {
            Ok(redraw) => {
                self.creature_menu = Some(menu);
                if let Some(area) = self
                    .creature_menu
                    .as_ref()
                    .and_then(|menu| menu.click_area())
                {
                    self.sync_menu_proxy(area);
                } else if let Some(proxy) = &mut self.menu_proxy {
                    // Nowhere to put the strip this frame: take the click target away with it.
                    proxy.hide();
                }
                if redraw {
                    self.request_overlay_redraw();
                }
            }
            Err(reason) => {
                self.creature_menu = Some(menu);
                self.close_creature_menu(reason);
            }
        }
    }

    pub(super) fn advance_creature_menu(
        &mut self,
        menu: &mut CreatureMenu,
        dt: f32,
    ) -> std::result::Result<bool, MenuDismissal> {
        let settings_focused = self
            .settings_window
            .as_ref()
            .is_some_and(|window| window.window.has_focus());
        if menu.target() == MenuTarget::House {
            return self.advance_house_menu(menu, dt, settings_focused);
        }
        if menu.target() == MenuTarget::Wonder {
            return self.advance_wonder_menu(menu, dt, settings_focused);
        }
        let (anchor, position) = {
            let world = self.world.as_ref().ok_or(MenuDismissal::Gone)?;
            let settings = &world.save.settings;
            let creature = world
                .save
                .creatures
                .iter()
                .chain(world.save.visitors.on_stage())
                .find(|creature| creature.id == menu.creature_id())
                .ok_or(MenuDismissal::Gone)?;
            let monitor_id = creature.state.surface.monitor_id;
            let occluded = self
                .monitors
                .iter()
                .find(|monitor| monitor.id == monitor_id)
                .is_some_and(|monitor| {
                    settings.fullscreen_app_occlusion
                        && monitor_has_fullscreen_window(monitor.bounds, &self.cached_windows)
                });
            if let Some(reason) = menu.interruption(MenuWorld {
                present: true,
                monitor_id,
                handled: matches!(
                    creature.state.action,
                    ActionKind::Dragged | ActionKind::Tossed
                ),
                hidden: menu_hidden(creature, settings, self.habitat_editor.is_some()),
                occluded,
                settings_focused,
            }) {
                return Err(reason);
            }
            let anchor = self
                .overlays
                .values()
                .find(|overlay| overlay.monitor.id == monitor_id)
                .and_then(|overlay| overlay.menu_anchor(creature, settings.display_scale));
            (anchor, creature.state.position)
        };
        menu.attach(anchor);
        let cursor = self
            .current_cursor
            .available
            .then_some(self.current_cursor.position);
        let tick = menu.track(dt, cursor, position);
        match tick.dismissal {
            Some(reason) => Err(reason),
            None => Ok(tick.redraw),
        }
    }

    pub(super) fn sync_menu_proxy(&mut self, body: DesktopRect) {
        let Some(monitor) = self
            .creature_menu
            .as_ref()
            .and_then(|menu| {
                let id = menu.monitor_id();
                self.monitors.iter().find(|monitor| monitor.id == id)
            })
            .cloned()
        else {
            return;
        };
        let origin = self
            .overlays
            .values()
            .find(|overlay| overlay.monitor.id == monitor.id)
            .and_then(|overlay| overlay.window.outer_position().ok())
            .unwrap_or(PhysicalPosition::new(0, 0));
        if let Some(proxy) = &mut self.menu_proxy {
            proxy.sync(body, &monitor, origin);
        }
    }

    pub(super) fn handle_menu_proxy_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: &WindowEvent,
    ) {
        if let WindowEvent::MouseInput {
            state: ElementState::Pressed,
            button,
            ..
        } = event
        {
            let cursor = self.current_cursor.position;
            match button {
                // Right-clicking the strip is another way of saying "never mind".
                MouseButton::Right => self.close_creature_menu(MenuDismissal::Answered),
                MouseButton::Left => {
                    let chosen = self
                        .creature_menu
                        .as_ref()
                        .and_then(|menu| menu.item_at(cursor));
                    // A press on the frame or in a gap between cells is not a choice, and the
                    // menu stays open rather than closing under a near miss.
                    if let Some(icon) = chosen {
                        self.choose_menu_item(event_loop, icon);
                    }
                }
                _ => {}
            }
        }
    }

    /// Act on a menu choice. Choosing Moment opens or closes the strip of moments beside the menu,
    /// and the menu stays. So does a snack or a toy the companion cannot take just now, which
    /// says "not now" under it. Anything else closes it: the simulation answers with its own
    /// thought bubbles, including when a creature is busy and politely declines.
    pub(super) fn choose_menu_item(&mut self, event_loop: &ActiveEventLoop, icon: MenuIcon) {
        if icon == MenuIcon::Moment {
            let items = self.moment_items();
            if let Some(menu) = &mut self.creature_menu {
                menu.toggle_side(&items);
            }
            // Placed and made clickable at once, under the click that asked for it.
            self.sync_creature_menu(0.0);
            self.request_overlay_redraw();
            return;
        }
        let Some(menu) = self.creature_menu.as_ref() else {
            return;
        };
        let creature_id = menu.creature_id();
        let target = menu.target();
        // A snack or a toy held out to a companion that can no longer take it (it has started a
        // climb, set off on a journey, gone up to a wonder) is not an answer: the strip stays and
        // says so, rather than closing on a click that seemed to do nothing. A companion that
        // can take it, and turns it down, answers in its own bubble as ever.
        if target == MenuTarget::Wonder {
            if let Some(kind) = crate::creature_menu::wonder_of_menu(creature_id) {
                self.choose_wonder_item(event_loop, kind, icon);
            }
            return;
        }
        if target != MenuTarget::House && matches!(icon, MenuIcon::Snack | MenuIcon::Toy) {
            let desktop = self.snapshot();
            let command = if icon == MenuIcon::Snack {
                WorldCommand::OfferSnack { creature_id }
            } else {
                WorldCommand::OfferToy { creature_id }
            };
            let offered = self
                .world
                .as_mut()
                .is_some_and(|world| world.handle_command(command, &desktop));
            if offered {
                self.close_creature_menu(MenuDismissal::Answered);
                let _ = self.save();
            } else if let Some(menu) = &mut self.creature_menu {
                menu.mark_unavailable(icon);
                self.request_overlay_redraw();
            }
            return;
        }
        self.close_creature_menu(MenuDismissal::Answered);
        if target == MenuTarget::House {
            self.choose_house_item(event_loop, creature_id, icon);
            return;
        }
        let desktop = self.snapshot();
        match icon {
            MenuIcon::Home
            | MenuIcon::Picnic
            | MenuIcon::Dance
            | MenuIcon::Nap
            | MenuIcon::Stop => {
                let command = match icon {
                    MenuIcon::Picnic => WorldCommand::InviteVillageMoment {
                        creature_id,
                        moment: VillageMoment::Picnic,
                    },
                    MenuIcon::Dance => WorldCommand::InviteVillageMoment {
                        creature_id,
                        moment: VillageMoment::Dance,
                    },
                    MenuIcon::Nap => WorldCommand::InviteVillageMoment {
                        creature_id,
                        moment: VillageMoment::Nap,
                    },
                    MenuIcon::Stop => WorldCommand::StopVillageMoment,
                    _ => WorldCommand::SendHome,
                };
                if let Some(world) = &mut self.world {
                    world.handle_command(command, &desktop);
                }
            }
            MenuIcon::Out => {
                if let Some(world) = &mut self.world {
                    world.handle_command(WorldCommand::SendOut, &desktop);
                }
            }
            // Opens the strip of moments instead, and snacks and toys are held out above; only a
            // house is looked inside, and only a wonder is played on, moved or put away.
            MenuIcon::Moment
            | MenuIcon::Inside
            | MenuIcon::Snack
            | MenuIcon::Toy
            | MenuIcon::Play
            | MenuIcon::Move
            | MenuIcon::PutAway => {}
            MenuIcon::Profile => {
                self.show_settings(event_loop);
                if let Some(window) = &mut self.settings_window {
                    match target {
                        MenuTarget::Member => window.select_creature(creature_id),
                        // A guest has no colony profile of its own, so its page is the journal,
                        // where its visit is written down.
                        MenuTarget::Guest => window.select_journal(),
                        MenuTarget::House => window.select_house(creature_id),
                        MenuTarget::Wonder => window.select_journal(),
                    }
                }
            }
            MenuIcon::Stay => {
                let result = self
                    .world
                    .as_mut()
                    .map(|world| world.ask_visitor_to_stay(OffsetDateTime::now_utc(), &desktop));
                if let Some(Err(error)) = result {
                    tracing::warn!(
                        reason = colony_management_category(&error),
                        "the visitor could not be asked to stay"
                    );
                }
            }
            MenuIcon::CopyCode => {
                let code = self
                    .world
                    .as_ref()
                    .and_then(|world| world.visitor_share_code());
                if let Some(code) = code {
                    // egui owns the clipboard, and it only hands text over from inside one of its
                    // own frames, so this goes through the very window the Colony page's copy
                    // buttons use — which also gives the owner something that says it worked.
                    self.show_settings(event_loop);
                    if let Some(window) = &mut self.settings_window {
                        window.select_journal();
                        window.copy_text(code, "Visitor code copied");
                    }
                }
            }
        }
        let _ = self.save();
    }

    /// What the Moment item offers: a way to stop the moment under way, if there is one, and then
    /// every moment the village could share right now.
    pub(super) fn moment_items(&self) -> Vec<MenuIcon> {
        let Some(world) = &self.world else {
            return Vec::new();
        };
        world
            .village_moment()
            .map(|_| MenuIcon::Stop)
            .into_iter()
            .chain(
                world
                    .available_village_moments()
                    .into_iter()
                    .map(|moment| match moment {
                        VillageMoment::Picnic => MenuIcon::Picnic,
                        VillageMoment::Dance => MenuIcon::Dance,
                        VillageMoment::Nap => MenuIcon::Nap,
                    }),
            )
            // A village that stays out offers the way back out of a visit beside its moments.
            .chain((world.village_moment().is_none() && sends_out(world)).then_some(MenuIcon::Out))
            .collect()
    }
}

/// Whether a creature's menu has nothing to be drawn over: the colony is hidden or cannot be
/// handled, the habitat editor is open, or the creature is not out to be seen, still on its way
/// or gone inside its house. The overlay leaves out a menu whose creature it does not draw (see
/// `gpu::occlusion::drawn_on_monitor`), so the menu closes then too, and its click target goes
/// with it rather than staying over an empty strip of desktop.
pub(super) fn menu_hidden(creature: &Creature, settings: &Settings, editing_habitat: bool) -> bool {
    !settings.visible
        || !settings.direct_manipulation
        || editing_habitat
        || creature.state.arrival_delay_secs > 0.0
        || creature.state.indoors
}

/// Whether a companion's menu offers to send everyone back out: during a visit home, while the
/// village stays on the desktop.
pub(super) fn sends_out(world: &World) -> bool {
    world.save.home.stays_out && world.save.home.is_active()
}
