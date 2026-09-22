use crate::creature_menu::{LocalRect, MenuAnchor, MenuPlacement, SidePlacement};
use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use formiga_art::{
    AnimationSpec, BUBBLE_ANCHOR, BUBBLE_CELL, BodyClip, BodyPresentation,
    COLONY_OBJECT_ATLAS_HEIGHT, COLONY_OBJECT_ATLAS_WIDTH, COLONY_OBJECT_SIZE,
    ColonyObjectRenderer, CreatureRenderer, FACE_FRAME_SIZE, FRAME_SIZE, FaceRenderState,
    FramePlacement, MenuIcon, MenuLayout, MilestoneBubbleRenderer, PixelPoint, PropAnchor,
    ResidentMark, Rgba, SHELTER_SIZE, ShelterRenderer, SpriteRect, TRINKET_ATLAS_HEIGHT,
    TRINKET_ATLAS_WIDTH, TRINKET_CELL, TRINKET_FRAME_GLINT, TRINKET_FRAME_REST, TrinketAnchor,
    TrinketAtlasRenderer, UI_ATLAS_HEIGHT, UI_ATLAS_WIDTH, UiAtlasRenderer, VILLAGE_ATLAS_SIZE,
    VILLAGE_HOUSES, VillageCell,
};
use formiga_core::{
    ActionKind, ApplicationOcclusionRule, ColonyObject, Creature, CreatureId, CursorSnapshot,
    DesktopRect, DesktopWindow, HabitatPolicy, HabitatZoneKind, MonitorInfo, Point, SaveFile,
    ShelterDecorationKind, ShelterGenome, ShelterStyle, SleepNudge, ThoughtBubble,
    accessible_regions, resolved_home_anchor,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;
mod atlas;
mod occlusion;
mod resources;
mod ui;
mod village;
use atlas::*;
pub(crate) use occlusion::monitor_has_fullscreen_window;
use occlusion::*;
use village::*;

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
// Four creatures, a village of four dwellings and the two trees that bookend them, sixteen
// keepsakes hung between the pair, one bubble, and eight belongings.
const INITIAL_VERTEX_CAPACITY: usize = 258;
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
    marks: [Option<ResidentMark>; VILLAGE_HOUSES],
    styles: [ShelterStyle; VILLAGE_HOUSES],
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
    /// Whether it is dark out where the owner is, so the houses glow from inside.
    pub night: bool,
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
    /// A strip opened beside the menu, if any.
    pub side: Option<SideView<'a>>,
}

/// A strip opened beside the menu, already placed by `creature_menu`.
#[derive(Clone, Copy)]
pub struct SideView<'a> {
    pub layout: &'a MenuLayout,
    pub placement: SidePlacement,
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

/// The rope a friend tows a sleeper on: two texels, the rope itself and the shade under it,
/// made the first time one is drawn and kept for the life of the overlay.
struct RopeGpu {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

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
    rope: Option<RopeGpu>,
    tree_vertex_cache_key: Option<TreeVertexCacheKey>,
    tree_vertices: Vec<Vertex>,
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
            rope: None,
            tree_vertex_cache_key: None,
            tree_vertices: Vec::new(),
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
        self.tree_vertex_cache_key = None;
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
            self.ensure_shelter(
                save.home.drawn_shelter(),
                &visible_decorations[..count],
                ResidentMark::for_village(&save.creatures, &save.home.cottage_order),
                save.home.house_style_list(&save.creatures),
            );
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
        // The colony sheet is wanted when somebody is holding a find, and when either tree has
        // any of them hung in it. One texture serves both.
        let tree_keepsakes = shelter_visible && !save.companion.scrapbook.is_empty();
        if tree_keepsakes
            || visible
                .iter()
                .any(|creature| creature.state.action == ActionKind::PresentDiscovery)
        {
            self.ensure_trinket_atlas(save);
        }
        let object_vertices = if !shelter_visible
            || (save.objects.objects.is_empty() && !save.home.has_ground_items())
        {
            Vec::new()
        } else {
            self.ensure_colony_object_atlas(save.colony_seed);
            self.cached_colony_object_vertices(save).to_vec()
        };
        let village_vertices = if shelter_visible {
            self.village_vertices(save, ui.night)
        } else {
            Vec::new()
        };
        let tree_vertices = if tree_keepsakes {
            self.cached_tree_vertices(save).to_vec()
        } else {
            Vec::new()
        };
        let mut vertices = Vec::with_capacity(
            object_vertices.len()
                + visible.len() * 18
                + village_vertices.len()
                + tree_vertices.len()
                + usize::from(bubble_creature.is_some()) * 6,
        );
        let mut creature_draws = Vec::with_capacity(visible.len());
        // The village first, then what the colony keeps in the two yards over the top of it:
        // belongings stand on the ground in front of a trunk rather than behind it.
        vertices.extend_from_slice(&village_vertices);
        let shelter_vertex_count = village_vertices.len();
        let object_vertex_start = vertices.len();
        vertices.extend_from_slice(&object_vertices);
        let object_vertex_count = object_vertices.len();
        let tree_vertex_start = vertices.len();
        vertices.extend_from_slice(&tree_vertices);
        let tree_vertex_count = tree_vertices.len();
        // A rope for every sleeper a friend is towing, behind the two of them so each end
        // disappears into whoever is holding it.
        let rope_start = vertices.len();
        for creature in &visible {
            if let Some(SleepNudge::Towed { by }) = creature.state.nudge
                && let Some(tower) = visible.iter().find(|other| other.id == by)
            {
                vertices.extend(self.rope_vertices(tower, creature, save.settings.display_scale));
            }
        }
        let rope_vertex_count = vertices.len() - rope_start;
        if rope_vertex_count > 0 {
            self.ensure_rope();
        }
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
            // Every quad this frame lives in the one buffer, so it is bound once and each draw
            // names its own stretch of it. Handing the pass a fresh slice per draw asked the
            // driver to rebind the same buffer a dozen times a frame for nothing.
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            let draw = |pass: &mut wgpu::RenderPass<'_>,
                        bind_group: &wgpu::BindGroup,
                        start: usize,
                        count: usize| {
                pass.set_bind_group(1, bind_group, &[]);
                pass.draw(start as u32..(start + count) as u32, 0..1);
            };
            if shelter_vertex_count > 0
                && let Some(shelter) = &self.shelter
            {
                draw(&mut pass, &shelter.bind_group, 0, shelter_vertex_count);
            }
            if object_vertex_count > 0
                && let Some(objects) = &self.colony_objects
            {
                draw(
                    &mut pass,
                    &objects.bind_group,
                    object_vertex_start,
                    object_vertex_count,
                );
            }
            // Whatever the colony has found, over the tree each one hangs in: one more bind
            // group at most, and the same sheet a companion holding a keepsake samples from.
            if tree_vertex_count > 0
                && let Some(trinkets) = &self.trinkets
            {
                draw(
                    &mut pass,
                    &trinkets.bind_group,
                    tree_vertex_start,
                    tree_vertex_count,
                );
            }
            if rope_vertex_count > 0
                && let Some(rope) = &self.rope
            {
                draw(&mut pass, &rope.bind_group, rope_start, rope_vertex_count);
            }
            for (creature_id, start, has_trinket) in creature_draws {
                if let Some(sprite) = self.sprites.get(&creature_id) {
                    draw(&mut pass, &sprite.body_bind_group, start, 6);
                    draw(&mut pass, &sprite.face_bind_group, start + 6, 6);
                    if has_trinket && let Some(trinkets) = &self.trinkets {
                        draw(&mut pass, &trinkets.bind_group, start + 12, 6);
                    }
                }
            }
            if bubble_vertex_count > 0
                && let Some(bubble) = &self.bubble
            {
                draw(
                    &mut pass,
                    &bubble.bind_group,
                    bubble_start,
                    bubble_vertex_count,
                );
            }
            // Every icon bubble and every part of the open menu comes out of one texture, so the
            // whole on-desktop UI is a single extra bind group and a single extra draw, last and
            // therefore on top of the colony it is talking about.
            if ui_vertex_count > 0
                && let Some(atlas) = &self.ui_atlas
            {
                draw(&mut pass, &atlas.bind_group, ui_start, ui_vertex_count);
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
        let cottages = formiga_core::colony_cottage_list(&save.creatures);
        let object_visible = shelter_visible
            && ((!save.objects.objects.is_empty()
                && formiga_core::home_object_positions(
                    &save.home,
                    cottages.as_slice(),
                    std::slice::from_ref(&self.monitor),
                    &save.settings.habitat,
                    save.settings.display_scale,
                )
                .iter()
                .any(Option::is_some))
                || save.home.has_ground_items());
        creature_visible || shelter_visible || object_visible
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
        // gesture, a celebration or a habit shows its own baked clip in place of the action's,
        // and a twirl or a turn round flips the way it faces while it lasts.
        let BodyPresentation {
            clip,
            frame,
            facing_right,
        } = BodyPresentation::for_creature(creature);
        let slot = atlas_slot(clip, frame);
        let column = slot % ATLAS_COLUMNS;
        let row = slot / ATLAS_COLUMNS;
        let mut u_left = column as f32 * FRAME_SIZE as f32 / sprite.body_atlas_width as f32;
        let mut u_right = (column + 1) as f32 * FRAME_SIZE as f32 / sprite.body_atlas_width as f32;
        let v_top = row as f32 * FRAME_SIZE as f32 / sprite.body_atlas_height as f32;
        let v_bottom = (row + 1) as f32 * FRAME_SIZE as f32 / sprite.body_atlas_height as f32;
        if !facing_right {
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
        let anchor_x = if facing_right {
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
        if !facing_right {
            source_face_state.gaze.x = -source_face_state.gaze.x;
        }
        let face_slot = face_atlas_slot(source_face_state);
        let face_column = face_slot % FACE_ATLAS_COLUMNS;
        let face_row = face_slot / FACE_ATLAS_COLUMNS;
        let mut face_u_left =
            face_column as f32 * FACE_FRAME_SIZE as f32 / sprite.face_atlas_width as f32;
        let mut face_u_right =
            (face_column + 1) as f32 * FACE_FRAME_SIZE as f32 / sprite.face_atlas_width as f32;
        if !facing_right {
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
            let anchor = PropAnchor::facing(facing_right);
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

/// The drawable for a window of `layout` physical pixels drawn at `1 / divisor` resolution.
fn drawable_size(layout: PhysicalSize<u32>, divisor: u32) -> (u32, u32) {
    (
        layout.width.div_ceil(divisor).max(1),
        layout.height.div_ceil(divisor).max(1),
    )
}

#[cfg(test)]
mod tests;
