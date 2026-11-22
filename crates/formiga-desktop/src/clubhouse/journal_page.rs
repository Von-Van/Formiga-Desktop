//! The Journal page: every moment written down, searched and filtered, the guest book, the
//! scrapbook of finds, the souvenirs brought home from Formiga Hill, the cups won in Formiga
//! Battle, and the page of wonders.

use super::shell::Shell;
use super::*;
use formiga_core::{BattleKeepsake, Souvenir, WonderKind};

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
    /// Whether the guest book shows every visitor it holds, rather than the latest few.
    all_visitors: bool,
    /// Whether the search and the filters are out, behind the magnifier.
    pub searching: bool,
    /// Where each moment's pin was drawn last frame, by when the moment was: what a pointer or
    /// the review tests find it by, since it is drawn rather than written.
    pub(crate) pin_spots: Vec<(OffsetDateTime, egui::Rect)>,
    /// The pictures taped in beside the newest days' best moments, each with what it was drawn
    /// from, kept while they are on the page.
    vignettes: Vec<(VignetteKey, TextureHandle, egui::Rect)>,
    /// One still picture of every kind of wonder, side by side, for the page of them.
    wonder_sheet: Option<([u8; 32], TextureHandle)>,
    /// Every souvenir's picture side by side, each on its velvet, made once one has come home.
    souvenir_strip: Option<TextureHandle>,
    /// Every one of Battle's keepsakes side by side, each on its shelf, made once one has come
    /// home.
    cup_strip: Option<TextureHandle>,
}

impl JournalState {
    pub(super) fn texture_ids(&self) -> impl Iterator<Item = egui::TextureId> + '_ {
        self.wonder_sheet
            .iter()
            .map(|(_, t)| t.id())
            .chain(self.vignettes.iter().map(|(_, t, _)| t.id()))
            .chain(self.souvenir_strip.iter().map(TextureHandle::id))
            .chain(self.cup_strip.iter().map(TextureHandle::id))
    }

    pub(super) fn release_images(&mut self) {
        self.vignettes.clear();
        self.wonder_sheet = None;
        self.souvenir_strip = None;
        self.cup_strip = None;
    }
}

/// The heading one moment belongs under: Today, Yesterday, or its own local date. A clock that
/// has been wound back can leave a moment in the future; it still reads as today.
fn day_heading(at: OffsetDateTime, now: OffsetDateTime, offset: time::UtcOffset) -> String {
    let (local, today) = (at.to_offset(offset).date(), now.to_offset(offset).date());
    match (today - local).whole_days() {
        days if days <= 0 => "Today".into(),
        1 => "Yesterday".into(),
        _ => dates::day_heading(local, today.year()),
    }
}

/// A day as a folded line writes it where its time would be: "Today", "Yesterday", "Mon 14 Sep".
fn short_day(at: OffsetDateTime, now: OffsetDateTime, offset: time::UtcOffset) -> String {
    let (local, today) = (at.to_offset(offset).date(), now.to_offset(offset).date());
    match (today - local).whole_days() {
        days if days <= 0 => "Today".into(),
        1 => "Yesterday".into(),
        _ => format!(
            "{} {}",
            &local.weekday().to_string()[..3],
            dates::short_date(local, today.year())
        ),
    }
}

/// What a vignette was drawn from: the moment, and how each companion in it looked.
type VignetteKey = (
    (i128, Option<CreatureId>, String),
    Vec<(CreatureId, AppearanceGenome)>,
);

/// How many of the newest days have their best moment taped in as a picture.
const PICTURED_DAYS: usize = 2;

/// Who a moment's picture shows: whoever it is about, the friend for a friendship, and the whole
/// colony (four at most) for a ritual or a visitor.
fn vignette_cast<'a>(save: &'a SaveFile, entry: &JournalEntry) -> Vec<&'a Creature> {
    let find = |id: CreatureId| save.creatures.iter().find(|c| c.id == id);
    match entry.moment {
        JournalMoment::Ritual(_) => save.creatures.iter().take(4).collect(),
        JournalMoment::Friendship(friend) => entry
            .creature
            .and_then(find)
            .into_iter()
            .chain(find(friend))
            .collect(),
        JournalMoment::Visit(_) | JournalMoment::Revisit(..) => entry
            .creature
            .and_then(find)
            .or_else(|| save.creatures.first())
            .into_iter()
            .collect(),
        _ => entry.creature.and_then(find).into_iter().collect(),
    }
}

impl JournalState {
    /// A moment's picture, taped onto the page with its caption under it: composed by the art
    /// from the companions it is about, at a whole number of points to each of its pixels, the
    /// find drawn over the finder's paws for a first find.
    fn vignette(&mut self, ui: &mut Ui, shell: &mut Shell, save: &SaveFile, entry: &JournalEntry) {
        let Some(vignette) = formiga_art::Vignette::for_moment(&entry.moment) else {
            return;
        };
        let cast = vignette_cast(save, entry);
        if cast.is_empty() {
            return;
        }
        let key: VignetteKey = (
            entry_key(entry),
            cast.iter().map(|c| (c.id, c.appearance.clone())).collect(),
        );
        // Only the figures are kept as a picture, cut to the part of it they cover; the sky and
        // the ground are flat fills painted under them.
        let (texture, within) = match self.vignettes.iter().find(|(drawn, _, _)| *drawn == key) {
            Some((_, texture, within)) => (texture.clone(), *within),
            None => {
                let genomes: Vec<&AppearanceGenome> = cast.iter().map(|c| &c.appearance).collect();
                let figures = formiga_art::render_vignette_figures(vignette, &genomes);
                let (left, top, right, bottom) = figures.alpha_bounds().unwrap_or((0, 0, 0, 0));
                let mut cut = Canvas::new(right - left + 1, bottom - top + 1);
                for y in 0..cut.height() as i32 {
                    for x in 0..cut.width() as i32 {
                        cut.set(x, y, figures.get(left as i32 + x, top as i32 + y));
                    }
                }
                let within = egui::Rect::from_min_size(
                    egui::pos2(left as f32, top as f32),
                    egui::vec2(cut.width() as f32, cut.height() as f32),
                );
                let texture = upload(ui.ctx(), "journal-vignette", &cut);
                self.vignettes.push((key, texture.clone(), within));
                if self.vignettes.len() > PICTURED_DAYS {
                    let _ = self.vignettes.remove(0);
                }
                (texture, within)
            }
        };
        let width = formiga_art::VIGNETTE_WIDTH as f32;
        let height = formiga_art::VIGNETTE_HEIGHT as f32;
        let unit = ((ui.available_width() - 40.0) / width)
            .floor()
            .clamp(1.0, 2.0);
        let atlas = vignette
            .find_spot()
            .and(story::find_record(save, entry))
            .map(|record| (record.variant, shell.trinket_atlas(ui, save)));
        ui.add_space(6.0);
        paper::pasted(ui, width * unit + 32.0, 0.0, true, |ui| {
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(width * unit, height * unit),
                egui::Sense::hover(),
            );
            let mut fills = journal::Fills::default();
            for (x, y, w, h, colour) in vignette.backdrop() {
                fills.rect(
                    egui::Rect::from_min_size(
                        rect.min + egui::vec2(x as f32, y as f32) * unit,
                        egui::vec2(w as f32, h as f32) * unit,
                    ),
                    Color32::from_rgba_unmultiplied(colour.r, colour.g, colour.b, colour.a),
                );
            }
            fills.paint(ui.painter());
            ui.painter().image(
                texture.id(),
                egui::Rect::from_min_size(
                    rect.min + within.min.to_vec2() * unit,
                    within.size() * unit,
                ),
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            if let (Some((x, y)), Some((variant, atlas))) = (vignette.find_spot(), atlas) {
                ui.painter().image(
                    atlas.id(),
                    egui::Rect::from_min_size(
                        rect.min + egui::vec2(x as f32, y as f32) * unit,
                        egui::vec2(16.0, 16.0) * unit,
                    ),
                    Shell::trinket_uv(variant),
                    Color32::WHITE,
                );
            }
            ui.add_space(4.0);
            ui.label(RichText::new(moment_text(save, entry)).color(ink()));
        });
        ui.add_space(6.0);
    }
}

/// What tells one moment from another for the widgets drawn for it: two moments can share a
/// minute and a companion, but not also what happened.
pub(crate) fn entry_key(entry: &JournalEntry) -> (i128, Option<CreatureId>, String) {
    (
        entry.at.unix_timestamp_nanos(),
        entry.creature,
        format!("{:?}", entry.moment),
    )
}

/// A pushpin, drawn: a round head over a short point, in `colour`.
fn pin_glyph(painter: &egui::Painter, rect: egui::Rect, colour: Color32) {
    let head = rect.center() - egui::vec2(0.0, rect.height() * 0.15);
    painter.line_segment(
        [
            head,
            head + egui::vec2(-rect.width() * 0.25, rect.height() * 0.4),
        ],
        egui::Stroke::new(2.0, colour),
    );
    painter.circle_filled(head, rect.width() * 0.26, colour);
}

/// A moment on its line, as Today writes it, with a pin at the line's end that shows while the
/// line is pointed at or the pin has the keyboard, and stays on a kept moment; a kept moment has
/// a strip of tape across its time.
#[allow(clippy::too_many_arguments)]
fn moment_card(
    ui: &mut Ui,
    shell: &mut Shell,
    save: &SaveFile,
    entry: &JournalEntry,
    when: &str,
    outcome: &mut SettingsOutcome,
    new: bool,
    pin_spots: &mut Vec<(OffsetDateTime, egui::Rect)>,
) {
    let pinned = save.companion.pinned(entry);
    let pin_side = 20.0;
    // Kept clear of the page's right edge, where the scroll bar answers to the pointer.
    let edge = 14.0;
    let row = today::moment_row(ui, shell, save, entry, when, new, pin_side + edge + 4.0);
    let spot = egui::Rect::from_min_size(
        egui::pos2(row.right() - edge - pin_side, row.bottom() - pin_side - 2.0),
        egui::vec2(pin_side, pin_side),
    );
    let room = pinned || save.companion.pins.len() < MAX_PINNED_ENTRIES;
    let response = ui.interact(
        spot,
        egui::Id::new(("pin", entry_key(entry))),
        if room {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    let line_hovered = ui.rect_contains_pointer(row);
    if pinned || line_hovered || response.has_focus() {
        let colour = if pinned || response.hovered() {
            forest()
        } else {
            muted()
        };
        pin_glyph(ui.painter(), spot, colour);
    }
    if pinned {
        paper::tape_strip(
            ui.painter(),
            row.left_bottom() + egui::vec2(18.0, -12.0),
            egui::vec2(40.0, 12.0),
            -8f32.to_radians(),
        );
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            spot.expand(2.0),
            0.0,
            egui::Stroke::new(2.0, forest()),
            egui::StrokeKind::Outside,
        );
    }
    if room && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let label = if pinned { "Unpin" } else { "Pin" };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, room, label));
    let response = if pinned {
        response.on_hover_text("Kept. Click to let it go with the rest in time.")
    } else if room {
        response.on_hover_text("Keep this moment, however long ago it gets.")
    } else {
        response.on_hover_text(format!(
            "All {MAX_PINNED_ENTRIES} pins are in use. Unpin one to keep another."
        ))
    };
    if response.clicked() {
        if pinned {
            outcome.unpin_moment = Some(entry.clone());
        } else {
            outcome.pin_moment = Some(entry.clone());
        }
    }
    pin_spots.push((entry.at, spot));
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
    journal::kicker(
        ui,
        &format!(
            "Favorite visitors · {} of {MAX_FAVORITE_VISITORS}",
            save.visitors.favorites.len()
        ),
    );
    ui.add_space(6.0);
    let visiting = save.visitors.guest.as_ref();
    for favorite in save.visitors.favorites.iter().rev() {
        let here_now = visiting.is_some_and(|guest| guest.creature.origin == favorite.origin);
        card(ui, |ui| {
            ui.small(if here_now {
                "Visiting right now".to_owned()
            } else {
                format!("Kept since {}", dates::on(favorite.kept_at_utc, offset))
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
    all_visitors: &mut bool,
) {
    let visiting = save.visitors.guest.as_ref();
    // Nobody has come by yet: the page has no guest book until somebody does.
    if visiting.is_none()
        && save.visitors.guest_book.is_empty()
        && save.visitors.favorites.is_empty()
    {
        return;
    }
    journal::kicker(ui, "The guest book");
    ui.add_space(6.0);
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
        // The latest few under a heading like every other on the page, and the rest of the book
        // when asked for.
        const SHOWN: usize = 3;
        let total = save.visitors.guest_book.len();
        journal::kicker(ui, &format!("Visitors before this · {total}"));
        ui.add_space(6.0);
        let shown = if *all_visitors { total } else { SHOWN };
        {
            {
                for entry in save.visitors.guest_book.iter().rev().take(shown) {
                    let visits = save.visitors.visits_of(&entry.origin);
                    card(ui, |ui| {
                        ui.small(format!(
                            "{} · {}{}",
                            dates::on(entry.visited_at_utc, offset),
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
                if total > SHOWN {
                    let more = if *all_visitors {
                        "Show fewer".to_owned()
                    } else {
                        format!("Show all {total}")
                    };
                    if ui
                        .link(more)
                        .on_hover_text(format!(
                            "The last {MAX_GUEST_BOOK_ENTRIES} visitors stay on this computer. \
                             A code recreates how a visitor looked and what they were like, and \
                             nothing else."
                        ))
                        .clicked()
                    {
                        *all_visitors = !*all_visitors;
                    }
                }
            }
        }
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
    let offset = local_offset();
    let now = OffsetDateTime::now_utc();
    state.pin_spots.clear();
    // The magnifier in the heading's corner brings out the search and the filters.
    let corner = ui.max_rect().right_top();
    journal::page_heading(
        ui,
        crate::settings::SettingsTab::Journal,
        "Journal",
        "Small moments, written down before anyone forgets them.",
    );
    let filtering =
        state.filter.is_some() || state.kind.is_some() || !state.search.trim().is_empty();
    let search_id = egui::Id::new("journal-search");
    if ui.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
        state.searching = true;
        ui.memory_mut(|memory| memory.request_focus(search_id));
    }
    let lens = egui::Rect::from_min_size(corner + egui::vec2(-34.0, 30.0), egui::vec2(30.0, 30.0));
    let magnifier = companion_page::face_button(
        ui,
        lens,
        egui::Id::new("journal-magnifier"),
        "Search the journal",
    )
    .on_hover_text(format!(
        "Search the journal, or show only some of it ({}F)",
        journal::shortcut_modifier()
    ));
    {
        let open = state.searching || filtering;
        let colour = if open || magnifier.hovered() {
            forest()
        } else {
            muted()
        };
        let painter = ui.painter();
        let glass = lens.center() - egui::vec2(3.0, 3.0);
        painter.circle_stroke(glass, 8.0, egui::Stroke::new(2.5, colour));
        painter.line_segment(
            [glass + egui::vec2(6.0, 6.0), glass + egui::vec2(12.0, 12.0)],
            egui::Stroke::new(3.0, colour),
        );
    }
    if magnifier.clicked() {
        state.searching = !state.searching;
        if state.searching {
            ui.memory_mut(|memory| memory.request_focus(search_id));
        } else {
            state.search.clear();
            state.kind = None;
            state.filter = None;
        }
    }
    // A colony only just begun has nothing written down but its arrival: the founders on their
    // plate and one line, and nothing to look through yet.
    if (save.companion.journal.is_empty() && save.companion.pins.is_empty())
        || (today::just_begun(save, now) && save.companion.pins.is_empty())
    {
        let local_today = today::today(save, now.to_offset(offset).date(), offset);
        let mut here = crate::settings::SettingsTab::Journal;
        today::TodayState::right_now(ui, shell, save, now, &local_today, &mut here);
        journal::write(
            ui,
            format!(
                "Day {}. Nothing written down yet; I'll jot things here as they happen.",
                today::colony_day(save, now)
            ),
        );
        ui.add_space(14.0);
        guest_book(ui, save, shell, outcome, offset, &mut state.all_visitors);
        state.scrapbook(ui, shell, save);
        state.souvenirs(ui, save);
        state.cups(ui, save);
        state.wonders(ui, save, outcome);
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
                // The date and the moment as one line of type, so they share a baseline and the
                // line wraps as a whole in a narrow window.
                let mut line = journal::label_job(
                    &format!("{}   ", dates::on(entry.at, offset)),
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
            let when = dates::kept(entry.at, offset);
            moment_card(
                ui,
                shell,
                save,
                entry,
                &when,
                outcome,
                false,
                &mut state.pin_spots,
            );
        }
        ui.separator();
        ui.add_space(8.0);
    }
    // Search, then the kinds of moment and the companions the journal actually mentions, so a
    // filter never offers an empty result: out from behind the magnifier while asked for, or
    // while something is being searched or filtered.
    if state.searching || filtering {
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
                if state.search.is_empty() {
                    state.searching = false;
                }
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
    if filtering {
        journal::write(
            ui,
            RichText::new(format!("Showing {} of {unpinned} moments", showing.len()))
                .small()
                .color(muted()),
        );
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
    let reading = reading.cloned().unwrap_or_default();
    // Day by day, under each day's heading; a day with only one moment folds to the one line,
    // its day written where its time would be.
    let mut days: Vec<(String, Vec<&JournalEntry>)> = Vec::new();
    for entry in showing {
        let day = day_heading(entry.at, now, offset);
        match days.last_mut() {
            Some((heading, entries)) if *heading == day => entries.push(entry),
            _ => days.push((day, vec![entry])),
        }
    }
    for (index, (day, entries)) in days.into_iter().enumerate() {
        // A line left blank before each day.
        ui.add_space(4.0);
        // The newest days' best moment, taped in as a picture: a landmark first, then anything
        // else worth one.
        let pictured = (index < PICTURED_DAYS && !filtering)
            .then(|| {
                let worth =
                    |e: &&&JournalEntry| formiga_art::Vignette::for_moment(&e.moment).is_some();
                entries
                    .iter()
                    .filter(worth)
                    .find(|e| e.moment.is_landmark())
                    .or_else(|| entries.iter().find(worth))
                    .copied()
            })
            .flatten();
        if let [entry] = entries.as_slice() {
            if let Some(entry) = pictured {
                state.vignette(ui, shell, save, entry);
            }
            let when = format!(
                "{} · {}",
                short_day(entry.at, now, offset),
                dates::clock(entry.at, offset)
            );
            moment_card(
                ui,
                shell,
                save,
                entry,
                &when,
                outcome,
                reading.is_new(entry),
                &mut state.pin_spots,
            );
            continue;
        }
        journal::kicker(ui, &day);
        if let Some(entry) = pictured {
            state.vignette(ui, shell, save, entry);
        }
        for entry in entries {
            moment_card(
                ui,
                shell,
                save,
                entry,
                &dates::clock(entry.at, offset),
                outcome,
                reading.is_new(entry),
                &mut state.pin_spots,
            );
        }
    }
    ui.add_space(4.0);
    journal::write(
        ui,
        RichText::new(format!(
            "The most recent {MAX_JOURNAL_ENTRIES} moments stay on this computer, holding on to \
             up to {MAX_LANDMARK_ENTRIES} milestones longer, plus up to {MAX_PINNED_ENTRIES} you \
             keep. Missed time is never replayed."
        ))
        .small()
        .color(muted()),
    );
    ui.add_space(20.0);
    ui.separator();
    ui.add_space(14.0);
    guest_book(ui, save, shell, outcome, offset, &mut state.all_visitors);
    state.scrapbook(ui, shell, save);
    state.souvenirs(ui, save);
    state.cups(ui, save);
    state.wonders(ui, save, outcome);
}

impl JournalState {
    /// The finds themselves, for the Journal: only what has actually turned up, newest first,
    /// with who found it and when. What is still to find lives in the Collection.
    pub fn scrapbook(&mut self, ui: &mut Ui, shell: &mut Shell, save: &SaveFile) {
        let offset = local_offset();
        let atlas = shell.trinket_atlas(ui, save);
        let mut found = save.companion.scrapbook.clone();
        found.sort_by_key(|record| std::cmp::Reverse((record.first_at, record.variant)));
        // Nothing found yet: no scrapbook until the first find.
        if found.is_empty() {
            return;
        }
        journal::kicker(ui, &format!("The scrapbook · {} found", found.len()));
        ui.add_space(6.0);
        // Pasted in like snapshots, a row at a time: each find drawn large over its name, every
        // other one taped, with what it is and who found it when on hover.
        let side = 48.0;
        let tile = 104.0;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(14.0, 14.0);
            for (index, record) in found.iter().enumerate() {
                let Some(info) = formiga_core::trinket_info(record.variant) else {
                    continue;
                };
                // The finder's name was kept when it was found, so a companion who has since left
                // is still the one credited.
                let finder = record
                    .finder
                    .and_then(|id| save.creatures.iter().find(|c| c.id == id))
                    .map_or(record.finder_name.as_str(), |c| c.name.as_str());
                make_room(ui, tile + 8.0);
                let pasted = paper::pasted(ui, tile, 0.0, index % 2 == 0, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add(Shell::trinket_image(&atlas, info.variant, side));
                        ui.label(RichText::new(info.name).color(ink()));
                        ui.label(
                            RichText::new(dates::on(record.first_at, offset))
                                .small()
                                .color(muted()),
                        );
                    });
                });
                pasted.response.on_hover_text(format!(
                    "{} Found by {finder} · {}",
                    info.description,
                    dates::on(record.first_at, offset)
                ));
            }
        });
        ui.add_space(8.0);
        ui.small("Only the first find of each kind is recorded, and only on this computer.");
    }

    /// The souvenirs the colony has brought home from Formiga Hill, newest first, each with the day
    /// it came. They are only here to be looked at, and the page has no place for them at all until
    /// one has come home, so a colony that has never been to the Hill has nothing here to fill.
    pub fn souvenirs(&mut self, ui: &mut Ui, save: &SaveFile) {
        let kept = &save.trips.souvenirs;
        if kept.is_empty() {
            return;
        }
        let offset = local_offset();
        let strip = self
            .souvenir_strip
            .get_or_insert_with(|| {
                upload(ui.ctx(), "colony-souvenirs", &formiga_art::souvenir_strip())
            })
            .clone();
        ui.add_space(14.0);
        journal::kicker(ui, &format!("Souvenirs · {} from Formiga Hill", kept.len()));
        ui.add_space(6.0);
        let kinds = Souvenir::ALL.len() as f32;
        for record in kept.iter().rev() {
            let index = Souvenir::ALL
                .iter()
                .position(|souvenir| *souvenir == record.souvenir)
                .unwrap_or_default() as f32;
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    let uv = egui::Rect::from_min_max(
                        egui::pos2(index / kinds, 0.0),
                        egui::pos2((index + 1.0) / kinds, 1.0),
                    );
                    // Six screen points to each pixel of the picture and its velvet.
                    let side = formiga_art::SOUVENIR_TILE as f32 * 6.0;
                    ui.add(
                        egui::Image::new(&strip)
                            .uv(uv)
                            .maintain_aspect_ratio(false)
                            .corner_radius(3.0)
                            .fit_to_exact_size(egui::vec2(side, side)),
                    );
                    ui.vertical(|ui| {
                        ui.strong(record.souvenir.name());
                        ui.label(record.souvenir.description());
                        ui.small(format!(
                            "Brought home from Formiga Hill · {}",
                            dates::on(record.brought_home_at_utc, offset)
                        ));
                    });
                });
            });
            ui.add_space(8.0);
        }
        ui.small("Each souvenir comes home once, and is kept only on this computer.");
    }

    /// What the colony has won in Formiga Battle: its trophies and medals, newest first, each with
    /// the day it was won, under a line of the Handler's name, the cups won and the points. Like
    /// the souvenirs, there is no place for them at all until the first has come home.
    pub fn cups(&mut self, ui: &mut Ui, save: &SaveFile) {
        let battle = &save.battle;
        let kept = &battle.keepsakes;
        if kept.is_empty() {
            return;
        }
        let offset = local_offset();
        let strip = self
            .cup_strip
            .get_or_insert_with(|| upload(ui.ctx(), "colony-cups", &formiga_art::cup_strip()))
            .clone();
        ui.add_space(14.0);
        journal::kicker(ui, &format!("Cups · {} from Formiga Battle", kept.len()));
        ui.add_space(4.0);
        ui.label(RichText::new(battle_standing(&battle.record, kept)).color(muted()));
        ui.add_space(6.0);
        let kinds = BattleKeepsake::ALL.len() as f32;
        for record in kept.iter().rev() {
            let index = BattleKeepsake::ALL
                .iter()
                .position(|keepsake| *keepsake == record.keepsake)
                .unwrap_or_default() as f32;
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    let uv = egui::Rect::from_min_max(
                        egui::pos2(index / kinds, 0.0),
                        egui::pos2((index + 1.0) / kinds, 1.0),
                    );
                    // Five screen points to each pixel of the picture and its shelf.
                    let side = formiga_art::CUP_TILE as f32 * 5.0;
                    ui.add(
                        egui::Image::new(&strip)
                            .uv(uv)
                            .maintain_aspect_ratio(false)
                            .corner_radius(3.0)
                            .fit_to_exact_size(egui::vec2(side, side)),
                    );
                    ui.vertical(|ui| {
                        ui.strong(record.keepsake.name());
                        ui.label(record.keepsake.description());
                        ui.small(format!(
                            "Won in Formiga Battle · {}",
                            dates::on(record.earned_at_utc, offset)
                        ));
                    });
                });
            });
            ui.add_space(8.0);
        }
        ui.small("Each keepsake comes home once, and is kept only on this computer.");
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
    /// a shadow with a hint. One that has turned up can be put out on the desktop from here, up
    /// to three at a time, and put away again.
    pub fn wonders(&mut self, ui: &mut Ui, save: &SaveFile, outcome: &mut SettingsOutcome) {
        let offset = local_offset();
        let sheet = self.wonder_sheet(ui, save);
        let found = &save.companion.wonders;
        // No page of wonders until the first one has turned up.
        if found.is_empty() {
            return;
        }
        ui.add_space(14.0);
        journal::kicker(
            ui,
            &format!(
                "Wonders · {} of {} found",
                found.len(),
                WonderKind::ALL.len()
            ),
        );
        ui.small(
            "Now and then something to play on turns up for a little while — a chair, a leaf sled, a \
             fountain — and whoever it turned up for goes straight over to have a go. Put one you \
             have found out on the desktop and it stays: click it there to call somebody over, move \
             it, or put it away.",
        );
        let out = &save.companion.placed_wonders;
        let room = out.len() < formiga_core::MAX_PLACED_WONDERS;
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
                            ui.small(format!(
                                "First found by {finder} · {} · {} {}",
                                dates::on(record.first_at, offset),
                                record.goes,
                                if record.goes == 1 { "go" } else { "goes" }
                            ));
                            ui.horizontal_wrapped(|ui| {
                                if out.iter().any(|placed| placed.kind == kind) {
                                    if ui.button("Put away").clicked() {
                                        outcome.put_away_wonder = Some(kind);
                                    }
                                    ui.small("Out on the desktop");
                                } else if ui
                                    .add_enabled(room, egui::Button::new("Put it out"))
                                    .on_disabled_hover_text(
                                        "Three are out already. Put one away first.",
                                    )
                                    .clicked()
                                {
                                    outcome.put_out_wonder = Some(kind);
                                }
                            });
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

/// The line over the cups: the Handler, if they have signed their name, how many cups the colony
/// has won (a trophy each), and its points on the Open Circuit.
fn battle_standing(
    record: &formiga_core::BattleRecord,
    kept: &[formiga_core::BattleKeepsakeRecord],
) -> String {
    let won = kept
        .iter()
        .filter(|record| record.keepsake.id().starts_with("battle.trophy."))
        .count();
    let cups = match won {
        1 => "1 cup won".to_owned(),
        won => format!("{won} cups won"),
    };
    let points = match record.circuit_points {
        1 => "1 circuit point".to_owned(),
        points => format!("{points} circuit points"),
    };
    match &record.handler {
        Some(handler) => format!("Handler {} · {cups} · {points}", handler.name),
        None => format!("{cups} · {points}"),
    }
}
