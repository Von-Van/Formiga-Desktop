//! Writing on a notebook's ruled lines, the same way in every Formiga notebook: each line of
//! writing rests on a rule however it wraps, a row of things rests on one rule together, a control
//! sits with its words on the line, and whatever follows a picture or a card goes on the first
//! rule clear of it.
//!
//! A page says where its rules are with a [`RuleGrid`], kept for the frame while the page is
//! written ([`RuleGrid::keep`]) and let go once it is ([`RuleGrid::forget`]). The page draws its
//! own rules, each with its top at a line of the grid; what is written finds them through the
//! grid. Off a ruled page, or on a card over one, everything here is an ordinary label.

use egui::{Color32, Rect, Ui, pos2, vec2};
use std::sync::Arc;

/// Where a page's ruled lines are this frame: the top of one every `pitch` from `top`, wherever
/// the page has been scrolled to. A line of writing laid out from one of them rests on the next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuleGrid {
    pub top: f32,
    pub pitch: f32,
}

impl RuleGrid {
    fn id() -> egui::Id {
        egui::Id::new("formiga-pencil-rule-grid")
    }

    /// Keeps this grid for what is written on the page from here on.
    pub fn keep(self, ctx: &egui::Context) {
        ctx.data_mut(|data| data.insert_temp(Self::id(), self));
    }

    /// Lets the grid go once the page has been written, so nothing off it lines up with it.
    pub fn forget(ctx: &egui::Context) {
        ctx.data_mut(|data| data.remove::<Self>(Self::id()));
    }

    pub fn of(ctx: &egui::Context) -> Option<Self> {
        ctx.data(|data| data.get_temp(Self::id()))
    }

    /// The first ruled line at or below `y`.
    pub fn rule_at_or_below(self, y: f32) -> f32 {
        self.top + ((y - self.top) / self.pitch - 1e-3).ceil() * self.pitch
    }

    /// How many rules a line of writing `size` points tall takes: one for writing, two for a
    /// page's name. Read from the size rather than the font's own line, which for the hand
    /// leaves room for its tallest loops and would give every line of writing two rules.
    pub fn lines_for(self, size: f32) -> f32 {
        (size * 1.15 / self.pitch).ceil().max(1.0)
    }
}

/// How far above the top of its rule a line of writing's baseline is, in points: the hand's
/// round letters dip a little below their baseline, so they rest on the rule rather than in it.
pub const LIFT: f32 = 1.0;

/// Where a galley's first line rests, down from its top.
pub fn baseline_of(galley: &egui::Galley) -> Option<f32> {
    galley
        .rows
        .first()
        .and_then(|row| row.glyphs.first())
        .map(|glyph| row_y(galley) + glyph.pos.y)
}

fn row_y(galley: &egui::Galley) -> f32 {
    galley.rows.first().map_or(0.0, |row| row.pos.y)
}

/// Where to put `galley`'s top so its first line rests on the rule whose top is at `rule`.
pub fn on_rule(galley: &egui::Galley, rule: f32) -> f32 {
    rule - LIFT - baseline_of(galley).unwrap_or(galley.size().y)
}

/// Moves what is written next down to the page's next ruled line: the first at or below the
/// bottom of whatever came before, so writing follows writing line after line, and follows a card
/// or a row of buttons on the first line clear of it. Nothing happens off the page or in a row.
pub fn to_next_rule(ui: &mut Ui) {
    let Some(grid) = RuleGrid::of(ui.ctx()) else {
        return;
    };
    if ui.layout().main_dir().is_horizontal() {
        return;
    }
    let y = ui.cursor().top();
    let rule = grid.rule_at_or_below(y - ui.spacing().item_spacing.y);
    ui.add_space(rule - y);
}

/// Writes `text` on the page's lines: on the next ruled line, and each line of it, however it
/// wraps, resting on a line of its own; text too tall for one line, like a page's name, takes two.
pub fn write(ui: &mut Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    write_with(ui, text, None, egui::Sense::hover(), None)
}

/// `write`, wrapped at `wrap` points rather than the room there is, answering to `sense` (a link
/// written on the page, say), and run over with a highlighter in `marker` if given: a band behind
/// the letters of each line, from just above their capitals down to the rule they rest on. In a
/// row (`ruled_row`), each thing written rests on the row's line.
pub fn write_with(
    ui: &mut Ui,
    text: impl Into<egui::WidgetText>,
    wrap: Option<f32>,
    sense: egui::Sense,
    marker: Option<Color32>,
) -> egui::Response {
    let text = text.into();
    let Some(grid) = RuleGrid::of(ui.ctx()) else {
        return ui.add(egui::Label::new(text).wrap().sense(sense));
    };
    to_next_rule(ui);
    let width = wrap.unwrap_or_else(|| ui.available_width()).max(24.0);
    let mut job = Arc::unwrap_or_clone(text.into_layout_job(
        ui.style(),
        egui::FontSelection::Default,
        egui::Align::Min,
    ));
    job.wrap.max_width = width;
    let size = job
        .sections
        .iter()
        .map(|section| section.format.font_id.size)
        .fold(0.0_f32, f32::max);
    let line = grid.lines_for(size) * grid.pitch;
    for section in &mut job.sections {
        section.format.line_height = Some(line);
    }
    let galley = ui.painter().layout_job(job);
    let rows = galley.rows.len().max(1) as f32;
    let baseline = baseline_of(&galley).unwrap_or(line - LIFT);
    let (rect, response) = ui.allocate_exact_size(vec2(galley.size().x, rows * line), sense);
    if ui.is_rect_visible(rect) {
        let at = pos2(rect.left(), rect.top() + line - LIFT - baseline);
        if let Some(marker) = marker {
            let cap = galley
                .rows
                .first()
                .and_then(|row| row.glyphs.first())
                .map_or(line * 0.6, |glyph| glyph.font_ascent);
            for row in &galley.rows {
                let rule = at.y + row.pos.y - row_y(&galley) + baseline + LIFT;
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        pos2(at.x + row.rect().left() - 2.0, rule - LIFT - cap - 2.0),
                        pos2(at.x + row.rect().right() + 2.0, rule),
                    ),
                    0.0,
                    marker,
                );
            }
        }
        ui.painter().galley(at, galley, ui.visuals().text_color());
    }
    if sense.senses_click() && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// One control on the page's next ruled line, a checkbox or a row of buttons, set so that the
/// words on it rest on the line as written words do. A control taller than the line stands up
/// above it, and what follows goes on the next line clear of it.
pub fn on_the_line<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let Some(grid) = RuleGrid::of(ui.ctx()) else {
        return ui.scope(add);
    };
    to_next_rule(ui);
    let band = ui.cursor().top();
    let height = ui.spacing().interact_size.y;
    let size = egui::TextStyle::Body.resolve(ui.style()).size;
    // The middle of a control's words is about a third of their size above where they rest.
    let middle = band + grid.pitch - LIFT - size * 0.35;
    let top = (middle - height / 2.0).max(band - grid.pitch / 2.0);
    let rect = Rect::from_min_size(
        pos2(ui.cursor().left(), top),
        vec2(ui.available_width(), height),
    );
    let shown = ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        add,
    );
    // The control's clickable edge may hang a little past the line under its words; what follows
    // still goes on the very next line, unless the control reaches half a line past it.
    let lines = ((shown.response.rect.bottom() - band) / grid.pitch - 0.5)
        .ceil()
        .max(1.0);
    let next = band + lines * grid.pitch + ui.spacing().item_spacing.y;
    ui.add_space(next - ui.cursor().top());
    shown
}

/// A row on the page's next ruled line: whatever is written in it with `write` rests on that line.
pub fn ruled_row<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    to_next_rule(ui);
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), add)
}

/// `ruled_row`, wrapping onto the lines below when it runs out of room: each thing written in it
/// rests on its own row's line.
pub fn ruled_row_wrapping<R>(
    ui: &mut Ui,
    add: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    to_next_rule(ui);
    ui.with_layout(
        egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
        |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            add(ui)
        },
    )
}

/// A band `lines` rules tall from the next ruled line, for something drawn by hand to stand on its
/// last rule; `fallback` points tall off a ruled page.
pub fn band(ui: &mut Ui, lines: f32, fallback: f32) -> Rect {
    let height = match RuleGrid::of(ui.ctx()) {
        Some(grid) => {
            to_next_rule(ui);
            lines * grid.pitch
        }
        None => fallback,
    };
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), egui::Sense::hover());
    rect
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_next_rule_is_never_above_where_it_is_asked_from() {
        let grid = RuleGrid {
            top: 10.0,
            pitch: 18.0,
        };
        assert_eq!(grid.rule_at_or_below(10.0), 10.0);
        assert_eq!(grid.rule_at_or_below(11.0), 28.0);
        assert_eq!(grid.rule_at_or_below(28.0), 28.0);
        assert_eq!(grid.rule_at_or_below(-30.0), -26.0);
    }

    /// Where each line of `text`, written on a page ruled every `pitch` from the top, rests, and
    /// where the rules are.
    fn written(text: &str, size: f32, pitch: f32) -> Vec<f32> {
        let ctx = egui::Context::default();
        crate::install(&ctx, crate::Everywhere::Yes);
        // Fonts come into use from the next pass.
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .textures_delta
            .clear();
        let mut rests = Vec::new();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(160.0, 600.0))),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            RuleGrid {
                top: ui.max_rect().top(),
                pitch,
            }
            .keep(ui.ctx());
            write(ui, egui::RichText::new(text).size(size));
            write(ui, egui::RichText::new("and after it").size(size));
            RuleGrid::forget(ui.ctx());
        });
        fn walk(shape: &egui::Shape, out: &mut Vec<f32>) {
            match shape {
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, out)),
                egui::Shape::Text(text) if text.opacity_factor == 0.0 => {
                    for row in &text.galley.rows {
                        if let Some(glyph) = row.glyphs.first() {
                            out.push(text.pos.y + row.pos.y + glyph.pos.y);
                        }
                    }
                }
                _ => {}
            }
        }
        for clipped in &output.shapes {
            walk(&clipped.shape, &mut rests);
        }
        output.textures_delta.clear();
        rests
    }

    #[test]
    fn every_line_of_writing_rests_just_above_a_rule_however_it_wraps() {
        for (size, pitch) in [(15.0, 24.0), (13.0, 21.0), (15.0, 18.0)] {
            let rests = written(
                "Two down, one to go! The visitors are waiting by the gate.",
                size,
                pitch,
            );
            assert!(rests.len() >= 3, "{rests:?}");
            for rest in &rests {
                let above = (rest + LIFT) / pitch;
                assert!(
                    (above - above.round()).abs() < 0.05,
                    "{size} pt on {pitch}: {rest} is not {LIFT} above a rule"
                );
            }
            // One rule to each line of writing, not two.
            for pair in rests.windows(2) {
                assert!((pair[1] - pair[0] - pitch).abs() < 0.5, "{rests:?}");
            }
        }
    }
}
