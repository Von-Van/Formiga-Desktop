use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use formiga_art::{AnimationSpec, CreatureRenderer, FRAME_SIZE};
use formiga_core::{
    ActionKind, ApplicationOcclusionRule, Creature, CreatureId, CursorSnapshot, DesktopRect,
    DesktopWindow, HabitatPolicy, HabitatZoneKind, MonitorInfo, SaveFile, accessible_regions,
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
#[derive(Clone, Copy, Pod, Zeroable)]
struct OcclusionUniform {
    rects: [[f32; 4]; MAX_OCCLUSION_RECTS],
    metadata: [u32; 4],
}

struct SpriteGpu {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    reduce_motion: bool,
    atlas_width: u32,
    atlas_height: u32,
}

const ATLAS_COLUMNS: u32 = 10;
const GAZE_VARIANTS: u32 = 3;

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
}

impl OverlayRenderer {
    pub async fn new(window: Arc<Window>, monitor: MonitorInfo) -> Result<Self> {
        let instance = wgpu::Instance::default();
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
        let alpha_mode = [
            wgpu::CompositeAlphaMode::PostMultiplied,
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::Inherit,
        ]
        .into_iter()
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
            contents: bytemuck::cast_slice(&[Vertex::zeroed(); 24]),
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
        })
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
        let visible: Vec<&Creature> = save
            .creatures
            .iter()
            .filter(|creature| {
                creature.state.surface.monitor_id == self.monitor.id
                    && creature.state.arrival_delay_secs <= 0.0
            })
            .collect();
        for creature in &visible {
            self.ensure_sprite(creature, save.settings.reduce_motion);
        }
        self.sprites
            .retain(|id, _| visible.iter().any(|creature| creature.id == *id));
        let mut vertices = Vec::with_capacity(visible.len() * 6);
        for creature in &visible {
            let sprite = self.sprites.get(&creature.id).expect("sprite atlas exists");
            let gaze = gaze_direction(creature, cursor, save.settings.cursor_reactions);
            vertices.extend_from_slice(&self.vertices_for(
                creature,
                save.settings.display_scale,
                sprite,
                gaze,
            ));
        }
        if !vertices.is_empty() {
            self.queue
                .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        }
        let occlusion = self.occlusion_uniform(windows, &save.settings.application_occlusion_rules);
        self.queue
            .write_buffer(&self.occlusion_buffer, 0, bytemuck::bytes_of(&occlusion));
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
            for (index, creature) in visible.iter().enumerate() {
                if let Some(sprite) = self.sprites.get(&creature.id) {
                    let start = (index * 6 * std::mem::size_of::<Vertex>()) as u64;
                    let end = start + (6 * std::mem::size_of::<Vertex>()) as u64;
                    pass.set_bind_group(1, &sprite.bind_group, &[]);
                    pass.set_vertex_buffer(0, self.vertex_buffer.slice(start..end));
                    pass.draw(0..6, 0..1);
                }
            }
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(output);
        Ok(())
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

    fn occlusion_uniform(
        &self,
        windows: &[DesktopWindow],
        rules: &[ApplicationOcclusionRule],
    ) -> OcclusionUniform {
        let rects = visible_occlusion_rects(self.monitor.bounds, windows, rules);
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
            let (atlas_width, atlas_height, pixels) = build_atlas_pixels(creature, reduce_motion);
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("procedural creature atlas"),
                size: wgpu::Extent3d {
                    width: atlas_width,
                    height: atlas_height,
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
                    bytes_per_row: Some(atlas_width * 4),
                    rows_per_image: Some(atlas_height),
                },
                wgpu::Extent3d {
                    width: atlas_width,
                    height: atlas_height,
                    depth_or_array_layers: 1,
                },
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("creature frame bindings"),
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
            self.sprites.insert(
                creature.id,
                SpriteGpu {
                    _texture: texture,
                    bind_group,
                    reduce_motion,
                    atlas_width,
                    atlas_height,
                },
            );
        }
    }

    fn vertices_for(
        &self,
        creature: &Creature,
        display_scale: u8,
        sprite: &SpriteGpu,
        gaze_x: i8,
    ) -> [Vertex; 6] {
        // Creature scale is expressed in physical pixels. Applying the monitor scale factor a
        // second time made a 3x creature twice the intended size on Retina displays.
        let sprite_size = FRAME_SIZE as f32 * display_scale as f32;
        let local_x =
            (creature.state.position.x - self.monitor.bounds.x) * self.monitor.scale_factor;
        let local_y =
            (creature.state.position.y - self.monitor.bounds.y) * self.monitor.scale_factor;
        let left = (local_x - sprite_size / 2.0) / self.config.width as f32 * 2.0 - 1.0;
        let right = (local_x + sprite_size / 2.0) / self.config.width as f32 * 2.0 - 1.0;
        let top = 1.0 - (local_y - sprite_size) / self.config.height as f32 * 2.0;
        let bottom = 1.0 - local_y / self.config.height as f32 * 2.0;
        let spec = AnimationSpec::for_action(creature.state.action);
        let frame = ((creature.state.action_elapsed * spec.fps as f32) as u8) % spec.frames;
        let slot = atlas_slot(creature.state.action, frame, gaze_x);
        let column = slot % ATLAS_COLUMNS;
        let row = slot / ATLAS_COLUMNS;
        let mut u_left = column as f32 * FRAME_SIZE as f32 / sprite.atlas_width as f32;
        let mut u_right = (column + 1) as f32 * FRAME_SIZE as f32 / sprite.atlas_width as f32;
        let v_top = row as f32 * FRAME_SIZE as f32 / sprite.atlas_height as f32;
        let v_bottom = (row + 1) as f32 * FRAME_SIZE as f32 / sprite.atlas_height as f32;
        if !creature.state.facing_right {
            std::mem::swap(&mut u_left, &mut u_right);
        }
        let occlusion_enabled = (creature.state.action != ActionKind::Dragged) as u8 as f32;
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
    }
}

pub(crate) fn visible_occlusion_rects(
    monitor: DesktopRect,
    windows: &[DesktopWindow],
    rules: &[ApplicationOcclusionRule],
) -> Vec<DesktopRect> {
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

fn build_atlas_pixels(creature: &Creature, reduce_motion: bool) -> (u32, u32, Vec<u8>) {
    let total_slots = total_animation_frames() * GAZE_VARIANTS;
    let rows = total_slots.div_ceil(ATLAS_COLUMNS);
    let width = ATLAS_COLUMNS * FRAME_SIZE;
    let height = rows * FRAME_SIZE;
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    for gaze_x in -1_i8..=1 {
        for action in ActionKind::ALL {
            let spec = AnimationSpec::for_action(action);
            for frame in 0..spec.frames {
                let canvas = CreatureRenderer::render_frame_with_options(
                    &creature.appearance,
                    action,
                    frame,
                    true,
                    reduce_motion,
                    gaze_x,
                );
                let slot = atlas_slot(action, frame, gaze_x);
                blit_atlas_frame(
                    &mut pixels,
                    width,
                    slot % ATLAS_COLUMNS * FRAME_SIZE,
                    slot / ATLAS_COLUMNS * FRAME_SIZE,
                    &canvas.rgba_bytes(),
                );
            }
        }
    }
    (width, height, pixels)
}

fn blit_atlas_frame(target: &mut [u8], width: u32, origin_x: u32, origin_y: u32, frame: &[u8]) {
    for y in 0..FRAME_SIZE {
        let source_start = (y * FRAME_SIZE * 4) as usize;
        let target_start = ((origin_y + y) * width * 4 + origin_x * 4) as usize;
        target[target_start..target_start + (FRAME_SIZE * 4) as usize]
            .copy_from_slice(&frame[source_start..source_start + (FRAME_SIZE * 4) as usize]);
    }
}

fn total_animation_frames() -> u32 {
    ActionKind::ALL
        .into_iter()
        .map(|action| u32::from(AnimationSpec::for_action(action).frames))
        .sum()
}

fn atlas_slot(action: ActionKind, frame: u8, gaze_x: i8) -> u32 {
    let action_offset: u32 = ActionKind::ALL
        .into_iter()
        .take_while(|candidate| *candidate != action)
        .map(|candidate| u32::from(AnimationSpec::for_action(candidate).frames))
        .sum();
    let gaze_group = u32::from((gaze_x.clamp(-1, 1) + 1) as u8);
    gaze_group * total_animation_frames() + action_offset + u32::from(frame)
}

fn gaze_direction(creature: &Creature, cursor: CursorSnapshot, enabled: bool) -> i8 {
    if !enabled || !cursor.available || creature.state.position.distance(cursor.position) > 240.0 {
        return 0;
    }
    let delta = cursor.position.x - creature.state.position.x;
    if delta.abs() < 8.0 {
        0
    } else if delta > 0.0 {
        1
    } else {
        -1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::{ApplicationKey, Point};

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
        assert!(visible_occlusion_rects(windows[0].bounds, &windows, &[rule]).is_empty());
    }
}
