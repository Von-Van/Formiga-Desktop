//! A page that has more to it than reads at a glance keeps the rest in sections. The page shows
//! what matters most first, then a short list of its sections, each a line with its name and how
//! it stands; choosing one opens it in the list's place, under a way back. Back, or Escape, goes
//! back to the list where it was left, with the section just read marked, and each section keeps
//! how far down it was scrolled.
//!
//! Which section is open is kept for as long as the notebook is open, and in nothing saved. A step
//! of the tour that points at something in a section opens that section.

use super::*;
use crate::clubhouse::tour::TourMark;
use crate::settings::SettingsTab;

/// One section of a page, as its line in the list says it.
pub(crate) struct Section {
    /// What the page calls it when it is open; never shown.
    pub(crate) key: &'static str,
    pub(crate) title: String,
    /// How it stands, in a few words: what is chosen there, or how much there is.
    pub(crate) summary: String,
    /// What in it the tour points at, if anything: that step opens the section.
    pub(crate) mark: Option<TourMark>,
}

impl Section {
    pub(crate) fn new(
        key: &'static str,
        title: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            key,
            title: title.into(),
            summary: summary.into(),
            mark: None,
        }
    }

    pub(crate) fn marked(mut self, mark: TourMark) -> Self {
        self.mark = Some(mark);
        self
    }
}

/// Which section of each page is open, and which was read last.
#[derive(Default)]
pub(crate) struct Sections {
    open: BTreeMap<u8, &'static str>,
    last: BTreeMap<u8, &'static str>,
}

fn slot(page: SettingsTab) -> u8 {
    journal::page_number(page) as u8
}

impl Sections {
    /// The section open on `page`, if one is.
    pub(crate) fn open(&self, page: SettingsTab) -> Option<&'static str> {
        self.open.get(&slot(page)).copied()
    }

    pub(crate) fn open_section(&mut self, page: SettingsTab, key: &'static str) {
        self.open.insert(slot(page), key);
        self.last.insert(slot(page), key);
    }

    /// Back to the page's list.
    pub(crate) fn close(&mut self, page: SettingsTab) {
        self.open.remove(&slot(page));
    }

    fn last(&self, page: SettingsTab) -> Option<&'static str> {
        self.last.get(&slot(page)).copied()
    }
}

/// The section open on `page`, if one is, once the tour has had its say: a step of the tour that
/// points into a section opens it. A page draws what matters most above its list only while this
/// is `None`.
pub(crate) fn current(
    shell: &mut shell::Shell,
    page: SettingsTab,
    sections: &[Section],
) -> Option<&'static str> {
    if let Some(mark) = shell.tour.pointing_at()
        && let Some(section) = sections.iter().find(|section| section.mark == Some(mark))
        && shell.sections.open(page) != Some(section.key)
    {
        shell.sections.open_section(page, section.key);
    }
    // A key that no longer names a section, such as Formiga Farm's once Farm is uninstalled,
    // leaves the list showing.
    match shell.sections.open(page) {
        Some(key) if sections.iter().any(|section| section.key == key) => Some(key),
        Some(_) => {
            shell.sections.close(page);
            None
        }
        None => None,
    }
}

/// The list of `page`'s sections, or the open one's name under a way back to the list. Returns
/// the section open, for the page to draw beneath. Escape goes back while nothing is being typed.
pub(crate) fn show(
    ui: &mut Ui,
    shell: &mut shell::Shell,
    page: SettingsTab,
    sections: &[Section],
) -> Option<&'static str> {
    let open = current(shell, page, sections)
        .and_then(|key| sections.iter().find(|section| section.key == key));
    match open {
        Some(section) => {
            let escape = !ui.ctx().text_edit_focused()
                && ui
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            let back = ui
                .button(format!("\u{2039} {}", journal::page_label(page)))
                .on_hover_text("Back to the rest of this page (Esc)");
            if back.clicked() || escape {
                shell.sections.close(page);
                ui.ctx().request_repaint();
                return None;
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new(&section.title)
                    .size(20.0 * journal::text_scale(ui).min(1.2))
                    .color(ink()),
            );
            ui.add_space(6.0);
            Some(section.key)
        }
        None => {
            let last = shell.sections.last(page);
            for section in sections {
                if entry(ui, section, last == Some(section.key)).clicked() {
                    shell.sections.open_section(page, section.key);
                    ui.ctx().request_repaint();
                }
            }
            None
        }
    }
}

/// A section's line in the list: its name, how it stands beneath in quieter words, and a mark at
/// the end that it opens. It lights as a button does when pointed at or reached from the
/// keyboard, and the section read last keeps a bar down its left edge.
fn entry(ui: &mut Ui, section: &Section, last: bool) -> egui::Response {
    let width = ui.available_width();
    let pad = egui::vec2(14.0, 9.0);
    let wrap = (width - pad.x * 2.0 - 24.0).max(80.0);
    let title = egui::WidgetText::from(RichText::new(&section.title).color(ink())).into_galley(
        ui,
        Some(egui::TextWrapMode::Wrap),
        wrap,
        egui::TextStyle::Body,
    );
    let summary = (!section.summary.is_empty()).then(|| {
        egui::WidgetText::from(RichText::new(&section.summary).small().color(muted())).into_galley(
            ui,
            Some(egui::TextWrapMode::Wrap),
            wrap,
            egui::TextStyle::Small,
        )
    });
    let height = pad.y * 2.0
        + title.size().y
        + summary
            .as_ref()
            .map_or(0.0, |summary| summary.size().y + 2.0);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &section.title));
    if ui.is_rect_visible(rect) {
        let lit = response.hovered() || response.has_focus();
        let mut fills = journal::Fills::default();
        let fill = if lit { mint() } else { card_fill() };
        journal::stepped_box(&mut fills, rect, fill, if lit { line() } else { gold() });
        if last {
            fills.rect(
                egui::Rect::from_min_size(
                    rect.min + egui::vec2(2.0, 2.0),
                    egui::vec2(4.0, rect.height() - 4.0),
                ),
                forest(),
            );
        }
        fills.paint(ui.painter());
        let painter = ui.painter();
        let text_at = rect.min + pad;
        painter.galley(text_at, title.clone(), ink());
        if let Some(summary) = summary {
            painter.galley(
                text_at + egui::vec2(0.0, title.size().y + 2.0),
                summary,
                muted(),
            );
        }
        painter.text(
            egui::pos2(rect.right() - pad.x, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            "›",
            egui::FontId::proportional(20.0),
            forest(),
        );
        if response.has_focus() {
            painter.rect_stroke(
                rect.expand(2.0),
                0.0,
                egui::Stroke::new(2.0, forest()),
                egui::StrokeKind::Outside,
            );
        }
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.add_space(2.0);
    response
}
