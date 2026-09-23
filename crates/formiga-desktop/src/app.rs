use crate::card_export::{
    choose_card_destination, choose_colony_card_destination, choose_postcard_destination,
    export_colony_card_to_selected_destination, export_postcard_to_selected_destination,
    export_to_selected_destination,
};
use crate::creature_menu::{CreatureMenu, MenuDismissal, MenuTarget, MenuWorld};
use crate::gpu::{
    MenuView, OverlayRenderer, OverlayUi, SideView, VillageScene, monitor_has_fullscreen_window,
};
use crate::interaction::{InteractionProxy, MenuProxy, ProxyRuntimeState};
use crate::platform;
use crate::reference_match::match_reference_file;
use crate::settings::{
    ColonyView, GenerationPreview, PreviewAcceptance, SettingsOutcome, SettingsWindow,
};
use crate::sticker_export::{
    choose_sticker_destination,
    export_to_selected_destination as export_sticker_to_selected_destination,
};
use crate::tray::{TrayAction, TrayState};
use crate::updater::{
    DownloadedUpdate, UpdateController, UpdateRelease, UpdateStatus, check_github, download_update,
};
use anyhow::{Context, Result};
use formiga_art::{AnimationSpec, BodyPresentation, MenuIcon};
use formiga_core::*;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use time::OffsetDateTime;
use tray_icon::menu::MenuEvent;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId, WindowLevel};
mod cadence;
mod habitat_editor;
mod menus;
mod settings_window;
mod updates;
use cadence::*;
use habitat_editor::*;

#[derive(Debug)]
pub enum UserEvent {
    Menu(MenuEvent),
    Update(UpdateEvent),
}

#[derive(Debug)]
pub enum UpdateEvent {
    CheckFinished(std::result::Result<Option<UpdateRelease>, String>),
    DownloadFinished(std::result::Result<DownloadedUpdate, String>),
}

#[derive(Clone, Debug)]
struct MilestoneNotice {
    creature_id: CreatureId,
    expires_at: Instant,
}

pub struct FormigaApp {
    overlays: BTreeMap<WindowId, OverlayRenderer>,
    monitors: Vec<MonitorInfo>,
    world: Option<World>,
    tray: Option<TrayState>,
    save_store: SaveStore,
    previous_cursor: Option<(Point, Instant)>,
    last_tick: Instant,
    last_save: Instant,
    /// The most urgent thing waiting to be written since the last save.
    save_waiting: SaveUrgency,
    redraw_due: Instant,
    cached_windows: Vec<DesktopWindow>,
    last_window_scan: Instant,
    observation_epoch: Instant,
    window_sample: Option<WindowSample>,
    window_scan_initialized: bool,
    last_display_scan: Instant,
    log_dir: PathBuf,
    current_cursor: CursorSnapshot,
    settings_window: Option<SettingsWindow>,
    interaction_proxies: BTreeMap<WindowId, InteractionProxy>,
    /// The one open right-click menu, and the small native window that takes clicks on it. The
    /// window outlives any single menu so that closing one never destroys the window whose own
    /// click is being handled.
    creature_menu: Option<CreatureMenu>,
    menu_proxy: Option<MenuProxy>,
    habitat_editor: Option<HabitatEditor>,
    event_proxy: EventLoopProxy<UserEvent>,
    updates: UpdateController,
    milestone_notice: Option<MilestoneNotice>,
    recovery_pending: bool,
    /// Whether it is dark out where the owner is, and when that was last looked at. The local
    /// clock is read at most once a minute rather than every frame.
    night: bool,
    night_checked: Option<Instant>,
}

impl FormigaApp {
    pub fn new(log_dir: PathBuf, event_proxy: EventLoopProxy<UserEvent>) -> Result<Self> {
        let data_dir = crate::data_dir()?;
        let updates = UpdateController::load(&data_dir);
        Ok(Self {
            overlays: BTreeMap::new(),
            monitors: Vec::new(),
            world: None,
            tray: None,
            save_store: SaveStore::new(data_dir.join("colony.json")),
            previous_cursor: None,
            last_tick: Instant::now(),
            last_save: Instant::now(),
            save_waiting: SaveUrgency::None,
            redraw_due: Instant::now(),
            cached_windows: Vec::new(),
            last_window_scan: Instant::now() - Duration::from_secs(2),
            observation_epoch: Instant::now(),
            window_sample: None,
            window_scan_initialized: false,
            last_display_scan: Instant::now() - Duration::from_secs(2),
            log_dir,
            current_cursor: CursorSnapshot::default(),
            settings_window: None,
            interaction_proxies: BTreeMap::new(),
            creature_menu: None,
            menu_proxy: None,
            habitat_editor: None,
            event_proxy,
            updates,
            milestone_notice: None,
            recovery_pending: false,
            night: false,
            night_checked: None,
        })
    }

    fn initialize(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        if !self.overlays.is_empty() {
            return Ok(());
        }
        self.create_overlays(event_loop)?;
        let desktop = self.snapshot();
        self.current_cursor = desktop.cursor;
        let now = OffsetDateTime::now_utc();
        let mut recovery_reason = None;
        let (world, first_launch) = match self.save_store.load() {
            Ok(Some(save)) => (World::from_save(save), false),
            Ok(None) => (World::new(new_colony_seed()?, now, &desktop), true),
            Err(error) => {
                tracing::error!(%error, "save could not be loaded; preserving files for recovery");
                self.recovery_pending = true;
                recovery_reason = Some(error.to_string());
                (World::new(new_colony_seed()?, now, &desktop), true)
            }
        };
        self.tray = Some(TrayState::new(&world.save.settings)?);
        self.world = Some(world);
        self.sync_overlay_visibility();
        self.save()?;
        if first_launch {
            self.show_settings(event_loop);
            if let Some(window) = &mut self.settings_window {
                window.clubhouse.recovery = recovery_reason;
            }
        }
        if self
            .updates
            .should_check_automatically(OffsetDateTime::now_utc())
        {
            self.start_update_check();
        }
        Ok(())
    }

    fn create_overlays(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        self.sync_overlays(event_loop)?;
        anyhow::ensure!(
            !self.overlays.is_empty(),
            "no desktop monitors are available"
        );
        Ok(())
    }

    fn sync_overlays(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let primary = event_loop.primary_monitor();
        let mut discovered = Vec::new();
        for monitor in event_loop.available_monitors() {
            let position = monitor.position();
            let size = monitor.size();
            let scale = monitor.scale_factor() as f32;
            let primary_monitor = primary.as_ref().is_some_and(|candidate| {
                candidate.position() == position && candidate.size() == size
            });
            let id = monitor_id(position, size, scale, monitor.name().as_deref());
            let bounds = platform::canonical_monitor_bounds(
                position.x,
                position.y,
                size.width,
                size.height,
                scale,
            );
            let info = MonitorInfo {
                id,
                display_key: platform::display_key(&monitor),
                bounds,
                usable_bounds: colony_bounds(bounds, platform::work_area_insets(&monitor)),
                scale_factor: scale,
                primary: primary_monitor,
            };
            discovered.push((info, position, size));
        }

        self.overlays.retain(|_, overlay| {
            discovered
                .iter()
                .any(|(info, _, _)| info.id == overlay.monitor.id)
        });
        for (info, position, size) in &discovered {
            if let Some(overlay) = self
                .overlays
                .values_mut()
                .find(|overlay| overlay.monitor.id == info.id)
            {
                overlay.monitor = info.clone();
                continue;
            }
            let attributes = Window::default_attributes()
                .with_title("Formiga Desktop Ecosystem")
                .with_position(*position)
                .with_inner_size(*size)
                .with_resizable(false)
                .with_decorations(false)
                .with_transparent(true)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_active(false)
                .with_visible(false);
            let window = Arc::new(
                event_loop
                    .create_window(attributes)
                    .context("create desktop overlay")?,
            );
            platform::configure_native_overlay(&window, false);
            let renderer = pollster::block_on(OverlayRenderer::new(window, info.clone()))?;
            self.overlays.insert(renderer.window.id(), renderer);
        }
        let monitors: Vec<_> = discovered.into_iter().map(|(info, _, _)| info).collect();
        if monitors != self.monitors {
            self.cached_windows.clear();
            self.window_scan_initialized = false;
        }
        self.monitors = monitors;
        self.last_display_scan = Instant::now();
        self.sync_overlay_visibility();
        Ok(())
    }

    fn sync_overlay_visibility(&mut self) {
        let Some(world) = &self.world else { return };
        let settings = &world.save.settings;
        let editor_active = self.habitat_editor.is_some();
        for overlay in self.overlays.values_mut() {
            overlay.set_hittest_enabled(editor_active);
            // Full-screen occlusion is applied while rendering, not by ordering the native window
            // out. A full-screen app owns its own Space, but one overlay window is shared by every
            // Space, so hiding it here blanked the colony everywhere and it only came back on
            // whichever Space happened to be active when it was ordered front again.
            let enabled = settings.visible
                && !accessible_regions(&settings.habitat, &overlay.monitor).is_empty();
            overlay.set_visible(enabled);
        }
    }

    fn tick_interval(&self) -> Duration {
        let Some(world) = &self.world else {
            return Duration::from_millis(250);
        };
        if world.is_interacting() {
            return Duration::from_millis(50);
        }
        // An open menu is sampling the cursor for its hover highlight, so it needs the same
        // cadence a drag does. It is the owner who opened it and it closes itself within eight
        // seconds, so this is bounded and always something the person at the desk asked for.
        if self.creature_menu.is_some() {
            return Duration::from_millis(50);
        }
        let drawing = self.overlays.values().any(|overlay| {
            overlay.is_visible()
                && !(world.save.settings.fullscreen_app_occlusion
                    && monitor_has_fullscreen_window(overlay.monitor.bounds, &self.cached_windows))
        });
        if !world.save.settings.visible || !drawing {
            return Duration::from_millis(250);
        }
        world_tick_interval(world)
    }

    fn snapshot(&mut self) -> DesktopSnapshot {
        let (mut cursor, idle_duration) = platform::cursor_and_idle(self.previous_cursor);
        let cursor_sample_millis = self.observation_epoch.elapsed().as_millis() as u64;
        self.previous_cursor = cursor
            .available
            .then_some((cursor.position, Instant::now()));
        platform::normalize_cursor(&mut cursor, &self.monitors);
        let frequent_window_scan = self.world.as_ref().is_some_and(|world| {
            world_needs_frequent_window_scan(world, platform::left_button_down())
        });
        let scan_interval = if frequent_window_scan {
            Duration::from_millis(250)
        } else {
            Duration::from_secs(1)
        };
        if !self.window_scan_initialized || self.last_window_scan.elapsed() >= scan_interval {
            let windows = platform::visible_windows();
            self.window_sample = Some(WindowSample {
                monotonic_millis: self.observation_epoch.elapsed().as_millis() as u64,
                reliable: windows.is_some(),
            });
            if let Some(mut windows) = windows {
                platform::normalize_windows(&mut windows, &self.monitors);
                self.cached_windows = windows;
            }
            self.last_window_scan = Instant::now();
            self.window_scan_initialized = true;
        }
        DesktopSnapshot {
            monitors: self.monitors.clone(),
            windows: self.cached_windows.clone(),
            cursor,
            idle_duration,
            window_sample: self.window_sample,
            cursor_sample_millis: Some(cursor_sample_millis),
        }
    }

    fn tick(&mut self) -> bool {
        let now = Instant::now();
        if self
            .milestone_notice
            .as_ref()
            .is_some_and(|notice| now >= notice.expires_at)
        {
            self.milestone_notice = None;
        }
        let tick_interval = self.tick_interval();
        if now.duration_since(self.last_tick) < tick_interval {
            return false;
        }
        let dt = now.duration_since(self.last_tick).as_secs_f32().min(0.2);
        self.last_tick = now;
        if self
            .night_checked
            .is_none_or(|checked| now.duration_since(checked) >= Duration::from_secs(60))
        {
            let night =
                is_night(OffsetDateTime::now_utc().to_offset(crate::clubhouse::local_offset()));
            if night != self.night {
                // The houses light up, or go dark, on the next frame rather than the next move.
                self.request_overlay_redraw();
            }
            self.night = night;
            self.night_checked = Some(now);
        }
        let desktop = self.snapshot();
        self.current_cursor = desktop.cursor;
        let left_button_down = platform::left_button_down();
        if let Some(world) = &mut self.world {
            if world.is_interacting() {
                if !desktop.cursor.available {
                    world.handle_command(WorldCommand::CancelInteraction, &desktop);
                } else if left_button_down {
                    world.handle_command(
                        WorldCommand::UpdateInteraction {
                            cursor: desktop.cursor.position,
                            velocity: desktop.cursor.velocity,
                        },
                        &desktop,
                    );
                } else {
                    // The proxy owns the release, but a press that ends outside it never reaches
                    // the window. Without this the session stays open forever: the creature keeps
                    // following the cursor and `begin_drag` refuses every later grab. Drop the
                    // creature where the button actually came up rather than cancelling it back
                    // to wherever the drag started.
                    world.handle_command(
                        WorldCommand::EndInteraction {
                            cursor: desktop.cursor.position,
                            velocity: desktop.cursor.velocity,
                        },
                        &desktop,
                    );
                }
            }
            let mut filtered = desktop;
            if !world.save.settings.cursor_reactions {
                filtered.cursor.available = false;
            }
            if !world.save.settings.window_ledges {
                filtered.windows.clear();
            }
            world.tick(OffsetDateTime::now_utc(), dt, &filtered);
            let mut milestone = None;
            for event in world.drain_events() {
                self.save_waiting = self.save_waiting.max(event.save_urgency());
                if let WorldEvent::ProfileChanged {
                    creature_id,
                    new_descriptor: Some(_),
                    show_milestone: true,
                } = event
                    && self.milestone_notice.is_none()
                {
                    milestone = Some(creature_id);
                }
                tracing::debug!(event = world_event_category(&event), "world event");
            }
            if let Some(creature_id) = milestone {
                let can_show = world
                    .save
                    .creatures
                    .iter()
                    .find(|creature| creature.id == creature_id)
                    .and_then(|creature| {
                        self.monitors
                            .iter()
                            .find(|monitor| monitor.id == creature.state.surface.monitor_id)
                    })
                    .is_some_and(|monitor| {
                        !monitor_has_fullscreen_window(monitor.bounds, &self.cached_windows)
                    });
                if can_show {
                    self.milestone_notice = Some(MilestoneNotice {
                        creature_id,
                        expires_at: now + Duration::from_secs(5),
                    });
                }
            }
        }
        // Arrivals and the houses are written at once; everyday movement waits for the next
        // routine checkpoint; and a colony with nothing waiting is still written periodically.
        if save_due(self.save_waiting, self.last_save.elapsed())
            && let Err(error) = self.save()
        {
            tracing::error!(%error, "colony save failed");
        }
        self.sync_overlay_visibility();
        self.sync_creature_menu(dt);
        let interval = self
            .world
            .as_ref()
            .map(world_redraw_interval)
            .unwrap_or(Duration::from_millis(250));
        if now >= self.redraw_due {
            if let Some(world) = &self.world {
                let habitat_editor = self.habitat_editor.as_ref().map(|editor| &editor.draft);
                let ui_active = self.creature_menu.is_some() || !world.thought_bubbles().is_empty();
                for overlay in self.overlays.values() {
                    if overlay.is_visible()
                        && overlay.needs_redraw(
                            &world.save,
                            habitat_editor,
                            &self.cached_windows,
                            ui_active,
                        )
                    {
                        overlay.window.request_redraw();
                    }
                }
            }
            let phased_deadline = self.redraw_due + interval;
            self.redraw_due = if phased_deadline > now {
                phased_deadline
            } else {
                now + interval
            };
        }
        true
    }

    fn handle_menu(&mut self, event_loop: &ActiveEventLoop, event: &MenuEvent) {
        let Some(previous_launch) = self
            .world
            .as_ref()
            .map(|world| world.save.settings.launch_at_login)
        else {
            return;
        };
        let action = {
            let Some(world) = &mut self.world else { return };
            let Some(tray) = &mut self.tray else { return };
            tray.handle(event, &mut world.save.settings)
        };
        match action {
            TrayAction::Quit => {
                let _ = self.save();
                event_loop.exit();
            }
            TrayAction::ResetColony => {
                if self.recovery_pending {
                    self.show_settings(event_loop);
                    return;
                }
                if let Err(error) = self.save_store.preserve_recovery_files() {
                    self.show_settings(event_loop);
                    self.settings_error(format!("Could not preserve the previous colony: {error}"));
                    return;
                }
                let desktop = self.snapshot();
                match new_colony_seed() {
                    Ok(seed) => {
                        if let Some(world) = &mut self.world {
                            world.reset(seed, OffsetDateTime::now_utc(), &desktop);
                        }
                    }
                    Err(error) => tracing::error!(%error, "could not generate a new colony seed"),
                }
                let _ = self.save();
            }
            TrayAction::SettingsChanged => {
                if let Err(error) = self.finish_settings_change(previous_launch) {
                    self.show_settings(event_loop);
                    self.settings_error(error.to_string());
                }
            }
            TrayAction::OpenLogs => {
                if let Err(error) = platform::open_directory(&self.log_dir) {
                    tracing::error!(%error, "could not open diagnostic log directory");
                }
            }
            TrayAction::OpenSettings => self.show_settings(event_loop),
            TrayAction::QuietMoment => {
                if let Some(world) = &mut self.world {
                    let minutes = if world.save.companion.quiet_until.is_some() {
                        0
                    } else {
                        30
                    };
                    world.set_quiet_mode(minutes, OffsetDateTime::now_utc());
                }
                let _ = self.save();
            }
            TrayAction::GatherCreatures => {
                let desktop = self.snapshot();
                if let Some(world) = &mut self.world {
                    world.handle_command(WorldCommand::GatherCreatures, &desktop);
                }
                let _ = self.save();
            }
            TrayAction::CheckForUpdates => {
                if matches!(
                    self.updates.status(),
                    UpdateStatus::Available(_)
                        | UpdateStatus::Downloading(_)
                        | UpdateStatus::Ready(_)
                ) {
                    self.show_update_settings(event_loop);
                } else {
                    self.show_update_settings(event_loop);
                    self.start_update_check();
                }
            }
            TrayAction::None => {}
        }
    }

    fn sync_interaction_proxies(&mut self, event_loop: &ActiveEventLoop) {
        if self.habitat_editor.is_some() {
            for proxy in self.interaction_proxies.values_mut() {
                proxy.hide();
            }
            return;
        }
        let Some(world) = &self.world else { return };
        let enabled = world.save.settings.visible && world.save.settings.direct_manipulation;
        // Whoever is visiting is petted and offered things exactly like a member, so it gets a
        // proxy of its own for as long as it is out.
        let desired: Vec<_> = world
            .save
            .creatures
            .iter()
            .chain(world.save.visitors.on_stage())
            .filter(|creature| enabled && creature.state.arrival_delay_secs <= 0.0)
            .map(|creature| creature.id)
            .collect();
        self.interaction_proxies
            .retain(|_, proxy| desired.contains(&proxy.creature_id));
        for creature_id in desired {
            if !self
                .interaction_proxies
                .values()
                .any(|proxy| proxy.creature_id == creature_id)
            {
                match InteractionProxy::new(event_loop, creature_id) {
                    Ok(proxy) => {
                        self.interaction_proxies.insert(proxy.id(), proxy);
                    }
                    Err(error) => {
                        tracing::error!(%error, creature_id, "could not create interaction proxy")
                    }
                }
            }
        }
        let dragging = world.is_dragging();
        for proxy in self.interaction_proxies.values_mut() {
            let Some(creature) = world
                .save
                .creatures
                .iter()
                .chain(world.save.visitors.on_stage())
                .find(|creature| creature.id == proxy.creature_id)
            else {
                continue;
            };
            let Some(monitor) = self
                .monitors
                .iter()
                .find(|monitor| monitor.id == creature.state.surface.monitor_id)
                .or_else(|| self.monitors.iter().find(|monitor| monitor.primary))
            else {
                continue;
            };
            let origin = self
                .overlays
                .values()
                .find(|overlay| overlay.monitor.id == monitor.id)
                .and_then(|overlay| overlay.window.outer_position().ok())
                .unwrap_or(PhysicalPosition::new(0, 0));
            let dragging_this = dragging && creature.state.action == ActionKind::Dragged;
            let fullscreen_hidden = world.save.settings.fullscreen_app_occlusion
                && !dragging_this
                && monitor_has_fullscreen_window(monitor.bounds, &self.cached_windows);
            proxy.sync(
                creature,
                &world.save.settings,
                monitor,
                origin,
                self.current_cursor,
                ProxyRuntimeState {
                    dragging: dragging_this,
                    occluded: fullscreen_hidden,
                },
            );
        }
    }

    fn handle_proxy_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: &WindowEvent,
    ) -> bool {
        if self
            .menu_proxy
            .as_ref()
            .is_some_and(|proxy| proxy.id() == window_id)
        {
            self.handle_menu_proxy_event(event_loop, event);
            return true;
        }
        if !self.interaction_proxies.contains_key(&window_id) {
            return false;
        }
        match event {
            // A secondary click opens the creature's menu instead of picking it up. On macOS a
            // one-button click with Control held means the same thing and arrives as a primary
            // press, so it is separated out before the drag path below sees it.
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                self.toggle_creature_menu(event_loop, window_id);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } if platform::secondary_click_modifier() => {
                self.toggle_creature_menu(event_loop, window_id);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let cursor = self.current_cursor.position;
                // An open menu is in front of whatever it is about: a press on the strip belongs
                // to the strip's own window, never to a creature that happens to be behind it.
                if self
                    .creature_menu
                    .as_ref()
                    .is_some_and(|menu| menu.contains(cursor))
                {
                    return true;
                }
                // macOS proxies carry no native window shape, so an opaque neighbouring square
                // can receive a press aimed at the creature drawn underneath it. That happens
                // constantly at the shelter, where the whole colony shares one corner. Resolve
                // the press against the alpha masks instead of trusting the delivering window.
                let hits: Vec<_> = self
                    .interaction_proxies
                    .iter()
                    .filter(|(_, proxy)| proxy.hit_test(cursor.x, cursor.y))
                    .map(|(id, proxy)| (*id, proxy.creature_id))
                    .collect();
                let draw_order = self.draw_order();
                if let Some((target_window, target_creature)) =
                    resolve_press_target(window_id, &hits, &draw_order)
                {
                    let started = self.world.as_mut().is_some_and(|world| {
                        world.handle_command(
                            WorldCommand::BeginInteraction {
                                creature_id: target_creature,
                                cursor,
                            },
                            &DesktopSnapshot::default(),
                        )
                    });
                    if started && let Some(proxy) = self.interaction_proxies.get(&target_window) {
                        proxy.begin_capture();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let desktop = self.snapshot();
                if let Some(world) = &mut self.world {
                    world.handle_command(
                        WorldCommand::EndInteraction {
                            cursor: desktop.cursor.position,
                            velocity: desktop.cursor.velocity,
                        },
                        &desktop,
                    );
                }
                if let Some(proxy) = self.interaction_proxies.get(&window_id) {
                    proxy.end_capture();
                }
                let _ = self.save();
            }
            WindowEvent::CloseRequested => {
                if let Some(proxy) = self.interaction_proxies.get_mut(&window_id) {
                    proxy.hide();
                }
            }
            _ => {}
        }
        true
    }

    /// Back to front, so a press lands on whoever is drawn on top. The guest is drawn last.
    fn draw_order(&self) -> Vec<CreatureId> {
        self.world
            .as_ref()
            .map(|world| {
                world
                    .save
                    .creatures
                    .iter()
                    .chain(world.save.visitors.on_stage())
                    .map(|creature| creature.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn request_overlay_redraw(&mut self) {
        self.redraw_due = Instant::now();
        for overlay in self.overlays.values() {
            if overlay.is_visible() {
                overlay.window.request_redraw();
            }
        }
    }

    fn save(&mut self) -> Result<()> {
        if self.recovery_pending || self.habitat_editor.is_some() {
            return Ok(());
        }
        if let Some(world) = &self.world {
            if world.is_interacting() {
                return Ok(());
            }
            self.save_store.save(&world.save)?;
            self.last_save = Instant::now();
            self.save_waiting = SaveUrgency::None;
        }
        Ok(())
    }
}

impl ApplicationHandler<UserEvent> for FormigaApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.initialize(event_loop) {
            tracing::error!(%error, "Formiga failed to initialize");
            event_loop.exit();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Menu(event) => self.handle_menu(event_loop, &event),
            UserEvent::Update(event) => self.handle_update_event(event_loop, event),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.habitat_editor.is_some()
            && let WindowEvent::KeyboardInput { event, .. } = &event
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            match event.physical_key {
                PhysicalKey::Code(KeyCode::Escape) => {
                    self.finish_habitat_editor(false);
                    return;
                }
                PhysicalKey::Code(KeyCode::Enter) => {
                    self.finish_habitat_editor(true);
                    return;
                }
                _ => {}
            }
        }
        if self.handle_habitat_editor_event(window_id, &event) {
            return;
        }
        if self.handle_proxy_event(event_loop, window_id, &event) {
            return;
        }
        if self
            .settings_window
            .as_ref()
            .is_some_and(|window| window.id() == window_id)
        {
            if matches!(event, WindowEvent::CloseRequested) {
                self.finish_habitat_editor(false);
                if let Some(window) = &mut self.settings_window {
                    window.hide();
                }
                return;
            }
            let mut outcome = None;
            if let Some(window) = &mut self.settings_window {
                match &event {
                    WindowEvent::RedrawRequested => {
                        let Some(world) = &self.world else {
                            return;
                        };
                        window.clubhouse.last_edit = world.last_edit().map(ColonyEdit::describe);
                        match window.render(
                            event_loop,
                            &self.monitors,
                            &self.cached_windows,
                            self.updates.status(),
                            self.updates.automatic_checks(),
                            ColonyView {
                                save: &world.save,
                                creatures: &world.save.creatures,
                                relationships: &world.save.relationships,
                            },
                        ) {
                            Ok(value) => outcome = Some(value),
                            Err(error) => tracing::error!(%error, "settings render failed"),
                        }
                    }
                    WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                        window.resize();
                    }
                    _ => {
                        if window.on_event(&event) {
                            window.window.request_redraw();
                        }
                    }
                }
            }
            if let Some(outcome) = outcome {
                self.handle_settings_outcome(event_loop, outcome);
            }
            return;
        }
        match event {
            WindowEvent::RedrawRequested => {
                let milestone = self
                    .milestone_notice
                    .as_ref()
                    .filter(|notice| Instant::now() < notice.expires_at)
                    .map(|notice| notice.creature_id);
                // The strip is drawn by the overlay of the display its creature is on, and by
                // no other, so a menu never appears twice on a multi-monitor desk.
                let menu = match (&self.creature_menu, self.overlays.get(&window_id)) {
                    (Some(menu), Some(overlay)) if overlay.monitor.id == menu.monitor_id() => {
                        menu.placement().map(|placement| MenuView {
                            creature_id: menu.creature_id(),
                            items: menu.items(),
                            layout: menu.layout(),
                            placement,
                            hovered: menu.hovered(),
                            side: menu.side().and_then(|side| {
                                side.placement().map(|placement| SideView {
                                    layout: side.layout(),
                                    placement,
                                    hovered: side.hovered(),
                                })
                            }),
                        })
                    }
                    _ => None,
                };
                // Who is indoors, which houses are being seen to, and what is loose about the
                // village. Each is empty, and costs nothing, while nobody is doing any of it.
                let (occupied, motions, loose) =
                    self.world.as_ref().map_or_else(Default::default, |world| {
                        (
                            world.house_occupancy(),
                            world.house_motions(),
                            world.loose_props(&self.monitors),
                        )
                    });
                let village = VillageScene {
                    occupied: &occupied,
                    motions: &motions,
                    loose: &loose,
                    clock: self.observation_epoch.elapsed().as_secs_f32(),
                };
                if let (Some(overlay), Some(world)) =
                    (self.overlays.get_mut(&window_id), &self.world)
                    && let Err(error) = overlay.render(
                        &world.save,
                        self.current_cursor,
                        self.habitat_editor.as_ref().map(|editor| &editor.draft),
                        &self.cached_windows,
                        milestone,
                        OverlayUi {
                            bubbles: world.thought_bubbles(),
                            reduce_motion: world.save.settings.reduce_motion,
                            night: self.night,
                            menu,
                            village,
                        },
                    )
                {
                    tracing::error!(%error, "overlay render failed");
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(overlay) = self.overlays.get_mut(&window_id) {
                    overlay.resize(size);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(overlay) = self.overlays.get_mut(&window_id) {
                    overlay.resize(overlay.window.inner_size());
                }
            }
            WindowEvent::CloseRequested => {
                if let Some(overlay) = self.overlays.get_mut(&window_id) {
                    overlay.set_visible(false);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.last_display_scan.elapsed() >= Duration::from_secs(2)
            && let Err(error) = self.sync_overlays(event_loop)
        {
            tracing::error!(%error, "could not refresh displays");
        }
        let ticked = self.tick();
        if let (Some(tray), Some(world)) = (&mut self.tray, &self.world) {
            tray.sync_quiet(world.save.companion.quiet_until.is_some());
        }
        if ticked {
            self.sync_interaction_proxies(event_loop);
        }
        let tick_interval = self.tick_interval();
        let mut deadline = self.last_tick + tick_interval;
        if let Some(window) = &mut self.settings_window
            && let Some(due) = window.repaint_due
        {
            if Instant::now() >= due {
                window.repaint_due = None;
                window.window.request_redraw();
            } else {
                deadline = deadline.min(due);
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.finish_habitat_editor(false);
        if let Some(world) = &mut self.world {
            world.handle_command(WorldCommand::CancelInteraction, &DesktopSnapshot::default());
        }
        let _ = self.save();
    }
}

fn monitor_id(
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    scale: f32,
    name: Option<&str>,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    position.hash(&mut hasher);
    size.hash(&mut hasher);
    scale.to_bits().hash(&mut hasher);
    name.unwrap_or("display").hash(&mut hasher);
    hasher.finish()
}

/// The part of a display the colony may use: below the menu bar, and clear of the strip the
/// system keeps along the bottom for the Dock or the taskbar. The village stands on the floor of
/// this, so whatever is reserved here is what the houses sit on top of rather than behind.
/// The ground a display gives the colony: whatever the system's own bars leave of it, as the
/// platform reports the display's work area, so a Dock on the side, a taller menu bar, a taskbar
/// along the top, or a bar that hides all move the ground with them. Where the work area cannot
/// be read, the old fixed strips stand in: a menu bar's height along the top, and the factory Dock
/// or taskbar along the bottom. Either way the ground keeps at least 100 points each way.
fn colony_bounds(bounds: DesktopRect, insets: Option<platform::Insets>) -> DesktopRect {
    let insets = insets.unwrap_or(platform::Insets {
        top: 24.0,
        bottom: platform::BOTTOM_RESERVED,
        ..Default::default()
    });
    let inset = |value: f32, room: f32| {
        if value.is_finite() {
            value.clamp(0.0, (room - 100.0).max(0.0))
        } else {
            0.0
        }
    };
    let left = inset(insets.left, bounds.width);
    let right = inset(insets.right, bounds.width - left);
    let top = inset(insets.top, bounds.height);
    let bottom = inset(insets.bottom, bounds.height - top);
    DesktopRect {
        x: bounds.x + left,
        y: bounds.y + top,
        width: (bounds.width - left - right).max(100.0),
        height: (bounds.height - top - bottom).max(100.0),
    }
}

/// Chooses which interaction proxy owns a left-press.
///
/// The window that receives the press wins whenever its own alpha mask covers the cursor. When it
/// does not — a transparent part of an overlapping proxy swallowed the click — the press goes to
/// the mask that does cover the cursor, preferring the creature drawn last, because that is the
/// one the user can actually see on top.
fn resolve_press_target<I: Copy + PartialEq>(
    pressed: I,
    hits: &[(I, CreatureId)],
    draw_order: &[CreatureId],
) -> Option<(I, CreatureId)> {
    if let Some(entry) = hits.iter().find(|(id, _)| *id == pressed) {
        return Some(*entry);
    }
    hits.iter()
        .max_by_key(|(_, creature)| draw_order.iter().position(|id| id == creature))
        .copied()
}

/// A short, fixed category for a refused colony change, in the style of `world_event_category`:
/// enough to tell a diagnostic log what happened, with nothing about the colony in it.
fn colony_management_category(error: &ColonyManagementError) -> &'static str {
    match error {
        ColonyManagementError::ColonyFull => "colony_full",
        ColonyManagementError::AdultLimit => "adult_limit",
        ColonyManagementError::CreatureNotFound => "creature_not_found",
        ColonyManagementError::LastAdult => "last_adult",
        ColonyManagementError::CreatureKept => "creature_kept",
        ColonyManagementError::DuplicateIdentity => "duplicate_identity",
    }
}

/// Whether it is dark where the owner is: from seven in the evening until seven in the morning,
/// when the houses are lit from inside.
fn is_night(local: OffsetDateTime) -> bool {
    local.hour() >= 19 || local.hour() < 7
}

fn world_event_category(event: &WorldEvent) -> &'static str {
    match event {
        WorldEvent::CreatureSpawned { .. } => "creature_spawned",
        WorldEvent::ActionStarted { .. } => "action_started",
        WorldEvent::ActionCompleted { .. } => "action_completed",
        WorldEvent::SurfaceChanged { .. } => "surface_changed",
        WorldEvent::CursorReaction { .. } => "cursor_reaction",
        WorldEvent::WindowReaction { .. } => "window_reaction",
        WorldEvent::BondInteraction { .. } => "bond_interaction",
        WorldEvent::CreatureSlept { .. } => "creature_slept",
        WorldEvent::CreatureWoke { .. } => "creature_woke",
        WorldEvent::CreatureRested { .. } => "creature_rested",
        WorldEvent::SleepInterrupted { .. } => "sleep_interrupted",
        WorldEvent::CreaturePetted { .. } => "creature_petted",
        WorldEvent::CreaturePlaced { .. } => "creature_placed",
        WorldEvent::ObservationElapsed { .. } => "observation_elapsed",
        WorldEvent::ProfileChanged { .. } => "profile_changed",
        WorldEvent::DragStarted { .. } => "drag_started",
        WorldEvent::DragEnded { .. } => "drag_ended",
        WorldEvent::TossLanded { .. } => "toss_landed",
        WorldEvent::OfferAnswered { .. } => "offer_answered",
        WorldEvent::HomeAppeared => "home_appeared",
        WorldEvent::HomeDisappeared { .. } => "home_disappeared",
        WorldEvent::RitualStarted { .. } => "ritual_started",
        WorldEvent::RitualCompleted { .. } => "ritual_completed",
        WorldEvent::RitualInterrupted { .. } => "ritual_interrupted",
        WorldEvent::ColonyObjectAdded { .. } => "colony_object_added",
        WorldEvent::VillageUnlocked { .. } => "village_unlocked",
        WorldEvent::HabitLearned { .. } => "habit_learned",
    }
}

/// A companion's name as it stands now, for naming the change about to be made to it.
fn creature_name(world: &World, creature_id: CreatureId) -> String {
    world
        .save
        .creatures
        .iter()
        .find(|creature| creature.id == creature_id)
        .map_or_else(
            || "a companion".to_owned(),
            |creature| creature.name.clone(),
        )
}

#[cfg(test)]
mod tests;
