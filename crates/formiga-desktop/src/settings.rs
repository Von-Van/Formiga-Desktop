use crate::clubhouse::{self, Clubhouse, forest, gold, ink, mint, paper, rail, rail_ink};
use crate::updater::{APP_VERSION, UpdateStatus};
use anyhow::{Context as _, Result};
use formiga_core::{
    ActionKind, AppearancePreferences, ApplicationOcclusionRule, Creature, CreatureId,
    CreatureRelationship, DesktopRect, DesktopWindow, HabitatPolicy, HabitatPreset, HabitatZone,
    HabitatZoneKind, MonitorInfo, Settings, SharedCreatureSeed, ThemeChoice, closest_companion,
    encode_creature_seed, profile_descriptors, relationship_between, validate_creature_name,
    validate_habitat,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsTab {
    General,
    #[default]
    Colony,
    Studio,
    Home,
    Journal,
    Habitat,
    Applications,
    About,
}

#[derive(Default)]
pub struct SettingsOutcome {
    pub preview_shared: Option<SharedCreatureSeed>,
    pub quiet_minutes: Option<u16>,
    pub complete_onboarding: bool,
    pub home_corner: Option<formiga_core::HomeCorner>,
    pub home_display: Option<formiga_core::DisplayKey>,
    pub hidden_decorations: Option<u8>,
    pub move_object: Option<(usize, usize)>,
    pub save_mode: Option<(usize, formiga_core::BehaviorPreset)>,
    pub export_colony: bool,
    pub restore_colony: bool,
    pub start_fresh_recovery: bool,
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
    pub export_creature_card: Option<CreatureId>,
    pub export_creature_sticker: Option<(CreatureId, formiga_art::StickerClip, u32)>,
    pub export_colony_card: bool,
    pub set_creature_kept: Option<(CreatureId, bool)>,
    pub remove_creature: Option<CreatureId>,
    pub request_random_creature: bool,
    pub request_reference_creature: bool,
    pub accept_creature_preview: Option<PreviewAcceptance>,
    /// A friend's code, asked over for a day.
    pub invite_visitor: Option<SharedCreatureSeed>,
    /// Whoever is visiting right now, asked to stay for good.
    pub ask_visitor_to_stay: bool,
    pub regenerate_unkept: bool,
    pub appearance: Option<AppearancePreferences>,
    pub schedule: Option<formiga_core::RoutineSchedule>,
    pub resume_routine: bool,
    pub pin_moment: Option<formiga_core::JournalEntry>,
    pub unpin_moment: Option<formiga_core::JournalEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewAcceptance {
    Shared {
        shared: SharedCreatureSeed,
        replace: Option<CreatureId>,
    },
    Add {
        design: Option<formiga_core::CreatureDesign>,
        source_seed: [u8; 32],
    },
    Replace {
        design: Option<formiga_core::CreatureDesign>,
        creature_id: CreatureId,
        source_seed: [u8; 32],
    },
}

#[derive(Clone)]
pub struct GenerationPreview {
    pub shared: Option<SharedCreatureSeed>,
    pub creature: Creature,
    pub source_seed: [u8; 32],
    pub similarity: Option<u8>,
    pub summary: String,
}

#[derive(Clone, Copy)]
pub struct ColonyView<'a> {
    pub save: &'a formiga_core::SaveFile,
    pub creatures: &'a [Creature],
    pub relationships: &'a [CreatureRelationship],
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
    applied_appearance: Option<AppearancePreferences>,
    applied_system_theme: Option<egui::Theme>,
    draft: Settings,
    saved: Settings,
    save_location: String,
    tab: SettingsTab,
    error: Option<String>,
    editor_active: bool,
    creatures: Vec<Creature>,
    relationships: Vec<CreatureRelationship>,
    creature_names: BTreeMap<CreatureId, String>,
    selected_creature: Option<CreatureId>,
    pub clubhouse: Clubhouse,
    pub repaint_due: Option<std::time::Instant>,
    ui_visible: bool,
    occluded: bool,
    remove_confirmation: Option<CreatureId>,
    bulk_confirmation: bool,
    /// Text the desktop asked to put on the clipboard. egui owns the clipboard, and it only hands
    /// anything over from inside a frame, so the request waits here for the next one.
    pending_copy: Option<String>,
}

impl SettingsWindow {
    pub async fn new(
        event_loop: &ActiveEventLoop,
        settings: &Settings,
        creatures: &[Creature],
        relationships: &[CreatureRelationship],
        save_location: &std::path::Path,
    ) -> Result<Self> {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Formiga · Your colony")
                        .with_inner_size(LogicalSize::new(940.0, 720.0))
                        .with_min_inner_size(LogicalSize::new(760.0, 560.0))
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
        configure_style(&context, AppearancePreferences::default());
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
            applied_appearance: None,
            applied_system_theme: None,
            draft: settings.clone(),
            saved: settings.clone(),
            save_location: save_location.display().to_string(),
            tab: SettingsTab::default(),
            error: None,
            editor_active: false,
            creatures: creatures.to_vec(),
            relationships: relationships.to_vec(),
            creature_names: creatures
                .iter()
                .map(|creature| (creature.id, creature.name.clone()))
                .collect(),
            selected_creature: creatures.first().map(|creature| creature.id),
            clubhouse: Clubhouse::default(),
            repaint_due: None,
            ui_visible: false,
            occluded: false,
            remove_confirmation: None,
            bulk_confirmation: false,
            pending_copy: None,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn show(
        &mut self,
        settings: &Settings,
        creatures: &[Creature],
        relationships: &[CreatureRelationship],
    ) {
        self.draft = settings.clone();
        self.saved = settings.clone();
        self.error = None;
        self.creatures = creatures.to_vec();
        self.relationships = relationships.to_vec();
        self.creature_names = creatures
            .iter()
            .map(|creature| (creature.id, creature.name.clone()))
            .collect();
        self.ui_visible = true;
        self.occluded = false;
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

    pub fn hide(&mut self) {
        self.window.set_visible(false);
        self.ui_visible = false;
        self.repaint_due = None;
        for id in self.clubhouse.texture_ids() {
            self.renderer.free_texture(&id);
        }
        self.clubhouse.release_images();
        // A close can arrive before the first preview repaint. Drop pending artwork uploads,
        // while preserving any font update needed when the same window opens again.
        let mut pending = self.context.tex_manager().write().take_delta();
        for (id, deltas) in &pending.set {
            if !pending.free.contains(id) {
                for delta in deltas {
                    self.renderer
                        .update_texture(&self.device, &self.queue, *id, delta);
                }
            }
        }
        for id in &pending.free {
            self.renderer.free_texture(id);
        }
        pending.clear();
    }

    pub fn set_generation_preview(&mut self, preview: GenerationPreview) {
        self.clubhouse.push_preview(&self.context, preview);
        self.tab = SettingsTab::Studio;
        self.error = None;
        self.window.request_redraw();
    }

    pub fn acknowledge_preferences(&mut self, settings: &Settings) {
        self.saved = settings.clone();
        self.draft = settings.clone();
        self.window.request_redraw();
    }

    pub fn notify(&mut self, message: impl Into<String>) {
        self.clubhouse.notify(message);
        self.window.request_redraw();
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
        self.window.request_redraw();
    }

    pub fn clear_generation_preview(&mut self) {
        self.clubhouse.clear_previews();
        self.window.request_redraw();
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

    /// Open the Colony page on one creature, for the profile item of its right-click menu. Call
    /// this after `show`, which re-checks the selection against the colony it is handed.
    pub fn select_creature(&mut self, creature_id: CreatureId) {
        if self.creatures.iter().any(|c| c.id == creature_id) {
            self.selected_creature = Some(creature_id);
        }
        self.tab = SettingsTab::Colony;
        self.window.request_redraw();
    }

    /// Open the Journal page, which is where a visit is written down.
    pub fn select_journal(&mut self) {
        self.tab = SettingsTab::Journal;
        self.window.request_redraw();
    }

    /// Put text on the system clipboard through egui, exactly as the Colony page's copy buttons
    /// do. It is handed over on the next frame, when egui's platform output is delivered.
    pub fn copy_text(&mut self, text: String, notice: impl Into<String>) {
        self.pending_copy = Some(text);
        self.clubhouse.notify(notice);
        self.window.request_redraw();
    }

    pub fn on_event(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::Occluded(occluded) = event {
            self.occluded = *occluded;
            if *occluded {
                self.repaint_due = None;
            } else if self.ui_visible {
                self.window.request_redraw();
            }
        }
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
        colony: ColonyView<'_>,
    ) -> Result<SettingsOutcome> {
        if !self.ui_visible || self.occluded {
            return Ok(SettingsOutcome::default());
        }
        // The window follows the saved preference, and re-reads the system's appearance with it,
        // so "match system" keeps up without anything polling for it.
        let appearance = colony.save.companion.appearance;
        if self.applied_appearance != Some(appearance)
            || (appearance.theme == ThemeChoice::System
                && self.applied_system_theme != self.context.system_theme())
        {
            configure_style(&self.context, appearance);
            self.applied_appearance = Some(appearance);
            self.applied_system_theme = self.context.system_theme();
        }
        self.creatures = colony.creatures.to_vec();
        self.relationships = colony.relationships.to_vec();
        self.creature_names
            .retain(|id, _| colony.creatures.iter().any(|c| c.id == *id));
        for creature in colony.creatures {
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
        let relationships = self.relationships.clone();
        let mut creature_names = self.creature_names.clone();
        let mut selected_creature = self.selected_creature;
        self.clubhouse.retain_portraits(&creatures);
        let mut remove_confirmation = self.remove_confirmation;
        let mut bulk_confirmation = self.bulk_confirmation;
        let mut pending_copy = self.pending_copy.take();
        let mut full_output = context.run_ui(input, |ui| {
            // `take` rather than a clone: egui may run a frame's UI more than once, and the
            // clipboard should be written exactly as often as the owner asked for it.
            if let Some(text) = pending_copy.take() {
                ui.ctx().copy_text(text);
            }
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
                &relationships,
                &mut creature_names,
                &mut selected_creature,
                &mut self.clubhouse,
                colony.save,
                &mut remove_confirmation,
                &mut bulk_confirmation,
                &mut outcome,
            );
        });
        self.draft = draft;
        self.tab = tab;
        self.error = error;
        self.creature_names = creature_names;
        self.selected_creature = selected_creature;
        self.repaint_due = if self.ui_visible {
            full_output
                .viewport_output
                .get(&egui::ViewportId::ROOT)
                .and_then(|v| {
                    std::time::Instant::now()
                        .checked_add(v.repaint_delay.max(std::time::Duration::from_millis(50)))
                })
        } else {
            None
        };
        self.remove_confirmation = remove_confirmation;
        self.bulk_confirmation = bulk_confirmation;

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
        let texture_frees: Vec<_> = full_output.textures_delta.free.drain().collect();
        full_output.textures_delta.clear();
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
                for id in &texture_frees {
                    self.renderer.free_texture(id);
                }
                self.repaint_due =
                    std::time::Instant::now().checked_add(std::time::Duration::from_millis(250));
                return Ok(outcome);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                for id in &texture_frees {
                    self.renderer.free_texture(id);
                }
                self.repaint_due =
                    std::time::Instant::now().checked_add(std::time::Duration::from_millis(250));
                return Ok(outcome);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                for id in &texture_frees {
                    self.renderer.free_texture(id);
                }
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
        for id in &texture_frees {
            self.renderer.free_texture(id);
        }
        Ok(outcome)
    }
}

/// Apply the reader's chosen appearance. Called whenever the preference or the system's own
/// appearance changes, which is the only thing that moves the interface between themes.
fn configure_style(context: &egui::Context, appearance: AppearancePreferences) {
    let dark = match appearance.theme {
        ThemeChoice::Dark => true,
        ThemeChoice::Light => false,
        ThemeChoice::System => context.system_theme() == Some(egui::Theme::Dark),
    };
    clubhouse::set_dark_interface(dark);
    let theme = if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    context.set_theme(theme);
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = paper();
    visuals.window_fill = paper();
    visuals.override_text_color = Some(ink());
    visuals.extreme_bg_color = if dark {
        egui::Color32::from_rgb(23, 28, 26)
    } else {
        egui::Color32::from_rgb(255, 250, 234)
    };
    visuals.faint_bg_color = if dark {
        egui::Color32::from_rgb(34, 42, 38)
    } else {
        egui::Color32::from_rgb(239, 225, 188)
    };
    visuals.selection.bg_fill = mint();
    visuals.selection.stroke = egui::Stroke::new(1.0, forest());
    let inactive = if dark {
        egui::Color32::from_rgb(45, 54, 49)
    } else {
        egui::Color32::from_rgb(242, 230, 196)
    };
    visuals.widgets.inactive.bg_fill = inactive;
    visuals.widgets.inactive.weak_bg_fill = inactive;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, gold());
    visuals.widgets.hovered.bg_fill = mint();
    visuals.widgets.hovered.weak_bg_fill = mint();
    visuals.widgets.active.bg_fill = mint();
    visuals.widgets.active.weak_bg_fill = mint();
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, gold());
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, ink());
    // Keyboard focus has to be obvious in either theme, so it is drawn in the accent rather
    // than in the platform's own faint default.
    visuals.widgets.active.bg_stroke = egui::Stroke::new(2.0, forest());
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, forest());
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, ink());
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, ink());
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, ink());
    // A disabled control stays legible: dimmed, never invisible.
    visuals.widgets.noninteractive.weak_bg_fill = inactive;
    context.set_visuals(visuals);
    // Modest text scaling, applied to every style the window uses so nothing is left behind.
    let scale = f32::from(appearance.text_scale.clamp(100, 150)) / 100.0;
    context.style_mut_of(theme, |style| {
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(12.0, 8.0);
        style.spacing.interact_size.y = 32.0 * scale;
        style.spacing.interact_size.x *= scale;
        style.spacing.icon_width *= scale;
        style.spacing.icon_width_inner *= scale;
        style.animation_time = 0.0;
        for (text_style, size) in [
            (egui::TextStyle::Body, 14.0),
            (egui::TextStyle::Button, 14.0),
            (egui::TextStyle::Heading, 25.0),
            (egui::TextStyle::Small, 10.5),
            (egui::TextStyle::Monospace, 13.0),
        ] {
            style
                .text_styles
                .insert(text_style, egui::FontId::proportional(size * scale));
        }
    });
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
    relationships: &[CreatureRelationship],
    creature_names: &mut BTreeMap<CreatureId, String>,
    selected_creature: &mut Option<CreatureId>,
    clubhouse: &mut Clubhouse,
    save: &formiga_core::SaveFile,
    remove_confirmation: &mut Option<CreatureId>,
    bulk_confirmation: &mut bool,
    outcome: &mut SettingsOutcome,
) {
    // The rail grows with the text and scrolls if it still does not fit, so every page stays
    // reachable at the smallest window the app allows and the largest text it offers.
    let text_scale = root.text_style_height(&egui::TextStyle::Body) / 14.0;
    egui::Panel::left("colony-navigation")
        .exact_size(176.0 * text_scale.clamp(1.0, 1.5))
        .resizable(false)
        .frame(egui::Frame::new().fill(rail()).inner_margin(18))
        .show(root, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new("FORMIGA").size(25.0).color(rail_ink()));
                    ui.label(
                        egui::RichText::new("A little life here.")
                            .size(12.0)
                            .color(rail_ink()),
                    );
                    ui.add_space(28.0);
                    for (candidate, label) in [
                        (SettingsTab::Colony, "Your colony"),
                        (SettingsTab::Studio, "Creature studio"),
                        (SettingsTab::Home, "Home & keepsakes"),
                        (SettingsTab::Journal, "Journal"),
                        (SettingsTab::Habitat, "Habitat"),
                        (SettingsTab::Applications, "Applications"),
                        (SettingsTab::General, "Preferences"),
                        (SettingsTab::About, "About & backups"),
                    ] {
                        let active = *tab == candidate;
                        let label = if candidate == SettingsTab::Colony
                            && creatures.iter().any(|c| {
                                c.memory.profile_revision > c.memory.viewed_profile_revision
                            }) {
                            format!("{label} •")
                        } else {
                            label.to_owned()
                        };
                        let button =
                            egui::Button::new(egui::RichText::new(label).color(if active {
                                ink()
                            } else {
                                rail_ink()
                            }))
                            .fill(if active { mint() } else { rail() })
                            .stroke(egui::Stroke::NONE);
                        if ui
                            .add_sized([140.0 * text_scale, 38.0 * text_scale], button)
                            .clicked()
                        {
                            *tab = candidate;
                            *error = None;
                        }
                    }
                    ui.add_space(24.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "{} companions\nEntirely on your computer",
                            creatures.len()
                        ))
                        .small()
                        .color(rail_ink()),
                    );
                });
        });
    egui::Panel::bottom("settings-footer")
        .frame(egui::Frame::new().fill(paper()).inner_margin(14))
        .show(root, |ui| {
            if let Some(message) = error.as_deref() {
                ui.colored_label(egui::Color32::from_rgb(145, 58, 44), message);
            }
            if let Some((message, started)) = &clubhouse.feedback {
                if started.elapsed() < std::time::Duration::from_secs(4) {
                    ui.colored_label(forest(), message);
                    ui.ctx().request_repaint_after(
                        std::time::Duration::from_secs(4).saturating_sub(started.elapsed()),
                    );
                } else {
                    clubhouse.feedback = None;
                }
            }
            let adoption_footer =
                *tab == SettingsTab::Studio && clubhouse.adoption_footer(ui, save, outcome);
            if !adoption_footer || settings != saved {
                ui.horizontal(|ui| {
                    let dirty = settings != saved;
                    ui.label(if dirty {
                        "Unapplied preferences"
                    } else {
                        "Preferences are saved"
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                dirty && !editor_active,
                                egui::Button::new("Apply changes").fill(mint()),
                            )
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
                        if ui.add_enabled(dirty, egui::Button::new("Revert")).clicked() {
                            *settings = saved.clone();
                            *error = None;
                            clubhouse.notify("Preferences reverted");
                        }
                    });
                });
            }
        });
    egui::CentralPanel::default().frame(egui::Frame::new().fill(paper()).inner_margin(24)).show(root, |ui| {
        egui::ScrollArea::vertical().id_salt(format!("page-{tab:?}")).auto_shrink([false, false]).show(ui, |ui| {
            if let Some(reason) = clubhouse.recovery.clone() {
                clubhouse::card(ui, |ui| {
                    ui.heading("Your saved colony needs attention");
                    ui.label(reason);
                    ui.label("Your original files are untouched. This temporary colony will not overwrite them. Restore a backup below, or preserve the files and start fresh.");
                    if ui.button("Restore a colony backup…").clicked() { outcome.restore_colony = true; }
                    ui.checkbox(&mut clubhouse.fresh_confirmed, "Keep recovery copies and start a new colony");
                    if ui.add_enabled(clubhouse.fresh_confirmed, egui::Button::new("Start fresh with recovery copies")).clicked() { outcome.start_fresh_recovery = true; }
                });
                ui.add_space(16.0);
            }
            match tab {
                SettingsTab::Colony => {
                    clubhouse::title(ui, "Your colony", "Familiar faces. Small adventures. A home that grows.");
                    clubhouse.intro(ui, save, outcome);
                    colony_tab(ui, ColonyView { creatures, relationships, save }, creature_names, selected_creature, monitors, error, remove_confirmation, bulk_confirmation, clubhouse, outcome);
                }
                SettingsTab::Studio => clubhouse.studio(ui, save, selected_creature, outcome),
                SettingsTab::Home => clubhouse.home(ui, save, monitors, outcome),
                SettingsTab::Journal => clubhouse::journal(ui, save, clubhouse, outcome),
                SettingsTab::General => general_tab(ui, settings, save, clubhouse, outcome),
                SettingsTab::Habitat => {
                    clubhouse::title(ui, "Room to roam", "Choose where your companions feel at home.");
                    clubhouse::habitat_map(ui, &settings.habitat, monitors);
                    habitat_tab(ui, settings, monitors, editor_active, outcome);
                }
                SettingsTab::Applications => {
                    clubhouse::title(ui, "Space for your work", "Choose which windows can cover your companions.");
                    applications_tab(ui, settings, windows, outcome);
                }
                SettingsTab::About => {
                    clubhouse::title(ui, "Made for a quiet desktop", "Your colony belongs to you.");
                    clubhouse::card(ui, |ui| {
                        ui.strong("Colony backups");
                        ui.label("A full backup includes names, memories, relationships, the journal, and preferences. Keep it private or move it to another computer.");
                        if ui.button("Export full colony…").clicked() { outcome.export_colony = true; }
                        ui.checkbox(&mut clubhouse.restore_confirmed, "Restore a backup in place of this colony; keep recovery copies first");
                        if ui.add_enabled(clubhouse.restore_confirmed, egui::Button::new("Choose backup to restore…")).clicked() { outcome.restore_colony = true; }
                    });
                    ui.add_space(18.0);
                    about_tab(ui, outcome, save_location, update_status, automatic_update_checks);
                }
            }
        });
    });
}

fn general_tab(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    save: &formiga_core::SaveFile,
    clubhouse: &mut Clubhouse,
    outcome: &mut SettingsOutcome,
) {
    clubhouse::title(
        ui,
        "At your own pace",
        "A few gentle adjustments for your desktop.",
    );
    clubhouse::quiet_controls(ui, save, outcome);
    ui.add_space(16.0);
    clubhouse::card(ui, |ui| {
        ui.strong("Saved routines");
        ui.small("Keep a Work and Relax setup for habitat, movement, and cursor reactions. Loading a routine previews its preferences before Apply.");
        for (index, name) in ["Work", "Relax"].into_iter().enumerate() {
            ui.horizontal(|ui| {
                ui.strong(name);
                if ui.button("Save current preferences").clicked() {
                    outcome.save_mode =
                        Some((index, formiga_core::BehaviorPreset::capture(settings)));
                }
                if ui
                    .add_enabled(
                        save.companion.modes[index].is_some(),
                        egui::Button::new("Load"),
                    )
                    .clicked()
                    && let Some(mode) = &save.companion.modes[index]
                {
                    mode.apply(settings);
                    clubhouse.notify(format!("{name} routine loaded · Apply to use it"));
                }
            });
        }
    });
    ui.add_space(16.0);
    clubhouse::schedule_controls(ui, save, outcome);
    ui.add_space(16.0);
    clubhouse::appearance_controls(ui, save, outcome);
    ui.add_space(18.0);
    ui.strong("On your desktop");
    ui.checkbox(&mut settings.visible, "Show colony");
    ui.checkbox(&mut settings.paused, "Pause ambient behavior");
    ui.checkbox(
        &mut settings.direct_manipulation,
        "Allow petting and dragging",
    );
    ui.checkbox(&mut settings.cursor_reactions, "React to cursor movement");
    ui.checkbox(
        &mut settings.window_ledges,
        "Explore application-window ledges",
    );
    ui.checkbox(&mut settings.reduce_motion, "Reduce motion");
    ui.checkbox(&mut settings.launch_at_login, "Launch at login");
    ui.add_space(10.0);
    ui.strong("Creature size");
    ui.horizontal(|ui| {
        for (scale, name) in [(2, "Small"), (3, "Medium"), (4, "Large")] {
            ui.selectable_value(&mut settings.display_scale, scale, name);
        }
    });
    ui.add_space(18.0);
    if ui.button("Show introduction again").clicked() {
        clubhouse.show_intro = true;
        clubhouse.onboarding_step = 0;
        clubhouse.notify("Introduction ready in Your colony");
    }
}

#[allow(clippy::too_many_arguments)]
fn colony_tab(
    ui: &mut egui::Ui,
    colony: ColonyView<'_>,
    creature_names: &mut BTreeMap<CreatureId, String>,
    selected_creature: &mut Option<CreatureId>,
    monitors: &[MonitorInfo],
    error: &mut Option<String>,
    remove_confirmation: &mut Option<CreatureId>,
    bulk_confirmation: &mut bool,
    clubhouse: &mut Clubhouse,
    outcome: &mut SettingsOutcome,
) {
    let creatures = colony.creatures;
    let relationships = colony.relationships;
    ui.horizontal_wrapped(|ui| {
        for creature in creatures {
            ui.selectable_value(selected_creature, Some(creature.id), &creature.name);
        }
    });
    let Some(creature) = selected_creature
        .and_then(|id| creatures.iter().find(|c| c.id == id))
        .or_else(|| creatures.first())
    else {
        return;
    };
    *selected_creature = Some(creature.id);
    outcome.viewed_profile = Some(creature.id);
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        clubhouse.portrait(ui, creature, 144.0);
        ui.vertical(|ui| {
            ui.heading(&creature.name);
            ui.label(creature.appearance.design.map_or_else(
                || clubhouse::words(&format!("{:?}", creature.appearance.family)),
                |d| d.body.label().to_owned(),
            ));
            ui.label(format!(
                "Currently {}",
                activity_label(creature.state.action)
            ));
            if let Some(parent_id) = creature.role.parent_id() {
                ui.small(format!(
                    "Mini cared for by {}",
                    creatures
                        .iter()
                        .find(|c| c.id == parent_id)
                        .map_or("a colony adult", |c| c.name.as_str())
                ));
            }
            let descriptors = profile_descriptors(creature);
            if descriptors.is_empty() {
                ui.small("Still developing preferences");
            }
            ui.horizontal_wrapped(|ui| {
                for descriptor in descriptors {
                    egui::Frame::new()
                        .fill(mint())
                        .inner_margin(5)
                        .show(ui, |ui| {
                            ui.label(descriptor.label());
                        });
                }
            });
        });
    });
    ui.add_space(18.0);
    ui.strong("Life here");
    let days_alive = (time::OffsetDateTime::now_utc() - creature.born_at_utc)
        .whole_days()
        .max(0);
    ui.horizontal_wrapped(|ui| {
        for (number, label) in [
            (days_alive.to_string(), "days together"),
            (creature.memory.times_petted.to_string(), "pets received"),
            (
                creature.memory.discoveries_found.to_string(),
                "treasures found",
            ),
            (creature.memory.window_climbs.to_string(), "windows climbed"),
        ] {
            clubhouse::card(ui, |ui| {
                ui.set_min_width(86.0);
                ui.label(egui::RichText::new(number).size(24.0).color(forest()));
                ui.small(label);
            });
        }
    });
    ui.add_space(8.0);
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

    let closest_friend = closest_companion(relationships, creature.id)
        .and_then(|id| creatures.iter().find(|other| other.id == id));
    if let Some(friend) = closest_friend {
        ui.label(format!("Closest to {}", friend.name));
        if let Some(relationship) = relationship_between(relationships, creature.id, friend.id) {
            ui.label(format!(
                "Bond: {} • {}",
                bond_label(relationship.affinity, relationship.avoidance),
                play_label(relationship.playfulness)
            ));
        }
    } else {
        ui.label("Closest friend: still getting acquainted");
    }
    ui.add_space(18.0);
    ui.group(|ui| {
        ui.strong("Name");
        ui.small("A name for your companion. Their preferences grow through experience.");
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
    ui.group(|ui| {
        ui.strong("Colony care");
        let mut kept = creature.kept;
        if ui
            .checkbox(&mut kept, "Keep this creature during bulk regeneration")
            .changed()
        {
            outcome.set_creature_kept = Some((creature.id, kept));
            *error = None;
        }
        ui.label(
            egui::RichText::new(
                "Individual replacement also requires Keep to be turned off first.",
            )
            .small(),
        );

        let adult_count = creatures
            .iter()
            .filter(|candidate| candidate.role.is_adult())
            .count();
        let can_remove = !(creature.role.is_adult() && adult_count == 1);
        if *remove_confirmation == Some(creature.id) {
            ui.colored_label(
                egui::Color32::from_rgb(241, 181, 102),
                format!("Remove {} and its local history?", creature.name),
            );
            ui.horizontal(|ui| {
                if ui.button("Remove permanently").clicked() {
                    outcome.remove_creature = Some(creature.id);
                    *remove_confirmation = None;
                }
                if ui.button("Cancel").clicked() {
                    *remove_confirmation = None;
                }
            });
        } else if ui
            .add_enabled(can_remove, egui::Button::new("Remove from colony…"))
            .clicked()
        {
            *remove_confirmation = Some(creature.id);
        }
        if !can_remove {
            ui.label(
                egui::RichText::new("A colony must keep at least one full-size creature.").small(),
            );
        }
    });

    ui.add_space(8.0);
    ui.group(|ui| {
        ui.strong("Share this creature");
        ui.label("The code recreates innate appearance and personality, not its name or history.");
        let code = encode_creature_seed(creature.origin);
        ui.horizontal(|ui| {
            ui.monospace(format!("{}…{}", &code[..15], &code[code.len() - 8..]));
            if ui.button("Copy seed").clicked() {
                ui.ctx().copy_text(code);
                clubhouse.notify("Creature code copied");
            }
        });
        ui.add_space(5.0);
        ui.label("Make a 960×600 illustrated keepsake using the visible profile details.");
        if ui.button("Export creature card…").clicked() {
            outcome.export_creature_card = Some(creature.id);
        }
        ui.add_space(8.0);
        ui.label("Or a looping sticker on a transparent background: just the drawing, animated.");
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("sticker-clip")
                .selected_text(clubhouse.sticker_clip.label())
                .show_ui(ui, |ui| {
                    for clip in formiga_art::StickerClip::ALL {
                        ui.selectable_value(&mut clubhouse.sticker_clip, clip, clip.label());
                    }
                });
            ui.selectable_value(&mut clubhouse.small_sticker, false, "8×");
            ui.selectable_value(&mut clubhouse.small_sticker, true, "4×");
            if ui.button("Export sticker…").clicked() {
                outcome.export_creature_sticker = Some((
                    creature.id,
                    clubhouse.sticker_clip,
                    clubhouse.sticker_scale(),
                ));
            }
        });
    });

    ui.add_space(8.0);

    ui.collapsing("Start over with unkept companions…", |ui| {
        let count = creatures.iter().filter(|c| !c.kept).count();
        ui.checkbox(
            bulk_confirmation,
            format!("Replace {count} unkept companions and their histories"),
        );
        if ui
            .add_enabled(
                *bulk_confirmation && count > 0,
                egui::Button::new("Regenerate unkept companions"),
            )
            .clicked()
        {
            outcome.regenerate_unkept = true;
            *bulk_confirmation = false;
        }
    });
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs(1));
}

fn activity_label(action: ActionKind) -> String {
    match action {
        ActionKind::Idle => "taking a quiet moment".into(),
        ActionKind::Traverse => "exploring".into(),
        ActionKind::Homebound => "settling at home".into(),
        ActionKind::PetReaction => "enjoying your company".into(),
        _ => clubhouse::words(&format!("{action:?}")).to_lowercase(),
    }
}

fn bond_label(affinity: u8, avoidance: u8) -> &'static str {
    if avoidance >= 160 {
        "keeping some distance"
    } else if affinity >= 192 {
        "devoted"
    } else if affinity >= 112 {
        "close"
    } else if affinity >= 48 {
        "warming up"
    } else {
        "new friends"
    }
}

fn play_label(playfulness: u8) -> &'static str {
    if playfulness >= 160 {
        "very playful"
    } else if playfulness >= 72 {
        "playful"
    } else {
        "gentle"
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

fn habitat_preset_label(preset: HabitatPreset) -> &'static str {
    match preset {
        HabitatPreset::EntireDesktop => "Entire desktop",
        HabitatPreset::PrimaryDisplay => "Primary display",
        HabitatPreset::BottomEdge => "Bottom edge",
        HabitatPreset::BottomCorners => "Bottom corners",
        HabitatPreset::LowerHalf => "Lower half",
        HabitatPreset::Custom => "Custom regions",
    }
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
        .selected_text(habitat_preset_label(settings.habitat.preset))
        .show_ui(ui, |ui| {
            for preset in [
                HabitatPreset::EntireDesktop,
                HabitatPreset::PrimaryDisplay,
                HabitatPreset::BottomEdge,
                HabitatPreset::BottomCorners,
                HabitatPreset::LowerHalf,
                HabitatPreset::Custom,
            ] {
                ui.selectable_value(
                    &mut settings.habitat.preset,
                    preset,
                    habitat_preset_label(preset),
                );
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
    ui.collapsing("Advanced · exact region coordinates", |ui| {
        ui.small("Coordinates are fractions of the selected display (0–1).");
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
                    ui.add(
                        egui::DragValue::new(&mut zone.normalized_bounds.width).range(0.05..=1.0),
                    );
                    ui.label("h");
                    ui.add(
                        egui::DragValue::new(&mut zone.normalized_bounds.height).range(0.05..=1.0),
                    );
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
    });
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

#[cfg(test)]
#[path = "settings_review.rs"]
mod review;
