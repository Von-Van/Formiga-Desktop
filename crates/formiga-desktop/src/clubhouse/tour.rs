//! The tour a new colony is given the first time the settings window opens: a card above each
//! page in turn, walking through the desktop basics and then what every page is for. It turns the
//! pages itself, outlines the part of the page it is talking about and brings it into view,
//! notices when a companion is petted, carried or asked for something on the desktop, and can be
//! skipped at any step or taken again from Preferences.
//!
//! Nothing about where the tour has got to is saved. A colony that has finished or skipped it has
//! `onboarding_complete` set, which is all the save knows; a window closed part way keeps its place
//! for as long as Formiga runs, since closing the settings window only hides it.

use super::*;
use crate::settings::SettingsTab;

/// A part of a settings page the tour can point at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TourMark {
    Name,
    Wardrobe,
    Share,
    Collection,
    Discover,
    Village,
    HomeCorner,
    Quiet,
}

/// Something a step asks to be tried on the desktop, which it notices once it has been.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Try {
    Pet,
    Carry,
    Menu,
}

/// One card of the tour: the page it is shown on, what it says, and what it points at.
struct Step {
    page: SettingsTab,
    title: &'static str,
    text: &'static str,
    mark: Option<TourMark>,
    try_it: Option<Try>,
}

const fn step(page: SettingsTab, title: &'static str, text: &'static str) -> Step {
    Step {
        page,
        title,
        text,
        mark: None,
        try_it: None,
    }
}

const fn marked(
    page: SettingsTab,
    mark: TourMark,
    title: &'static str,
    text: &'static str,
) -> Step {
    Step {
        mark: Some(mark),
        ..step(page, title, text)
    }
}

const fn tried(try_it: Try, title: &'static str, text: &'static str) -> Step {
    Step {
        try_it: Some(try_it),
        ..step(SettingsTab::Colony, title, text)
    }
}

/// The whole tour, in the order it is taken: the desktop first, then every page of this window
/// from the top of the rail to the bottom, and back to where it began.
const STEPS: [Step; 18] = [
    step(
        SettingsTab::Colony,
        "Welcome to your colony",
        "Formiga is a small colony of companions who live on your desktop. They wander about, \
         climb onto your windows, and keep a village of little houses in the corner of your \
         screen. This tour shows you around in a few minutes. Skip it whenever you like, and \
         take it again any time from Preferences.",
    ),
    tried(
        Try::Pet,
        "Say hello",
        "Click your companion on the desktop to pet it. A quick click is enough. Watch its face.",
    ),
    tried(
        Try::Carry,
        "Pick them up",
        "Drag a companion to carry it about. Let go gently to set it down on a window or the \
         desktop; let go on the move and it gets a soft toss.",
    ),
    tried(
        Try::Menu,
        "Ask for something",
        "Right-click a companion (Control-click on a Mac) for its menu: a snack, a toy, sending \
         everyone home, or its own page here. While the houses are out, it offers something for \
         everyone instead: a picnic, a dance or a nap.",
    ),
    step(
        SettingsTab::Colony,
        "The Formiga icon",
        "Formiga keeps an icon in the menu bar, or in the notification area on Windows. Its menu \
         shows and hides the colony, pauses it, brings everyone back down where you can see them, \
         lets them settle at home for half an hour, and opens this window.",
    ),
    marked(
        SettingsTab::Colony,
        TourMark::Name,
        "A page for each companion",
        "Every companion has a page here; choose one from the names at the top. Give them a \
         name, and say where they like to be: wherever they like, close to home, down on the \
         floor, or up high on your windows.",
    ),
    marked(
        SettingsTab::Colony,
        TourMark::Wardrobe,
        "Something to wear",
        "Things the colony finds become things to wear. Point at one to see your companion in \
         it, click to put it on, and click Nothing to take it off.",
    ),
    marked(
        SettingsTab::Colony,
        TourMark::Share,
        "Share a companion",
        "Copy a companion's code for a friend to meet them, or export a picture card or an \
         animated sticker of them.",
    ),
    marked(
        SettingsTab::Colony,
        TourMark::Collection,
        "The collection",
        "Everything the colony could ever find. What has turned up shows in colour, and the \
         rest as a shape with a hint about where to look. Click a find to hang it in the \
         village's trees.",
    ),
    marked(
        SettingsTab::Studio,
        TourMark::Discover,
        "The creature studio",
        "New companions find their way here over time, and you can meet some yourself: \
         discover four at a time, keep the colours or the body you like for the next four, or \
         start from a picture. Adopt one into the colony, or paste a friend's code to meet theirs.",
    ),
    marked(
        SettingsTab::Home,
        TourMark::Village,
        "Your village",
        "The houses in the corner of your screen, just as they stand. Choose Arrange to carry \
         cottages along the row and to put down gardens, spots and ornaments. Click a house to \
         see who lives there, choose what it is built as, and decorate it.",
    ),
    marked(
        SettingsTab::Home,
        TourMark::HomeCorner,
        "Where the village lives",
        "Move the village to the other corner of the screen or to another display, and choose \
         its colours. Further down, export a portrait of everyone or a postcard from the village.",
    ),
    step(
        SettingsTab::Journal,
        "The journal",
        "The colony's days, written down as they happen: arrivals, finds, games and habits. Keep \
         the moments you love, look through everything the colony has found, and see who has \
         come to visit.",
    ),
    step(
        SettingsTab::Habitat,
        "Room to roam",
        "Choose where companions may go: the whole desktop, one display, along the bottom, or \
         places you draw yourself with Edit on desktop. Gather creatures here brings everyone back \
         inside.",
    ),
    step(
        SettingsTab::Applications,
        "Space for your work",
        "Choose which apps' windows cover your companions, and whether they hide behind anything \
         full screen. Formiga looks only at where windows are, never at what is in them.",
    ),
    marked(
        SettingsTab::General,
        TourMark::Quiet,
        "At your own pace",
        "Let everyone settle at home for a quiet while, keep a Work and a Relax routine and \
         switch between them on a schedule, and choose this window's look and text size. Further \
         down: what companions may do on your desktop, and how big they are.",
    ),
    step(
        SettingsTab::About,
        "Backups and updates",
        "Your colony lives only on this computer. Export a full backup to keep it safe or to \
         move it, restore one, and check for updates.",
    ),
    step(
        SettingsTab::Colony,
        "Make yourself at home",
        "That's the tour. Your colony grows by itself: someone new arrives after the first hour \
         and more over the weeks that follow, along with keepsakes to find and new things for the \
         village. Take the tour again any time from Preferences.",
    ),
];

/// What the rail calls each page, for pointing back to the one the tour is waiting on.
fn page_name(page: SettingsTab) -> &'static str {
    match page {
        SettingsTab::Colony => "Your colony",
        SettingsTab::Studio => "Creature studio",
        SettingsTab::Home => "Home & keepsakes",
        SettingsTab::Journal => "Journal",
        SettingsTab::Habitat => "Habitat",
        SettingsTab::Applications => "Applications",
        SettingsTab::General => "Preferences",
        SettingsTab::About => "About & backups",
    }
}

/// How many times each thing a step can ask for has been done, all told.
fn tried_counts(save: &SaveFile, menus_opened: u64) -> [u64; 3] {
    let total = |count: fn(&Creature) -> u32| {
        save.creatures
            .iter()
            .map(|creature| u64::from(count(creature)))
            .sum::<u64>()
    };
    [
        total(|creature| creature.memory.times_petted),
        total(|creature| {
            creature
                .memory
                .placements
                .saturating_add(creature.memory.times_tossed)
        }),
        menus_opened,
    ]
}

const fn try_index(try_it: Try) -> usize {
    match try_it {
        Try::Pet => 0,
        Try::Carry => 1,
        Try::Menu => 2,
    }
}

/// Where the tour has got to. Never saved.
#[derive(Default)]
pub(crate) struct TourState {
    /// The step showing, while the tour is being taken.
    step: Option<usize>,
    /// Finished or skipped in this window, so it is not started again before the save says so.
    done: bool,
    /// The step whose page has been turned to.
    turned: Option<usize>,
    /// The step whose mark has been brought into view.
    scrolled: Option<usize>,
    /// How many times each thing had been tried when this step began. A step notices it being
    /// tried from then on, so taking the tour again asks for it to be tried again.
    counts_at_start: [u64; 3],
    /// How many times a companion's menu has been opened on the desktop, as the app counts it.
    pub(crate) menus_opened: u64,
}

impl TourState {
    /// The mark the step showing points at, if it points at one.
    fn mark(&self) -> Option<TourMark> {
        self.step.and_then(|step| STEPS[step].mark)
    }

    /// How many steps the tour has.
    #[cfg(test)]
    pub(crate) const LENGTH: usize = STEPS.len();
}

impl Clubhouse {
    /// Takes the tour from the beginning, whether or not the colony has taken it before.
    pub fn take_the_tour(&mut self) {
        self.tour.step = Some(0);
        self.tour.done = false;
        self.tour.turned = None;
        self.tour.scrolled = None;
    }

    fn finish_tour(&mut self, outcome: &mut SettingsOutcome) {
        self.tour.step = None;
        self.tour.done = true;
        outcome.complete_onboarding = true;
    }

    /// The tour card, above whichever page is showing, so it stays in view however far the page
    /// is scrolled to what it points at. A colony that has never finished or skipped the tour
    /// starts it; the tour turns to each step's page as the step begins, and anywhere else it
    /// waits, with a way back to it.
    pub(crate) fn tour_card(
        &mut self,
        ui: &mut Ui,
        save: &SaveFile,
        tab: &mut SettingsTab,
        outcome: &mut SettingsOutcome,
    ) {
        if self.tour.step.is_none() && !self.tour.done && !save.companion.onboarding_complete {
            self.take_the_tour();
        }
        let Some(index) = self.tour.step else {
            return;
        };
        let step = &STEPS[index];
        if self.tour.turned != Some(index) {
            self.tour.turned = Some(index);
            self.tour.counts_at_start = tried_counts(save, self.tour.menus_opened);
            if *tab != step.page {
                *tab = step.page;
                ui.ctx().request_repaint();
            }
        }
        card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(format!("TOUR · {} OF {}", index + 1, STEPS.len()))
                    .small()
                    .color(forest()),
            );
            if *tab != step.page {
                ui.label(format!("The tour is waiting on {}.", page_name(step.page)));
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(egui::Button::new("Back to the tour").fill(mint()))
                        .clicked()
                    {
                        *tab = step.page;
                    }
                    if ui.small_button("Skip the tour").clicked() {
                        self.finish_tour(outcome);
                    }
                });
                return;
            }
            // A little larger than the page's own text, and larger with it.
            let body = ui
                .style()
                .text_styles
                .get(&egui::TextStyle::Body)
                .map_or(14.0, |font| font.size);
            ui.label(RichText::new(step.title).size(body * 1.3).color(ink()));
            ui.label(step.text);
            if let Some(try_it) = step.try_it {
                let now = tried_counts(save, self.tour.menus_opened);
                let index = try_index(try_it);
                if now[index] > self.tour.counts_at_start[index] {
                    ui.colored_label(forest(), "Lovely — you've tried it!");
                } else {
                    // Watching for it: the desktop can be tried without this window noticing.
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_secs(1));
                }
            }
            let last = index + 1 == STEPS.len();
            ui.horizontal_wrapped(|ui| {
                if index > 0 && ui.button("Back").clicked() {
                    self.tour.step = Some(index - 1);
                }
                let next = if last { "Finish" } else { "Next" };
                if ui.add(egui::Button::new(next).fill(mint())).clicked() {
                    if last {
                        self.finish_tour(outcome);
                    } else {
                        self.tour.step = Some(index + 1);
                    }
                }
                if !last && ui.small_button("Skip the tour").clicked() {
                    self.finish_tour(outcome);
                }
            });
        });
        ui.add_space(12.0);
    }

    /// Outlines `rect`, a part of the page, if it is what the tour's step is pointing at, and
    /// brings it into view once as the step begins.
    pub(crate) fn tour_mark(&mut self, ui: &Ui, mark: TourMark, rect: egui::Rect) {
        if self.tour.mark() != Some(mark) {
            return;
        }
        ui.painter().rect_stroke(
            rect.expand(4.0),
            6.0,
            egui::Stroke::new(2.0, forest()),
            egui::StrokeKind::Outside,
        );
        if self.tour.scrolled != self.tour.step {
            self.tour.scrolled = self.tour.step;
            ui.scroll_to_rect(rect, Some(egui::Align::Min));
        }
    }
}
