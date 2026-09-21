use crate::card_export::{
    choose_card_destination, choose_colony_card_destination,
    export_colony_card_to_selected_destination, export_to_selected_destination,
};
use crate::creature_menu::{CreatureMenu, MenuDismissal, MenuTarget, MenuWorld};
use crate::gpu::{MenuView, OverlayRenderer, OverlayUi, monitor_has_fullscreen_window};
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
use formiga_art::{AnimationSpec, BodyClip, MenuIcon};
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

#[derive(Clone, Copy, Debug)]
struct HabitatEditorDrag {
    zone_id: u64,
    monitor_id: MonitorId,
    start: Point,
    mode: HabitatEditorDragMode,
}

#[derive(Clone, Copy, Debug)]
enum HabitatEditorDragMode {
    Create,
    Move {
        original: DesktopRect,
    },
    Resize {
        original: DesktopRect,
        left: bool,
        right: bool,
        top: bool,
        bottom: bool,
    },
}

#[derive(Clone, Debug)]
struct HabitatEditor {
    draft: HabitatPolicy,
    previous_paused: bool,
    drag: Option<HabitatEditorDrag>,
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
                usable_bounds: colony_bounds(bounds),
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
            let mut save_on_transition = false;
            let mut milestone = None;
            for event in world.drain_events() {
                save_on_transition |= matches!(
                    event,
                    WorldEvent::CreatureSpawned { .. }
                        | WorldEvent::ActionStarted { .. }
                        | WorldEvent::SurfaceChanged { .. }
                        | WorldEvent::HomeAppeared
                        | WorldEvent::HomeDisappeared { .. }
                        | WorldEvent::RitualStarted { .. }
                        | WorldEvent::RitualInterrupted { .. }
                        | WorldEvent::ColonyObjectAdded { .. }
                        | WorldEvent::ShelterDecorationAdded { .. }
                );
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
            if save_on_transition && let Err(error) = self.save() {
                tracing::error!(%error, "transition save failed");
            }
        }
        if self.last_save.elapsed() >= Duration::from_secs(30)
            && let Err(error) = self.save()
        {
            tracing::error!(%error, "periodic save failed");
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

    fn start_update_check(&mut self) {
        if !self.updates.begin_check() {
            return;
        }
        self.sync_update_ui();
        let proxy = self.event_proxy.clone();
        std::thread::Builder::new()
            .name("formiga-update-check".into())
            .spawn(move || {
                let result = check_github().map_err(|error| error.to_string());
                let _ = proxy.send_event(UserEvent::Update(UpdateEvent::CheckFinished(result)));
            })
            .expect("spawn update-check worker");
    }

    fn start_update_download(&mut self) {
        let Some((release, directory)) = self.updates.begin_download() else {
            return;
        };
        self.sync_update_ui();
        let proxy = self.event_proxy.clone();
        std::thread::Builder::new()
            .name("formiga-update-download".into())
            .spawn(move || {
                let result = download_update(release, directory).map_err(|error| error.to_string());
                let _ = proxy.send_event(UserEvent::Update(UpdateEvent::DownloadFinished(result)));
            })
            .expect("spawn update-download worker");
    }

    fn handle_update_event(&mut self, event_loop: &ActiveEventLoop, event: UpdateEvent) {
        let reveal = match event {
            UpdateEvent::CheckFinished(result) => self.updates.finish_check(result),
            UpdateEvent::DownloadFinished(result) => {
                self.updates.finish_download(result);
                true
            }
        };
        if let UpdateStatus::Failed(error) = self.updates.status() {
            tracing::warn!(%error, "update operation failed");
        }
        self.sync_update_ui();
        if reveal {
            self.show_update_settings(event_loop);
        }
    }

    fn show_update_settings(&mut self, event_loop: &ActiveEventLoop) {
        self.show_settings(event_loop);
        if let Some(window) = &mut self.settings_window {
            window.select_about();
        }
    }

    fn sync_update_ui(&mut self) {
        if let Some(tray) = &self.tray {
            tray.sync_update(self.updates.status());
        }
        if let Some(window) = &self.settings_window {
            window.window.request_redraw();
        }
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

    /// A secondary click on a creature: open its menu, or close the one it already has. Only one
    /// menu is ever open, so asking for another creature's simply replaces it.
    fn toggle_creature_menu(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId) {
        let cursor = self.current_cursor.position;
        let hits: Vec<_> = self
            .interaction_proxies
            .iter()
            .filter(|(_, proxy)| proxy.hit_test(cursor.x, cursor.y))
            .map(|(id, proxy)| (*id, proxy.creature_id))
            .collect();
        let draw_order = self.draw_order();
        let Some((_, creature_id)) = resolve_press_target(window_id, &hits, &draw_order) else {
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

    fn open_creature_menu(&mut self, event_loop: &ActiveEventLoop, creature_id: CreatureId) {
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
        let menu = CreatureMenu::new(creature_id, target, can_stay, monitor_id, settings_focused);
        if self.menu_proxy.is_none() {
            match MenuProxy::new(event_loop) {
                Ok(proxy) => self.menu_proxy = Some(proxy),
                Err(error) => {
                    tracing::error!(%error, "could not create the creature menu proxy");
                    return;
                }
            }
        }
        self.creature_menu = Some(menu);
        // Place and show it now rather than on the next tick, so the strip appears under the
        // click that asked for it instead of up to a frame later.
        self.sync_creature_menu(0.0);
    }

    fn close_creature_menu(&mut self, reason: MenuDismissal) {
        if self.creature_menu.take().is_none() {
            return;
        }
        tracing::debug!(?reason, "creature menu closed");
        if let Some(proxy) = &mut self.menu_proxy {
            proxy.hide();
        }
        self.request_overlay_redraw();
    }

    fn request_overlay_redraw(&mut self) {
        self.redraw_due = Instant::now();
        for overlay in self.overlays.values() {
            if overlay.is_visible() {
                overlay.window.request_redraw();
            }
        }
    }

    /// Keep the open menu attached to its creature, follow the cursor across its cells, and close
    /// it when any of the dismissal rules fires. `creature_menu` owns every rule; this only feeds
    /// it the world and acts on the answer.
    fn sync_creature_menu(&mut self, dt: f32) {
        let Some(mut menu) = self.creature_menu.take() else {
            return;
        };
        let outcome = self.advance_creature_menu(&mut menu, dt);
        match outcome {
            Ok(redraw) => {
                self.creature_menu = Some(menu);
                if let Some(placement) = self
                    .creature_menu
                    .as_ref()
                    .and_then(|menu| menu.placement())
                {
                    self.sync_menu_proxy(placement.body_desktop());
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

    fn advance_creature_menu(
        &mut self,
        menu: &mut CreatureMenu,
        dt: f32,
    ) -> std::result::Result<bool, MenuDismissal> {
        let settings_focused = self
            .settings_window
            .as_ref()
            .is_some_and(|window| window.window.has_focus());
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
                hidden: !settings.visible
                    || !settings.direct_manipulation
                    || self.habitat_editor.is_some()
                    || creature.state.arrival_delay_secs > 0.0,
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

    fn sync_menu_proxy(&mut self, body: DesktopRect) {
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

    fn handle_menu_proxy_event(&mut self, event_loop: &ActiveEventLoop, event: &WindowEvent) {
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

    /// Act on a menu choice. The menu always closes: the simulation answers with its own thought
    /// bubble, including when a creature is busy and politely declines.
    fn choose_menu_item(&mut self, event_loop: &ActiveEventLoop, icon: MenuIcon) {
        let Some(menu) = self.creature_menu.as_ref() else {
            return;
        };
        let creature_id = menu.creature_id();
        let target = menu.target();
        self.close_creature_menu(MenuDismissal::Answered);
        let desktop = self.snapshot();
        match icon {
            MenuIcon::Snack | MenuIcon::Toy | MenuIcon::Home => {
                let command = match icon {
                    MenuIcon::Snack => WorldCommand::OfferSnack { creature_id },
                    MenuIcon::Toy => WorldCommand::OfferToy { creature_id },
                    _ => WorldCommand::SendHome,
                };
                if let Some(world) = &mut self.world {
                    world.handle_command(command, &desktop);
                }
            }
            MenuIcon::Profile => {
                self.show_settings(event_loop);
                if let Some(window) = &mut self.settings_window {
                    match target {
                        MenuTarget::Member => window.select_creature(creature_id),
                        // A guest has no colony profile of its own, so its page is the journal,
                        // where its visit is written down.
                        MenuTarget::Guest => window.select_journal(),
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

    fn show_settings(&mut self, event_loop: &ActiveEventLoop) {
        let Some((settings, creatures, relationships)) = self.world.as_ref().map(|world| {
            (
                world.save.settings.clone(),
                world.save.creatures.clone(),
                world.save.relationships.clone(),
            )
        }) else {
            return;
        };
        if self.settings_window.is_none() {
            match pollster::block_on(SettingsWindow::new(
                event_loop,
                &settings,
                &creatures,
                &relationships,
                self.save_store.path(),
            )) {
                Ok(window) => self.settings_window = Some(window),
                Err(error) => {
                    tracing::error!(%error, "could not create settings window");
                    return;
                }
            }
        }
        if let Some(window) = &mut self.settings_window {
            window.show(&settings, &creatures, &relationships);
        }
    }

    fn finish_settings_change(&mut self, previous_launch: bool) -> Result<()> {
        let mut failure = None;
        {
            let Some(world) = &mut self.world else {
                return Ok(());
            };
            if world.save.settings.launch_at_login != previous_launch
                && let Err(error) =
                    platform::set_launch_at_login(world.save.settings.launch_at_login)
            {
                tracing::error!(%error, "could not update launch-at-login");
                world.save.settings.launch_at_login = previous_launch;
                failure = Some(error);
            }
            if let Some(tray) = &self.tray {
                tray.sync(&world.save.settings);
            }
        }
        self.sync_overlay_visibility();
        for overlay in self.overlays.values() {
            if overlay.is_visible() {
                overlay.window.request_redraw();
            }
        }
        self.redraw_due = Instant::now();
        self.save()?;
        if let Some(error) = failure {
            return Err(error.context("Could not update launch at login"));
        }
        Ok(())
    }

    fn handle_settings_outcome(&mut self, event_loop: &ActiveEventLoop, outcome: SettingsOutcome) {
        if outcome.export_colony {
            self.export_colony();
        }
        if outcome.restore_colony {
            self.restore_colony();
        }
        if outcome.start_fresh_recovery && self.recovery_pending {
            match self.save_store.preserve_recovery_files() {
                Ok(_) => {
                    self.recovery_pending = false;
                    if let Some(window) = &mut self.settings_window {
                        window.clubhouse.recovery = None;
                    }
                    self.save_with_feedback(
                        "New colony saved · original files preserved as recovery copies",
                    );
                }
                Err(error) => {
                    self.settings_error(format!("Could not preserve recovery files: {error}"))
                }
            }
        }
        let mut companion_changed = false;
        if let Some(world) = &mut self.world {
            if let Some(minutes) = outcome.quiet_minutes {
                world.set_quiet_mode(minutes, OffsetDateTime::now_utc());
                companion_changed = true;
            }
            if outcome.complete_onboarding {
                world.save.companion.onboarding_complete = true;
                companion_changed = true;
            }
            if let Some(corner) = outcome.home_corner {
                world.save.home.corner = corner;
                companion_changed = true;
            }
            if let Some(display) = outcome.home_display {
                world.save.home.display = Some(display);
                companion_changed = true;
            }
            if let Some(hidden) = outcome.hidden_decorations {
                world.save.home.hidden_decorations = hidden & 0x3f;
                companion_changed = true;
            }
            if let Some((a, b)) = outcome.move_object
                && a < world.save.objects.objects.len()
                && b < world.save.objects.objects.len()
            {
                world.save.objects.objects.swap(a, b);
                companion_changed = true;
            }
            if let Some((index, preset)) = outcome.save_mode
                && index < 2
            {
                world.save.companion.modes[index] = Some(preset);
                companion_changed = true;
            }
            if let Some(appearance) = outcome.appearance {
                world.save.companion.appearance = appearance;
                world.save.companion.appearance.normalize();
                companion_changed = true;
            }
            if let Some(schedule) = outcome.schedule.clone() {
                world.save.companion.schedule = schedule;
                world.save.companion.normalize();
                // Editing the routine by hand is the reader speaking last.
                world.override_routine();
                companion_changed = true;
            }
            if outcome.resume_routine {
                world.resume_routine(OffsetDateTime::now_utc());
                companion_changed = true;
            }
            if let Some(entry) = &outcome.pin_moment {
                companion_changed |= world.save.companion.pin(entry);
            }
            if let Some(entry) = &outcome.unpin_moment {
                world.save.companion.unpin(entry);
                companion_changed = true;
            }
        }
        if companion_changed {
            self.save_with_feedback("Colony changes saved");
            self.redraw_due = Instant::now();
            for overlay in self.overlays.values() {
                overlay.window.request_redraw();
            }
        }
        if let Some(creature_id) = outcome.export_creature_card
            && let Some(creature) = self.world.as_ref().and_then(|world| {
                world
                    .save
                    .creatures
                    .iter()
                    .find(|creature| creature.id == creature_id)
            })
        {
            let selected = choose_card_destination(creature);
            match export_to_selected_destination(creature, selected) {
                Ok(Some(_)) => self.settings_notice("Creature card exported"),
                Ok(None) => {}
                Err(error) => {
                    self.settings_error(format!("Could not export the creature card: {error}"))
                }
            }
        }
        if let Some((creature_id, clip, scale)) = outcome.export_creature_sticker
            && let Some(creature) = self.world.as_ref().and_then(|world| {
                world
                    .save
                    .creatures
                    .iter()
                    .find(|creature| creature.id == creature_id)
            })
        {
            // The dialog runs first; a cancelled one never renders a frame.
            let selected = choose_sticker_destination(creature, clip);
            match export_sticker_to_selected_destination(creature, clip, scale, selected) {
                Ok(Some(_)) => self.settings_notice("Sticker exported"),
                Ok(None) => {}
                Err(error) => self.settings_error(format!("Could not export the sticker: {error}")),
            }
        }
        if outcome.export_colony_card
            && let Some(save) = self.world.as_ref().map(|world| &world.save)
        {
            let selected = choose_colony_card_destination();
            match export_colony_card_to_selected_destination(save, selected) {
                Ok(Some(_)) => self.settings_notice("Colony portrait exported"),
                Ok(None) => {}
                Err(error) => {
                    self.settings_error(format!("Could not export the colony portrait: {error}"))
                }
            }
        }
        if let Some((creature_id, kept)) = outcome.set_creature_kept {
            let result = self
                .world
                .as_mut()
                .map(|world| world.set_creature_kept(creature_id, kept));
            match result {
                Some(Ok(())) => {
                    let _ = self.save();
                }
                Some(Err(error)) => {
                    if let Some(window) = &mut self.settings_window {
                        window.set_error(error.to_string());
                    }
                }
                None => {}
            }
        }
        if let Some(creature_id) = outcome.remove_creature {
            let result = self
                .world
                .as_mut()
                .map(|world| world.remove_colony_creature(creature_id));
            match result {
                Some(Ok(())) => {
                    let _ = self.save();
                    self.redraw_due = Instant::now();
                    for overlay in self.overlays.values() {
                        overlay.window.request_redraw();
                    }
                }
                Some(Err(error)) => {
                    if let Some(window) = &mut self.settings_window {
                        window.set_error(error.to_string());
                    }
                }
                None => {}
            }
        }
        if let Some(shared) = outcome.preview_shared {
            let desktop = self.snapshot();
            let creature = World::from_shared_creature(shared, OffsetDateTime::now_utc(), &desktop)
                .save
                .creatures
                .remove(0);
            if let Some(window) = &mut self.settings_window {
                window.clear_generation_preview();
                window.set_generation_preview(GenerationPreview { shared: Some(shared), source_seed: shared.source_colony_seed, creature, similarity: None,
                    summary: "An exact shared appearance and personality, with a fresh life in your colony.".into() });
            }
        }
        if outcome.request_random_creature {
            let (locked, lock_colors, lock_body) = self
                .settings_window
                .as_ref()
                .map(|w| w.clubhouse.locks())
                .unwrap_or((None, false, false));
            let desktop = self.snapshot();
            let seeds: Result<Vec<_>, _> = (0..4).map(|_| new_colony_seed()).collect();
            match seeds {
                Ok(seeds) => {
                    if let Some(window) = &mut self.settings_window {
                        window.clear_generation_preview();
                    }
                    for source_seed in seeds {
                        let mut creature =
                            World::preview_adult(source_seed, OffsetDateTime::now_utc(), &desktop);
                        if let (Some(previous), Some(mut design)) =
                            (locked, creature.appearance.design)
                        {
                            if lock_colors {
                                design.coat = previous.coat;
                                design.accent = previous.accent;
                            }
                            if lock_body {
                                design.body = previous.body;
                                design.width = previous.width;
                                design.height = previous.height;
                                design.head = previous.head;
                                design.legs = previous.legs;
                            }
                            apply_creature_design(&mut creature, Some(design));
                        }
                        if let Some(window) = &mut self.settings_window {
                            window.set_generation_preview(GenerationPreview {
                                shared: None,
                                creature,
                                source_seed,
                                similarity: None,
                                summary: "A new companion with fresh memories.".into(),
                            });
                        }
                    }
                }
                Err(error) => {
                    self.settings_error(format!("Could not generate companions: {error}"))
                }
            }
        }
        if outcome.request_reference_creature
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Character image", &["png", "jpg", "jpeg"])
                .pick_file()
        {
            match new_colony_seed() {
                Ok(search_seed) => {
                    let desktop = self.snapshot();
                    match match_reference_file(
                        &path,
                        search_seed,
                        OffsetDateTime::now_utc(),
                        &desktop,
                    ) {
                        Ok(reference) => {
                            if let Some(window) = &mut self.settings_window {
                                window.clear_generation_preview();
                                window.set_generation_preview(GenerationPreview {
                                    shared: None,
                                    creature: reference.creature,
                                    source_seed: reference.source_seed,
                                    similarity: Some(reference.similarity),
                                    summary: reference.summary.to_owned(),
                                });
                            }
                        }
                        Err(error) => {
                            if let Some(window) = &mut self.settings_window {
                                window.set_error(format!(
                                    "Could not use that reference image: {error}"
                                ));
                            }
                        }
                    }
                }
                Err(error) => {
                    if let Some(window) = &mut self.settings_window {
                        window.set_error(format!("Could not generate a secure seed: {error}"));
                    }
                }
            }
        }
        if let Some(acceptance) = outcome.accept_creature_preview {
            let desktop = self.snapshot();
            let now = OffsetDateTime::now_utc();
            let result = self.world.as_mut().map(|world| match acceptance {
                PreviewAcceptance::Shared { shared, replace } => {
                    world.adopt_shared_creature(shared, replace, now, &desktop)
                }
                PreviewAcceptance::Add {
                    source_seed,
                    design,
                } => world.add_designed_adult(source_seed, design, now, &desktop),
                PreviewAcceptance::Replace {
                    creature_id,
                    source_seed,
                    design,
                } => world.replace_creature_with_design(
                    creature_id,
                    source_seed,
                    design,
                    now,
                    &desktop,
                ),
            });
            match result {
                Some(Ok(_)) => {
                    if let Some(window) = &mut self.settings_window {
                        window.clear_generation_preview();
                    }
                    self.save_with_feedback("Companion welcomed into the colony");
                    self.redraw_due = Instant::now();
                    for overlay in self.overlays.values() {
                        overlay.window.request_redraw();
                    }
                }
                Some(Err(error)) => {
                    if let Some(window) = &mut self.settings_window {
                        window.set_error(error.to_string());
                    }
                }
                None => {}
            }
        }
        if let Some(shared) = outcome.invite_visitor {
            let desktop = self.snapshot();
            let now = OffsetDateTime::now_utc();
            let result = self
                .world
                .as_mut()
                .map(|world| world.invite_visitor(shared, now, &desktop));
            match result {
                Some(Ok(())) => {
                    if let Some(window) = &mut self.settings_window {
                        window.clear_generation_preview();
                    }
                    self.save_with_feedback("Your friend is on their way over");
                    self.redraw_due = Instant::now();
                    for overlay in self.overlays.values() {
                        overlay.window.request_redraw();
                    }
                }
                Some(Err(error)) => {
                    if let Some(window) = &mut self.settings_window {
                        window.set_error(error.to_string());
                    }
                }
                None => {}
            }
        }
        if outcome.ask_visitor_to_stay {
            let desktop = self.snapshot();
            let now = OffsetDateTime::now_utc();
            let result = self
                .world
                .as_mut()
                .map(|world| world.ask_visitor_to_stay(now, &desktop));
            match result {
                Some(Ok(_)) => {
                    self.save_with_feedback("Your visitor is staying for good");
                    self.redraw_due = Instant::now();
                    for overlay in self.overlays.values() {
                        overlay.window.request_redraw();
                    }
                }
                Some(Err(error)) => {
                    if let Some(window) = &mut self.settings_window {
                        window.set_error(error.to_string());
                    }
                }
                None => {}
            }
        }
        if outcome.regenerate_unkept {
            let adult_targets = self.world.as_ref().map_or(0, |world| {
                world
                    .save
                    .creatures
                    .iter()
                    .filter(|creature| !creature.kept && creature.role.is_adult())
                    .count()
            });
            let seeds: Result<Vec<_>, _> = (0..adult_targets).map(|_| new_colony_seed()).collect();
            match seeds {
                Ok(seeds) => {
                    let desktop = self.snapshot();
                    let changed = self.world.as_mut().map_or(0, |world| {
                        world.regenerate_unkept(&seeds, OffsetDateTime::now_utc(), &desktop)
                    });
                    if changed > 0 {
                        let _ = self.save();
                        self.redraw_due = Instant::now();
                        for overlay in self.overlays.values() {
                            overlay.window.request_redraw();
                        }
                    }
                }
                Err(error) => {
                    if let Some(window) = &mut self.settings_window {
                        window.set_error(format!("Could not generate secure seeds: {error}"));
                    }
                }
            }
        }
        let renamed = outcome.rename_creature.is_some();
        let mut save_profile = false;
        if let Some((creature_id, name)) = outcome.rename_creature
            && let Some(world) = &mut self.world
        {
            match world.rename_creature(creature_id, &name) {
                Ok(()) => save_profile = true,
                Err(error) => tracing::warn!(%error, "invalid creature name rejected"),
            }
        }
        if let Some(creature_id) = outcome.viewed_profile
            && let Some(world) = &mut self.world
        {
            save_profile |= world.mark_profile_viewed(creature_id);
        }
        if save_profile {
            if renamed {
                self.save_with_feedback("Name saved");
            } else {
                let _ = self.save();
            }
        }
        if let Some(settings) = outcome.applied {
            let previous_launch = self
                .world
                .as_ref()
                .map(|world| world.save.settings.launch_at_login)
                .unwrap_or(false);
            if let Some(world) = &mut self.world {
                world.save.settings = settings;
            }
            match self.finish_settings_change(previous_launch) {
                Ok(()) => {
                    if let (Some(window), Some(world)) = (&mut self.settings_window, &self.world) {
                        window.acknowledge_preferences(&world.save.settings);
                    }
                    self.settings_notice(if self.recovery_pending {
                        "Temporary preferences · finish recovery to save"
                    } else {
                        "Changes applied"
                    });
                }
                Err(error) => self.settings_error(error.to_string()),
            }
        }
        if outcome.gather {
            let desktop = self.snapshot();
            if let Some(world) = &mut self.world {
                world.handle_command(WorldCommand::GatherCreatures, &desktop);
            }
            let _ = self.save();
        }
        if outcome.open_logs
            && let Err(error) = platform::open_directory(&self.log_dir)
        {
            tracing::error!(%error, "could not open diagnostic log directory");
        }
        if let Some(enabled) = outcome.automatic_update_checks {
            if let Err(error) = self.updates.set_automatic_checks(enabled) {
                tracing::error!(%error, "could not save update preference");
            }
            if enabled
                && self
                    .updates
                    .should_check_automatically(OffsetDateTime::now_utc())
            {
                self.start_update_check();
            }
        }
        if outcome.check_updates {
            self.start_update_check();
        }
        if outcome.download_update {
            self.start_update_download();
        }
        if outcome.install_update
            && let Some(downloaded) = self.updates.ready_update().cloned()
        {
            let _ = self.save();
            match platform::launch_update(&downloaded.path) {
                Ok(quit) if quit => event_loop.exit(),
                Ok(_) => {}
                Err(error) => {
                    tracing::error!(%error, "could not launch update installer");
                    self.updates.fail(error.to_string());
                    self.sync_update_ui();
                }
            }
        }
        if outcome.browse_application
            && let Some((application, display_name)) = platform::browse_application()
            && let Some(window) = &mut self.settings_window
        {
            window.add_application(application, display_name);
        }
        if let Some(draft) = outcome.edit_habitat {
            self.begin_habitat_editor(draft);
        }
        if outcome.apply_habitat_edit {
            self.finish_habitat_editor(true);
        }
        if outcome.cancel_habitat_edit {
            self.finish_habitat_editor(false);
        }
        if outcome.reset_habitat_edit
            && let Some(editor) = &mut self.habitat_editor
        {
            editor.draft = HabitatPolicy::default();
            editor.drag = None;
            if let Some(window) = &mut self.settings_window {
                window.set_habitat(editor.draft.clone());
            }
            for overlay in self.overlays.values() {
                overlay.window.request_redraw();
            }
        }
    }

    fn begin_habitat_editor(&mut self, draft: HabitatPolicy) {
        if self.habitat_editor.is_some() {
            return;
        }
        let previous_paused = self
            .world
            .as_ref()
            .is_some_and(|world| world.save.settings.paused);
        if let Some(world) = &mut self.world {
            world.save.settings.paused = true;
        }
        self.habitat_editor = Some(HabitatEditor {
            draft,
            previous_paused,
            drag: None,
        });
        for overlay in self.overlays.values_mut() {
            overlay.set_hittest_enabled(true);
            overlay.window.request_redraw();
        }
        if let Some(window) = &mut self.settings_window {
            window.set_editor_active(true);
            window.window.set_window_level(WindowLevel::AlwaysOnTop);
            window.window.focus_window();
        }
        self.raise_settings_above_overlays();
    }

    fn finish_habitat_editor(&mut self, apply: bool) {
        let Some(editor) = self.habitat_editor.take() else {
            return;
        };
        let accepted = apply && validate_habitat(&editor.draft, &self.monitors).is_ok();
        if let Some(world) = &mut self.world {
            world.save.settings.paused = editor.previous_paused;
            if accepted {
                world.save.settings.habitat = editor.draft;
            }
        }
        for overlay in self.overlays.values_mut() {
            overlay.set_hittest_enabled(false);
            overlay.window.request_redraw();
        }
        let habitat = self
            .world
            .as_ref()
            .map(|world| world.save.settings.habitat.clone())
            .unwrap_or_default();
        if let Some(window) = &mut self.settings_window {
            window.set_editor_active(false);
            window.window.set_window_level(WindowLevel::Normal);
            window.set_habitat(habitat);
            window.window.focus_window();
        }
        self.raise_settings_above_overlays();
        if accepted {
            let desktop = self.snapshot();
            if let Some(world) = &mut self.world {
                world.handle_command(WorldCommand::GatherCreatures, &desktop);
            }
            let _ = self.save();
        }
    }

    fn handle_habitat_editor_event(&mut self, window_id: WindowId, event: &WindowEvent) -> bool {
        if self.habitat_editor.is_none() {
            return false;
        }
        let Some(monitor) = self
            .overlays
            .get(&window_id)
            .map(|overlay| overlay.monitor.clone())
        else {
            return false;
        };
        if !habitat_editor_claims(event) {
            return false;
        }

        if let WindowEvent::CursorMoved { position, .. } = event {
            self.current_cursor = CursorSnapshot {
                position: Point {
                    x: monitor.bounds.x + position.x as f32 / monitor.scale_factor,
                    y: monitor.bounds.y + position.y as f32 / monitor.scale_factor,
                },
                velocity: Point::default(),
                available: true,
            };
            self.update_habitat_editor_drag(monitor.id, self.current_cursor.position);
            if let Some(overlay) = self.overlays.get(&window_id) {
                overlay.window.request_redraw();
            }
        }

        match event {
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } if matches!(
                button,
                MouseButton::Left | MouseButton::Right | MouseButton::Middle
            ) =>
            {
                self.start_habitat_editor_drag(&monitor, *button);
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button,
                ..
            } => {
                if matches!(button, MouseButton::Left | MouseButton::Right) {
                    self.end_habitat_editor_drag(&monitor);
                }
                // The press that began this gesture ordered a full-screen overlay in front of the
                // settings window, and Apply and Cancel live there and nowhere else. Hand it back
                // its place now the gesture is over, rather than leaving a desktop that answers
                // nothing but the tray.
                self.raise_settings_above_overlays();
            }
            _ => {}
        }
        true
    }

    /// Keep the settings window above the editor's overlays for as long as the edit lasts, and
    /// put it back among them afterwards.
    fn raise_settings_above_overlays(&mut self) {
        let raised = self.habitat_editor.is_some();
        if let Some(window) = &self.settings_window {
            platform::raise_above_overlays(&window.window, raised);
        }
    }

    fn start_habitat_editor_drag(&mut self, monitor: &MonitorInfo, button: MouseButton) {
        let Some(editor) = &mut self.habitat_editor else {
            return;
        };
        if editor.drag.is_some() {
            return;
        }
        let point = monitor.usable_bounds.clamp(self.current_cursor.position);
        let hit = editor
            .draft
            .zones
            .iter()
            .rev()
            .find(|zone| {
                zone.enabled
                    && zone.display == monitor.display_key
                    && denormalized_zone_rect(monitor.usable_bounds, zone.normalized_bounds)
                        .contains(point)
            })
            .map(|zone| (zone.id, zone.normalized_bounds));
        if let Some((zone_id, original)) = hit {
            editor.draft.preset = HabitatPreset::Custom;
            if button == MouseButton::Middle {
                editor.draft.zones.retain(|zone| zone.id != zone_id);
                if let Some(window) = &mut self.settings_window {
                    window.set_habitat(editor.draft.clone());
                }
                return;
            }
            if button == MouseButton::Right {
                if let Some(zone) = editor
                    .draft
                    .zones
                    .iter_mut()
                    .find(|zone| zone.id == zone_id)
                {
                    zone.kind = match zone.kind {
                        HabitatZoneKind::Allowed => HabitatZoneKind::Excluded,
                        HabitatZoneKind::Excluded => HabitatZoneKind::Allowed,
                    };
                }
                if let Some(window) = &mut self.settings_window {
                    window.set_habitat(editor.draft.clone());
                }
                return;
            }
            let rect = denormalized_zone_rect(monitor.usable_bounds, original);
            let threshold = 12.0;
            let left = (point.x - rect.x).abs() <= threshold;
            let right = (point.x - rect.right()).abs() <= threshold;
            let top = (point.y - rect.y).abs() <= threshold;
            let bottom = (point.y - rect.bottom()).abs() <= threshold;
            let mode = if left || right || top || bottom {
                HabitatEditorDragMode::Resize {
                    original,
                    left,
                    right,
                    top,
                    bottom,
                }
            } else {
                HabitatEditorDragMode::Move { original }
            };
            editor.drag = Some(HabitatEditorDrag {
                zone_id,
                monitor_id: monitor.id,
                start: point,
                mode,
            });
            return;
        }
        if editor.draft.zones.len() >= MAX_HABITAT_ZONES || button == MouseButton::Middle {
            return;
        }
        let zone_id = editor
            .draft
            .zones
            .iter()
            .map(|zone| zone.id)
            .max()
            .unwrap_or_default()
            + 1;
        editor.draft.preset = HabitatPreset::Custom;
        editor.draft.zones.push(HabitatZone {
            id: zone_id,
            display: monitor.display_key,
            normalized_bounds: normalized_drag_rect(monitor.usable_bounds, point, point),
            kind: if button == MouseButton::Right {
                HabitatZoneKind::Excluded
            } else {
                HabitatZoneKind::Allowed
            },
            enabled: true,
        });
        editor.drag = Some(HabitatEditorDrag {
            zone_id,
            monitor_id: monitor.id,
            start: point,
            mode: HabitatEditorDragMode::Create,
        });
    }

    fn update_habitat_editor_drag(&mut self, monitor_id: MonitorId, point: Point) {
        let Some(editor) = &mut self.habitat_editor else {
            return;
        };
        let Some(drag) = editor.drag else { return };
        if drag.monitor_id != monitor_id {
            return;
        }
        let Some(monitor) = self.monitors.iter().find(|item| item.id == monitor_id) else {
            return;
        };
        if let Some(zone) = editor
            .draft
            .zones
            .iter_mut()
            .find(|zone| zone.id == drag.zone_id)
        {
            let point = monitor.usable_bounds.clamp(point);
            zone.normalized_bounds = match drag.mode {
                HabitatEditorDragMode::Create => {
                    normalized_drag_rect(monitor.usable_bounds, drag.start, point)
                }
                HabitatEditorDragMode::Move { original } => {
                    let dx = (point.x - drag.start.x) / monitor.usable_bounds.width;
                    let dy = (point.y - drag.start.y) / monitor.usable_bounds.height;
                    DesktopRect {
                        x: (original.x + dx).clamp(0.0, 1.0 - original.width),
                        y: (original.y + dy).clamp(0.0, 1.0 - original.height),
                        ..original
                    }
                }
                HabitatEditorDragMode::Resize {
                    original,
                    left,
                    right,
                    top,
                    bottom,
                } => resized_zone(
                    monitor.usable_bounds,
                    original,
                    drag.start,
                    point,
                    [left, right, top, bottom],
                ),
            };
        }
    }

    fn end_habitat_editor_drag(&mut self, monitor: &MonitorInfo) {
        self.update_habitat_editor_drag(monitor.id, self.current_cursor.position);
        let Some(editor) = &mut self.habitat_editor else {
            return;
        };
        let Some(drag) = editor.drag.take() else {
            return;
        };
        let too_small = matches!(drag.mode, HabitatEditorDragMode::Create)
            && editor
                .draft
                .zones
                .iter()
                .find(|zone| zone.id == drag.zone_id)
                .is_none_or(|zone| {
                    zone.normalized_bounds.width * monitor.usable_bounds.width < 16.0
                        || zone.normalized_bounds.height * monitor.usable_bounds.height < 16.0
                });
        if too_small {
            editor.draft.zones.retain(|zone| zone.id != drag.zone_id);
        }
        if let Some(window) = &mut self.settings_window {
            window.set_habitat(editor.draft.clone());
        }
    }

    fn settings_notice(&mut self, message: impl Into<String>) {
        if let Some(window) = &mut self.settings_window {
            window.notify(message);
        }
    }
    fn settings_error(&mut self, message: impl Into<String>) {
        if let Some(window) = &mut self.settings_window {
            window.set_error(message);
        }
    }
    fn save_with_feedback(&mut self, message: &str) {
        match self.save() {
            Ok(()) if !self.recovery_pending => self.settings_notice(message),
            Ok(()) => self
                .settings_notice("Temporary changes · restore a backup or finish recovery to save"),
            Err(error) => self.settings_error(format!("Could not save changes: {error}")),
        }
    }
    fn export_colony(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Formiga colony", &["json"])
            .set_file_name("Formiga-colony.json")
            .save_file()
        else {
            return;
        };
        if path == self.save_store.path() {
            self.settings_error("Choose a backup location outside the active colony file.");
            return;
        }
        let Some(world) = &self.world else {
            return;
        };
        match SaveStore::new(&path).save(&world.save) {
            Ok(()) => self.settings_notice("Full colony backup exported"),
            Err(error) => self.settings_error(format!("Could not export the colony: {error}")),
        }
    }
    fn restore_colony(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Formiga colony", &["json"])
            .pick_file()
        else {
            return;
        };
        let mut save = match SaveStore::read_snapshot(&path) {
            Ok(save) => save,
            Err(error) => {
                self.settings_error(format!("That backup could not be opened: {error}"));
                return;
            }
        };
        let confirmed = rfd::MessageDialog::new().set_title("Restore your colony?")
            .set_description(format!("Restore {} companions from this backup? Your current colony files will be kept as recovery copies first.", save.creatures.len()))
            .set_buttons(rfd::MessageButtons::OkCancel).show() == rfd::MessageDialogResult::Ok;
        if !confirmed {
            return;
        }
        if validate_habitat(&save.settings.habitat, &self.monitors).is_err() {
            save.settings.habitat = HabitatPolicy::default();
        }
        let imported = World::from_save(save);
        let result = self
            .save_store
            .preserve_recovery_files()
            .and_then(|_| self.save_store.save(&imported.save));
        if let Err(error) = result {
            self.settings_error(format!("Restore stopped; could not safely save: {error}"));
            return;
        }
        let previous_launch = self
            .world
            .as_ref()
            .is_some_and(|w| w.save.settings.launch_at_login);
        self.world = Some(imported);
        self.recovery_pending = false;
        self.milestone_notice = None;
        if let (Some(window), Some(world)) = (&mut self.settings_window, &self.world) {
            window.clubhouse.recovery = None;
            window.clubhouse.restore_confirmed = false;
            window.clear_generation_preview();
            window.show(
                &world.save.settings,
                &world.save.creatures,
                &world.save.relationships,
            );
        }
        if let Err(error) = self.finish_settings_change(previous_launch) {
            self.settings_error(format!(
                "Colony restored, but a preference could not be applied: {error}"
            ));
            return;
        }
        self.redraw_due = Instant::now();
        self.settings_notice("Colony restored · previous files kept as recovery copies");
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
                        })
                    }
                    _ => None,
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
                            menu,
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
fn colony_bounds(bounds: DesktopRect) -> DesktopRect {
    let top_inset = 24.0;
    DesktopRect {
        x: bounds.x,
        y: bounds.y + top_inset,
        width: bounds.width,
        height: (bounds.height - top_inset - platform::BOTTOM_RESERVED).max(100.0),
    }
}

/// What an open habitat editor takes for itself: the pointer, and nothing else.
///
/// While it is open a press on the desktop draws a region rather than reaching the colony, so the
/// editor answers the mouse before anybody else does. Every other event still belongs to the
/// overlay it was sent to — a redraw above all. An editor that claimed those too left the colony
/// frozen on its last frame and never drew one of the regions it was asked for, which is exactly
/// what an application that has crashed looks like.
fn habitat_editor_claims(event: &WindowEvent) -> bool {
    matches!(
        event,
        WindowEvent::CursorMoved { .. } | WindowEvent::MouseInput { .. }
    )
}

fn normalized_drag_rect(bounds: DesktopRect, a: Point, b: Point) -> DesktopRect {
    let left = a.x.min(b.x).clamp(bounds.x, bounds.right());
    let top = a.y.min(b.y).clamp(bounds.y, bounds.bottom());
    let right = a.x.max(b.x).clamp(bounds.x, bounds.right());
    let bottom = a.y.max(b.y).clamp(bounds.y, bounds.bottom());
    DesktopRect {
        x: (left - bounds.x) / bounds.width,
        y: (top - bounds.y) / bounds.height,
        width: ((right - left) / bounds.width).max(0.0001),
        height: ((bottom - top) / bounds.height).max(0.0001),
    }
}

fn denormalized_zone_rect(bounds: DesktopRect, normalized: DesktopRect) -> DesktopRect {
    DesktopRect {
        x: bounds.x + normalized.x * bounds.width,
        y: bounds.y + normalized.y * bounds.height,
        width: normalized.width * bounds.width,
        height: normalized.height * bounds.height,
    }
}

fn resized_zone(
    bounds: DesktopRect,
    original: DesktopRect,
    start: Point,
    current: Point,
    edges: [bool; 4],
) -> DesktopRect {
    let [resize_left, resize_right, resize_top, resize_bottom] = edges;
    let dx = (current.x - start.x) / bounds.width;
    let dy = (current.y - start.y) / bounds.height;
    let mut left = original.x;
    let mut right = original.right();
    let mut top = original.y;
    let mut bottom = original.bottom();
    if resize_left {
        left = (left + dx).clamp(0.0, right - 0.02);
    }
    if resize_right {
        right = (right + dx).clamp(left + 0.02, 1.0);
    }
    if resize_top {
        top = (top + dy).clamp(0.0, bottom - 0.02);
    }
    if resize_bottom {
        bottom = (bottom + dy).clamp(top + 0.02, 1.0);
    }
    DesktopRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
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

fn world_needs_frequent_window_scan(world: &World, button_down: bool) -> bool {
    if world.save.settings.paused {
        return false;
    }
    // A fresh window list every quarter second matters only while a window could actually be
    // moving under a creature. A creature already riding, climbing, landing, squeezing, or being
    // carried needs it throughout. One simply standing on a ledge needs it only while the mouse
    // button is held, because that is the only way a window gets dragged out from under it; the
    // ordinary once-a-second scan notices everything else. A creature on the floor is carried by
    // no window at all. Scanning four times a second whenever anyone stood on a ledge meant
    // scanning four times a second nearly always, and the window list is the most expensive thing
    // this app asks the system for.
    world.save.creatures.iter().any(|creature| {
        (button_down && creature.state.surface.window_key.is_some())
            || matches!(
                creature.state.action,
                ActionKind::SqueezeWindow
                    | ActionKind::RideWindow
                    | ActionKind::Dragged
                    | ActionKind::Landing
                    | ActionKind::ClimbWindow
                    | ActionKind::Tossed
            )
    })
}

fn world_has_spatial_motion(world: &World) -> bool {
    if world.save.settings.paused {
        return false;
    }
    world.save.creatures.iter().any(|creature| {
        creature.state.velocity.x.abs() > 0.1
            || creature.state.velocity.y.abs() > 0.1
            || matches!(
                creature.state.action,
                ActionKind::Traverse
                    | ActionKind::SqueezeWindow
                    | ActionKind::Sprint
                    | ActionKind::InvestigateCursor
                    | ActionKind::AvoidCursor
                    | ActionKind::ReactToWindow
                    | ActionKind::Follow
                    | ActionKind::Dragged
                    | ActionKind::Landing
                    | ActionKind::ClimbWindow
                    | ActionKind::Tossed
            )
    })
}

fn world_redraw_interval(world: &World) -> Duration {
    if world.is_interacting() {
        return Duration::from_millis(50);
    }
    if world.save.settings.paused {
        return Duration::from_secs(1);
    }
    // The simulation itself advances at 20 Hz, so presenting faster would only repeat identical
    // positions. Pose-only activities follow their authored atlas frame rate instead.
    if world_has_spatial_motion(world) {
        return Duration::from_millis(50);
    }
    let fps = world
        .save
        .creatures
        .iter()
        .filter(|creature| creature.state.arrival_delay_secs <= 0.0)
        .map(|creature| AnimationSpec::for_clip(BodyClip::for_creature(creature)).fps)
        .max()
        .unwrap_or(2)
        .max(1);
    Duration::from_secs_f32(1.0 / f32::from(fps))
}

fn world_tick_interval(world: &World) -> Duration {
    if world.is_interacting() {
        return Duration::from_millis(50);
    }
    if world.save.settings.paused {
        return Duration::from_millis(250);
    }
    if world_has_spatial_motion(world) {
        return Duration::from_millis(50);
    }
    let has_expressive_action = world.save.creatures.iter().any(|creature| {
        creature.state.arrival_delay_secs <= 0.0
            && AnimationSpec::for_clip(BodyClip::for_creature(creature)).fps >= 8
    });
    let needs_responsive_gaze =
        world.save.settings.cursor_reactions
            && world.save.creatures.iter().any(|creature| {
                matches!(creature.state.action, ActionKind::Idle | ActionKind::Perch)
            });
    // Interaction proxies only refresh their native hit region on a tick, so a resting creature
    // still needs a responsive cadence to feel grabbable. Homebound creatures belong here too:
    // they are the stillest state in the simulation and the one users reach for at the shelter.
    let needs_responsive_grab = world.save.settings.direct_manipulation
        && world.save.creatures.iter().any(|creature| {
            matches!(
                creature.state.action,
                ActionKind::Idle | ActionKind::Perch | ActionKind::Homebound
            )
        });
    if has_expressive_action || needs_responsive_gaze || needs_responsive_grab {
        Duration::from_millis(100)
    } else {
        Duration::from_millis(200)
    }
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
        WorldEvent::ShelterDecorationAdded { .. } => "shelter_decoration_added",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        colony_bounds, habitat_editor_claims, resolve_press_target, world_redraw_interval,
    };
    use formiga_core::*;
    use std::time::Duration;
    use winit::event::WindowEvent;

    /// A press on the desktop belongs to the open editor, which draws a region with it instead of
    /// letting it reach the colony. A redraw never does: the editor claimed one for a while, and
    /// the overlay it was addressed to then stopped painting for as long as the editor was open,
    /// so the colony sat frozen on its last frame and not one of the regions being dragged out
    /// was ever drawn.
    #[test]
    fn the_habitat_editor_takes_the_pointer_and_leaves_the_overlay_its_redraw() {
        let device_id = winit::event::DeviceId::dummy();
        assert!(habitat_editor_claims(&WindowEvent::MouseInput {
            device_id,
            state: winit::event::ElementState::Pressed,
            button: winit::event::MouseButton::Left,
        }));
        assert!(habitat_editor_claims(&WindowEvent::CursorMoved {
            device_id,
            position: winit::dpi::PhysicalPosition::new(4.0, 4.0),
        }));
        for event in [
            WindowEvent::RedrawRequested,
            WindowEvent::Resized(winit::dpi::PhysicalSize::new(800, 600)),
            WindowEvent::CloseRequested,
        ] {
            assert!(
                !habitat_editor_claims(&event),
                "the editor swallowed {event:?}, which the overlay needs"
            );
        }
    }

    /// How tall the strip along the bottom of the screen really is when the owner has never
    /// touched it: the Dock with its factory forty-eight point tiles inside a panel of its own,
    /// or the Windows 11 taskbar.
    #[cfg(target_os = "macos")]
    const FACTORY_SYSTEM_STRIP: f32 = 75.0;
    #[cfg(target_os = "windows")]
    const FACTORY_SYSTEM_STRIP: f32 = 48.0;

    /// The colony stands on the floor of the ground it is given, so that floor has to clear the
    /// strip the system keeps for itself along the bottom of the display. It stood forty points
    /// up, well inside a Dock at its factory size, and the village lived behind one.
    #[test]
    fn the_village_stands_on_top_of_the_dock_rather_than_behind_it() {
        for screen in [
            DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 1512.0,
                height: 982.0,
            },
            DesktopRect {
                x: -1920.0,
                y: 120.0,
                width: 1920.0,
                height: 1080.0,
            },
        ] {
            let usable = colony_bounds(screen);
            // Where the houses are founded and where everybody's feet meet the ground.
            let ground = usable.bottom() - 4.0;
            assert!(
                screen.bottom() - ground >= FACTORY_SYSTEM_STRIP,
                "the village stands {} points up, inside a strip {FACTORY_SYSTEM_STRIP} tall",
                screen.bottom() - ground
            );
            assert_eq!(usable.x, screen.x);
            assert_eq!(usable.width, screen.width);
        }
    }

    #[test]
    fn a_gesture_is_presented_at_its_own_frame_rate_rather_than_the_action_beneath_it() {
        let mut world = World::new(
            [5; 32],
            time::OffsetDateTime::UNIX_EPOCH,
            &DesktopSnapshot::default(),
        );
        for creature in &mut world.save.creatures {
            creature.state.action = ActionKind::InspectScreen;
            creature.state.arrival_delay_secs = 0.0;
            creature.state.velocity = Point::default();
            creature.state.attention = None;
        }
        // Inspecting animates at four frames a second; a cheer over it animates at eight.
        assert_eq!(world_redraw_interval(&world), Duration::from_secs_f32(0.25));
        world.save.creatures[0].state.attention = Some(AttentionPose {
            target: Point::default(),
            emotion: AttentionEmotion::Enjoying,
            hanging: 0.0,
            gesture: Some(Gesture::Cheer),
        });
        assert_eq!(
            world_redraw_interval(&world),
            Duration::from_secs_f32(0.125)
        );
    }

    #[test]
    fn press_stays_on_the_window_whose_own_mask_covers_the_cursor() {
        let hits = [(10u32, 1u64), (20u32, 2u64)];
        let order = [1u64, 2u64];
        assert_eq!(
            resolve_press_target(20u32, &hits, &order),
            Some((20u32, 2u64))
        );
    }

    #[test]
    fn press_on_a_transparent_overlap_reaches_the_creature_underneath() {
        // The colony shares one corner at the shelter, so the press lands on proxy 30 even though
        // only creatures 1 and 2 have opaque pixels under the cursor.
        let hits = [(10u32, 1u64), (20u32, 2u64)];
        let order = [1u64, 2u64, 3u64];
        assert_eq!(
            resolve_press_target(30u32, &hits, &order),
            Some((20u32, 2u64)),
            "should pick the creature drawn last, which is the visible one on top"
        );
    }

    #[test]
    fn press_over_no_creature_starts_no_drag() {
        assert_eq!(resolve_press_target(30u32, &[], &[1u64]), None);
    }
}
