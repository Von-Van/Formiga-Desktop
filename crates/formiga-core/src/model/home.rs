//! The colony's home: where it stands, its houses, and everything arranged about the village.

use super::{
    Creature, CreatureId, DecorationSlot, DisplayKey, GardenKind, GardenPatch, HangoutKind,
    HangoutSpot, HouseDressing, HouseStyleChoice, MAX_COLONY_CREATURES, MAX_GARDENS, MAX_HANGOUTS,
    MAX_HOUSE_DECORATIONS, MAX_ORNAMENTS, OrnamentKind, OrnamentSpot, ShelterDecorationKind,
    ShelterGenome, ShelterStyle, TREE_HOOKS, TreeKeepsakes, VillageUnlocks,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HomeCorner {
    #[default]
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColonyHome {
    pub display: Option<DisplayKey>,
    pub corner: HomeCorner,
    pub shelter: ShelterGenome,
    #[serde(with = "time::serde::rfc3339::option")]
    pub active_since_utc: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_disappeared_utc: Option<OffsetDateTime>,
    /// The spots the person at the desk has put down on the ground between the houses. Absent
    /// from the file while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hangouts: Vec<HangoutSpot>,
    /// The order the cottages stand in, as the person at the desk arranged them: companions by
    /// id, the founder's colony house always first. Absent while they stand as they arrived.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cottage_order: Vec<CreatureId>,
    /// A named palette the village is painted in, in place of the colours it was generated with.
    /// Absent while it keeps its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<VillagePalette>,
    /// Little garden patches planted along the ground. Absent while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gardens: Vec<GardenPatch>,
    /// House types chosen by hand, one at most for each companion who keeps a house. A house
    /// with none is the colony's own type for the colony house and its keeper's own for a
    /// cottage. Absent while none has been chosen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub house_styles: Vec<HouseStyleChoice>,
    /// What the village has to choose from, and when the next thing arrives.
    #[serde(default)]
    pub unlocks: VillageUnlocks,
    /// The decorations each house wears, by the companion who keeps it. A house with no entry
    /// wears none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dressing: Vec<HouseDressing>,
    /// Ornaments set out on the village ground. Absent while there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ornaments: Vec<OrnamentSpot>,
    /// The keepsakes chosen to hang in the two trees. Absent while the trees fill themselves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree_keepsakes: Option<TreeKeepsakes>,
    /// A picture the village is laid out on, in place of the strip along the bottom of the
    /// display. Absent while the village keeps to the strip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenery: Option<VillageScenery>,
}

/// A place the village can be laid out on: a picture with its own paths, stairs and cliffs, and
/// a spot on it for every house and both trees. Choosing none keeps the village to its strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VillageScenery {
    /// Terraces round a pond, with a waterfall, two bridges and stairs between the levels.
    Pond,
}

impl VillageScenery {
    pub const ALL: [Self; 1] = [Self::Pond];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Pond => "The pond",
        }
    }
}

/// A palette the village can be painted in: a hand-made pairing of a main colour for roofs,
/// caps, leaves and fabric with an accent for the trim. Choosing none keeps the colours the
/// colony's own seed gave it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VillagePalette {
    Meadow,
    Blossom,
    Harbour,
    Autumn,
    Twilight,
    Pebble,
}

impl VillagePalette {
    pub const ALL: [Self; 6] = [
        Self::Meadow,
        Self::Blossom,
        Self::Harbour,
        Self::Autumn,
        Self::Twilight,
        Self::Pebble,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Meadow => "Meadow",
            Self::Blossom => "Blossom",
            Self::Harbour => "Harbour",
            Self::Autumn => "Autumn",
            Self::Twilight => "Twilight",
            Self::Pebble => "Pebble",
        }
    }

    /// The main and accent palettes it paints the village with, from the same twelve hand-made
    /// palettes the colony's own colours are drawn from. Some houses and the tree wear the main
    /// colour most and some the accent, so both halves of a pair belong to its name: two greens
    /// for a meadow, two pinks for blossom, sea blues for a harbour, warm oranges for autumn,
    /// dusky purples for twilight, and stone grey with moss for pebbles.
    pub const fn palettes(self) -> (u8, u8) {
        match self {
            Self::Meadow => (4, 8),
            Self::Blossom => (0, 10),
            Self::Harbour => (6, 1),
            Self::Autumn => (2, 5),
            Self::Twilight => (7, 3),
            Self::Pebble => (9, 11),
        }
    }
}

impl ColonyHome {
    pub fn from_seed(
        seed: [u8; 32],
        display: Option<DisplayKey>,
        active_since_utc: Option<OffsetDateTime>,
        last_disappeared_utc: Option<OffsetDateTime>,
    ) -> Self {
        let detail_seed = u64::from_le_bytes(seed[8..16].try_into().unwrap());
        Self {
            display,
            corner: if seed[0] & 1 == 0 {
                HomeCorner::BottomLeft
            } else {
                HomeCorner::BottomRight
            },
            shelter: ShelterGenome {
                style: ShelterStyle::ALL[usize::from(seed[1] % 4)],
                palette_index: seed[2] % 12,
                accent_index: seed[3] % 12,
                width: 34 + seed[4] % 9,
                height: 27 + seed[5] % 10,
                detail_seed,
            },
            active_since_utc,
            last_disappeared_utc,
            hangouts: Vec::new(),
            cottage_order: Vec::new(),
            palette: None,
            gardens: Vec::new(),
            house_styles: Vec::new(),
            unlocks: VillageUnlocks::starting(),
            dressing: Vec::new(),
            ornaments: Vec::new(),
            tree_keepsakes: None,
            scenery: None,
        }
    }

    /// The type of every house in the village, in the order they stand: whatever was chosen for
    /// it, or else the colony's own type for the colony house and the keeper's own for a
    /// cottage. Slots past the last house are the colony's own type.
    pub fn house_style_list(&self, creatures: &[Creature]) -> [ShelterStyle; MAX_COLONY_CREATURES] {
        let mut styles = [self.shelter.style; MAX_COLONY_CREATURES];
        let owners = crate::house_owners(creatures, &self.cottage_order);
        for (slot, keeper) in owners.as_slice().iter().enumerate() {
            styles[slot] = match self.house_style(*keeper) {
                Some(chosen) => chosen,
                None if slot == 0 => self.shelter.style,
                None => creatures
                    .iter()
                    .find(|creature| creature.id == *keeper)
                    .map_or(self.shelter.style, ShelterStyle::for_keeper),
            };
        }
        styles
    }

    /// The house type chosen by hand for the house this companion keeps, if one was.
    pub fn house_style(&self, keeper: CreatureId) -> Option<ShelterStyle> {
        self.house_styles
            .iter()
            .find(|choice| choice.keeper == keeper)
            .map(|choice| choice.style)
    }

    /// Choose a type for the house this companion keeps, or with `None` give it back its own.
    pub fn set_house_style(&mut self, keeper: CreatureId, style: Option<ShelterStyle>) {
        self.house_styles.retain(|choice| choice.keeper != keeper);
        if let Some(style) = style {
            self.house_styles.push(HouseStyleChoice { keeper, style });
        }
        self.normalize_village();
    }

    /// The decorations the house this companion keeps wears, one per slot at most, in slot order.
    pub fn decorations_of(&self, keeper: CreatureId) -> &[ShelterDecorationKind] {
        self.dressing
            .iter()
            .find(|dressing| dressing.keeper == keeper)
            .map_or(&[], |dressing| dressing.decorations.as_slice())
    }

    /// The decoration in one slot of the house this companion keeps.
    pub fn decoration_in(
        &self,
        keeper: CreatureId,
        slot: DecorationSlot,
    ) -> Option<ShelterDecorationKind> {
        self.decorations_of(keeper)
            .iter()
            .copied()
            .find(|kind| kind.slot() == slot)
    }

    /// Hang a decoration on the house this companion keeps, in the slot it belongs to, replacing
    /// whatever was there; or with `None`, take down whatever is in `slot`. Refused for a
    /// decoration the village has not got yet, or one that does not belong in `slot`.
    pub fn set_decoration(
        &mut self,
        keeper: CreatureId,
        slot: DecorationSlot,
        kind: Option<ShelterDecorationKind>,
    ) -> bool {
        if let Some(kind) = kind
            && (kind.slot() != slot || !self.unlocks.decorations.contains(&kind))
        {
            return false;
        }
        let index = match self.dressing.iter().position(|d| d.keeper == keeper) {
            Some(index) => index,
            None => {
                self.dressing.push(HouseDressing {
                    keeper,
                    decorations: Vec::new(),
                });
                self.dressing.len() - 1
            }
        };
        let decorations = &mut self.dressing[index].decorations;
        decorations.retain(|existing| existing.slot() != slot);
        if let Some(kind) = kind {
            decorations.push(kind);
        }
        self.normalize_village();
        true
    }

    /// The decorations every house in the village wears, in the order the houses stand.
    pub fn house_decoration_list(
        &self,
        creatures: &[Creature],
    ) -> [Vec<ShelterDecorationKind>; MAX_COLONY_CREATURES] {
        let mut lists: [Vec<ShelterDecorationKind>; MAX_COLONY_CREATURES] = Default::default();
        let owners = crate::house_owners(creatures, &self.cottage_order);
        for (slot, keeper) in owners.as_slice().iter().enumerate() {
            lists[slot] = self.decorations_of(*keeper).to_vec();
        }
        lists
    }

    /// The shelter as it is drawn: the colony's own, repainted in the palette chosen for the
    /// village if one was. Its style, size and details are never touched.
    pub fn drawn_shelter(&self) -> ShelterGenome {
        let mut shelter = self.shelter;
        if let Some(palette) = self.palette {
            (shelter.palette_index, shelter.accent_index) = palette.palettes();
        }
        shelter
    }

    /// Stand the cottages in a new order, given as the companions who keep them. The founder's
    /// colony house stays first whatever the order says, a cottage left out keeps its place after
    /// the ones given, and an order that is just the order everyone arrived in is not written
    /// down at all.
    pub fn arrange_cottages(&mut self, order: Vec<CreatureId>, creatures: &[Creature]) {
        self.cottage_order = order;
        self.normalize_village();
        let arranged = crate::house_owners(creatures, &self.cottage_order);
        self.cottage_order = if arranged == crate::house_owners(creatures, &[]) {
            Vec::new()
        } else {
            arranged.as_slice()[1..].to_vec()
        };
    }

    /// Plant a patch, move it, or with `None` dig it up. A fraction outside the ground is brought
    /// back onto it; one that is not a number is refused, and so is a garden the village has not
    /// got yet or one more than the ground holds. A new patch is planted `now` and starts as a
    /// sprout; moving one keeps it growing where it is.
    pub fn set_garden(
        &mut self,
        kind: GardenKind,
        along: Option<f32>,
        now: OffsetDateTime,
    ) -> bool {
        match along {
            Some(along) if !along.is_finite() => false,
            Some(along) => {
                let along = along.clamp(0.0, 1.0);
                match self.gardens.iter_mut().find(|patch| patch.kind == kind) {
                    Some(patch) => patch.along = along,
                    None => {
                        if !self.unlocks.gardens.contains(&kind)
                            || self.gardens.len() >= MAX_GARDENS
                        {
                            return false;
                        }
                        self.gardens.push(GardenPatch {
                            kind,
                            along,
                            planted_at_utc: Some(now),
                        });
                    }
                }
                self.normalize_village();
                true
            }
            None => {
                self.gardens.retain(|patch| patch.kind != kind);
                true
            }
        }
    }

    /// Set an ornament out, move it, or with `None` take it in again, on the same terms as a
    /// garden.
    pub fn set_ornament(&mut self, kind: OrnamentKind, along: Option<f32>) -> bool {
        match along {
            Some(along) if !along.is_finite() => false,
            Some(along) => {
                let along = along.clamp(0.0, 1.0);
                match self.ornaments.iter_mut().find(|spot| spot.kind == kind) {
                    Some(spot) => spot.along = along,
                    None => {
                        if !self.unlocks.ornaments.contains(&kind)
                            || self.ornaments.len() >= MAX_ORNAMENTS
                        {
                            return false;
                        }
                        self.ornaments.push(OrnamentSpot { kind, along });
                    }
                }
                self.normalize_village();
                true
            }
            None => {
                self.ornaments.retain(|spot| spot.kind != kind);
                true
            }
        }
    }

    /// Everything the person at the desk arranged about the village put back as it was
    /// generated: the cottages in the order their keepers arrived, the colony's own colours and
    /// house types, and nothing planted or set out on the ground. Hangout spots, decorations and
    /// the keepsakes in the trees are left as they were.
    pub fn reset_arrangement(&mut self) {
        self.cottage_order.clear();
        self.palette = None;
        self.gardens.clear();
        self.ornaments.clear();
        self.house_styles.clear();
    }

    /// One of each kind at most, each somewhere on the ground and each something the village
    /// has, and no more than the ground holds; one dressing per house, one decoration per slot.
    pub fn normalize_village(&mut self) {
        self.unlocks.normalize();
        self.normalize_hangouts();
        let mut seen = Vec::new();
        let gardens = &self.unlocks.gardens;
        self.gardens.retain(|patch| {
            let fresh = patch.along.is_finite()
                && !seen.contains(&patch.kind)
                && gardens.contains(&patch.kind);
            seen.push(patch.kind);
            fresh
        });
        for patch in &mut self.gardens {
            patch.along = patch.along.clamp(0.0, 1.0);
        }
        self.gardens.sort_by_key(|patch| patch.kind.index());
        self.gardens.truncate(MAX_GARDENS);
        let mut seen = Vec::new();
        let ornaments = &self.unlocks.ornaments;
        self.ornaments.retain(|spot| {
            let fresh = spot.along.is_finite()
                && !seen.contains(&spot.kind)
                && ornaments.contains(&spot.kind);
            seen.push(spot.kind);
            fresh
        });
        for spot in &mut self.ornaments {
            spot.along = spot.along.clamp(0.0, 1.0);
        }
        self.ornaments.sort_by_key(|spot| spot.kind.index());
        self.ornaments.truncate(MAX_ORNAMENTS);
        let mut seen = Vec::new();
        self.cottage_order.retain(|id| {
            let fresh = !seen.contains(id);
            seen.push(*id);
            fresh
        });
        self.cottage_order.truncate(MAX_COLONY_CREATURES);
        let mut seen = Vec::new();
        self.house_styles.retain(|choice| {
            let fresh = !seen.contains(&choice.keeper);
            seen.push(choice.keeper);
            fresh
        });
        self.house_styles.truncate(MAX_COLONY_CREATURES);
        let mut seen = Vec::new();
        let decorations = &self.unlocks.decorations;
        self.dressing.retain(|dressing| {
            let fresh = !seen.contains(&dressing.keeper);
            seen.push(dressing.keeper);
            fresh
        });
        for dressing in &mut self.dressing {
            let mut slots = Vec::with_capacity(MAX_HOUSE_DECORATIONS);
            dressing.decorations.retain(|kind| {
                let fresh = !slots.contains(&kind.slot()) && decorations.contains(kind);
                slots.push(kind.slot());
                fresh
            });
            dressing.decorations.sort_by_key(|kind| kind.slot());
        }
        self.dressing
            .retain(|dressing| !dressing.decorations.is_empty());
        self.dressing.truncate(MAX_COLONY_CREATURES);
        if let Some(chosen) = &mut self.tree_keepsakes {
            let mut seen = Vec::new();
            for hook in &mut chosen.hooks {
                if let Some(variant) = *hook
                    && (variant >= crate::TRINKET_VARIANTS || seen.contains(&variant))
                {
                    *hook = None;
                }
                if let Some(variant) = *hook {
                    seen.push(variant);
                }
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.active_since_utc.is_some()
    }

    /// The spot of this kind, if one has been put down.
    pub fn hangout(&self, kind: HangoutKind) -> Option<HangoutSpot> {
        self.hangouts.iter().copied().find(|spot| spot.kind == kind)
    }

    /// The patch of this kind, if one has been planted.
    pub fn garden(&self, kind: GardenKind) -> Option<GardenPatch> {
        self.gardens
            .iter()
            .copied()
            .find(|patch| patch.kind == kind)
    }

    /// The ornament of this kind, if one has been set out.
    pub fn ornament(&self, kind: OrnamentKind) -> Option<OrnamentSpot> {
        self.ornaments
            .iter()
            .copied()
            .find(|spot| spot.kind == kind)
    }

    /// Whether anything has been put down, planted or set out on the village ground.
    pub fn has_ground_items(&self) -> bool {
        !self.hangouts.is_empty() || !self.gardens.is_empty() || !self.ornaments.is_empty()
    }

    /// Put a spot down, move it, or with `None` pick it up again. A fraction outside the ground
    /// is brought back onto it; one that is not a number is refused, and so is a spot the village
    /// has not got yet or one more than the ground holds.
    pub fn set_hangout(&mut self, kind: HangoutKind, along: Option<f32>) -> bool {
        match along {
            Some(along) if !along.is_finite() => false,
            Some(along) => {
                let along = along.clamp(0.0, 1.0);
                match self.hangouts.iter_mut().find(|spot| spot.kind == kind) {
                    Some(spot) => spot.along = along,
                    None => {
                        if !self.unlocks.hangouts.contains(&kind)
                            || self.hangouts.len() >= MAX_HANGOUTS
                        {
                            return false;
                        }
                        self.hangouts.push(HangoutSpot { kind, along });
                    }
                }
                self.normalize_hangouts();
                true
            }
            None => {
                self.hangouts.retain(|spot| spot.kind != kind);
                true
            }
        }
    }

    /// One spot of each kind at most, each somewhere on the ground, in a stable order.
    pub fn normalize_hangouts(&mut self) {
        let mut seen = Vec::new();
        let hangouts = &self.unlocks.hangouts;
        self.hangouts.retain(|spot| {
            let fresh = spot.along.is_finite()
                && !seen.contains(&spot.kind)
                && hangouts.contains(&spot.kind);
            seen.push(spot.kind);
            fresh
        });
        for spot in &mut self.hangouts {
            spot.along = spot.along.clamp(0.0, 1.0);
        }
        self.hangouts.sort_by_key(|spot| spot.kind.index());
        self.hangouts.truncate(MAX_HANGOUTS);
    }

    /// Hang the keepsakes in the trees by hand: a variant or nothing for each hook, or with
    /// `None` let the trees fill themselves again.
    pub fn set_tree_keepsakes(&mut self, hooks: Option<[Option<u8>; TREE_HOOKS]>) {
        self.tree_keepsakes = hooks.map(|hooks| TreeKeepsakes { hooks });
        self.normalize_village();
    }
}

impl Default for ColonyHome {
    fn default() -> Self {
        Self::from_seed([0; 32], None, None, None)
    }
}
