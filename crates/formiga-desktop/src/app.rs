use crate::gpu::OverlayRenderer;
use crate::platform;
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
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Window, WindowId, WindowLevel};

#[derive(Debug)]
pub enum UserEvent {
    Menu(MenuEvent),
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
        let world = match self.save_store.load() {
            Ok(Some(save)) => World::from_save(save),
            Ok(None) => World::new(new_colony_seed()?, now, &desktop),
            Err(error) => {
                tracing::error!(%error, "save could not be loaded; starting a new colony");
                World::new(new_colony_seed()?, now, &desktop)
            }
        };
        for overlay in self.overlays.values() {
            let enabled = world.save.settings.visible
                && (!world.save.settings.primary_display_only || overlay.monitor.primary);
            overlay.window.set_visible(enabled);
        }
        self.tray = Some(TrayState::new(&world.save.settings)?);
        self.world = Some(world);
        self.save()?;
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
                        && (!world.save.settings.primary_display_only || info.primary);
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
                    && (!world.save.settings.primary_display_only || info.primary);
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
        if let Some(world) = &mut self.world {
            let mut filtered = desktop;
            if world.save.settings.primary_display_only {
                filtered.monitors.retain(|monitor| monitor.primary);
                if let Some(primary) = filtered.monitors.first() {
                    filtered.windows.retain(|window| {
                        primary.bounds.contains(Point {
                            x: window.bounds.x + window.bounds.width * 0.5,
                            y: window.bounds.y,
                        })
                    });
                }
            }
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
        let Some(world) = &mut self.world else { return };
        let Some(tray) = &mut self.tray else { return };
        let previous_launch = world.save.settings.launch_at_login;
        match tray.handle(event, &mut world.save.settings) {
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
                if world.save.settings.launch_at_login != previous_launch
                    && let Err(error) =
                        platform::set_launch_at_login(world.save.settings.launch_at_login)
                {
                    tracing::error!(%error, "could not update launch-at-login");
                    world.save.settings.launch_at_login = previous_launch;
                    tray.launch_at_login.set_checked(previous_launch);
                }
                for overlay in self.overlays.values() {
                    let enabled = world.save.settings.visible
                        && (!world.save.settings.primary_display_only || overlay.monitor.primary);
                    overlay.window.set_visible(enabled);
                }
                let _ = self.save();
            }
            TrayAction::OpenLogs => {
                if let Err(error) = platform::open_directory(&self.log_dir) {
                    tracing::error!(%error, "could not open diagnostic log directory");
                }
            }
            TrayAction::None => {}
        }
    }

    fn save(&mut self) -> Result<()> {
        if let Some(world) = &self.world {
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
        _event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::RedrawRequested => {
                if let (Some(overlay), Some(world)) =
                    (self.overlays.get_mut(&window_id), &self.world)
                    && let Err(error) = overlay.render(&world.save, self.current_cursor)
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
            )
    })
}
