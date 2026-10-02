//! The Journal page: every moment written down, searched and filtered, the guest book, the
//! scrapbook of finds, and the page of wonders.

use super::shell::Shell;
use super::*;
use formiga_core::WonderKind;

/// The Journal page's own state: how the journal is filtered and searched, and the picture of
/// the wonders. A view preference, never saved.
#[derive(Default)]
pub(crate) struct JournalState {
    /// Which companion the journal is filtered to, if any.
    pub filter: Option<CreatureId>,
    /// Which kind of moment the journal is filtered to, if any.
    pub kind: Option<MomentKind>,
    /// What the journal is being searched for.
    pub search: String,
    /// One still picture of every kind of wonder, side by side, for the page of them.
    wonder_sheet: Option<([u8; 32], TextureHandle)>,
}

impl JournalState {
    pub(super) fn texture_ids(&self) -> impl Iterator<Item = egui::TextureId> + '_ {
        self.wonder_sheet.iter().map(|(_, t)| t.id())
    }

    pub(super) fn release_images(&mut self) {
        self.wonder_sheet = None;
    }
}

/// The heading one moment belongs under: Today, Yesterday, or its own local date. A clock that
/// has been wound back can leave a moment in the future; it still reads as today.
fn day_heading(at: OffsetDateTime, now: OffsetDateTime, offset: time::UtcOffset) -> String {
    let (local, today) = (at.to_offset(offset).date(), now.to_offset(offset).date());
    match (today - local).whole_days() {
        days if days <= 0 => "Today".into(),
        1 => "Yesterday".into(),
        _ => local.to_string(),
    }
}

fn moment_card(
    ui: &mut Ui,
    save: &SaveFile,
    entry: &JournalEntry,
    offset: time::UtcOffset,
    outcome: &mut SettingsOutcome,
    new: bool,
) {
    // A line in the log: when, what, and whether it is kept. A kept moment is dated, since it
    // stays long after its day; one under its day's heading needs only the time.
    let local = entry.at.to_offset(offset);
    let pinned = save.companion.pinned(entry);
    let scale = journal::text_scale(ui);
    let when = if pinned {
        format!(
            "{:02} {} · {:02}:{:02}",
            local.day(),
            &format!("{:?}", local.month())[..3],
            local.hour(),
            local.minute()
        )
    } else {
        format!("{:02}:{:02}", local.hour(), local.minute())
    };
    ui.horizontal(|ui| {
        let when_width = if pinned { 104.0 } else { 46.0 } * scale;
        ui.allocate_ui_with_layout(
            egui::vec2(when_width, 0.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_width(when_width);
                ui.label(journal::label_job(
                    &when,
                    journal::label_size(scale),
                    muted(),
                ));
            },
        );
        let action = if pinned { "Unpin" } else { "Pin" };
        let action_width = text_width(ui, RichText::new(action).small()) + 12.0;
        let text_room =
            (ui.available_width() - action_width - ui.spacing().item_spacing.x * 2.0).max(80.0);
        ui.allocate_ui_with_layout(
            egui::vec2(text_room, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(text_room);
                let text = moment_text(save, entry);
                // A landmark is set a little stronger, so arrivals and friendships stand out in a
                // long run of everyday moments.
                let text = if entry.moment.is_landmark() {
                    RichText::new(text).strong()
                } else {
                    RichText::new(text)
                };
                let text = if new {
                    text.background_color(note_fill())
                } else {
                    text
                };
                let label = ui.add(egui::Label::new(text).wrap());
                if new {
                    label.on_hover_text("New since the journal was last read");
                }
            },
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if pinned {
                if ui
                    .add(egui::Button::new(RichText::new("Unpin").small()).frame(false))
                    .clicked()
                {
                    outcome.unpin_moment = Some(entry.clone());
                }
            } else {
                let room = save.companion.pins.len() < MAX_PINNED_ENTRIES;
                let button = ui.add_enabled(
                    room,
                    egui::Button::new(RichText::new("Pin").small()).frame(false),
                );
                if button.clicked() {
                    outcome.pin_moment = Some(entry.clone());
                }
                if !room {
                    button.on_hover_text(format!(
                        "All {MAX_PINNED_ENTRIES} pins are in use. Unpin one to keep another."
                    ));
                }
            }
        });
    });
    ui.add_space(4.0);
}

fn label_was_shortened(name: &str) -> bool {
    name.chars().count() > 16
}

/// A button that keeps a visitor as a favorite, or says it already is one.
fn keep_favorite_button(
    ui: &mut Ui,
    save: &SaveFile,
    name: &str,
    origin: CreatureOrigin,
    outcome: &mut SettingsOutcome,
) {
    if save.visitors.is_favorite(&origin) {
        ui.small("★ A favorite");
        return;
    }
    let room = save.visitors.favorites.len() < MAX_FAVORITE_VISITORS;
    let button = ui.add_enabled(room, egui::Button::new("☆ Keep as a favorite").small());
    if button.clicked() {
        outcome.keep_favorite_visitor = Some((name.to_owned(), origin));
    }
    if !room {
        button.on_hover_text(format!(
            "All {MAX_FAVORITE_VISITORS} favorites are kept. Forget one to keep another."
        ));
    }
}

/// Visitors kept to invite again. One click asks a favorite over for a day, through exactly the
/// invitation a friend's code makes, so the same checks answer: nobody else can be visiting, and
/// a favorite who has since come to live here is already home.
fn favorite_visitors(
    ui: &mut Ui,
    save: &SaveFile,
    outcome: &mut SettingsOutcome,
    offset: time::UtcOffset,
) {
    if save.visitors.favorites.is_empty() {
        return;
    }
    ui.label(
        RichText::new(format!(
            "FAVORITE VISITORS · {} of {MAX_FAVORITE_VISITORS}",
            save.visitors.favorites.len()
        ))
        .color(forest())
        .size(11.0),
    );
    ui.add_space(6.0);
    let visiting = save.visitors.guest.as_ref();
    for favorite in save.visitors.favorites.iter().rev() {
        let here_now = visiting.is_some_and(|guest| guest.creature.origin == favorite.origin);
        card(ui, |ui| {
            let kept = favorite.kept_at_utc.to_offset(offset);
            ui.small(if here_now {
                "Visiting right now".to_owned()
            } else {
                format!("Kept since {}", kept.date())
            });
            ui.strong(&favorite.name);
            ui.horizontal_wrapped(|ui| {
                if !here_now {
                    let free = visiting.is_none();
                    let invite =
                        ui.add_enabled(free, egui::Button::new("Invite for a day").fill(mint()));
                    if invite.clicked() {
                        outcome.invite_visitor = Some(SharedCreatureSeed::from(favorite.origin));
                    }
                    if !free {
                        invite.on_hover_text("Someone is already visiting the houses.");
                    }
                }
                if ui.small_button("Forget").clicked() {
                    outcome.forget_favorite_visitor = Some(favorite.origin);
                }
            });
        });
        ui.add_space(8.0);
    }
}

/// Who has come by the houses: whoever is here now, the favorites kept to invite again, and the
/// last two dozen visitors before them. A line in the book carries a name, a day, and the code
/// that recreates the visitor — the same code they would have handed over themselves.
fn guest_book(
    ui: &mut Ui,
    save: &SaveFile,
    shell: &mut Shell,
    outcome: &mut SettingsOutcome,
    offset: time::UtcOffset,
) {
    let visiting = save.visitors.guest.as_ref();
    journal::kicker(ui, "The guest book");
    ui.add_space(6.0);
    if visiting.is_none()
        && save.visitors.guest_book.is_empty()
        && save.visitors.favorites.is_empty()
    {
        card(ui, |ui| {
            ui.strong("No visitors yet");
            ui.label(
                "Now and then someone passing through stops at the houses while they are out. \
                 A friend can also send you their companion's code, which you paste into the \
                 creature studio to invite them over for a day.",
            );
        });
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(8.0);
        return;
    }
    if let Some(guest) = visiting {
        card(ui, |ui| {
            ui.strong(format!("{} is visiting", guest.creature.name));
            ui.small(match guest.source {
                VisitorSource::Invited => "Invited with a code, here for the day.",
                VisitorSource::Wanderer => "Passing through, and stopped at the houses.",
            });
            // A face the colony has seen before: how many times, and when it was last here.
            if let Some((before, last)) = save.visitors.earlier_visits() {
                ui.small(format!(
                    "Back for a {} visit; last here {}. Whoever lived here then remembers them.",
                    ordinal(u32::from(before) + 1),
                    today::ago_text(last, OffsetDateTime::now_utc(), offset)
                ));
            }
            ui.horizontal_wrapped(|ui| {
                let room = save.visitors.can_stay(&save.creatures);
                let stay = ui.add_enabled(room, egui::Button::new("Ask to stay").fill(mint()));
                if stay.clicked() {
                    outcome.ask_visitor_to_stay = true;
                }
                if !room {
                    stay.on_hover_text(
                        "Your colony is full. Keep their code and ask them again another time.",
                    );
                }
                if ui.button("Copy code").clicked() {
                    ui.ctx()
                        .copy_text(encode_creature_seed(guest.creature.origin));
                    shell.notify("Visitor code copied");
                }
                keep_favorite_button(
                    ui,
                    save,
                    &guest.creature.name,
                    guest.creature.origin,
                    outcome,
                );
            });
        });
        ui.add_space(8.0);
    }
    favorite_visitors(ui, save, outcome, offset);
    if !save.visitors.guest_book.is_empty() {
        let heading = format!("Visitors before this · {}", save.visitors.guest_book.len());
        egui::CollapsingHeader::new(heading)
            .id_salt("guest-book")
            .show(ui, |ui| {
                for entry in save.visitors.guest_book.iter().rev() {
                    let local = entry.visited_at_utc.to_offset(offset);
                    let visits = save.visitors.visits_of(&entry.origin);
                    card(ui, |ui| {
                        ui.small(format!(
                            "{} · {}{}",
                            local.date(),
                            match entry.source {
                                VisitorSource::Invited => "invited for a day",
                                VisitorSource::Wanderer => "came by the houses",
                            },
                            if visits > 1 {
                                format!(" · {visits} visits in the book")
                            } else {
                                String::new()
                            }
                        ));
                        ui.strong(&entry.name);
                        ui.horizontal_wrapped(|ui| {
                            if ui.small_button("Copy code").clicked() {
                                ui.ctx().copy_text(encode_creature_seed(entry.origin));
                                shell.notify("Visitor code copied");
                            }
                            keep_favorite_button(ui, save, &entry.name, entry.origin, outcome);
                        });
                    });
                    ui.add_space(8.0);
                }
                ui.small(format!(
                    "The last {MAX_GUEST_BOOK_ENTRIES} visitors stay on this computer. A code \
                     recreates how a visitor looked and what they were like, and nothing else."
                ));
            });
    }
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(8.0);
}

/// Whether a moment matches what the journal is being searched for: its words as the journal
/// writes them, and the name of whoever it is about, ignoring case.
pub(crate) fn moment_matches(save: &SaveFile, entry: &JournalEntry, search: &str) -> bool {
    let search = search.trim().to_lowercase();
    if search.is_empty() {
        return true;
    }
    let text = moment_text(save, entry).to_lowercase();
    search.split_whitespace().all(|word| text.contains(word))
}

impl JournalState {
    /// The Journal page. `reading` is what was new when the reader turned to it.
    pub(crate) fn show(
        &mut self,
        ui: &mut Ui,
        shell: &mut Shell,
        save: &SaveFile,
        reading: Option<&today::Reading>,
        outcome: &mut SettingsOutcome,
    ) {
        page(ui, save, self, shell, reading, outcome);
    }
}

fn page(
    ui: &mut Ui,
    save: &SaveFile,
    state: &mut JournalState,
    shell: &mut Shell,
    reading: Option<&today::Reading>,
    outcome: &mut SettingsOutcome,
) {
    journal::page_heading(
        ui,
        crate::settings::SettingsTab::Journal,
        "Journal",
        "Small moments, written down before anyone forgets them.",
    );
    let offset = local_offset();
    let now = OffsetDateTime::now_utc();
    if save.companion.journal.is_empty() && save.companion.pins.is_empty() {
        card(ui, |ui| {
            ui.heading("The story is just beginning");
            ui.label(
                "Nothing has been written down yet. Arrivals, friendships, finds, new habits, \
                 shared moments and visitors appear here as they happen — on their own time, \
                 never on a schedule.",
            );
            ui.small(
                "Leave Formiga running and come back later; the Today page will say what is new.",
            );
        });
        ui.add_space(14.0);
        guest_book(ui, save, shell, outcome, offset);
        state.scrapbook(ui, shell, save);
        state.wonders(ui, save);
        return;
    }
    // The colony's landmarks, which the journal keeps past their turn to roll out: who arrived,
    // who grew close, and who first came by.
    let landmarks: Vec<&JournalEntry> = save
        .companion
        .journal
        .iter()
        .rev()
        .filter(|entry| entry.moment.is_landmark())
        .collect();
    if !landmarks.is_empty() {
        journal::kicker(ui, "Milestones").on_hover_text(
            "Arrivals, friendships and first visits. The journal holds on to these when \
             everyday moments roll out.",
        );
        ui.add_space(6.0);
        wide_card(ui, |ui| {
            for entry in landmarks.iter().take(8) {
                let local = entry.at.to_offset(offset);
                // The date and the moment as one line of type, so they share a baseline and the
                // line wraps as a whole in a narrow window.
                let mut line = journal::label_job(
                    &format!(
                        "{:02} {} {}   ",
                        local.day(),
                        &format!("{:?}", local.month())[..3],
                        local.year()
                    ),
                    journal::label_size(journal::text_scale(ui)),
                    muted(),
                );
                let body = ui
                    .style()
                    .text_styles
                    .get(&egui::TextStyle::Body)
                    .cloned()
                    .unwrap_or_else(|| egui::FontId::proportional(14.0));
                line.append(
                    &moment_text(save, entry),
                    0.0,
                    egui::TextFormat {
                        font_id: body,
                        color: ink(),
                        ..Default::default()
                    },
                );
                ui.add(egui::Label::new(line).wrap());
            }
            if landmarks.len() > 8 {
                ui.small(format!(
                    "{} earlier milestones further down.",
                    landmarks.len() - 8
                ));
            }
        });
        ui.add_space(12.0);
    }
    // Kept moments sit above the rolling journal and are not repeated inside it. They are read
    // from the pins themselves rather than from the journal, so keeping a moment still keeps it
    // once the rolling sixty-four have moved on past it — which is the whole point of a pin.
    let mut pinned: Vec<JournalEntry> = save
        .companion
        .pins
        .iter()
        .map(|pin| JournalEntry {
            at: pin.at,
            creature: pin.creature,
            moment: pin.moment.clone(),
        })
        .collect();
    pinned.sort_by_key(|entry| std::cmp::Reverse(entry.at));
    if !pinned.is_empty() {
        journal::kicker(
            ui,
            &format!("Kept · {} of {MAX_PINNED_ENTRIES}", pinned.len()),
        )
        .on_hover_text("Moments you pinned. They stay here however long ago they happened.");
        ui.add_space(6.0);
        for entry in &pinned {
            moment_card(ui, save, entry, offset, outcome, false);
        }
        ui.separator();
        ui.add_space(8.0);
    }
    // Search, then the kinds of moment and the companions the journal actually mentions, so a
    // filter never offers an empty result.
    let search_id = egui::Id::new("journal-search");
    if ui.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
        ui.memory_mut(|memory| memory.request_focus(search_id));
    }
    ui.horizontal(|ui| {
        let field = ui.add(
            egui::TextEdit::singleline(&mut state.search)
                .id(search_id)
                .hint_text(format!(
                    "Search the journal ({}F)",
                    journal::shortcut_modifier()
                ))
                .desired_width((ui.available_width() - 90.0).clamp(120.0, 360.0)),
        );
        if field.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            state.search.clear();
        }
        if !state.search.is_empty() && ui.small_button("Clear").clicked() {
            state.search.clear();
        }
    });
    ui.add_space(6.0);
    let kinds: Vec<MomentKind> = MomentKind::ALL
        .into_iter()
        .filter(|kind| {
            save.companion
                .journal
                .iter()
                .any(|entry| entry.moment.kind() == *kind)
        })
        .collect();
    if kinds.len() > 1 {
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(state.kind.is_none(), "Every kind")
                .clicked()
            {
                state.kind = None;
            }
            for kind in kinds {
                if ui
                    .selectable_label(state.kind == Some(kind), kind.label())
                    .clicked()
                {
                    state.kind = (state.kind != Some(kind)).then_some(kind);
                }
            }
        });
    }
    let mut mentioned: Vec<CreatureId> = save
        .companion
        .journal
        .iter()
        .filter_map(|entry| entry.creature)
        .collect();
    mentioned.sort_unstable();
    mentioned.dedup();
    if mentioned.len() > 1 {
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(state.filter.is_none(), "Everyone")
                .clicked()
            {
                state.filter = None;
            }
            for id in mentioned {
                let name = save
                    .creatures
                    .iter()
                    .find(|c| c.id == id)
                    .map_or_else(|| "A past companion".to_string(), |c| c.name.clone());
                // Names are capped at 24 characters when they are chosen, but a filter row is a
                // row of chips: keep one long name from pushing the rest onto its own line.
                let label = if name.chars().count() > 16 {
                    format!("{}…", name.chars().take(15).collect::<String>())
                } else {
                    name.clone()
                };
                let chip = ui.selectable_label(state.filter == Some(id), label);
                if label_was_shortened(&name) {
                    chip.clone().on_hover_text(name);
                }
                if chip.clicked() {
                    state.filter = (state.filter != Some(id)).then_some(id);
                }
            }
        });
    }
    ui.add_space(10.0);
    let unpinned = save
        .companion
        .journal
        .iter()
        .filter(|entry| !save.companion.pinned(entry))
        .count();
    let showing: Vec<&JournalEntry> = save
        .companion
        .journal
        .iter()
        .rev()
        .filter(|entry| !save.companion.pinned(entry))
        .filter(|entry| state.filter.is_none_or(|id| entry.creature == Some(id)))
        .filter(|entry| state.kind.is_none_or(|kind| entry.moment.kind() == kind))
        .filter(|entry| moment_matches(save, entry, &state.search))
        .collect();
    let filtering =
        state.filter.is_some() || state.kind.is_some() || !state.search.trim().is_empty();
    if filtering {
        ui.small(format!("Showing {} of {unpinned} moments", showing.len()));
        ui.add_space(4.0);
    }
    if showing.is_empty() {
        card(ui, |ui| {
            if filtering {
                ui.strong("No moments match");
                ui.label("Nothing the journal still holds matches all of that.");
                if ui.button("Clear the search and filters").clicked() {
                    state.search.clear();
                    state.kind = None;
                    state.filter = None;
                }
            } else {
                ui.strong("Nothing else here yet");
                ui.label("Every moment the journal holds is already kept above.");
            }
        });
        ui.add_space(8.0);
    }
    let mut heading = String::new();
    let reading = reading.cloned().unwrap_or_default();
    for entry in showing {
        let day = day_heading(entry.at, now, offset);
        if day != heading {
            ui.add_space(4.0);
            journal::kicker(ui, &day);
            ui.add_space(6.0);
            heading = day;
        }
        moment_card(
            ui,
            save,
            &entry.clone(),
            offset,
            outcome,
            reading.is_new(entry),
        );
    }
    ui.small(format!(
        "The most recent {MAX_JOURNAL_ENTRIES} moments stay on this computer, holding on to up to \
         {MAX_LANDMARK_ENTRIES} milestones longer, plus up to {MAX_PINNED_ENTRIES} you keep. \
         Missed time is never replayed."
    ));
    ui.add_space(20.0);
    ui.separator();
    ui.add_space(14.0);
    guest_book(ui, save, shell, outcome, offset);
    state.scrapbook(ui, shell, save);
    state.wonders(ui, save);
}

impl JournalState {
    /// The finds themselves, for the Journal: only what has actually turned up, newest first,
    /// with who found it and when. What is still to find lives in the Collection.
    pub fn scrapbook(&mut self, ui: &mut Ui, shell: &mut Shell, save: &SaveFile) {
        let offset = local_offset();
        let atlas = shell.trinket_atlas(ui, save);
        let mut found = save.companion.scrapbook.clone();
        found.sort_by_key(|record| std::cmp::Reverse((record.first_at, record.variant)));
        ui.label(
            RichText::new(format!("THE SCRAPBOOK · {} found", found.len()))
                .color(forest())
                .size(11.0),
        );
        ui.add_space(6.0);
        if found.is_empty() {
            card(ui, |ui| {
                ui.strong("Nothing found yet");
                ui.label(
                    "When a companion brings something back for the first time, it is recorded \
                     here with the date and who found it. Everything still to find is in the \
                     Collection, on the Your colony page.",
                );
            });
            ui.add_space(8.0);
            return;
        }
        for record in &found {
            let Some(info) = formiga_core::trinket_info(record.variant) else {
                continue;
            };
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(Shell::trinket_image(&atlas, info.variant, 48.0));
                    ui.vertical(|ui| {
                        ui.strong(info.name);
                        ui.label(info.description);
                        // The finder's name was kept when it was found, so a companion who has
                        // since left is still the one credited.
                        let finder = record
                            .finder
                            .and_then(|id| save.creatures.iter().find(|c| c.id == id))
                            .map_or(record.finder_name.as_str(), |c| c.name.as_str());
                        let at = record.first_at.to_offset(offset);
                        ui.small(format!("Found by {finder} · {}", at.date()));
                    });
                });
            });
            ui.add_space(8.0);
        }
        ui.small("Only the first find of each kind is recorded, and only on this computer.");
    }

    /// One still picture of every kind of wonder in a row, cut from the frames the desktop draws.
    fn wonder_sheet(&mut self, ui: &Ui, save: &SaveFile) -> TextureHandle {
        if self
            .wonder_sheet
            .as_ref()
            .is_none_or(|(seed, _)| *seed != save.colony_seed)
        {
            use formiga_art::{WONDER_CELL_HEIGHT, WONDER_CELL_WIDTH, WonderRenderer};
            let kinds = WonderKind::ALL.len() as u32;
            let mut sheet = formiga_art::Canvas::new(WONDER_CELL_WIDTH * kinds, WONDER_CELL_HEIGHT);
            for (index, kind) in WonderKind::ALL.into_iter().enumerate() {
                let frames = WonderRenderer::render(kind, save.colony_seed);
                // The level frame for a seesaw; the first for everything else.
                let rest = if kind == WonderKind::Seesaw { 3 } else { 0 };
                for y in 0..WONDER_CELL_HEIGHT as i32 {
                    for x in 0..WONDER_CELL_WIDTH as i32 {
                        let pixel = frames.get(rest * WONDER_CELL_WIDTH as i32 + x, y);
                        if pixel.a > 0 {
                            sheet.set(index as i32 * WONDER_CELL_WIDTH as i32 + x, y, pixel);
                        }
                    }
                }
            }
            let texture = upload(ui.ctx(), "colony-wonders", &sheet);
            self.wonder_sheet = Some((save.colony_seed, texture));
        }
        self.wonder_sheet.as_ref().unwrap().1.clone()
    }

    /// The notebook's page of wonders: every kind there is, the ones that have turned up drawn
    /// with who first found it, when, and how many goes the colony has had since, and the rest as
    /// a shadow with a hint. Nothing here can be placed or kept: a wonder turns up by itself.
    pub fn wonders(&mut self, ui: &mut Ui, save: &SaveFile) {
        let offset = local_offset();
        let sheet = self.wonder_sheet(ui, save);
        let found = &save.companion.wonders;
        ui.add_space(14.0);
        ui.label(
            RichText::new(format!(
                "WONDERS · {} of {} found",
                found.len(),
                WonderKind::ALL.len()
            ))
            .color(forest())
            .size(11.0),
        );
        ui.small(
            "Now and then something to play on turns up for a little while — a chair, a leaf sled, a \
             fountain — and whoever it turned up for goes straight over to have a go.",
        );
        ui.add_space(6.0);
        let kinds = WonderKind::ALL.len() as f32;
        for (index, kind) in WonderKind::ALL.into_iter().enumerate() {
            let record = found.iter().find(|record| record.kind == kind);
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    let uv = egui::Rect::from_min_max(
                        egui::pos2(index as f32 / kinds, 0.0),
                        egui::pos2((index + 1) as f32 / kinds, 1.0),
                    );
                    // Something still to find is its own shape and nothing more.
                    let tint = if record.is_some() {
                        Color32::WHITE
                    } else {
                        Color32::from_rgba_unmultiplied(0, 0, 0, 70)
                    };
                    ui.add(
                        egui::Image::new(&sheet)
                            .uv(uv)
                            .tint(tint)
                            .maintain_aspect_ratio(false)
                            .fit_to_exact_size(egui::vec2(112.0, 64.0)),
                    );
                    ui.vertical(|ui| match record {
                        Some(record) => {
                            ui.strong(kind.label());
                            ui.label(kind.description());
                            let finder = record
                                .finder
                                .and_then(|id| save.creatures.iter().find(|c| c.id == id))
                                .map_or(record.finder_name.as_str(), |c| c.name.as_str());
                            let at = record.first_at.to_offset(offset);
                            ui.small(format!(
                                "First found by {finder} · {} · {} {}",
                                at.date(),
                                record.goes,
                                if record.goes == 1 { "go" } else { "goes" }
                            ));
                        }
                        None => {
                            ui.strong("Still to find");
                            ui.label(kind.hint());
                        }
                    });
                });
            });
            ui.add_space(6.0);
        }
    }
}
