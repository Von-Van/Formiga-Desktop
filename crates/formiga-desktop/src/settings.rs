use crate::updater::{APP_VERSION, UpdateStatus};
use anyhow::{Context as _, Result};
use formiga_core::{
    ApplicationOcclusionRule, Creature, CreatureId, DesktopRect, DesktopWindow, HabitatPolicy,
    HabitatPreset, HabitatZone, HabitatZoneKind, MonitorInfo, Settings, profile_descriptors,
    validate_creature_name, validate_habitat,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsTab {
    #[default]
    General,
    Colony,
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
    pub automatic_update_checks: Option<bool>,
    pub check_updates: bool,
    pub download_update: bool,
    pub install_update: bool,
    pub rename_creature: Option<(CreatureId, String)>,
    pub viewed_profile: Option<CreatureId>,
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
    creatures: Vec<Creature>,
    creature_names: BTreeMap<CreatureId, String>,
    selected_creature: Option<CreatureId>,
}

impl SettingsWindow {
    pub async fn new(
        event_loop: &ActiveEventLoop,
        settings: &Settings,
        creatures: &[Creature],
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
            creatures: creatures.to_vec(),
            creature_names: creatures
                .iter()
                .map(|creature| (creature.id, creature.name.clone()))
                .collect(),
            selected_creature: creatures.first().map(|creature| creature.id),
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn show(&mut self, settings: &Settings, creatures: &[Creature]) {
        self.draft = settings.clone();
        self.saved = settings.clone();
        self.error = None;
        self.creatures = creatures.to_vec();
        self.creature_names = creatures
            .iter()
            .map(|creature| (creature.id, creature.name.clone()))
            .collect();
        if self
            .selected_creature
            .is_none_or(|selected| !creatures.iter().any(|creature| creature.id == selected))
        {
            self.selected_creature = creatures.first().map(|creature| creature.id);
        }
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

    pub fn select_about(&mut self) {
        self.tab = SettingsTab::About;
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
        update_status: &UpdateStatus,
        automatic_update_checks: bool,
        creatures: &[Creature],
    ) -> Result<SettingsOutcome> {
        self.creatures = creatures.to_vec();
        for creature in creatures {
            self.creature_names
                .entry(creature.id)
                .or_insert_with(|| creature.name.clone());
        }
        let input = self.state.take_egui_input(&self.window);
        let context = self.context.clone();
        let mut outcome = SettingsOutcome::default();
        let mut draft = self.draft.clone();
        let saved = self.saved.clone();
        let save_location = self.save_location.clone();
        let mut tab = self.tab;
        let mut error = self.error.clone();
        let editor_active = self.editor_active;
        let creatures = self.creatures.clone();
        let mut creature_names = self.creature_names.clone();
        let mut selected_creature = self.selected_creature;
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
                update_status,
                automatic_update_checks,
                &creatures,
                &mut creature_names,
                &mut selected_creature,
                &mut outcome,
            );
        });
        self.draft = draft;
        self.tab = tab;
        self.error = error;
        self.creature_names = creature_names;
        self.selected_creature = selected_creature;
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
    update_status: &UpdateStatus,
    automatic_update_checks: bool,
    creatures: &[Creature],
    creature_names: &mut BTreeMap<CreatureId, String>,
    selected_creature: &mut Option<CreatureId>,
    outcome: &mut SettingsOutcome,
) {
    egui::CentralPanel::default().show(root, |ui| {
        ui.heading("Formiga");
        ui.label(egui::RichText::new("A quiet little ecosystem for your desktop").italics());
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            for (candidate, label) in [
                (SettingsTab::General, "General"),
                (
                    SettingsTab::Colony,
                    if creatures.iter().any(|creature| {
                        creature.memory.profile_revision > creature.memory.viewed_profile_revision
                    }) {
                        "Colony •"
                    } else {
                        "Colony"
                    },
                ),
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
            SettingsTab::Colony => colony_tab(
                ui,
                creatures,
                creature_names,
                selected_creature,
                monitors,
                error,
                outcome,
            ),
            SettingsTab::Habitat => habitat_tab(ui, settings, monitors, editor_active, outcome),
            SettingsTab::Applications => applications_tab(ui, settings, windows, outcome),
            SettingsTab::About => about_tab(
                ui,
                outcome,
                save_location,
                update_status,
                automatic_update_checks,
            ),
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
        "Allow creatures to be petted and dragged",
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

fn colony_tab(
    ui: &mut egui::Ui,
    creatures: &[Creature],
    creature_names: &mut BTreeMap<CreatureId, String>,
    selected_creature: &mut Option<CreatureId>,
    monitors: &[MonitorInfo],
    error: &mut Option<String>,
    outcome: &mut SettingsOutcome,
) {
    if creatures.is_empty() {
        ui.label("Your first creature is still finding its way here.");
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for creature in creatures {
            let unread = creature.memory.profile_revision > creature.memory.viewed_profile_revision;
            let label = if unread {
                format!("{} •", creature.name)
            } else {
                creature.name.clone()
            };
            ui.selectable_value(selected_creature, Some(creature.id), label);
        }
    });
    ui.add_space(8.0);
    let Some(creature) = selected_creature
        .and_then(|selected| creatures.iter().find(|creature| creature.id == selected))
        .or_else(|| creatures.first())
    else {
        return;
    };
    *selected_creature = Some(creature.id);
    outcome.viewed_profile = Some(creature.id);

    ui.heading(&creature.name);
    ui.label(
        format!("{:?}", creature.appearance.family).replace("SoftQuadruped", "Soft Quadruped"),
    );
    let descriptors = profile_descriptors(creature);
    if descriptors.is_empty() {
        ui.label(egui::RichText::new("Still developing preferences").italics());
    } else {
        ui.label(
            descriptors
                .iter()
                .map(|descriptor| descriptor.label())
                .collect::<Vec<_>>()
                .join(" • "),
        );
    }

    ui.add_space(8.0);
    ui.group(|ui| {
        ui.strong("Name");
        ui.label("This is the only part of a creature profile that can be changed.");
        let name = creature_names
            .entry(creature.id)
            .or_insert_with(|| creature.name.clone());
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(name).char_limit(24));
            if ui.button("Save name").clicked() {
                match validate_creature_name(name) {
                    Ok(validated) => {
                        *name = validated.clone();
                        outcome.rename_creature = Some((creature.id, validated));
                        *error = None;
                    }
                    Err(message) => *error = Some(message.to_string()),
                }
            }
        });
    });

    ui.add_space(8.0);
    ui.strong("Life here");
    let days_alive = (time::OffsetDateTime::now_utc() - creature.born_at_utc)
        .whole_days()
        .max(0);
    ui.label(format!("Has lived here for {days_alive} days"));
    ui.label(format!(
        "Has found {} trinkets",
        creature.memory.discoveries_found
    ));
    ui.label(format!(
        "Has climbed {} windows",
        creature.memory.window_climbs
    ));
    ui.label(format!(
        "Has been petted {} times",
        creature.memory.times_petted
    ));

    if let Some(preferred) = creature.memory.preferred_region {
        ui.label(format!(
            "Favorite region: {} on {}",
            region_label(preferred.cell),
            display_label(preferred.display, monitors)
        ));
    } else if let Some(favorite) = creature.memory.favorite_display {
        ui.label(format!(
            "Favorite display: {}",
            display_label(favorite.display, monitors)
        ));
    } else {
        ui.label("Favorite place: still deciding");
    }

    let closest_friend = creature
        .state
        .relationships
        .iter()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .and_then(|(id, _)| creatures.iter().find(|other| other.id == *id));
    if let Some(friend) = closest_friend {
        ui.label(format!("Closest to {}", friend.name));
    } else {
        ui.label("Closest friend: still getting acquainted");
    }
}

fn display_label(display: formiga_core::DisplayKey, monitors: &[MonitorInfo]) -> String {
    monitors
        .iter()
        .position(|monitor| monitor.display_key == display)
        .map_or_else(
            || "a previous display".to_owned(),
            |index| format!("Display {}", index + 1),
        )
}

fn region_label(cell: u8) -> &'static str {
    [
        "upper left",
        "upper center",
        "upper right",
        "middle left",
        "center",
        "middle right",
        "lower left",
        "lower center",
        "lower right",
    ]
    .get(usize::from(cell.min(8)))
    .copied()
    .unwrap_or("center")
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
    ui.checkbox(
        &mut settings.fullscreen_app_occlusion,
        "Hide creatures behind full-screen applications",
    );
    ui.label(
        "Enabled by default. Detection uses only window and display bounds, without reading application content.",
    );
    ui.separator();
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

fn about_tab(
    ui: &mut egui::Ui,
    outcome: &mut SettingsOutcome,
    save_location: &str,
    update_status: &UpdateStatus,
    automatic_update_checks: bool,
) {
    ui.heading(format!("Formiga {APP_VERSION}"));
    ui.label("Procedural desktop fauna, generated and simulated entirely on your computer.");
    ui.add_space(8.0);
    ui.label(
        "No accounts, screenshots, window titles, keystrokes, behavioral uploads, or telemetry.",
    );
    ui.label("Optional update checks contact only the public Formiga repository on GitHub.");
    let mut automatic = automatic_update_checks;
    if ui
        .checkbox(&mut automatic, "Check GitHub for updates automatically")
        .changed()
    {
        outcome.automatic_update_checks = Some(automatic);
    }
    ui.add_space(6.0);
    ui.group(|ui| {
        ui.strong("Updates");
        match update_status {
            UpdateStatus::Idle => {
                ui.label("No update check has run in this session.");
                if ui.button("Check for Updates…").clicked() {
                    outcome.check_updates = true;
                }
            }
            UpdateStatus::Checking => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Checking GitHub Releases…");
                });
            }
            UpdateStatus::UpToDate { .. } => {
                ui.label("Formiga is up to date.");
                if ui.button("Check Again").clicked() {
                    outcome.check_updates = true;
                }
            }
            UpdateStatus::Available(release) => {
                let preview = if release.prerelease { " preview" } else { "" };
                ui.label(format!(
                    "Formiga {}{preview} is available.",
                    release.version
                ));
                if !release.notes.trim().is_empty() {
                    ui.collapsing("Release notes", |ui| {
                        ui.label(release.notes.trim());
                    });
                }
                ui.hyperlink_to("View this release on GitHub", &release.page_url);
                if ui.button("Download Verified Update").clicked() {
                    outcome.download_update = true;
                }
            }
            UpdateStatus::Downloading(release) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!("Downloading Formiga {}…", release.version));
                });
            }
            UpdateStatus::Ready(downloaded) => {
                ui.label(format!(
                    "Formiga {} is downloaded and SHA-256 verified.",
                    downloaded.release.version
                ));
                #[cfg(target_os = "windows")]
                let install_label = "Run Update Installer and Quit Formiga";
                #[cfg(target_os = "macos")]
                let install_label = "Open Update Disk Image";
                if ui.button(install_label).clicked() {
                    outcome.install_update = true;
                }
            }
            UpdateStatus::Failed(message) => {
                ui.colored_label(
                    egui::Color32::from_rgb(241, 142, 119),
                    format!("Update check failed: {message}"),
                );
                if ui.button("Try Again").clicked() {
                    outcome.check_updates = true;
                }
            }
        }
    });
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
