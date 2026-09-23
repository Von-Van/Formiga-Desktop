//! The GPU resources an overlay creates on demand and keeps: creature atlases, the village and
//! belongings textures, the rope, the trinket sheet, thought bubbles and the interface atlas.
use super::*;

impl OverlayRenderer {
    pub(super) fn ensure_sprite(&mut self, creature: &Creature, save: &SaveFile) {
        let reduce_motion = save.settings.reduce_motion;
        let outline = save.companion.appearance.sprite_outline;
        // What it is wearing is baked into every frame, so choosing something else, or taking it
        // off, bakes the atlas again. Nothing else about the choice is looked at frame to frame.
        let requires_bake = self.sprites.get(&creature.id).is_none_or(|sprite| {
            sprite.reduce_motion != reduce_motion
                || sprite.outline != outline
                || sprite.dress.map(|dress| dress.accessory) != creature.accessory
        });
        if requires_bake {
            let dress = creature.accessory.map(|accessory| {
                let members: Vec<formiga_art::Palette> = save
                    .creatures
                    .iter()
                    .map(|member| formiga_art::palette_for(&member.appearance))
                    .collect();
                AccessoryArt::resolve(accessory, save.colony_seed, &members)
            });
            let atlas = build_atlas_pixels(creature, reduce_motion, outline, dress);
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
                    dress,
                },
            );
        }
    }

    pub(super) fn ensure_shelter(&mut self, look: VillageLook) {
        if self
            .shelter
            .as_ref()
            .is_some_and(|shelter| shelter.look == look)
        {
            return;
        }
        let pixels = ShelterRenderer::render_look(&look, true).rgba_bytes();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("procedural colony village"),
            size: wgpu::Extent3d {
                width: VILLAGE_ATLAS_WIDTH,
                height: VILLAGE_ATLAS_HEIGHT,
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
                bytes_per_row: Some(VILLAGE_ATLAS_WIDTH * 4),
                rows_per_image: Some(VILLAGE_ATLAS_HEIGHT),
            },
            wgpu::Extent3d {
                width: VILLAGE_ATLAS_WIDTH,
                height: VILLAGE_ATLAS_HEIGHT,
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
            look,
        });
    }

    pub(super) fn ensure_colony_object_atlas(&mut self, colony_seed: [u8; 32]) {
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

    pub(super) fn ensure_rope(&mut self) {
        if self.rope.is_some() {
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tow rope"),
            size: wgpu::Extent3d {
                width: 2,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let texels: Vec<u8> = ROPE.iter().chain(ROPE_SHADE.iter()).copied().collect();
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &texels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 2,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tow rope bindings"),
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
        self.rope = Some(RopeGpu {
            _texture: texture,
            bind_group,
        });
    }

    pub(super) fn ensure_trinket_atlas(&mut self, save: &SaveFile) {
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

    pub(super) fn ensure_bubble(&mut self, creature_id: CreatureId) {
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

    /// Build the UI atlas the first time a bubble or a menu wants it, and let it go again once
    /// nothing has for a while. It is one 256x80 RGBA upload — 81,920 bytes — so the overlay of a
    /// quiet colony holds nothing for a feature nobody is using.
    pub(super) fn sync_ui_atlas(&mut self, needed: bool) {
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
}
