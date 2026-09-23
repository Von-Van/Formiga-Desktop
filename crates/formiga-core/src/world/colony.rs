use super::*;

impl World {
    pub fn rename_creature(
        &mut self,
        creature_id: CreatureId,
        name: &str,
    ) -> Result<(), CreatureNameError> {
        let name = validate_creature_name(name)?;
        if let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == creature_id)
        {
            creature.name = name;
        }
        Ok(())
    }

    pub fn mark_profile_viewed(&mut self, creature_id: CreatureId) -> bool {
        if let Some(creature) = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == creature_id)
        {
            if creature.memory.viewed_profile_revision == creature.memory.profile_revision {
                return false;
            }
            creature.memory.viewed_profile_revision = creature.memory.profile_revision;
            return true;
        }
        false
    }

    pub fn set_creature_kept(
        &mut self,
        creature_id: CreatureId,
        kept: bool,
    ) -> Result<(), ColonyManagementError> {
        let creature = self
            .save
            .creatures
            .iter_mut()
            .find(|creature| creature.id == creature_id)
            .ok_or(ColonyManagementError::CreatureNotFound)?;
        creature.kept = kept;
        Ok(())
    }

    /// Import the exact source appearance/personality while assigning fresh local history.
    pub fn adopt_shared_creature(
        &mut self,
        shared: SharedCreatureSeed,
        replace: Option<CreatureId>,
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
    ) -> Result<CreatureId, ColonyManagementError> {
        let mut incoming = Self::from_shared_creature(shared, now, desktop)
            .save
            .creatures
            .remove(0);
        let replaced = if let Some(id) = replace {
            let index = self
                .save
                .creatures
                .iter()
                .position(|c| c.id == id)
                .ok_or(ColonyManagementError::CreatureNotFound)?;
            let old = &self.save.creatures[index];
            if old.kept {
                return Err(ColonyManagementError::CreatureKept);
            }
            if !old.role.is_adult() && adult_count(&self.save.creatures) >= MAX_ADULT_CREATURES {
                return Err(ColonyManagementError::AdultLimit);
            }
            Some((index, old.colony_order))
        } else {
            if self.save.creatures.len() >= MAX_COLONY_CREATURES {
                return Err(ColonyManagementError::ColonyFull);
            }
            if adult_count(&self.save.creatures) >= MAX_ADULT_CREATURES {
                return Err(ColonyManagementError::AdultLimit);
            }
            None
        };
        if self
            .save
            .creatures
            .iter()
            .any(|c| Some(c.id) != replace && c.id == incoming.id)
        {
            return Err(ColonyManagementError::DuplicateIdentity);
        }
        incoming.colony_order = replaced.map_or_else(
            || next_colony_order(&self.save.creatures),
            |(_, colony_order)| colony_order,
        );
        if self
            .save
            .creatures
            .iter()
            .any(|c| Some(c.id) != replace && c.name == incoming.name)
        {
            incoming.name = format!(
                "{} {}",
                incoming.name.chars().take(18).collect::<String>(),
                incoming.colony_order + 1
            );
        }
        let id = incoming.id;
        if let Some((index, _)) = replaced {
            return Ok(self.replace_creature_at(index, incoming));
        }
        self.register_creature_runtime(&incoming);
        self.save.creatures.push(incoming);
        rebalance_minis(&mut self.save.creatures);
        normalize_relationships(&mut self.save);
        Self::emit(
            &mut self.events,
            WorldEvent::CreatureSpawned { creature_id: id },
        );
        Ok(id)
    }

    pub fn preview_adult(
        source_seed: [u8; 32],
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
    ) -> Creature {
        generated_adult(source_seed, now, desktop, 0, &[], true)
    }

    /// Set where a companion's owner would like it to roam. Returns whether anything changed.
    pub fn set_roaming_leaning(
        &mut self,
        creature_id: CreatureId,
        leaning: RoamingLeaning,
    ) -> bool {
        let Some(creature) = creature_mut(&mut self.save.creatures, creature_id) else {
            return false;
        };
        let changed = creature.leaning != leaning;
        creature.leaning = leaning;
        changed
    }

    /// Dress a companion in something made from a find, pin a find on it, or with `None` take
    /// off whatever it is wearing. Only a member of the colony can be dressed, and only in
    /// something the colony has found. Returns whether anything changed.
    pub fn set_accessory(
        &mut self,
        creature_id: CreatureId,
        accessory: Option<Accessory>,
    ) -> Result<bool, AccessoryError> {
        if let Some(accessory) = accessory
            && !accessory.available(&self.save.companion.scrapbook)
        {
            return Err(AccessoryError::NotFound);
        }
        let creature = creature_mut(&mut self.save.creatures, creature_id)
            .ok_or(AccessoryError::UnknownCreature(creature_id))?;
        let changed = creature.accessory != accessory;
        creature.accessory = accessory;
        Ok(changed)
    }

    pub fn add_designed_adult(
        &mut self,
        source_seed: [u8; 32],
        design: Option<CreatureDesign>,
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
    ) -> Result<CreatureId, ColonyManagementError> {
        if self.save.creatures.len() >= MAX_COLONY_CREATURES {
            return Err(ColonyManagementError::ColonyFull);
        }
        if adult_count(&self.save.creatures) >= MAX_ADULT_CREATURES {
            return Err(ColonyManagementError::AdultLimit);
        }
        let existing_names: Vec<_> = self
            .save
            .creatures
            .iter()
            .map(|creature| creature.name.clone())
            .collect();
        let order = next_colony_order(&self.save.creatures);
        let mut creature = generated_adult(source_seed, now, desktop, order, &existing_names, true);
        if let Some(design) = design {
            apply_creature_design(&mut creature, Some(design));
        }
        if self
            .save
            .creatures
            .iter()
            .any(|existing| existing.id == creature.id)
        {
            return Err(ColonyManagementError::DuplicateIdentity);
        }
        let id = creature.id;
        self.register_creature_runtime(&creature);
        self.save.creatures.push(creature);
        rebalance_minis(&mut self.save.creatures);
        normalize_relationships(&mut self.save);
        Self::emit(
            &mut self.events,
            WorldEvent::CreatureSpawned { creature_id: id },
        );
        Ok(id)
    }

    pub fn replace_creature_with_adult(
        &mut self,
        creature_id: CreatureId,
        source_seed: [u8; 32],
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
    ) -> Result<CreatureId, ColonyManagementError> {
        self.replace_creature_with_design(creature_id, source_seed, None, now, desktop)
    }

    pub fn replace_creature_with_design(
        &mut self,
        creature_id: CreatureId,
        source_seed: [u8; 32],
        design: Option<CreatureDesign>,
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
    ) -> Result<CreatureId, ColonyManagementError> {
        let index = self
            .save
            .creatures
            .iter()
            .position(|creature| creature.id == creature_id)
            .ok_or(ColonyManagementError::CreatureNotFound)?;
        if self.save.creatures[index].kept {
            return Err(ColonyManagementError::CreatureKept);
        }
        let old = &self.save.creatures[index];
        let colony_order = old.colony_order;
        if !old.role.is_adult() && adult_count(&self.save.creatures) >= MAX_ADULT_CREATURES {
            return Err(ColonyManagementError::AdultLimit);
        }
        let existing_names: Vec<_> = self
            .save
            .creatures
            .iter()
            .filter(|creature| creature.id != creature_id)
            .map(|creature| creature.name.clone())
            .collect();
        let mut replacement = generated_adult(
            source_seed,
            now,
            desktop,
            colony_order,
            &existing_names,
            true,
        );
        if let Some(design) = design {
            apply_creature_design(&mut replacement, Some(design));
        }
        if self
            .save
            .creatures
            .iter()
            .any(|existing| existing.id != creature_id && existing.id == replacement.id)
        {
            return Err(ColonyManagementError::DuplicateIdentity);
        }
        Ok(self.replace_creature_at(index, replacement))
    }

    pub fn remove_colony_creature(
        &mut self,
        creature_id: CreatureId,
    ) -> Result<(), ColonyManagementError> {
        let index = self
            .save
            .creatures
            .iter()
            .position(|creature| creature.id == creature_id)
            .ok_or(ColonyManagementError::CreatureNotFound)?;
        if self.save.creatures[index].role.is_adult() && adult_count(&self.save.creatures) == 1 {
            return Err(ColonyManagementError::LastAdult);
        }
        let colony_house_keeper = house_owners(&self.save.creatures, &self.save.home.cottage_order)
            .as_slice()
            .first()
            .copied();
        self.save.creatures.remove(index);
        self.save.home.cottage_order.retain(|id| *id != creature_id);
        self.save
            .home
            .house_styles
            .retain(|choice| choice.keeper != creature_id);
        // A cottage's decorations go with it. The colony house stays, and so does everything
        // hung on it: they pass to whoever keeps it now.
        let successor = house_owners(&self.save.creatures, &self.save.home.cottage_order)
            .as_slice()
            .first()
            .copied();
        let home = &mut self.save.home;
        if colony_house_keeper == Some(creature_id)
            && let Some(successor) = successor
            && !home
                .dressing
                .iter()
                .any(|dressing| dressing.keeper == successor)
        {
            for dressing in &mut home.dressing {
                if dressing.keeper == creature_id {
                    dressing.keeper = successor;
                }
            }
        }
        home.dressing
            .retain(|dressing| dressing.keeper != creature_id);
        self.remove_creature_runtime(creature_id);
        rebalance_minis(&mut self.save.creatures);
        normalize_relationships(&mut self.save);
        Ok(())
    }

    /// Start the unkept members of the colony over: every unkept adult is replaced by a fresh
    /// one drawn from `source_seeds`, and every unkept mini simply leaves.
    ///
    /// Returns how many creatures the colony gained, lost or swapped, which is what the caller
    /// needs in order to decide whether anything is worth writing down and redrawing. Running
    /// out of seeds only costs the adults that had none left: the minis owe nothing to a seed
    /// and still go.
    pub fn regenerate_unkept(
        &mut self,
        source_seeds: &[[u8; 32]],
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
    ) -> usize {
        let targets: Vec<_> = self
            .save
            .creatures
            .iter()
            .filter(|creature| !creature.kept)
            .map(|creature| (creature.id, creature.role.is_adult()))
            .collect();
        let mut changed = 0;
        let mut seed_index = 0;
        for (creature_id, is_adult) in targets {
            if is_adult {
                let Some(seed) = source_seeds.get(seed_index).copied() else {
                    continue;
                };
                seed_index += 1;
                if self
                    .replace_creature_with_adult(creature_id, seed, now, desktop)
                    .is_ok()
                {
                    changed += 1;
                }
            } else if self.remove_colony_creature(creature_id).is_ok() {
                changed += 1;
            }
        }
        changed
    }

    /// Hand a creature's place in the colony to `replacement`: the same slot in `creatures`,
    /// which is both draw order and the order the colony is listed in, the same spot on screen,
    /// and the same minis calling it a parent. A swap that removed and appended instead would
    /// quietly send the creature to the back of the group every time it was redesigned.
    ///
    /// Everything the departing creature was part-way through is dropped and the newcomer gets
    /// its own runtime, so no plan survives pointing at a creature that no longer exists.
    fn replace_creature_at(&mut self, index: usize, mut replacement: Creature) -> CreatureId {
        let old = &self.save.creatures[index];
        let old_id = old.id;
        let new_id = replacement.id;
        replacement.state.position = old.state.position;
        replacement.state.surface = old.state.surface.clone();
        for creature in &mut self.save.creatures {
            if creature.role.parent_id() == Some(old_id) {
                creature.role = CreatureRole::Mini { parent_id: new_id };
            }
        }
        // The newcomer keeps the cottage where it stood, and whatever it was built as.
        for id in &mut self.save.home.cottage_order {
            if *id == old_id {
                *id = new_id;
            }
        }
        for choice in &mut self.save.home.house_styles {
            if choice.keeper == old_id {
                choice.keeper = new_id;
            }
        }
        for dressing in &mut self.save.home.dressing {
            if dressing.keeper == old_id {
                dressing.keeper = new_id;
            }
        }
        self.remove_creature_runtime(old_id);
        self.register_creature_runtime(&replacement);
        self.save.creatures[index] = replacement;
        rebalance_minis(&mut self.save.creatures);
        normalize_relationships(&mut self.save);
        Self::emit(
            &mut self.events,
            WorldEvent::CreatureSpawned {
                creature_id: new_id,
            },
        );
        new_id
    }

    /// Settle everything: release every plan the colony is part-way through that lives only in
    /// memory — a journey between windows and the route it belonged to, a throw still in the
    /// air, a chosen action, a visit to another creature, and any attention scene along with
    /// whoever had stopped to watch it. Creatures are left standing exactly where they are; the
    /// caller decides where they go next, and nothing saved is touched.
    ///
    /// The village appearing, a quiet spell starting and a gather asked for at the desk are the
    /// same moment told three ways, so they all end up here rather than each clearing whichever
    /// plans its author happened to think of.
    pub(super) fn clear_runtime_plans(&mut self) {
        self.drop_all_tows();
        self.window_journeys.clear();
        self.window_routes.clear();
        self.tosses.clear();
        self.action_choices.clear();
        self.bond_plans.clear();
        self.clear_attention();
        self.end_village_life();
        self.clear_beats();
    }

    pub(super) fn register_creature_runtime(&mut self, creature: &Creature) {
        self.rngs
            .insert(creature.id, ChaCha12Rng::from_seed(creature.behavior_seed));
        self.ambient_timers.insert(
            creature.id,
            AmbientTimers {
                inspect_remaining: self.ambient_rng.random_range(INSPECT_INTERVAL_SECS),
                dangle_remaining: self.ambient_rng.random_range(DANGLE_INTERVAL_SECS),
                climb_rest: 0.0,
            },
        );
    }

    pub(super) fn remove_creature_runtime(&mut self, creature_id: CreatureId) {
        self.forget_tows_of(creature_id);
        self.cancel_creature_attention(creature_id);
        let mut interrupted: BTreeSet<_> = self
            .action_choices
            .iter()
            .filter(|(_, choice)| choice.target_creature == Some(creature_id))
            .map(|(actor, _)| *actor)
            .chain(
                self.bond_plans
                    .iter()
                    .filter(|(_, plan)| plan.target == creature_id)
                    .map(|(actor, _)| *actor),
            )
            .collect();
        if self.colony_plan.as_ref().is_some_and(|plan| {
            plan.participants
                .iter()
                .any(|participant| participant.creature_id == creature_id)
        }) && let Some(plan) = self.colony_plan.take()
        {
            interrupted.extend(
                plan.participants
                    .into_iter()
                    .map(|participant| participant.creature_id),
            );
        }
        for actor in interrupted {
            if let Some(creature) = self
                .save
                .creatures
                .iter_mut()
                .find(|creature| creature.id == actor)
            {
                creature.state.action = ActionKind::Idle;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = 1.0;
            }
        }
        self.rngs.remove(&creature_id);
        self.ambient_timers.remove(&creature_id);
        self.village_life.remove(&creature_id);
        self.window_journeys.remove(&creature_id);
        self.window_routes.remove(&creature_id);
        self.tosses.remove(&creature_id);
        self.action_choices.retain(|actor, choice| {
            *actor != creature_id && choice.target_creature != Some(creature_id)
        });
        self.bond_plans
            .retain(|actor, plan| *actor != creature_id && plan.target != creature_id);
        self.sleep_elapsed.remove(&creature_id);
        self.calm_proximity_seconds
            .retain(|(a, b), _| *a != creature_id && *b != creature_id);
        self.pending_home_greetings.remove(&creature_id);
        self.reacted_to_toss
            .retain(|(actor, target)| *actor != creature_id && *target != creature_id);
        self.watched_climb
            .retain(|(actor, target)| *actor != creature_id && *target != creature_id);
        if self
            .interaction
            .as_ref()
            .is_some_and(|interaction| interaction.creature_id == creature_id)
        {
            self.interaction = None;
        }
    }

    pub fn from_shared_creature(
        shared: SharedCreatureSeed,
        now: OffsetDateTime,
        desktop: &DesktopSnapshot,
    ) -> Self {
        let colony_seed = derive_imported_colony_seed(shared);
        let mut colony = Self::new(colony_seed, now, desktop);
        let spawn_state = colony.save.creatures[0].state.clone();
        let mut creature = generate_source_creature(shared, now, desktop);
        creature.generation = 0;
        creature.colony_order = 0;
        creature.born_at_utc = now;
        creature.memory = CreatureMemory::default();
        creature.tendencies = LearnedTendencies::default();
        creature.routines = RoutineTable::default();
        creature.role = CreatureRole::Adult;
        creature.kept = true;
        creature.mini_arrivals = MiniArrivalState {
            enabled: true,
            arrived: [false; 2],
        };
        creature.state = spawn_state;
        colony.save.companion.journal.clear();
        colony
            .save
            .companion
            .remember(Some(creature.id), crate::JournalMoment::Arrival, now);
        colony.save.creatures[0] = creature;
        colony.save.arrival_state.arrived[0] = true;
        colony.save.arrival_state.arrived[1] = true;
        Self::from_save(colony.save)
    }
}

pub(super) fn normalize_colony_roles(save: &mut SaveFile) {
    if save.creatures.is_empty() {
        return;
    }
    if !save
        .creatures
        .iter()
        .any(|creature| creature.role.is_adult())
    {
        save.creatures[0].role = CreatureRole::Adult;
    }
    rebalance_minis(&mut save.creatures);
}

pub(super) fn adult_count(creatures: &[Creature]) -> usize {
    creatures
        .iter()
        .filter(|creature| creature.role.is_adult())
        .count()
}

pub(super) fn mini_count_for_parent(creatures: &[Creature], parent_id: CreatureId) -> usize {
    creatures
        .iter()
        .filter(|creature| creature.role.parent_id() == Some(parent_id))
        .count()
}

pub(super) fn balanced_parent_id(creatures: &[Creature]) -> Option<CreatureId> {
    creatures
        .iter()
        .filter(|creature| creature.role.is_adult())
        .filter(|creature| mini_count_for_parent(creatures, creature.id) < MAX_MINIS_PER_ADULT)
        .min_by_key(|creature| {
            (
                mini_count_for_parent(creatures, creature.id),
                creature.born_at_utc,
                creature.colony_order,
            )
        })
        .map(|creature| creature.id)
}

pub(super) fn next_colony_order(creatures: &[Creature]) -> u8 {
    creatures
        .iter()
        .map(|creature| creature.colony_order)
        .max()
        .unwrap_or_default()
        .saturating_add(1)
}

pub(super) fn rebalance_minis(creatures: &mut [Creature]) {
    let mut adults: Vec<_> = creatures
        .iter()
        .filter(|creature| creature.role.is_adult())
        .map(|creature| (creature.born_at_utc, creature.colony_order, creature.id))
        .collect();
    adults.sort_by_key(|adult| *adult);
    if adults.is_empty() {
        return;
    }
    let mut minis: Vec<_> = creatures
        .iter()
        .filter(|creature| !creature.role.is_adult())
        .map(|creature| (creature.born_at_utc, creature.colony_order, creature.id))
        .collect();
    minis.sort_by_key(|mini| *mini);
    for (index, (_, _, mini_id)) in minis.into_iter().enumerate() {
        let parent_id = adults[index % adults.len()].2;
        if let Some(mini) = creatures.iter_mut().find(|creature| creature.id == mini_id) {
            mini.role = CreatureRole::Mini { parent_id };
        }
    }
}
