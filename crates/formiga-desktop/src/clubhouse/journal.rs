//! The settings window drawn as a researcher's field notebook: a leather cover with the
//! notebook's name stitched on a patch, the pages' index tabs cut down its edge, and each page
//! ruled paper with a margin line, headed "Field notes · Nº 0X" in a neutral, observing voice.
//!
//! Everything here is a flat fill or a line: the ruled lines, the margin, the stitching, the
//! stepped pixel borders and the turn of a page. The only pictures are the ones the settings
//! window already draws. Turning a page is one quad and two overlays for 0.76 seconds, and under
//! Reduce motion the page is simply there.

use super::*;
use crate::settings::SettingsTab;
use egui::{Align2, FontId, Pos2, Rect, pos2, vec2};

/// The pages in the order their tabs run down the cover.
pub(crate) const PAGES: [SettingsTab; 8] = [
    SettingsTab::Colony,
    SettingsTab::Studio,
    SettingsTab::Home,
    SettingsTab::Journal,
    SettingsTab::Habitat,
    SettingsTab::Applications,
    SettingsTab::General,
    SettingsTab::About,
];

/// A page's place in the notebook, from 1.
pub(crate) fn page_number(page: SettingsTab) -> usize {
    PAGES
        .iter()
        .position(|candidate| *candidate == page)
        .map_or(1, |index| index + 1)
}

/// What a page's tab says.
pub(crate) fn page_label(page: SettingsTab) -> &'static str {
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

/// What kind of field note a page is.
fn page_kind(page: SettingsTab) -> &'static str {
    match page {
        SettingsTab::Colony => "SPECIMEN REGISTER",
        SettingsTab::Studio => "SKETCHBOOK",
        SettingsTab::Home => "THE SETTLEMENT",
        SettingsTab::Journal => "DAILY LOG",
        SettingsTab::Habitat => "RANGE SURVEY",
        SettingsTab::Applications => "COVER",
        SettingsTab::General => "CONDITIONS",
        SettingsTab::About => "COLOPHON",
    }
}

/// How far the window's text is scaled, from 1 to 1.5, read from the size of body text.
pub(crate) fn text_scale(ui: &Ui) -> f32 {
    (ui.text_style_height(&egui::TextStyle::Body) / 14.0).clamp(1.0, 1.5)
}

/// The one size every printed label in the notebook is set at, for text scaled by `scale`: one
/// size rather than a size for each kind of label, so the window's font atlas holds one set of
/// these glyphs and not half a dozen.
pub(crate) fn label_size(scale: f32) -> f32 {
    10.5 * scale
}

/// Rectangles gathered into one mesh. The stitching, the ruling, the dashes and the stepped edges
/// come to hundreds of little rectangles a frame; as one mesh they are a single shape to draw, with
/// hard pixel edges, rather than hundreds to tessellate one at a time.
#[derive(Default)]
pub(crate) struct Fills(egui::Mesh);

impl Fills {
    pub(crate) fn rect(&mut self, rect: Rect, color: Color32) {
        if rect.is_positive() && color.a() > 0 {
            self.0.add_colored_rect(rect, color);
        }
    }

    /// A band shading from `left` to `right` across `rect`.
    pub(crate) fn shade(&mut self, rect: Rect, left: Color32, right: Color32) {
        if !rect.is_positive() {
            return;
        }
        let base = self.0.vertices.len() as u32;
        self.0.colored_vertex(rect.left_top(), left);
        self.0.colored_vertex(rect.right_top(), right);
        self.0.colored_vertex(rect.right_bottom(), right);
        self.0.colored_vertex(rect.left_bottom(), left);
        self.0.add_triangle(base, base + 1, base + 2);
        self.0.add_triangle(base, base + 2, base + 3);
    }

    pub(crate) fn paint(self, painter: &egui::Painter) {
        if !self.0.is_empty() {
            painter.add(egui::Shape::mesh(self.0));
        }
    }
}

/// A printed label: small spaced capitals, the way the notebook's headings are set.
pub(crate) fn label_job(text: &str, size: f32, color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        egui::TextFormat {
            font_id: FontId::monospace(size),
            color,
            extra_letter_spacing: (size * 0.08).round().max(0.5),
            ..Default::default()
        },
    );
    job
}

/// A section's printed label on a page: "TALLY · LIFE HERE", "ON YOUR DESKTOP".
pub(crate) fn kicker(ui: &mut Ui, text: &str) -> egui::Response {
    let size = label_size(text_scale(ui));
    ui.label(label_job(text, size, forest()))
}

/// The head of a page: what kind of field note it is and its number, its name, and a line about
/// it in the notebook's observing voice.
pub(crate) fn page_heading(ui: &mut Ui, page: SettingsTab, heading: &str, observation: &str) {
    let scale = text_scale(ui);
    ui.label(label_job(
        &format!(
            "Field notes · Nº {:02} · {}",
            page_number(page),
            page_kind(page)
        ),
        label_size(scale),
        forest(),
    ));
    ui.add_space(2.0);
    // The name grows with the text, but not all the way: at the largest text its glyphs would be
    // big enough to double the font atlas the whole window draws from.
    ui.label(
        RichText::new(heading)
            .size(30.0 * scale.min(1.2))
            .color(ink()),
    );
    ui.label(RichText::new(observation).color(muted()));
    ui.add_space(12.0);
}

/// What the Your colony page observes about the colony: how many specimens there are, and how
/// many of them are napping.
pub(crate) fn colony_observation(creatures: &[Creature]) -> String {
    const WORDS: [&str; 13] = [
        "No", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten",
        "Eleven", "Twelve",
    ];
    let count = creatures.len();
    let number = WORDS
        .get(count)
        .map_or_else(|| count.to_string(), |word| (*word).to_owned());
    let noun = if count == 1 { "specimen" } else { "specimens" };
    let napping = creatures
        .iter()
        .filter(|creature| creature.state.action == ActionKind::Sleep)
        .count();
    let tail = if count == 0 {
        "The notebook is waiting."
    } else if napping == count {
        "All accounted for, all of them napping."
    } else if napping * 2 > count {
        "All accounted for, most of them napping."
    } else if napping > 0 {
        "All accounted for, one or two of them napping."
    } else {
        "All accounted for, and all of them awake."
    };
    format!("{number} {noun} observed. {tail}")
}

/// A sticky note, taped to the page: the tour. Yellow, with a stepped edge and a shadow, and no
/// wider than a note would be.
pub(crate) fn sticky_note<R>(
    ui: &mut Ui,
    contents: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    let shadow = ui.painter().add(egui::Shape::Noop);
    let width = ui.available_width().min(520.0 * text_scale(ui));
    let shown = egui::Frame::new()
        .fill(note_fill())
        .inner_margin(egui::Margin {
            left: 14,
            right: 14,
            top: 16,
            bottom: 12,
        })
        .show(ui, |ui| {
            ui.set_width(width - 28.0);
            contents(ui)
        });
    let rect = shown.response.rect;
    ui.painter().set(
        shadow,
        egui::Shape::rect_filled(
            rect.translate(vec2(3.0, 4.0)),
            0.0,
            Color32::from_black_alpha(70),
        ),
    );
    let mut fills = Fills::default();
    stepped_outline(&mut fills, rect, 2.0, line());
    fills.rect(
        Rect::from_center_size(pos2(rect.center().x, rect.top()), vec2(48.0, 14.0)),
        tape(),
    );
    fills.paint(ui.painter());
    shown
}

/// A two-pixel outline drawn outside `rect` with its corner pixels left out, the stepped edge every
/// box in the notebook has instead of a rounded one.
pub(crate) fn stepped_outline(fills: &mut Fills, rect: Rect, width: f32, color: Color32) {
    let (l, r, t, b) = (rect.left(), rect.right(), rect.top(), rect.bottom());
    for edge in [
        Rect::from_min_max(pos2(l, t - width), pos2(r, t)),
        Rect::from_min_max(pos2(l, b), pos2(r, b + width)),
        Rect::from_min_max(pos2(l - width, t), pos2(l, b)),
        Rect::from_min_max(pos2(r, t), pos2(r + width, b)),
    ] {
        fills.rect(edge, color);
    }
}

/// A box with a stepped outline, filled.
pub(crate) fn stepped_box(fills: &mut Fills, rect: Rect, fill: Color32, outline: Color32) {
    fills.rect(rect, fill);
    stepped_outline(fills, rect, 2.0, outline);
}

/// A dashed line from `from` to `to`, horizontal or vertical.
pub(crate) fn dashes(
    fills: &mut Fills,
    from: Pos2,
    to: Pos2,
    dash: f32,
    gap: f32,
    width: f32,
    color: Color32,
) {
    let horizontal = (to.y - from.y).abs() < 0.5;
    let length = if horizontal {
        to.x - from.x
    } else {
        to.y - from.y
    };
    let mut at = 0.0;
    while at < length {
        let end = (at + dash).min(length);
        let piece = if horizontal {
            Rect::from_min_max(
                pos2(from.x + at, from.y - width / 2.0),
                pos2(from.x + end, from.y + width / 2.0),
            )
        } else {
            Rect::from_min_max(
                pos2(from.x - width / 2.0, from.y + at),
                pos2(from.x + width / 2.0, from.y + end),
            )
        };
        fills.rect(piece, color);
        at += dash + gap;
    }
}

/// A dashed outline round `rect`.
pub(crate) fn dashed_outline(
    fills: &mut Fills,
    rect: Rect,
    dash: f32,
    gap: f32,
    width: f32,
    color: Color32,
) {
    let r = rect.shrink(width / 2.0);
    dashes(fills, r.left_top(), r.right_top(), dash, gap, width, color);
    dashes(
        fills,
        r.left_bottom(),
        r.right_bottom(),
        dash,
        gap,
        width,
        color,
    );
    dashes(
        fills,
        r.left_top(),
        r.left_bottom(),
        dash,
        gap,
        width,
        color,
    );
    dashes(
        fills,
        r.right_top(),
        r.right_bottom(),
        dash,
        gap,
        width,
        color,
    );
}

/// Where everything in the window goes for a window `width` wide with text scaled by `scale`.
/// At the size it was drawn for the page has the notebook's full margins; in a narrow window, or
/// with large text, the margins and the binding give up their room before the page's contents do.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Spread {
    /// How wide the cover is to the left of the page, its tabs included.
    pub(crate) cover: f32,
    /// How wide the leather is to the right of the page, where it is bound.
    pub(crate) binding: f32,
    /// How far the margin line is in from the page's left edge.
    pub(crate) margin: f32,
    /// How far the writing is in from each side of the page.
    pub(crate) left: f32,
    pub(crate) right: f32,
    /// How far down the window the page begins.
    pub(crate) top: f32,
}

impl Spread {
    pub(crate) fn of(width: f32, scale: f32) -> Self {
        let cover = (46.0 + 150.0 * scale).round();
        let roomy = width - cover - 32.0 >= 640.0;
        if roomy {
            Self {
                cover,
                binding: 32.0,
                margin: 50.0,
                left: 70.0,
                right: 30.0,
                top: 24.0,
            }
        } else {
            Self {
                cover,
                binding: 12.0,
                margin: 22.0,
                left: 36.0,
                right: 12.0,
                top: 24.0,
            }
        }
    }
}

/// The distance between ruled lines, and how far down the page the first one is.
const RULE_PITCH: f32 = 29.0;
const FIRST_RULE: f32 = 98.0;

/// The leather all of the window is bound in, its stitching, and the brass at its corners.
pub(crate) fn paint_cover(painter: &egui::Painter, window: Rect) {
    let mut fills = Fills::default();
    fills.rect(window, leather());
    // The grain: a darker band down the spine.
    fills.rect(
        Rect::from_min_max(
            pos2(window.right() - 8.0, window.top()),
            window.right_bottom(),
        ),
        leather_dark(),
    );
    dashed_outline(&mut fills, window.shrink(10.0), 6.0, 5.0, 2.0, stitch());
    // Brass corners on the cover's outer edge.
    for top in [true, false] {
        let y = if top {
            window.top()
        } else {
            window.bottom() - 22.0
        };
        let bar_y = if top { y } else { y + 16.0 };
        fills.rect(
            Rect::from_min_size(pos2(window.left(), bar_y), vec2(22.0, 6.0)),
            stitch(),
        );
        fills.rect(
            Rect::from_min_size(pos2(window.left(), y), vec2(6.0, 22.0)),
            stitch(),
        );
    }
    fills.paint(painter);
}

/// The cloth patch with the notebook's name on it, at the top of the cover, and where it ends.
fn paint_patch(painter: &egui::Painter, origin: Pos2, scale: f32) -> f32 {
    let width = 166.0 * scale.min(1.25);
    let name = painter.layout_no_wrap(
        "FORMIGA".into(),
        FontId::monospace(20.0 * scale.min(1.25)),
        patch_ink(),
    );
    let sub = painter.layout_no_wrap(
        "FIELD NOTES".into(),
        FontId::monospace(label_size(scale)),
        patch_ink(),
    );
    let height = 12.0 + name.size().y + 7.0 + 2.0 + 6.0 + sub.size().y + 11.0;
    let rect = Rect::from_min_size(origin, vec2(width, height));
    let mut fills = Fills::default();
    fills.rect(
        rect.translate(vec2(3.0, 4.0)),
        Color32::from_black_alpha(90),
    );
    stepped_box(&mut fills, rect, patch(), leather_dark());
    dashed_outline(&mut fills, rect.shrink(4.0), 4.0, 4.0, 2.0, stitch());
    let name_y = rect.top() + 12.0;
    let name_x = rect.center().x - name.size().x / 2.0;
    for x in [name_x - 13.0, name_x + name.size().x + 7.0] {
        fills.rect(
            Rect::from_min_size(pos2(x, name_y + name.size().y / 2.0 - 3.0), vec2(6.0, 6.0)),
            mint(),
        );
    }
    let rule_y = name_y + name.size().y + 7.0;
    fills.rect(
        Rect::from_min_size(pos2(rect.left() + 12.0, rule_y), vec2(width - 24.0, 2.0)),
        stitch().gamma_multiply(0.6),
    );
    fills.paint(painter);
    painter.galley(pos2(name_x, name_y), name, patch_ink());
    painter.galley(
        pos2(rect.center().x - sub.size().x / 2.0, rule_y + 2.0 + 6.0),
        sub,
        patch_ink(),
    );
    rect.bottom()
}

/// One tab down the cover's edge, as it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabState {
    /// The page the notebook is open at: paper, joined to the page.
    Open,
    /// A page further on, not yet turned to.
    Ahead,
    /// A page already turned past, pressed into the leather.
    Behind,
}

/// The cover's tabs, its patch, and the note of conditions tucked under them. Choosing a tab
/// turns to its page. Returns where the open page's tab is, which the page itself draws, since it
/// is joined to the page and sits over the page's edge.
#[allow(clippy::too_many_arguments)]
pub(crate) fn cover_tabs(
    ui: &mut Ui,
    cover: Rect,
    page_left: f32,
    tab: &mut SettingsTab,
    unseen: bool,
    conditions: &[String],
    scale: f32,
) -> (Rect, bool) {
    let painter = ui.painter().clone();
    let patch_bottom = paint_patch(
        &painter,
        pos2(cover.left() + 18.0, cover.top() + 22.0),
        scale,
    );
    let open = page_number(*tab);
    // The tabs share what the cover has below the patch: as tall as the text asks for, and
    // shorter in a short window, so all eight are always there to be turned to.
    let top = patch_bottom + 20.0;
    let pitch = (44.0 * scale).min((cover.bottom() - 12.0 - top) / PAGES.len() as f32);
    let height = pitch + 2.0;
    let mut y = top;
    let mut open_rect = Rect::NOTHING;
    let mut turned = false;
    for page in PAGES {
        let number = page_number(page);
        let state = match number.cmp(&open) {
            std::cmp::Ordering::Equal => TabState::Open,
            std::cmp::Ordering::Greater => TabState::Ahead,
            std::cmp::Ordering::Less => TabState::Behind,
        };
        let rect = match state {
            TabState::Open => Rect::from_min_max(
                pos2(cover.left() + 8.0, y),
                pos2(page_left + 2.0, y + height),
            ),
            TabState::Ahead => {
                Rect::from_min_max(pos2(cover.left() + 8.0, y), pos2(page_left, y + height))
            }
            TabState::Behind => Rect::from_min_max(
                pos2(cover.left() + 18.0, y + 4.0),
                pos2(page_left - 6.0, y + pitch),
            ),
        };
        let label = if page == SettingsTab::Colony && unseen {
            format!("{} •", page_label(page))
        } else {
            page_label(page).to_owned()
        };
        if state == TabState::Open {
            open_rect = rect;
        } else {
            let id = ui.id().with(("journal-tab", number));
            let mut response = ui.interact(rect, id, egui::Sense::click());
            if state == TabState::Behind {
                response = response.on_hover_text(format!("Turn back to {}", page_label(page)));
            }
            paint_tab(
                &painter,
                rect,
                state,
                number,
                &label,
                scale,
                response.hovered(),
            );
            if response.clicked() {
                *tab = page;
                turned = true;
            }
        }
        y += pitch;
    }
    if !conditions.is_empty() {
        paint_conditions(
            &painter,
            pos2(cover.left() + 24.0, y + 26.0),
            (page_left - cover.left() - 44.0).max(120.0),
            cover.bottom() - 16.0,
            conditions,
            scale,
        );
    }
    (open_rect, turned)
}

/// A tab, in whichever state it is in.
fn paint_tab(
    painter: &egui::Painter,
    rect: Rect,
    state: TabState,
    number: usize,
    label: &str,
    scale: f32,
    hovered: bool,
) {
    let (fill, number_color, label_color) = match state {
        TabState::Open => (paper(), forest(), ink()),
        TabState::Ahead => (if hovered { paper() } else { faint() }, muted(), ink()),
        TabState::Behind => (indent(), deboss(), deboss()),
    };
    let mut fills = Fills::default();
    fills.rect(rect, fill);
    match state {
        TabState::Behind => {
            // Pressed in: shadow along the top and left, a glint along the bottom.
            fills.rect(
                Rect::from_min_size(rect.left_top(), vec2(rect.width(), 3.0)),
                Color32::from_black_alpha(97),
            );
            fills.rect(
                Rect::from_min_size(rect.left_top(), vec2(3.0, rect.height())),
                Color32::from_black_alpha(97),
            );
            fills.rect(
                Rect::from_min_max(pos2(rect.left(), rect.bottom() - 2.0), rect.right_bottom()),
                Color32::from_rgba_unmultiplied(255, 225, 190, 20),
            );
            if hovered {
                fills.rect(rect, Color32::from_white_alpha(10));
            }
        }
        TabState::Open | TabState::Ahead => {
            // The page's ruling carries on across a tab cut from it.
            let mut rule_y = rect.top() + 27.0 * scale;
            while rule_y < rect.bottom() - 1.0 {
                fills.rect(
                    Rect::from_min_size(pos2(rect.left(), rule_y), vec2(rect.width(), 2.0)),
                    rule(),
                );
                rule_y += RULE_PITCH;
            }
            let (l, r, t, b) = (rect.left(), rect.right(), rect.top(), rect.bottom());
            fills.rect(Rect::from_min_max(pos2(l - 2.0, t), pos2(l, b)), line());
            fills.rect(Rect::from_min_max(pos2(l, t - 2.0), pos2(r, t)), line());
            fills.rect(Rect::from_min_max(pos2(l, b), pos2(r, b + 2.0)), line());
        }
    }
    fills.paint(painter);
    let number = painter.layout_no_wrap(
        format!("{number:02}"),
        FontId::monospace(label_size(scale)),
        number_color,
    );
    let text = painter.layout_no_wrap(
        label.to_owned(),
        FontId::proportional(14.0 * scale),
        label_color,
    );
    let x = rect.left() + 10.0;
    let number_width = number.size().x;
    painter.galley(
        pos2(x, rect.center().y - number.size().y / 2.0),
        number,
        number_color,
    );
    painter.galley(
        pos2(
            x + number_width + 8.0,
            rect.center().y - text.size().y / 2.0,
        ),
        text,
        label_color,
    );
}

/// The note tucked under the tabs: what a researcher would jot down about the colony right now.
/// Its lines are set in the window's small text, so they share its glyphs.
fn paint_conditions(
    painter: &egui::Painter,
    origin: Pos2,
    width: f32,
    floor: f32,
    lines: &[String],
    scale: f32,
) {
    let title = painter.layout_no_wrap(
        "CONDITIONS".into(),
        FontId::monospace(label_size(scale)),
        forest(),
    );
    let body: Vec<_> = lines
        .iter()
        .map(|line| {
            painter.layout(
                line.clone(),
                FontId::proportional(10.5 * scale),
                ink(),
                width - 20.0,
            )
        })
        .collect();
    let height =
        10.0 + title.size().y + 6.0 + body.iter().map(|galley| galley.size().y).sum::<f32>() + 9.0;
    let rect = Rect::from_min_size(origin, vec2(width, height));
    // A note that would hang off the foot of the cover is left in the pocket.
    if rect.bottom() > floor {
        return;
    }
    let mut fills = Fills::default();
    fills.rect(
        rect.translate(vec2(3.0, 4.0)),
        Color32::from_black_alpha(77),
    );
    stepped_box(&mut fills, rect, card_fill(), leather_dark());
    fills.rect(
        Rect::from_center_size(pos2(rect.center().x, rect.top()), vec2(36.0, 12.0)),
        tape(),
    );
    fills.paint(painter);
    let mut y = rect.top() + 10.0;
    let title_height = title.size().y;
    painter.galley(pos2(rect.left() + 10.0, y), title, forest());
    y += title_height + 6.0;
    for galley in body {
        let height = galley.size().y;
        painter.galley(pos2(rect.left() + 10.0, y), galley, ink());
        y += height;
    }
}

/// What the note on the cover says: where the colony is, and what someone is up to, written the
/// way a researcher jots it down.
pub(crate) fn conditions(save: &SaveFile, monitors: &[MonitorInfo]) -> Vec<String> {
    let creatures = &save.creatures;
    let mut lines = Vec::new();
    if save.settings.paused {
        lines.push("Everything is paused.".to_owned());
    } else if creatures.is_empty() {
        lines.push("Nobody here yet.".to_owned());
    } else if creatures
        .iter()
        .all(|creature| creature.state.action == ActionKind::Homebound)
    {
        lines.push("Everyone is at home.".to_owned());
    } else {
        // The display most of the colony is out on, by the number the window gives it.
        let busiest = monitors
            .iter()
            .enumerate()
            .max_by_key(|(_, monitor)| {
                creatures
                    .iter()
                    .filter(|creature| creature.state.surface.monitor_id == monitor.id)
                    .count()
            })
            .map(|(index, _)| index + 1);
        lines.push(match busiest {
            Some(display) if monitors.len() > 1 => format!("Out on Display {display}."),
            _ => "Out on the desktop.".to_owned(),
        });
    }
    if let Some(high) = creatures
        .iter()
        .find(|creature| creature.state.surface.window_key.is_some())
    {
        lines.push(format!("{} is up high again.", high.name));
    } else if let Some(asleep) = creatures
        .iter()
        .find(|creature| creature.state.action == ActionKind::Sleep)
    {
        lines.push(format!("{} is napping.", asleep.name));
    }
    lines.push(match creatures.len() {
        1 => "One specimen.".to_owned(),
        count => format!("{count} specimens."),
    });
    lines.push("Entirely on this computer.".to_owned());
    lines
}

/// The leather to the right of the page, where the notebook is bound.
pub(crate) fn paint_binding(painter: &egui::Painter, rect: Rect) {
    if rect.width() <= 0.0 {
        return;
    }
    let mut fills = Fills::default();
    let spine = Rect::from_min_max(
        pos2(rect.right() - (rect.width() * 0.4).max(4.0), rect.top()),
        rect.right_bottom(),
    );
    fills.rect(spine, leather_dark());
    let mut y = rect.top() + 80.0;
    while y < rect.bottom() - 40.0 {
        fills.rect(
            Rect::from_min_size(pos2(spine.left() - 3.0, y), vec2(spine.width() + 3.0, 2.0)),
            stitch().gamma_multiply(0.5),
        );
        y += 200.0;
    }
    fills.paint(painter);
}

/// The page itself: paper with its ruling and margin line, a stepped edge, and the shade of the
/// binding down its right side. `open_tab` is left out of the edge, since that tab is joined to
/// the page.
pub(crate) fn paint_page(
    painter: &egui::Painter,
    page: Rect,
    spread: Spread,
    footer_top: f32,
    open_tab: Rect,
) {
    let mut fills = Fills::default();
    fills.rect(
        page.translate(vec2(6.0, 6.0)),
        Color32::from_black_alpha(64),
    );
    fills.rect(page, paper());
    let mut y = page.top() + FIRST_RULE;
    while y < footer_top - 2.0 {
        fills.rect(
            Rect::from_min_max(pos2(page.left(), y), pos2(page.right(), y + 2.0)),
            rule(),
        );
        y += RULE_PITCH;
    }
    fills.rect(
        Rect::from_min_size(
            pos2(page.left() + spread.margin, page.top()),
            vec2(2.0, page.height()),
        ),
        margin_line(),
    );
    fills.shade(
        Rect::from_min_max(pos2(page.right() - 14.0, page.top()), page.right_bottom()),
        Color32::TRANSPARENT,
        Color32::from_rgba_unmultiplied(40, 25, 15, 46),
    );
    // The edge, but not where the open page's tab joins it.
    let (l, r, t, b) = (page.left(), page.right(), page.top(), page.bottom());
    fills.rect(Rect::from_min_max(pos2(l, t - 2.0), pos2(r, t)), line());
    fills.rect(Rect::from_min_max(pos2(r, t), pos2(r + 2.0, b)), line());
    let gap = open_tab.y_range();
    if open_tab.is_positive() {
        fills.rect(
            Rect::from_min_max(pos2(l - 2.0, t), pos2(l, gap.min.max(t))),
            line(),
        );
        fills.rect(
            Rect::from_min_max(pos2(l - 2.0, gap.max.min(b)), pos2(l, b)),
            line(),
        );
    } else {
        fills.rect(Rect::from_min_max(pos2(l - 2.0, t), pos2(l, b)), line());
    }
    fills.paint(painter);
}

/// The open page's tab, drawn over the page's edge it is joined to.
pub(crate) fn paint_open_tab(
    painter: &egui::Painter,
    rect: Rect,
    page: SettingsTab,
    unseen: bool,
    scale: f32,
) {
    if !rect.is_positive() {
        return;
    }
    let label = if page == SettingsTab::Colony && unseen {
        format!("{} •", page_label(page))
    } else {
        page_label(page).to_owned()
    };
    paint_tab(
        painter,
        rect,
        TabState::Open,
        page_number(page),
        &label,
        scale,
        false,
    );
}

/// The page's number, in its bottom corner above the footer.
pub(crate) fn paint_page_number(
    painter: &egui::Painter,
    page: Rect,
    footer_top: f32,
    number: usize,
    scale: f32,
) {
    painter.text(
        pos2(page.right() - 26.0, footer_top - 8.0),
        Align2::RIGHT_BOTTOM,
        format!("P. {number:02}"),
        FontId::monospace(label_size(scale)),
        muted(),
    );
}

/// How long a page takes to turn.
const TURN_SECONDS: f64 = 0.76;

/// A page being turned: from which page, when it began, and which way.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PageTurn {
    started: f64,
    forward: bool,
}

/// Turns the page when the window goes from one page to another, unless motion is reduced, and
/// draws the turning leaf over the page while it turns: the leaf lifted away toward the binding
/// going forward and back toward the cover going back, darkening as it goes, with a shadow on the
/// page it uncovers.
pub(crate) fn turn_page(
    ui: &Ui,
    turn: &mut Option<PageTurn>,
    shown: &mut Option<SettingsTab>,
    tab: SettingsTab,
    page: Rect,
    reduce_motion: bool,
) {
    let now = ui.input(|input| input.time);
    if *shown != Some(tab) {
        if let Some(previous) = *shown
            && !reduce_motion
        {
            *turn = Some(PageTurn {
                started: now,
                forward: page_number(tab) > page_number(previous),
            });
        }
        *shown = Some(tab);
    }
    let Some(current) = *turn else {
        return;
    };
    let progress = ((now - current.started) / TURN_SECONDS).clamp(0.0, 1.0) as f32;
    if progress >= 1.0 || reduce_motion {
        *turn = None;
        return;
    }
    ui.ctx().request_repaint();
    // Quick to lift, slow to settle.
    let eased = 1.0 - (1.0 - progress).powi(3);
    let width = page.width() * (1.0 - eased);
    let leaf = if current.forward {
        Rect::from_min_max(pos2(page.right() - width, page.top()), page.right_bottom())
    } else {
        Rect::from_min_max(page.left_top(), pos2(page.left() + width, page.bottom()))
    };
    let painter = ui.painter_at(page);
    let mut fills = Fills::default();
    // The shadow the leaf throws on the page it uncovers, beside its free edge.
    let shadow_width = 36.0 * (1.0 - eased);
    if current.forward {
        fills.shade(
            Rect::from_min_max(
                pos2(leaf.left() - shadow_width, page.top()),
                pos2(leaf.left(), page.bottom()),
            ),
            Color32::TRANSPARENT,
            Color32::from_black_alpha(70),
        );
    } else {
        fills.shade(
            Rect::from_min_max(
                pos2(leaf.right(), page.top()),
                pos2(leaf.right() + shadow_width, page.bottom()),
            ),
            Color32::from_black_alpha(70),
            Color32::TRANSPARENT,
        );
    }
    fills.rect(leaf, paper());
    let mut y = page.top() + FIRST_RULE;
    while y < page.bottom() {
        fills.rect(
            Rect::from_min_max(pos2(leaf.left(), y), pos2(leaf.right(), y + 2.0)),
            rule(),
        );
        y += RULE_PITCH;
    }
    fills.rect(leaf, Color32::from_black_alpha((eased * 60.0) as u8));
    fills.paint(&painter);
}
