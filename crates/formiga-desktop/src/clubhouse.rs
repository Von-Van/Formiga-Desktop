//! On-demand native UI resources. No resources or timers are added to desktop overlays.
use crate::settings::{GenerationPreview, PreviewAcceptance, SettingsOutcome};
use egui::{Color32, RichText, TextureHandle, Ui};
use formiga_art::{
    COLONY_OBJECT_SIZE, Canvas, ColonyObjectRenderer, CreatureRenderer, SHELTER_SIZE,
    ShelterRenderer, StickerClip, TRINKET_ATLAS_WIDTH, TRINKET_CELL, TRINKET_FRAME_REST,
    TrinketAtlasRenderer,
};
use formiga_core::*;
use std::collections::BTreeMap;
use time::OffsetDateTime;

pub(crate) mod arrange;
pub(crate) mod collection;
pub(crate) mod home;
pub(crate) mod journal;
pub(crate) mod journal_page;
pub(crate) mod recovery;
pub(crate) mod shell;
pub(crate) mod studio;
pub(crate) mod today;
pub(crate) mod tour;

/// The interface palette. Cream, forest, and mint by daylight; charcoal and sage after dark. The
/// settings window sets this once whenever the preference or the system appearance changes, and
/// the desktop overlays never consult it — creatures look the same either way.
static DARK_INTERFACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_dark_interface(dark: bool) {
    DARK_INTERFACE.store(dark, std::sync::atomic::Ordering::Relaxed);
}

pub fn dark_interface() -> bool {
    DARK_INTERFACE.load(std::sync::atomic::Ordering::Relaxed)
}

fn shade(light: (u8, u8, u8), dark: (u8, u8, u8)) -> Color32 {
    let (r, g, b) = if dark_interface() { dark } else { light };
    Color32::from_rgb(r, g, b)
}

/// The page itself.
pub fn paper() -> Color32 {
    shade((249, 240, 212), (30, 36, 33))
}

/// Body text, and anything that has to be read on `paper`.
pub fn ink() -> Color32 {
    shade((35, 54, 47), (230, 237, 228))
}

/// Headings, banners, and the accent the interface is built around.
pub fn forest() -> Color32 {
    shade((28, 65, 55), (159, 196, 168))
}

/// Selection and the fill behind a chosen control.
pub fn mint() -> Color32 {
    shade((174, 221, 186), (47, 81, 71))
}

/// Hairlines and card edges.
pub fn gold() -> Color32 {
    shade((205, 177, 122), (78, 90, 79))
}

/// The face of a card, a shade away from the page.
pub fn card_fill() -> Color32 {
    shade((255, 248, 228), (38, 46, 42))
}

/// Text that is present but not being asked for attention.
pub fn muted() -> Color32 {
    shade((110, 119, 108), (150, 162, 148))
}

/// The notebook's outlines: plum by daylight, as the creatures are outlined, and sage after dark.
pub fn line() -> Color32 {
    shade((59, 43, 58), (85, 100, 90))
}

/// The ruled lines across a page.
pub fn rule() -> Color32 {
    shade((233, 219, 178), (38, 46, 42))
}

/// The clay a habitat keeps companions out of.
pub fn clay() -> Color32 {
    shade((217, 159, 137), (138, 90, 76))
}

/// Sand: the part of a display outside the habitat.
pub fn ground() -> Color32 {
    shade((219, 210, 185), (43, 51, 47))
}

/// The page's margin line, a little quieter than clay after dark.
pub fn margin_line() -> Color32 {
    shade((217, 159, 137), (94, 64, 56))
}

/// A sticky note: the tour, and the conditions noted on the cover.
pub fn note_fill() -> Color32 {
    shade((246, 220, 154), (74, 68, 51))
}

/// The red of a rubber stamp, and of the dashes round whatever the tour points at.
pub fn stamp() -> Color32 {
    shade((181, 84, 63), (212, 140, 118))
}

/// Somewhere to write: a text field.
pub fn field() -> Color32 {
    shade((255, 250, 234), (23, 28, 26))
}

/// A page tab not yet turned to.
pub fn faint() -> Color32 {
    shade((239, 225, 188), (34, 42, 38))
}

/// The leather cover, its darker grain, and the stitching round it.
pub fn leather() -> Color32 {
    shade((91, 58, 39), (43, 31, 24))
}

pub fn leather_dark() -> Color32 {
    shade((62, 39, 25), (23, 17, 13))
}

pub fn stitch() -> Color32 {
    shade((205, 177, 122), (122, 106, 80))
}

/// The cloth patch on the cover with the notebook's name on it, and the name.
pub fn patch() -> Color32 {
    shade((28, 65, 55), (35, 59, 51))
}

pub fn patch_ink() -> Color32 {
    shade((249, 240, 212), (159, 196, 168))
}

/// A tab already turned past, pressed into the leather, and the lettering pressed into it.
pub fn indent() -> Color32 {
    shade((74, 47, 31), (31, 22, 17))
}

pub fn deboss() -> Color32 {
    shade((138, 106, 82), (90, 70, 54))
}

/// The tape holding a note to the cover.
pub fn tape() -> Color32 {
    if dark_interface() {
        Color32::from_rgba_unmultiplied(150, 160, 140, 89)
    } else {
        Color32::from_rgba_unmultiplied(240, 230, 190, 209)
    }
}

/// The notebook: what every page shares, each page's own state, and the notebook's own chrome.
///
/// A page is drawn by its own state and handed the shell, so it can change itself and what every
/// page shares — the portraits, the sheet of finds, the tour, the footer's line of feedback — and
/// nothing that belongs to another page.
#[derive(Default)]
pub struct Clubhouse {
    /// What every page shares.
    pub(crate) shell: shell::Shell,
    /// Each page's own state.
    pub(crate) today: today::TodayState,
    pub(crate) journal: journal_page::JournalState,
    pub(crate) colony: collection::ColonyState,
    pub(crate) studio: studio::StudioState,
    pub(crate) home: home::HomeState,
    pub(crate) recovery: recovery::RecoveryState,
    /// What was new when the reader turned to Today or the Journal, kept while they read either.
    /// Shared by those two pages alone, and handed to each.
    pub(crate) reading: Option<today::Reading>,
    /// The last change to the colony that can still be taken back, as the footer names it, and
    /// how many earlier ones can be taken back after it. Set by the app before every frame, from
    /// the world, which is where the changes are kept.
    pub last_edit: Option<String>,
    pub earlier_edits: usize,
    /// Formiga Home as the notebook may offer it: nothing at all while it is not installed. Set
    /// by the app before every frame.
    pub formiga_home: FormigaHomeView,
    /// Formiga Farm as the notebook may offer it: nothing at all while it is not installed. Set
    /// by the app before every frame.
    pub formiga_farm: FormigaFarmView,
    /// The page being turned, while it turns, and the page shown last frame, to notice a new one.
    pub(crate) page_turn: Option<journal::PageTurn>,
    pub(crate) shown_page: Option<crate::settings::SettingsTab>,
}

/// What the notebook may offer about Formiga Home.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FormigaHomeView {
    /// Formiga Home is installed.
    pub installed: bool,
    /// Whose house is open in it, while one is.
    pub open: Option<CreatureId>,
    /// Whether a house could be opened just now: none is open, and the colony is here.
    pub can_open: bool,
}

/// What the notebook may offer about Formiga Farm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FormigaFarmView {
    /// Formiga Farm is installed.
    pub installed: bool,
    /// What is open in it, while something is.
    pub open: Option<crate::farm::FarmRequest>,
    /// Whether it could be opened just now: nothing is open in it, and the colony is here.
    pub can_open: bool,
}

/// What a companion's body is called: its sculpted form's plan, its recipe's body, or the family
/// a look from before recipes belongs to.
pub fn body_label(appearance: &formiga_core::AppearanceGenome) -> String {
    match (&appearance.sculpt, appearance.design) {
        (Some(sculpt), _) => sculpt.plan.label().to_owned(),
        (None, Some(design)) => design.body.label().to_owned(),
        (None, None) => words(&format!("{:?}", appearance.family)),
    }
}

pub fn upload(context: &egui::Context, name: &str, canvas: &Canvas) -> TextureHandle {
    context.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied(
            [canvas.width() as usize, canvas.height() as usize],
            &canvas.rgba_bytes(),
        ),
        egui::TextureOptions::NEAREST,
    )
}
impl Clubhouse {
    pub fn notify(&mut self, message: impl Into<String>) {
        self.shell.notify(message);
    }

    /// Every texture the notebook is holding, on every page.
    pub fn texture_ids(&self) -> Vec<egui::TextureId> {
        self.shell
            .texture_ids()
            .chain(self.studio.texture_ids())
            .chain(self.home.texture_ids())
            .chain(self.journal.texture_ids())
            .chain(self.colony.texture_ids())
            .collect()
    }

    /// Let go of every texture, on every page; each is drawn again when next needed.
    pub fn release_images(&mut self) {
        self.shell.release_images();
        self.studio.release_images();
        self.home.release_images();
        self.journal.release_images();
        self.colony.release_images();
    }
}

/// A tile in a wrapped row: a space of its own, filled and edged, for whatever is painted into it,
/// and something to click. A `Frame` works out where it goes before the row has decided whether it
/// still fits, so a row of them never wraps and runs off the side of the page; a space allocated
/// whole does wrap, like a word.
///
/// `label` is what a screen reader calls it, and `selected` whether it is the one chosen; a tile
/// with the keyboard on it is ringed in the accent, like every other control.
pub(crate) fn tile(
    ui: &mut Ui,
    size: egui::Vec2,
    fill: Color32,
    edge: Color32,
    label: &str,
    selected: bool,
) -> (egui::Response, egui::Rect) {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, label)
    });
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, fill);
    painter.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
    if response.has_focus() {
        painter.rect_stroke(
            rect.expand(2.0),
            0.0,
            egui::Stroke::new(2.0, forest()),
            egui::StrokeKind::Outside,
        );
    }
    (response, rect)
}

/// How wide `text` is drawn on a single line in `ui`.
pub(crate) fn text_width(ui: &Ui, text: impl Into<egui::WidgetText>) -> f32 {
    text.into()
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        )
        .size()
        .x
}

/// Starts the next row of a wrapping layout if what is about to go in it, `width` wide, has no
/// room left in this one. egui learns how big a frame, a column or a combo box is only once it
/// has been placed, so one that did not fit ran on past the end of the row and took the page
/// past the edge of the window with it, where the scroll area cut it off.
pub(crate) fn make_room(ui: &mut Ui, width: f32) {
    let row_begun = ui.cursor().min.x > ui.max_rect().min.x + 0.5;
    if row_begun && ui.available_size_before_wrap().x < width {
        ui.end_row();
    }
}

/// How wide a combo box showing `selected` is drawn: its text and its arrow, and never less than
/// egui's own width for one, inside the padding of a button.
pub(crate) fn combo_width(ui: &Ui, selected: &str) -> f32 {
    let spacing = ui.spacing();
    let padding = 2.0 * spacing.button_padding.x;
    let text = egui::WidgetText::from(selected)
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Button,
        )
        .size()
        .x;
    // A point to spare, so the text is never a hair too wide for the room it is given.
    (text + spacing.icon_spacing + spacing.icon_width).max(spacing.combo_width - padding)
        + padding
        + 1.0
}

/// How much wider a [`card`] is than what is in it: its margin and its border, either side.
pub(crate) const CARD_EDGES: f32 = 2.0 * (14.0 + 2.0);

pub fn card<R>(ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let shown = egui::Frame::new()
        .fill(card_fill())
        .inner_margin(14)
        .show(ui, contents);
    // A card in the notebook is a slip of paper with the stepped edge every box has.
    let mut fills = journal::Fills::default();
    journal::stepped_outline(&mut fills, shown.response.rect, 2.0, line());
    fills.paint(ui.painter());
    shown
}
/// A [`card`] as wide as the page, for a page whose sections should read as one column rather
/// than a ragged stack of slips.
pub fn wide_card<R>(ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let width = ui.available_width() - CARD_EDGES;
    card(ui, |ui| {
        ui.set_min_width(width.max(0.0));
        contents(ui)
    })
}
pub fn words(value: &str) -> String {
    let mut result = String::new();
    for (index, c) in value.chars().enumerate() {
        if index > 0 && c.is_uppercase() {
            result.push(' ');
        }
        result.push(c);
    }
    result
}
/// The reader's own clock, so a journal reads in local days rather than in UTC.
pub fn local_offset() -> time::UtcOffset {
    time::UtcOffset::local_offset_at(OffsetDateTime::now_utc()).unwrap_or(time::UtcOffset::UTC)
}

/// What one moment says. Names are read from the colony as it is now, so a rename reads through.
pub fn moment_text(save: &SaveFile, entry: &JournalEntry) -> String {
    let name = entry
        .creature
        .and_then(|id| save.creatures.iter().find(|c| c.id == id))
        .map_or("A past companion", |c| c.name.as_str());
    match entry.moment {
        JournalMoment::Arrival => format!("{name} joined the colony"),
        JournalMoment::Friendship(friend_id) => format!(
            "{name} grew close to {}",
            save.creatures
                .iter()
                .find(|c| c.id == friend_id)
                .map_or("a past companion", |c| c.name.as_str())
        ),
        JournalMoment::Discovery => format!("{name} found a little treasure"),
        JournalMoment::Preference(descriptor) => format!(
            "{name} is becoming known for: {}",
            descriptor.label().to_lowercase()
        ),
        JournalMoment::Ritual(kind) => format!(
            "The colony shared {}",
            match kind {
                RitualKind::Picnic => "a picnic",
                RitualKind::GroupNap => "a group nap",
                RitualKind::FloorRace => "a little race",
                RitualKind::ShelterGathering => "a gathering at home",
                RitualKind::Catch => "a game of catch",
                RitualKind::GroupPresentation => "their discoveries",
                RitualKind::HatchDay => "a hatch-day celebration",
                RitualKind::QuietDayHuddle => "a quiet huddle",
                RitualKind::LateNightSleepPile => "a late-night sleep pile",
                RitualKind::Dance => "a dance",
            }
        ),
        JournalMoment::Object(kind) => format!(
            "A {} found a place beside the home",
            words(&format!("{kind:?}")).to_lowercase()
        ),
        JournalMoment::Decoration(kind) => format!(
            "The home earned a {}",
            words(&format!("{kind:?}")).to_lowercase()
        ),
        JournalMoment::Unlocked(item) => format!(
            "Something new for the village: {}",
            item.label().to_lowercase()
        ),
        // A visitor is never a colony member, so the moment carries the name it went by.
        JournalMoment::Visit(ref visitor) => format!("{visitor} came by the houses"),
        JournalMoment::Revisit(ref visitor, visit) => format!(
            "{visitor} came back to visit, for the {} time",
            ordinal(u32::from(visit))
        ),
        JournalMoment::Habit(habit) => format!(
            "{name} picked up a little habit: {}",
            habit.label().to_lowercase()
        ),
        JournalMoment::Wonder(kind) => {
            format!("{name} found a wonder: the {}", kind.label().to_lowercase())
        }
        // Always Desktop's own words: nothing Formiga Hill sent is copied into the journal.
        JournalMoment::Trip => "The colony took the train to Formiga Hill and came home".to_owned(),
        // Nor anything Formiga Home sent: only that the owner was there, and whose house it was.
        JournalMoment::HouseVisit => match entry
            .creature
            .and_then(|id| save.creatures.iter().find(|c| c.id == id))
        {
            Some(keeper) => format!("You spent a while inside {}'s house", keeper.name),
            None => "You spent a while inside a house in the village".to_owned(),
        },
        // Nor anything Formiga Farm sent: only that a companion has a new look.
        JournalMoment::NewLook => format!("{name} has a new look, shaped in Formiga Farm"),
    }
}

/// "second", "third", … "12th".
pub(crate) fn ordinal(n: u32) -> String {
    const WORDS: [&str; 11] = [
        "", "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth",
        "tenth",
    ];
    if let Some(word) = WORDS.get(n as usize).filter(|w| !w.is_empty()) {
        return (*word).to_owned();
    }
    let suffix = match (n % 10, n % 100) {
        (1, x) if x != 11 => "st",
        (2, x) if x != 12 => "nd",
        (3, x) if x != 13 => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// Charcoal or cream, and how large the words are. These change the settings window only; the
/// creatures on the desktop are untouched by either.
pub fn appearance_controls(ui: &mut Ui, save: &SaveFile, outcome: &mut SettingsOutcome) {
    let mut appearance = save.companion.appearance;
    card(ui, |ui| {
        ui.strong("Appearance");
        ui.small("Applies to this window. Your colony looks the same either way.");
        ui.horizontal_wrapped(|ui| {
            ui.label("Theme");
            for (choice, name) in [
                (ThemeChoice::System, "Match system"),
                (ThemeChoice::Light, "Cream"),
                (ThemeChoice::Dark, "Charcoal"),
            ] {
                ui.selectable_value(&mut appearance.theme, choice, name);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Text size");
            for scale in [100u8, 110, 125, 150] {
                ui.selectable_value(&mut appearance.text_scale, scale, format!("{scale}%"));
            }
        });
        ui.checkbox(
            &mut appearance.sprite_outline,
            "Outline creatures for busy wallpaper",
        )
        .on_hover_text(
            "Draws a soft edge behind each creature so it stays readable on bright or detailed \
             backgrounds. It never changes where you can click.",
        );
    });
    if appearance != save.companion.appearance {
        appearance.normalize();
        outcome.appearance = Some(appearance);
    }
}

const WEEKDAYS: [(&str, u8); 7] = [
    ("Mon", 0),
    ("Tue", 1),
    ("Wed", 2),
    ("Thu", 3),
    ("Fri", 4),
    ("Sat", 5),
    ("Sun", 6),
];

/// Opt-in weekday changes between the two saved routines. Nothing here runs on a timer: the next
/// change is simply a moment the app already knows to wake for.
pub fn schedule_controls(ui: &mut Ui, save: &SaveFile, outcome: &mut SettingsOutcome) {
    let mut schedule = save.companion.schedule.clone();
    let offset = local_offset();
    let saved = [
        save.companion.modes[0].is_some(),
        save.companion.modes[1].is_some(),
    ];
    card(ui, |ui| {
        ui.strong("A weekly routine");
        ui.small(
            "Move between your saved Work and Relax routines at times you choose. Everything \
             else — showing the colony, pausing, a quiet moment — stays yours.",
        );
        ui.checkbox(&mut schedule.enabled, "Follow a weekly routine");
        if !schedule.enabled {
            return;
        }
        if !saved[0] || !saved[1] {
            ui.colored_label(
                forest(),
                "Save both a Work and a Relax routine above for this to have somewhere to go.",
            );
        }
        // What is in force now, and what happens next.
        let local = OffsetDateTime::now_utc().to_offset(offset);
        let active = schedule.intended(local);
        ui.horizontal_wrapped(|ui| {
            ui.label("Now:");
            ui.strong(match active {
                Some(0) => "Work",
                Some(_) => "Relax",
                None => "Nothing scheduled yet",
            });
            if schedule.overridden {
                ui.colored_label(forest(), "· changed by hand");
                if ui.small_button("Follow the routine again").clicked() {
                    outcome.resume_routine = true;
                }
            }
        });
        if let Some((at, preset)) = schedule.next_after(local) {
            ui.small(format!(
                "Next: {} at {:02}:{:02} on {}",
                if preset == 0 { "Work" } else { "Relax" },
                at.hour(),
                at.minute(),
                at.date()
            ));
        }
        ui.add_space(8.0);
        let mut remove = None;
        for (index, transition) in schedule.transitions.iter_mut().enumerate() {
            ui.horizontal_wrapped(|ui| {
                for (name, bit) in WEEKDAYS {
                    let mut on = transition.days & (1 << bit) != 0;
                    if ui.selectable_label(on, name).clicked() {
                        on = !on;
                        if on {
                            transition.days |= 1 << bit;
                        } else {
                            transition.days &= !(1 << bit);
                        }
                    }
                }
                let (mut hour, mut minute) = (transition.minute / 60, transition.minute % 60);
                ui.add(egui::DragValue::new(&mut hour).range(0..=23).prefix("at "));
                ui.add(egui::DragValue::new(&mut minute).range(0..=59).prefix(":"));
                transition.minute = (hour.min(23) * 60 + minute.min(59)).min(1439);
                ui.selectable_value(&mut transition.preset, 0, "Work");
                ui.selectable_value(&mut transition.preset, 1, "Relax");
                if ui.small_button("Remove").clicked() {
                    remove = Some(index);
                }
            });
            if transition.days == 0 {
                ui.small(
                    RichText::new("Choose at least one day for this change to mean anything.")
                        .color(muted()),
                );
            }
        }
        if let Some(index) = remove {
            schedule.transitions.remove(index);
        }
        let room = schedule.transitions.len() < MAX_SCHEDULED_TRANSITIONS;
        let add = ui.add_enabled(room, egui::Button::new("Add a change").small());
        if add.clicked() {
            schedule.transitions.push(ScheduledTransition {
                days: 0b0011111,
                minute: 9 * 60,
                preset: 0,
            });
        }
        if !room {
            add.on_hover_text(format!(
                "A week holds {MAX_SCHEDULED_TRANSITIONS} changes. Remove one to add another."
            ));
        }
        ui.small("Times are your own local time. A change you miss while away is not replayed.");
    });
    if schedule != save.companion.schedule {
        outcome.schedule = Some(schedule);
    }
}

pub fn quiet_controls(ui: &mut Ui, save: &SaveFile, outcome: &mut SettingsOutcome) {
    card(ui, |ui| {
        ui.strong("A quiet moment");
        if let Some(until) = save.companion.quiet_until {
            let minutes = (until - OffsetDateTime::now_utc()).whole_minutes().max(0) + 1;
            ui.label(format!("Settling at home · about {minutes} min remaining"));
            if ui.button("Return to normal").clicked() {
                outcome.quiet_minutes = Some(0);
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(30));
        } else {
            ui.label("Let everyone settle at home, then return to their usual adventures.");
            ui.horizontal(|ui| {
                for minutes in [15, 30, 60] {
                    if ui.button(format!("{minutes} minutes")).clicked() {
                        outcome.quiet_minutes = Some(minutes);
                    }
                }
            });
        }
    });
}
pub fn habitat_map(ui: &mut Ui, policy: &HabitatPolicy, monitors: &[MonitorInfo]) {
    if monitors.is_empty() {
        return;
    }
    let min_x = monitors
        .iter()
        .map(|m| m.bounds.x)
        .fold(f32::INFINITY, f32::min);
    let min_y = monitors
        .iter()
        .map(|m| m.bounds.y)
        .fold(f32::INFINITY, f32::min);
    let max_x = monitors
        .iter()
        .map(|m| m.bounds.right())
        .fold(f32::NEG_INFINITY, f32::max);
    let max_y = monitors
        .iter()
        .map(|m| m.bounds.bottom())
        .fold(f32::NEG_INFINITY, f32::max);
    let (area, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 176.0),
        egui::Sense::hover(),
    );
    let factor =
        ((area.width() - 20.0) / (max_x - min_x).max(1.0)).min(140.0 / (max_y - min_y).max(1.0));
    for (index, monitor) in monitors.iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            area.min
                + egui::vec2(
                    10.0 + (monitor.bounds.x - min_x) * factor,
                    (monitor.bounds.y - min_y) * factor,
                ),
            egui::vec2(
                monitor.bounds.width * factor,
                monitor.bounds.height * factor,
            ),
        );
        ui.painter().rect_filled(rect, 0.0, ground());
        for region in accessible_regions(policy, monitor) {
            let zone = egui::Rect::from_min_size(
                rect.min
                    + egui::vec2(
                        (region.x - monitor.bounds.x) * factor,
                        (region.y - monitor.bounds.y) * factor,
                    ),
                egui::vec2(region.width * factor, region.height * factor),
            );
            ui.painter().rect_filled(zone.intersect(rect), 0.0, mint());
        }
        for zone in policy.zones.iter().filter(|z| {
            z.enabled && z.display == monitor.display_key && z.kind == HabitatZoneKind::Excluded
        }) {
            let bounds = zone.normalized_bounds;
            let usable = monitor.usable_bounds;
            let zone = egui::Rect::from_min_size(
                rect.min
                    + egui::vec2(
                        (usable.x - monitor.bounds.x + bounds.x * usable.width) * factor,
                        (usable.y - monitor.bounds.y + bounds.y * usable.height) * factor,
                    ),
                egui::vec2(
                    bounds.width * usable.width * factor,
                    bounds.height * usable.height * factor,
                ),
            );
            ui.painter().rect_filled(zone.intersect(rect), 0.0, clay());
        }
        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(2.0, line()),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("Display {}", index + 1),
            egui::FontId::proportional(13.0),
            ink(),
        );
    }
    ui.small("Mint: allowed · Clay: excluded · Gray: outside the habitat");
    ui.add_space(14.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A keepsake is one cell of a sheet sixteen cells wide. egui measures an image by its whole
    /// texture and not by the part a uv shows, so an image left to keep that sheet's shape is
    /// letterboxed into a sliver and the keepsake inside it is squashed flat. Every keepsake the
    /// scrapbook draws has to come out square.
    #[test]
    fn a_keepsake_is_drawn_square_however_the_sheet_it_is_cut_from_is_shaped() {
        let sheet = egui::vec2(
            TRINKET_ATLAS_WIDTH as f32,
            formiga_art::TRINKET_RESTING_HEIGHT as f32,
        );
        assert!(
            (sheet.x - sheet.y).abs() > 16.0,
            "the sheet has to be out of square for this to be worth pinning"
        );
        let context = egui::Context::default();
        let atlas = context.load_texture(
            "colony-trinkets",
            egui::ColorImage::filled([sheet.x as usize, sheet.y as usize], egui::Color32::WHITE),
            egui::TextureOptions::NEAREST,
        );
        let plenty = egui::vec2(512.0, 512.0);
        for size in [24.0_f32, 48.0, 96.0] {
            for variant in [0, 7, 15] {
                let image = shell::Shell::trinket_image(&atlas, variant, size);
                let drawn = image.calc_size(plenty, image.size());
                assert_eq!(
                    drawn,
                    egui::vec2(size, size),
                    "keepsake {variant} asked for at {size} came out {drawn:?}"
                );
            }
        }
    }
}
