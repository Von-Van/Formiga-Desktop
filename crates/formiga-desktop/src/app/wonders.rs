//! The app's side of the wonders the person at the desk puts out: a window over each that takes a
//! click on it, the menu a click opens, what each item does, and moving one — the wonder follows
//! the pointer until the next click sets it down, or a right click leaves it where it was.

use super::*;
use crate::creature_menu::{wonder_menu_id, wonder_of_menu};
use crate::wonder_proxies::{PlacedWonderShape, WONDER_SQUARE, WonderProxy};
use formiga_art::{WONDER_CELL_HEIGHT, WONDER_GROUND};

impl FormigaApp {
    /// Whether the wonders put out can be clicked just now: the colony is showing and can be
    /// handled, and nothing else has the desktop.
    fn wonders_clickable(&self) -> bool {
        let Some(world) = &self.world else {
            return false;
        };
        world.save.settings.visible
            && world.save.settings.direct_manipulation
            && self.habitat_editor.is_none()
            && !self.colony_away()
            && !self.recovery_pending
    }

    /// The wonders put out where they can be clicked now: none while they cannot be.
    pub(super) fn placed_wonder_shapes(&mut self) -> Vec<PlacedWonderShape> {
        if !self.wonders_clickable() {
            return Vec::new();
        }
        let Some(world) = &self.world else {
            return Vec::new();
        };
        let views = world.placed_wonders();
        self.wonder_shapes.placed(
            &views,
            &self.monitors,
            world.save.settings.display_scale,
            world.save.colony_seed,
        )
    }

    /// What the overlay draws for the wonder being moved, if one is: the same wonder hanging
    /// under the pointer, on the display the pointer is over.
    pub(super) fn moving_wonder_view(
        &self,
        mut view: formiga_core::WonderView,
    ) -> formiga_core::WonderView {
        if self.moving_wonder != Some(view.kind) || !self.current_cursor.available {
            return view;
        }
        let cursor = self.current_cursor.position;
        let Some(monitor) = self
            .monitors
            .iter()
            .find(|monitor| monitor.bounds.contains(cursor))
        else {
            return view;
        };
        let scale = self
            .world
            .as_ref()
            .map_or(1, |world| world.save.settings.display_scale);
        let point = f32::from(scale) / monitor.scale_factor;
        let side = WONDER_SQUARE as f32 * point;
        let below = (WONDER_CELL_HEIGHT as i32 - WONDER_GROUND) as f32 * point;
        view.monitor_id = monitor.id;
        view.window_key = None;
        view.at = Point {
            x: cursor.x,
            y: cursor.y + side / 2.0 - below,
        };
        view
    }

    /// A window over each wonder put out while they can be clicked, and none otherwise. A wonder
    /// takes a click only on its own pixels, and never where a companion or the open menu is in
    /// front of it; the one being moved takes the next click anywhere near the pointer.
    pub(super) fn sync_wonder_proxies(&mut self, event_loop: &ActiveEventLoop) {
        let wonders = self.placed_wonder_shapes();
        if self
            .moving_wonder
            .is_some_and(|kind| !wonders.iter().any(|wonder| wonder.kind == kind))
        {
            self.moving_wonder = None;
        }
        self.wonder_proxies
            .retain(|_, proxy| wonders.iter().any(|wonder| wonder.kind == proxy.kind));
        for wonder in &wonders {
            if self
                .wonder_proxies
                .values()
                .any(|proxy| proxy.kind == wonder.kind)
            {
                continue;
            }
            match WonderProxy::new(event_loop, wonder.kind) {
                Ok(proxy) => {
                    self.wonder_proxies.insert(proxy.id(), proxy);
                }
                Err(error) => tracing::error!(%error, "could not create a wonder proxy"),
            }
        }
        if wonders.is_empty() {
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
        let moving = self.moving_wonder;
        for proxy in self.wonder_proxies.values_mut() {
            let Some(wonder) = wonders.iter().find(|wonder| wonder.kind == proxy.kind) else {
                continue;
            };
            let following = moving == Some(proxy.kind);
            // While it follows the pointer it is over whichever display the pointer is on.
            let monitor_id = if following && cursor.available {
                self.monitors
                    .iter()
                    .find(|monitor| monitor.bounds.contains(cursor.position))
                    .map_or(wonder.monitor_id, |monitor| monitor.id)
            } else {
                wonder.monitor_id
            };
            let Some(monitor) = self
                .monitors
                .iter()
                .find(|monitor| monitor.id == monitor_id)
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
                wonder,
                monitor,
                origin,
                cursor,
                in_front && !following,
                settings.display_scale,
                following,
            );
        }
    }

    /// A click on a wonder opens its menu, or closes it again; while one is being moved, a click
    /// sets it down there and a right click leaves it where it was. A companion in front of the
    /// wonder gets the click instead, and so does the rest of a drag begun on one.
    pub(super) fn handle_wonder_proxy_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: &WindowEvent,
    ) {
        let cursor = self.current_cursor.position;
        let Some(kind) = self.wonder_proxies.get(&window_id).map(|proxy| proxy.kind) else {
            return;
        };
        if self.moving_wonder == Some(kind) {
            if let WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } = event
            {
                self.moving_wonder = None;
                if *button == MouseButton::Left {
                    let desktop = self.snapshot();
                    if let Some(world) = &mut self.world {
                        world.handle_command(
                            WorldCommand::MoveWonder { kind, to: cursor },
                            &desktop,
                        );
                    }
                    let _ = self.save();
                }
                self.request_overlay_redraw();
            }
            return;
        }
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
                let on_it = self
                    .wonder_proxies
                    .get(&window_id)
                    .is_some_and(|proxy| proxy.hit_test(cursor));
                if on_it {
                    self.toggle_wonder_menu(event_loop, kind);
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

    /// Open the menu of a wonder put out, or close it if it is the one open.
    fn toggle_wonder_menu(&mut self, event_loop: &ActiveEventLoop, kind: WonderKind) {
        if self.creature_menu.as_ref().is_some_and(|menu| {
            menu.target() == MenuTarget::Wonder && menu.creature_id() == wonder_menu_id(kind)
        }) {
            self.close_creature_menu(MenuDismissal::Answered);
            return;
        }
        let Some(wonder) = self
            .placed_wonder_shapes()
            .into_iter()
            .find(|wonder| wonder.kind == kind)
        else {
            return;
        };
        let settings_focused = self
            .settings_window
            .as_ref()
            .is_some_and(|window| window.window.has_focus());
        let menu = CreatureMenu::for_wonder(kind, wonder.monitor_id, settings_focused);
        if !self.ensure_menu_proxy(event_loop) {
            return;
        }
        self.creature_menu = Some(menu);
        self.menus_opened += 1;
        self.sync_creature_menu(0.0);
    }

    /// Keep a wonder's menu over it, and close it when the wonder can no longer be clicked: the
    /// same rules as a house's.
    pub(super) fn advance_wonder_menu(
        &mut self,
        menu: &mut CreatureMenu,
        dt: f32,
        settings_focused: bool,
    ) -> std::result::Result<bool, MenuDismissal> {
        let kind = wonder_of_menu(menu.creature_id()).ok_or(MenuDismissal::Gone)?;
        let wonder = self
            .placed_wonder_shapes()
            .into_iter()
            .find(|wonder| wonder.kind == kind)
            .ok_or(MenuDismissal::Gone)?;
        let world = self.world.as_ref().ok_or(MenuDismissal::Gone)?;
        let settings = &world.save.settings;
        let occluded = self
            .monitors
            .iter()
            .find(|monitor| monitor.id == wonder.monitor_id)
            .is_some_and(|monitor| {
                settings.fullscreen_app_occlusion
                    && monitor_has_fullscreen_window(monitor.bounds, &self.cached_windows)
            });
        if let Some(reason) = menu.interruption(MenuWorld {
            present: true,
            monitor_id: wonder.monitor_id,
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
            .find(|overlay| overlay.monitor.id == wonder.monitor_id)
            .map(|overlay| overlay.wonder_menu_anchor(&wonder, settings.display_scale));
        menu.attach(anchor);
        let cursor = self
            .current_cursor
            .available
            .then_some(self.current_cursor.position);
        let tick = menu.track(dt, cursor, wonder.centre());
        match tick.dismissal {
            Some(reason) => Err(reason),
            None => Ok(tick.redraw),
        }
    }

    /// What a wonder's menu does. A go asked for that nobody can have just now keeps the menu
    /// open and says "not now" under it, as a snack nobody can take does.
    pub(super) fn choose_wonder_item(
        &mut self,
        event_loop: &ActiveEventLoop,
        kind: WonderKind,
        icon: MenuIcon,
    ) {
        let desktop = self.snapshot();
        match icon {
            MenuIcon::Play => {
                let started = self.world.as_mut().is_some_and(|world| {
                    world.handle_command(WorldCommand::PlayOnWonder { kind }, &desktop)
                });
                if !started {
                    if let Some(menu) = &mut self.creature_menu {
                        menu.mark_unavailable(icon);
                        self.request_overlay_redraw();
                    }
                    return;
                }
            }
            MenuIcon::Move => {
                if let Some(world) = &mut self.world {
                    // Its players are let go where they stand before it is lifted.
                    if world.placed_wonder_in_use(kind) {
                        world.handle_command(
                            WorldCommand::MoveWonder {
                                kind,
                                to: world
                                    .placed_wonders()
                                    .iter()
                                    .find(|view| view.kind == kind)
                                    .map_or(self.current_cursor.position, |view| view.at),
                            },
                            &desktop,
                        );
                    }
                }
                self.moving_wonder = Some(kind);
            }
            MenuIcon::PutAway => {
                if let Some(world) = &mut self.world {
                    world.handle_command(WorldCommand::PutAwayWonder { kind }, &desktop);
                }
            }
            MenuIcon::Profile => {
                self.show_settings(event_loop);
                if let Some(window) = &mut self.settings_window {
                    window.select_journal();
                }
            }
            _ => {}
        }
        self.close_creature_menu(MenuDismissal::Answered);
        let _ = self.save();
        self.request_overlay_redraw();
    }
}
