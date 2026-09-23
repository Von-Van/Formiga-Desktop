//! The village's geometry: houses, trees, the keepsakes hung in them, belongings on the ground, and
//! the tow rope.
use super::*;

/// The rope's colour and the darker line under it, which reads as its round underside.
pub(super) const ROPE: [u8; 4] = [196, 152, 98, 255];
pub(super) const ROPE_SHADE: [u8; 4] = [104, 72, 50, 255];

/// Where to sample each of the rope texture's two texels.
pub(super) const ROPE_UV: [f32; 2] = [0.25, 0.5];
pub(super) const ROPE_SHADE_UV: [f32; 2] = [0.75, 0.5];

/// The rope between a friend's hand and the sleeper it is towing, as the art pixels it covers, in
/// the overlay's own pixels: sagging a little between its ends, one art pixel thick, with the
/// shade under every pixel of it. `px` is one art pixel's size. Shade comes first, so the rope is
/// laid over it wherever the two meet on a slope.
pub(super) fn rope_pixels(from: (f32, f32), to: (f32, f32), px: f32) -> Vec<((f32, f32), bool)> {
    let px = px.max(1.0);
    let reach = (to.0 - from.0).abs().max((to.1 - from.1).abs());
    let steps = (reach / px).ceil().max(1.0) as usize * 2;
    let sag = (to.0 - from.0).abs() * 0.18;
    let mut cells: Vec<(i32, i32)> = Vec::with_capacity(steps + 1);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = from.0 + (to.0 - from.0) * t;
        let y = from.1 + (to.1 - from.1) * t + sag * 4.0 * t * (1.0 - t);
        let cell = ((x / px).floor() as i32, (y / px).floor() as i32);
        if cells.last() != Some(&cell) {
            cells.push(cell);
        }
    }
    let at = |(column, row): (i32, i32)| (column as f32 * px, row as f32 * px);
    cells
        .iter()
        .map(|&(column, row)| (at((column, row + 1)), true))
        .chain(cells.iter().map(|&cell| (at(cell), false)))
        .collect()
}

/// What the sheet depends on: the colony seed it is derived from, and the colours of everyone it
/// has to stay distinct from.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct TrinketAtlasKey {
    pub(super) colony_seed: [u8; 32],
    pub(super) members: Vec<Rgba>,
}

impl TrinketAtlasKey {
    pub(super) fn of(save: &SaveFile) -> Self {
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
pub(super) struct ObjectVertexCacheKey {
    pub(super) objects: Vec<ColonyObject>,
    /// How far along each garden patch has grown, which changes by itself every few hours.
    pub(super) stages: Vec<formiga_core::GardenStage>,
    pub(super) cottages: Vec<formiga_core::DwellingKind>,
    pub(super) home: formiga_core::ColonyHome,
    pub(super) habitat: HabitatPolicy,
    pub(super) monitor_bounds: DesktopRect,
    pub(super) monitor_usable_bounds: DesktopRect,
    pub(super) monitor_scale_factor: f32,
    pub(super) display_scale: u8,
}

/// What the keepsakes hung in the two trees depend on: which kinds the colony has found, and the
/// geometry that decides where the trees themselves stand. Nothing here changes frame to frame,
/// so the quads are built once and reused exactly as the colony's belongings are.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct TreeVertexCacheKey {
    pub(super) found: Vec<u8>,
    pub(super) cottages: Vec<formiga_core::DwellingKind>,
    pub(super) home: formiga_core::ColonyHome,
    pub(super) habitat: HabitatPolicy,
    pub(super) monitor_bounds: DesktopRect,
    pub(super) monitor_usable_bounds: DesktopRect,
    pub(super) monitor_scale_factor: f32,
    pub(super) display_scale: u8,
}

/// Where a lot samples the village atlas, as the top-left texture coordinate of its cell: each
/// house its own cell, by day or lit after dark and with somebody at home or not, and both trees
/// the one tree cell, the inward one drawn from it mirrored.
pub(super) fn village_cell(cell: VillageCell) -> (f32, f32) {
    let (x, y) = ShelterRenderer::village_cell(cell);
    (
        x as f32 / VILLAGE_ATLAS_WIDTH as f32,
        y as f32 / VILLAGE_ATLAS_HEIGHT as f32,
    )
}

/// How far the top of a house leans or settles while its keeper sees to it, in art pixels, as
/// `(across, down)`: a cap wobbling when it is patted, a tent swaying as its flap is retied, a
/// cushion house puffing up as it is plumped. Its foot never moves.
pub(super) fn house_sway(motion: Option<HouseMotion>) -> (f32, f32) {
    let Some(motion) = motion else {
        return (0.0, 0.0);
    };
    let turn = motion.progress * std::f32::consts::TAU;
    let fading = 1.0 - motion.progress;
    match motion.chore {
        BeatKind::InspectCap => ((turn * 3.0).sin() * fading, 0.0),
        BeatKind::AdjustFlap => ((turn * 2.0).sin() * 0.5 * fading, 0.0),
        BeatKind::FluffCushion => (0.0, -(turn * 2.0).sin().abs()),
        _ => (0.0, 0.0),
    }
}

/// Which of the two Zs drifting up out of a sleeping house is where, at `clock` seconds: each
/// rises and drifts sideways over a couple of seconds, half a cycle behind the other, and is
/// gone for the last part of its climb. Offsets are in art pixels from the top of the roof.
pub(super) fn snores(clock: f32) -> impl Iterator<Item = (f32, f32)> {
    const CYCLE: f32 = 2.4;
    (0..2).filter_map(move |index| {
        let t = (clock / CYCLE + index as f32 * 0.5).rem_euclid(1.0);
        (t < 0.8).then_some((4.0 + t * 6.0, -4.0 - t * 14.0))
    })
}

/// Whether a tree at this end of the village samples its cell mirrored. The inward bookend does,
/// so the pair reads as two trees rather than one drawn twice; the anchors come in mirrored
/// pairs, so every keepsake still lands on a cord.
pub(super) fn tree_is_mirrored(end: formiga_core::TreeEnd) -> bool {
    end == formiga_core::TreeEnd::Inward
}

/// Where one keepsake's 16x16 quad is centred, in this display's own pixels, given where the tree
/// it hangs on stands. `scale` is how many display pixels one shelter pixel covers. The anchor is
/// the one `trinket_place` reports, already mirrored if it belongs to the mirrored tree.
pub(super) fn hung_trinket_centre(
    tree_x: f32,
    tree_y: f32,
    anchor: TrinketAnchor,
    scale: f32,
) -> (f32, f32) {
    let cell = SHELTER_SIZE as f32;
    (
        tree_x + (anchor.x as f32 - cell / 2.0) * scale,
        tree_y - (cell - anchor.y as f32) * scale,
    )
}

/// Every kind the colony has found, once each, in catalogue order.
pub(super) fn found_trinkets(save: &SaveFile) -> Vec<u8> {
    let mut found: Vec<u8> = save
        .companion
        .scrapbook
        .iter()
        .map(|record| record.variant)
        .collect();
    found.sort_unstable();
    found.dedup();
    found
}

impl OverlayRenderer {
    /// One sheet per colony, rebuilt only when the colony seed or a member's colours change —
    /// the same caching the colony-object atlas uses.
    /// The rope between a friend's hand, held behind it as it pulls, and the side of the sleeper
    /// it is towing, as quads in clip space.
    pub(super) fn rope_vertices(
        &self,
        tower: &Creature,
        sleeper: &Creature,
        display_scale: u8,
    ) -> Vec<Vertex> {
        let px = f32::from(display_scale.max(1));
        let frame = formiga_art::FRAME_SIZE as f32 * px;
        let local = |point: formiga_core::Point| {
            (
                (point.x - self.monitor.bounds.x) * self.monitor.scale_factor,
                (point.y - self.monitor.bounds.y) * self.monitor.scale_factor,
            )
        };
        let (tower_x, tower_y) = local(tower.state.position);
        let (sleeper_x, sleeper_y) = local(sleeper.state.position);
        let ahead = if tower.state.position.x >= sleeper.state.position.x {
            1.0
        } else {
            -1.0
        };
        let hand = (tower_x - ahead * frame * 0.12, tower_y - frame * 0.34);
        let tied = (sleeper_x + ahead * frame * 0.18, sleeper_y - frame * 0.2);
        let (width, height) = (self.layout.width as f32, self.layout.height as f32);
        let mut vertices = Vec::new();
        for ((x, y), shade) in rope_pixels(hand, tied, px) {
            let uv = if shade { ROPE_SHADE_UV } else { ROPE_UV };
            let (left, right) = (x / width * 2.0 - 1.0, (x + px) / width * 2.0 - 1.0);
            let (top, bottom) = (1.0 - y / height * 2.0, 1.0 - (y + px) / height * 2.0);
            let vertex = |position| Vertex {
                position,
                uv,
                occlusion_enabled: 1.0,
            };
            vertices.extend_from_slice(&[
                vertex([left, top]),
                vertex([right, top]),
                vertex([right, bottom]),
                vertex([left, top]),
                vertex([right, bottom]),
                vertex([left, bottom]),
            ]);
        }
        vertices
    }

    pub(super) fn cached_colony_object_vertices(&mut self, save: &SaveFile) -> &[Vertex] {
        let cottages = formiga_core::colony_cottages(&save.creatures);
        let now = save.maximum_seen_utc;
        let key = ObjectVertexCacheKey {
            objects: save.objects.objects.clone(),
            stages: save
                .home
                .gardens
                .iter()
                .map(|patch| patch.stage(now))
                .collect(),
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
        let places = formiga_core::home_object_positions(
            &save.home,
            &cottages,
            std::slice::from_ref(&self.monitor),
            &save.settings.habitat,
            save.settings.display_scale,
        );
        // A yard is scattered in depth as well as sideways, so the things standing further back
        // are laid down first and whatever is nearest the front of a trunk covers them.
        let mut yard: Vec<(&ColonyObject, formiga_core::Point)> = save
            .objects
            .objects
            .iter()
            .take(formiga_core::MAX_COLONY_OBJECTS)
            .enumerate()
            .filter_map(|(slot, object)| {
                let (monitor_id, point) = places[slot]?;
                (monitor_id == self.monitor.id).then_some((object, point))
            })
            .collect();
        yard.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
        for (object, point) in yard {
            self.object_vertices
                .extend_from_slice(&self.object_vertices_for(
                    ColonyObjectRenderer::object_cell(object.kind),
                    false,
                    point,
                    save.settings.display_scale,
                ));
        }
        // The spots put down and the patches planted on the ground between the houses, the
        // lookout turned to face out over the open desktop.
        let middle = self.monitor.usable_bounds.x + self.monitor.usable_bounds.width / 2.0;
        let ground = formiga_core::home_ground_positions(
            &save.home,
            &cottages,
            std::slice::from_ref(&self.monitor),
            &save.settings.habitat,
            save.settings.display_scale,
        );
        for (item, monitor_id, point) in ground {
            if monitor_id != self.monitor.id {
                continue;
            }
            // A garden is drawn at whatever stage it has grown to by now.
            let stage = match item {
                formiga_core::GroundItem::Garden(kind) => save
                    .home
                    .garden(kind)
                    .map_or(formiga_core::GardenStage::Grown, |patch| patch.stage(now)),
                _ => formiga_core::GardenStage::Grown,
            };
            self.object_vertices
                .extend_from_slice(&self.object_vertices_for(
                    ColonyObjectRenderer::ground_cell(item, stage),
                    ColonyObjectRenderer::ground_mirrored(item, point.x, middle),
                    point,
                    save.settings.display_scale,
                ));
        }
        self.object_vertex_cache_key = Some(key);
        &self.object_vertices
    }

    pub(super) fn object_vertices_for(
        &self,
        cell: u32,
        mirrored: bool,
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
        let (mut u_left, v_top, mut u_right, v_bottom) = ColonyObjectRenderer::cell_uv(cell);
        if mirrored {
            std::mem::swap(&mut u_left, &mut u_right);
        }
        let vertex = |position, uv| Vertex {
            position,
            uv,
            occlusion_enabled: 1.0,
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

    /// One cell of the object sheet centred on `centre`, in this display's own pixels.
    pub(super) fn object_quad_at(
        &self,
        cell: u32,
        (centre_x, centre_y): (f32, f32),
        mirrored: bool,
        display_scale: u8,
        occlusion_enabled: f32,
    ) -> [Vertex; 6] {
        let size = COLONY_OBJECT_SIZE as f32 * f32::from(display_scale);
        let centre_x = self.snap(centre_x);
        let centre_y = self.snap(centre_y);
        let left = (centre_x - size * 0.5) / self.layout.width as f32 * 2.0 - 1.0;
        let right = (centre_x + size * 0.5) / self.layout.width as f32 * 2.0 - 1.0;
        let top = 1.0 - (centre_y - size * 0.5) / self.layout.height as f32 * 2.0;
        let bottom = 1.0 - (centre_y + size * 0.5) / self.layout.height as f32 * 2.0;
        let (mut u_left, v_top, mut u_right, v_bottom) = ColonyObjectRenderer::cell_uv(cell);
        if mirrored {
            std::mem::swap(&mut u_left, &mut u_right);
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

    /// Everything loose about the village this frame, from the colony's object sheet: a leaf
    /// drifting down onto somebody's face or an apple rolling away, a leaf coming off a leaf
    /// house as it is tidied, and the Zs drifting up out of a house somebody is asleep in.
    pub(super) fn village_prop_vertices(
        &self,
        save: &SaveFile,
        scene: VillageScene<'_>,
    ) -> Vec<Vertex> {
        if scene.loose.is_empty() && scene.occupied.is_empty() && scene.motions.is_empty() {
            return Vec::new();
        }
        let display_scale = save.settings.display_scale;
        let unit = f32::from(display_scale);
        let local = |point: formiga_core::Point| {
            (
                (point.x - self.monitor.bounds.x) * self.monitor.scale_factor,
                (point.y - self.monitor.bounds.y) * self.monitor.scale_factor,
            )
        };
        let half = COLONY_OBJECT_SIZE as f32 * unit / 2.0;
        let mut vertices = Vec::new();
        for (prop, at) in scene.loose {
            if !self.monitor.bounds.contains(*at) {
                continue;
            }
            let (x, y) = local(*at);
            // An apple sits on the ground it is rolling along; a leaf is wherever it has got to.
            let y = if *prop == VillageProp::Apple {
                y - half + 4.0 * unit
            } else {
                y
            };
            vertices.extend_from_slice(&self.object_quad_at(
                ColonyObjectRenderer::prop_cell(PropSprite::of(*prop)),
                (x, y),
                false,
                display_scale,
                1.0,
            ));
        }
        let cottages = formiga_core::colony_cottages(&save.creatures);
        let styles = save.home.house_style_list(&save.creatures);
        let house = |slot: usize| {
            formiga_core::home_dwelling_position(
                &save.home,
                slot,
                &cottages,
                std::slice::from_ref(&self.monitor),
                &save.settings.habitat,
                display_scale,
            )
            .filter(|(monitor_id, _)| *monitor_id == self.monitor.id)
            .map(|(_, point)| {
                let roof = formiga_core::house_roof_height(
                    &save.home.shelter,
                    styles[slot.min(styles.len() - 1)],
                    slot == 0,
                );
                let (x, ground) = local(point);
                (x, ground, ground - roof * unit)
            })
        };
        for occupancy in scene.occupied.iter().filter(|house| house.napping) {
            let Some((x, _, roof)) = house(occupancy.slot) else {
                continue;
            };
            for (across, up) in snores(scene.clock) {
                vertices.extend_from_slice(&self.object_quad_at(
                    ColonyObjectRenderer::prop_cell(PropSprite::Snore),
                    (x + across * unit, roof + up * unit),
                    false,
                    display_scale,
                    1.0,
                ));
            }
        }
        for motion in scene
            .motions
            .iter()
            .filter(|motion| motion.chore == BeatKind::TidyLeaves)
        {
            let Some((x, ground, roof)) = house(motion.slot) else {
                continue;
            };
            // One leaf let go of at the eaves, swaying down to the ground.
            let t = ((motion.progress - 0.3) / 0.6).clamp(0.0, 1.0);
            if t <= 0.0 || t >= 1.0 {
                continue;
            }
            let fall = roof + (ground - roof) * t;
            let sway = (t * std::f32::consts::TAU * 1.5).sin() * 4.0 * unit;
            vertices.extend_from_slice(&self.object_quad_at(
                ColonyObjectRenderer::prop_cell(PropSprite::Leaf),
                (x + 12.0 * unit + sway, fall),
                false,
                display_scale,
                1.0,
            ));
        }
        vertices
    }

    /// Every dwelling in the village and the two keepsake trees that bookend it, sampled from
    /// their own cells of the shared atlas. The colony house is always first; companion cottages
    /// follow along the same ground line, and a tree closes each end of it.
    pub(super) fn village_vertices(
        &self,
        save: &SaveFile,
        night: bool,
        scene: VillageScene<'_>,
    ) -> Vec<Vertex> {
        let cottages = formiga_core::colony_cottages(&save.creatures);
        let mut vertices = Vec::with_capacity((cottages.len() + 3) * 6);
        for end in formiga_core::TreeEnd::BOTH {
            if let Some((monitor_id, point)) = formiga_core::home_tree_position(
                &save.home,
                end,
                &cottages,
                std::slice::from_ref(&self.monitor),
                &save.settings.habitat,
                save.settings.display_scale,
            ) && monitor_id == self.monitor.id
            {
                vertices.extend_from_slice(&self.village_cell_vertices(
                    point,
                    village_cell(VillageCell::Tree),
                    tree_is_mirrored(end),
                    save.settings.display_scale,
                    (0.0, 0.0),
                ));
            }
        }
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
            // Somebody at home behind a drawn curtain, and the house answering whatever its
            // keeper is doing to it.
            let occupied = scene.occupied.iter().any(|house| house.slot == slot);
            let motion = scene
                .motions
                .iter()
                .copied()
                .find(|motion| motion.slot == slot);
            vertices.extend_from_slice(&self.village_cell_vertices(
                point,
                village_cell(VillageCell::House {
                    slot,
                    lit: night,
                    occupied,
                }),
                false,
                save.settings.display_scale,
                house_sway(motion),
            ));
        }
        vertices
    }

    /// One quadrant of the village atlas, standing on the ground line at `anchor`. `mirror` swaps
    /// the cell's own left and right edges, which is how the inward tree is drawn from the same
    /// cell as the outward one without a second texture.
    pub(super) fn village_cell_vertices(
        &self,
        anchor: formiga_core::Point,
        (u, v): (f32, f32),
        mirror: bool,
        display_scale: u8,
        (lean, settle): (f32, f32),
    ) -> [Vertex; 6] {
        let unit = f32::from(display_scale);
        let size = SHELTER_SIZE as f32 * unit;
        let local_x = self.snap((anchor.x - self.monitor.bounds.x) * self.monitor.scale_factor);
        let local_y = self.snap((anchor.y - self.monitor.bounds.y) * self.monitor.scale_factor);
        let x = |value: f32| value / self.layout.width as f32 * 2.0 - 1.0;
        let y = |value: f32| 1.0 - value / self.layout.height as f32 * 2.0;
        // The foot of the cell stays where it stands; only its top leans or settles.
        let (top_left, top_right) = (
            x(local_x - size / 2.0 + lean * unit),
            x(local_x + size / 2.0 + lean * unit),
        );
        let (left, right) = (x(local_x - size / 2.0), x(local_x + size / 2.0));
        let top = y(local_y - size + settle * unit);
        let bottom = y(local_y);
        let (cell_u, cell_v) = (
            SHELTER_SIZE as f32 / VILLAGE_ATLAS_WIDTH as f32,
            SHELTER_SIZE as f32 / VILLAGE_ATLAS_HEIGHT as f32,
        );
        let (u_left, u_right) = if mirror {
            (u + cell_u, u)
        } else {
            (u, u + cell_u)
        };
        let vertex = |position, uv| Vertex {
            position,
            uv,
            occlusion_enabled: 1.0,
        };
        [
            vertex([top_left, top], [u_left, v]),
            vertex([top_right, top], [u_right, v]),
            vertex([right, bottom], [u_right, v + cell_v]),
            vertex([top_left, top], [u_left, v]),
            vertex([right, bottom], [u_right, v + cell_v]),
            vertex([left, bottom], [u_left, v + cell_v]),
        ]
    }

    /// Every keepsake the colony has found, one 16x16 quad each on the anchors of whichever tree
    /// it hangs in. Built only when the scrapbook, the colony's houses, the home, the habitat,
    /// this display's geometry or the drawing scale change — the same caching the colony's
    /// belongings use — and never touched again from frame to frame. Both trees are resolved
    /// once here rather than once per keepsake.
    pub(super) fn cached_tree_vertices(&mut self, save: &SaveFile) -> &[Vertex] {
        let cottages = formiga_core::colony_cottages(&save.creatures);
        let key = TreeVertexCacheKey {
            found: found_trinkets(save),
            cottages: cottages.clone(),
            home: save.home.clone(),
            habitat: save.settings.habitat.clone(),
            monitor_bounds: self.monitor.bounds,
            monitor_usable_bounds: self.monitor.usable_bounds,
            monitor_scale_factor: self.monitor.scale_factor,
            display_scale: save.settings.display_scale,
        };
        if self.tree_vertex_cache_key.as_ref() == Some(&key) {
            return &self.tree_vertices;
        }
        self.tree_vertices.clear();
        let mut trees = [None; formiga_core::TreeEnd::BOTH.len()];
        for (slot, end) in formiga_core::TreeEnd::BOTH.into_iter().enumerate() {
            trees[slot] = formiga_core::home_tree_position(
                &save.home,
                end,
                &cottages,
                std::slice::from_ref(&self.monitor),
                &save.settings.habitat,
                save.settings.display_scale,
            )
            .filter(|(monitor_id, _)| *monitor_id == self.monitor.id)
            .map(|(_, point)| point);
        }
        for (variant, end, anchor) in
            formiga_art::hung_trinkets(&save.home, &save.companion.scrapbook)
        {
            let slot = formiga_core::TreeEnd::BOTH
                .iter()
                .position(|candidate| *candidate == end)
                .unwrap_or(0);
            let Some(tree) = trees[slot] else {
                continue;
            };
            self.tree_vertices
                .extend_from_slice(&self.hung_trinket_vertices(
                    tree,
                    variant,
                    anchor,
                    save.settings.display_scale,
                ));
        }
        self.tree_vertex_cache_key = Some(key);
        &self.tree_vertices
    }

    /// One found keepsake, drawn at the anchor for its own catalogue slot, so a thing the colony
    /// has already got used to seeing on one branch of one tree stays on that branch.
    pub(super) fn hung_trinket_vertices(
        &self,
        tree: formiga_core::Point,
        variant: u8,
        anchor: TrinketAnchor,
        display_scale: u8,
    ) -> [Vertex; 6] {
        let unit = f32::from(display_scale);
        let size = TRINKET_CELL as f32 * unit;
        let (local_x, local_y) = hung_trinket_centre(
            self.snap((tree.x - self.monitor.bounds.x) * self.monitor.scale_factor),
            self.snap((tree.y - self.monitor.bounds.y) * self.monitor.scale_factor),
            anchor,
            unit,
        );
        let left = (local_x - size / 2.0) / self.layout.width as f32 * 2.0 - 1.0;
        let right = (local_x + size / 2.0) / self.layout.width as f32 * 2.0 - 1.0;
        let top = 1.0 - (local_y - size / 2.0) / self.layout.height as f32 * 2.0;
        let bottom = 1.0 - (local_y + size / 2.0) / self.layout.height as f32 * 2.0;
        let (x, y, width, height) = TrinketAtlasRenderer::cell_rect(variant, TRINKET_FRAME_REST);
        let u0 = x as f32 / TRINKET_ATLAS_WIDTH as f32;
        let u1 = (x + width) as f32 / TRINKET_ATLAS_WIDTH as f32;
        let v0 = y as f32 / TRINKET_ATLAS_HEIGHT as f32;
        let v1 = (y + height) as f32 / TRINKET_ATLAS_HEIGHT as f32;
        let vertex = |position, uv| Vertex {
            position,
            uv,
            occlusion_enabled: 1.0,
        };
        [
            vertex([left, top], [u0, v0]),
            vertex([right, top], [u1, v0]),
            vertex([right, bottom], [u1, v1]),
            vertex([left, top], [u0, v0]),
            vertex([right, bottom], [u1, v1]),
            vertex([left, bottom], [u0, v1]),
        ]
    }
}
