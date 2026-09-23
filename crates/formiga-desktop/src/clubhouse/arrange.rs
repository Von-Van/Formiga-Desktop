//! The Home page's village, arranged where it stands. The preview draws the corner exactly as the
//! desktop does — both trees, every house, the keepsakes in the branches, the belongings in the
//! yards and whatever is on the ground — and in Arrange mode it can be taken hold of: a cottage
//! dragged along the row to stand somewhere else, a garden, a spot or an ornament dragged along
//! the ground, and a house picked out to choose its type and what it wears. Nothing is written
//! until something is let go, and every change goes through the same outcomes the rest of the
//! page uses, so each one can be taken back.

use super::*;
use formiga_core::{DecorationSlot, GroundItem, OrnamentKind, ShelterDecorationKind};

/// Something picked out in the preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Picked {
    /// A house, by the companion who keeps it.
    House(CreatureId),
    /// Something on the village ground.
    Ground(GroundItem),
}

/// Something being dragged across the preview, and where the pointer has it now, in the desktop's
/// own points.
#[derive(Clone, Copy, Debug)]
struct Drag {
    picked: Picked,
    x: f32,
}

/// The Home page's hold on the preview. A view state, never saved.
#[derive(Clone, Debug, Default)]
pub(crate) struct ArrangeState {
    pub(crate) arranging: bool,
    pub(crate) picked: Option<Picked>,
    drag: Option<Drag>,
    /// Where everything that can be picked out was drawn last frame, for tests to take hold of.
    #[cfg(test)]
    pub(crate) shown: Vec<(Picked, egui::Rect)>,
}

/// What the village preview draws each lot from, and in what order it lays them down: the houses
/// and the two trees, then the keepsakes hanging in their branches, then the belongings standing
/// on the ground in front of the trunks.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum VillageLayer {
    Dwelling,
    Trinket,
    Belonging,
}

/// One thing the preview draws: where it stands in the desktop's own points, what it samples,
/// from which sheet, and what it is, if it is something that can be picked out.
struct Lot {
    rect: egui::Rect,
    uv: egui::Rect,
    layer: VillageLayer,
    picks: Option<Picked>,
    /// A cottage's place in the row, counting the colony house as zero, for dragging.
    slot: Option<usize>,
}

/// How far one press of an arrow key moves something along the ground.
const NUDGE: f32 = 0.04;

/// Where a cell sits in the atlas's top row — every house by day with nobody home, and the tree —
/// the only row the Home page holds, as texture coordinates.
fn village_uv(cell: formiga_art::VillageCell) -> egui::Rect {
    let (x, y) = ShelterRenderer::village_cell(cell);
    let (width, height) = (formiga_art::VILLAGE_ATLAS_WIDTH as f32, SHELTER_SIZE as f32);
    let size = SHELTER_SIZE as f32;
    egui::Rect::from_min_max(
        egui::pos2(x as f32 / width, y as f32 / height),
        egui::pos2((x as f32 + size) / width, (y as f32 + size) / height),
    )
}

/// Where a cell of the object sheet sits, as texture coordinates, turned round if `mirrored`.
pub(super) fn object_uv(cell: u32, mirrored: bool) -> egui::Rect {
    let (mut left, top, mut right, bottom) = ColonyObjectRenderer::cell_uv(cell);
    if mirrored {
        std::mem::swap(&mut left, &mut right);
    }
    egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, bottom))
}

/// The cell something on the ground is drawn from, a garden at whatever stage it has grown to.
pub(super) fn ground_cell(save: &SaveFile, item: GroundItem) -> u32 {
    let stage = match item {
        GroundItem::Garden(kind) => save.home.garden(kind).map_or(GardenStage::Grown, |patch| {
            patch.stage(save.maximum_seen_utc)
        }),
        _ => GardenStage::Grown,
    };
    ColonyObjectRenderer::ground_cell(item, stage)
}

/// Where something on the ground stands, as a fraction of the ground a companion may stand on.
fn along_of(save: &SaveFile, item: GroundItem) -> Option<f32> {
    match item {
        GroundItem::Garden(kind) => save.home.garden(kind).map(|patch| patch.along),
        GroundItem::Hangout(kind) => save.home.hangout(kind).map(|spot| spot.along),
        GroundItem::Ornament(kind) => save.home.ornament(kind).map(|spot| spot.along),
    }
}

/// The outcome that puts `item` down at `along`, moves it there, or with `None` takes it up.
fn place(outcome: &mut SettingsOutcome, item: GroundItem, along: Option<f32>) {
    match item {
        GroundItem::Garden(kind) => outcome.set_garden = Some((kind, along)),
        GroundItem::Hangout(kind) => outcome.set_hangout = Some((kind, along)),
        GroundItem::Ornament(kind) => outcome.set_ornament = Some((kind, along)),
    }
}

/// Somewhere with room for one more thing on the ground: the middle of the widest stretch
/// between what is already down and the two ends.
fn free_along(save: &SaveFile) -> f32 {
    let mut taken: Vec<f32> = save
        .home
        .gardens
        .iter()
        .map(|patch| patch.along)
        .chain(save.home.hangouts.iter().map(|spot| spot.along))
        .chain(save.home.ornaments.iter().map(|spot| spot.along))
        .collect();
    taken.push(-0.05);
    taken.push(1.05);
    taken.sort_by(f32::total_cmp);
    taken
        .windows(2)
        .max_by(|a, b| (a[1] - a[0]).total_cmp(&(b[1] - b[0])))
        // To a thousandth, so the middle of an empty ground is the middle, not a hair short of it.
        .map_or(0.5, |gap| {
            (((gap[0] + gap[1]) / 2.0).clamp(0.0, 1.0) * 1000.0).round() / 1000.0
        })
}

/// What the preview says a picked-out thing is.
fn picked_label(save: &SaveFile, picked: Picked) -> String {
    match picked {
        Picked::House(keeper) => {
            let owners = formiga_core::house_owners(&save.creatures, &save.home.cottage_order);
            let slot = owners
                .as_slice()
                .iter()
                .position(|id| *id == keeper)
                .unwrap_or(0);
            let name = save
                .creatures
                .iter()
                .find(|creature| creature.id == keeper)
                .map_or("a companion", |creature| creature.name.as_str());
            let style = save.home.house_style_list(&save.creatures)[slot];
            if slot == 0 {
                format!("The colony house · {name}'s · {}", style.label())
            } else {
                format!("Cottage {slot} · {name}'s · {}", style.label())
            }
        }
        Picked::Ground(item) => item.label().to_owned(),
    }
}

impl Clubhouse {
    /// The village as it stands, drawn from the very functions the desktop places it with, and —
    /// in Arrange mode — something to take hold of. Looking at it never calls the colony home or
    /// changes what any creature is doing.
    pub(super) fn village_preview(
        &mut self,
        ui: &mut Ui,
        save: &SaveFile,
        monitors: &[MonitorInfo],
        outcome: &mut SettingsOutcome,
    ) {
        let trinkets = self.trinket_atlas(ui, save);
        let (Some(home), Some((_, objects_texture))) = (&self.home_texture, &self.object_texture)
        else {
            return;
        };
        let (village, objects_texture) = (home.texture.clone(), objects_texture.clone());
        let cottages = formiga_core::colony_cottages(&save.creatures);
        let scale = save.settings.display_scale;
        let owners = formiga_core::house_owners(&save.creatures, &save.home.cottage_order);
        let owners = owners.as_slice();
        let mut lots: Vec<Lot> = Vec::new();
        let mut home_monitor = None;
        let hung = formiga_art::hung_trinkets(&save.home, &save.companion.scrapbook);
        for end in formiga_core::TreeEnd::BOTH {
            let Some((monitor_id, point)) = formiga_core::home_tree_position(
                &save.home,
                end,
                &cottages,
                monitors,
                &save.settings.habitat,
                scale,
            ) else {
                continue;
            };
            home_monitor.get_or_insert(monitor_id);
            let size = SHELTER_SIZE as f32;
            let corner = egui::pos2(point.x - size / 2.0, point.y - size);
            // The inward tree is the same atlas cell sampled the other way round, so the two
            // bookends are not the same drawing twice.
            let tree = village_uv(formiga_art::VillageCell::Tree);
            let (u_left, u_right) = if end == formiga_core::TreeEnd::Inward {
                (tree.max.x, tree.min.x)
            } else {
                (tree.min.x, tree.max.x)
            };
            lots.push(Lot {
                rect: egui::Rect::from_min_size(corner, egui::vec2(size, size)),
                uv: egui::Rect::from_min_max(
                    egui::pos2(u_left, tree.min.y),
                    egui::pos2(u_right, tree.max.y),
                ),
                layer: VillageLayer::Dwelling,
                picks: None,
                slot: None,
            });
            let half = TRINKET_CELL as f32 / 2.0;
            for (variant, hangs_in, anchor) in &hung {
                if *hangs_in != end {
                    continue;
                }
                lots.push(Lot {
                    rect: egui::Rect::from_min_size(
                        corner + egui::vec2(anchor.x as f32 - half, anchor.y as f32 - half),
                        egui::vec2(TRINKET_CELL as f32, TRINKET_CELL as f32),
                    ),
                    uv: Self::trinket_uv(*variant),
                    layer: VillageLayer::Trinket,
                    picks: None,
                    slot: None,
                });
            }
        }
        for slot in 0..=cottages.len() {
            let Some((monitor_id, point)) = formiga_core::home_dwelling_position(
                &save.home,
                slot,
                &cottages,
                monitors,
                &save.settings.habitat,
                scale,
            ) else {
                continue;
            };
            home_monitor.get_or_insert(monitor_id);
            if home_monitor != Some(monitor_id) {
                continue;
            }
            // Each house's own cell, the same one the desktop samples by day.
            let size = SHELTER_SIZE as f32;
            lots.push(Lot {
                rect: egui::Rect::from_min_size(
                    egui::pos2(point.x - size / 2.0, point.y - size),
                    egui::vec2(size, size),
                ),
                uv: village_uv(formiga_art::VillageCell::House {
                    slot,
                    lit: false,
                    occupied: false,
                }),
                layer: VillageLayer::Dwelling,
                picks: owners.get(slot).map(|keeper| Picked::House(*keeper)),
                slot: Some(slot),
            });
        }
        let yard = formiga_core::home_object_positions(
            &save.home,
            &cottages,
            monitors,
            &save.settings.habitat,
            scale,
        );
        let objects = save.objects.objects.len().min(MAX_COLONY_OBJECTS);
        for (slot, place) in yard.iter().enumerate().take(objects) {
            let Some((monitor_id, point)) = place else {
                continue;
            };
            if home_monitor != Some(*monitor_id) {
                continue;
            }
            let kind = save.objects.objects[slot].kind;
            let size = COLONY_OBJECT_SIZE as f32;
            lots.push(Lot {
                rect: egui::Rect::from_min_size(
                    egui::pos2(point.x - size / 2.0, point.y - size),
                    egui::vec2(size, size),
                ),
                uv: object_uv(ColonyObjectRenderer::object_cell(kind), false),
                layer: VillageLayer::Belonging,
                picks: None,
                slot: None,
            });
        }
        // The spots put down, the patches planted and the ornaments set out on the ground, drawn
        // as the desktop draws them: the lookout turned out over the open desktop, and every
        // garden at the stage it has grown to.
        let ground = formiga_core::home_ground_positions(
            &save.home,
            &cottages,
            monitors,
            &save.settings.habitat,
            scale,
        );
        for (item, monitor_id, point) in ground {
            if home_monitor != Some(monitor_id) {
                continue;
            }
            let middle = monitors
                .iter()
                .find(|monitor| monitor.id == monitor_id)
                .map_or(point.x, |monitor| {
                    monitor.usable_bounds.x + monitor.usable_bounds.width / 2.0
                });
            let size = COLONY_OBJECT_SIZE as f32;
            lots.push(Lot {
                rect: egui::Rect::from_min_size(
                    egui::pos2(point.x - size / 2.0, point.y - size),
                    egui::vec2(size, size),
                ),
                uv: object_uv(
                    ground_cell(save, item),
                    ColonyObjectRenderer::ground_mirrored(item, point.x, middle),
                ),
                layer: VillageLayer::Belonging,
                picks: Some(Picked::Ground(item)),
                slot: None,
            });
        }
        let arranging = self.arrange.arranging;
        let width = ui.available_width().clamp(280.0, 720.0);
        let frame = egui::vec2(width, if arranging { 230.0 } else { 170.0 });
        let sense = if arranging {
            egui::Sense::click_and_drag()
        } else {
            egui::Sense::click()
        };
        let (response, painter) = ui.allocate_painter(frame, sense);
        let area = response.rect;
        painter.rect_filled(area, 4.0, card_fill());
        if lots.is_empty() {
            painter.text(
                area.center(),
                egui::Align2::CENTER_CENTER,
                if save.home.is_active() {
                    "No room for the village here"
                } else {
                    "The colony is out exploring"
                },
                egui::TextStyle::Small.resolve(ui.style()),
                muted(),
            );
            return;
        }
        // Fit the whole strip, without ever stretching a pixel out of square.
        let bounds = lots
            .iter()
            .fold(lots[0].rect, |acc, lot| acc.union(lot.rect))
            .expand(4.0);
        let fit = (area.width() / bounds.width().max(1.0))
            .min(area.height() / bounds.height().max(1.0))
            .clamp(0.25, 3.0);
        let origin = area.center() - bounds.size() * fit / 2.0;
        let to_screen = |rect: egui::Rect| {
            egui::Rect::from_min_size(origin + (rect.min - bounds.min) * fit, rect.size() * fit)
        };
        let to_desktop_x = |x: f32| (x - origin.x) / fit + bounds.min.x;
        // Whatever is under the pointer that can be picked out: the thing in front first.
        let under = |pos: egui::Pos2| {
            lots.iter()
                .rev()
                .filter(|lot| lot.picks.is_some())
                .filter(|lot| to_screen(lot.rect).shrink(fit * 2.0).contains(pos))
                .max_by_key(|lot| lot.layer)
                .map(|lot| (lot.picks.expect("filtered"), lot.slot))
        };
        let commons = formiga_core::home_commons(
            &save.home,
            &cottages,
            monitors,
            &save.settings.habitat,
            scale,
        );
        // Taking hold of something, carrying it, and letting it go.
        if arranging {
            // What was under the pointer when it went down, not wherever it has got to by the
            // time the press has become a drag.
            let pressed_at = ui.input(|input| input.pointer.press_origin());
            if response.drag_started()
                && let Some(pos) = pressed_at.or_else(|| response.interact_pointer_pos())
                && let Some((picked, slot)) = under(pos)
                // The colony house always stands first, so it stays where it is.
                && slot != Some(0)
            {
                self.arrange.drag = Some(Drag {
                    picked,
                    x: to_desktop_x(pos.x),
                });
                self.arrange.picked = Some(picked);
            }
            if let Some(drag) = &mut self.arrange.drag
                && let Some(pos) = response.interact_pointer_pos()
            {
                drag.x = to_desktop_x(pos.x);
            }
            if response.drag_stopped()
                && let Some(drag) = self.arrange.drag.take()
            {
                self.drop_dragged(save, &lots, commons, drag, outcome);
            }
        } else {
            self.arrange.drag = None;
        }
        if response.clicked() {
            self.arrange.picked = response
                .interact_pointer_pos()
                .and_then(under)
                .map(|(p, _)| p);
            response.request_focus();
        }
        if arranging && response.has_focus() {
            self.nudge_picked(ui, save, owners, outcome);
        }
        // The houses and both trees behind, then what hangs in them, then everything on the
        // ground in front, exactly as the desktop layers them. Whatever is being carried is
        // drawn where the pointer has it instead, a little see-through.
        let dragging = self.arrange.drag;
        let mut drawn: Vec<&Lot> = lots.iter().collect();
        drawn.sort_by_key(|lot| lot.layer);
        for lot in &drawn {
            let carried = dragging.is_some_and(|drag| lot.picks == Some(drag.picked));
            let texture = match lot.layer {
                VillageLayer::Dwelling => village.id(),
                VillageLayer::Trinket => trinkets.id(),
                VillageLayer::Belonging => objects_texture.id(),
            };
            let placed = to_screen(lot.rect);
            if carried {
                painter.image(
                    texture,
                    placed,
                    lot.uv,
                    Color32::from_rgba_unmultiplied(255, 255, 255, 70),
                );
            } else {
                painter.image(texture, placed, lot.uv, Color32::WHITE);
            }
        }
        #[cfg(test)]
        {
            self.arrange.shown = lots
                .iter()
                .filter_map(|lot| Some((lot.picks?, to_screen(lot.rect))))
                .collect();
        }
        let hovered = response
            .hover_pos()
            .and_then(under)
            .map(|(picked, _)| picked);
        for lot in &drawn {
            let Some(picks) = lot.picks else {
                continue;
            };
            let placed = to_screen(lot.rect);
            if self.arrange.picked == Some(picks) {
                painter.rect_stroke(
                    placed,
                    3.0,
                    egui::Stroke::new(2.0, forest()),
                    egui::StrokeKind::Outside,
                );
            } else if hovered == Some(picks) && dragging.is_none() {
                painter.rect_stroke(
                    placed,
                    3.0,
                    egui::Stroke::new(1.0, gold()),
                    egui::StrokeKind::Outside,
                );
            }
        }
        if let Some(drag) = dragging {
            self.draw_carried(
                &painter,
                save,
                &lots,
                commons,
                drag,
                &to_screen,
                fit,
                [village.id(), objects_texture.id()],
            );
        }
        let hint = if dragging.is_some() {
            None
        } else if let Some(picked) = hovered {
            Some(picked_label(save, picked))
        } else if arranging {
            Some(
                "Drag a cottage along the row, or anything on the ground along the ground."
                    .to_owned(),
            )
        } else {
            Some("Click a house to decorate it.".to_owned())
        };
        if let Some(hint) = hint {
            painter.text(
                area.left_top() + egui::vec2(8.0, 6.0),
                egui::Align2::LEFT_TOP,
                hint,
                egui::TextStyle::Small.resolve(ui.style()),
                muted(),
            );
        }
    }

    /// Where something being carried would land, drawn over the preview: a cottage at the pointer
    /// with a marker in the gap it would drop into, or something on the ground at the pointer
    /// with the stretch of ground it can go anywhere along picked out beneath it.
    #[allow(clippy::too_many_arguments)]
    fn draw_carried(
        &self,
        painter: &egui::Painter,
        save: &SaveFile,
        lots: &[Lot],
        commons: Option<formiga_core::HomeCommons>,
        drag: Drag,
        to_screen: &dyn Fn(egui::Rect) -> egui::Rect,
        fit: f32,
        [village, objects]: [egui::TextureId; 2],
    ) {
        let Some(lot) = lots.iter().find(|lot| lot.picks == Some(drag.picked)) else {
            return;
        };
        let mut rect = lot.rect;
        match drag.picked {
            Picked::House(_) => {
                rect = rect.translate(egui::vec2(drag.x - rect.center().x, 0.0));
                // The gap it would drop into, between the houses either side of it.
                let (before, after) = self.cottage_neighbours(save, lots, drag);
                let marker_x = match (before, after) {
                    (Some(a), Some(b)) => (a + b) / 2.0,
                    (Some(a), None) => a + SHELTER_SIZE as f32 * 0.55,
                    (None, Some(b)) => b - SHELTER_SIZE as f32 * 0.55,
                    (None, None) => drag.x,
                };
                let top = to_screen(lot.rect);
                let x = to_screen(egui::Rect::from_min_size(
                    egui::pos2(marker_x, lot.rect.top()),
                    egui::vec2(0.0, 0.0),
                ))
                .left();
                painter.line_segment(
                    [egui::pos2(x, top.top()), egui::pos2(x, top.bottom())],
                    egui::Stroke::new(2.0, gold()),
                );
                painter.image(village, to_screen(rect), lot.uv, Color32::WHITE);
            }
            Picked::Ground(_) => {
                if let Some(commons) = commons {
                    let (low, high) = commons.standing_span();
                    let x = drag.x.clamp(low, high);
                    rect = rect.translate(egui::vec2(x - rect.center().x, 0.0));
                    let ground = |x: f32| {
                        to_screen(egui::Rect::from_min_size(
                            egui::pos2(x, commons.ground_y),
                            egui::vec2(0.0, 0.0),
                        ))
                        .min
                    };
                    painter.line_segment(
                        [ground(low), ground(high)],
                        egui::Stroke::new(fit.max(2.0), mint()),
                    );
                }
                painter.image(objects, to_screen(rect), lot.uv, Color32::WHITE);
            }
        }
    }

    /// The centres of the houses either side of where a carried cottage would drop, in the
    /// order the village runs.
    fn cottage_neighbours(
        &self,
        save: &SaveFile,
        lots: &[Lot],
        drag: Drag,
    ) -> (Option<f32>, Option<f32>) {
        let outward = save.home.corner == HomeCorner::BottomLeft;
        let mut centres: Vec<f32> = lots
            .iter()
            .filter(|lot| lot.slot.is_some() && lot.picks != Some(drag.picked))
            .map(|lot| lot.rect.center().x)
            .collect();
        centres.sort_by(f32::total_cmp);
        if !outward {
            centres.reverse();
        }
        let ahead = |centre: f32| {
            if outward {
                centre < drag.x
            } else {
                centre > drag.x
            }
        };
        let before = centres.iter().copied().rev().find(|c| ahead(*c));
        let after = centres.iter().copied().find(|c| !ahead(*c));
        (before, after)
    }

    /// Lets go of whatever was being carried: a cottage drops into the gap it was carried to,
    /// and something on the ground stays where it was let go, as a fraction of the ground.
    fn drop_dragged(
        &mut self,
        save: &SaveFile,
        lots: &[Lot],
        commons: Option<formiga_core::HomeCommons>,
        drag: Drag,
        outcome: &mut SettingsOutcome,
    ) {
        match drag.picked {
            Picked::House(keeper) => {
                let owners = formiga_core::house_owners(&save.creatures, &save.home.cottage_order);
                let mut order: Vec<CreatureId> = owners
                    .as_slice()
                    .iter()
                    .skip(1)
                    .copied()
                    .filter(|id| *id != keeper)
                    .collect();
                let outward = save.home.corner == HomeCorner::BottomLeft;
                // However many cottages stand ahead of where it was let go, in the order the
                // village runs, is where it goes in the row.
                let ahead = lots
                    .iter()
                    .filter(|lot| lot.slot.is_some_and(|slot| slot > 0))
                    .filter(|lot| lot.picks != Some(drag.picked))
                    .filter(|lot| {
                        if outward {
                            lot.rect.center().x < drag.x
                        } else {
                            lot.rect.center().x > drag.x
                        }
                    })
                    .count();
                order.insert(ahead.min(order.len()), keeper);
                if order.as_slice() != &owners.as_slice()[1..] {
                    outcome.cottage_order = Some(order);
                }
            }
            Picked::Ground(item) => {
                let Some(commons) = commons else {
                    return;
                };
                let (low, high) = commons.standing_span();
                if high - low < 1.0 {
                    return;
                }
                let along = ((drag.x - low) / (high - low)).clamp(0.0, 1.0);
                if along_of(save, item).is_none_or(|at| (at - along).abs() > 0.005) {
                    place(outcome, item, Some(along));
                }
            }
        }
    }

    /// The arrow keys move whatever is picked out: something on the ground a little way along
    /// it, and a cottage one place along the row.
    fn nudge_picked(
        &mut self,
        ui: &Ui,
        save: &SaveFile,
        owners: &[CreatureId],
        outcome: &mut SettingsOutcome,
    ) {
        let (left, right) = ui.input(|input| {
            (
                input.key_pressed(egui::Key::ArrowLeft),
                input.key_pressed(egui::Key::ArrowRight),
            )
        });
        if !left && !right {
            return;
        }
        match self.arrange.picked {
            Some(Picked::Ground(item)) => {
                if let Some(at) = along_of(save, item) {
                    let step = if right { NUDGE } else { -NUDGE };
                    place(outcome, item, Some((at + step).clamp(0.0, 1.0)));
                }
            }
            Some(Picked::House(keeper)) => {
                let Some(slot) = owners.iter().position(|id| *id == keeper) else {
                    return;
                };
                // Along the row the way the village runs: right is further out in a bottom-left
                // village and closer in a bottom-right one.
                let outward = save.home.corner == HomeCorner::BottomLeft;
                let further = right == outward;
                let mut order: Vec<CreatureId> = owners[1..].to_vec();
                if slot == 0 {
                    return;
                }
                let index = slot - 1;
                if further && index + 1 < order.len() {
                    order.swap(index, index + 1);
                    outcome.cottage_order = Some(order);
                } else if !further && index > 0 {
                    order.swap(index, index - 1);
                    outcome.cottage_order = Some(order);
                }
            }
            None => {}
        }
    }

    /// The house picked out in the preview: whose it is, what it is built as, and what it wears
    /// in each of its six places, from what the village has to choose from so far.
    pub(super) fn picked_house(
        &mut self,
        ui: &mut Ui,
        save: &SaveFile,
        outcome: &mut SettingsOutcome,
    ) {
        let Some(Picked::House(keeper_id)) = self.arrange.picked else {
            return;
        };
        let owners = formiga_core::house_owners(&save.creatures, &save.home.cottage_order);
        let Some(slot) = owners.as_slice().iter().position(|id| *id == keeper_id) else {
            self.arrange.picked = None;
            return;
        };
        let Some(keeper) = save
            .creatures
            .iter()
            .find(|creature| creature.id == keeper_id)
        else {
            return;
        };
        card(ui, |ui| {
            ui.horizontal(|ui| {
                let curtain = formiga_art::ResidentMark::of(keeper);
                swatch(ui, curtain.cloth, curtain.tie);
                ui.strong(if slot == 0 {
                    format!("The colony house · {}'s", keeper.name)
                } else {
                    format!("Cottage {slot} · {}'s", keeper.name)
                });
                // Who else lives there: a mini shares its big version's house.
                let minis: Vec<&str> = save
                    .creatures
                    .iter()
                    .filter(|creature| creature.role.parent_id() == Some(keeper.id))
                    .map(|creature| creature.name.as_str())
                    .collect();
                if !minis.is_empty() {
                    ui.small(format!("with {}", minis.join(" and ")));
                }
            });
            ui.horizontal(|ui| {
                ui.label("Built as");
                let own = if slot == 0 {
                    save.home.shelter.style
                } else {
                    ShelterStyle::for_keeper(keeper)
                };
                let chosen = save.home.house_style(keeper.id);
                egui::ComboBox::from_id_salt(("house-type", keeper.id))
                    .selected_text(chosen.unwrap_or(own).label())
                    .show_ui(ui, |ui| {
                        let own_label = format!("Its own · {}", own.label());
                        if ui.selectable_label(chosen.is_none(), own_label).clicked()
                            && chosen.is_some()
                        {
                            outcome.house_style = Some((keeper.id, None));
                        }
                        for style in ShelterStyle::ALL {
                            if ui
                                .selectable_label(chosen == Some(style), style.label())
                                .clicked()
                                && chosen != Some(style)
                            {
                                outcome.house_style = Some((keeper.id, Some(style)));
                            }
                        }
                    });
            });
            ui.add_space(6.0);
            ui.small("What it wears, one thing in each place:");
            egui::Grid::new(("house-decorations", keeper.id))
                .num_columns(4)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    for (index, place) in DecorationSlot::ALL.into_iter().enumerate() {
                        ui.label(place.label());
                        let current = save.home.decoration_in(keeper.id, place);
                        let choices: Vec<ShelterDecorationKind> = save
                            .home
                            .unlocks
                            .decorations
                            .iter()
                            .copied()
                            .filter(|kind| kind.slot() == place)
                            .collect();
                        egui::ComboBox::from_id_salt(("decoration", keeper.id, index))
                            .width(130.0)
                            .selected_text(current.map_or("Nothing", ShelterDecorationKind::label))
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(current.is_none(), "Nothing").clicked()
                                    && current.is_some()
                                {
                                    outcome.set_decoration = Some((keeper.id, place, None));
                                }
                                for kind in choices {
                                    if ui
                                        .selectable_label(current == Some(kind), kind.label())
                                        .clicked()
                                        && current != Some(kind)
                                    {
                                        outcome.set_decoration =
                                            Some((keeper.id, place, Some(kind)));
                                    }
                                }
                            });
                        if index % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
            let waiting = ShelterDecorationKind::ALL.len() - save.home.unlocks.decorations.len();
            if waiting > 0 {
                ui.small(format!(
                    "{waiting} more decorations are still to come; something new arrives every \
                     day or two."
                ));
            }
        });
    }

    /// Everything that can go on the village ground, as three shelves of what the village has
    /// so far: gardens, spots to spend time at, and ornaments. Clicking one puts it down
    /// somewhere with room, or takes it up again; where it stands is arranged in the preview.
    pub(super) fn ground_catalogue(
        &mut self,
        ui: &mut Ui,
        save: &SaveFile,
        outcome: &mut SettingsOutcome,
    ) {
        let texture = self
            .object_texture
            .as_ref()
            .map(|(_, texture)| texture.clone());
        let shelf = |ui: &mut Ui,
                     title: &str,
                     blurb: &str,
                     items: Vec<(GroundItem, &'static str)>,
                     total: usize,
                     most: usize,
                     outcome: &mut SettingsOutcome| {
            let down = items
                .iter()
                .filter(|(item, _)| along_of(save, *item).is_some())
                .count();
            ui.horizontal(|ui| {
                ui.strong(title);
                ui.small(format!(
                    "{} of {total} so far · {down} of {most} out",
                    items.len()
                ));
            });
            ui.small(blurb);
            ui.horizontal_wrapped(|ui| {
                for (item, description) in items {
                    let placed = along_of(save, item).is_some();
                    let full = !placed && down >= most;
                    let (tile, rect) = tile(
                        ui,
                        egui::vec2(96.0, 78.0),
                        if placed { mint() } else { card_fill() },
                        gold(),
                    );
                    if let Some(texture) = &texture {
                        ui.painter().image(
                            texture.id(),
                            egui::Rect::from_center_size(
                                egui::pos2(rect.center().x, rect.top() + 26.0),
                                egui::vec2(40.0, 40.0),
                            ),
                            object_uv(ground_cell(save, item), false),
                            Color32::WHITE,
                        );
                    }
                    let name = ui.painter().layout(
                        item.label().to_owned(),
                        egui::TextStyle::Small.resolve(ui.style()),
                        ink(),
                        rect.width() - 8.0,
                    );
                    let name_at = egui::pos2(
                        rect.center().x - name.size().x / 2.0,
                        rect.bottom() - 4.0 - name.size().y,
                    );
                    ui.painter().galley(name_at, name, ink());
                    let tile = tile.on_hover_text(if full {
                        format!("{description}\n\nTake one of the others up first.")
                    } else if placed {
                        format!("{description}\n\nClick to take it up again.")
                    } else {
                        format!("{description}\n\nClick to put it down.")
                    });
                    if tile.clicked() && !full {
                        place(
                            outcome,
                            item,
                            if placed { None } else { Some(free_along(save)) },
                        );
                    }
                }
            });
            ui.add_space(10.0);
        };
        card(ui, |ui| {
            shelf(
                ui,
                "Gardens",
                "Patches the colony waters, looks in on, picks from and shows off. Each grows by \
                 itself — nothing wilts for want of tending.",
                save.home
                    .unlocks
                    .gardens
                    .iter()
                    .map(|kind| (GroundItem::Garden(*kind), kind.description()))
                    .collect(),
                GardenKind::ALL.len(),
                formiga_core::MAX_GARDENS,
                outcome,
            );
            shelf(
                ui,
                "Hangout spots",
                "Somewhere the colony goes to nap, snack or look about while the houses are out.",
                save.home
                    .unlocks
                    .hangouts
                    .iter()
                    .map(|kind| (GroundItem::Hangout(*kind), kind.description()))
                    .collect(),
                HangoutKind::ALL.len(),
                formiga_core::MAX_HANGOUTS,
                outcome,
            );
            shelf(
                ui,
                "Ornaments",
                "Something to stand on the ground and be wandered over to.",
                save.home
                    .unlocks
                    .ornaments
                    .iter()
                    .map(|kind| (GroundItem::Ornament(*kind), kind.description()))
                    .collect(),
                OrnamentKind::ALL.len(),
                formiga_core::MAX_ORNAMENTS,
                outcome,
            );
        });
    }
}
