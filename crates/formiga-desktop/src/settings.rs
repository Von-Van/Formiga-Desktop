use crate::clubhouse::tour::TourMark;
use crate::clubhouse::{self, Clubhouse, forest, gold, ink, mint, paper};
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
pub(crate) enum SettingsTab {
    #[default]
    Today,
    General,
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
    pub move_object: Option<(usize, usize)>,
    /// Put a hangout spot down at a fraction along the village ground, move it, or with `None`
    /// pick it up again.
    pub set_hangout: Option<(formiga_core::HangoutKind, Option<f32>)>,
    /// The cottages in a new order, as the companions who keep them.
    pub cottage_order: Option<Vec<CreatureId>>,
    /// Open the house this companion keeps in Formiga Home.
    pub open_house: Option<CreatureId>,
    /// Bring the household of the house open in Formiga Home back out.
    pub bring_household_back: bool,
    /// A house built as another type, by the companion who keeps it, or with `None` as its own.
    pub house_style: Option<(CreatureId, Option<formiga_core::ShelterStyle>)>,
    /// A named palette for the village, or `Some(None)` for its own colours again.
    pub village_palette: Option<Option<formiga_core::VillagePalette>>,
    /// Scenery to lay the village out on, or `Some(None)` for its strip again.
    pub village_scenery: Option<Option<formiga_core::VillageScenery>>,
    /// Plant a garden patch at a fraction along the village ground, move it, or with `None` dig
    /// it up.
    pub set_garden: Option<(formiga_core::GardenKind, Option<f32>)>,
    /// Set an ornament out at a fraction along the village ground, move it, or with `None` take
    /// it in.
    pub set_ornament: Option<(formiga_core::OrnamentKind, Option<f32>)>,
    /// Hang a decoration in one place on the house a companion keeps, or with `None` take down
    /// whatever is there.
    pub set_decoration: Option<(
        CreatureId,
        formiga_core::DecorationSlot,
        Option<formiga_core::ShelterDecorationKind>,
    )>,
    /// The keepsakes chosen to hang in the two trees, or `Some(None)` to let the trees fill
    /// themselves again.
    pub tree_keepsakes: Option<Option<[Option<u8>; formiga_core::TREE_HOOKS]>>,
    /// Something for a companion to wear, or `None` to take off whatever it is wearing.
    pub set_accessory: Option<(CreatureId, Option<formiga_core::Accessory>)>,
    /// The cottages, colours and gardens put back as the village grew.
    pub reset_village: bool,
    /// Take back the last change made to the colony.
    pub undo_last_edit: bool,
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
    /// A postcard of the colony in this scene, with this caption, which may be empty.
    pub export_postcard: Option<(formiga_art::PostcardScene, String)>,
    pub set_creature_kept: Option<(CreatureId, bool)>,
    pub remove_creature: Option<CreatureId>,
    pub request_random_creature: bool,
    pub request_reference_creature: bool,
    /// Four more takes on the picture the ones on show came from.
    pub request_reference_retry: bool,
    pub accept_creature_preview: Option<PreviewAcceptance>,
    /// A friend's code, asked over for a day.
    pub invite_visitor: Option<SharedCreatureSeed>,
    /// Whoever is visiting right now, asked to stay for good.
    pub ask_visitor_to_stay: bool,
    /// A visitor kept to invite again: the name it went by and the origin that recreates it.
    pub keep_favorite_visitor: Option<(String, formiga_core::CreatureOrigin)>,
    /// Where one companion's owner would like it to roam.
    pub set_roaming_leaning: Option<(CreatureId, formiga_core::RoamingLeaning)>,
    pub forget_favorite_visitor: Option<formiga_core::CreatureOrigin>,
    pub regenerate_unkept: bool,
    pub appearance: Option<AppearancePreferences>,
    pub schedule: Option<formiga_core::RoutineSchedule>,
    pub resume_routine: bool,
    pub pin_moment: Option<formiga_core::JournalEntry>,
    pub unpin_moment: Option<formiga_core::JournalEntry>,
    /// The journal has been read up to its newest moment.
    pub mark_journal_read: bool,
    /// Try writing the colony again, after a save that failed.
    pub retry_save: bool,
    /// Close the notebook, as its window's close button would: ⌘W, or Ctrl+W.
    pub close_notebook: bool,
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
        placement: crate::notebook_window::Placement,
        proxy: winit::event_loop::EventLoopProxy<crate::app::UserEvent>,
    ) -> Result<Self> {
        let (min_width, min_height) = crate::notebook_window::MIN_SIZE;
        let attributes = Window::default_attributes()
            .with_title("Formiga · Field notebook")
            .with_inner_size(placement.size)
            .with_min_inner_size(LogicalSize::new(min_width, min_height))
            .with_maximized(placement.maximized)
            .with_visible(false);
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .context("create settings window")?,
        );
        // The remembered corner is the window's outer one, title bar and all. Set after the window
        // exists, because on macOS a position given when it is created places the content's
        // corner instead, which would walk the window down by a title bar every time it opened.
        if let Some(position) = placement.position {
            window.set_outer_position(position);
        }
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
        let mut state = egui_winit::State::new(
            context.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(adapter.limits().max_texture_dimension_2d as usize),
        );
        // Screen readers can read and work the notebook. The adapter is set up while the window
        // is still hidden, as it has to be, and does nothing until an assistive app asks.
        state.init_accesskit(event_loop, window.as_ref(), proxy);
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

    /// Where the window is now and how large, to open it there next time. `None` while the
    /// system cannot say, or while it is minimised to the Dock or taskbar.
    pub fn geometry(&self) -> Option<crate::notebook_window::NotebookGeometry> {
        if self.window.is_minimized() == Some(true) {
            return None;
        }
        let position = self.window.outer_position().ok()?;
        let size = self
            .window
            .inner_size()
            .to_logical::<f64>(self.window.scale_factor());
        let maximized = self.window.is_maximized();
        (size.width > 0.0 && size.height > 0.0).then_some(
            crate::notebook_window::NotebookGeometry {
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
                maximized,
            },
        )
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
        self.clubhouse.studio.push_preview(&self.context, preview);
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
        self.clubhouse.studio.clear_previews();
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

    /// Open the Home page with the house `keeper` keeps picked out: its own settings, and the way
    /// into it in Formiga Home.
    pub fn select_house(&mut self, keeper: CreatureId) {
        self.clubhouse.home.arrange.picked = Some(crate::clubhouse::arrange::Picked::House(keeper));
        self.tab = SettingsTab::Home;
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

    /// A screen reader asked for the notebook's contents or to use one of its controls.
    pub fn on_accesskit(&mut self, event: egui_winit::accesskit_winit::WindowEvent) {
        use egui_winit::accesskit_winit::WindowEvent as Ask;
        match event {
            Ask::InitialTreeRequested => self.context.enable_accesskit(),
            Ask::ActionRequested(request) => self.state.on_accesskit_action_request(request),
            Ask::AccessibilityDeactivated => self.context.disable_accesskit(),
        }
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
        if appearance_changed(
            self.applied_appearance,
            self.applied_system_theme,
            appearance,
            self.context.system_theme(),
        ) {
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
        self.clubhouse.shell.retain_portraits(&creatures);
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

/// Whether the notebook has to apply its appearance again: the preference changed, or it follows
/// the system and the system's own appearance changed since it was last applied — the system
/// switching to dark at dusk, say, while the notebook is open.
pub(crate) fn appearance_changed(
    applied: Option<AppearancePreferences>,
    applied_system: Option<egui::Theme>,
    wanted: AppearancePreferences,
    system: Option<egui::Theme>,
) -> bool {
    applied != Some(wanted) || (wanted.theme == ThemeChoice::System && applied_system != system)
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
    // A field notebook has square, pixel edges: buttons and fields are outlined in the plum the
    // creatures are, two pixels wide, with nothing rounded anywhere.
    visuals.widgets.inactive.weak_bg_fill = clubhouse::card_fill();
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(2.0, clubhouse::line());
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(2.0, clubhouse::line());
    visuals.widgets.active.bg_stroke = egui::Stroke::new(2.0, forest());
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, gold());
    visuals.extreme_bg_color = clubhouse::field();
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::ZERO;
        widget.expansion = 0.0;
    }
    visuals.window_corner_radius = egui::CornerRadius::ZERO;
    visuals.menu_corner_radius = egui::CornerRadius::ZERO;
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
    // The window is a field notebook: a leather cover with the pages' tabs down its edge, the
    // open page beside it, and the binding to its right. The cover grows with the text, and in a
    // narrow window the page's margins give up their room before its contents do.
    let text_scale = clubhouse::journal::text_scale(root);
    let window = root.max_rect();
    let spread = clubhouse::journal::Spread::of(window.width(), text_scale);
    clubhouse::journal::paint_cover(root.painter(), window);
    let marks = clubhouse::journal::TabMarks {
        colony: creatures
            .iter()
            .any(|c| c.memory.profile_revision > c.memory.viewed_profile_revision),
        journal: save.companion.unread().next().is_some()
            && !matches!(*tab, SettingsTab::Today | SettingsTab::Journal),
    };
    if root
        .ctx()
        .input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::W))
    {
        outcome.close_notebook = true;
    }
    // A page can be turned from the keyboard, as well as by its tab.
    if let Some(page) = clubhouse::journal::page_shortcut(root.ctx(), *tab) {
        *tab = page;
        *error = None;
    }
    // Turning to Today or the Journal reads the journal. What was new stays marked for as long
    // as the reader stays on those two pages, and is let go of when they turn elsewhere.
    if matches!(*tab, SettingsTab::Today | SettingsTab::Journal) {
        if clubhouse.reading.is_none() {
            clubhouse.reading = Some(clubhouse::today::Reading {
                unread: save.companion.unread().cloned().collect(),
            });
        }
        if save.companion.unread().next().is_some()
            || save.companion.journal.iter().map(|e| e.at).max() > save.companion.journal_seen_until
        {
            outcome.mark_journal_read = true;
        }
    } else {
        clubhouse.reading = None;
    }
    let conditions = clubhouse::journal::conditions(save, monitors);
    let page_left = window.left() + spread.cover;
    let mut open_tab = egui::Rect::NOTHING;
    egui::Panel::left("journal-cover")
        .exact_size(spread.cover)
        .resizable(false)
        .frame(egui::Frame::NONE)
        .show(root, |ui| {
            let cover = ui.max_rect();
            let (rect, turned) = clubhouse::journal::cover_tabs(
                ui,
                cover,
                page_left,
                tab,
                marks,
                &conditions,
                text_scale,
            );
            open_tab = rect;
            if turned {
                *error = None;
            }
        });
    egui::Panel::right("journal-binding")
        .exact_size(spread.binding)
        .resizable(false)
        .frame(egui::Frame::NONE)
        .show(root, |ui| {
            clubhouse::journal::paint_binding(ui.painter(), ui.max_rect());
        });
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.outer_margin(egui::Margin {
            left: 0,
            right: 0,
            top: spread.top as i8,
            bottom: 0,
        }))
        .show(root, |ui| {
            let page = ui.max_rect();
            // Painted for the whole window rather than for this panel, since the page's edge and
            // its shadow fall just outside it, on the binding.
            let window_painter = ui.ctx().layer_painter(egui::LayerId::background());
            clubhouse::journal::paint_page(&window_painter, page, spread, page.bottom(), open_tab);
            let footer = egui::Panel::bottom("settings-footer")
                .frame(egui::Frame::NONE.fill(paper()).inner_margin(egui::Margin {
                    left: spread.left as i8,
                    right: spread.right as i8,
                    top: 14,
                    bottom: 12,
                }))
                .show(ui, |ui| {
            if let Some(message) = error.as_deref() {
                ui.colored_label(egui::Color32::from_rgb(145, 58, 44), message);
            }
            if let Some((message, started)) = &clubhouse.shell.feedback {
                if started.elapsed() < std::time::Duration::from_secs(4) {
                    ui.colored_label(forest(), message);
                    ui.ctx().request_repaint_after(
                        std::time::Duration::from_secs(4).saturating_sub(started.elapsed()),
                    );
                } else {
                    clubhouse.shell.feedback = None;
                }
            }
            // The last change to who lives here or how the village is laid out, while it can
            // still be taken back: on every page, since a change on one page shows on another.
            if let Some(change) = clubhouse.last_edit.clone()
                && ui
                    .button(format!("Undo {change}"))
                    .on_hover_text(undo_hint(clubhouse.earlier_edits))
                    .clicked()
            {
                outcome.undo_last_edit = true;
            }
            let adoption_footer =
                *tab == SettingsTab::Studio && clubhouse.studio.adoption_footer(ui, save, outcome);
            if !adoption_footer || settings != saved {
                let dirty = settings != saved;
                let pending = pending_changes(saved, settings);
                let status_text = if !dirty {
                    "Preferences are saved".to_owned()
                } else if pending.is_empty() || pending.len() > 3 {
                    "Unapplied preferences".to_owned()
                } else {
                    format!("Not applied yet: {}", pending.join(", "))
                };
                let status = status_text.as_str();
                // The status and its two buttons share a row where they fit; in a narrow window
                // the buttons go to a row of their own rather than over the status.
                let padding = 2.0 * ui.spacing().button_padding.x;
                let needed = clubhouse::text_width(ui, status)
                    + clubhouse::text_width(ui, "Apply changes")
                    + clubhouse::text_width(ui, "Revert")
                    + 2.0 * padding
                    + 3.0 * ui.spacing().item_spacing.x;
                let mut buttons = |ui: &mut egui::Ui| {
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
                };
                if ui.available_width() >= needed {
                    ui.horizontal(|ui| {
                        ui.label(status);
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            &mut buttons,
                        );
                    });
                } else {
                    ui.label(status);
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        &mut buttons,
                    );
                }
            }
                })
                .response
                .rect;
            // The margin line runs on down through the footer, under a dashed line across the
            // top of it, and the page's number sits just above it.
            {
                let painter = ui.painter();
                let mut fills = clubhouse::journal::Fills::default();
                fills.rect(
                    egui::Rect::from_min_max(
                        egui::pos2(page.left() + spread.margin, footer.top()),
                        egui::pos2(page.left() + spread.margin + 2.0, page.bottom()),
                    ),
                    clubhouse::margin_line(),
                );
                clubhouse::journal::dashes(
                    &mut fills,
                    egui::pos2(page.left() + 8.0, footer.top()),
                    egui::pos2(page.right() - 8.0, footer.top()),
                    6.0,
                    5.0,
                    2.0,
                    gold(),
                );
                fills.paint(painter);
                clubhouse::journal::paint_page_number(
                    painter,
                    page,
                    footer.top(),
                    clubhouse::journal::page_number(*tab),
                    text_scale,
                );
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                    left: spread.left as i8,
                    right: spread.right as i8,
                    top: 24,
                    bottom: 22,
                }))
                .show(ui, |ui| {
        // The tour, above the page rather than on it, so it stays in view while the page scrolls
        // to whatever it is pointing at.
        clubhouse.shell.tour_card(ui, save, tab, outcome);
        egui::ScrollArea::vertical().id_salt(format!("page-{tab:?}")).auto_shrink([false, false]).show(ui, |ui| {
            clubhouse.recovery.cards(ui, save_location, outcome);
            match tab {
                SettingsTab::Today => clubhouse.today.show(ui, &mut clubhouse.shell, save, monitors, tab, clubhouse.reading.as_ref()),
                SettingsTab::Colony => {
                    clubhouse::journal::page_heading(ui, SettingsTab::Colony, "Your colony", &clubhouse::journal::colony_observation(creatures));
                    colony_tab(ui, ColonyView { creatures, relationships, save }, creature_names, selected_creature, monitors, error, remove_confirmation, bulk_confirmation, &mut clubhouse.colony, &mut clubhouse.shell, outcome);
                }
                SettingsTab::Studio => clubhouse.studio.show(ui, &mut clubhouse.shell, save, selected_creature, outcome),
                SettingsTab::Home => clubhouse.home.show(
                    ui,
                    &mut clubhouse.shell,
                    save,
                    monitors,
                    clubhouse.formiga_home,
                    outcome,
                ),
                SettingsTab::Journal => clubhouse.journal.show(ui, &mut clubhouse.shell, save, clubhouse.reading.as_ref(), outcome),
                SettingsTab::General => general_tab(ui, settings, save, &mut clubhouse.shell, outcome),
                SettingsTab::Habitat => {
                    clubhouse::journal::page_heading(ui, SettingsTab::Habitat, "Habitat", "Where the colony may wander. Mint is welcome; clay is not.");
                    clubhouse::habitat_map(ui, &settings.habitat, monitors);
                    habitat_tab(ui, settings, monitors, editor_active, outcome);
                }
                SettingsTab::Applications => {
                    clubhouse::journal::page_heading(ui, SettingsTab::Applications, "Applications", "Windows allowed to stand in front of the colony. They don't mind.");
                    applications_tab(ui, settings, windows, outcome);
                }
                SettingsTab::About => {
                    clubhouse::journal::page_heading(ui, SettingsTab::About, "About & backups", "Who keeps this notebook, and where to find a spare copy.");
                    clubhouse.recovery.backups(ui, outcome);
                    ui.add_space(18.0);
                    about_tab(ui, outcome, save_location, update_status, automatic_update_checks);
                }
            }
        });
                });
            clubhouse::journal::paint_open_tab(
                &ui.ctx().layer_painter(egui::LayerId::background()),
                open_tab,
                *tab,
                marks,
                text_scale,
            );
            clubhouse::journal::turn_page(
                ui,
                &mut clubhouse.page_turn,
                &mut clubhouse.shown_page,
                *tab,
                page,
                save.settings.reduce_motion,
            );
        });
}

/// What the undo button says about the changes behind the one it takes back.
pub(crate) fn undo_hint(earlier: usize) -> String {
    match earlier {
        0 => "The only change kept. Changes are kept until Formiga quits.".to_owned(),
        1 => "One earlier change can be undone after this one.".to_owned(),
        n => format!("{n} earlier changes can be undone after this one, newest first."),
    }
}

/// The preferences changed but not yet applied, by the names their controls go by.
pub(crate) fn pending_changes(saved: &Settings, draft: &Settings) -> Vec<&'static str> {
    let mut pending = Vec::new();
    for (changed, name) in [
        (saved.visible != draft.visible, "show colony"),
        (saved.paused != draft.paused, "pause"),
        (
            saved.direct_manipulation != draft.direct_manipulation,
            "petting",
        ),
        (saved.cursor_reactions != draft.cursor_reactions, "cursor"),
        (saved.window_ledges != draft.window_ledges, "window ledges"),
        (saved.reduce_motion != draft.reduce_motion, "reduce motion"),
        (
            saved.launch_at_login != draft.launch_at_login,
            "launch at login",
        ),
        (saved.display_scale != draft.display_scale, "size"),
        (saved.habitat != draft.habitat, "habitat"),
        (
            saved.application_occlusion_rules != draft.application_occlusion_rules
                || saved.fullscreen_app_occlusion != draft.fullscreen_app_occlusion,
            "applications",
        ),
    ] {
        if changed {
            pending.push(name);
        }
    }
    pending
}

fn general_tab(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    save: &formiga_core::SaveFile,
    shell: &mut clubhouse::shell::Shell,
    outcome: &mut SettingsOutcome,
) {
    clubhouse::journal::page_heading(
        ui,
        SettingsTab::General,
        "Preferences",
        "Conditions of study. Adjust gently; the specimens notice.",
    );
    let quiet = ui
        .scope(|ui| clubhouse::quiet_controls(ui, save, outcome))
        .response
        .rect;
    shell.tour_mark(ui, TourMark::Quiet, quiet);
    ui.add_space(16.0);
    clubhouse::card(ui, |ui| {
        ui.strong("Saved routines").on_hover_text(
            "A routine is a snapshot of where the colony may go, whether it climbs windows, \
             whether it reacts to the cursor, and whether motion is reduced.",
        );
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
                    shell.notify(format!("{name} routine loaded · Apply to use it"));
                }
            });
        }
    });
    ui.add_space(16.0);
    clubhouse::schedule_controls(ui, save, outcome);
    ui.add_space(16.0);
    clubhouse::appearance_controls(ui, save, outcome);
    ui.add_space(18.0);
    clubhouse::journal::kicker(ui, "On your desktop");
    ui.small("Changes here take effect when you apply them, and the footer says what changed.");
    ui.checkbox(&mut settings.visible, "Show colony")
        .on_hover_text(
            "Hidden, the colony carries on out of sight: it keeps its days, and the journal keeps \
         writing.",
        );
    ui.checkbox(&mut settings.paused, "Pause ambient behavior")
        .on_hover_text(
            "Everyone holds still where they are. Nothing new happens, and nothing is replayed \
         when you resume.",
        );
    ui.checkbox(
        &mut settings.direct_manipulation,
        "Allow petting and dragging",
    )
    .on_hover_text("Off, clicks pass straight through companions to whatever is underneath.");
    ui.checkbox(&mut settings.cursor_reactions, "React to cursor movement")
        .on_hover_text(
            "Companions notice the pointer: a curious one comes to look, a wary one steps aside. \
             Only where the pointer is — never what it is pointing at.",
        );
    ui.checkbox(
        &mut settings.window_ledges,
        "Explore application-window ledges",
    )
    .on_hover_text(
        "Companions may climb onto the tops of your windows and ride along when one moves. \
         Formiga sees only where windows are, never what is in them.",
    );
    ui.checkbox(&mut settings.reduce_motion, "Reduce motion")
        .on_hover_text(
            "Calmer movement on the desktop — no bounces, tosses or wiggles — and no page turns in \
         this notebook.",
        );
    ui.checkbox(&mut settings.launch_at_login, "Launch at login")
        .on_hover_text("Start Formiga when you sign in, so the colony is there when you are.");
    ui.add_space(10.0);
    clubhouse::journal::kicker(ui, "Creature size")
        .on_hover_text("How large companions are drawn. It changes nothing about what they do.");
    ui.horizontal(|ui| {
        for (scale, name) in [(2, "Small"), (3, "Medium"), (4, "Large")] {
            ui.selectable_value(&mut settings.display_scale, scale, name);
        }
    });
    ui.add_space(18.0);
    if ui.button("Take the tour").clicked() {
        shell.take_the_tour();
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
    page: &mut clubhouse::collection::ColonyState,
    shell: &mut clubhouse::shell::Shell,
    outcome: &mut SettingsOutcome,
) {
    let creatures = colony.creatures;
    let relationships = colony.relationships;
    // Every specimen by its number in the register and its name.
    ui.horizontal_wrapped(|ui| {
        let scale = clubhouse::journal::text_scale(ui);
        for (index, creature) in creatures.iter().enumerate() {
            let mut entry = egui::text::LayoutJob::default();
            entry.append(
                &format!("{:02} ", index + 1),
                0.0,
                egui::TextFormat {
                    font_id: egui::FontId::monospace(clubhouse::journal::label_size(scale)),
                    color: clubhouse::muted(),
                    valign: egui::Align::Center,
                    ..Default::default()
                },
            );
            entry.append(
                &creature.name,
                0.0,
                egui::TextFormat {
                    font_id: egui::FontId::proportional(14.0 * scale),
                    color: ink(),
                    valign: egui::Align::Center,
                    ..Default::default()
                },
            );
            ui.selectable_value(selected_creature, Some(creature.id), entry);
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
    let number = creatures
        .iter()
        .position(|c| c.id == creature.id)
        .map_or(1, |index| index + 1);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            shell.portrait(ui, creature, 144.0);
            ui.label(clubhouse::journal::label_job(
                &format!("Fig. Nº {number:02}"),
                clubhouse::journal::label_size(clubhouse::journal::text_scale(ui)),
                clubhouse::muted(),
            ));
        });
        ui.vertical(|ui| {
            clubhouse::journal::kicker(ui, &format!("Specimen Nº {number:02}"));
            ui.heading(&creature.name);
            // Who it is, then its three traits, then what it has come to lean toward more than
            // anyone else here.
            ui.label(
                egui::RichText::new(creature.temperament_phrase())
                    .strong()
                    .color(forest()),
            );
            ui.horizontal_wrapped(|ui| {
                for t in creature.traits() {
                    clubhouse::make_room(ui, clubhouse::text_width(ui, t.label()) + 10.0);
                    egui::Frame::new()
                        .fill(mint())
                        .inner_margin(5)
                        .show(ui, |ui| {
                            ui.label(t.label());
                        });
                }
            });
            let learned = profile_descriptors(creature, creatures);
            if !learned.is_empty() {
                ui.small(format!(
                    "Lately: {}",
                    learned
                        .iter()
                        .map(|descriptor| descriptor.label())
                        .collect::<Vec<_>>()
                        .join(" · ")
                ));
            }
            ui.small(format!(
                "Body: {} · observed {}",
                creature.appearance.design.map_or_else(
                    || clubhouse::words(&format!("{:?}", creature.appearance.family)),
                    |d| d.body.label().to_owned(),
                ),
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
            // Its own little ways: how it celebrates, and the habits it has picked up here.
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(
                    egui::RichText::new("Habits:")
                        .small()
                        .color(clubhouse::muted()),
                );
                ui.small(little_ways(creature));
            });
        });
    });
    ui.add_space(18.0);
    clubhouse::journal::kicker(ui, "Tally · Life here");
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
            // The number over its label, a tally box no narrower than 96.
            let width = clubhouse::text_width(ui, egui::RichText::new(&number).size(24.0))
                .max(clubhouse::text_width(
                    ui,
                    egui::RichText::new(label).small(),
                ))
                .max(96.0);
            clubhouse::make_room(ui, width + clubhouse::CARD_EDGES);
            clubhouse::card(ui, |ui| {
                ui.vertical(|ui| {
                    ui.set_min_width(96.0);
                    ui.label(egui::RichText::new(number).size(24.0).color(ink()));
                    ui.small(label);
                });
            });
        }
    });
    ui.add_space(8.0);
    clubhouse::journal::kicker(ui, "Observations");
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
                together_label(creature, friend, relationship)
            ));
        }
    } else {
        ui.label("Closest friend: still getting acquainted");
    }
    // What the notebook can say about this companion with evidence, and the moments it shared
    // most recently, read from what was recorded and from nothing else.
    let now = time::OffsetDateTime::now_utc();
    let offset = clubhouse::local_offset();
    if let Some(friend) = closest_friend {
        let (low, high) = (creature.id.min(friend.id), creature.id.max(friend.id));
        if let Some(grew_close) = colony.save.companion.journal.iter().find(|e| {
            e.creature == Some(low) && e.moment == formiga_core::JournalMoment::Friendship(high)
        }) {
            ui.small(format!(
                "Grew close to {} on {}, as the journal has it.",
                friend.name,
                grew_close.at.to_offset(offset).date()
            ));
        }
        if let Some(memory) =
            formiga_core::tally_between(&colony.save.tallies, creature.id, friend.id)
                .and_then(|pair| pair.tally.memory)
        {
            ui.small(format!(
                "Last time together: {} {}.",
                clubhouse::today::shared_text(memory.kind),
                clubhouse::today::ago_text(memory.at, now, offset)
            ));
        }
    }
    let noticed = formiga_core::observations_of(colony.save, creature.id);
    if !noticed.is_empty() {
        ui.add_space(6.0);
        for observation in &noticed {
            let (said, evidence) =
                clubhouse::today::observation_text(colony.save, monitors, observation);
            ui.label(said);
            ui.small(egui::RichText::new(evidence).color(clubhouse::muted()));
        }
    }
    ui.add_space(18.0);
    let name = ui.group(|ui| {
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
        // Where they like to be: a leaning, not a rule. The habitat, pausing, and hiding still
        // decide where they can go.
        ui.horizontal_wrapped(|ui| {
            ui.label("Likes to be");
            egui::ComboBox::from_id_salt(("roaming-leaning", creature.id))
                .selected_text(creature.leaning.label())
                .show_ui(ui, |ui| {
                    for leaning in formiga_core::RoamingLeaning::ALL {
                        let chosen = creature.leaning == leaning;
                        if ui
                            .selectable_label(chosen, leaning.label())
                            .on_hover_text(leaning.description())
                            .clicked()
                            && !chosen
                        {
                            outcome.set_roaming_leaning = Some((creature.id, leaning));
                        }
                    }
                });
            ui.small(creature.leaning.description());
        });
    });
    shell.tour_mark(ui, TourMark::Name, name.response.rect);

    ui.add_space(8.0);
    page.wardrobe(ui, shell, colony.save, creature, outcome);
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
    let share = ui.group(|ui| {
        ui.strong("Share this creature");
        ui.label("The code recreates innate appearance and personality, not its name or history.");
        let code = encode_creature_seed(creature.origin);
        ui.horizontal(|ui| {
            ui.monospace(format!("{}…{}", &code[..15], &code[code.len() - 8..]));
            if ui.button("Copy seed").clicked() {
                ui.ctx().copy_text(code);
                shell.notify("Creature code copied");
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
                .selected_text(page.sticker_clip.label())
                .show_ui(ui, |ui| {
                    for clip in formiga_art::StickerClip::ALL {
                        ui.selectable_value(&mut page.sticker_clip, clip, clip.label());
                    }
                });
            ui.selectable_value(&mut page.small_sticker, false, "8×");
            ui.selectable_value(&mut page.small_sticker, true, "4×");
            if ui.button("Export sticker…").clicked() {
                outcome.export_creature_sticker =
                    Some((creature.id, page.sticker_clip, page.sticker_scale()));
            }
        });
    });
    shell.tour_mark(ui, TourMark::Share, share.response.rect);

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
    // The whole colony after the one companion: everyone's friendships, playmates and tensions.
    ui.add_space(18.0);
    colony_standings_card(ui, creatures, relationships, selected_creature);
    // Everything the colony can find, and which of it hangs in the trees.
    ui.add_space(18.0);
    page.collection(ui, shell, colony.save, outcome);
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs(1));
}

/// How a companion celebrates and the habits it has picked up, in the order it picked them up.
fn little_ways(creature: &Creature) -> String {
    let mut ways = vec![formiga_core::Celebration::for_creature(creature).label()];
    ways.extend(creature.memory.habits.iter().map(|habit| habit.label()));
    ways.join(" · ")
}

pub(crate) fn activity_label(action: ActionKind) -> String {
    match action {
        ActionKind::Idle => "taking a quiet moment".into(),
        ActionKind::Traverse => "exploring".into(),
        ActionKind::Homebound => "settling at home".into(),
        ActionKind::PetReaction => "enjoying your company".into(),
        _ => clubhouse::words(&format!("{action:?}")).to_lowercase(),
    }
}

/// How two companions get along, read from nothing but their shared record, in the order the
/// colony page lists them: a pair keeping its distance first, since that is the thing worth
/// noticing, then close friends, then playmates, then everyone still getting acquainted. The
/// thresholds are the ones `bond_label` and `play_label` already describe a bond with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Standing {
    Distant,
    Close,
    Playmates,
    Acquainting,
}

impl Standing {
    fn of(relationship: &formiga_core::CreatureRelationship) -> Self {
        if relationship.avoidance >= 160 {
            Self::Distant
        } else if relationship.affinity >= 112 {
            Self::Close
        } else if relationship.playfulness >= 72 {
            Self::Playmates
        } else {
            Self::Acquainting
        }
    }

    fn heading(self) -> &'static str {
        match self {
            Self::Distant => "Keeping their distance",
            Self::Close => "Close friends",
            Self::Playmates => "Playmates",
            Self::Acquainting => "Still getting to know each other",
        }
    }
}

/// Two companions, named in colony order, and how they get along.
#[derive(Clone, Copy, Debug)]
struct PairStanding {
    standing: Standing,
    a: CreatureId,
    b: CreatureId,
    relationship: formiga_core::CreatureRelationship,
}

/// Every pair in the colony once, grouped by how they get along and closest first within each
/// group. A pair with no shared record yet has simply not spent time together, and reads as
/// getting acquainted; nothing is inferred beyond the four scores the colony keeps.
fn colony_standings(
    creatures: &[Creature],
    relationships: &[formiga_core::CreatureRelationship],
) -> Vec<PairStanding> {
    let mut pairs = Vec::new();
    for (index, a) in creatures.iter().enumerate() {
        for b in &creatures[index + 1..] {
            let relationship = relationship_between(relationships, a.id, b.id)
                .copied()
                .unwrap_or_default();
            pairs.push(PairStanding {
                standing: Standing::of(&relationship),
                a: a.id,
                b: b.id,
                relationship,
            });
        }
    }
    pairs.sort_by_key(|pair| {
        (
            pair.standing,
            std::cmp::Reverse(if pair.standing == Standing::Distant {
                i16::from(pair.relationship.avoidance)
            } else {
                pair.relationship.closeness()
            }),
        )
    });
    pairs
}

/// The whole colony's friendships, playmates, and tensions on one card, so the social life that
/// plays out on the desktop is easy to notice here too. A name opens that companion's profile.
fn colony_standings_card(
    ui: &mut egui::Ui,
    creatures: &[Creature],
    relationships: &[formiga_core::CreatureRelationship],
    selected_creature: &mut Option<CreatureId>,
) {
    if creatures.len() < 2 {
        return;
    }
    let name = |id: CreatureId| {
        creatures
            .iter()
            .find(|creature| creature.id == id)
            .map_or("A companion", |creature| creature.name.as_str())
    };
    clubhouse::card(ui, |ui| {
        ui.strong("How everyone gets along");
        let mut heading = None;
        for PairStanding {
            standing,
            a,
            b,
            relationship,
        } in colony_standings(creatures, relationships)
        {
            if heading != Some(standing) {
                heading = Some(standing);
                ui.add_space(6.0);
                ui.small(egui::RichText::new(standing.heading()).color(forest()));
            }
            let involves_selected = *selected_creature == Some(a) || *selected_creature == Some(b);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (index, id) in [a, b].into_iter().enumerate() {
                    if index == 1 {
                        ui.label("&");
                    }
                    let text = egui::RichText::new(name(id));
                    let text = if involves_selected {
                        text.strong()
                    } else {
                        text
                    };
                    if ui.link(text).clicked() {
                        *selected_creature = Some(id);
                    }
                }
                let together = creatures
                    .iter()
                    .find(|creature| creature.id == a)
                    .zip(creatures.iter().find(|creature| creature.id == b))
                    .map_or_else(
                        || play_label(relationship.playfulness),
                        |(a, b)| together_label(a, b, &relationship),
                    );
                ui.label(format!(
                    "· {} · {}",
                    bond_label(relationship.affinity, relationship.avoidance),
                    together
                ));
            });
        }
    });
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

/// How two companions are together, beyond how close they are: how they play, or, while they do
/// not play much, what their temperaments make of each other.
fn together_label(
    a: &Creature,
    b: &Creature,
    relationship: &formiga_core::CreatureRelationship,
) -> &'static str {
    use formiga_core::TemperamentKind as Kind;
    if relationship.playfulness >= 72 {
        return play_label(relationship.playfulness);
    }
    let (a, b) = (a.temperament(), b.temperament());
    let feisty = |t: &formiga_core::Temperament| t.axes.feistiness >= 0.65;
    let both = |test: &dyn Fn(&formiga_core::Temperament) -> bool| test(&a) && test(&b);
    if both(&feisty) {
        "squabbling"
    } else if both(&|t| t.kind == Kind::Troublemaker) {
        "partners in mischief"
    } else if (a.kind == Kind::Grump || b.kind == Kind::Grump) && feisty(&a) != feisty(&b) {
        "an odd couple"
    } else if both(&|t| t.axes.social <= 0.3) {
        "quiet company"
    } else if both(&|t| t.axes.energy <= 0.35) {
        "easy company"
    } else if relationship.affinity >= 112 {
        "cozy"
    } else {
        "polite"
    }
}

/// What each habitat preset leaves companions, said on hover.
fn habitat_preset_help(preset: HabitatPreset) -> &'static str {
    match preset {
        HabitatPreset::EntireDesktop => "Every display, edge to edge.",
        HabitatPreset::PrimaryDisplay => "Only the main display; the others stay clear.",
        HabitatPreset::BottomEdge => {
            "A band along the bottom quarter of each display, just above the Dock or taskbar."
        }
        HabitatPreset::BottomCorners => "The two bottom corners of each display.",
        HabitatPreset::LowerHalf => "The lower half of each display.",
        HabitatPreset::Custom => {
            "Regions you draw yourself, with Edit on desktop or the list below."
        }
    }
}

pub(crate) fn display_label(display: formiga_core::DisplayKey, monitors: &[MonitorInfo]) -> String {
    monitors
        .iter()
        .position(|monitor| monitor.display_key == display)
        .map_or_else(
            || "a previous display".to_owned(),
            |index| format!("Display {}", index + 1),
        )
}

pub(crate) fn region_label(cell: u8) -> &'static str {
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
                )
                .on_hover_text(habitat_preset_help(preset));
            }
        });
    if settings.habitat.preset != previous_preset
        && settings.habitat.preset != HabitatPreset::Custom
    {
        settings.habitat.zones.clear();
    }
    ui.horizontal(|ui| {
        if !editor_active
            && ui
                .button("Edit on desktop")
                .on_hover_text(
                    "Draw the habitat right on your screen: drag with the left button to welcome an \
                     area, with the right to keep companions out of one. Enter applies, Escape \
                     cancels.",
                )
                .clicked()
        {
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
        if ui
            .button("Gather creatures here")
            .on_hover_text("Brings everyone back inside the habitat, wherever they have wandered.")
            .clicked()
        {
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
    ui.add_space(8.0);
    clubhouse::journal::kicker(ui, "May cover companions");
    ui.small("Selected application windows visually cover creatures inside their visible area.");
    let mut remove = None;
    for (index, rule) in settings.application_occlusion_rules.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            // The application's own name is the checkbox's label, so a screen reader says which
            // application it is about.
            ui.checkbox(&mut rule.enabled, rule.display_name.as_str());
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
    clubhouse::wide_card(ui, |ui| {
        clubhouse::journal::kicker(ui, "Colophon");
        ui.heading(format!("Formiga {APP_VERSION}"));
        ui.label("Procedural desktop fauna, generated and simulated entirely on your computer.");
        ui.small(format!(
            "A {} build for {}. MIT licensed; the versions of everything it is built from are \
             recorded in Cargo.lock.",
            if cfg!(debug_assertions) {
                "development"
            } else {
                "release"
            },
            if cfg!(target_os = "macos") {
                "macOS"
            } else {
                "Windows"
            }
        ));
        ui.hyperlink_to(
            "Project repository",
            "https://github.com/Von-Van/Formiga-Desktop",
        );
    });
    ui.add_space(12.0);
    clubhouse::wide_card(ui, |ui| {
        clubhouse::journal::kicker(ui, "Privacy");
        ui.label(
            "No accounts, screenshots, window titles, keystrokes, behavioral uploads, or \
             telemetry. Formiga sees where windows are, never what is in them.",
        );
        ui.small("Optional update checks contact only the public Formiga repository on GitHub.");
    });
    ui.add_space(12.0);
    clubhouse::wide_card(ui, |ui| {
        clubhouse::journal::kicker(ui, "Updates");
        let mut automatic = automatic_update_checks;
        if ui
            .checkbox(&mut automatic, "Check GitHub for updates automatically")
            .on_hover_text(
                "At most once a day, when Formiga starts. Nothing is downloaded without asking.",
            )
            .changed()
        {
            outcome.automatic_update_checks = Some(automatic);
        }
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
            UpdateStatus::UpToDate { checked_at_unix } => {
                let checked = time::OffsetDateTime::from_unix_timestamp(*checked_at_unix)
                    .map(|at| at.to_offset(clubhouse::local_offset()));
                ui.label(match checked {
                    Ok(at) => format!(
                        "Formiga is up to date, as of {:02}:{:02}.",
                        at.hour(),
                        at.minute()
                    ),
                    Err(_) => "Formiga is up to date.".to_owned(),
                });
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
                if ui
                    .button("Download Verified Update")
                    .on_hover_text(
                        "Downloaded in the background and checked against the release's SHA-256 \
                         before it can be opened. Your colony carries on meanwhile.",
                    )
                    .clicked()
                {
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
                ui.small(
                    "Your colony is saved before the installer opens, and carries on afterwards.",
                );
                #[cfg(target_os = "windows")]
                let install_label = "Run Update Installer and Quit Formiga";
                #[cfg(target_os = "macos")]
                let install_label = "Open Update Disk Image";
                if ui.button(install_label).clicked() {
                    outcome.install_update = true;
                }
            }
            UpdateStatus::Failed(message) => {
                let (headline, advice) = crate::explain::update(message);
                ui.colored_label(egui::Color32::from_rgb(196, 88, 64), headline);
                ui.label(advice);
                if ui.button("Try Again").clicked() {
                    outcome.check_updates = true;
                }
            }
        }
    });
    ui.add_space(12.0);
    clubhouse::wide_card(ui, |ui| {
        clubhouse::journal::kicker(ui, "Where things are kept");
        ui.label(format!("Colony: {save_location}"));
        ui.small(
            "A backup copy sits beside it, and the notebook remembers its own window position in \
             a small file next to them. The logs say what Formiga did, never what was on your \
             screen.",
        );
        if ui.button("Open diagnostic logs").clicked() {
            outcome.open_logs = true;
        }
    });
}

#[cfg(test)]
#[path = "settings_review.rs"]
mod review;
