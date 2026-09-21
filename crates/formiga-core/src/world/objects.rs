use super::*;

pub(crate) fn scheduled_colony_object_at(
    colony_seed: [u8; 32],
    ordinal: u32,
    from: OffsetDateTime,
) -> OffsetDateTime {
    let streams = SeedStream::new(colony_seed);
    let mut rng = streams.rng("colony-object-schedule", u64::from(ordinal));
    from + Duration::days(rng.random_range(3..=7))
}

pub(crate) fn scheduled_shelter_decoration_at(
    colony_seed: [u8; 32],
    ordinal: u32,
    from: OffsetDateTime,
) -> OffsetDateTime {
    let streams = SeedStream::new(colony_seed);
    let mut rng = streams.rng("shelter-decoration-schedule", u64::from(ordinal));
    from + Duration::days(rng.random_range(4..=9))
}

impl World {
    pub(super) fn process_colony_objects(
        &mut self,
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
    ) {
        if self.save.objects.objects.len() >= MAX_COLONY_OBJECTS
            || self.save.objects.next_at_utc > now
        {
            return;
        }
        let mut monitors: Vec<_> = desktop
            .monitors
            .iter()
            .filter(|monitor| !accessible_regions(&self.save.settings.habitat, monitor).is_empty())
            .collect();
        monitors.sort_by_key(|monitor| {
            (
                self.save.home.display != Some(monitor.display_key),
                !monitor.primary,
                monitor.id,
            )
        });
        let Some(monitor) = monitors.first().copied() else {
            return;
        };
        let Some(anchor) = resolved_home_anchor(
            &self.save.home,
            monitor,
            self.save.settings.display_scale,
            &self.save.settings.habitat,
        ) else {
            return;
        };
        let streams = SeedStream::new(self.save.colony_seed);
        let mut rng = streams.rng("colony-object", u64::from(self.save.objects.ordinal));
        let kind = ColonyObjectKind::ALL[rng.random_range(0..ColonyObjectKind::ALL.len())];
        let cottages = colony_cottages(&self.save.creatures);
        let point = home_object_position(
            &self.save.home,
            self.save.objects.objects.len(),
            &cottages,
            &desktop.monitors,
            &self.save.settings.habitat,
            self.save.settings.display_scale,
        )
        .map_or(anchor, |(_, point)| point);
        let mut id = rng.random::<u64>();
        while self
            .save
            .objects
            .objects
            .iter()
            .any(|object| object.id == id)
        {
            id = id.wrapping_add(1);
        }
        let object = ColonyObject {
            id,
            kind,
            display: monitor.display_key,
            normalized_position: Point {
                x: ((point.x - monitor.usable_bounds.x) / monitor.usable_bounds.width)
                    .clamp(0.0, 1.0),
                y: ((point.y - monitor.usable_bounds.y) / monitor.usable_bounds.height)
                    .clamp(0.0, 1.0),
            },
            role: kind.default_role(),
        };
        self.save.objects.objects.push(object);
        self.save.objects.ordinal = self.save.objects.ordinal.saturating_add(1);
        self.save.objects.next_at_utc =
            scheduled_colony_object_at(self.save.colony_seed, self.save.objects.ordinal, now);
        Self::emit(
            &mut self.events,
            WorldEvent::ColonyObjectAdded {
                object_id: id,
                kind,
            },
        );
    }

    pub(super) fn process_shelter_decorations(&mut self, now: OffsetDateTime) {
        if self.save.home.decorations.decorations.len() >= MAX_SHELTER_DECORATIONS
            || self.save.home.decorations.next_at_utc > now
        {
            return;
        }
        let Some(kind) = preferred_shelter_decoration(&self.save) else {
            return;
        };
        self.save.home.decorations.decorations.push(kind);
        self.save.home.decorations.ordinal = self.save.home.decorations.ordinal.saturating_add(1);
        self.save.home.decorations.next_at_utc = scheduled_shelter_decoration_at(
            self.save.colony_seed,
            self.save.home.decorations.ordinal,
            now,
        );
        Self::emit(
            &mut self.events,
            WorldEvent::ShelterDecorationAdded { kind },
        );
    }

    pub(super) fn reconcile_colony_objects(&mut self, desktop: &DesktopSnapshot) {
        // This runs on every tick whether or not anything has moved, so it resolves the village
        // once and reads every belonging's place out of that rather than asking eight times.
        let cottages = colony_cottage_list(&self.save.creatures);
        let places = home_object_positions(
            &self.save.home,
            cottages.as_slice(),
            &desktop.monitors,
            &self.save.settings.habitat,
            self.save.settings.display_scale,
        );
        for (slot, object) in self.save.objects.objects.iter_mut().enumerate() {
            let Some((monitor_id, point)) = places.get(slot).copied().flatten() else {
                continue;
            };
            let Some(monitor) = desktop
                .monitors
                .iter()
                .find(|monitor| monitor.id == monitor_id)
            else {
                continue;
            };
            object.display = monitor.display_key;
            object.normalized_position = Point {
                x: ((point.x - monitor.usable_bounds.x) / monitor.usable_bounds.width)
                    .clamp(0.0, 1.0),
                y: ((point.y - monitor.usable_bounds.y) / monitor.usable_bounds.height)
                    .clamp(0.0, 1.0),
            };
        }
    }
}

pub(super) fn nearby_object_utility(
    creature: &Creature,
    objects: &[ColonyObject],
    cottages: &[DwellingKind],
    desktop: &DesktopSnapshot,
    policy: &HabitatPolicy,
    home: &ColonyHome,
    display_scale: u8,
) -> ObjectUtility {
    let mut utility = ObjectUtility::default();
    if !home.is_active() {
        return utility;
    }
    // One village resolve for the whole yard, rather than one per belonging per creature.
    let places = home_object_positions(home, cottages, &desktop.monitors, policy, display_scale);
    for (slot, object) in objects.iter().take(MAX_COLONY_OBJECTS).enumerate() {
        let Some((monitor_id, point)) = places[slot] else {
            continue;
        };
        if monitor_id != creature.state.surface.monitor_id {
            continue;
        }
        let distance = creature.state.position.distance(point);
        if distance > 160.0 {
            continue;
        }
        utility.add(object.role, 0.08 * (1.0 - distance / 160.0));
    }
    utility
}

pub(super) fn preferred_shelter_decoration(save: &SaveFile) -> Option<ShelterDecorationKind> {
    let mut scores = [0_u64; MAX_SHELTER_DECORATIONS];
    for creature in &save.creatures {
        let memory = &creature.memory;
        scores[ShelterDecorationKind::Leaf.index()] = scores[ShelterDecorationKind::Leaf.index()]
            .saturating_add(u64::from(memory.ledge_seconds / 60))
            .saturating_add(u64::from(memory.window_climbs).saturating_mul(20));
        scores[ShelterDecorationKind::Banner.index()] = scores
            [ShelterDecorationKind::Banner.index()]
        .saturating_add(u64::from(memory.times_petted).saturating_mul(3))
        .saturating_add(u64::from(memory.play_sessions).saturating_mul(2));
        scores[ShelterDecorationKind::Stone.index()] = scores[ShelterDecorationKind::Stone.index()]
            .saturating_add(u64::from(memory.placements).saturating_mul(4))
            .saturating_add(u64::from(memory.home_visits).saturating_mul(6));
        scores[ShelterDecorationKind::Flower.index()] = scores
            [ShelterDecorationKind::Flower.index()]
        .saturating_add(u64::from(memory.discoveries_found).saturating_mul(4))
        .saturating_add(u64::from(memory.times_petted));
        scores[ShelterDecorationKind::Lamp.index()] = scores[ShelterDecorationKind::Lamp.index()]
            .saturating_add(u64::from(memory.longest_sleep_seconds / 60))
            .saturating_add(u64::from(memory.home_visits).saturating_mul(8));
        scores[ShelterDecorationKind::RoofOrnament.index()] = scores
            [ShelterDecorationKind::RoofOrnament.index()]
        .saturating_add(u64::from(memory.window_climbs).saturating_mul(20))
        .saturating_add(u64::from(memory.window_ride_seconds / 60))
        .saturating_add(u64::from(memory.discoveries_found).saturating_mul(3));
    }
    for relationship in &save.relationships {
        scores[ShelterDecorationKind::Banner.index()] = scores
            [ShelterDecorationKind::Banner.index()]
        .saturating_add(u64::from(relationship.affinity))
        .saturating_add(u64::from(relationship.familiarity));
        scores[ShelterDecorationKind::Flower.index()] = scores
            [ShelterDecorationKind::Flower.index()]
        .saturating_add(u64::from(relationship.playfulness));
        scores[ShelterDecorationKind::Stone.index()] = scores[ShelterDecorationKind::Stone.index()]
            .saturating_add(u64::from(relationship.avoidance));
    }
    if let Some(kind) = save.ritual.last_kind {
        let decoration = match kind {
            RitualKind::Picnic => ShelterDecorationKind::Flower,
            RitualKind::GroupNap | RitualKind::LateNightSleepPile => ShelterDecorationKind::Lamp,
            RitualKind::FloorRace => ShelterDecorationKind::RoofOrnament,
            RitualKind::ShelterGathering | RitualKind::QuietDayHuddle => {
                ShelterDecorationKind::Leaf
            }
            RitualKind::Catch | RitualKind::GroupPresentation | RitualKind::HatchDay => {
                ShelterDecorationKind::Banner
            }
        };
        scores[decoration.index()] = scores[decoration.index()].saturating_add(256);
    }
    for object in &save.objects.objects {
        let decoration = match object.kind {
            ColonyObjectKind::Pillow | ColonyObjectKind::Blanket | ColonyObjectKind::Lamp => {
                ShelterDecorationKind::Lamp
            }
            ColonyObjectKind::Toy | ColonyObjectKind::Cup => ShelterDecorationKind::Banner,
            ColonyObjectKind::Plant => ShelterDecorationKind::Flower,
            ColonyObjectKind::Paper => ShelterDecorationKind::Leaf,
            ColonyObjectKind::Pebble => ShelterDecorationKind::Stone,
        };
        scores[decoration.index()] = scores[decoration.index()].saturating_add(128);
    }

    let streams = SeedStream::new(save.colony_seed);
    let mut rng = streams.rng(
        "shelter-decoration-choice",
        u64::from(save.home.decorations.ordinal),
    );
    ShelterDecorationKind::ALL
        .into_iter()
        .filter(|kind| !save.home.decorations.decorations.contains(kind))
        .map(|kind| (scores[kind.index()], rng.random::<u16>(), kind))
        .max()
        .map(|(_, _, kind)| kind)
}
