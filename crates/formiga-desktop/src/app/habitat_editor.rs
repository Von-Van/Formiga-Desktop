//! The desktop region editor: drawing, moving and resizing the rectangles where creatures may and
//! may not go, over the overlays, with the settings window kept in front.
use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct HabitatEditorDrag {
    pub(super) zone_id: u64,
    pub(super) monitor_id: MonitorId,
    pub(super) start: Point,
    pub(super) mode: HabitatEditorDragMode,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum HabitatEditorDragMode {
    Create,
    Move {
        original: DesktopRect,
    },
    Resize {
        original: DesktopRect,
        left: bool,
        right: bool,
        top: bool,
        bottom: bool,
    },
}

#[derive(Clone, Debug)]
pub(super) struct HabitatEditor {
    pub(super) draft: HabitatPolicy,
    pub(super) previous_paused: bool,
    pub(super) drag: Option<HabitatEditorDrag>,
}

/// What an open habitat editor takes for itself: the pointer, and nothing else.
///
/// While it is open a press on the desktop draws a region rather than reaching the colony, so the
/// editor answers the mouse before anybody else does. Every other event still belongs to the
/// overlay it was sent to — a redraw above all. An editor that claimed those too left the colony
/// frozen on its last frame and never drew one of the regions it was asked for, which is exactly
/// what an application that has crashed looks like.
pub(super) fn habitat_editor_claims(event: &WindowEvent) -> bool {
    matches!(
        event,
        WindowEvent::CursorMoved { .. } | WindowEvent::MouseInput { .. }
    )
}

pub(super) fn normalized_drag_rect(bounds: DesktopRect, a: Point, b: Point) -> DesktopRect {
    let left = a.x.min(b.x).clamp(bounds.x, bounds.right());
    let top = a.y.min(b.y).clamp(bounds.y, bounds.bottom());
    let right = a.x.max(b.x).clamp(bounds.x, bounds.right());
    let bottom = a.y.max(b.y).clamp(bounds.y, bounds.bottom());
    DesktopRect {
        x: (left - bounds.x) / bounds.width,
        y: (top - bounds.y) / bounds.height,
        width: ((right - left) / bounds.width).max(0.0001),
        height: ((bottom - top) / bounds.height).max(0.0001),
    }
}

pub(super) fn denormalized_zone_rect(bounds: DesktopRect, normalized: DesktopRect) -> DesktopRect {
    DesktopRect {
        x: bounds.x + normalized.x * bounds.width,
        y: bounds.y + normalized.y * bounds.height,
        width: normalized.width * bounds.width,
        height: normalized.height * bounds.height,
    }
}

pub(super) fn resized_zone(
    bounds: DesktopRect,
    original: DesktopRect,
    start: Point,
    current: Point,
    edges: [bool; 4],
) -> DesktopRect {
    let [resize_left, resize_right, resize_top, resize_bottom] = edges;
    let dx = (current.x - start.x) / bounds.width;
    let dy = (current.y - start.y) / bounds.height;
    let mut left = original.x;
    let mut right = original.right();
    let mut top = original.y;
    let mut bottom = original.bottom();
    if resize_left {
        left = (left + dx).clamp(0.0, right - 0.02);
    }
    if resize_right {
        right = (right + dx).clamp(left + 0.02, 1.0);
    }
    if resize_top {
        top = (top + dy).clamp(0.0, bottom - 0.02);
    }
    if resize_bottom {
        bottom = (bottom + dy).clamp(top + 0.02, 1.0);
    }
    DesktopRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    }
}

impl FormigaApp {
    pub(super) fn begin_habitat_editor(&mut self, draft: HabitatPolicy) {
        if self.habitat_editor.is_some() {
            return;
        }
        let previous_paused = self
            .world
            .as_ref()
            .is_some_and(|world| world.save.settings.paused);
        if let Some(world) = &mut self.world {
            world.save.settings.paused = true;
        }
        self.habitat_editor = Some(HabitatEditor {
            draft,
            previous_paused,
            drag: None,
        });
        for overlay in self.overlays.values_mut() {
            overlay.set_hittest_enabled(true);
            overlay.window.request_redraw();
        }
        if let Some(window) = &mut self.settings_window {
            window.set_editor_active(true);
            window.window.set_window_level(WindowLevel::AlwaysOnTop);
            window.window.focus_window();
        }
        self.raise_settings_above_overlays();
    }

    pub(super) fn finish_habitat_editor(&mut self, apply: bool) {
        let Some(editor) = self.habitat_editor.take() else {
            return;
        };
        let accepted = apply && validate_habitat(&editor.draft, &self.monitors).is_ok();
        if let Some(world) = &mut self.world {
            world.save.settings.paused = editor.previous_paused;
            if accepted {
                world.save.settings.habitat = editor.draft;
            }
        }
        for overlay in self.overlays.values_mut() {
            overlay.set_hittest_enabled(false);
            overlay.window.request_redraw();
        }
        let habitat = self
            .world
            .as_ref()
            .map(|world| world.save.settings.habitat.clone())
            .unwrap_or_default();
        if let Some(window) = &mut self.settings_window {
            window.set_editor_active(false);
            window.window.set_window_level(WindowLevel::Normal);
            window.set_habitat(habitat);
            window.window.focus_window();
        }
        self.raise_settings_above_overlays();
        if accepted {
            let desktop = self.snapshot();
            if let Some(world) = &mut self.world {
                world.handle_command(WorldCommand::GatherCreatures, &desktop);
            }
            let _ = self.save();
        }
    }

    pub(super) fn handle_habitat_editor_event(
        &mut self,
        window_id: WindowId,
        event: &WindowEvent,
    ) -> bool {
        if self.habitat_editor.is_none() {
            return false;
        }
        let Some(monitor) = self
            .overlays
            .get(&window_id)
            .map(|overlay| overlay.monitor.clone())
        else {
            return false;
        };
        if !habitat_editor_claims(event) {
            return false;
        }

        if let WindowEvent::CursorMoved { position, .. } = event {
            self.current_cursor = CursorSnapshot {
                position: Point {
                    x: monitor.bounds.x + position.x as f32 / monitor.scale_factor,
                    y: monitor.bounds.y + position.y as f32 / monitor.scale_factor,
                },
                velocity: Point::default(),
                available: true,
            };
            self.update_habitat_editor_drag(monitor.id, self.current_cursor.position);
            if let Some(overlay) = self.overlays.get(&window_id) {
                overlay.window.request_redraw();
            }
        }

        match event {
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } if matches!(
                button,
                MouseButton::Left | MouseButton::Right | MouseButton::Middle
            ) =>
            {
                self.start_habitat_editor_drag(&monitor, *button);
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button,
                ..
            } => {
                if matches!(button, MouseButton::Left | MouseButton::Right) {
                    self.end_habitat_editor_drag(&monitor);
                }
                // The press that began this gesture ordered a full-screen overlay in front of the
                // settings window, and Apply and Cancel live there and nowhere else. Hand it back
                // its place now the gesture is over, rather than leaving a desktop that answers
                // nothing but the tray.
                self.raise_settings_above_overlays();
            }
            _ => {}
        }
        true
    }

    /// Keep the settings window above the editor's overlays for as long as the edit lasts, and
    /// put it back among them afterwards.
    pub(super) fn raise_settings_above_overlays(&mut self) {
        let raised = self.habitat_editor.is_some();
        if let Some(window) = &self.settings_window {
            platform::raise_above_overlays(&window.window, raised);
        }
    }

    pub(super) fn start_habitat_editor_drag(&mut self, monitor: &MonitorInfo, button: MouseButton) {
        let Some(editor) = &mut self.habitat_editor else {
            return;
        };
        if editor.drag.is_some() {
            return;
        }
        let point = monitor.usable_bounds.clamp(self.current_cursor.position);
        let hit = editor
            .draft
            .zones
            .iter()
            .rev()
            .find(|zone| {
                zone.enabled
                    && zone.display == monitor.display_key
                    && denormalized_zone_rect(monitor.usable_bounds, zone.normalized_bounds)
                        .contains(point)
            })
            .map(|zone| (zone.id, zone.normalized_bounds));
        if let Some((zone_id, original)) = hit {
            editor.draft.preset = HabitatPreset::Custom;
            if button == MouseButton::Middle {
                editor.draft.zones.retain(|zone| zone.id != zone_id);
                if let Some(window) = &mut self.settings_window {
                    window.set_habitat(editor.draft.clone());
                }
                return;
            }
            if button == MouseButton::Right {
                if let Some(zone) = editor
                    .draft
                    .zones
                    .iter_mut()
                    .find(|zone| zone.id == zone_id)
                {
                    zone.kind = match zone.kind {
                        HabitatZoneKind::Allowed => HabitatZoneKind::Excluded,
                        HabitatZoneKind::Excluded => HabitatZoneKind::Allowed,
                    };
                }
                if let Some(window) = &mut self.settings_window {
                    window.set_habitat(editor.draft.clone());
                }
                return;
            }
            let rect = denormalized_zone_rect(monitor.usable_bounds, original);
            let threshold = 12.0;
            let left = (point.x - rect.x).abs() <= threshold;
            let right = (point.x - rect.right()).abs() <= threshold;
            let top = (point.y - rect.y).abs() <= threshold;
            let bottom = (point.y - rect.bottom()).abs() <= threshold;
            let mode = if left || right || top || bottom {
                HabitatEditorDragMode::Resize {
                    original,
                    left,
                    right,
                    top,
                    bottom,
                }
            } else {
                HabitatEditorDragMode::Move { original }
            };
            editor.drag = Some(HabitatEditorDrag {
                zone_id,
                monitor_id: monitor.id,
                start: point,
                mode,
            });
            return;
        }
        if editor.draft.zones.len() >= MAX_HABITAT_ZONES || button == MouseButton::Middle {
            return;
        }
        let zone_id = editor
            .draft
            .zones
            .iter()
            .map(|zone| zone.id)
            .max()
            .unwrap_or_default()
            + 1;
        editor.draft.preset = HabitatPreset::Custom;
        editor.draft.zones.push(HabitatZone {
            id: zone_id,
            display: monitor.display_key,
            normalized_bounds: normalized_drag_rect(monitor.usable_bounds, point, point),
            kind: if button == MouseButton::Right {
                HabitatZoneKind::Excluded
            } else {
                HabitatZoneKind::Allowed
            },
            enabled: true,
        });
        editor.drag = Some(HabitatEditorDrag {
            zone_id,
            monitor_id: monitor.id,
            start: point,
            mode: HabitatEditorDragMode::Create,
        });
    }

    pub(super) fn update_habitat_editor_drag(&mut self, monitor_id: MonitorId, point: Point) {
        let Some(editor) = &mut self.habitat_editor else {
            return;
        };
        let Some(drag) = editor.drag else { return };
        if drag.monitor_id != monitor_id {
            return;
        }
        let Some(monitor) = self.monitors.iter().find(|item| item.id == monitor_id) else {
            return;
        };
        if let Some(zone) = editor
            .draft
            .zones
            .iter_mut()
            .find(|zone| zone.id == drag.zone_id)
        {
            let point = monitor.usable_bounds.clamp(point);
            zone.normalized_bounds = match drag.mode {
                HabitatEditorDragMode::Create => {
                    normalized_drag_rect(monitor.usable_bounds, drag.start, point)
                }
                HabitatEditorDragMode::Move { original } => {
                    let dx = (point.x - drag.start.x) / monitor.usable_bounds.width;
                    let dy = (point.y - drag.start.y) / monitor.usable_bounds.height;
                    DesktopRect {
                        x: (original.x + dx).clamp(0.0, 1.0 - original.width),
                        y: (original.y + dy).clamp(0.0, 1.0 - original.height),
                        ..original
                    }
                }
                HabitatEditorDragMode::Resize {
                    original,
                    left,
                    right,
                    top,
                    bottom,
                } => resized_zone(
                    monitor.usable_bounds,
                    original,
                    drag.start,
                    point,
                    [left, right, top, bottom],
                ),
            };
        }
    }

    pub(super) fn end_habitat_editor_drag(&mut self, monitor: &MonitorInfo) {
        self.update_habitat_editor_drag(monitor.id, self.current_cursor.position);
        let Some(editor) = &mut self.habitat_editor else {
            return;
        };
        let Some(drag) = editor.drag.take() else {
            return;
        };
        let too_small = matches!(drag.mode, HabitatEditorDragMode::Create)
            && editor
                .draft
                .zones
                .iter()
                .find(|zone| zone.id == drag.zone_id)
                .is_none_or(|zone| {
                    zone.normalized_bounds.width * monitor.usable_bounds.width < 16.0
                        || zone.normalized_bounds.height * monitor.usable_bounds.height < 16.0
                });
        if too_small {
            editor.draft.zones.retain(|zone| zone.id != drag.zone_id);
        }
        if let Some(window) = &mut self.settings_window {
            window.set_habitat(editor.draft.clone());
        }
    }
}
