use crate::creature_menu::{LocalRect, MenuAnchor, MenuPlacement};
use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use formiga_art::{
    AnimationSpec, BUBBLE_ANCHOR, BUBBLE_CELL, BodyClip, COLONY_OBJECT_ATLAS_HEIGHT,
    COLONY_OBJECT_ATLAS_WIDTH, COLONY_OBJECT_SIZE, ColonyObjectRenderer, CreatureRenderer,
    FACE_FRAME_SIZE, FRAME_SIZE, FaceRenderState, FramePlacement, MenuIcon, MenuLayout,
    MilestoneBubbleRenderer, MotionSignature, PixelPoint, PropAnchor, Rgba, SHELTER_SIZE,
    ShelterRenderer, SpriteRect, TRINKET_ATLAS_HEIGHT, TRINKET_ATLAS_WIDTH, TRINKET_CELL,
    TRINKET_FRAME_GLINT, TRINKET_FRAME_REST, TrinketAtlasRenderer, UI_ATLAS_HEIGHT, UI_ATLAS_WIDTH,
    UiAtlasRenderer, VILLAGE_ATLAS_SIZE,
};
use formiga_core::{
    ActionKind, ApplicationOcclusionRule, ColonyObject, Creature, CreatureId, CursorSnapshot,
    DesktopRect, DesktopWindow, HabitatPolicy, HabitatZoneKind, MonitorInfo, Point, SaveFile,
    ShelterDecorationKind, ShelterGenome, ThoughtBubble, accessible_regions, resolved_home_anchor,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    occlusion_enabled: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ZoneVertex {
    position: [f32; 2],
    color: [f32; 4],
}

const MAX_OCCLUSION_RECTS: usize = 64;
// Four creatures, a full village of four dwellings, one bubble, and eight objects.
const INITIAL_VERTEX_CAPACITY: usize = 150;
const SURFACE_RECOVERY_STALLS: u8 = 3;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Pod, Zeroable)]
struct OcclusionUniform {
    rects: [[f32; 4]; MAX_OCCLUSION_RECTS],
    metadata: [u32; 4],
}

struct SpriteGpu {
    _body_texture: wgpu::Texture,
    body_bind_group: wgpu::BindGroup,
    _face_texture: wgpu::Texture,
    face_bind_group: wgpu::BindGroup,
    reduce_motion: bool,
    outline: bool,
    body_atlas_width: u32,
    body_atlas_height: u32,
    face_atlas_width: u32,
    face_atlas_height: u32,
    face_anchors: Vec<PixelPoint>,
    /// The topmost and just-past-the-lowest drawn rows of each baked body frame, in art pixels
    /// from the frame's top edge. A bubble hangs off the crown rather than off the frame, so a
    /// mini keeps its bubble as close to its head as an adult does.
    silhouette: Vec<(u8, u8)>,
    resting_baseline: u32,
}

struct ShelterGpu {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    genome: ShelterGenome,
    decorations: Vec<ShelterDecorationKind>,
}

struct BubbleGpu {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    creature_id: CreatureId,
    width: u32,
    height: u32,
}

/// The one small texture every icon bubble and every menu part is sampled from. It is built the
/// first time anything asks for it and dropped again once nothing has, so a colony that is simply
/// being watched carries none of it.
struct UiAtlasGpu {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

/// Renders without a bubble or a menu before the UI atlas is released again. At the overlay's
/// redraw cadence this is comfortably more than ten seconds of quiet.
const UI_ATLAS_IDLE_FRAMES: u32 = 240;

/// What the overlay should draw on top of the colony this frame. Everything here is runtime-only:
/// none of it is in the save, and none of it is remembered between frames.
#[derive(Clone, Copy, Default)]
pub struct OverlayUi<'a> {
    /// The simulation's answers, one per creature at most.
    pub bubbles: &'a [ThoughtBubble],
    pub reduce_motion: bool,
    /// The open right-click menu, if it belongs to a creature on this monitor.
    pub menu: Option<MenuView<'a>>,
}

/// One open creature menu, already placed by `creature_menu`.
#[derive(Clone, Copy)]
pub struct MenuView<'a> {
    pub creature_id: CreatureId,
    pub items: &'a [MenuIcon; 4],
    pub layout: &'a MenuLayout,
    pub placement: MenuPlacement,
    pub hovered: Option<usize>,
}

struct ColonyObjectsGpu {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    colony_seed: [u8; 32],
}

/// The colony's own sheet of found things. One texture for the whole colony rather than eight
/// slots inside every creature's face texture, so a keepsake costs the same whether one companion
/// is holding it or four are, and so the scrapbook and the desktop can never disagree.
struct TrinketAtlasGpu {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    key: TrinketAtlasKey,
}

/// What the sheet depends on: the colony seed it is derived from, and the colours of everyone it
/// has to stay distinct from.
#[derive(Clone, Debug, PartialEq)]
struct TrinketAtlasKey {
    colony_seed: [u8; 32],
    members: Vec<Rgba>,
}

impl TrinketAtlasKey {
    fn of(save: &SaveFile) -> Self {
        Self {
            colony_seed: save.colony_seed,
            members: save
                .creatures
                .iter()
                .flat_map(|creature| {
                    let palette = formiga_art::palette_for(&creature.appearance);
                    [
                        palette.coat,
                        palette.accent,
                        palette.highlight,
                        palette.shadow,
                    ]
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ObjectVertexCacheKey {
    objects: Vec<ColonyObject>,
    cottages: Vec<formiga_core::DwellingKind>,
    home: formiga_core::ColonyHome,
    habitat: HabitatPolicy,
    monitor_bounds: DesktopRect,
    monitor_usable_bounds: DesktopRect,
    monitor_scale_factor: f32,
    display_scale: u8,
}

const ATLAS_COLUMNS: u32 = 10;
// There are exactly 27 eyelid/gaze combinations per expression. Keeping one expression per row
// avoids padding slots and leaves enough texture budget for additional pre-baked body actions.
const FACE_ATLAS_COLUMNS: u32 = 27;

pub struct OverlayRenderer {
    pub window: Arc<Window>,
    pub monitor: MonitorInfo,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// The window's own size in physical pixels. Every position is laid out against this, so the
    /// picture lands in the same place whatever size the drawable underneath it is.
    layout: PhysicalSize<u32>,
    /// 1 draws at full backing resolution; 2 draws at half and lets Core Animation magnify it.
    render_divisor: u32,
    pipeline: wgpu::RenderPipeline,
    zone_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    occlusion_buffer: wgpu::Buffer,
    occlusion_bind_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: usize,
    zone_vertex_buffer: wgpu::Buffer,
    sprites: BTreeMap<CreatureId, SpriteGpu>,
    shelter: Option<ShelterGpu>,
    bubble: Option<BubbleGpu>,
    ui_atlas: Option<UiAtlasGpu>,
    ui_atlas_idle: u32,
    colony_objects: Option<ColonyObjectsGpu>,
    trinkets: Option<TrinketAtlasGpu>,
    object_vertex_cache_key: Option<ObjectVertexCacheKey>,
    object_vertices: Vec<Vertex>,
    last_occlusion: Option<OcclusionUniform>,
    occlusion_windows: Vec<DesktopWindow>,
    occlusion_rules: Vec<ApplicationOcclusionRule>,
    occlusion_fullscreen_enabled: bool,
    occlusion_monitor_bounds: DesktopRect,
    occlusion_rects: Vec<DesktopRect>,
    occlusion_initialized: bool,
    has_visual_content: bool,
    visible: bool,
    hittest_enabled: bool,
    consecutive_surface_stalls: u8,
}

impl OverlayRenderer {
    pub async fn new(window: Arc<Window>, monitor: MonitorInfo) -> Result<Self> {
        let instance = overlay_gpu_instance();
        let surface = instance
            .create_surface(window.clone())
            .context("create transparent surface")?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                apply_limit_buckets: false,
            })
            .await
            .context("find GPU adapter")?;
        let required_limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Formiga GPU"),
                required_features: wgpu::Features::empty(),
                // Full-monitor overlays can exceed the downlevel 2048px texture cap on Retina
                // and 4K displays. Keep conservative feature limits but use the adapter's real
                // resolution limits for the swapchain.
                required_limits,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            })
            .await
            .context("create GPU device")?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(caps.formats[0]);
        let alpha_mode = overlay_alpha_preference()
            .iter()
            .copied()
            .find(|mode| caps.alpha_modes.contains(mode))
            .unwrap_or(caps.alpha_modes[0]);
        anyhow::ensure!(
            alpha_mode != wgpu::CompositeAlphaMode::Opaque,
            "GPU surface exposes no transparent alpha mode"
        );
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            // Every drawable in the pool is a full-screen image. The overlay presents at twenty
            // frames a second at most, far below any display's refresh, so a second queued frame
            // never helps — it only holds another screen's worth of memory.
            desired_maximum_frame_latency: 1,
            alpha_mode,
            view_formats: Vec::new(),
        };
        surface.configure(&device, &config);
        crate::platform::use_nearest_overlay_filter(&window);
        let layout = size;
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("creature texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let occlusion_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("application occlusion layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let occlusion_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("application occlusion rectangles"),
            contents: bytemuck::bytes_of(&OcclusionUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let occlusion_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("application occlusion bindings"),
            layout: &occlusion_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: occlusion_buffer.as_entire_binding(),
            }],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nearest pixel sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Formiga sprite shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Formiga pipeline layout"),
            bind_group_layouts: &[Some(&occlusion_layout), Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 16,
                    shader_location: 2,
                },
            ],
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Formiga sprite pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(vertex_layout)],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let zone_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Formiga habitat editor layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let zone_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Formiga habitat editor pipeline"),
            layout: Some(&zone_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_zone"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<ZoneVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 8,
                            shader_location: 1,
                        },
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_zone"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("creature vertices"),
            contents: bytemuck::cast_slice(&[Vertex::zeroed(); INITIAL_VERTEX_CAPACITY]),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let zone_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("habitat editor vertices"),
            contents: bytemuck::cast_slice(&[ZoneVertex::zeroed(); 768]),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        Ok(Self {
            window,
            monitor,
            surface,
            device,
            queue,
            config,
            layout,
            render_divisor: 1,
            pipeline,
            zone_pipeline,
            bind_group_layout,
            occlusion_buffer,
            occlusion_bind_group,
            sampler,
            vertex_buffer,
            vertex_capacity: INITIAL_VERTEX_CAPACITY,
            zone_vertex_buffer,
            sprites: BTreeMap::new(),
            shelter: None,
            bubble: None,
            ui_atlas: None,
            ui_atlas_idle: 0,
            colony_objects: None,
            trinkets: None,
            object_vertex_cache_key: None,
            object_vertices: Vec::new(),
            last_occlusion: None,
            occlusion_windows: Vec::new(),
            occlusion_rules: Vec::new(),
            occlusion_fullscreen_enabled: false,
            occlusion_monitor_bounds: DesktopRect::default(),
            occlusion_rects: Vec::new(),
            occlusion_initialized: false,
            has_visual_content: false,
            visible: false,
            hittest_enabled: false,
            consecutive_surface_stalls: 0,
        })
    }

    pub fn set_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        self.window.set_visible(visible);
        self.visible = visible;
        if !visible {
            // A hidden overlay shows no bubbles and no menu, and cannot be asked to until it comes
            // back, so the UI atlas goes now rather than waiting out its idle count.
            self.ui_atlas = None;
            self.ui_atlas_idle = 0;
        }
        if visible {
            // Showing a window rebuilds native state on both platforms, so re-apply the whole
            // overlay configuration — input mode, transparency, and Spaces membership — after
            // every hide/show cycle rather than only at creation.
            crate::platform::configure_native_overlay(&self.window, self.hittest_enabled);
            crate::platform::use_nearest_overlay_filter(&self.window);
            self.window.request_redraw();
        }
    }

    pub fn set_hittest_enabled(&mut self, enabled: bool) {
        if self.hittest_enabled == enabled {
            return;
        }
        self.hittest_enabled = enabled;
        if self.visible {
            crate::platform::set_overlay_hittest(&self.window, enabled);
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.layout = size;
        (self.config.width, self.config.height) = drawable_size(size, self.render_divisor);
        self.surface.configure(&self.device, &self.config);
    }

    /// Draw at half the backing resolution whenever that loses nothing. A creature at an even
    /// size on a Retina display has art pixels that are a whole number of points, so a half-size
    /// image magnified without smoothing is the same picture, pixel for pixel — at a quarter of
    /// the memory for every full-screen drawable in the pool and a quarter of the pixels to fill.
    /// At an odd size, or on a display that is not Retina, the overlay keeps full resolution.
    fn apply_render_divisor(&mut self, display_scale: u8) {
        let divisor = if crate::platform::OVERLAY_HALF_RESOLUTION
            && self.monitor.scale_factor >= 2.0
            && display_scale.is_multiple_of(2)
        {
            2
        } else {
            1
        };
        if divisor == self.render_divisor {
            return;
        }
        self.render_divisor = divisor;
        (self.config.width, self.config.height) = drawable_size(self.layout, divisor);
        self.surface.configure(&self.device, &self.config);
        // Occlusion is measured in drawable pixels, and cached belongings are snapped to the grid.
        self.last_occlusion = None;
        self.object_vertex_cache_key = None;
    }

    /// Round a physical-pixel position onto the drawable's own pixel grid, so every sprite edge
    /// falls exactly between two drawable pixels and nearest sampling stays even.
    fn snap(&self, physical: f32) -> f32 {
        let divisor = self.render_divisor as f32;
        (physical / divisor).round() * divisor
    }

    pub fn render(
        &mut self,
        save: &SaveFile,
        cursor: CursorSnapshot,
        habitat_editor: Option<&HabitatPolicy>,
        windows: &[DesktopWindow],
        milestone: Option<CreatureId>,
        ui: OverlayUi<'_>,
    ) -> Result<()> {
        self.apply_render_divisor(save.settings.display_scale);
        self.update_occlusion_cache(save, windows);
        let monitor_fully_occluded = rects_cover(self.monitor.bounds, &self.occlusion_rects);
        let occlusion = self.occlusion_uniform(&self.occlusion_rects);
        // Whoever is visiting draws exactly like a member — atlas, face, props, bubbles and all —
        // and is evicted by the same `retain` below the moment it goes home.
        let visible = drawn_on_monitor(save, self.monitor.id, monitor_fully_occluded);
        for creature in &visible {
            self.ensure_sprite(
                creature,
                save.settings.reduce_motion,
                save.companion.appearance.sprite_outline,
            );
        }
        let shelter_visible = !monitor_fully_occluded
            && save.home.is_active()
            && save.home.display == Some(self.monitor.display_key)
            && resolved_home_anchor(
                &save.home,
                &self.monitor,
                save.settings.display_scale,
                &save.settings.habitat,
            )
            .is_some();
        if shelter_visible {
            let mut visible_decorations = [ShelterDecorationKind::Leaf; 6];
            let mut count = 0;
            for kind in &save.home.decorations.decorations {
                if save.home.hidden_decorations & (1 << kind.index()) == 0 {
                    visible_decorations[count] = *kind;
                    count += 1;
                }
            }
            self.ensure_shelter(save.home.shelter, &visible_decorations[..count]);
        }
        self.sprites
            .retain(|id, _| visible.iter().any(|creature| creature.id == *id));
        let bubble_creature = milestone.and_then(|creature_id| {
            visible
                .iter()
                .find(|creature| creature.id == creature_id)
                .copied()
        });
        if let Some(creature) = bubble_creature {
            self.ensure_bubble(creature.id);
        } else {
            self.bubble = None;
        }
        if visible
            .iter()
            .any(|creature| creature.state.action == ActionKind::PresentDiscovery)
        {
            self.ensure_trinket_atlas(save);
        }
        let object_vertices = if !shelter_visible || save.objects.objects.is_empty() {
            Vec::new()
        } else {
            self.ensure_colony_object_atlas(save.colony_seed);
            self.cached_colony_object_vertices(save).to_vec()
        };
        let village_vertices = if shelter_visible {
            self.village_vertices(save)
        } else {
            Vec::new()
        };
        let mut vertices = Vec::with_capacity(
            object_vertices.len()
                + visible.len() * 18
                + village_vertices.len()
                + usize::from(bubble_creature.is_some()) * 6,
        );
        let mut creature_draws = Vec::with_capacity(visible.len());
        vertices.extend_from_slice(&object_vertices);
        let object_vertex_count = object_vertices.len();
        vertices.extend_from_slice(&village_vertices);
        let shelter_vertex_count = village_vertices.len();
        for creature in &visible {
            let sprite = self.sprites.get(&creature.id).expect("sprite atlas exists");
            let face_state = CreatureRenderer::resolve_face_state(
                creature,
                cursor,
                save.settings.cursor_reactions,
            );
            let start = vertices.len();
            let (body, face, trinket) =
                self.vertices_for(creature, save.settings.display_scale, sprite, face_state);
            vertices.extend_from_slice(&body);
            vertices.extend_from_slice(&face);
            if let Some(trinket) = trinket {
                vertices.extend_from_slice(&trinket);
            }
            creature_draws.push((creature.id, start, trinket.is_some()));
        }
        let bubble_start = vertices.len();
        if let Some(creature) = bubble_creature
            && let Some(bubble_vertices) =
                self.bubble_vertices(creature, save.settings.display_scale)
        {
            vertices.extend_from_slice(&bubble_vertices);
        }
        let bubble_vertex_count = vertices.len() - bubble_start;
        let ui_vertices = self.ui_vertices(&visible, save.settings.display_scale, ui);
        self.sync_ui_atlas(!ui_vertices.is_empty());
        let ui_start = vertices.len();
        vertices.extend_from_slice(&ui_vertices);
        let ui_vertex_count = ui_vertices.len();
        if !vertices.is_empty() {
            self.ensure_vertex_capacity(vertices.len());
            self.queue
                .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        }
        if self.last_occlusion != Some(occlusion) {
            self.queue
                .write_buffer(&self.occlusion_buffer, 0, bytemuck::bytes_of(&occlusion));
            self.last_occlusion = Some(occlusion);
        }
        let zone_vertices = habitat_editor
            .map(|policy| self.zone_vertices(policy))
            .unwrap_or_default();
        if !zone_vertices.is_empty() {
            self.queue.write_buffer(
                &self.zone_vertex_buffer,
                0,
                bytemuck::cast_slice(&zone_vertices),
            );
        }
        let has_visual_content = !vertices.is_empty() || !zone_vertices.is_empty();
        if !has_visual_content && !self.has_visual_content {
            return Ok(());
        }
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(output)
            | wgpu::CurrentSurfaceTexture::Suboptimal(output) => {
                self.consecutive_surface_stalls = 0;
                output
            }
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.recover_surface();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                self.consecutive_surface_stalls = self.consecutive_surface_stalls.saturating_add(1);
                if surface_stalls_require_recovery(self.consecutive_surface_stalls) {
                    tracing::warn!("overlay surface stalled; reconfiguring presentation");
                    self.recover_surface();
                }
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                anyhow::bail!("surface texture validation failed")
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Formiga frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("transparent creature pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if !zone_vertices.is_empty() {
                pass.set_pipeline(&self.zone_pipeline);
                pass.set_vertex_buffer(0, self.zone_vertex_buffer.slice(..));
                pass.draw(0..zone_vertices.len() as u32, 0..1);
            }
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.occlusion_bind_group, &[]);
            if object_vertex_count > 0
                && let Some(objects) = &self.colony_objects
            {
                pass.set_bind_group(1, &objects.bind_group, &[]);
                pass.set_vertex_buffer(
                    0,
                    self.vertex_buffer
                        .slice(..(object_vertex_count * std::mem::size_of::<Vertex>()) as u64),
                );
                pass.draw(0..object_vertex_count as u32, 0..1);
            }
            if shelter_vertex_count > 0
                && let Some(shelter) = &self.shelter
            {
                let shelter_start = (object_vertex_count * std::mem::size_of::<Vertex>()) as u64;
                let shelter_end =
                    shelter_start + (shelter_vertex_count * std::mem::size_of::<Vertex>()) as u64;
                pass.set_bind_group(1, &shelter.bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice(shelter_start..shelter_end));
                pass.draw(0..shelter_vertex_count as u32, 0..1);
            }
            for (creature_id, start, has_trinket) in creature_draws {
                if let Some(sprite) = self.sprites.get(&creature_id) {
                    let body_start = (start * std::mem::size_of::<Vertex>()) as u64;
                    let body_end = body_start + (6 * std::mem::size_of::<Vertex>()) as u64;
                    pass.set_bind_group(1, &sprite.body_bind_group, &[]);
                    pass.set_vertex_buffer(0, self.vertex_buffer.slice(body_start..body_end));
                    pass.draw(0..6, 0..1);
                    let face_end = body_end + (6 * std::mem::size_of::<Vertex>()) as u64;
                    pass.set_bind_group(1, &sprite.face_bind_group, &[]);
                    pass.set_vertex_buffer(0, self.vertex_buffer.slice(body_end..face_end));
                    pass.draw(0..6, 0..1);
                    if has_trinket && let Some(trinkets) = &self.trinkets {
                        let trinket_end = face_end + (6 * std::mem::size_of::<Vertex>()) as u64;
                        pass.set_bind_group(1, &trinkets.bind_group, &[]);
                        pass.set_vertex_buffer(0, self.vertex_buffer.slice(face_end..trinket_end));
                        pass.draw(0..6, 0..1);
                    }
                }
            }
            if bubble_vertex_count > 0
                && let Some(bubble) = &self.bubble
            {
                let start = (bubble_start * std::mem::size_of::<Vertex>()) as u64;
                let end = start + (bubble_vertex_count * std::mem::size_of::<Vertex>()) as u64;
                pass.set_bind_group(1, &bubble.bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice(start..end));
                pass.draw(0..bubble_vertex_count as u32, 0..1);
            }
            // Every icon bubble and every part of the open menu comes out of one texture, so the
            // whole on-desktop UI is a single extra bind group and a single extra draw, last and
            // therefore on top of the colony it is talking about.
            if ui_vertex_count > 0
                && let Some(atlas) = &self.ui_atlas
            {
                let start = (ui_start * std::mem::size_of::<Vertex>()) as u64;
                let end = start + (ui_vertex_count * std::mem::size_of::<Vertex>()) as u64;
                pass.set_bind_group(1, &atlas.bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice(start..end));
                pass.draw(0..ui_vertex_count as u32, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(output);
        self.has_visual_content = has_visual_content;
        Ok(())
    }

    fn ensure_vertex_capacity(&mut self, required: usize) {
        let Some(capacity) = expanded_vertex_capacity(self.vertex_capacity, required) else {
            return;
        };
        self.vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("creature vertices"),
            size: (capacity * std::mem::size_of::<Vertex>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.vertex_capacity = capacity;
    }

    fn recover_surface(&mut self) {
        self.layout = self.window.inner_size();
        (self.config.width, self.config.height) = drawable_size(self.layout, self.render_divisor);
        self.surface.configure(&self.device, &self.config);
        self.last_occlusion = None;
        self.has_visual_content = false;
        self.consecutive_surface_stalls = 0;
        self.window.request_redraw();
    }

    pub fn needs_redraw(
        &self,
        save: &SaveFile,
        habitat_editor: Option<&HabitatPolicy>,
        windows: &[DesktopWindow],
        ui_active: bool,
    ) -> bool {
        if self.has_visual_content || habitat_editor.is_some() || ui_active {
            return true;
        }
        let occlusion_rects = visible_occlusion_rects(
            self.monitor.bounds,
            windows,
            &save.settings.application_occlusion_rules,
            save.settings.fullscreen_app_occlusion,
        );
        let fully_occluded = rects_cover(self.monitor.bounds, &occlusion_rects);
        let creature_visible = !drawn_on_monitor(save, self.monitor.id, fully_occluded).is_empty();
        let shelter_visible = !fully_occluded
            && save.home.is_active()
            && save.home.display == Some(self.monitor.display_key)
            && resolved_home_anchor(
                &save.home,
                &self.monitor,
                save.settings.display_scale,
                &save.settings.habitat,
            )
            .is_some();
        let cottages = formiga_core::colony_cottages(&save.creatures);
        let object_visible = shelter_visible
            && save.objects.objects.iter().enumerate().any(|(slot, _)| {
                formiga_core::home_object_position(
                    &save.home,
                    slot,
                    &cottages,
                    std::slice::from_ref(&self.monitor),
                    &save.settings.habitat,
                    save.settings.display_scale,
                )
                .is_some()
            });
        creature_visible || shelter_visible || object_visible
    }

    fn update_occlusion_cache(&mut self, save: &SaveFile, windows: &[DesktopWindow]) {
        let rules = &save.settings.application_occlusion_rules;
        let fullscreen = save.settings.fullscreen_app_occlusion;
        let changed = !self.occlusion_initialized
            || self.occlusion_monitor_bounds != self.monitor.bounds
            || self.occlusion_fullscreen_enabled != fullscreen
            || self.occlusion_windows != windows
            || self.occlusion_rules != *rules;
        if !changed {
            return;
        }
        self.occlusion_rects =
            visible_occlusion_rects(self.monitor.bounds, windows, rules, fullscreen);
        self.occlusion_windows.clear();
        self.occlusion_windows.extend_from_slice(windows);
        self.occlusion_rules.clone_from(rules);
        self.occlusion_fullscreen_enabled = fullscreen;
        self.occlusion_monitor_bounds = self.monitor.bounds;
        self.occlusion_initialized = true;
    }

    fn zone_vertices(&self, policy: &HabitatPolicy) -> Vec<ZoneVertex> {
        let mut vertices = Vec::new();
        for region in accessible_regions(policy, &self.monitor) {
            vertices.extend_from_slice(&self.zone_rect_vertices(region, [0.18, 0.72, 0.45, 0.20]));
        }
        for zone in policy.zones.iter().filter(|zone| {
            zone.enabled
                && zone.display == self.monitor.display_key
                && zone.kind == HabitatZoneKind::Excluded
        }) {
            let bounds = self.monitor.usable_bounds;
            let rect = formiga_core::DesktopRect {
                x: bounds.x + zone.normalized_bounds.x.clamp(0.0, 1.0) * bounds.width,
                y: bounds.y + zone.normalized_bounds.y.clamp(0.0, 1.0) * bounds.height,
                width: zone.normalized_bounds.width.clamp(0.0, 1.0) * bounds.width,
                height: zone.normalized_bounds.height.clamp(0.0, 1.0) * bounds.height,
            };
            if let Some(rect) = rect.intersection(bounds) {
                vertices
                    .extend_from_slice(&self.zone_rect_vertices(rect, [0.88, 0.30, 0.25, 0.28]));
            }
        }
        vertices.truncate(768);
        vertices
    }

    fn occlusion_uniform(&self, rects: &[DesktopRect]) -> OcclusionUniform {
        let mut uniform = OcclusionUniform::zeroed();
        // The shader compares these with the fragment's own position, which is in drawable pixels.
        let to_drawable = self.monitor.scale_factor / self.render_divisor as f32;
        for (target, rect) in uniform.rects.iter_mut().zip(rects.iter()) {
            *target = [
                (rect.x - self.monitor.bounds.x) * to_drawable,
                (rect.y - self.monitor.bounds.y) * to_drawable,
                (rect.right() - self.monitor.bounds.x) * to_drawable,
                (rect.bottom() - self.monitor.bounds.y) * to_drawable,
            ];
        }
        uniform.metadata[0] = rects.len().min(MAX_OCCLUSION_RECTS) as u32;
        uniform
    }

    fn zone_rect_vertices(
        &self,
        rect: formiga_core::DesktopRect,
        color: [f32; 4],
    ) -> [ZoneVertex; 6] {
        let left_px = self.snap((rect.x - self.monitor.bounds.x) * self.monitor.scale_factor);
        let right_px =
            self.snap((rect.right() - self.monitor.bounds.x) * self.monitor.scale_factor);
        let top_px = self.snap((rect.y - self.monitor.bounds.y) * self.monitor.scale_factor);
        let bottom_px =
            self.snap((rect.bottom() - self.monitor.bounds.y) * self.monitor.scale_factor);
        let left = left_px / self.layout.width as f32 * 2.0 - 1.0;
        let right = right_px / self.layout.width as f32 * 2.0 - 1.0;
        let top = 1.0 - top_px / self.layout.height as f32 * 2.0;
        let bottom = 1.0 - bottom_px / self.layout.height as f32 * 2.0;
        [
            ZoneVertex {
                position: [left, top],
                color,
            },
            ZoneVertex {
                position: [right, top],
                color,
            },
            ZoneVertex {
                position: [right, bottom],
                color,
            },
            ZoneVertex {
                position: [left, top],
                color,
            },
            ZoneVertex {
                position: [right, bottom],
                color,
            },
            ZoneVertex {
                position: [left, bottom],
                color,
            },
        ]
    }

    fn ensure_sprite(&mut self, creature: &Creature, reduce_motion: bool, outline: bool) {
        let requires_bake = self.sprites.get(&creature.id).is_none_or(|sprite| {
            sprite.reduce_motion != reduce_motion || sprite.outline != outline
        });
        if requires_bake {
            let atlas = build_atlas_pixels(creature, reduce_motion, outline);
            let body_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("procedural creature body atlas"),
                size: wgpu::Extent3d {
                    width: atlas.body_width,
                    height: atlas.body_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &body_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &atlas.body_pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(atlas.body_width * 4),
                    rows_per_image: Some(atlas.body_height),
                },
                wgpu::Extent3d {
                    width: atlas.body_width,
                    height: atlas.body_height,
                    depth_or_array_layers: 1,
                },
            );
            let body_view = body_texture.create_view(&wgpu::TextureViewDescriptor::default());
            let body_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("creature body frame bindings"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&body_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            let face_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("procedural creature face atlas"),
                size: wgpu::Extent3d {
                    width: atlas.face_width,
                    height: atlas.face_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &face_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &atlas.face_pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(atlas.face_width * 4),
                    rows_per_image: Some(atlas.face_height),
                },
                wgpu::Extent3d {
                    width: atlas.face_width,
                    height: atlas.face_height,
                    depth_or_array_layers: 1,
                },
            );
            let face_view = face_texture.create_view(&wgpu::TextureViewDescriptor::default());
            let face_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("creature face frame bindings"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&face_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.sprites.insert(
                creature.id,
                SpriteGpu {
                    _body_texture: body_texture,
                    body_bind_group,
                    _face_texture: face_texture,
                    face_bind_group,
                    reduce_motion,
                    outline,
                    body_atlas_width: atlas.body_width,
                    body_atlas_height: atlas.body_height,
                    face_atlas_width: atlas.face_width,
                    face_atlas_height: atlas.face_height,
                    face_anchors: atlas.face_anchors,
                    silhouette: atlas.silhouette,
                    resting_baseline: CreatureRenderer::resting_baseline(
                        &creature.appearance,
                        reduce_motion,
                    ),
                },
            );
        }
    }

    fn ensure_shelter(&mut self, genome: ShelterGenome, decorations: &[ShelterDecorationKind]) {
        if self
            .shelter
            .as_ref()
            .is_some_and(|shelter| shelter.genome == genome && shelter.decorations == decorations)
        {
            return;
        }
        let decorations: Vec<_> = decorations
            .iter()
            .copied()
            .take(formiga_core::MAX_SHELTER_DECORATIONS)
            .collect();
        let pixels = ShelterRenderer::render_village(&genome, &decorations).rgba_bytes();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("procedural colony village"),
            size: wgpu::Extent3d {
                width: VILLAGE_ATLAS_SIZE,
                height: VILLAGE_ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(VILLAGE_ATLAS_SIZE * 4),
                rows_per_image: Some(VILLAGE_ATLAS_SIZE),
            },
            wgpu::Extent3d {
                width: VILLAGE_ATLAS_SIZE,
                height: VILLAGE_ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("procedural village bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.shelter = Some(ShelterGpu {
            _texture: texture,
            bind_group,
            genome,
            decorations,
        });
    }

    fn ensure_colony_object_atlas(&mut self, colony_seed: [u8; 32]) {
        if self
            .colony_objects
            .as_ref()
            .is_some_and(|objects| objects.colony_seed == colony_seed)
        {
            return;
        }
        let pixels = ColonyObjectRenderer::render_atlas(colony_seed).rgba_bytes();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("static colony object atlas"),
            size: wgpu::Extent3d {
                width: COLONY_OBJECT_ATLAS_WIDTH,
                height: COLONY_OBJECT_ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(COLONY_OBJECT_ATLAS_WIDTH * 4),
                rows_per_image: Some(COLONY_OBJECT_ATLAS_HEIGHT),
            },
            wgpu::Extent3d {
                width: COLONY_OBJECT_ATLAS_WIDTH,
                height: COLONY_OBJECT_ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("static colony object bindings"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.colony_objects = Some(ColonyObjectsGpu {
            _texture: texture,
            bind_group,
            colony_seed,
        });
    }

    /// One sheet per colony, rebuilt only when the colony seed or a member's colours change —
    /// the same caching the colony-object atlas uses.
    fn ensure_trinket_atlas(&mut self, save: &SaveFile) {
        let key = TrinketAtlasKey::of(save);
        if self
            .trinkets
            .as_ref()
            .is_some_and(|trinkets| trinkets.key == key)
        {
            return;
        }
        let members: Vec<formiga_art::Palette> = save
            .creatures
            .iter()
            .map(|creature| formiga_art::palette_for(&creature.appearance))
            .collect();
        let pixels = TrinketAtlasRenderer::render(save.colony_seed, &members).rgba_bytes();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("static colony trinket atlas"),
            size: wgpu::Extent3d {
                width: TRINKET_ATLAS_WIDTH,
                height: TRINKET_ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(TRINKET_ATLAS_WIDTH * 4),
                rows_per_image: Some(TRINKET_ATLAS_HEIGHT),
            },
            wgpu::Extent3d {
                width: TRINKET_ATLAS_WIDTH,
                height: TRINKET_ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("static colony trinket bindings"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.trinkets = Some(TrinketAtlasGpu {
            _texture: texture,
            bind_group,
            key,
        });
    }

    fn cached_colony_object_vertices(&mut self, save: &SaveFile) -> &[Vertex] {
        let cottages = formiga_core::colony_cottages(&save.creatures);
        let key = ObjectVertexCacheKey {
            objects: save.objects.objects.clone(),
            cottages: cottages.clone(),
            home: save.home.clone(),
            habitat: save.settings.habitat.clone(),
            monitor_bounds: self.monitor.bounds,
            monitor_usable_bounds: self.monitor.usable_bounds,
            monitor_scale_factor: self.monitor.scale_factor,
            display_scale: save.settings.display_scale,
        };
        if self.object_vertex_cache_key.as_ref() == Some(&key) {
            return &self.object_vertices;
        }
        self.object_vertices.clear();
        for (slot, object) in save
            .objects
            .objects
            .iter()
            .take(formiga_core::MAX_COLONY_OBJECTS)
            .enumerate()
        {
            let Some((monitor_id, point)) = formiga_core::home_object_position(
                &save.home,
                slot,
                &cottages,
                std::slice::from_ref(&self.monitor),
                &save.settings.habitat,
                save.settings.display_scale,
            ) else {
                continue;
            };
            if monitor_id != self.monitor.id {
                continue;
            }
            self.object_vertices
                .extend_from_slice(&self.object_vertices_for(
                    object,
                    point,
                    save.settings.display_scale,
                ));
        }
        self.object_vertex_cache_key = Some(key);
        &self.object_vertices
    }

    fn object_vertices_for(
        &self,
        object: &ColonyObject,
        point: formiga_core::Point,
        display_scale: u8,
    ) -> [Vertex; 6] {
        let size = COLONY_OBJECT_SIZE as f32 * f32::from(display_scale);
        let local_x = self.snap((point.x - self.monitor.bounds.x) * self.monitor.scale_factor);
        let contact_y = self.snap((point.y - self.monitor.bounds.y) * self.monitor.scale_factor);
        let left = (local_x - size * 0.5) / self.layout.width as f32 * 2.0 - 1.0;
        let right = (local_x + size * 0.5) / self.layout.width as f32 * 2.0 - 1.0;
        let top_px = contact_y - size;
        let top = 1.0 - top_px / self.layout.height as f32 * 2.0;
        let bottom = 1.0 - contact_y / self.layout.height as f32 * 2.0;
        let u_left = f32::from(object.kind.index()) / 8.0;
        let u_right = f32::from(object.kind.index() + 1) / 8.0;
        let vertex = |position, uv| Vertex {
            position,
            uv,
            occlusion_enabled: 1.0,
        };
        [
            vertex([left, top], [u_left, 0.0]),
            vertex([right, top], [u_right, 0.0]),
            vertex([right, bottom], [u_right, 1.0]),
            vertex([left, top], [u_left, 0.0]),
            vertex([right, bottom], [u_right, 1.0]),
            vertex([left, bottom], [u_left, 1.0]),
        ]
    }

    fn ensure_bubble(&mut self, creature_id: CreatureId) {
        if self
            .bubble
            .as_ref()
            .is_some_and(|bubble| bubble.creature_id == creature_id)
        {
            return;
        }
        let canvas = MilestoneBubbleRenderer::render();
        let width = canvas.width();
        let height = canvas.height();
        let pixels = canvas.rgba_bytes();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("temporary milestone bubble"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("temporary milestone bubble bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.bubble = Some(BubbleGpu {
            _texture: texture,
            bind_group,
            creature_id,
            width,
            height,
        });
    }

    /// Every dwelling in the village, sampled from its own cell of the shared atlas. The
    /// colony house is always first; companion cottages follow along the same ground line.
    fn village_vertices(&self, save: &SaveFile) -> Vec<Vertex> {
        let cottages = formiga_core::colony_cottages(&save.creatures);
        let mut vertices = Vec::with_capacity((cottages.len() + 1) * 6);
        for slot in 0..=cottages.len() {
            let Some((monitor_id, point)) = formiga_core::home_dwelling_position(
                &save.home,
                slot,
                &cottages,
                std::slice::from_ref(&self.monitor),
                &save.settings.habitat,
                save.settings.display_scale,
            ) else {
                continue;
            };
            if monitor_id != self.monitor.id {
                continue;
            }
            let kind = if slot == 0 {
                formiga_core::DwellingKind::Main
            } else {
                cottages[slot - 1]
            };
            vertices.extend_from_slice(&self.dwelling_vertices(
                point,
                kind,
                save.settings.display_scale,
            ));
        }
        vertices
    }

    fn dwelling_vertices(
        &self,
        anchor: formiga_core::Point,
        kind: formiga_core::DwellingKind,
        display_scale: u8,
    ) -> [Vertex; 6] {
        let size = SHELTER_SIZE as f32 * f32::from(display_scale);
        let local_x = self.snap((anchor.x - self.monitor.bounds.x) * self.monitor.scale_factor);
        let local_y = self.snap((anchor.y - self.monitor.bounds.y) * self.monitor.scale_factor);
        let left = (local_x - size / 2.0) / self.layout.width as f32 * 2.0 - 1.0;
        let right = (local_x + size / 2.0) / self.layout.width as f32 * 2.0 - 1.0;
        let top = 1.0 - (local_y - size) / self.layout.height as f32 * 2.0;
        let bottom = 1.0 - local_y / self.layout.height as f32 * 2.0;
        let (u, v) = match kind {
            formiga_core::DwellingKind::Main => (0.0, 0.0),
            formiga_core::DwellingKind::Cottage => (0.5, 0.0),
            formiga_core::DwellingKind::MiniCottage => (0.0, 0.5),
        };
        let vertex = |position, uv| Vertex {
            position,
            uv,
            occlusion_enabled: 1.0,
        };
        [
            vertex([left, top], [u, v]),
            vertex([right, top], [u + 0.5, v]),
            vertex([right, bottom], [u + 0.5, v + 0.5]),
            vertex([left, top], [u, v]),
            vertex([right, bottom], [u + 0.5, v + 0.5]),
            vertex([left, bottom], [u, v + 0.5]),
        ]
    }

    fn bubble_vertices(&self, creature: &Creature, display_scale: u8) -> Option<[Vertex; 6]> {
        let bubble = self
            .bubble
            .as_ref()
            .filter(|bubble| bubble.creature_id == creature.id)?;
        let scale = 2.0;
        let width = bubble.width as f32 * scale;
        let height = bubble.height as f32 * scale;
        let local_x = self
            .snap((creature.state.position.x - self.monitor.bounds.x) * self.monitor.scale_factor);
        let contact_y = self
            .snap((creature.state.position.y - self.monitor.bounds.y) * self.monitor.scale_factor);
        let creature_height = FRAME_SIZE as f32 * f32::from(display_scale);
        let center_x = local_x.clamp(width / 2.0, self.layout.width as f32 - width / 2.0);
        let bottom_px = (contact_y - creature_height - 6.0).max(height);
        let top_px = bottom_px - height;
        let left = (center_x - width / 2.0) / self.layout.width as f32 * 2.0 - 1.0;
        let right = (center_x + width / 2.0) / self.layout.width as f32 * 2.0 - 1.0;
        let top = 1.0 - top_px / self.layout.height as f32 * 2.0;
        let bottom = 1.0 - bottom_px / self.layout.height as f32 * 2.0;
        let vertex = |position, uv| Vertex {
            position,
            uv,
            occlusion_enabled: 1.0,
        };
        Some([
            vertex([left, top], [0.0, 0.0]),
            vertex([right, top], [1.0, 0.0]),
            vertex([right, bottom], [1.0, 1.0]),
            vertex([left, top], [0.0, 0.0]),
            vertex([right, bottom], [1.0, 1.0]),
            vertex([left, bottom], [0.0, 1.0]),
        ])
    }

    /// Build the UI atlas the first time a bubble or a menu wants it, and let it go again once
    /// nothing has for a while. It is one 256x80 RGBA upload — 81,920 bytes — so the overlay of a
    /// quiet colony holds nothing for a feature nobody is using.
    fn sync_ui_atlas(&mut self, needed: bool) {
        if !needed {
            self.ui_atlas_idle = self.ui_atlas_idle.saturating_add(1);
            if self.ui_atlas_idle >= UI_ATLAS_IDLE_FRAMES {
                self.ui_atlas = None;
            }
            return;
        }
        self.ui_atlas_idle = 0;
        if self.ui_atlas.is_some() {
            return;
        }
        let pixels = UiAtlasRenderer::render().rgba_bytes();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("on-desktop ui atlas"),
            size: wgpu::Extent3d {
                width: UI_ATLAS_WIDTH,
                height: UI_ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(UI_ATLAS_WIDTH * 4),
                rows_per_image: Some(UI_ATLAS_HEIGHT),
            },
            wgpu::Extent3d {
                width: UI_ATLAS_WIDTH,
                height: UI_ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("on-desktop ui atlas bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.ui_atlas = Some(UiAtlasGpu {
            _texture: texture,
            bind_group,
        });
    }

    /// The creature's centre column, the crown of its head, and the row just past its lowest drawn
    /// pixel, all in monitor-local physical pixels. This is the frame the overlay actually draws,
    /// gesture and pose included, not the 48x48 box it is drawn inside.
    fn creature_extent(
        &self,
        creature: &Creature,
        sprite: &SpriteGpu,
        display_scale: u8,
    ) -> (f32, f32, f32) {
        let scale = f32::from(display_scale);
        let local_x = self
            .snap((creature.state.position.x - self.monitor.bounds.x) * self.monitor.scale_factor);
        let contact_y = self
            .snap((creature.state.position.y - self.monitor.bounds.y) * self.monitor.scale_factor);
        let placement = FramePlacement::for_creature(creature, sprite.resting_baseline);
        let frame_top = contact_y + placement.origin_y as f32 * scale;
        let clip = BodyClip::for_creature(creature);
        let frame =
            MotionSignature::for_creature(creature).frame(clip, creature.state.action_elapsed);
        // Mirroring a frame never changes which rows it fills, so one silhouette serves both ways.
        let (top, bottom) = sprite
            .silhouette
            .get(atlas_slot(clip, frame) as usize)
            .copied()
            .unwrap_or((0, FRAME_SIZE as u8));
        (
            local_x,
            frame_top + f32::from(top) * scale,
            frame_top + f32::from(bottom) * scale,
        )
    }

    /// Where a menu for this creature would go, for `creature_menu` to place a strip against.
    /// `None` until the creature's own atlas has been baked, which happens on its first frame.
    pub fn menu_anchor(&self, creature: &Creature, display_scale: u8) -> Option<MenuAnchor> {
        let sprite = self.sprites.get(&creature.id)?;
        let (centre_x, head_top, foot_bottom) =
            self.creature_extent(creature, sprite, display_scale);
        let bounds = self.monitor.bounds;
        let usable = self.monitor.usable_bounds;
        let factor = self.monitor.scale_factor;
        Some(MenuAnchor {
            centre_x,
            head_top,
            foot_bottom,
            art_scale: f32::from(display_scale),
            grid: self.render_divisor as f32,
            usable: LocalRect {
                x: (usable.x - bounds.x) * factor,
                y: (usable.y - bounds.y) * factor,
                width: usable.width * factor,
                height: usable.height * factor,
            },
            monitor_origin: Point {
                x: bounds.x,
                y: bounds.y,
            },
            scale_factor: factor,
        })
    }

    /// Every quad sampled from the UI atlas this frame: one bubble per creature answering, then
    /// the open menu's frame, its four cells, and the label tab under whichever one is hovered.
    fn ui_vertices(
        &self,
        visible: &[&Creature],
        display_scale: u8,
        ui: OverlayUi<'_>,
    ) -> Vec<Vertex> {
        let mut vertices = Vec::new();
        for bubble in ui.bubbles {
            let Some(creature) = visible
                .iter()
                .find(|creature| creature.id == bubble.creature_id)
            else {
                continue;
            };
            let Some(sprite) = self.sprites.get(&creature.id) else {
                continue;
            };
            let (centre_x, head_top, _) = self.creature_extent(creature, sprite, display_scale);
            let (x, y) = bubble_origin(centre_x, head_top, f32::from(display_scale), self.layout);
            // A dragged creature opts out of occlusion, and so does what it is thinking.
            let occlusion = (creature.state.action != ActionKind::Dragged) as u8 as f32;
            vertices.extend_from_slice(&ui_atlas_quad(
                UiAtlasRenderer::bubble(bubble.icon, bubble.growth(ui.reduce_motion)),
                self.snap(x),
                self.snap(y),
                f32::from(display_scale),
                false,
                occlusion,
                self.layout,
            ));
        }

        if let Some(menu) = ui.menu
            && visible
                .iter()
                .any(|creature| creature.id == menu.creature_id)
        {
            vertices.extend(menu_quads(menu, self.layout));
        }
        vertices
    }

    fn vertices_for(
        &self,
        creature: &Creature,
        display_scale: u8,
        sprite: &SpriteGpu,
        face_state: FaceRenderState,
    ) -> ([Vertex; 6], [Vertex; 6], Option<[Vertex; 6]>) {
        // Creature scale is expressed in physical pixels. Applying the monitor scale factor a
        // second time made a 3x creature twice the intended size on Retina displays.
        let sprite_size = FRAME_SIZE as f32 * display_scale as f32;
        let local_x = self
            .snap((creature.state.position.x - self.monitor.bounds.x) * self.monitor.scale_factor);
        let contact_y = self
            .snap((creature.state.position.y - self.monitor.bounds.y) * self.monitor.scale_factor);
        let placement = FramePlacement::for_creature(creature, sprite.resting_baseline);
        let frame_top = contact_y + placement.origin_y as f32 * display_scale as f32;
        let frame_bottom = frame_top + sprite_size;
        // Gap traversal reuses the normal walk atlas and briefly narrows both body and layered face
        // quads. The source texture and hit mask remain unchanged and no runtime art is generated.
        let horizontal_scale = creature_horizontal_scale(creature.state.action);
        let sprite_width = sprite_size * horizontal_scale;
        let left = (local_x - sprite_width / 2.0) / self.layout.width as f32 * 2.0 - 1.0;
        let right = (local_x + sprite_width / 2.0) / self.layout.width as f32 * 2.0 - 1.0;
        let top = 1.0 - frame_top / self.layout.height as f32 * 2.0;
        let bottom = 1.0 - frame_bottom / self.layout.height as f32 * 2.0;
        // Each creature keeps its own cadence and phase; the atlas and slots are unchanged. A
        // gesture shows its own baked clip in place of the action's.
        let clip = BodyClip::for_creature(creature);
        let frame =
            MotionSignature::for_creature(creature).frame(clip, creature.state.action_elapsed);
        let slot = atlas_slot(clip, frame);
        let column = slot % ATLAS_COLUMNS;
        let row = slot / ATLAS_COLUMNS;
        let mut u_left = column as f32 * FRAME_SIZE as f32 / sprite.body_atlas_width as f32;
        let mut u_right = (column + 1) as f32 * FRAME_SIZE as f32 / sprite.body_atlas_width as f32;
        let v_top = row as f32 * FRAME_SIZE as f32 / sprite.body_atlas_height as f32;
        let v_bottom = (row + 1) as f32 * FRAME_SIZE as f32 / sprite.body_atlas_height as f32;
        if !creature.state.facing_right {
            std::mem::swap(&mut u_left, &mut u_right);
        }
        let occlusion_enabled = (creature.state.action != ActionKind::Dragged) as u8 as f32;
        let body = [
            Vertex {
                position: [left, top],
                uv: [u_left, v_top],
                occlusion_enabled,
            },
            Vertex {
                position: [right, top],
                uv: [u_right, v_top],
                occlusion_enabled,
            },
            Vertex {
                position: [right, bottom],
                uv: [u_right, v_bottom],
                occlusion_enabled,
            },
            Vertex {
                position: [left, top],
                uv: [u_left, v_top],
                occlusion_enabled,
            },
            Vertex {
                position: [right, bottom],
                uv: [u_right, v_bottom],
                occlusion_enabled,
            },
            Vertex {
                position: [left, bottom],
                uv: [u_left, v_bottom],
                occlusion_enabled,
            },
        ];

        let anchor = sprite.face_anchors[slot as usize];
        let anchor_x = if creature.state.facing_right {
            anchor.x
        } else {
            FRAME_SIZE as i32 - anchor.x
        } as f32;
        let unscaled_face_center_x = local_x - sprite_size / 2.0 + anchor_x * display_scale as f32;
        let face_center_x = local_x + (unscaled_face_center_x - local_x) * horizontal_scale;
        let face_center_y = frame_top + anchor.y as f32 * display_scale as f32;
        let face_size = FACE_FRAME_SIZE as f32 * display_scale as f32;
        let face_width = face_size * horizontal_scale;
        let face_left = (face_center_x - face_width / 2.0) / self.layout.width as f32 * 2.0 - 1.0;
        let face_right = (face_center_x + face_width / 2.0) / self.layout.width as f32 * 2.0 - 1.0;
        let face_top = 1.0 - (face_center_y - face_size / 2.0) / self.layout.height as f32 * 2.0;
        let face_bottom = 1.0 - (face_center_y + face_size / 2.0) / self.layout.height as f32 * 2.0;
        let mut source_face_state = face_state;
        if !creature.state.facing_right {
            source_face_state.gaze.x = -source_face_state.gaze.x;
        }
        let face_slot = face_atlas_slot(source_face_state);
        let face_column = face_slot % FACE_ATLAS_COLUMNS;
        let face_row = face_slot / FACE_ATLAS_COLUMNS;
        let mut face_u_left =
            face_column as f32 * FACE_FRAME_SIZE as f32 / sprite.face_atlas_width as f32;
        let mut face_u_right =
            (face_column + 1) as f32 * FACE_FRAME_SIZE as f32 / sprite.face_atlas_width as f32;
        if !creature.state.facing_right {
            std::mem::swap(&mut face_u_left, &mut face_u_right);
        }
        let face_v_top = face_row as f32 * FACE_FRAME_SIZE as f32 / sprite.face_atlas_height as f32;
        let face_v_bottom =
            (face_row + 1) as f32 * FACE_FRAME_SIZE as f32 / sprite.face_atlas_height as f32;
        let face = [
            Vertex {
                position: [face_left, face_top],
                uv: [face_u_left, face_v_top],
                occlusion_enabled,
            },
            Vertex {
                position: [face_right, face_top],
                uv: [face_u_right, face_v_top],
                occlusion_enabled,
            },
            Vertex {
                position: [face_right, face_bottom],
                uv: [face_u_right, face_v_bottom],
                occlusion_enabled,
            },
            Vertex {
                position: [face_left, face_top],
                uv: [face_u_left, face_v_top],
                occlusion_enabled,
            },
            Vertex {
                position: [face_right, face_bottom],
                uv: [face_u_right, face_v_bottom],
                occlusion_enabled,
            },
            Vertex {
                position: [face_left, face_bottom],
                uv: [face_u_left, face_v_bottom],
                occlusion_enabled,
            },
        ];
        let trinket = (creature.state.action == ActionKind::PresentDiscovery).then(|| {
            // The colony's own sheet, not this creature's face texture: one quad sampling the
            // cell for the variant it found, on the glint frame for part of the presentation.
            let (cell_x, cell_y, _, _) = TrinketAtlasRenderer::cell_rect(
                creature.state.activity_variant,
                trinket_frame(frame),
            );
            let u_left = cell_x as f32 / TRINKET_ATLAS_WIDTH as f32;
            let u_right = (cell_x + TRINKET_CELL) as f32 / TRINKET_ATLAS_WIDTH as f32;
            let v_top = cell_y as f32 / TRINKET_ATLAS_HEIGHT as f32;
            let v_bottom = (cell_y + TRINKET_CELL) as f32 / TRINKET_ATLAS_HEIGHT as f32;
            // One explicit anchor, shared with the review sheets, rather than a number here.
            let anchor = PropAnchor::for_creature(creature);
            let center_x = face_center_x + anchor.dx * display_scale as f32;
            let center_y = face_center_y + anchor.dy * display_scale as f32;
            let left = (center_x - face_size / 2.0) / self.layout.width as f32 * 2.0 - 1.0;
            let right = (center_x + face_size / 2.0) / self.layout.width as f32 * 2.0 - 1.0;
            let top = 1.0 - (center_y - face_size / 2.0) / self.layout.height as f32 * 2.0;
            let bottom = 1.0 - (center_y + face_size / 2.0) / self.layout.height as f32 * 2.0;
            [
                Vertex {
                    position: [left, top],
                    uv: [u_left, v_top],
                    occlusion_enabled,
                },
                Vertex {
                    position: [right, top],
                    uv: [u_right, v_top],
                    occlusion_enabled,
                },
                Vertex {
                    position: [right, bottom],
                    uv: [u_right, v_bottom],
                    occlusion_enabled,
                },
                Vertex {
                    position: [left, top],
                    uv: [u_left, v_top],
                    occlusion_enabled,
                },
                Vertex {
                    position: [right, bottom],
                    uv: [u_right, v_bottom],
                    occlusion_enabled,
                },
                Vertex {
                    position: [left, bottom],
                    uv: [u_left, v_bottom],
                    occlusion_enabled,
                },
            ]
        });
        (body, face, trinket)
    }
}

fn expanded_vertex_capacity(current: usize, required: usize) -> Option<usize> {
    (required > current).then(|| required.next_power_of_two())
}

fn surface_stalls_require_recovery(stalls: u8) -> bool {
    stalls >= SURFACE_RECOVERY_STALLS
}

/// Transparent composite alpha modes to try, most preferred first.
///
/// Overlay draws use `BlendState::ALPHA_BLENDING`, so the surface texture always ends up holding
/// premultiplied color regardless of platform; only the compositor's interpretation differs.
fn overlay_alpha_preference() -> &'static [wgpu::CompositeAlphaMode] {
    #[cfg(target_os = "windows")]
    {
        // DirectComposition composites `DXGI_ALPHA_MODE_PREMULTIPLIED` only. wgpu maps
        // `PostMultiplied` to `DXGI_ALPHA_MODE_STRAIGHT`, which DXGI does not support for
        // composition swap chains even though it is advertised, leaving creatures invisible.
        &[
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::PostMultiplied,
            wgpu::CompositeAlphaMode::Inherit,
        ]
    }
    #[cfg(not(target_os = "windows"))]
    {
        // CAMetalLayer advertises `PostMultiplied` as its only transparent mode but composites it
        // as premultiplied, so it stays first on macOS.
        &[
            wgpu::CompositeAlphaMode::PostMultiplied,
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::Inherit,
        ]
    }
}

fn overlay_gpu_instance() -> wgpu::Instance {
    #[cfg(target_os = "windows")]
    {
        // An HWND swap chain cannot retain per-pixel transparency while WS_EX_LAYERED is active.
        // DirectComposition can target a layered HWND, preserving both alpha and Windows' reliable
        // cross-process click-through behavior.
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::DX12;
        descriptor.backend_options.dx12.presentation_system =
            wgpu::Dx12SwapchainKind::DxgiFromVisual;
        wgpu::Instance::new(descriptor)
    }
    #[cfg(not(target_os = "windows"))]
    {
        wgpu::Instance::default()
    }
}

pub(crate) fn visible_occlusion_rects(
    monitor: DesktopRect,
    windows: &[DesktopWindow],
    rules: &[ApplicationOcclusionRule],
    fullscreen_app_occlusion: bool,
) -> Vec<DesktopRect> {
    if fullscreen_app_occlusion && monitor_has_fullscreen_window(monitor, windows) {
        return vec![monitor];
    }
    let selected: BTreeSet<_> = rules
        .iter()
        .filter(|rule| rule.enabled)
        .map(|rule| &rule.application)
        .collect();
    let mut output = Vec::new();
    for target in windows.iter().filter(|window| {
        window.visible
            && !window.minimized
            && window
                .application
                .as_ref()
                .is_some_and(|application| selected.contains(application))
    }) {
        let Some(target_bounds) = target.bounds.intersection(monitor) else {
            continue;
        };
        let mut visible = vec![target_bounds];
        for covering in windows.iter().filter(|candidate| {
            candidate.visible
                && !candidate.minimized
                && candidate.z_order < target.z_order
                && candidate.key != target.key
        }) {
            visible = visible
                .into_iter()
                .flat_map(|rect| subtract_rect(rect, covering.bounds))
                .collect();
            if visible.is_empty() {
                break;
            }
        }
        output.extend(visible);
        if output.len() >= MAX_OCCLUSION_RECTS {
            output.truncate(MAX_OCCLUSION_RECTS);
            break;
        }
    }
    output
}

pub(crate) fn monitor_has_fullscreen_window(
    monitor: DesktopRect,
    windows: &[DesktopWindow],
) -> bool {
    windows.iter().any(|window| {
        window.visible && !window.minimized && bounds_match_monitor(window.bounds, monitor)
    })
}

fn bounds_match_monitor(window: DesktopRect, monitor: DesktopRect) -> bool {
    const EDGE_TOLERANCE: f32 = 3.0;
    (window.x - monitor.x).abs() <= EDGE_TOLERANCE
        && (window.y - monitor.y).abs() <= EDGE_TOLERANCE
        && (window.right() - monitor.right()).abs() <= EDGE_TOLERANCE
        && (window.bottom() - monitor.bottom()).abs() <= EDGE_TOLERANCE
}

fn rects_cover(target: DesktopRect, covering: &[DesktopRect]) -> bool {
    let mut remaining = vec![target];
    for cut in covering {
        remaining = remaining
            .into_iter()
            .flat_map(|rect| subtract_rect(rect, *cut))
            .collect();
        if remaining.is_empty() {
            return true;
        }
    }
    false
}

fn subtract_rect(source: DesktopRect, cut: DesktopRect) -> Vec<DesktopRect> {
    let Some(overlap) = source.intersection(cut) else {
        return vec![source];
    };
    [
        DesktopRect {
            x: source.x,
            y: source.y,
            width: source.width,
            height: overlap.y - source.y,
        },
        DesktopRect {
            x: source.x,
            y: overlap.bottom(),
            width: source.width,
            height: source.bottom() - overlap.bottom(),
        },
        DesktopRect {
            x: source.x,
            y: overlap.y,
            width: overlap.x - source.x,
            height: overlap.height,
        },
        DesktopRect {
            x: overlap.right(),
            y: overlap.y,
            width: source.right() - overlap.right(),
            height: overlap.height,
        },
    ]
    .into_iter()
    .filter(|rect| rect.width > 0.0 && rect.height > 0.0)
    .collect()
}

/// Who this monitor draws right now: its own colony members, and the guest while it is out.
///
/// The one list decides three things at once, which is why it is a function rather than a filter
/// written twice: which sprites are baked, which the overlay keeps — anyone not here is evicted —
/// and which creatures a bubble or a menu may be drawn against.
fn drawn_on_monitor(
    save: &SaveFile,
    monitor_id: formiga_core::MonitorId,
    fully_occluded: bool,
) -> Vec<&Creature> {
    save.creatures
        .iter()
        .chain(save.visitors.on_stage())
        .filter(|creature| {
            creature.state.surface.monitor_id == monitor_id
                && creature.state.arrival_delay_secs <= 0.0
                && (!fully_occluded || creature.state.action == ActionKind::Dragged)
        })
        .collect()
}

/// Where a creature's bubble cell goes, in monitor-local physical pixels.
///
/// `BUBBLE_ANCHOR` names the blank cell pixel one row under the tail's tip, and it lands on the
/// art pixel directly above the crown of the head, so the tail reaches down to a one-pixel gap and
/// never covers the creature. A creature at the very top or the very edge of a display keeps its
/// whole bubble on screen.
fn bubble_origin(
    centre_x: f32,
    head_top: f32,
    scale: f32,
    drawable: PhysicalSize<u32>,
) -> (f32, f32) {
    let width = BUBBLE_CELL.0 as f32 * scale;
    let height = BUBBLE_CELL.1 as f32 * scale;
    let x = centre_x - BUBBLE_ANCHOR.0 as f32 * scale;
    let y = head_top - (BUBBLE_ANCHOR.1 + 1) as f32 * scale;
    (
        x.clamp(0.0, (drawable.width as f32 - width).max(0.0)),
        y.clamp(0.0, (drawable.height as f32 - height).max(0.0)),
    )
}

/// The open menu, drawn: its frame, one quad per cell, and the label tab under whichever cell is
/// hovered. Below the creature the frame sprite is drawn upside down, which puts the notch on top
/// pointing up; the frame is symmetric about its body, so only the notch actually moves, and the
/// cells and the tab are placed the right way up on top of it.
fn menu_quads(menu: MenuView<'_>, drawable: PhysicalSize<u32>) -> Vec<Vertex> {
    let placement = menu.placement;
    let art = placement.art_scale;
    let Some(frame) = UiAtlasRenderer::menu_frame(menu.items.len() as u8) else {
        return Vec::new();
    };
    let mut vertices = Vec::with_capacity(6 * (menu.items.len() + 2));
    vertices.extend_from_slice(&ui_atlas_quad(
        frame,
        placement.x,
        placement.y,
        art,
        placement.below,
        1.0,
        drawable,
    ));
    let body = placement.body_rect();
    for (index, icon) in menu.items.iter().enumerate() {
        let Some(cell) = menu.layout.cell(index) else {
            continue;
        };
        vertices.extend_from_slice(&ui_atlas_quad(
            UiAtlasRenderer::menu_icon(*icon, menu.hovered == Some(index)),
            body.x + cell.x as f32 * art,
            body.y + cell.y as f32 * art,
            art,
            false,
            1.0,
            drawable,
        ));
    }
    if let Some(hovered) = menu.hovered
        && let (Some(tab), Some(icon)) = (menu.layout.label_tab(hovered), menu.layout.item(hovered))
    {
        vertices.extend_from_slice(&ui_atlas_quad(
            UiAtlasRenderer::menu_label(icon),
            placement.x + tab.x as f32 * art,
            placement.y + tab.y as f32 * art,
            art,
            false,
            1.0,
            drawable,
        ));
    }
    vertices
}

/// One nearest-sampled quad out of the UI atlas, placed by its top-left corner in monitor-local
/// physical pixels.
fn ui_atlas_quad(
    rect: SpriteRect,
    x: f32,
    y: f32,
    scale: f32,
    flip_vertically: bool,
    occlusion_enabled: f32,
    drawable: PhysicalSize<u32>,
) -> [Vertex; 6] {
    let left = x / drawable.width as f32 * 2.0 - 1.0;
    let right = (x + rect.width as f32 * scale) / drawable.width as f32 * 2.0 - 1.0;
    let top = 1.0 - y / drawable.height as f32 * 2.0;
    let bottom = 1.0 - (y + rect.height as f32 * scale) / drawable.height as f32 * 2.0;
    let u_left = rect.x as f32 / UI_ATLAS_WIDTH as f32;
    let u_right = (rect.x + rect.width) as f32 / UI_ATLAS_WIDTH as f32;
    let mut v_top = rect.y as f32 / UI_ATLAS_HEIGHT as f32;
    let mut v_bottom = (rect.y + rect.height) as f32 / UI_ATLAS_HEIGHT as f32;
    if flip_vertically {
        std::mem::swap(&mut v_top, &mut v_bottom);
    }
    let vertex = |position, uv| Vertex {
        position,
        uv,
        occlusion_enabled,
    };
    [
        vertex([left, top], [u_left, v_top]),
        vertex([right, top], [u_right, v_top]),
        vertex([right, bottom], [u_right, v_bottom]),
        vertex([left, top], [u_left, v_top]),
        vertex([right, bottom], [u_right, v_bottom]),
        vertex([left, bottom], [u_left, v_bottom]),
    ]
}

struct AtlasPixels {
    body_width: u32,
    body_height: u32,
    body_pixels: Vec<u8>,
    face_width: u32,
    face_height: u32,
    face_pixels: Vec<u8>,
    face_anchors: Vec<PixelPoint>,
    silhouette: Vec<(u8, u8)>,
}

/// The first and just-past-the-last rows a frame actually draws on, in art pixels. Measured after
/// the optional outline, because that is the picture the overlay puts on the desktop.
fn silhouette_rows(canvas: &formiga_art::Canvas) -> (u8, u8) {
    match canvas.alpha_bounds() {
        Some((_, top, _, bottom)) => (top as u8, (bottom + 1) as u8),
        None => (0, FRAME_SIZE as u8),
    }
}

fn build_atlas_pixels(creature: &Creature, reduce_motion: bool, outline: bool) -> AtlasPixels {
    let body_slots = total_animation_frames();
    let body_rows = body_slots.div_ceil(ATLAS_COLUMNS);
    let body_width = ATLAS_COLUMNS * FRAME_SIZE;
    let body_height = body_rows * FRAME_SIZE;
    let mut body_pixels = vec![0_u8; (body_width * body_height * 4) as usize];
    let mut face_anchors = vec![PixelPoint::default(); body_slots as usize];
    let mut silhouette = vec![(0_u8, FRAME_SIZE as u8); body_slots as usize];
    for clip in BodyClip::baked() {
        let spec = AnimationSpec::for_clip(clip);
        for frame in 0..spec.frames {
            let mut rendered = CreatureRenderer::render_body_frame(
                &creature.appearance,
                clip,
                frame,
                reduce_motion,
            );
            // Baked once into the atlas, so the edge costs nothing per frame and no draw call.
            if outline {
                CreatureRenderer::outline_frame(&mut rendered.canvas);
            }
            let slot = atlas_slot(clip, frame);
            face_anchors[slot as usize] = rendered.face_anchor;
            silhouette[slot as usize] = silhouette_rows(&rendered.canvas);
            blit_atlas_frame(
                &mut body_pixels,
                body_width,
                slot % ATLAS_COLUMNS * FRAME_SIZE,
                slot / ATLAS_COLUMNS * FRAME_SIZE,
                FRAME_SIZE,
                &rendered.canvas.rgba_bytes(),
            );
        }
    }

    let face_slots = face_slot_count();
    let face_rows = (face_slots + 8).div_ceil(FACE_ATLAS_COLUMNS);
    let face_width = FACE_ATLAS_COLUMNS * FACE_FRAME_SIZE;
    let face_height = face_rows * FACE_FRAME_SIZE;
    let mut face_pixels = vec![0_u8; (face_width * face_height * 4) as usize];
    for expression in formiga_art::ExpressionKind::ALL {
        for eyelids in formiga_art::EyelidPose::ALL {
            for gaze_y in -1_i8..=1 {
                for gaze_x in -1_i8..=1 {
                    let state = FaceRenderState {
                        expression,
                        eyelids,
                        gaze: formiga_art::GazeDirection::new(gaze_x, gaze_y),
                    };
                    let face = CreatureRenderer::render_face_frame(&creature.appearance, state);
                    let slot = face_atlas_slot(state);
                    blit_atlas_frame(
                        &mut face_pixels,
                        face_width,
                        slot % FACE_ATLAS_COLUMNS * FACE_FRAME_SIZE,
                        slot / FACE_ATLAS_COLUMNS * FACE_FRAME_SIZE,
                        FACE_FRAME_SIZE,
                        &face.rgba_bytes(),
                    );
                }
            }
        }
    }
    for variant in 0..8_u8 {
        let trinket = CreatureRenderer::render_trinket(&creature.appearance, variant);
        let slot = trinket_atlas_slot(variant);
        blit_atlas_frame(
            &mut face_pixels,
            face_width,
            slot % FACE_ATLAS_COLUMNS * FACE_FRAME_SIZE,
            slot / FACE_ATLAS_COLUMNS * FACE_FRAME_SIZE,
            FACE_FRAME_SIZE,
            &trinket.rgba_bytes(),
        );
    }
    AtlasPixels {
        body_width,
        body_height,
        body_pixels,
        face_width,
        face_height,
        face_pixels,
        face_anchors,
        silhouette,
    }
}

fn blit_atlas_frame(
    target: &mut [u8],
    width: u32,
    origin_x: u32,
    origin_y: u32,
    frame_size: u32,
    frame: &[u8],
) {
    for y in 0..frame_size {
        let source_start = (y * frame_size * 4) as usize;
        let target_start = ((origin_y + y) * width * 4 + origin_x * 4) as usize;
        target[target_start..target_start + (frame_size * 4) as usize]
            .copy_from_slice(&frame[source_start..source_start + (frame_size * 4) as usize]);
    }
}

fn total_animation_frames() -> u32 {
    BodyClip::baked()
        .map(|clip| u32::from(AnimationSpec::for_clip(clip).frames))
        .sum()
}

fn creature_horizontal_scale(action: ActionKind) -> f32 {
    if action == ActionKind::SqueezeWindow {
        0.72
    } else {
        1.0
    }
}

fn atlas_slot(clip: impl Into<BodyClip>, frame: u8) -> u32 {
    let clip = clip.into().body();
    let clip_offset: u32 = BodyClip::baked()
        .take_while(|candidate| *candidate != clip)
        .map(|candidate| u32::from(AnimationSpec::for_clip(candidate).frames))
        .sum();
    clip_offset + u32::from(frame)
}

fn face_slot_count() -> u32 {
    formiga_art::ExpressionKind::ALL.len() as u32 * formiga_art::EyelidPose::ALL.len() as u32 * 9
}

fn face_atlas_slot(state: FaceRenderState) -> u32 {
    state.expression.index() * 27 + state.eyelids.index() * 9 + state.gaze.index()
}

/// Which frame of a trinket to show. Presentation is four frames at 2fps, so a keepsake rests for
/// half a second and twinkles for half a second without any timer of its own.
fn trinket_frame(body_frame: u8) -> u8 {
    if body_frame % 4 >= 2 {
        TRINKET_FRAME_GLINT
    } else {
        TRINKET_FRAME_REST
    }
}

/// The eight slots kept in each creature's face texture. Nothing samples them any more — the
/// overlay and the scrapbook both draw from the colony atlas — but the layout, and the exact
/// per-creature texture budget it produces, are unchanged.
fn trinket_atlas_slot(variant: u8) -> u32 {
    face_slot_count() + u32::from(variant % 8)
}

/// The drawable for a window of `layout` physical pixels drawn at `1 / divisor` resolution.
fn drawable_size(layout: PhysicalSize<u32>, divisor: u32) -> (u32, u32) {
    (
        layout.width.div_ceil(divisor).max(1),
        layout.height.div_ceil(divisor).max(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::{ApplicationKey, DisplayKey, MonitorInfo, Point, World};

    fn window(
        key: u64,
        z_order: u32,
        bounds: DesktopRect,
        application: Option<ApplicationKey>,
    ) -> DesktopWindow {
        DesktopWindow {
            key,
            bounds,
            z_order,
            visible: true,
            minimized: false,
            application,
            application_name: None,
        }
    }

    #[test]
    fn multi_creature_presentation_capacity_and_stall_recovery_are_bounded() {
        assert_eq!(INITIAL_VERTEX_CAPACITY, 4 * 18 + 4 * 6 + 6 + 8 * 6);
        assert_eq!(
            expanded_vertex_capacity(INITIAL_VERTEX_CAPACITY, INITIAL_VERTEX_CAPACITY),
            None
        );
        assert_eq!(
            expanded_vertex_capacity(INITIAL_VERTEX_CAPACITY, INITIAL_VERTEX_CAPACITY + 1),
            Some(256)
        );
        assert!(!surface_stalls_require_recovery(2));
        assert!(surface_stalls_require_recovery(3));
    }

    #[test]
    fn selected_window_only_occludes_where_it_is_not_covered() {
        let selected = ApplicationKey::MacBundleId("example.selected".into());
        let rule = ApplicationOcclusionRule {
            application: selected.clone(),
            display_name: "Selected".into(),
            enabled: true,
        };
        let windows = [
            window(
                1,
                0,
                DesktopRect {
                    x: 50.0,
                    y: 0.0,
                    width: 50.0,
                    height: 100.0,
                },
                None,
            ),
            window(
                2,
                1,
                DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                Some(selected),
            ),
        ];
        let rects = visible_occlusion_rects(
            DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 200.0,
            },
            &windows,
            &[rule],
            false,
        );
        assert!(
            rects
                .iter()
                .any(|rect| rect.contains(Point { x: 25.0, y: 50.0 }))
        );
        assert!(
            !rects
                .iter()
                .any(|rect| rect.contains(Point { x: 75.0, y: 50.0 }))
        );
    }

    #[test]
    fn disabled_rule_does_not_occlude() {
        let application = ApplicationKey::MacBundleId("example.selected".into());
        let windows = [window(
            1,
            0,
            DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            Some(application.clone()),
        )];
        let rule = ApplicationOcclusionRule {
            application,
            display_name: "Selected".into(),
            enabled: false,
        };
        assert!(visible_occlusion_rects(windows[0].bounds, &windows, &[rule], false).is_empty());
    }

    #[test]
    fn fullscreen_window_occludes_by_default_without_an_application_rule() {
        let monitor = DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let windows = [window(1, 0, monitor, None)];
        assert_eq!(
            visible_occlusion_rects(monitor, &windows, &[], true),
            vec![monitor]
        );
    }

    #[test]
    fn fullscreen_occlusion_can_be_disabled() {
        let monitor = DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let windows = [window(1, 0, monitor, None)];
        assert!(visible_occlusion_rects(monitor, &windows, &[], false).is_empty());
    }

    #[test]
    fn maximized_work_area_window_is_not_treated_as_fullscreen() {
        let monitor = DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let windows = [window(
            1,
            0,
            DesktopRect {
                x: 0.0,
                y: 24.0,
                width: 1920.0,
                height: 1016.0,
            },
            None,
        )];
        assert!(visible_occlusion_rects(monitor, &windows, &[], true).is_empty());
    }

    #[test]
    fn fullscreen_window_only_occludes_its_own_monitor() {
        let left = DesktopRect {
            x: -1920.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let right = DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 2560.0,
            height: 1440.0,
        };
        let windows = [window(1, 0, left, None)];
        assert_eq!(
            visible_occlusion_rects(left, &windows, &[], true),
            vec![left]
        );
        assert!(visible_occlusion_rects(right, &windows, &[], true).is_empty());
    }

    #[test]
    fn a_desktop_full_of_chosen_windows_still_hands_the_shader_a_list_it_can_hold() {
        let monitor = DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let chosen = ApplicationKey::MacBundleId("example.chosen".into());
        // Forty tall windows from the chosen application, with one band lying across all of them,
        // so each contributes two visible pieces and the list would run to eighty.
        let mut windows: Vec<_> = (0..40)
            .map(|index| {
                window(
                    index as u64 + 2,
                    10 + index,
                    DesktopRect {
                        x: index as f32 * 40.0,
                        y: 0.0,
                        width: 30.0,
                        height: 1080.0,
                    },
                    Some(chosen.clone()),
                )
            })
            .collect();
        windows.push(window(
            1,
            0,
            DesktopRect {
                x: 0.0,
                y: 500.0,
                width: 1920.0,
                height: 80.0,
            },
            None,
        ));
        let rule = ApplicationOcclusionRule {
            application: chosen,
            display_name: "Chosen".into(),
            enabled: true,
        };
        let rects = visible_occlusion_rects(monitor, &windows, &[rule], false);
        assert_eq!(rects.len(), MAX_OCCLUSION_RECTS);
        // The shader reads a fixed array, and the list is cut to exactly what fits in it.
        assert_eq!(OcclusionUniform::zeroed().rects.len(), MAX_OCCLUSION_RECTS);
        // A crowd of narrow windows is still a crowd of narrow windows: it never adds up to
        // covering the display, which is what would stop the colony being drawn at all.
        assert!(!rects_cover(monitor, &rects));
    }

    #[test]
    fn every_motion_and_readability_choice_bakes_the_same_atlas_at_the_same_cost() {
        let world = World::new(
            [19; 32],
            time::OffsetDateTime::UNIX_EPOCH,
            &formiga_core::DesktopSnapshot::default(),
        );
        let creature = &world.save.creatures[0];
        let choices = [(false, false), (true, false), (false, true), (true, true)];
        let first: Vec<_> = choices
            .iter()
            .map(|&(reduce_motion, outline)| build_atlas_pixels(creature, reduce_motion, outline))
            .collect();
        for (choice, atlas) in choices.iter().zip(&first) {
            let bytes = atlas.body_pixels.len() + atlas.face_pixels.len();
            assert_eq!(bytes, 1_529_856, "{choice:?} costs {bytes} bytes");
            assert_eq!(atlas.face_anchors.len(), total_animation_frames() as usize);
        }
        // Thrown away and baked again, twice over: the same atlas, byte for byte, every time.
        for round in 1..3 {
            for (choice, previous) in choices.iter().zip(&first) {
                let atlas = build_atlas_pixels(creature, choice.0, choice.1);
                assert_eq!(
                    atlas.body_pixels, previous.body_pixels,
                    "{choice:?} {round}"
                );
                assert_eq!(
                    atlas.face_pixels, previous.face_pixels,
                    "{choice:?} {round}"
                );
            }
        }
        // The eight slots the face atlas has always ended with are still there and still the
        // same size, so switching the overlay to the colony sheet changed nothing per creature.
        assert_eq!(trinket_atlas_slot(0), face_slot_count());
        assert_eq!(trinket_atlas_slot(7) + 1, face_slot_count() + 8);
        assert_eq!(trinket_atlas_slot(8), trinket_atlas_slot(0));
    }

    /// Every variant the catalogue has — including the eight the simulation cannot pick yet —
    /// samples a real cell of the colony sheet, on both of its frames.
    #[test]
    fn every_trinket_in_the_catalogue_samples_its_own_cell_of_the_colony_sheet() {
        let mut seen = std::collections::BTreeSet::new();
        for variant in 0..formiga_core::TRINKET_VARIANTS {
            for body_frame in 0..4_u8 {
                let frame = trinket_frame(body_frame);
                let (x, y, width, height) =
                    formiga_art::TrinketAtlasRenderer::cell_rect(variant, frame);
                assert_eq!((width, height), (TRINKET_CELL, TRINKET_CELL));
                assert!(
                    x + width <= TRINKET_ATLAS_WIDTH && y + height <= TRINKET_ATLAS_HEIGHT,
                    "variant {variant} samples outside the sheet"
                );
                seen.insert((variant, x, y));
            }
        }
        // Sixteen variants, each on a rest cell and a glint cell.
        assert_eq!(seen.len(), usize::from(formiga_core::TRINKET_VARIANTS) * 2);
        // The presentation clip rests for half of itself and twinkles for the other half.
        assert_eq!(trinket_frame(0), formiga_art::TRINKET_FRAME_REST);
        assert_eq!(trinket_frame(1), formiga_art::TRINKET_FRAME_REST);
        assert_eq!(trinket_frame(2), formiga_art::TRINKET_FRAME_GLINT);
        assert_eq!(trinket_frame(3), formiga_art::TRINKET_FRAME_GLINT);
    }

    #[test]
    fn layered_atlas_matches_the_baked_budget_per_creature() {
        let desktop = formiga_core::DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: DisplayKey([1; 16]),
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1440.0,
                    height: 900.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 1440.0,
                    height: 836.0,
                },
                scale_factor: 2.0,
                primary: true,
            }],
            ..Default::default()
        };
        let world = World::new([7; 32], time::OffsetDateTime::UNIX_EPOCH, &desktop);
        let started = std::time::Instant::now();
        let atlas = build_atlas_pixels(&world.save.creatures[0], false, false);
        let bake_time = started.elapsed();
        let total_bytes = atlas.body_pixels.len() + atlas.face_pixels.len();
        eprintln!("layered atlas: {total_bytes} bytes, baked in {bake_time:?}");
        // 90 action frames and 34 gesture frames: ten columns by thirteen rows of 48px bodies,
        // plus the unchanged face atlas. Raised deliberately from 1,437,696 bytes in 0.57.1,
        // where the twelfth row was already full.
        assert_eq!(total_animation_frames(), 124);
        assert_eq!(total_bytes, 1_529_856);
        // Tripled in 0.58.0 so the pose vocabulary has somewhere to grow: the budget is what
        // stops a creature costing more than a creature should, not what stops it having poses.
        assert!(total_bytes <= 4_500_000, "atlas uses {total_bytes} bytes");
        assert!(total_bytes * 4 < 6_291_456, "four atlases exceed 6 MiB");
        assert!(total_bytes < atlas.body_pixels.len() * 3);
        // The optional outline is baked into the same atlas: no extra texture, no extra frame,
        // and the same bytes. It touches only pixels the creature itself does not occupy.
        let outlined = build_atlas_pixels(&world.save.creatures[0], false, true);
        assert_eq!(
            outlined.body_pixels.len() + outlined.face_pixels.len(),
            total_bytes
        );
        assert_eq!(outlined.face_anchors, atlas.face_anchors);
        let (mut added, mut changed) = (0, 0);
        for (plain, edged) in atlas
            .body_pixels
            .chunks_exact(4)
            .zip(outlined.body_pixels.chunks_exact(4))
        {
            match (plain[3] > 16, plain == edged) {
                (true, same) => changed += usize::from(!same),
                (false, false) => added += 1,
                _ => {}
            }
        }
        assert_eq!(
            changed, 0,
            "an outline never touches the creature's own pixels"
        );
        assert!(added > 0, "an outline does appear around the creature");
        assert_eq!(atlas.face_anchors.len(), total_animation_frames() as usize);
        assert_eq!(
            atlas_slot(ActionKind::Tossed, 2),
            atlas_slot(ActionKind::Dragged, 2)
        );
        assert_eq!(
            atlas_slot(ActionKind::SqueezeWindow, 2),
            atlas_slot(ActionKind::Traverse, 2)
        );
        // Gestures follow the action clips, and every baked frame owns exactly one slot.
        assert_eq!(atlas_slot(formiga_core::Gesture::Cheer, 0), 90);
        let mut slots = BTreeSet::new();
        for clip in BodyClip::baked() {
            for frame in 0..AnimationSpec::for_clip(clip).frames {
                let slot = atlas_slot(clip, frame);
                assert!(slot < total_animation_frames(), "{clip:?} {frame}");
                assert!(slots.insert(slot), "{clip:?} {frame} shares slot {slot}");
            }
        }
        assert_eq!(slots.len(), total_animation_frames() as usize);
        assert_eq!(creature_horizontal_scale(ActionKind::SqueezeWindow), 0.72);
        assert_eq!(creature_horizontal_scale(ActionKind::Traverse), 1.0);
        if !cfg!(debug_assertions) {
            assert!(
                bake_time < std::time::Duration::from_millis(75),
                "release atlas bake took {bake_time:?}"
            );
        }
    }

    // -------------------------------------------------------------------------------------
    // On-desktop UI: icon bubbles, the right-click menu, and whoever is visiting.
    // -------------------------------------------------------------------------------------

    /// One Retina monitor with a menu bar's worth of inset at the top.
    fn ui_desktop() -> formiga_core::DesktopSnapshot {
        formiga_core::DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: DisplayKey([3; 16]),
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1440.0,
                    height: 900.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 1440.0,
                    height: 836.0,
                },
                scale_factor: 2.0,
                primary: true,
            }],
            ..Default::default()
        }
    }

    /// The single place these tests build a guest, so the shape of `Visitor` only has to be
    /// followed in one spot.
    fn test_guest(creature: Creature) -> formiga_core::Visitor {
        let mut guest =
            formiga_core::Visitor::new(creature, formiga_core::VisitorSource::Wanderer, None);
        guest.on_stage = true;
        guest
    }

    fn ui_world() -> World {
        World::new([11; 32], time::OffsetDateTime::UNIX_EPOCH, &ui_desktop())
    }

    const DRAWABLE: PhysicalSize<u32> = PhysicalSize::new(1440, 900);

    /// A quad's pixel rectangle, back out of clip space: left, top, right, bottom.
    fn quad_pixels(quad: &[Vertex]) -> (f32, f32, f32, f32) {
        let to_x = |value: f32| (value + 1.0) / 2.0 * DRAWABLE.width as f32;
        let to_y = |value: f32| (1.0 - value) / 2.0 * DRAWABLE.height as f32;
        (
            to_x(quad[0].position[0]),
            to_y(quad[0].position[1]),
            to_x(quad[1].position[0]),
            to_y(quad[5].position[1]),
        )
    }

    /// Positions and UVs make a round trip through clip space, so they are compared at
    /// sub-pixel tolerance rather than bit for bit.
    #[track_caller]
    fn close(actual: f32, expected: f32, what: &str) {
        assert!(
            (actual - expected).abs() < 0.01,
            "{what}: {actual} is not {expected}"
        );
    }

    /// The atlas rect a quad samples, back out of its UVs: x, y, width, height.
    fn quad_sprite(quad: &[Vertex]) -> (f32, f32, f32, f32) {
        let left = quad[0].uv[0] * UI_ATLAS_WIDTH as f32;
        let right = quad[1].uv[0] * UI_ATLAS_WIDTH as f32;
        let top = quad[0].uv[1] * UI_ATLAS_HEIGHT as f32;
        let bottom = quad[5].uv[1] * UI_ATLAS_HEIGHT as f32;
        (left, top.min(bottom), right - left, (bottom - top).abs())
    }

    #[test]
    fn the_crown_of_a_head_is_the_frame_it_draws_not_the_box_it_is_drawn_in() {
        let world = ui_world();
        let creature = &world.save.creatures[0];
        let atlas = build_atlas_pixels(creature, false, false);
        assert_eq!(atlas.silhouette.len(), total_animation_frames() as usize);
        for clip in BodyClip::baked() {
            for frame in 0..AnimationSpec::for_clip(clip).frames {
                let (top, bottom) = atlas.silhouette[atlas_slot(clip, frame) as usize];
                assert!(top < bottom, "{clip:?} frame {frame} draws nothing");
                assert!(u32::from(bottom) <= FRAME_SIZE);
                // A creature never fills its own frame to the very top edge, which is exactly
                // why a bubble hangs off this row rather than off the frame.
                assert!(
                    top > 0,
                    "{clip:?} frame {frame} touches the frame's top edge"
                );
            }
        }
        // Mirroring a frame moves no row, so one silhouette serves a creature facing either way.
        let baked = atlas.silhouette[atlas_slot(ActionKind::Idle, 0) as usize];
        for facing_right in [false, true] {
            let mirrored = CreatureRenderer::render_composited_frame(
                &creature.appearance,
                BodyClip::from(ActionKind::Idle),
                0,
                facing_right,
                false,
                FaceRenderState {
                    expression: formiga_art::ExpressionKind::Neutral,
                    eyelids: formiga_art::EyelidPose::Open,
                    gaze: formiga_art::GazeDirection::new(0, 0),
                },
            );
            let bounds = mirrored.alpha_bounds().expect("the creature is drawn");
            // The composited frame carries the face too, which may reach a row above the body
            // on its own; the crown is never lower than the body's own first row.
            assert!(bounds.1 as u8 <= baked.0, "facing_right {facing_right}");
            assert!(bounds.1 > 0);
        }
    }

    #[test]
    fn a_smaller_creature_keeps_its_bubble_as_close_to_its_head_as_an_adult() {
        let world = ui_world();
        let adult = world.save.creatures[0].clone();
        let mut mini = adult.clone();
        mini.appearance.logical_size = adult.appearance.logical_size / 2;
        let slot = atlas_slot(ActionKind::Idle, 0) as usize;
        let adult_top = build_atlas_pixels(&adult, false, false).silhouette[slot].0;
        let mini_top = build_atlas_pixels(&mini, false, false).silhouette[slot].0;
        assert!(
            mini_top > adult_top,
            "a smaller creature's crown is further down its frame ({mini_top} vs {adult_top})"
        );
        // Both bubbles sit the same distance above their own crown, so neither floats.
        for (name, top) in [("adult", adult_top), ("mini", mini_top)] {
            let scale = 3.0;
            let head_top = 400.0 + f32::from(top) * scale;
            let (_, y) = bubble_origin(700.0, head_top, scale, DRAWABLE);
            assert_eq!(
                y + BUBBLE_ANCHOR.1 as f32 * scale,
                head_top - scale,
                "{name}: the anchor pixel belongs one art pixel above the crown"
            );
        }
    }

    #[test]
    fn a_bubble_is_centred_on_the_head_with_its_tail_a_pixel_clear_of_it() {
        for scale in [2.0_f32, 3.0, 4.0] {
            let (x, y) = bubble_origin(700.0, 400.0, scale, DRAWABLE);
            // Centred: the tail column is the creature's own centre column.
            assert_eq!(x + BUBBLE_ANCHOR.0 as f32 * scale, 700.0);
            // The tail's tip is the cell's row 14, so exactly one art pixel of daylight is left
            // between the tip and the crown.
            let tip_bottom = y + 15.0 * scale;
            assert_eq!(
                400.0 - tip_bottom,
                scale,
                "one art pixel of gap at {scale}x"
            );
            let quad = ui_atlas_quad(
                UiAtlasRenderer::bubble(
                    formiga_core::BubbleIcon::Heart,
                    formiga_core::BubbleGrowth::Full,
                ),
                x,
                y,
                scale,
                false,
                1.0,
                DRAWABLE,
            );
            let (left, top, right, bottom) = quad_pixels(&quad);
            close(right - left, BUBBLE_CELL.0 as f32 * scale, "bubble width");
            close(bottom - top, BUBBLE_CELL.1 as f32 * scale, "bubble height");
            close(left, x, "bubble left");
            close(top, y, "bubble top");
            let sprite = quad_sprite(&quad);
            close(sprite.2, BUBBLE_CELL.0 as f32, "bubble sprite width");
            close(sprite.3, BUBBLE_CELL.1 as f32, "bubble sprite height");
        }
    }

    #[test]
    fn a_bubble_at_the_top_or_the_edge_of_a_display_stays_on_it() {
        let scale = 4.0;
        let width = BUBBLE_CELL.0 as f32 * scale;
        let height = BUBBLE_CELL.1 as f32 * scale;
        // A creature with its head at the very top of the display.
        let (_, y) = bubble_origin(700.0, 6.0, scale, DRAWABLE);
        assert_eq!(y, 0.0);
        assert!(y + height <= DRAWABLE.height as f32);
        // And one pressed against either side.
        let (left, _) = bubble_origin(2.0, 400.0, scale, DRAWABLE);
        assert_eq!(left, 0.0);
        let (right, _) = bubble_origin(DRAWABLE.width as f32 - 2.0, 400.0, scale, DRAWABLE);
        assert_eq!(right, DRAWABLE.width as f32 - width);
        // Every growth step is the same cell, so growing never pushes a clamped bubble off.
        for growth in [
            formiga_core::BubbleGrowth::Small,
            formiga_core::BubbleGrowth::Medium,
            formiga_core::BubbleGrowth::Full,
        ] {
            let rect = UiAtlasRenderer::bubble(formiga_core::BubbleIcon::Snack, growth);
            assert_eq!((rect.width, rect.height), BUBBLE_CELL);
        }
    }

    fn placed_menu(below: bool) -> (MenuLayout, [MenuIcon; 4], MenuPlacement) {
        use crate::creature_menu::{LocalRect, MenuAnchor, MenuTarget, menu_items, place};
        let items = menu_items(MenuTarget::Member, false);
        let layout = MenuLayout::new(&items);
        let scale = 3.0;
        let usable = LocalRect {
            x: 0.0,
            y: 48.0,
            width: DRAWABLE.width as f32,
            height: DRAWABLE.height as f32 - 48.0,
        };
        let head_top = if below { usable.y + 2.0 } else { 500.0 };
        let placement = place(
            &layout,
            MenuAnchor {
                centre_x: 700.0,
                head_top,
                foot_bottom: head_top + 36.0 * scale,
                art_scale: scale,
                grid: 1.0,
                usable,
                monitor_origin: Point { x: 0.0, y: 0.0 },
                scale_factor: 2.0,
            },
        );
        assert_eq!(placement.below, below);
        (layout, items, placement)
    }

    #[test]
    fn an_open_menu_draws_one_frame_one_quad_per_cell_and_one_label_tab() {
        let (layout, items, placement) = placed_menu(false);
        let view = |hovered| MenuView {
            creature_id: 1,
            items: &items,
            layout: &layout,
            placement,
            hovered,
        };
        // Nothing hovered: the frame and its four cells, and no tab.
        assert_eq!(menu_quads(view(None), DRAWABLE).len(), 6 * 5);
        let hovered = menu_quads(view(Some(2)), DRAWABLE);
        assert_eq!(hovered.len(), 6 * 6);

        // The frame quad is the atlas's own four-cell frame, at the placed top-left.
        let expected = UiAtlasRenderer::menu_frame(4).expect("four cells have a frame");
        let sprite = quad_sprite(&hovered[..6]);
        close(sprite.0, expected.x as f32, "frame sprite x");
        close(sprite.1, expected.y as f32, "frame sprite y");
        close(sprite.2, expected.width as f32, "frame sprite width");
        close(sprite.3, expected.height as f32, "frame sprite height");
        let (left, top, right, bottom) = quad_pixels(&hovered[..6]);
        close(left, placement.x, "frame left");
        close(top, placement.y, "frame top");
        close(
            right - left,
            layout.size().0 as f32 * placement.art_scale,
            "frame width",
        );
        close(
            bottom - top,
            formiga_art::MENU_STRIP_HEIGHT as f32 * placement.art_scale,
            "frame height",
        );

        // Each cell quad lands on its own cell, and only the hovered one is the hovered sprite.
        let body = placement.body_rect();
        for index in 0..4 {
            let quad = &hovered[6 * (index + 1)..6 * (index + 2)];
            let cell = layout.cell(index).expect("four cells");
            let (left, top, right, bottom) = quad_pixels(quad);
            close(
                left,
                body.x + cell.x as f32 * placement.art_scale,
                "cell left",
            );
            close(
                top,
                body.y + cell.y as f32 * placement.art_scale,
                "cell top",
            );
            let side = formiga_art::MENU_CELL as f32 * placement.art_scale;
            close(right - left, side, "cell width");
            close(bottom - top, side, "cell height");
            close(
                quad_sprite(quad).0,
                UiAtlasRenderer::menu_icon(items[index], index == 2).x as f32,
                &format!("cell {index} samples the wrong icon"),
            );
        }
    }

    #[test]
    fn the_label_tab_hangs_under_the_hovered_cell_and_inside_the_strip() {
        let (layout, items, placement) = placed_menu(false);
        for hovered in 0..4 {
            let quads = menu_quads(
                MenuView {
                    creature_id: 1,
                    items: &items,
                    layout: &layout,
                    placement,
                    hovered: Some(hovered),
                },
                DRAWABLE,
            );
            let tab = &quads[6 * 5..];
            let (left, top, right, bottom) = quad_pixels(tab);
            let sprite = UiAtlasRenderer::menu_label(items[hovered]);
            close(quad_sprite(tab).0, sprite.x as f32, "tab sprite x");
            close(
                right - left,
                sprite.width as f32 * placement.art_scale,
                "tab width",
            );
            close(
                bottom - top,
                formiga_art::LABEL_TAB_HEIGHT as f32 * placement.art_scale,
                "tab height",
            );
            // Below the strip, never overlapping it, and never hanging off its sides.
            let frame = placement.frame_rect();
            assert!(top >= frame.bottom(), "the tab must clear the strip");
            assert!(left >= frame.x - 0.01 && right <= frame.right() + 0.01);
            // And under its own cell.
            let cell = layout.cell(hovered).expect("four cells");
            let cell_centre = placement.body_rect().x
                + (cell.x as f32 + formiga_art::MENU_CELL as f32 / 2.0) * placement.art_scale;
            assert!(
                (left..=right).contains(&cell_centre),
                "the tab for cell {hovered} is not under it"
            );
        }
    }

    #[test]
    fn a_menu_under_a_creature_flips_only_its_frame() {
        let (layout, items, placement) = placed_menu(true);
        let quads = menu_quads(
            MenuView {
                creature_id: 1,
                items: &items,
                layout: &layout,
                placement,
                hovered: Some(0),
            },
            DRAWABLE,
        );
        // The frame samples the same sprite upside down, which points the notch up instead.
        let frame = &quads[..6];
        assert!(
            frame[0].uv[1] > frame[5].uv[1],
            "the frame is not flipped, so the notch still points down"
        );
        close(
            quad_sprite(frame).0,
            UiAtlasRenderer::menu_frame(4).expect("frame").x as f32,
            "flipped frame sprite x",
        );
        // The cells and the tab are not flipped, and the cells start a notch further down.
        for index in 1..6 {
            let quad = &quads[6 * index..6 * (index + 1)];
            assert!(quad[0].uv[1] < quad[5].uv[1], "quad {index} is upside down");
        }
        let cells_top = quad_pixels(&quads[6..12]).1;
        let strip_top = quad_pixels(frame).1;
        close(
            cells_top - strip_top,
            (formiga_art::MENU_NOTCH_HEIGHT + 2) as f32 * placement.art_scale,
            "the cells start a notch and a border below the flipped strip",
        );
    }

    #[test]
    fn the_overlay_draws_whoever_is_visiting_and_forgets_it_the_moment_it_leaves() {
        let mut world = ui_world();
        let mut guest =
            World::preview_adult([42; 32], time::OffsetDateTime::UNIX_EPOCH, &ui_desktop());
        guest.state.surface.monitor_id = 1;
        guest.state.arrival_delay_secs = 0.0;
        let guest_id = guest.id;
        world.save.visitors.guest = Some(test_guest(guest));
        let drawn = |save: &SaveFile| -> Vec<CreatureId> {
            drawn_on_monitor(save, 1, false)
                .iter()
                .map(|creature| creature.id)
                .collect()
        };
        let with_guest = drawn(&world.save);
        assert!(
            with_guest.contains(&guest_id),
            "a guest on stage is drawn like anyone else"
        );
        assert_eq!(with_guest.last(), Some(&guest_id), "and drawn last, on top");

        // The sprite cache keeps exactly whoever is on that list, so stepping back inside is
        // enough to evict the guest's atlas — this is the overlay's own `retain`, verbatim.
        let mut sprites: BTreeSet<CreatureId> = with_guest.iter().copied().collect();
        world
            .save
            .visitors
            .guest
            .as_mut()
            .expect("a guest")
            .on_stage = false;
        let without_guest = drawn(&world.save);
        assert!(!without_guest.contains(&guest_id));
        sprites.retain(|id| without_guest.contains(id));
        assert!(
            !sprites.contains(&guest_id),
            "the guest's atlas is released"
        );
        assert_eq!(sprites.len(), without_guest.len());

        // And leaving for good is the same again.
        world.save.visitors.guest = None;
        assert_eq!(drawn(&world.save), without_guest);
        // A guest is never mistaken for a colony member.
        assert!(!world.save.creatures.iter().any(|c| c.id == guest_id));
    }

    #[test]
    fn a_fully_covered_monitor_drops_the_guest_too_unless_it_is_being_carried() {
        let mut world = ui_world();
        let mut guest =
            World::preview_adult([43; 32], time::OffsetDateTime::UNIX_EPOCH, &ui_desktop());
        guest.state.surface.monitor_id = 1;
        guest.state.arrival_delay_secs = 0.0;
        let guest_id = guest.id;
        world.save.visitors.guest = Some(test_guest(guest));
        assert!(drawn_on_monitor(&world.save, 1, true).is_empty());
        world
            .save
            .visitors
            .guest
            .as_mut()
            .expect("a guest")
            .creature
            .state
            .action = ActionKind::Dragged;
        assert_eq!(
            drawn_on_monitor(&world.save, 1, true)
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>(),
            vec![guest_id]
        );
    }
}
