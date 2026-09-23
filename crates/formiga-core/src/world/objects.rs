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

/// When the village is next given something new to choose from: a day or two after the last.
pub(crate) fn scheduled_village_unlock_at(
    colony_seed: [u8; 32],
    ordinal: u32,
    from: OffsetDateTime,
) -> OffsetDateTime {
    let streams = SeedStream::new(colony_seed);
    let mut rng = streams.rng("village-unlock-schedule", u64::from(ordinal));
    from + Duration::hours(rng.random_range(24..=48))
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
        // Something the yard does not have yet, while there is anything it does not have.
        let fresh: Vec<ColonyObjectKind> = ColonyObjectKind::ALL
            .into_iter()
            .filter(|kind| !self.save.objects.objects.iter().any(|o| o.kind == *kind))
            .collect();
        let kind = if fresh.is_empty() {
            ColonyObjectKind::ALL[rng.random_range(0..ColonyObjectKind::ALL.len())]
        } else {
            fresh[rng.random_range(0..fresh.len())]
        };
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

    /// Every day or two, one more thing for the village to choose from, picked by what the colony
    /// has been doing. A decoration goes straight up on the colony house when the place it hangs
    /// there is free, the way earned decorations always have; everything else waits on the Home
    /// page to be put down. At most one arrives however long the colony has been away.
    pub(super) fn process_village_unlocks(&mut self, now: OffsetDateTime) {
        if self.save.home.unlocks.next_at_utc > now {
            return;
        }
        let Some(item) = preferred_village_unlock(&self.save) else {
            return;
        };
        let home = &mut self.save.home;
        home.unlocks.grant(item);
        if let VillageItem::Decoration(kind) = item {
            let owners = house_owners(&self.save.creatures, &home.cottage_order);
            if let Some(keeper) = owners.as_slice().first().copied()
                && home.decoration_in(keeper, kind.slot()).is_none()
            {
                home.set_decoration(keeper, kind.slot(), Some(kind));
            }
        }
        home.unlocks.ordinal = home.unlocks.ordinal.saturating_add(1);
        home.unlocks.next_at_utc =
            scheduled_village_unlock_at(self.save.colony_seed, home.unlocks.ordinal, now);
        Self::emit(&mut self.events, WorldEvent::VillageUnlocked { item });
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

/// What a colony's days have mostly been about, as the village reads it when choosing what comes
/// next. Each thing the village can gain belongs to one of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VillageTheme {
    /// Climbing, riding and the view from up high.
    Sky,
    /// Leaves, vines and green things.
    Nature,
    /// Company, play and parties.
    Social,
    /// Naps, lamps and quiet evenings.
    Rest,
    /// Finding and looking at things.
    Curiosity,
    /// Home itself: doorsteps, paths and the things by the door.
    Home,
    /// Growing things.
    Garden,
    /// Games and making a noise.
    Play,
}

impl VillageTheme {
    const ALL: [Self; 8] = [
        Self::Sky,
        Self::Nature,
        Self::Social,
        Self::Rest,
        Self::Curiosity,
        Self::Home,
        Self::Garden,
        Self::Play,
    ];

    fn index(self) -> usize {
        self as usize
    }
}

fn theme_of(item: VillageItem) -> VillageTheme {
    use ShelterDecorationKind as D;
    use VillageTheme as T;
    match item {
        VillageItem::Decoration(kind) => match kind {
            D::RoofOrnament | D::WeatherVane | D::PerchedBird | D::Pinwheel | D::WindChime => {
                T::Sky
            }
            D::Leaf | D::Ivy | D::LeafGarland | D::Wreath => T::Nature,
            D::Banner | D::Pennant | D::PaperLanterns | D::FairyLights => T::Social,
            D::Lamp | D::Lantern | D::Clock => T::Rest,
            D::Birdhouse | D::Horseshoe => T::Curiosity,
            D::Stone | D::Woodpile | D::Barrel | D::Boots | D::Mailbox | D::HouseSign => T::Home,
            D::Flower
            | D::Pumpkin
            | D::Mushrooms
            | D::WateringCan
            | D::PottedPlant
            | D::WindowBox => T::Garden,
        },
        VillageItem::Hangout(kind) => match kind {
            HangoutKind::Cushion
            | HangoutKind::Hammock
            | HangoutKind::SunnyRock
            | HangoutKind::StargazingMat => T::Rest,
            HangoutKind::Blanket
            | HangoutKind::TeaTable
            | HangoutKind::Campfire
            | HangoutKind::Bench => T::Social,
            HangoutKind::Lookout | HangoutKind::BirdFeeder | HangoutKind::BookNook => T::Curiosity,
            HangoutKind::Swing
            | HangoutKind::Sandbox
            | HangoutKind::Puddle
            | HangoutKind::DrumStump => T::Play,
        },
        VillageItem::Garden(kind) => match kind {
            GardenKind::MushroomRing | GardenKind::Cactus => T::Nature,
            _ => T::Garden,
        },
        VillageItem::Ornament(kind) => match kind {
            OrnamentKind::LampPost => T::Rest,
            OrnamentKind::BirdBath | OrnamentKind::Signpost | OrnamentKind::MailboxPost => {
                T::Curiosity
            }
            OrnamentKind::WishingWell
            | OrnamentKind::PicketFence
            | OrnamentKind::SteppingStones
            | OrnamentKind::StoneCairn => T::Home,
            OrnamentKind::Scarecrow | OrnamentKind::Wheelbarrow | OrnamentKind::Beehive => {
                T::Garden
            }
            OrnamentKind::WindSpinner | OrnamentKind::FlagPole => T::Sky,
            OrnamentKind::LilyPond => T::Nature,
            OrnamentKind::LogStool => T::Social,
        },
    }
}

/// How much of each theme the colony's own record shows: its memories, its bonds, the last ritual
/// it held, the belongings it keeps, and the gardens it has planted.
fn theme_scores(save: &SaveFile) -> [u64; VillageTheme::ALL.len()] {
    use VillageTheme as T;
    let mut scores = [0_u64; VillageTheme::ALL.len()];
    let mut add = |theme: T, amount: u64| {
        scores[theme.index()] = scores[theme.index()].saturating_add(amount);
    };
    for creature in &save.creatures {
        let memory = &creature.memory;
        add(
            T::Sky,
            u64::from(memory.window_climbs).saturating_mul(20)
                + u64::from(memory.window_ride_seconds / 60),
        );
        add(T::Nature, u64::from(memory.ledge_seconds / 60));
        add(
            T::Social,
            u64::from(memory.times_petted).saturating_mul(3)
                + u64::from(memory.play_sessions).saturating_mul(2),
        );
        add(
            T::Rest,
            u64::from(memory.longest_sleep_seconds / 60)
                + u64::from(memory.home_visits).saturating_mul(4),
        );
        add(
            T::Curiosity,
            u64::from(memory.discoveries_found).saturating_mul(6),
        );
        add(
            T::Home,
            u64::from(memory.placements).saturating_mul(4)
                + u64::from(memory.home_visits).saturating_mul(6),
        );
        add(T::Play, u64::from(memory.play_sessions).saturating_mul(5));
    }
    for relationship in &save.relationships {
        add(
            T::Social,
            u64::from(relationship.affinity) + u64::from(relationship.familiarity),
        );
        add(T::Play, u64::from(relationship.playfulness));
    }
    if let Some(kind) = save.ritual.last_kind {
        let theme = match kind {
            RitualKind::Picnic => T::Garden,
            RitualKind::GroupNap | RitualKind::LateNightSleepPile => T::Rest,
            RitualKind::FloorRace => T::Play,
            RitualKind::ShelterGathering | RitualKind::QuietDayHuddle => T::Home,
            RitualKind::Catch
            | RitualKind::GroupPresentation
            | RitualKind::HatchDay
            | RitualKind::Dance => T::Social,
        };
        add(theme, 256);
    }
    for object in &save.objects.objects {
        let theme = match object.role {
            ColonyObjectRole::Sleep | ColonyObjectRole::Comfort => T::Rest,
            ColonyObjectRole::Play => T::Play,
            ColonyObjectRole::Social => T::Social,
            ColonyObjectRole::Curiosity => T::Curiosity,
        };
        add(theme, 128);
    }
    add(T::Garden, save.home.gardens.len() as u64 * 192);
    scores
}

/// What the village gains next. Categories take turns — the one with the most still to come is
/// likeliest — and within them what the colony has been doing decides, with the colony's own seed
/// to break ties. `None` once the village has everything.
pub(super) fn preferred_village_unlock(save: &SaveFile) -> Option<VillageItem> {
    let unlocks = &save.home.unlocks;
    let themes = theme_scores(save);
    let busiest = themes.iter().copied().max().unwrap_or(0).max(1) as f32;
    let still_to_come = |item: VillageItem| -> f32 {
        let (have, of) = match item {
            VillageItem::Decoration(_) => {
                (unlocks.decorations.len(), ShelterDecorationKind::ALL.len())
            }
            VillageItem::Hangout(_) => (unlocks.hangouts.len(), HangoutKind::ALL.len()),
            VillageItem::Garden(_) => (unlocks.gardens.len(), GardenKind::ALL.len()),
            VillageItem::Ornament(_) => (unlocks.ornaments.len(), OrnamentKind::ALL.len()),
        };
        1.0 - have as f32 / of.max(1) as f32
    };
    let streams = SeedStream::new(save.colony_seed);
    let mut rng = streams.rng("village-unlock-choice", u64::from(unlocks.ordinal));
    unlocks
        .remaining()
        .map(|item| {
            let theme = themes[theme_of(item).index()] as f32 / busiest;
            let jitter: f32 = rng.random_range(0.0..0.3);
            let score = still_to_come(item) * 0.6 + theme * 0.4 + jitter;
            (item, score)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(item, _)| item)
}
