//! The train, on the overlay of the display it stops at. Its frames are baked once when a trip's
//! scene begins and dropped when the scene is over, so an overlay that has never seen a trip holds
//! nothing for one.

use super::*;
use crate::hill::scene::{TRAIN_RISE, TrainPose};
use formiga_art::{TRAIN_FRAMES, TRAIN_HEIGHT, TRAIN_WIDTH, TrainLook, TrainRenderer};

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

impl OverlayRenderer {
    /// The train's frames, baked for this look unless they already are.
    pub(super) fn ensure_train(&mut self, view: &TrainView) {
        let key = TrainKey::of(view);
        if self.train.as_ref().is_some_and(|train| train.key == key) {
            return;
        }
        let strip = TrainRenderer::render_strip(&view.look);
        let (width, height) = (strip.width(), TRAIN_HEIGHT);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("train frames"),
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
            &strip.rgba_bytes(),
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
            label: Some("train bindings"),
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
        self.train = Some(TrainGpu {
            _texture: texture,
            bind_group,
            key,
        });
    }

    /// One quad: the frame for the moment, its left edge where the scene has it, standing on the
    /// ground creatures stand on, and turned round only if it is to face left.
    pub(super) fn train_vertices(&self, pose: TrainPose, display_scale: u8) -> [Vertex; 6] {
        let unit = f32::from(display_scale);
        let width_px = TRAIN_WIDTH as f32 * unit;
        let height_px = TRAIN_HEIGHT as f32 * unit;
        let left_px = self.snap((pose.left - self.monitor.bounds.x) * self.monitor.scale_factor);
        let ground = self.snap((pose.ground - self.monitor.bounds.y) * self.monitor.scale_factor);
        let top_px = ground - TRAIN_RISE as f32 * unit;
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
