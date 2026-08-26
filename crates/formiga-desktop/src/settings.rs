use anyhow::{Context as _, Result};
use formiga_core::{
    ApplicationOcclusionRule, DesktopRect, DesktopWindow, HabitatPolicy, HabitatPreset,
    HabitatZone, HabitatZoneKind, MonitorInfo, Settings, validate_habitat,
};
use std::sync::Arc;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsTab {
    #[default]
    General,
    Habitat,
    Applications,
    About,
}

#[derive(Default)]
pub struct SettingsOutcome {
    pub applied: Option<Settings>,
    pub gather: bool,
    pub edit_habitat: Option<HabitatPolicy>,
    pub apply_habitat_edit: bool,
    pub cancel_habitat_edit: bool,
    pub reset_habitat_edit: bool,
    pub browse_application: bool,
    pub open_logs: bool,
}

pub struct SettingsWindow {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    context: egui::Context,
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    draft: Settings,
    saved: Settings,
    save_location: String,
    tab: SettingsTab,
    error: Option<String>,
    editor_active: bool,
}

impl SettingsWindow {
    pub async fn new(
        event_loop: &ActiveEventLoop,
        settings: &Settings,
        save_location: &std::path::Path,
    ) -> Result<Self> {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Formiga Settings")
                        .with_inner_size(LogicalSize::new(680.0, 600.0))
                        .with_min_inner_size(LogicalSize::new(560.0, 480.0))
                        .with_visible(false),
                )
                .context("create settings window")?,
        );
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .context("create settings surface")?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                apply_limit_buckets: false,
            })
            .await
            .context("find settings GPU adapter")?;
        let limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Formiga settings GPU"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            })
            .await
            .context("create settings GPU device")?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: Vec::new(),
        };
        surface.configure(&device, &config);
        let context = egui::Context::default();
        configure_style(&context);
        let state = egui_winit::State::new(
            context.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(adapter.limits().max_texture_dimension_2d as usize),
        );
        let renderer =
            egui_wgpu::Renderer::new(&device, format, egui_wgpu::RendererOptions::default());
        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            context,
            state,
            renderer,
            draft: settings.clone(),
            saved: settings.clone(),
            save_location: save_location.display().to_string(),
            tab: SettingsTab::default(),
            error: None,
            editor_active: false,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn show(&mut self, settings: &Settings) {
        self.draft = settings.clone();
        self.saved = settings.clone();
        self.error = None;
        self.window.set_visible(true);
        self.window.focus_window();
        self.window.request_redraw();
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    pub fn add_application(
        &mut self,
        application: formiga_core::ApplicationKey,
        display_name: String,
    ) {
        if self
            .draft
            .application_occlusion_rules
            .iter()
            .any(|rule| rule.application == application)
        {
            return;
        }
        self.draft
            .application_occlusion_rules
            .push(ApplicationOcclusionRule {
                application,
                display_name,
                enabled: true,
            });
        self.window.request_redraw();
    }

    pub fn set_editor_active(&mut self, active: bool) {
        self.editor_active = active;
        self.window.request_redraw();
    }

    pub fn set_habitat(&mut self, habitat: HabitatPolicy) {
        self.draft.habitat = habitat;
        self.window.request_redraw();
    }

    pub fn on_event(&mut self, event: &WindowEvent) -> bool {
        self.state.on_window_event(&self.window, event).repaint
    }

    pub fn resize(&mut self) {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(
        &mut self,
        event_loop: &ActiveEventLoop,
        monitors: &[MonitorInfo],
        windows: &[DesktopWindow],
    ) -> Result<SettingsOutcome> {
        let input = self.state.take_egui_input(&self.window);
        let context = self.context.clone();
        let mut outcome = SettingsOutcome::default();
        let mut draft = self.draft.clone();
        let saved = self.saved.clone();
        let save_location = self.save_location.clone();
        let mut tab = self.tab;
        let mut error = self.error.clone();
        let editor_active = self.editor_active;
        let full_output = context.run_ui(input, |ui| {
            draw_settings(
                ui,
                &mut draft,
                &mut tab,
                &mut error,
                &saved,
                &save_location,
                monitors,
                windows,
                editor_active,
                &mut outcome,
            );
        });
        self.draft = draft;
        self.tab = tab;
        self.error = error;
        if let Some(applied) = &outcome.applied {
            self.saved = applied.clone();
        }
        self.state.handle_platform_output_with_event_loop(
            &self.window,
            event_loop,
            full_output.platform_output,
        );

        for (id, deltas) in &full_output.textures_delta.set {
            for delta in deltas {
                self.renderer
                    .update_texture(&self.device, &self.queue, *id, delta);
            }
        }
        let paint_jobs = self
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: full_output.pixels_per_point,
        };
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(output)
            | wgpu::CurrentSurfaceTexture::Suboptimal(output) => output,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(outcome);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(outcome);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                anyhow::bail!("settings surface validation failed")
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Formiga settings frame"),
            });
        let callback_buffers = self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &paint_jobs,
            &screen,
        );
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Formiga settings pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.035,
                            g: 0.047,
                            b: 0.043,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.renderer
                .render(&mut pass.forget_lifetime(), &paint_jobs, &screen);
        }
        self.queue
            .submit(callback_buffers.into_iter().chain(Some(encoder.finish())));
        self.queue.present(output);
        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }
        Ok(outcome)
    }
}

fn configure_style(context: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(16, 23, 21);
    visuals.window_fill = egui::Color32::from_rgb(20, 29, 26);
    visuals.selection.bg_fill = egui::Color32::from_rgb(75, 151, 116);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(75, 151, 116);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(49, 91, 72);
    context.set_visuals(visuals);
}

#[allow(clippy::too_many_arguments)]
fn draw_settings(
    root: &mut egui::Ui,
    settings: &mut Settings,
    tab: &mut SettingsTab,
    error: &mut Option<String>,
    saved: &Settings,
    save_location: &str,
    monitors: &[MonitorInfo],
    windows: &[DesktopWindow],
    editor_active: bool,
    outcome: &mut SettingsOutcome,
) {
    egui::CentralPanel::default().show(root, |ui| {
        ui.heading("Formiga");
        ui.label(egui::RichText::new("A quiet little ecosystem for your desktop").italics());
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            for (candidate, label) in [
                (SettingsTab::General, "General"),
                (SettingsTab::Habitat, "Habitat"),
                (SettingsTab::Applications, "Applications"),
                (SettingsTab::About, "About"),
            ] {
                ui.selectable_value(tab, candidate, label);
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| match tab {
            SettingsTab::General => general_tab(ui, settings),
            SettingsTab::Habitat => habitat_tab(ui, settings, monitors, editor_active, outcome),
            SettingsTab::Applications => applications_tab(ui, settings, windows, outcome),
            SettingsTab::About => about_tab(ui, outcome, save_location),
        });
        ui.separator();
        if let Some(message) = error.as_deref() {
            ui.colored_label(egui::Color32::from_rgb(241, 142, 119), message);
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!editor_active, egui::Button::new("Apply changes"))
                .clicked()
            {
                match validate_habitat(&settings.habitat, monitors) {
                    Ok(()) => {
                        outcome.applied = Some(settings.clone());
                        *error = None;
                    }
                    Err(message) => *error = Some(message.to_owned()),
                }
            }
            if ui.button("Revert").clicked() {
                *settings = saved.clone();
                *error = None;
            }
        });
    });
}

fn general_tab(ui: &mut egui::Ui, settings: &mut Settings) {
    ui.checkbox(&mut settings.visible, "Show ecosystem");
    ui.checkbox(&mut settings.paused, "Pause ambient behavior");
    ui.checkbox(
        &mut settings.direct_manipulation,
        "Allow creatures to be dragged",
    );
    ui.checkbox(&mut settings.cursor_reactions, "React to cursor movement");
    ui.checkbox(&mut settings.window_ledges, "Use application-window ledges");
    ui.checkbox(&mut settings.reduce_motion, "Reduce motion");
    ui.checkbox(&mut settings.launch_at_login, "Launch at login");
    ui.add_space(8.0);
    ui.label("Creature display scale");
    ui.horizontal(|ui| {
        for scale in 2..=4 {
            ui.selectable_value(&mut settings.display_scale, scale, format!("{scale}×"));
        }
    });
}

fn habitat_tab(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    monitors: &[MonitorInfo],
    editor_active: bool,
    outcome: &mut SettingsOutcome,
) {
    let previous_preset = settings.habitat.preset;
    egui::ComboBox::from_label("Preset")
        .selected_text(format!("{:?}", settings.habitat.preset))
        .show_ui(ui, |ui| {
            for preset in [
                HabitatPreset::EntireDesktop,
                HabitatPreset::PrimaryDisplay,
                HabitatPreset::BottomEdge,
                HabitatPreset::BottomCorners,
                HabitatPreset::LowerHalf,
                HabitatPreset::Custom,
            ] {
                ui.selectable_value(&mut settings.habitat.preset, preset, format!("{preset:?}"));
            }
        });
    if settings.habitat.preset != previous_preset
        && settings.habitat.preset != HabitatPreset::Custom
    {
        settings.habitat.zones.clear();
    }
    ui.horizontal(|ui| {
        if !editor_active && ui.button("Edit on desktop").clicked() {
            outcome.edit_habitat = Some(settings.habitat.clone());
        }
        if editor_active && ui.button("Apply desktop edit").clicked() {
            outcome.apply_habitat_edit = true;
        }
        if editor_active && ui.button("Cancel desktop edit").clicked() {
            outcome.cancel_habitat_edit = true;
        }
        if editor_active && ui.button("Reset").clicked() {
            outcome.reset_habitat_edit = true;
        }
        if ui.button("Gather creatures here").clicked() {
            outcome.gather = true;
        }
    });
    if editor_active {
        ui.colored_label(
            egui::Color32::from_rgb(126, 220, 170),
            "Desktop editor active: drag with the left button to allow an area, or the right button to exclude it.",
        );
    }
    ui.add_space(8.0);
    ui.label("Custom rectangles use normalized display coordinates (0–1).");
    let mut remove = None;
    let mut changed_zone = false;
    for (index, zone) in settings.habitat.zones.iter_mut().enumerate() {
        let before = zone.clone();
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut zone.enabled, "Enabled");
                ui.selectable_value(&mut zone.kind, HabitatZoneKind::Allowed, "Allowed");
                ui.selectable_value(&mut zone.kind, HabitatZoneKind::Excluded, "Excluded");
                if ui.small_button("Remove").clicked() {
                    remove = Some(index);
                }
            });
            ui.horizontal(|ui| {
                ui.label("x");
                ui.add(egui::DragValue::new(&mut zone.normalized_bounds.x).range(0.0..=1.0));
                ui.label("y");
                ui.add(egui::DragValue::new(&mut zone.normalized_bounds.y).range(0.0..=1.0));
                ui.label("w");
                ui.add(egui::DragValue::new(&mut zone.normalized_bounds.width).range(0.05..=1.0));
                ui.label("h");
                ui.add(egui::DragValue::new(&mut zone.normalized_bounds.height).range(0.05..=1.0));
            });
        });
        changed_zone |= *zone != before;
    }
    if let Some(index) = remove {
        settings.habitat.zones.remove(index);
        settings.habitat.preset = HabitatPreset::Custom;
    }
    if changed_zone {
        settings.habitat.preset = HabitatPreset::Custom;
    }
    if settings.habitat.zones.len() < 32 {
        ui.horizontal(|ui| {
            if ui.button("Add allowed zone").clicked() {
                add_zone(settings, monitors, HabitatZoneKind::Allowed);
            }
            if ui.button("Add exclusion zone").clicked() {
                add_zone(settings, monitors, HabitatZoneKind::Excluded);
            }
        });
    }
}

fn add_zone(settings: &mut Settings, monitors: &[MonitorInfo], kind: HabitatZoneKind) {
    let Some(monitor) = monitors
        .iter()
        .find(|monitor| monitor.primary)
        .or_else(|| monitors.first())
    else {
        return;
    };
    let id = settings
        .habitat
        .zones
        .iter()
        .map(|zone| zone.id)
        .max()
        .unwrap_or_default()
        + 1;
    settings.habitat.zones.push(HabitatZone {
        id,
        display: monitor.display_key,
        normalized_bounds: DesktopRect {
            x: 0.25,
            y: 0.5,
            width: 0.5,
            height: 0.45,
        },
        kind,
        enabled: true,
    });
    settings.habitat.preset = HabitatPreset::Custom;
}

fn applications_tab(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    windows: &[DesktopWindow],
    outcome: &mut SettingsOutcome,
) {
    ui.label("Selected application windows visually cover creatures inside their visible area.");
    let mut remove = None;
    for (index, rule) in settings.application_occlusion_rules.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.checkbox(&mut rule.enabled, "");
            ui.label(&rule.display_name);
            if ui.small_button("Remove").clicked() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        settings.application_occlusion_rules.remove(index);
    }
    ui.separator();
    ui.label("Running applications");
    let mut seen = std::collections::BTreeSet::new();
    for window in windows {
        let (Some(application), Some(name)) = (&window.application, &window.application_name)
        else {
            continue;
        };
        if !seen.insert(application.clone()) {
            continue;
        }
        let selected = settings
            .application_occlusion_rules
            .iter()
            .any(|rule| rule.application == *application);
        if ui
            .add_enabled(!selected, egui::Button::new(format!("Add {name}")))
            .clicked()
        {
            settings
                .application_occlusion_rules
                .push(ApplicationOcclusionRule {
                    application: application.clone(),
                    display_name: name.clone(),
                    enabled: true,
                });
        }
    }
    if ui.button("Add Application…").clicked() {
        outcome.browse_application = true;
    }
}

fn about_tab(ui: &mut egui::Ui, outcome: &mut SettingsOutcome, save_location: &str) {
    ui.heading("Formiga 0.2.0");
    ui.label("Procedural desktop fauna, generated and simulated entirely on your computer.");
    ui.add_space(8.0);
    ui.label("Local-only. No accounts, network service, screenshots, window titles, keystrokes, or telemetry.");
    ui.add_space(8.0);
    ui.label(format!("Save: {save_location}"));
    ui.label("License: MIT. Third-party dependency versions are recorded in Cargo.lock.");
    ui.hyperlink_to(
        "Project repository",
        "https://github.com/Von-Van/Formiga-Desktop",
    );
    ui.add_space(8.0);
    if ui.button("Open diagnostic logs").clicked() {
        outcome.open_logs = true;
    }
}
