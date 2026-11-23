//! The train, on the overlay of the display it stops at. Its frames are baked once when a trip's
//! scene begins and dropped when the scene is over, so an overlay that has never seen a trip holds
//! nothing for one. So are the windows the colony looks out of: one picture per seat, laid over
//! the train's own window wherever the passenger in it is aboard.

use super::*;
use crate::hill::scene::{TRAIN_RISE, TrainPose};
use formiga_art::{
    Canvas, TRAIN_FRAMES, TRAIN_HEIGHT, TRAIN_WIDTH, TRAIN_WINDOWS, TrainLook, TrainRenderer,
    WINDOW_HEIGHT, WINDOW_WIDTH, window_origins,
};
use formiga_core::{Creature, CreatureId};

/// The train on the desktop this frame, and the colours it is painted in.
#[derive(Clone, Copy, Debug)]
pub struct TrainView {
    pub pose: TrainPose,
    pub look: TrainLook,
}

/// What a baked train is: lit or not.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct TrainKey {
    lit: bool,
}

impl TrainKey {
    fn of(view: &TrainView) -> Self {
        Self { lit: view.look.lit }
    }
}

pub(super) struct TrainGpu {
    _texture: wgpu::Texture,
    pub(super) bind_group: wgpu::BindGroup,
    key: TrainKey,
}

/// Who a trip's windows were baked for, and in what light.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct PassengersKey {
    lit: bool,
    seats: [Option<CreatureId>; TRAIN_WINDOWS],
}

/// One window per seat, side by side, each with its passenger looking out.
pub(super) struct PassengersGpu {
    _texture: wgpu::Texture,
    pub(super) bind_group: wgpu::BindGroup,
    key: PassengersKey,
}

impl OverlayRenderer {
    /// The train's frames, baked for this look unless they already are.
    pub(super) fn ensure_train(&mut self, view: &TrainView) {
        let key = TrainKey::of(view);
        if self.train.as_ref().is_some_and(|train| train.key == key) {
            return;
        }
        let strip = TrainRenderer::render_strip(&view.look);
        let (texture, bind_group) = self.canvas_texture("train", &strip);
        self.train = Some(TrainGpu {
            _texture: texture,
            bind_group,
            key,
        });
    }

    /// The windows of this trip's passengers, baked for its seats and its light unless they
    /// already are. `creatures` is the colony the scene stages.
    pub(super) fn ensure_passengers(&mut self, view: &TrainView, creatures: &[Creature]) {
        let key = PassengersKey {
            lit: view.look.lit,
            seats: view.pose.seats,
        };
        if self
            .passengers
            .as_ref()
            .is_some_and(|passengers| passengers.key == key)
        {
            return;
        }
        let mut sheet = Canvas::new(WINDOW_WIDTH * TRAIN_WINDOWS as u32, WINDOW_HEIGHT);
        for (seat, id) in key.seats.iter().enumerate() {
            let Some(creature) = id.and_then(|id| creatures.iter().find(|c| c.id == id)) else {
                continue;
            };
            let window = TrainRenderer::passenger(&view.look, &creature.appearance);
            let left = (seat as u32 * WINDOW_WIDTH) as i32;
            for y in 0..WINDOW_HEIGHT as i32 {
                for x in 0..WINDOW_WIDTH as i32 {
                    sheet.set(left + x, y, window.get(x, y));
                }
            }
        }
        let (texture, bind_group) = self.canvas_texture("train passengers", &sheet);
        self.passengers = Some(PassengersGpu {
            _texture: texture,
            bind_group,
            key,
        });
    }

    /// A texture holding `canvas`, and the bindings to draw from it.
    fn canvas_texture(&self, label: &str, canvas: &Canvas) -> (wgpu::Texture, wgpu::BindGroup) {
        let (width, height) = (canvas.width(), canvas.height());
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
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
            &canvas.rgba_bytes(),
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
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        (texture, bind_group)
    }

    /// The train's top left on this display, in physical pixels, snapped as the train is drawn.
    fn train_origin(&self, pose: TrainPose, unit: f32) -> (f32, f32) {
        let left = self.snap((pose.left - self.monitor.bounds.x) * self.monitor.scale_factor);
        let ground = self.snap((pose.ground - self.monitor.bounds.y) * self.monitor.scale_factor);
        (left, ground - TRAIN_RISE as f32 * unit)
    }

    /// A quad for every window someone is looking out of just now, over the train's own window,
    /// moving with the train.
    pub(super) fn passenger_vertices(&self, pose: TrainPose, display_scale: u8) -> Vec<Vertex> {
        let unit = f32::from(display_scale);
        let (left_px, top_px) = self.train_origin(pose, unit);
        let x = |px: f32| px / self.layout.width as f32 * 2.0 - 1.0;
        let y = |px: f32| 1.0 - px / self.layout.height as f32 * 2.0;
        let vertex = |position, uv| Vertex {
            position,
            uv,
            occlusion_enabled: 1.0,
        };
        let mut vertices = Vec::new();
        for (seat, (wx, wy)) in window_origins().into_iter().enumerate() {
            if !pose.seated[seat] || pose.seats[seat].is_none() {
                continue;
            }
            // The art faces right; turned round, each window is where its mirror image is.
            let along = if pose.facing_right {
                wx
            } else {
                TRAIN_WIDTH - wx - WINDOW_WIDTH
            };
            let cell_left = left_px + along as f32 * unit;
            let cell_top = top_px + wy as f32 * unit;
            let seats = TRAIN_WINDOWS as f32;
            let (mut u_left, mut u_right) = (seat as f32 / seats, (seat as f32 + 1.0) / seats);
            if !pose.facing_right {
                std::mem::swap(&mut u_left, &mut u_right);
            }
            let (left, right) = (x(cell_left), x(cell_left + WINDOW_WIDTH as f32 * unit));
            let (top, bottom) = (y(cell_top), y(cell_top + WINDOW_HEIGHT as f32 * unit));
            vertices.extend_from_slice(&[
                vertex([left, top], [u_left, 0.0]),
                vertex([right, top], [u_right, 0.0]),
                vertex([right, bottom], [u_right, 1.0]),
                vertex([left, top], [u_left, 0.0]),
                vertex([right, bottom], [u_right, 1.0]),
                vertex([left, bottom], [u_left, 1.0]),
            ]);
        }
        vertices
    }

    /// One quad: the frame for the moment, its left edge where the scene has it, standing on the
    /// ground creatures stand on, and turned round only if it is to face left.
    pub(super) fn train_vertices(&self, pose: TrainPose, display_scale: u8) -> [Vertex; 6] {
        let unit = f32::from(display_scale);
        let width_px = TRAIN_WIDTH as f32 * unit;
        let height_px = TRAIN_HEIGHT as f32 * unit;
        let (left_px, top_px) = self.train_origin(pose, unit);
        let x = |px: f32| px / self.layout.width as f32 * 2.0 - 1.0;
        let y = |px: f32| 1.0 - px / self.layout.height as f32 * 2.0;
        let frames = f32::from(TRAIN_FRAMES);
        let frame = f32::from(pose.frame % TRAIN_FRAMES);
        let (mut u_left, mut u_right) = (frame / frames, (frame + 1.0) / frames);
        if !pose.facing_right {
            std::mem::swap(&mut u_left, &mut u_right);
        }
        let vertex = |position, uv| Vertex {
            position,
            uv,
            occlusion_enabled: 1.0,
        };
        let (left, right) = (x(left_px), x(left_px + width_px));
        let (top, bottom) = (y(top_px), y(top_px + height_px));
        [
            vertex([left, top], [u_left, 0.0]),
            vertex([right, top], [u_right, 0.0]),
            vertex([right, bottom], [u_right, 1.0]),
            vertex([left, top], [u_left, 0.0]),
            vertex([right, bottom], [u_right, 1.0]),
            vertex([left, bottom], [u_left, 1.0]),
        ]
    }
}
