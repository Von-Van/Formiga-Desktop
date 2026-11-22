//! A companion's own page in the notebook, as it opens: everyone's face along the top with the
//! chosen one taped, then the companion itself first — its portrait, its name in the hand, who it
//! is in one line, its traits, what it is doing — and its figures stamped in the margin, where a
//! field note's figures go. What the page goes on to say about it is written under that.

use super::shell::Shell;
use super::*;

/// What each face in the strip answers to, so the arrow keys know when the strip has the
/// keyboard.
pub(crate) fn face_id(id: CreatureId) -> egui::Id {
    egui::Id::new(("companion-face", id))
}

/// The companion before (`forward` false) or after the one chosen, in colony order, if there is
/// one that way.
pub(crate) fn neighbour(
    creatures: &[Creature],
    chosen: Option<CreatureId>,
    forward: bool,
) -> Option<CreatureId> {
    let index = creatures
        .iter()
        .position(|creature| Some(creature.id) == chosen)
        .unwrap_or(0);
    let next = if forward {
        index.checked_add(1)
    } else {
        index.checked_sub(1)
    }?;
    creatures.get(next).map(|creature| creature.id)
}

/// Turning between companions from the keyboard: ⌘[ and ⌘] (Ctrl elsewhere), and the left and
/// right arrows while nothing but a face in the strip has the keyboard. Taken before the
/// notebook's own page turns, so ⌘[ on the first companion and ⌘] on the last still turn the
/// page. Never while a text field or a pop-up is open.
pub(crate) fn turn_shortcut(
    ctx: &egui::Context,
    creatures: &[Creature],
    chosen: Option<CreatureId>,
) -> Option<CreatureId> {
    if ctx.text_edit_focused() || ctx.any_popup_open() {
        return None;
    }
    let focus_free = ctx
        .memory(|memory| memory.focused())
        .is_none_or(|focused| creatures.iter().any(|c| face_id(c.id) == focused));
    let back = neighbour(creatures, chosen, false);
    let forward = neighbour(creatures, chosen, true);
    ctx.input_mut(|input| {
        if back.is_some()
            && (input.consume_key(egui::Modifiers::COMMAND, egui::Key::OpenBracket)
                || (focus_free && input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)))
        {
            back
        } else if forward.is_some()
            && (input.consume_key(egui::Modifiers::COMMAND, egui::Key::CloseBracket)
                || (focus_free && input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)))
        {
            forward
        } else {
            None
        }
    })
}

/// Everyone's face in a row with their name under it, the chosen one with a strip of tape across
/// its top. Each face is a button: clicked, or Space or Enter while it has the keyboard, it turns
/// to that companion, which is returned.
pub(crate) fn face_strip(
    ui: &mut Ui,
    shell: &mut Shell,
    creatures: &[Creature],
    chosen: Option<CreatureId>,
) -> Option<CreatureId> {
    let scale = journal::text_scale(ui);
    let side = (40.0 * scale.min(1.2)).round();
    let name_font = formiga_pencil::hand(14.0 * scale);
    let mut turned = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        for creature in creatures {
            let is_chosen = Some(creature.id) == chosen;
            let colour = if is_chosen { ink() } else { muted() };
            let name =
                ui.painter()
                    .layout_no_wrap(creature.name.clone(), name_font.clone(), colour);
            let width = side.max(name.size().x) + 8.0;
            let (_, rect) = ui.allocate_space(egui::vec2(width, side + 6.0 + name.size().y));
            let response = ui.interact(rect, face_id(creature.id), egui::Sense::click());
            let face = egui::Rect::from_min_size(
                rect.center_top() - egui::vec2(side / 2.0, 0.0),
                egui::vec2(side, side),
            );
            let texture = shell.portrait_texture(ui.ctx(), creature);
            paper::face(ui.painter(), face, &texture, true);
            ui.painter().galley(
                egui::pos2(rect.center().x - name.size().x / 2.0, face.bottom() + 6.0),
                name,
                colour,
            );
            if is_chosen {
                paper::tape_strip(
                    ui.painter(),
                    face.center_top() + egui::vec2(0.0, 1.0),
                    egui::vec2(side * 0.8, 10.0),
                    -6f32.to_radians(),
                );
            }
            if response.has_focus() {
                ui.painter().rect_stroke(
                    rect.expand(2.0),
                    0.0,
                    egui::Stroke::new(2.0, forest()),
                    egui::StrokeKind::Outside,
                );
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    true,
                    is_chosen,
                    &creature.name,
                )
            });
            if response.clicked() && !is_chosen {
                turned = Some(creature.id);
            }
        }
    });
    turned
}

/// What the page says a companion is doing just now: what Right now says, and who with when it
/// is greeting, following or playing with a companion close by — "Up on a window, playing with
/// Tofu." Only a companion within reach on the same kind of footing is named; otherwise nobody
/// is.
pub(crate) fn right_now_line(creature: &Creature, creatures: &[Creature]) -> String {
    let with = |verb: &str| {
        creatures
            .iter()
            .filter(|other| other.id != creature.id)
            .filter(|other| other.state.surface.window_key == creature.state.surface.window_key)
            .map(|other| {
                let (dx, dy) = (
                    other.state.position.x - creature.state.position.x,
                    other.state.position.y - creature.state.position.y,
                );
                (other, dx * dx + dy * dy)
            })
            .filter(|(_, distance)| *distance <= 240.0 * 240.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(other, _)| format!("{verb} {}", other.name))
    };
    let named = match creature.state.action {
        ActionKind::SocialPlay => with("playing with"),
        ActionKind::Follow => with("following"),
        ActionKind::Greet => with("saying hello to"),
        _ => None,
    };
    let line = match named {
        Some(named) if creature.state.surface.window_key.is_some() => {
            format!("Up on a window, {named}")
        }
        Some(named) => today::capitalised(&named),
        None => today::doing_now(creature),
    };
    format!("{line}.")
}

/// The figures a companion's page stamps in its margin, one to each part: "109 days", "42 pets",
/// "7 finds", "18 climbs". A companion here less than a day is on its "first day".
pub(crate) fn tally_figures(days: i64, pets: u32, finds: u32, climbs: u32) -> [String; 4] {
    let count = |n: u64, one: &str, many: &str| {
        if n == 1 {
            format!("1 {one}")
        } else {
            format!("{n} {many}")
        }
    };
    [
        if days <= 0 {
            "First day".to_owned()
        } else {
            count(days as u64, "day", "days")
        },
        count(u64::from(pets), "pet", "pets"),
        count(u64::from(finds), "find", "finds"),
        count(u64::from(climbs), "climb", "climbs"),
    ]
}

/// The companion's figures, stamped at the right of the page under its portrait: on one line
/// where it fits, else two.
pub(crate) fn tally_stamp(ui: &mut Ui, creature: &Creature, now: OffsetDateTime) {
    let scale = journal::text_scale(ui);
    let figures = tally_figures(
        (now - creature.born_at_utc).whole_days(),
        creature.memory.times_petted,
        creature.memory.discoveries_found,
        creature.memory.window_climbs,
    );
    let size = journal::kicker_size(scale);
    let one_line = figures.join(" · ");
    let measured = |text: &str| {
        ui.painter()
            .layout_job(journal::kicker_job(text, size, stamp()))
            .size()
    };
    let room = ui.available_width();
    let lines: Vec<String> = if measured(&one_line).x + 40.0 <= room {
        vec![one_line]
    } else {
        vec![figures[..2].join(" · "), figures[2..].join(" · ")]
    };
    let height = lines.iter().map(|line| measured(line).y).sum::<f32>() + 18.0;
    let width = lines
        .iter()
        .map(|line| measured(line).x)
        .fold(0.0, f32::max)
        + 28.0;
    let (row, response) = ui.allocate_exact_size(egui::vec2(room, height), egui::Sense::hover());
    let centre = egui::pos2(row.right() - width / 2.0 - 8.0, row.center().y);
    let lines: Vec<(&str, f32)> = lines.iter().map(|line| (line.as_str(), size)).collect();
    paper::stamp(
        ui.painter(),
        centre,
        &lines,
        stamp(),
        None,
        (-3f32).to_radians(),
    );
    response.on_hover_text("Days together, pets received, treasures found and windows climbed.");
    journal::to_next_rule(ui);
}

/// Whether a companion came to live here on the reader's today.
pub(crate) fn arrived_today(
    creature: &Creature,
    now: OffsetDateTime,
    offset: time::UtcOffset,
) -> bool {
    creature.born_at_utc.to_offset(offset).date() >= now.to_offset(offset).date()
}

/// The companion first: its portrait, `portrait` across, beside its name in the hand, its
/// temperament once, its traits run over with a highlighter, and what it is doing. `aside` adds
/// whatever else belongs beside the portrait, under those.
pub(crate) fn header(
    ui: &mut Ui,
    shell: &mut Shell,
    creature: &Creature,
    creatures: &[Creature],
    now: OffsetDateTime,
    aside: impl FnOnce(&mut Ui),
) {
    let scale = journal::text_scale(ui);
    // 144 points where there is room beside it for the words, and never under 96.
    let portrait = if ui.available_width() >= 480.0 {
        144.0
    } else {
        96.0
    };
    journal::ruled_row(ui, |ui| {
        shell.portrait(ui, creature, portrait);
        ui.vertical(|ui| {
            journal::write(
                ui,
                RichText::new(&creature.name)
                    .font(formiga_pencil::hand_bold(40.0 * scale.min(1.2)))
                    .color(ink()),
            );
            journal::write(
                ui,
                RichText::new(creature.temperament_phrase()).color(forest()),
            );
            journal::write_marked(
                ui,
                creature
                    .traits()
                    .iter()
                    .map(|t| t.label())
                    .collect::<Vec<_>>()
                    .join(" · "),
                None,
                egui::Sense::hover(),
                Some(mint()),
            );
            journal::write(
                ui,
                RichText::new(right_now_line(creature, creatures)).color(muted()),
            );
            if arrived_today(creature, now, local_offset()) {
                journal::write(ui, RichText::new("Arrived today").color(muted()));
            }
            if let Some(parent_id) = creature.role.parent_id() {
                journal::write(
                    ui,
                    RichText::new(format!(
                        "A mini, cared for by {}",
                        creatures
                            .iter()
                            .find(|c| c.id == parent_id)
                            .map_or("a colony adult", |c| c.name.as_str())
                    ))
                    .color(muted()),
                );
            }
            aside(ui);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_figures_are_counted_the_way_they_are_said() {
        assert_eq!(
            tally_figures(109, 42, 7, 18).join(" · "),
            "109 days · 42 pets · 7 finds · 18 climbs"
        );
        assert_eq!(
            tally_figures(1, 1, 1, 1).join(" · "),
            "1 day · 1 pet · 1 find · 1 climb"
        );
        assert_eq!(tally_figures(0, 0, 0, 0)[0], "First day");
    }

    #[test]
    fn turning_stops_at_either_end() {
        let creature = |seed: u8| {
            World::preview_adult(
                [seed; 32],
                OffsetDateTime::UNIX_EPOCH,
                &DesktopSnapshot::default(),
            )
        };
        let mut creatures = vec![creature(1), creature(2), creature(3)];
        for (index, creature) in creatures.iter_mut().enumerate() {
            creature.id = index as CreatureId + 1;
        }
        assert_eq!(neighbour(&creatures, Some(1), false), None);
        assert_eq!(neighbour(&creatures, Some(1), true), Some(2));
        assert_eq!(neighbour(&creatures, Some(3), true), None);
        assert_eq!(neighbour(&creatures, Some(2), false), Some(1));
        assert_eq!(neighbour(&creatures, None, true), Some(2));
    }
}
