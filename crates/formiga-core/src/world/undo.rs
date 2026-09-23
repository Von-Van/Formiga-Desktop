//! Taking back the last change made to the colony from the settings window.
//!
//! Only one change is ever kept, and only for as long as the app runs: it is a way out of a
//! misclick, not a history. It covers the changes that decide who lives here and how the village
//! is laid out — a companion removed, replaced, started over or welcomed from the studio, and the
//! cottages, colours, gardens, spots, corner, display, decorations and keepsakes arranged on the
//! Home page. Everything else the colony did in the meantime is kept: undoing a removal brings the
//! companion back exactly as it left, and leaves everyone else as they are now.

use super::*;

/// A change to the colony that can be taken back, as the settings window names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColonyEdit {
    /// A companion taken out of the colony from its profile.
    Removed { name: String },
    /// A companion replaced from the studio, by name before it was replaced.
    Replaced { name: String },
    /// A new companion welcomed from the studio.
    Welcomed,
    /// Every companion not marked to keep started over at once.
    StartedOver,
    /// The cottages stood in a new order.
    MovedCottages,
    /// The village painted in a palette, or given back its own colours.
    PaintedVillage,
    /// A garden patch planted, moved or dug up.
    Garden(GardenKind),
    /// The cottage order, colours and gardens put back as the village grew.
    PutVillageBack,
    /// A hangout spot put down, moved or picked up.
    Hangout(HangoutKind),
    /// A house built as another type, or given back its own.
    HouseType,
    /// The home moved to another corner or display.
    MovedHome,
    /// A house's decorations hung or taken down.
    Decorations,
    /// An ornament set out, moved or taken in.
    Ornament(OrnamentKind),
    /// The keepsakes hanging in the trees chosen.
    TreeKeepsakes,
    /// The keepsakes rearranged.
    RearrangedKeepsakes,
}

impl ColonyEdit {
    /// What the undo button offers to take back, to follow "Undo": "removing Poppy".
    pub fn describe(&self) -> String {
        match self {
            Self::Removed { name } => format!("removing {name}"),
            Self::Replaced { name } => format!("replacing {name}"),
            Self::Welcomed => "welcoming a new companion".to_owned(),
            Self::StartedOver => "starting companions over".to_owned(),
            Self::MovedCottages => "moving the cottages".to_owned(),
            Self::PaintedVillage => "repainting the village".to_owned(),
            Self::Garden(kind) => format!("changing the {}", kind.label().to_lowercase()),
            Self::PutVillageBack => "putting the village back".to_owned(),
            Self::Hangout(kind) => format!("changing the {}", kind.label().to_lowercase()),
            Self::HouseType => "changing a house".to_owned(),
            Self::MovedHome => "moving the home".to_owned(),
            Self::Decorations => "changing a house's decorations".to_owned(),
            Self::Ornament(kind) => format!("changing the {}", kind.label().to_lowercase()),
            Self::TreeKeepsakes => "choosing the keepsakes in the trees".to_owned(),
            Self::RearrangedKeepsakes => "rearranging the keepsakes".to_owned(),
        }
    }

    /// Whether the change was to who lives here, rather than to how the village is laid out.
    fn is_about_companions(&self) -> bool {
        matches!(
            self,
            Self::Removed { .. } | Self::Replaced { .. } | Self::Welcomed | Self::StartedOver
        )
    }
}

/// Why the last change could not be taken back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum UndoError {
    #[error("there is nothing to undo")]
    NothingToUndo,
    #[error("there is no room to bring everyone back")]
    NoRoom,
}

/// The colony as it stood just before the last change, and what that change brought in.
#[derive(Clone, Debug)]
pub(super) struct UndoPoint {
    edit: ColonyEdit,
    creatures: Vec<Creature>,
    relationships: Vec<CreatureRelationship>,
    home: ColonyHome,
    objects: Vec<ColonyObject>,
    /// Companions the change itself brought in, which taking it back sends away again. Anyone
    /// who arrived on their own since stays.
    added: Vec<CreatureId>,
}

impl World {
    /// Make a change that can be taken back: `change` is applied, and if it changed who lives
    /// here or how the village is laid out, the colony as it stood before is kept, in place of any
    /// earlier change, until it is undone or the app quits. A change that fails, or that changes
    /// nothing, leaves the earlier one to undo.
    pub fn edit<T>(&mut self, edit: ColonyEdit, change: impl FnOnce(&mut World) -> T) -> T {
        let before = UndoPoint {
            edit,
            creatures: self.save.creatures.clone(),
            relationships: self.save.relationships.clone(),
            home: self.save.home.clone(),
            objects: self.save.objects.objects.clone(),
            added: Vec::new(),
        };
        let result = change(self);
        let ids = |creatures: &[Creature]| -> Vec<CreatureId> {
            creatures.iter().map(|creature| creature.id).collect()
        };
        let object_ids = |objects: &[ColonyObject]| -> Vec<u64> {
            objects.iter().map(|object| object.id).collect()
        };
        let changed = ids(&before.creatures) != ids(&self.save.creatures)
            || before.home != self.save.home
            || object_ids(&before.objects) != object_ids(&self.save.objects.objects);
        if changed {
            let added = self
                .save
                .creatures
                .iter()
                .map(|creature| creature.id)
                .filter(|id| !before.creatures.iter().any(|old| old.id == *id))
                .collect();
            self.last_edit = Some(UndoPoint { added, ..before });
        }
        result
    }

    /// The change the colony can take back, if there is one.
    pub fn last_edit(&self) -> Option<&ColonyEdit> {
        self.last_edit.as_ref().map(|point| &point.edit)
    }

    /// Forget the change that could have been taken back, when the colony itself is replaced.
    pub fn forget_last_edit(&mut self) {
        self.last_edit = None;
    }

    /// Take back the last change.
    ///
    /// A companion that was removed or replaced comes back exactly as it left — its memories, its
    /// bonds with everyone still here, its minis, and its cottage — with the same id, standing
    /// where it or its replacement last stood. A companion the change brought in goes again.
    /// Everyone else keeps everything they have done since, and anyone who arrived on their own
    /// in the meantime stays. A layout change puts back exactly the arrangement it changed.
    pub fn undo_last_edit(&mut self) -> Result<ColonyEdit, UndoError> {
        let point = self.last_edit.take().ok_or(UndoError::NothingToUndo)?;
        if point.edit.is_about_companions() {
            if let Err(error) = self.restore_companions(&point) {
                self.last_edit = Some(point);
                return Err(error);
            }
            self.save.home.cottage_order = point.home.cottage_order.clone();
            self.save.home.house_styles = point.home.house_styles.clone();
            self.save.home.dressing = point.home.dressing.clone();
        } else {
            let home = &mut self.save.home;
            home.corner = point.home.corner;
            home.display = point.home.display;
            home.hangouts = point.home.hangouts.clone();
            home.cottage_order = point.home.cottage_order.clone();
            home.palette = point.home.palette;
            home.gardens = point.home.gardens.clone();
            home.house_styles = point.home.house_styles.clone();
            home.dressing = point.home.dressing.clone();
            home.ornaments = point.home.ornaments.clone();
            home.tree_keepsakes = point.home.tree_keepsakes;
            // The keepsakes in the order they stood, and anything found since after them.
            let mut objects: Vec<ColonyObject> = point
                .objects
                .iter()
                .filter_map(|old| {
                    self.save
                        .objects
                        .objects
                        .iter()
                        .find(|object| object.id == old.id)
                        .cloned()
                })
                .collect();
            for object in &self.save.objects.objects {
                if !objects.iter().any(|kept| kept.id == object.id) {
                    objects.push(object.clone());
                }
            }
            self.save.objects.objects = objects;
        }
        self.save.home.normalize_village();
        self.clear_runtime_plans();
        Ok(point.edit)
    }

    fn restore_companions(&mut self, point: &UndoPoint) -> Result<(), UndoError> {
        let now = &self.save.creatures;
        let mut creatures: Vec<Creature> = Vec::with_capacity(point.creatures.len());
        let mut restored = Vec::new();
        for (index, before) in point.creatures.iter().enumerate() {
            if let Some(current) = now.iter().find(|creature| creature.id == before.id) {
                let mut kept = current.clone();
                kept.role = before.role;
                creatures.push(kept);
                continue;
            }
            // Back where it last stood, or where whoever took its place stands now.
            let mut back = before.clone();
            if let Some(stand_in) = now
                .get(index)
                .filter(|creature| point.added.contains(&creature.id))
            {
                back.state.position = stand_in.state.position;
                back.state.surface = stand_in.state.surface.clone();
            }
            back.state.action = ActionKind::Idle;
            back.state.action_elapsed = 0.0;
            back.state.action_duration = 1.0;
            back.state.velocity = Point::default();
            back.state.attention = None;
            back.state.flourish = None;
            restored.push(back.id);
            creatures.push(back);
        }
        for current in now {
            let known = point.creatures.iter().any(|old| old.id == current.id);
            if !known && !point.added.contains(&current.id) {
                creatures.push(current.clone());
            }
        }
        let adults = creatures
            .iter()
            .filter(|creature| creature.role.is_adult())
            .count();
        if adults > MAX_ADULT_CREATURES || creatures.len() > MAX_COLONY_CREATURES {
            return Err(UndoError::NoRoom);
        }

        for id in &point.added {
            if !creatures.iter().any(|creature| creature.id == *id) {
                self.remove_creature_runtime(*id);
            }
        }
        for creature in creatures
            .iter()
            .filter(|creature| restored.contains(&creature.id))
        {
            self.register_creature_runtime(creature);
        }
        let present: BTreeSet<CreatureId> = creatures.iter().map(|creature| creature.id).collect();
        let mut relationships: Vec<CreatureRelationship> = self
            .save
            .relationships
            .iter()
            .filter(|bond| !restored.contains(&bond.a) && !restored.contains(&bond.b))
            .cloned()
            .collect();
        relationships.extend(
            point
                .relationships
                .iter()
                .filter(|bond| restored.contains(&bond.a) || restored.contains(&bond.b))
                .filter(|bond| present.contains(&bond.a) && present.contains(&bond.b))
                .cloned(),
        );
        self.save.creatures = creatures;
        self.save.relationships = relationships;
        // A mini that arrived since, whose big version was the one the change brought in, is
        // given a big version from those here now.
        let orphaned = self.save.creatures.iter().any(|creature| {
            creature
                .role
                .parent_id()
                .is_some_and(|parent| !self.save.creatures.iter().any(|adult| adult.id == parent))
        });
        if orphaned {
            rebalance_minis(&mut self.save.creatures);
        }
        normalize_relationships(&mut self.save);
        Ok(())
    }
}
