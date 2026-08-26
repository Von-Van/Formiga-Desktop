use crate::gpu::OverlayRenderer;
use crate::interaction::InteractionProxy;
use crate::platform;
use crate::settings::{SettingsOutcome, SettingsWindow};
use crate::tray::{TrayAction, TrayState};
use anyhow::{Context, Result};
use directories::ProjectDirs;
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
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId, WindowLevel};

#[derive(Debug)]
pub enum UserEvent {
    Menu(MenuEvent),
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
    window_scan_initialized: bool,
    last_display_scan: Instant,
    log_dir: PathBuf,
    current_cursor: CursorSnapshot,
    settings_window: Option<SettingsWindow>,
    interaction_proxies: BTreeMap<WindowId, InteractionProxy>,
    habitat_editor: Option<HabitatEditor>,
}

impl FormigaApp {
    pub fn new(log_dir: PathBuf) -> Result<Self> {
        let project = ProjectDirs::from("com", "Formiga", "Formiga")
            .context("resolve application data directory")?;
        Ok(Self {
            overlays: BTreeMap::new(),
            monitors: Vec::new(),
            world: None,
            tray: None,
            save_store: SaveStore::new(project.data_dir().join("colony.json")),
            previous_cursor: None,
            last_tick: Instant::now(),
            last_save: Instant::now(),
            redraw_due: Instant::now(),
            cached_windows: Vec::new(),
            last_window_scan: Instant::now() - Duration::from_secs(2),
            window_scan_initialized: false,
            last_display_scan: Instant::now() - Duration::from_secs(2),
            log_dir,
            current_cursor: CursorSnapshot::default(),
            settings_window: None,
            interaction_proxies: BTreeMap::new(),
            habitat_editor: None,
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
        let (world, first_launch) = match self.save_store.load() {
            Ok(Some(save)) => (World::from_save(save), false),
            Ok(None) => (World::new(new_colony_seed()?, now, &desktop), true),
            Err(error) => {
                tracing::error!(%error, "save could not be loaded; starting a new colony");
                (World::new(new_colony_seed()?, now, &desktop), true)
            }
        };
        for overlay in self.overlays.values() {
            let enabled = world.save.settings.visible
                && !accessible_regions(&world.save.settings.habitat, &overlay.monitor).is_empty();
            overlay.window.set_visible(enabled);
        }
        self.tray = Some(TrayState::new(&world.save.settings)?);
        self.world = Some(world);
        self.save()?;
        if first_launch {
            self.show_settings(event_loop);
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
            let top_inset = 24.0;
            let bottom_inset = 40.0;
            let info = MonitorInfo {
                id,
                display_key: platform::display_key(&monitor),
                bounds,
                usable_bounds: DesktopRect {
                    x: bounds.x,
                    y: bounds.y + top_inset,
                    width: bounds.width,
                    height: (bounds.height - top_inset - bottom_inset).max(100.0),
                },
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
                if let Some(world) = &self.world {
                    let enabled = world.save.settings.visible
                        && !accessible_regions(&world.save.settings.habitat, info).is_empty();
                    overlay.window.set_visible(enabled);
                }
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
            platform::configure_native_overlay(&window);
            let renderer = pollster::block_on(OverlayRenderer::new(window, info.clone()))?;
            if let Some(world) = &self.world {
                let enabled = world.save.settings.visible
                    && !accessible_regions(&world.save.settings.habitat, info).is_empty();
                renderer.window.set_visible(enabled);
            }
            self.overlays.insert(renderer.window.id(), renderer);
        }
        let monitors: Vec<_> = discovered.into_iter().map(|(info, _, _)| info).collect();
        if monitors != self.monitors {
            self.cached_windows.clear();
            self.window_scan_initialized = false;
        }
        self.monitors = monitors;
        self.last_display_scan = Instant::now();
        Ok(())
    }

    fn snapshot(&mut self) -> DesktopSnapshot {
        let (mut cursor, idle_duration) = platform::cursor_and_idle(self.previous_cursor);
        self.previous_cursor = cursor
            .available
            .then_some((cursor.position, Instant::now()));
        platform::normalize_cursor(&mut cursor, &self.monitors);
        let creatures_are_moving = self.world.as_ref().is_some_and(world_is_moving);
        let scan_interval = if creatures_are_moving {
            Duration::from_millis(250)
        } else {
            Duration::from_secs(1)
        };
        if !self.window_scan_initialized || self.last_window_scan.elapsed() >= scan_interval {
            self.cached_windows = platform::visible_windows();
            platform::normalize_windows(&mut self.cached_windows, &self.monitors);
            self.last_window_scan = Instant::now();
            self.window_scan_initialized = true;
        }
        DesktopSnapshot {
            monitors: self.monitors.clone(),
            windows: self.cached_windows.clone(),
            cursor,
            idle_duration,
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32().min(0.2);
        if dt < 0.045 {
            return;
        }
        self.last_tick = now;
        let desktop = self.snapshot();
        self.current_cursor = desktop.cursor;
        if let Some(world) = &mut self.world {
            if world.is_dragging() {
                if desktop.cursor.available {
                    world.handle_command(
                        WorldCommand::UpdateDrag {
                            cursor: desktop.cursor.position,
                        },
                        &desktop,
                    );
                } else {
                    world.handle_command(WorldCommand::CancelDrag, &desktop);
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
            for event in world.drain_events() {
                save_on_transition |= matches!(
                    event,
                    WorldEvent::CreatureSpawned { .. }
                        | WorldEvent::ActionStarted { .. }
                        | WorldEvent::SurfaceChanged { .. }
                        | WorldEvent::HomeAppeared
                        | WorldEvent::HomeDisappeared { .. }
                );
                tracing::debug!(?event, "world event");
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
        let paused = self
            .world
            .as_ref()
            .is_some_and(|world| world.save.settings.paused);
        let interval = if paused {
            Duration::from_millis(500)
        } else if self.world.as_ref().is_some_and(world_is_moving) {
            Duration::from_millis(33)
        } else {
            Duration::from_millis(250)
        };
        if now >= self.redraw_due {
            for overlay in self.overlays.values() {
                if overlay.window.is_visible().unwrap_or(true) {
                    overlay.window.request_redraw();
                }
            }
            self.redraw_due = now + interval;
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
                self.finish_settings_change(previous_launch);
            }
            TrayAction::OpenLogs => {
                if let Err(error) = platform::open_directory(&self.log_dir) {
                    tracing::error!(%error, "could not open diagnostic log directory");
                }
            }
            TrayAction::OpenSettings => self.show_settings(event_loop),
            TrayAction::GatherCreatures => {
                let desktop = self.snapshot();
                if let Some(world) = &mut self.world {
                    world.handle_command(WorldCommand::GatherCreatures, &desktop);
                }
                let _ = self.save();
            }
            TrayAction::None => {}
        }
    }

    fn sync_interaction_proxies(&mut self, event_loop: &ActiveEventLoop) {
        if self.habitat_editor.is_some() {
            for proxy in self.interaction_proxies.values() {
                proxy.window.set_visible(false);
            }
            return;
        }
        let Some(world) = &self.world else { return };
        let enabled = world.save.settings.visible && world.save.settings.direct_manipulation;
        let desired: Vec<_> = world
            .save
            .creatures
            .iter()
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
            proxy.sync(
                creature,
                &world.save.settings,
                monitor,
                origin,
                self.current_cursor,
                dragging && creature.state.action == ActionKind::Dragged,
            );
        }
    }

    fn handle_proxy_event(&mut self, window_id: WindowId, event: &WindowEvent) -> bool {
        let Some(creature_id) = self
            .interaction_proxies
            .get(&window_id)
            .map(|proxy| proxy.creature_id)
        else {
            return false;
        };
        match event {
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let hit = self
                    .interaction_proxies
                    .get(&window_id)
                    .is_some_and(|proxy| {
                        proxy.hit_test(
                            self.current_cursor.position.x,
                            self.current_cursor.position.y,
                        )
                    });
                if hit {
                    let started = self.world.as_mut().is_some_and(|world| {
                        world.handle_command(
                            WorldCommand::BeginDrag {
                                creature_id,
                                cursor: self.current_cursor.position,
                            },
                            &DesktopSnapshot::default(),
                        )
                    });
                    if started && let Some(proxy) = self.interaction_proxies.get(&window_id) {
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
                        WorldCommand::EndDrag {
                            cursor: desktop.cursor.position,
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
                if let Some(proxy) = self.interaction_proxies.get(&window_id) {
                    proxy.window.set_visible(false);
                }
            }
            _ => {}
        }
        true
    }

    fn show_settings(&mut self, event_loop: &ActiveEventLoop) {
        let Some(settings) = self.world.as_ref().map(|world| world.save.settings.clone()) else {
            return;
        };
        if self.settings_window.is_none() {
            match pollster::block_on(SettingsWindow::new(
                event_loop,
                &settings,
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
            window.show(&settings);
        }
    }

    fn finish_settings_change(&mut self, previous_launch: bool) {
        let Some(world) = &mut self.world else { return };
        if world.save.settings.launch_at_login != previous_launch
            && let Err(error) = platform::set_launch_at_login(world.save.settings.launch_at_login)
        {
            tracing::error!(%error, "could not update launch-at-login");
            world.save.settings.launch_at_login = previous_launch;
        }
        if let Some(tray) = &self.tray {
            tray.sync(&world.save.settings);
        }
        for overlay in self.overlays.values() {
            let enabled = world.save.settings.visible
                && !accessible_regions(&world.save.settings.habitat, &overlay.monitor).is_empty();
            overlay.window.set_visible(enabled);
        }
        let _ = self.save();
    }

    fn handle_settings_outcome(&mut self, outcome: SettingsOutcome) {
        if let Some(settings) = outcome.applied {
            let previous_launch = self
                .world
                .as_ref()
                .map(|world| world.save.settings.launch_at_login)
                .unwrap_or(false);
            if let Some(world) = &mut self.world {
                world.save.settings = settings;
            }
            self.finish_settings_change(previous_launch);
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
        for overlay in self.overlays.values() {
            if let Err(error) = overlay.window.set_cursor_hittest(true) {
                tracing::warn!(%error, "could not enable habitat editor input");
            }
            overlay.window.request_redraw();
        }
        if let Some(window) = &mut self.settings_window {
            window.set_editor_active(true);
            window.window.set_window_level(WindowLevel::AlwaysOnTop);
            window.window.focus_window();
        }
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
        for overlay in self.overlays.values() {
            if let Err(error) = overlay.window.set_cursor_hittest(false) {
                tracing::warn!(%error, "could not restore overlay click-through");
            }
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
                button: MouseButton::Left | MouseButton::Right,
                ..
            } => {
                self.end_habitat_editor_drag(&monitor);
            }
            _ => {}
        }
        true
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

    fn save(&mut self) -> Result<()> {
        if self.habitat_editor.is_some() {
            return Ok(());
        }
        if let Some(world) = &self.world {
            if world.is_dragging() {
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
        if self.handle_proxy_event(window_id, &event) {
            return;
        }
        if self
            .settings_window
            .as_ref()
            .is_some_and(|window| window.id() == window_id)
        {
            if matches!(event, WindowEvent::CloseRequested) {
                self.finish_habitat_editor(false);
                if let Some(window) = &self.settings_window {
                    window.hide();
                }
                return;
            }
            let mut outcome = None;
            if let Some(window) = &mut self.settings_window {
                match &event {
                    WindowEvent::RedrawRequested => {
                        match window.render(event_loop, &self.monitors, &self.cached_windows) {
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
                self.handle_settings_outcome(outcome);
            }
            return;
        }
        match event {
            WindowEvent::RedrawRequested => {
                if let (Some(overlay), Some(world)) =
                    (self.overlays.get_mut(&window_id), &self.world)
                    && let Err(error) = overlay.render(
                        &world.save,
                        self.current_cursor,
                        self.habitat_editor.as_ref().map(|editor| &editor.draft),
                        &self.cached_windows,
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
                if let Some(overlay) = self.overlays.get(&window_id) {
                    overlay.window.set_visible(false);
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
        self.tick();
        self.sync_interaction_proxies(event_loop);
        let wait = if self
            .world
            .as_ref()
            .is_some_and(|world| world.save.settings.paused)
        {
            Duration::from_millis(250)
        } else {
            Duration::from_millis(20)
        };
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + wait));
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.finish_habitat_editor(false);
        if let Some(world) = &mut self.world {
            world.handle_command(WorldCommand::CancelDrag, &DesktopSnapshot::default());
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

fn world_is_moving(world: &World) -> bool {
    if world.save.settings.paused {
        return false;
    }
    world.save.creatures.iter().any(|creature| {
        creature.state.arrival_delay_secs > 0.0
            || creature.state.velocity.x.abs() > 0.1
            || matches!(
                creature.state.action,
                ActionKind::Traverse
                    | ActionKind::InvestigateCursor
                    | ActionKind::AvoidCursor
                    | ActionKind::ReactToWindow
                    | ActionKind::RideWindow
                    | ActionKind::SoloPlay
                    | ActionKind::Greet
                    | ActionKind::Follow
                    | ActionKind::SocialPlay
                    | ActionKind::Dragged
                    | ActionKind::Landing
            )
    })
}
