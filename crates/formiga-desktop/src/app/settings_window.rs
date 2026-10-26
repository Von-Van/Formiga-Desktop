//! The settings window: opening it, and acting on what it asks for, from colony changes and exports
//! to restores, with the notices and errors it shows.
use super::*;

impl FormigaApp {
    /// Remember where the notebook window is, for the next time it opens. Failing to is only
    /// logged: the notebook simply opens where it would have the first time.
    pub(super) fn remember_notebook_geometry(&mut self) {
        let Some(geometry) = self
            .settings_window
            .as_ref()
            .and_then(SettingsWindow::geometry)
        else {
            return;
        };
        if self.notebook_geometry == Some(geometry) {
            return;
        }
        match notebook_window::save(&self.data_dir, geometry) {
            Ok(()) => self.notebook_geometry = Some(geometry),
            Err(error) => tracing::warn!(%error, "could not remember the notebook window"),
        }
    }

    pub(super) fn show_settings(&mut self, event_loop: &ActiveEventLoop) {
        let Some((settings, creatures, relationships)) = self.world.as_ref().map(|world| {
            (
                world.save.settings.clone(),
                world.save.creatures.clone(),
                world.save.relationships.clone(),
            )
        }) else {
            return;
        };
        // An app installed since the last look is offered as soon as the notebook opens.
        self.look_for_apps_now();
        if self.settings_window.is_none() {
            let screens: Vec<notebook_window::Screen> = {
                let primary = event_loop.primary_monitor();
                event_loop
                    .available_monitors()
                    .map(|monitor| {
                        let (position, size) = (monitor.position(), monitor.size());
                        notebook_window::Screen {
                            x: position.x,
                            y: position.y,
                            width: size.width,
                            height: size.height,
                            scale: monitor.scale_factor(),
                            primary: primary.as_ref().is_some_and(|main| {
                                main.position() == position && main.size() == size
                            }),
                        }
                    })
                    .collect()
            };
            let placement = notebook_window::place(notebook_window::load(&self.data_dir), &screens);
            match pollster::block_on(SettingsWindow::new(
                event_loop,
                &settings,
                &creatures,
                &relationships,
                self.save_store.path(),
                placement,
                self.event_proxy.clone(),
            )) {
                Ok(window) => self.settings_window = Some(window),
                Err(error) => {
                    tracing::error!(%error, "could not create settings window");
                    return;
                }
            }
        }
        if let Some(window) = &mut self.settings_window {
            window.show(&settings, &creatures, &relationships);
        }
    }

    pub(super) fn finish_settings_change(&mut self, previous_launch: bool) -> Result<()> {
        let mut failure = None;
        {
            let Some(world) = &mut self.world else {
                return Ok(());
            };
            if world.save.settings.launch_at_login != previous_launch
                && let Err(error) =
                    platform::set_launch_at_login(world.save.settings.launch_at_login)
            {
                tracing::error!(%error, "could not update launch-at-login");
                world.save.settings.launch_at_login = previous_launch;
                failure = Some(error);
            }
            if let Some(tray) = &self.tray {
                tray.sync(&world.save.settings);
            }
        }
        self.sync_overlay_visibility();
        for overlay in self.overlays.values() {
            if overlay.is_visible() {
                overlay.window.request_redraw();
            }
        }
        self.redraw_due = Instant::now();
        self.save()?;
        if let Some(error) = failure {
            return Err(error.context("Could not update launch at login"));
        }
        Ok(())
    }

    /// Act on everything the settings window asked for, one method per request, in the order
    /// they have always run. The order matters: the colony's own changes are gathered into one
    /// save with one notice, and every later request sees the colony as those changes left it.
    pub(super) fn handle_settings_outcome(
        &mut self,
        event_loop: &ActiveEventLoop,
        outcome: SettingsOutcome,
    ) {
        self.handle_backup_requests(&outcome);
        self.open_requested_companion_apps(&outcome);
        self.apply_colony_edits(&outcome);
        self.export_requested_pictures(&outcome);
        if let Some((creature_id, kept)) = outcome.set_creature_kept {
            self.keep_creature(creature_id, kept);
        }
        if let Some(creature_id) = outcome.remove_creature {
            self.remove_creature_from_colony(creature_id);
        }
        if let Some(shared) = outcome.preview_shared {
            self.preview_shared_creature(shared);
        }
        if outcome.request_random_creature {
            self.preview_random_creatures();
        }
        if let Some(path) = self.reference_image_to_match(&outcome) {
            self.preview_reference_matches(path);
        }
        if let Some(acceptance) = outcome.accept_creature_preview {
            self.welcome_previewed_creature(acceptance);
        }
        if let Some(shared) = outcome.invite_visitor {
            self.invite_shared_visitor(shared);
        }
        if outcome.ask_visitor_to_stay {
            self.let_visitor_stay();
        }
        if outcome.regenerate_unkept {
            self.regenerate_unkept_creatures();
        }
        if outcome.undo_last_edit {
            self.undo_last_colony_edit();
        }
        self.save_profile_changes(outcome.rename_creature, outcome.viewed_profile);
        if let Some(settings) = outcome.applied {
            self.apply_preferences(settings);
        }
        if outcome.gather {
            self.gather_colony();
        }
        if outcome.open_logs {
            self.open_log_directory();
        }
        if let Some(enabled) = outcome.automatic_update_checks {
            self.set_automatic_update_checks(enabled);
        }
        if let Some(on) = outcome.echo_fox {
            self.toggle_echo_fox(on);
        }
        if outcome.check_updates {
            self.start_update_check();
        }
        if outcome.download_update {
            self.start_update_download();
        }
        if outcome.install_update {
            self.install_downloaded_update(event_loop);
        }
        if outcome.browse_application {
            self.browse_for_application();
        }
        if let Some(draft) = outcome.edit_habitat {
            self.begin_habitat_editor(draft);
        }
        if outcome.apply_habitat_edit {
            self.finish_habitat_editor(true);
        }
        if outcome.cancel_habitat_edit {
            self.finish_habitat_editor(false);
        }
        if outcome.reset_habitat_edit {
            self.reset_habitat_draft();
        }
    }

    /// The About & backups page's requests: an export, a restore, keeping the new colony after a
    /// recovery, and trying a failed save again.
    fn handle_backup_requests(&mut self, outcome: &SettingsOutcome) {
        if outcome.export_colony {
            self.export_colony();
        }
        if outcome.restore_colony {
            self.restore_colony();
        }
        if outcome.start_fresh_recovery && self.recovery_pending {
            match self.save_store.preserve_recovery_files() {
                Ok(_) => {
                    self.recovery_pending = false;
                    if let Some(window) = &mut self.settings_window {
                        window.clubhouse.recovery.reason = None;
                    }
                    self.save_with_feedback(
                        "New colony saved · original files preserved as recovery copies",
                    );
                }
                Err(error) => {
                    self.settings_error(format!("Could not preserve recovery files: {error}"))
                }
            }
        }
        if outcome.retry_save {
            match self.save() {
                Ok(()) => {}
                Err(error) => tracing::warn!(%error, "colony save still failing"),
            }
        }
    }

    /// A visit to a companion app the notebook asked for: a house in Formiga Home, the household
    /// coming back, a session in Formiga Farm, or the train to Formiga Hill or Formiga Battle.
    fn open_requested_companion_apps(&mut self, outcome: &SettingsOutcome) {
        if let Some(keeper) = outcome.open_house {
            self.open_house(keeper);
        }
        if outcome.bring_household_back {
            self.bring_household_back();
        }
        if let Some(request) = outcome.open_farm {
            self.open_farm(request);
        }
        match outcome.open_app {
            Some(crate::clubhouse::AppOpen::Hill) => self.go_to_hill(),
            Some(crate::clubhouse::AppOpen::Battle) => self.play_battle(),
            None => {}
        }
    }

    /// The colony's own changes, each made through `World`, then saved once with one notice. The
    /// branches share what they gather: whether anything changed, and the one change said in its
    /// own words, which a later branch may replace, as it always has. Each does nothing without a
    /// world.
    fn apply_colony_edits(&mut self, outcome: &SettingsOutcome) {
        let mut companion_changed = false;
        // A change to what the companions will do, said in its own words rather than as a plain
        // "saved": see `notices`.
        let mut specific_notice: Option<String> = None;
        let local = OffsetDateTime::now_utc().to_offset(crate::clubhouse::local_offset());
        self.edit_quiet_mode_and_onboarding(
            outcome,
            local,
            &mut companion_changed,
            &mut specific_notice,
        );
        self.move_home(outcome, &mut companion_changed, &mut specific_notice);
        self.edit_village_decorations(outcome, &mut companion_changed);
        self.edit_hangouts_and_houses(outcome, &mut companion_changed, &mut specific_notice);
        self.edit_gardens_and_layout(outcome, &mut companion_changed, &mut specific_notice);
        self.edit_routine(outcome, local, &mut companion_changed, &mut specific_notice);
        self.edit_journal(outcome, &mut companion_changed);
        self.edit_visitors_and_leanings(outcome, &mut companion_changed, &mut specific_notice);
        if companion_changed {
            self.save_with_feedback(specific_notice.as_deref().unwrap_or("Colony changes saved"));
            self.redraw_due = Instant::now();
            for overlay in self.overlays.values() {
                overlay.window.request_redraw();
            }
        }
    }

    /// Quiet mode for some minutes, said with when it ends, and the end of onboarding.
    fn edit_quiet_mode_and_onboarding(
        &mut self,
        outcome: &SettingsOutcome,
        local: OffsetDateTime,
        changed: &mut bool,
        notice: &mut Option<String>,
    ) {
        let Some(world) = &mut self.world else {
            return;
        };
        if let Some(minutes) = outcome.quiet_minutes {
            world.set_quiet_mode(minutes, OffsetDateTime::now_utc());
            *changed = true;
            let until = world
                .save
                .companion
                .quiet_until
                .map(|until| until.to_offset(local.offset()));
            *notice = Some(crate::notices::quiet(minutes, until));
        }
        if outcome.complete_onboarding {
            world.save.companion.onboarding_complete = true;
            *changed = true;
        }
    }

    /// The home moved to another corner or another display. Every change to how the village is
    /// laid out goes through `World::edit`, so the last one can be taken back.
    fn move_home(
        &mut self,
        outcome: &SettingsOutcome,
        changed: &mut bool,
        notice: &mut Option<String>,
    ) {
        let Some(world) = &mut self.world else {
            return;
        };
        if let Some(corner) = outcome.home_corner {
            world.edit(ColonyEdit::MovedHome, |world| {
                world.save.home.corner = corner
            });
            *changed = true;
            *notice = Some(crate::notices::home_moved(Some(corner), None));
        }
        if let Some(display) = outcome.home_display {
            world.edit(ColonyEdit::MovedHome, |world| {
                world.save.home.display = Some(display);
            });
            *changed = true;
            let label = crate::settings::display_label(display, &self.monitors);
            *notice = Some(crate::notices::home_moved(None, Some(&label)));
        }
    }

    /// Decorations on a house, an ornament, the keepsakes on the tree, and what a companion wears:
    /// none has anything to say beyond "saved".
    fn edit_village_decorations(&mut self, outcome: &SettingsOutcome, changed: &mut bool) {
        let Some(world) = &mut self.world else {
            return;
        };
        if let Some((keeper, slot, kind)) = outcome.set_decoration {
            *changed |= world.edit(ColonyEdit::Decorations, |world| {
                world.save.home.set_decoration(keeper, slot, kind)
            });
        }
        if let Some((kind, along)) = outcome.set_ornament {
            *changed |= world.edit(ColonyEdit::Ornament(kind), |world| {
                world.save.home.set_ornament(kind, along)
            });
        }
        if let Some(hooks) = outcome.tree_keepsakes {
            world.edit(ColonyEdit::TreeKeepsakes, |world| {
                world.save.home.set_tree_keepsakes(hooks);
            });
            *changed = true;
        }
        if let Some((creature_id, accessory)) = outcome.set_accessory {
            match world.set_accessory(creature_id, accessory) {
                Ok(changed_accessory) => *changed |= changed_accessory,
                Err(error) => {
                    if let Some(window) = &mut self.settings_window {
                        window.set_error(error.to_string());
                    }
                }
            }
        }
    }

    /// A hangout put down, moved or taken up (said in its own words), the cottages' order, and
    /// each house's style, depth and the village's paint.
    fn edit_hangouts_and_houses(
        &mut self,
        outcome: &SettingsOutcome,
        changed: &mut bool,
        notice: &mut Option<String>,
    ) {
        let Some(world) = &mut self.world else {
            return;
        };
        if let Some((kind, along)) = outcome.set_hangout {
            let before = world.save.home.hangout(kind).map(|spot| spot.along);
            let moved = world.edit(ColonyEdit::Hangout(kind), |world| {
                world.save.home.set_hangout(kind, along)
            });
            *changed |= moved;
            if moved {
                let after = world.save.home.hangout(kind).map(|spot| spot.along);
                *notice = crate::notices::hangout(kind, before, after).or(notice.take());
            }
        }
        if let Some(order) = outcome.cottage_order.clone() {
            world.edit(ColonyEdit::MovedCottages, |world| {
                let creatures = world.save.creatures.clone();
                world.save.home.arrange_cottages(order, &creatures);
            });
            *changed = true;
        }
        if let Some((keeper, style)) = outcome.house_style {
            world.edit(ColonyEdit::HouseType, |world| {
                world.save.home.set_house_style(keeper, style);
            });
            *changed = true;
        }
        if let Some((keeper, depth)) = outcome.house_depth {
            world.edit(ColonyEdit::HouseDepth, |world| {
                world.save.home.set_house_depth(keeper, depth);
            });
            *changed = true;
        }
        if let Some(palette) = outcome.village_palette {
            world.edit(ColonyEdit::PaintedVillage, |world| {
                world.save.home.palette = palette;
            });
            *changed = true;
        }
    }

    /// A garden patch planted or taken up (said only when the patch comes or goes), the village
    /// put back as it was, and two keepsakes swapped.
    fn edit_gardens_and_layout(
        &mut self,
        outcome: &SettingsOutcome,
        changed: &mut bool,
        notice: &mut Option<String>,
    ) {
        let Some(world) = &mut self.world else {
            return;
        };
        if let Some((kind, along)) = outcome.set_garden {
            let was_planted = world.save.home.gardens.iter().any(|p| p.kind == kind);
            let replanted = world.edit(ColonyEdit::Garden(kind), |world| {
                // A patch put down now starts growing from now, by the colony's own clock.
                let now = world.save.maximum_seen_utc;
                world.save.home.set_garden(kind, along, now)
            });
            *changed |= replanted;
            let planted = world.save.home.gardens.iter().any(|p| p.kind == kind);
            if replanted && planted != was_planted {
                *notice = Some(crate::notices::garden(kind, planted));
            }
        }
        if outcome.reset_village {
            world.edit(ColonyEdit::PutVillageBack, |world| {
                world.save.home.reset_arrangement();
            });
            *changed = true;
        }
        if let Some((a, b)) = outcome.move_object
            && a < world.save.objects.objects.len()
            && b < world.save.objects.objects.len()
        {
            world.edit(ColonyEdit::RearrangedKeepsakes, |world| {
                world.save.objects.objects.swap(a, b);
            });
            *changed = true;
        }
    }

    /// The routine: a mode kept, the appearance preferences, a schedule edited by hand, and the
    /// routine resumed after an override, each said with what it will do.
    fn edit_routine(
        &mut self,
        outcome: &SettingsOutcome,
        local: OffsetDateTime,
        changed: &mut bool,
        notice: &mut Option<String>,
    ) {
        let Some(world) = &mut self.world else {
            return;
        };
        if let Some((index, preset)) = &outcome.save_mode
            && *index < 2
        {
            world.save.companion.modes[*index] = Some(preset.clone());
            *changed = true;
            *notice = Some(crate::notices::routine_kept(*index));
        }
        if let Some(appearance) = outcome.appearance {
            world.save.companion.appearance = appearance;
            world.save.companion.appearance.normalize();
            *changed = true;
        }
        if let Some(schedule) = outcome.schedule.clone() {
            world.save.companion.schedule = schedule;
            world.save.companion.normalize();
            // Editing the routine by hand is the reader speaking last.
            world.override_routine();
            *changed = true;
            *notice = Some(crate::notices::routine_saved(
                &world.save.companion.schedule,
                local,
            ));
        }
        if outcome.resume_routine {
            world.resume_routine(OffsetDateTime::now_utc());
            *changed = true;
            *notice = Some(crate::notices::routine_resumed(
                &world.save.companion.schedule,
                local,
            ));
        }
    }

    /// The journal read, and a moment pinned or unpinned.
    fn edit_journal(&mut self, outcome: &SettingsOutcome, changed: &mut bool) {
        let Some(world) = &mut self.world else {
            return;
        };
        // Reading the journal changes nothing worth a notice: it is written with the next
        // routine checkpoint, quietly.
        if outcome.mark_journal_read && world.save.companion.mark_read() {
            self.save_waiting = self.save_waiting.max(SaveUrgency::Routine);
        }
        if let Some(entry) = &outcome.pin_moment {
            *changed |= world.save.companion.pin(entry);
        }
        if let Some(entry) = &outcome.unpin_moment {
            world.save.companion.unpin(entry);
            *changed = true;
        }
    }

    /// A visitor kept as a favorite or forgotten, and a companion's roaming leaning, said with
    /// the change from what it was.
    fn edit_visitors_and_leanings(
        &mut self,
        outcome: &SettingsOutcome,
        changed: &mut bool,
        notice: &mut Option<String>,
    ) {
        let Some(world) = &mut self.world else {
            return;
        };
        if let Some((name, origin)) = &outcome.keep_favorite_visitor {
            match world
                .save
                .visitors
                .keep_favorite(name, *origin, OffsetDateTime::now_utc())
            {
                Ok(()) => *changed = true,
                Err(error) => {
                    if let Some(window) = &mut self.settings_window {
                        window.set_error(error.to_string());
                    }
                }
            }
        }
        if let Some(origin) = &outcome.forget_favorite_visitor {
            *changed |= world.save.visitors.forget_favorite(origin);
        }
        if let Some((creature_id, leaning)) = outcome.set_roaming_leaning {
            let before = world
                .save
                .creatures
                .iter()
                .find(|c| c.id == creature_id)
                .map(|c| c.leaning);
            if world.set_roaming_leaning(creature_id, leaning)
                && let Some(before) = before
                && let Some(creature) = world.save.creatures.iter().find(|c| c.id == creature_id)
            {
                *changed = true;
                *notice = Some(crate::notices::leaning(&creature.name, before, leaning));
            }
        }
    }

    /// The pictures the notebook exports: a creature card, a sticker, the colony portrait and a
    /// postcard. Each asks where to put it first; a cancelled dialog renders nothing.
    fn export_requested_pictures(&mut self, outcome: &SettingsOutcome) {
        if let Some(creature_id) = outcome.export_creature_card
            && let Some(creature) = self.world.as_ref().and_then(|world| {
                world
                    .save
                    .creatures
                    .iter()
                    .find(|creature| creature.id == creature_id)
            })
        {
            let selected = choose_card_destination(creature);
            match export_to_selected_destination(creature, selected) {
                Ok(Some(_)) => self.settings_notice("Creature card exported"),
                Ok(None) => {}
                Err(error) => self.export_failed("the creature card", &error),
            }
        }
        if let Some((creature_id, clip, scale)) = outcome.export_creature_sticker
            && let Some(creature) = self.world.as_ref().and_then(|world| {
                world
                    .save
                    .creatures
                    .iter()
                    .find(|creature| creature.id == creature_id)
            })
        {
            // The dialog runs first; a cancelled one never renders a frame.
            let selected = choose_sticker_destination(creature, clip);
            match export_sticker_to_selected_destination(creature, clip, scale, selected) {
                Ok(Some(_)) => self.settings_notice("Sticker exported"),
                Ok(None) => {}
                Err(error) => self.export_failed("the sticker", &error),
            }
        }
        if outcome.export_colony_card
            && let Some(save) = self.world.as_ref().map(|world| &world.save)
        {
            let selected = choose_colony_card_destination();
            match export_colony_card_to_selected_destination(save, selected) {
                Ok(Some(_)) => self.settings_notice("Colony portrait exported"),
                Ok(None) => {}
                Err(error) => self.export_failed("the colony portrait", &error),
            }
        }
        if let Some((scene, caption)) = outcome.export_postcard.as_ref()
            && let Some(save) = self.world.as_ref().map(|world| &world.save)
        {
            let selected = choose_postcard_destination(*scene);
            match export_postcard_to_selected_destination(save, *scene, caption, selected) {
                Ok(Some(_)) => self.settings_notice("Postcard exported"),
                Ok(None) => {}
                Err(error) => self.export_failed("the postcard", &error),
            }
        }
    }

    /// Keep a companion through a start-over, or stop keeping it; a refused change is shown in
    /// the window.
    fn keep_creature(&mut self, creature_id: CreatureId, kept: bool) {
        let result = self
            .world
            .as_mut()
            .map(|world| world.set_creature_kept(creature_id, kept));
        match result {
            Some(Ok(())) => {
                let _ = self.save();
            }
            Some(Err(error)) => {
                if let Some(window) = &mut self.settings_window {
                    window.set_error(error.to_string());
                }
            }
            None => {}
        }
    }

    /// Remove a companion from the colony as an undoable edit named after it, and redraw so it
    /// is gone from the overlays at once.
    fn remove_creature_from_colony(&mut self, creature_id: CreatureId) {
        let result = self.world.as_mut().map(|world| {
            let name = creature_name(world, creature_id);
            world.edit(ColonyEdit::Removed { name }, |world| {
                world.remove_colony_creature(creature_id)
            })
        });
        match result {
            Some(Ok(())) => {
                let _ = self.save();
                self.redraw_due = Instant::now();
                for overlay in self.overlays.values() {
                    overlay.window.request_redraw();
                }
            }
            Some(Err(error)) => {
                if let Some(window) = &mut self.settings_window {
                    window.set_error(error.to_string());
                }
            }
            None => {}
        }
    }

    /// Show a shared seed code's companion in the studio as the one preview, before it is
    /// welcomed.
    fn preview_shared_creature(&mut self, shared: SharedCreatureSeed) {
        let desktop = self.snapshot();
        let creature = World::from_shared_creature(shared, OffsetDateTime::now_utc(), &desktop)
            .save
            .creatures
            .remove(0);
        if let Some(window) = &mut self.settings_window {
            window.clear_generation_preview();
            window.set_generation_preview(GenerationPreview {
                shared: Some(shared),
                source_seed: shared.source_colony_seed,
                creature,
                similarity: None,
                summary:
                    "An exact shared appearance and personality, with a fresh life in your colony."
                        .into(),
                drawn: None,
            });
        }
    }

    /// Four fresh companions for the studio, keeping whatever the studio has locked (colours,
    /// body) from the previous preview.
    fn preview_random_creatures(&mut self) {
        let (locked, lock_colors, lock_body) = self
            .settings_window
            .as_ref()
            .map(|w| w.clubhouse.studio.locks())
            .unwrap_or((None, false, false));
        let desktop = self.snapshot();
        let seeds: Result<Vec<_>, _> = (0..4).map(|_| new_colony_seed()).collect();
        match seeds {
            Ok(seeds) => {
                if let Some(window) = &mut self.settings_window {
                    window.clear_generation_preview();
                }
                for source_seed in seeds {
                    let mut creature =
                        World::preview_adult(source_seed, OffsetDateTime::now_utc(), &desktop);
                    if let (Some(previous), Some(mut design)) = (locked, creature.appearance.design)
                    {
                        if lock_colors {
                            design.coat = previous.coat;
                            design.accent = previous.accent;
                        }
                        if lock_body {
                            design.body = previous.body;
                            design.width = previous.width;
                            design.height = previous.height;
                            design.head = previous.head;
                            design.legs = previous.legs;
                        }
                        apply_creature_design(&mut creature, Some(design));
                    }
                    if let Some(window) = &mut self.settings_window {
                        window.set_generation_preview(GenerationPreview {
                            shared: None,
                            creature,
                            source_seed,
                            similarity: None,
                            summary: "A new companion with fresh memories.".into(),
                            drawn: None,
                        });
                    }
                }
            }
            Err(error) => self.settings_error(format!("Could not generate companions: {error}")),
        }
    }

    /// The reference image to match: the one the studio already has when a retry is asked for,
    /// or one picked in a file dialog for a new request. None when neither was asked.
    fn reference_image_to_match(&self, outcome: &SettingsOutcome) -> Option<PathBuf> {
        if outcome.request_reference_retry {
            self.settings_window
                .as_ref()
                .and_then(|window| window.clubhouse.studio.reference.clone())
        } else if outcome.request_reference_creature {
            rfd::FileDialog::new()
                .add_filter("Character image", &["png", "jpg", "jpeg"])
                .pick_file()
        } else {
            None
        }
    }

    /// Fill the studio with the takes that come closest to a reference image, and remember the
    /// image so the search can be run again.
    fn preview_reference_matches(&mut self, path: PathBuf) {
        match new_colony_seed() {
            Ok(search_seed) => {
                let desktop = self.snapshot();
                match match_reference_file(&path, search_seed, OffsetDateTime::now_utc(), &desktop)
                {
                    Ok(takes) => {
                        if let Some(window) = &mut self.settings_window {
                            window.clear_generation_preview();
                            for take in takes {
                                window.set_generation_preview(GenerationPreview {
                                    shared: None,
                                    creature: take.creature,
                                    source_seed: take.source_seed,
                                    similarity: Some(take.similarity),
                                    summary: take.summary.to_owned(),
                                    drawn: None,
                                });
                            }
                            window.clubhouse.studio.reference = Some(path);
                        }
                    }
                    Err(error) => {
                        if let Some(window) = &mut self.settings_window {
                            window.set_error(crate::explain::reference_image(&error));
                        }
                    }
                }
            }
            Err(error) => {
                if let Some(window) = &mut self.settings_window {
                    window.set_error(format!("Could not generate a secure seed: {error}"));
                }
            }
        }
    }

    /// Welcome a previewed companion into the colony, or let it replace one, as one undoable edit
    /// named after whoever it replaces.
    fn welcome_previewed_creature(&mut self, acceptance: PreviewAcceptance) {
        let desktop = self.snapshot();
        let now = OffsetDateTime::now_utc();
        let result = self.world.as_mut().map(|world| {
            let replaced = match acceptance {
                PreviewAcceptance::Shared { replace, .. }
                | PreviewAcceptance::Drawn { replace, .. } => replace,
                PreviewAcceptance::Add { .. } => None,
                PreviewAcceptance::Replace { creature_id, .. } => Some(creature_id),
            };
            let edit = match replaced {
                Some(creature_id) => ColonyEdit::Replaced {
                    name: creature_name(world, creature_id),
                },
                None => ColonyEdit::Welcomed,
            };
            world.edit(edit, |world| match acceptance {
                PreviewAcceptance::Shared { shared, replace } => {
                    world.adopt_shared_creature(shared, replace, now, &desktop)
                }
                PreviewAcceptance::Add {
                    source_seed,
                    design,
                } => world.add_designed_adult(source_seed, design, now, &desktop),
                PreviewAcceptance::Replace {
                    creature_id,
                    source_seed,
                    design,
                } => world.replace_creature_with_design(
                    creature_id,
                    source_seed,
                    design,
                    now,
                    &desktop,
                ),
                // Welcomed as any sketch is, and then given Formiga Farm's design the one way
                // Desktop gives any companion one.
                PreviewAcceptance::Drawn {
                    source_seed,
                    recipe,
                    design,
                    replace,
                } => match replace {
                    Some(creature_id) => world.replace_creature_with_design(
                        creature_id,
                        source_seed,
                        recipe,
                        now,
                        &desktop,
                    ),
                    None => world.add_designed_adult(source_seed, recipe, now, &desktop),
                }
                .inspect(|id| {
                    if let Some(creature) = world.save.creatures.iter_mut().find(|c| c.id == *id) {
                        formiga_farm_contract::apply_design(creature, &design);
                    }
                }),
            })
        });
        match result {
            Some(Ok(_)) => {
                if let Some(window) = &mut self.settings_window {
                    window.clear_generation_preview();
                }
                self.save_with_feedback("Companion welcomed into the colony");
                self.redraw_due = Instant::now();
                for overlay in self.overlays.values() {
                    overlay.window.request_redraw();
                }
            }
            Some(Err(error)) => {
                if let Some(window) = &mut self.settings_window {
                    window.set_error(error.to_string());
                }
            }
            None => {}
        }
    }

    /// Invite a friend's shared companion over as a visitor.
    fn invite_shared_visitor(&mut self, shared: SharedCreatureSeed) {
        let desktop = self.snapshot();
        let now = OffsetDateTime::now_utc();
        let result = self
            .world
            .as_mut()
            .map(|world| world.invite_visitor(shared, now, &desktop));
        match result {
            Some(Ok(())) => {
                if let Some(window) = &mut self.settings_window {
                    window.clear_generation_preview();
                }
                self.save_with_feedback("Your friend is on their way over");
                self.redraw_due = Instant::now();
                for overlay in self.overlays.values() {
                    overlay.window.request_redraw();
                }
            }
            Some(Err(error)) => {
                if let Some(window) = &mut self.settings_window {
                    window.set_error(error.to_string());
                }
            }
            None => {}
        }
    }

    /// Ask the current visitor to stay for good.
    fn let_visitor_stay(&mut self) {
        let desktop = self.snapshot();
        let now = OffsetDateTime::now_utc();
        let result = self
            .world
            .as_mut()
            .map(|world| world.ask_visitor_to_stay(now, &desktop));
        match result {
            Some(Ok(_)) => {
                self.save_with_feedback("Your visitor is staying for good");
                self.redraw_due = Instant::now();
                for overlay in self.overlays.values() {
                    overlay.window.request_redraw();
                }
            }
            Some(Err(error)) => {
                if let Some(window) = &mut self.settings_window {
                    window.set_error(error.to_string());
                }
            }
            None => {}
        }
    }

    /// Start over with every adult not marked kept, one secure seed each, as one undoable edit.
    fn regenerate_unkept_creatures(&mut self) {
        let adult_targets = self.world.as_ref().map_or(0, |world| {
            world
                .save
                .creatures
                .iter()
                .filter(|creature| !creature.kept && creature.role.is_adult())
                .count()
        });
        let seeds: Result<Vec<_>, _> = (0..adult_targets).map(|_| new_colony_seed()).collect();
        match seeds {
            Ok(seeds) => {
                let desktop = self.snapshot();
                let changed = self.world.as_mut().map_or(0, |world| {
                    world.edit(ColonyEdit::StartedOver, |world| {
                        world.regenerate_unkept(&seeds, OffsetDateTime::now_utc(), &desktop)
                    })
                });
                if changed > 0 {
                    let _ = self.save();
                    self.redraw_due = Instant::now();
                    for overlay in self.overlays.values() {
                        overlay.window.request_redraw();
                    }
                }
            }
            Err(error) => {
                if let Some(window) = &mut self.settings_window {
                    window.set_error(format!("Could not generate secure seeds: {error}"));
                }
            }
        }
    }

    /// Take back the last undoable edit, saying which.
    fn undo_last_colony_edit(&mut self) {
        let result = self.world.as_mut().map(World::undo_last_edit);
        match result {
            Some(Ok(edit)) => {
                self.save_with_feedback(&format!("Undid {}", edit.describe()));
                self.redraw_due = Instant::now();
                for overlay in self.overlays.values() {
                    overlay.window.request_redraw();
                }
            }
            Some(Err(error)) => self.settings_error(format!("Could not undo: {error}")),
            None => {}
        }
    }

    /// A companion renamed, or its profile seen for the first time: one save for both, with a
    /// notice only for the name.
    fn save_profile_changes(
        &mut self,
        rename_creature: Option<(CreatureId, String)>,
        viewed_profile: Option<CreatureId>,
    ) {
        let renamed = rename_creature.is_some();
        let mut save_profile = false;
        if let Some((creature_id, name)) = rename_creature
            && let Some(world) = &mut self.world
        {
            match world.rename_creature(creature_id, &name) {
                Ok(()) => save_profile = true,
                Err(error) => tracing::warn!(%error, "invalid creature name rejected"),
            }
        }
        if let Some(creature_id) = viewed_profile
            && let Some(world) = &mut self.world
        {
            save_profile |= world.mark_profile_viewed(creature_id);
        }
        if save_profile {
            if renamed {
                self.save_with_feedback("Name saved");
            } else {
                let _ = self.save();
            }
        }
    }

    /// Apply the preferences page, saying what changed, and tell the window they took; while a
    /// recovery is pending they are not saved, and the notice says so.
    fn apply_preferences(&mut self, settings: Settings) {
        let previous_launch = self
            .world
            .as_ref()
            .map(|world| world.save.settings.launch_at_login)
            .unwrap_or(false);
        let mut notice = None;
        if let Some(world) = &mut self.world {
            notice = crate::notices::preferences_applied(&world.save.settings, &settings);
            world.save.settings = settings;
        }
        match self.finish_settings_change(previous_launch) {
            Ok(()) => {
                if let (Some(window), Some(world)) = (&mut self.settings_window, &self.world) {
                    window.acknowledge_preferences(&world.save.settings);
                }
                if self.recovery_pending {
                    self.settings_notice("Temporary preferences · finish recovery to save");
                } else {
                    self.settings_notice(notice.unwrap_or_else(|| "Changes applied".into()));
                }
            }
            Err(error) => self.settings_error(error.to_string()),
        }
    }

    /// Gather the colony on the current desktop.
    fn gather_colony(&mut self) {
        let desktop = self.snapshot();
        if let Some(world) = &mut self.world {
            world.handle_command(WorldCommand::GatherCreatures, &desktop);
        }
        let _ = self.save();
    }

    /// Open the diagnostic log directory in the file manager; failing to is only logged.
    fn open_log_directory(&mut self) {
        if let Err(error) = platform::open_directory(&self.log_dir) {
            tracing::error!(%error, "could not open diagnostic log directory");
        }
    }

    /// Turn the daily update check on or off, checking at once when turning it on is due.
    fn set_automatic_update_checks(&mut self, enabled: bool) {
        if let Err(error) = self.updates.set_automatic_checks(enabled) {
            tracing::error!(%error, "could not save update preference");
        }
        if enabled
            && self
                .updates
                .should_check_automatically(OffsetDateTime::now_utc())
        {
            self.start_update_check();
        }
    }

    /// Echo Fox Mode on or off, redrawing so every companion changes at once.
    fn toggle_echo_fox(&mut self, on: bool) {
        let Some(world) = &mut self.world else {
            return;
        };
        world.set_echo_fox(on);
        self.settings_notice(if on {
            "Echo Fox Mode"
        } else {
            "Everyone is themselves again"
        });
        for overlay in self.overlays.values() {
            overlay.window.request_redraw();
        }
    }

    /// Launch a downloaded update's installer after saving, quitting when the installer wants
    /// the app gone; a failure is shown as a dialog and kept in the updater's state.
    fn install_downloaded_update(&mut self, event_loop: &ActiveEventLoop) {
        let Some(downloaded) = self.updates.ready_update().cloned() else {
            return;
        };
        let _ = self.save();
        match platform::launch_update(&downloaded.path) {
            Ok(quit) if quit => event_loop.exit(),
            Ok(_) => {}
            Err(error) => {
                tracing::error!(%error, "could not launch update installer");
                let message = format!("could not launch the update installer: {error}");
                let (headline, advice) = crate::explain::update(&message);
                self.updates.fail(message);
                self.sync_update_ui();
                self.failure_dialog(headline, &advice);
            }
        }
    }

    /// Pick an application in a file dialog and add it to the habitat's list.
    fn browse_for_application(&mut self) {
        if let Some((application, display_name)) = platform::browse_application()
            && let Some(window) = &mut self.settings_window
        {
            window.add_application(application, display_name);
        }
    }

    /// Put the habitat editor's draft back to the default policy, dropping any drag in progress.
    fn reset_habitat_draft(&mut self) {
        let Some(editor) = &mut self.habitat_editor else {
            return;
        };
        editor.draft = HabitatPolicy::default();
        editor.drag = None;
        if let Some(window) = &mut self.settings_window {
            window.set_habitat(editor.draft.clone());
        }
        for overlay in self.overlays.values() {
            overlay.window.request_redraw();
        }
    }

    pub(super) fn settings_notice(&mut self, message: impl Into<String>) {
        if let Some(window) = &mut self.settings_window {
            window.notify(message);
        }
    }

    /// Something the reader asked for went wrong and they need to know before carrying on: said
    /// in a native dialog, since it answers what they just did, and kept on the notebook's footer
    /// afterwards. Only ever for something the reader started, never for background work.
    pub(super) fn failure_dialog(&mut self, title: &str, message: &str) {
        self.settings_error(message.to_owned());
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title(title)
            .set_description(message)
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }

    /// An export the reader chose a place for could not be written there.
    pub(super) fn export_failed(&mut self, what: &str, error: &anyhow::Error) {
        tracing::warn!(%error, what, "export failed");
        let title = format!("Could not save {what}");
        self.failure_dialog(&title, &crate::explain::export(what, error));
    }

    pub(super) fn settings_error(&mut self, message: impl Into<String>) {
        if let Some(window) = &mut self.settings_window {
            window.set_error(message);
        }
    }

    pub(super) fn save_with_feedback(&mut self, message: &str) {
        match self.save() {
            Ok(()) if !self.recovery_pending => self.settings_notice(message),
            Ok(()) => self
                .settings_notice("Temporary changes · restore a backup or finish recovery to save"),
            Err(error) => self.settings_error(format!("Could not save changes: {error}")),
        }
    }

    pub(super) fn export_colony(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Formiga colony", &["json"])
            .set_file_name("Formiga-colony.json")
            .save_file()
        else {
            return;
        };
        if path == self.save_store.path() {
            self.settings_error("Choose a backup location outside the active colony file.");
            return;
        }
        let Some(world) = &self.world else {
            return;
        };
        match SaveStore::new(&path).save(&world.save) {
            Ok(()) => self.settings_notice("Full colony backup exported"),
            Err(error) => {
                let error = anyhow::Error::from(error);
                self.export_failed("the colony backup", &error);
            }
        }
    }

    pub(super) fn restore_colony(&mut self) {
        // A colony away in Hill or Battle comes home with that app's results, which would
        // otherwise be kept by whichever colony had been restored in its place.
        if let Some(app) = self.colony_away_in() {
            self.settings_notice(format!(
                "The colony is away in {}; restore a backup once it is home",
                app.name
            ));
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Formiga colony", &["json"])
            .pick_file()
        else {
            return;
        };
        let mut save = match SaveStore::read_snapshot(&path) {
            Ok(save) => save.into_inner(),
            Err(error) => {
                self.failure_dialog(
                    "That backup could not be opened",
                    &crate::explain::colony_file(&error),
                );
                return;
            }
        };
        let confirmed = rfd::MessageDialog::new().set_title("Restore your colony?")
            .set_description(format!("Restore {} companions from this backup? Your current colony files will be kept as recovery copies first.", save.creatures.len()))
            .set_buttons(rfd::MessageButtons::OkCancel).show() == rfd::MessageDialogResult::Ok;
        if !confirmed {
            return;
        }
        if validate_habitat(&save.settings.habitat, &self.monitors).is_err() {
            save.settings.habitat = HabitatPolicy::default();
        }
        let imported = World::from_save(save);
        let result = self
            .save_store
            .preserve_recovery_files()
            .and_then(|_| self.save_store.save(&imported.save));
        if let Err(error) = result {
            self.settings_error(format!("Restore stopped; could not safely save: {error}"));
            return;
        }
        let previous_launch = self
            .world
            .as_ref()
            .is_some_and(|w| w.save.settings.launch_at_login);
        // A house open in Home, and a companion open in Farm, belong to the colony being
        // replaced.
        self.abandon_visits();
        self.world = Some(imported);
        self.recovery_pending = false;
        self.milestone_notice = None;
        if let (Some(window), Some(world)) = (&mut self.settings_window, &self.world) {
            window.clubhouse.recovery.reason = None;
            window.clubhouse.recovery.restore_confirmed = false;
            window.clear_generation_preview();
            window.show(
                &world.save.settings,
                &world.save.creatures,
                &world.save.relationships,
            );
        }
        if let Err(error) = self.finish_settings_change(previous_launch) {
            self.settings_error(format!(
                "Colony restored, but a preference could not be applied: {error}"
            ));
            return;
        }
        self.redraw_due = Instant::now();
        self.settings_notice("Colony restored · previous files kept as recovery copies");
    }
}
