use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use formiga_art::{
    AnimationSpec, CreatureRenderer, FACE_FRAME_SIZE, FRAME_SIZE, FaceRenderState, FramePlacement,
    PixelPoint, SHELTER_SIZE, ShelterRenderer,
};
use formiga_core::{
    ActionKind, ApplicationOcclusionRule, Creature, CreatureId, CursorSnapshot, DesktopRect,
    DesktopWindow, HabitatPolicy, HabitatZoneKind, MonitorInfo, SaveFile, ShelterGenome,
    accessible_regions, resolved_home_anchor,
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
    body_atlas_width: u32,
    body_atlas_height: u32,
    face_atlas_width: u32,
    face_atlas_height: u32,
    face_anchors: Vec<PixelPoint>,
    resting_baseline: u32,
}

struct ShelterGpu {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    genome: ShelterGenome,
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
    pipeline: wgpu::RenderPipeline,
    zone_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    occlusion_buffer: wgpu::Buffer,
    occlusion_bind_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    vertex_buffer: wgpu::Buffer,
    zone_vertex_buffer: wgpu::Buffer,
    sprites: BTreeMap<CreatureId, SpriteGpu>,
    shelter: Option<ShelterGpu>,
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
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: Vec::new(),
        };
        surface.configure(&device, &config);
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
            contents: bytemuck::cast_slice(&[Vertex::zeroed(); 78]),
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
            pipeline,
            zone_pipeline,
            bind_group_layout,
            occlusion_buffer,
            occlusion_bind_group,
            sampler,
            vertex_buffer,
            zone_vertex_buffer,
            sprites: BTreeMap::new(),
            shelter: None,
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
        })
    }

    pub fn set_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        self.window.set_visible(visible);
        self.visible = visible;
        if visible {
            // Winit rebuilds native styles while showing a window, so restore the requested input
            // mode after every hide/show cycle.
            crate::platform::set_overlay_hittest(&self.window, self.hittest_enabled);
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
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(
        &mut self,
        save: &SaveFile,
        cursor: CursorSnapshot,
        habitat_editor: Option<&HabitatPolicy>,
        windows: &[DesktopWindow],
    ) -> Result<()> {
        self.update_occlusion_cache(save, windows);
        let monitor_fully_occluded = rects_cover(self.monitor.bounds, &self.occlusion_rects);
        let occlusion = self.occlusion_uniform(&self.occlusion_rects);
        let visible: Vec<&Creature> = save
            .creatures
            .iter()
            .filter(|creature| {
                creature.state.surface.monitor_id == self.monitor.id
                    && creature.state.arrival_delay_secs <= 0.0
                    && (!monitor_fully_occluded || creature.state.action == ActionKind::Dragged)
            })
            .collect();
        for creature in &visible {
            self.ensure_sprite(creature, save.settings.reduce_motion);
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
            self.ensure_shelter(save.home.shelter);
        }
        self.sprites
            .retain(|id, _| visible.iter().any(|creature| creature.id == *id));
        let mut vertices =
            Vec::with_capacity(visible.len() * 18 + usize::from(shelter_visible) * 6);
        let mut creature_draws = Vec::with_capacity(visible.len());
        if shelter_visible && let Some(shelter_vertices) = self.shelter_vertices(save) {
            vertices.extend_from_slice(&shelter_vertices);
        }
        let shelter_vertex_count = if shelter_visible { 6 } else { 0 };
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
        if !vertices.is_empty() {
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
            wgpu::CurrentSurfaceTexture::Success(output) => output,
            wgpu::CurrentSurfaceTexture::Suboptimal(output) => output,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
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
            if shelter_vertex_count > 0
                && let Some(shelter) = &self.shelter
            {
                pass.set_bind_group(1, &shelter.bind_group, &[]);
                pass.set_vertex_buffer(
                    0,
                    self.vertex_buffer
                        .slice(..(shelter_vertex_count * std::mem::size_of::<Vertex>()) as u64),
                );
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
                    if has_trinket {
                        let trinket_end = face_end + (6 * std::mem::size_of::<Vertex>()) as u64;
                        pass.set_vertex_buffer(0, self.vertex_buffer.slice(face_end..trinket_end));
                        pass.draw(0..6, 0..1);
                    }
                }
            }
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(output);
        self.has_visual_content = has_visual_content;
        Ok(())
    }

    pub fn needs_redraw(
        &self,
        save: &SaveFile,
        habitat_editor: Option<&HabitatPolicy>,
        windows: &[DesktopWindow],
    ) -> bool {
        if self.has_visual_content || habitat_editor.is_some() {
            return true;
        }
        let occlusion_rects = visible_occlusion_rects(
            self.monitor.bounds,
            windows,
            &save.settings.application_occlusion_rules,
            save.settings.fullscreen_app_occlusion,
        );
        let fully_occluded = rects_cover(self.monitor.bounds, &occlusion_rects);
        let creature_visible = save.creatures.iter().any(|creature| {
            creature.state.surface.monitor_id == self.monitor.id
                && creature.state.arrival_delay_secs <= 0.0
                && (!fully_occluded || creature.state.action == ActionKind::Dragged)
        });
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
        creature_visible || shelter_visible
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
        for (target, rect) in uniform.rects.iter_mut().zip(rects.iter()) {
            *target = [
                (rect.x - self.monitor.bounds.x) * self.monitor.scale_factor,
                (rect.y - self.monitor.bounds.y) * self.monitor.scale_factor,
                (rect.right() - self.monitor.bounds.x) * self.monitor.scale_factor,
                (rect.bottom() - self.monitor.bounds.y) * self.monitor.scale_factor,
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
        let left_px = (rect.x - self.monitor.bounds.x) * self.monitor.scale_factor;
        let right_px = (rect.right() - self.monitor.bounds.x) * self.monitor.scale_factor;
        let top_px = (rect.y - self.monitor.bounds.y) * self.monitor.scale_factor;
        let bottom_px = (rect.bottom() - self.monitor.bounds.y) * self.monitor.scale_factor;
        let left = left_px / self.config.width as f32 * 2.0 - 1.0;
        let right = right_px / self.config.width as f32 * 2.0 - 1.0;
        let top = 1.0 - top_px / self.config.height as f32 * 2.0;
        let bottom = 1.0 - bottom_px / self.config.height as f32 * 2.0;
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

    fn ensure_sprite(&mut self, creature: &Creature, reduce_motion: bool) {
        let requires_bake = self
            .sprites
            .get(&creature.id)
            .is_none_or(|sprite| sprite.reduce_motion != reduce_motion);
        if requires_bake {
            let atlas = build_atlas_pixels(creature, reduce_motion);
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
                    body_atlas_width: atlas.body_width,
                    body_atlas_height: atlas.body_height,
                    face_atlas_width: atlas.face_width,
                    face_atlas_height: atlas.face_height,
                    face_anchors: atlas.face_anchors,
                    resting_baseline: CreatureRenderer::resting_baseline(
                        &creature.appearance,
                        reduce_motion,
                    ),
                },
            );
        }
    }

    fn ensure_shelter(&mut self, genome: ShelterGenome) {
        if self
            .shelter
            .as_ref()
            .is_some_and(|shelter| shelter.genome == genome)
        {
            return;
        }
        let pixels = ShelterRenderer::render(&genome).rgba_bytes();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("procedural colony shelter"),
            size: wgpu::Extent3d {
                width: SHELTER_SIZE,
                height: SHELTER_SIZE,
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
                bytes_per_row: Some(SHELTER_SIZE * 4),
                rows_per_image: Some(SHELTER_SIZE),
            },
            wgpu::Extent3d {
                width: SHELTER_SIZE,
                height: SHELTER_SIZE,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("procedural shelter bind group"),
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
        });
    }

    fn shelter_vertices(&self, save: &SaveFile) -> Option<[Vertex; 6]> {
        let anchor = resolved_home_anchor(
            &save.home,
            &self.monitor,
            save.settings.display_scale,
            &save.settings.habitat,
        )?;
        let size = SHELTER_SIZE as f32 * f32::from(save.settings.display_scale);
        let local_x = (anchor.x - self.monitor.bounds.x) * self.monitor.scale_factor;
        let local_y = (anchor.y - self.monitor.bounds.y) * self.monitor.scale_factor;
        let left = (local_x - size / 2.0) / self.config.width as f32 * 2.0 - 1.0;
        let right = (local_x + size / 2.0) / self.config.width as f32 * 2.0 - 1.0;
        let top = 1.0 - (local_y - size) / self.config.height as f32 * 2.0;
        let bottom = 1.0 - local_y / self.config.height as f32 * 2.0;
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
        let local_x =
            (creature.state.position.x - self.monitor.bounds.x) * self.monitor.scale_factor;
        let contact_y =
            (creature.state.position.y - self.monitor.bounds.y) * self.monitor.scale_factor;
        let placement = FramePlacement::for_action(creature.state.action, sprite.resting_baseline);
        let frame_top = contact_y + placement.origin_y as f32 * display_scale as f32;
        let frame_bottom = frame_top + sprite_size;
        let left = (local_x - sprite_size / 2.0) / self.config.width as f32 * 2.0 - 1.0;
        let right = (local_x + sprite_size / 2.0) / self.config.width as f32 * 2.0 - 1.0;
        let top = 1.0 - frame_top / self.config.height as f32 * 2.0;
        let bottom = 1.0 - frame_bottom / self.config.height as f32 * 2.0;
        let spec = AnimationSpec::for_action(creature.state.action);
        let frame = spec.frame_at(creature.state.action_elapsed);
        let slot = atlas_slot(creature.state.action, frame);
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
        let face_center_x = local_x - sprite_size / 2.0 + anchor_x * display_scale as f32;
        let face_center_y = frame_top + anchor.y as f32 * display_scale as f32;
        let face_size = FACE_FRAME_SIZE as f32 * display_scale as f32;
        let face_left = (face_center_x - face_size / 2.0) / self.config.width as f32 * 2.0 - 1.0;
        let face_right = (face_center_x + face_size / 2.0) / self.config.width as f32 * 2.0 - 1.0;
        let face_top = 1.0 - (face_center_y - face_size / 2.0) / self.config.height as f32 * 2.0;
        let face_bottom = 1.0 - (face_center_y + face_size / 2.0) / self.config.height as f32 * 2.0;
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
            let trinket_slot = trinket_atlas_slot(creature.state.activity_variant);
            let trinket_column = trinket_slot % FACE_ATLAS_COLUMNS;
            let trinket_row = trinket_slot / FACE_ATLAS_COLUMNS;
            let u_left =
                trinket_column as f32 * FACE_FRAME_SIZE as f32 / sprite.face_atlas_width as f32;
            let u_right = (trinket_column + 1) as f32 * FACE_FRAME_SIZE as f32
                / sprite.face_atlas_width as f32;
            let v_top =
                trinket_row as f32 * FACE_FRAME_SIZE as f32 / sprite.face_atlas_height as f32;
            let v_bottom =
                (trinket_row + 1) as f32 * FACE_FRAME_SIZE as f32 / sprite.face_atlas_height as f32;
            let center_y = face_center_y - 14.0 * display_scale as f32;
            let left = (face_center_x - face_size / 2.0) / self.config.width as f32 * 2.0 - 1.0;
            let right = (face_center_x + face_size / 2.0) / self.config.width as f32 * 2.0 - 1.0;
            let top = 1.0 - (center_y - face_size / 2.0) / self.config.height as f32 * 2.0;
            let bottom = 1.0 - (center_y + face_size / 2.0) / self.config.height as f32 * 2.0;
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

struct AtlasPixels {
    body_width: u32,
    body_height: u32,
    body_pixels: Vec<u8>,
    face_width: u32,
    face_height: u32,
    face_pixels: Vec<u8>,
    face_anchors: Vec<PixelPoint>,
}

fn build_atlas_pixels(creature: &Creature, reduce_motion: bool) -> AtlasPixels {
    let body_slots = total_animation_frames();
    let body_rows = body_slots.div_ceil(ATLAS_COLUMNS);
    let body_width = ATLAS_COLUMNS * FRAME_SIZE;
    let body_height = body_rows * FRAME_SIZE;
    let mut body_pixels = vec![0_u8; (body_width * body_height * 4) as usize];
    let mut face_anchors = vec![PixelPoint::default(); body_slots as usize];
    for action in ActionKind::BODY_CLIPS {
        let spec = AnimationSpec::for_action(action);
        for frame in 0..spec.frames {
            let rendered = CreatureRenderer::render_body_frame(
                &creature.appearance,
                action,
                frame,
                reduce_motion,
            );
            let slot = atlas_slot(action, frame);
            face_anchors[slot as usize] = rendered.face_anchor;
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
    ActionKind::BODY_CLIPS
        .into_iter()
        .map(|action| u32::from(AnimationSpec::for_action(action).frames))
        .sum()
}

fn atlas_slot(action: ActionKind, frame: u8) -> u32 {
    let action = AnimationSpec::body_action(action);
    let action_offset: u32 = ActionKind::BODY_CLIPS
        .into_iter()
        .take_while(|candidate| *candidate != action)
        .map(|candidate| u32::from(AnimationSpec::for_action(candidate).frames))
        .sum();
    action_offset + u32::from(frame)
}

fn face_slot_count() -> u32 {
    formiga_art::ExpressionKind::ALL.len() as u32 * formiga_art::EyelidPose::ALL.len() as u32 * 9
}

fn face_atlas_slot(state: FaceRenderState) -> u32 {
    state.expression.index() * 27 + state.eyelids.index() * 9 + state.gaze.index()
}

fn trinket_atlas_slot(variant: u8) -> u32 {
    face_slot_count() + u32::from(variant % 8)
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
        let atlas = build_atlas_pixels(&world.save.creatures[0], false);
        let bake_time = started.elapsed();
        let total_bytes = atlas.body_pixels.len() + atlas.face_pixels.len();
        eprintln!("layered atlas: {total_bytes} bytes, baked in {bake_time:?}");
        assert_eq!(total_animation_frames(), 90);
        assert_eq!(total_bytes, 1_161_216);
        assert!(total_bytes <= 1_200_000, "atlas uses {total_bytes} bytes");
        assert!(total_bytes * 4 < 4_718_592, "four atlases exceed 4.5 MiB");
        assert!(total_bytes < atlas.body_pixels.len() * 3);
        assert_eq!(atlas.face_anchors.len(), total_animation_frames() as usize);
        assert_eq!(
            atlas_slot(ActionKind::Tossed, 2),
            atlas_slot(ActionKind::Dragged, 2)
        );
        if !cfg!(debug_assertions) {
            assert!(
                bake_time < std::time::Duration::from_millis(75),
                "release atlas bake took {bake_time:?}"
            );
        }
    }
}
