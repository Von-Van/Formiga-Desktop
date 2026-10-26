//! What gets pasted and stamped into the notebook rather than written on its lines: a card held
//! down with tape, a rubber stamp, a strip of tape, and the little picture each kind of journal
//! moment is jotted with, cut from the creatures' own speech bubbles.
//!
//! Tilted things are tilted as flat shapes and turned text; nothing here animates.

use super::*;
use egui::emath::Rot2;
use egui::{Pos2, Rect, Vec2, pos2, vec2};
use formiga_core::BubbleIcon;

/// The picture a journal moment is jotted beside: the speech bubble its companions would show.
pub(crate) fn moment_icon(moment: &JournalMoment) -> BubbleIcon {
    match moment {
        JournalMoment::Arrival | JournalMoment::HouseVisit => BubbleIcon::Home,
        JournalMoment::Friendship(_) => BubbleIcon::Heart,
        JournalMoment::Discovery
        | JournalMoment::Object(_)
        | JournalMoment::Decoration(_)
        | JournalMoment::Unlocked(_)
        | JournalMoment::NewLook => BubbleIcon::Sparkle,
        JournalMoment::Preference(_) => BubbleIcon::Watching,
        JournalMoment::Ritual(
            RitualKind::GroupNap | RitualKind::QuietDayHuddle | RitualKind::LateNightSleepPile,
        ) => BubbleIcon::Sleepy,
        JournalMoment::Ritual(RitualKind::Catch | RitualKind::FloorRace) => BubbleIcon::Toy,
        JournalMoment::Ritual(_) => BubbleIcon::Music,
        JournalMoment::Visit(_) | JournalMoment::Revisit(..) => BubbleIcon::Hello,
        JournalMoment::Habit(_) => BubbleIcon::Dizzy,
        JournalMoment::Wonder(_) => BubbleIcon::Swoon,
        JournalMoment::Trip => BubbleIcon::Stay,
        JournalMoment::Battle(_) => BubbleIcon::Surprise,
    }
}

/// The picture for something a pair shared.
pub(crate) fn shared_icon(kind: formiga_core::SharedMomentKind) -> BubbleIcon {
    use formiga_core::SharedMomentKind;
    match kind {
        SharedMomentKind::Rest => BubbleIcon::Sleepy,
        SharedMomentKind::Play => BubbleIcon::Toy,
        SharedMomentKind::Gift => BubbleIcon::Sparkle,
        SharedMomentKind::Greeting => BubbleIcon::Hello,
        SharedMomentKind::Calm => BubbleIcon::Music,
        SharedMomentKind::Squabble => BubbleIcon::Grumble,
    }
}

/// A rectangle `size` big with its centre at `centre`, turned by `angle` about `pivot`, added to
/// `mesh` in one colour.
fn turned_rect(
    mesh: &mut egui::Mesh,
    pivot: Pos2,
    angle: f32,
    centre: Pos2,
    size: Vec2,
    color: Color32,
) {
    if color.a() == 0 || size.x <= 0.0 || size.y <= 0.0 {
        return;
    }
    let turn = Rot2::from_angle(angle);
    let half = size / 2.0;
    let base = mesh.vertices.len() as u32;
    for corner in [
        vec2(-half.x, -half.y),
        vec2(half.x, -half.y),
        vec2(half.x, half.y),
        vec2(-half.x, half.y),
    ] {
        let at = pivot + turn * (centre - pivot + corner);
        mesh.colored_vertex(at, color);
    }
    mesh.add_triangle(base, base + 1, base + 2);
    mesh.add_triangle(base, base + 2, base + 3);
}

/// A strip of tape `size` big, centred on `centre` and turned by `angle`.
pub(crate) fn tape_strip(painter: &egui::Painter, centre: Pos2, size: Vec2, angle: f32) {
    let mut mesh = egui::Mesh::default();
    turned_rect(&mut mesh, centre, angle, centre, size, tape());
    painter.add(egui::Shape::mesh(mesh));
}

/// The outline of `rect` turned by `angle` about its centre, `width` thick, corners stepped.
fn turned_outline(mesh: &mut egui::Mesh, rect: Rect, angle: f32, width: f32, color: Color32) {
    let c = rect.center();
    let (w, h) = (rect.width(), rect.height());
    for (centre, size) in [
        (pos2(c.x, rect.top() - width / 2.0), vec2(w, width)),
        (pos2(c.x, rect.bottom() + width / 2.0), vec2(w, width)),
        (pos2(rect.left() - width / 2.0, c.y), vec2(width, h)),
        (pos2(rect.right() + width / 2.0, c.y), vec2(width, h)),
    ] {
        turned_rect(mesh, c, angle, centre, size, color);
    }
}

/// A rubber stamp pressed at `centre`, turned by `angle`: each line in the pencil hand's bold at
/// its size, in `color`, inside a double edge, the inner one faint, the way ink takes on paper.
/// `fill` is what shows through it, if anything. Returns the space it covers, unturned.
pub(crate) fn stamp(
    painter: &egui::Painter,
    centre: Pos2,
    lines: &[(&str, f32)],
    color: Color32,
    fill: Option<Color32>,
    angle: f32,
) -> Rect {
    let galleys: Vec<_> = lines
        .iter()
        .map(|(text, size)| painter.layout_job(journal::kicker_job(text, *size, color)))
        .collect();
    let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let height: f32 = galleys.iter().map(|g| g.size().y).sum();
    let rect = Rect::from_center_size(centre, vec2(width + 24.0, height + 8.0));
    let mut mesh = egui::Mesh::default();
    if let Some(fill) = fill {
        turned_rect(&mut mesh, centre, angle, centre, rect.size(), fill);
    }
    turned_outline(&mut mesh, rect, angle, 2.0, color);
    turned_outline(
        &mut mesh,
        rect.shrink(2.0),
        angle,
        2.0,
        color.gamma_multiply(0.35),
    );
    painter.add(egui::Shape::mesh(mesh));
    let turn = Rot2::from_angle(angle);
    let mut y = rect.top() + 4.0;
    for galley in galleys {
        let size = galley.size();
        let top_left = pos2(centre.x - size.x / 2.0, y);
        let at = centre + turn * (top_left - centre);
        painter.add(egui::epaint::TextShape::new(at, galley, color).with_angle(angle));
        y += size.y;
    }
    rect
}

/// How far a pasted card's tape hangs over its corner, and how big it is.
const TAPE: Vec2 = vec2(46.0, 14.0);

/// A card pasted onto the page: the card's face with a stepped edge and a soft hard shadow under
/// it, and, with `taped`, a strip of tape across its upper left corner. Whatever is drawn inside
/// is set as on any card, not on the page's lines. At least `min_height` tall, and as wide as
/// `width`.
pub(crate) fn pasted<R>(
    ui: &mut Ui,
    width: f32,
    min_height: f32,
    taped: bool,
    contents: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    let grid = journal::RuleGrid::of(ui.ctx());
    journal::RuleGrid::forget(ui.ctx());
    let shadow = ui.painter().add(egui::Shape::Noop);
    let shown = egui::Frame::new()
        .fill(card_fill())
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_width((width - 32.0).max(0.0));
            ui.set_min_height((min_height - 28.0).max(0.0));
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), contents)
                .inner
        });
    if let Some(grid) = grid {
        grid.keep(ui.ctx());
    }
    let rect = shown.response.rect;
    ui.painter().set(
        shadow,
        egui::Shape::rect_filled(
            rect.translate(vec2(4.0, 5.0)),
            0.0,
            Color32::from_black_alpha(36),
        ),
    );
    let mut fills = journal::Fills::default();
    journal::stepped_outline(&mut fills, rect, 2.0, line());
    fills.paint(ui.painter());
    if taped {
        tape_strip(
            ui.painter(),
            rect.left_top() + vec2(13.0, 3.0),
            TAPE,
            -28f32.to_radians(),
        );
    }
    shown
}

/// A portrait in the notebook's frame, `size` across: the forest mat with a gold stepped edge, or,
/// for a companion standing on the ground, no frame at all.
pub(crate) fn face(
    painter: &egui::Painter,
    rect: Rect,
    texture: &egui::TextureHandle,
    framed: bool,
) {
    let image = if framed {
        let mut fills = journal::Fills::default();
        journal::stepped_box(&mut fills, rect, forest(), gold());
        fills.paint(painter);
        rect.shrink((rect.width() * 0.08).round().max(1.0))
    } else {
        rect
    };
    painter.image(
        texture.id(),
        image,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}
