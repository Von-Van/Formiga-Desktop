//! Which parts of a display are covered, by chosen applications or a full-screen window, and the
//! habitat zones the editor draws.
use super::*;

pub(crate) fn visible_occlusion_rects(
    monitor: DesktopRect,
    windows: &[DesktopWindow],
    rules: &[ApplicationOcclusionRule],
    fullscreen_app_occlusion: bool,
) -> Vec<DesktopRect> {
    if fullscreen_app_occlusion && monitor_has_fullscreen_window(monitor, windows) {
        return vec![monitor];
    }
    let selected: BTreeSet<_> = rules
        .iter()
        .filter(|rule| rule.enabled)
        .map(|rule| &rule.application)
        .collect();
    let mut output = Vec::new();
    for target in windows.iter().filter(|window| {
        window.visible
            && !window.minimized
            && window
                .application
                .as_ref()
                .is_some_and(|application| selected.contains(application))
    }) {
        let Some(target_bounds) = target.bounds.intersection(monitor) else {
            continue;
        };
        let mut visible = vec![target_bounds];
        for covering in windows.iter().filter(|candidate| {
            candidate.visible
                && !candidate.minimized
                && candidate.z_order < target.z_order
                && candidate.key != target.key
        }) {
            visible = visible
                .into_iter()
                .flat_map(|rect| subtract_rect(rect, covering.bounds))
                .collect();
            if visible.is_empty() {
                break;
            }
        }
        output.extend(visible);
        if output.len() >= MAX_OCCLUSION_RECTS {
            output.truncate(MAX_OCCLUSION_RECTS);
            break;
        }
    }
    output
}

pub(crate) fn monitor_has_fullscreen_window(
    monitor: DesktopRect,
    windows: &[DesktopWindow],
) -> bool {
    windows.iter().any(|window| {
        window.visible && !window.minimized && bounds_match_monitor(window.bounds, monitor)
    })
}

pub(super) fn bounds_match_monitor(window: DesktopRect, monitor: DesktopRect) -> bool {
    pub(super) const EDGE_TOLERANCE: f32 = 3.0;
    (window.x - monitor.x).abs() <= EDGE_TOLERANCE
        && (window.y - monitor.y).abs() <= EDGE_TOLERANCE
        && (window.right() - monitor.right()).abs() <= EDGE_TOLERANCE
        && (window.bottom() - monitor.bottom()).abs() <= EDGE_TOLERANCE
}

pub(super) fn rects_cover(target: DesktopRect, covering: &[DesktopRect]) -> bool {
    let mut remaining = vec![target];
    for cut in covering {
        remaining = remaining
            .into_iter()
            .flat_map(|rect| subtract_rect(rect, *cut))
            .collect();
        if remaining.is_empty() {
            return true;
        }
    }
    false
}

pub(super) fn subtract_rect(source: DesktopRect, cut: DesktopRect) -> Vec<DesktopRect> {
    let Some(overlap) = source.intersection(cut) else {
        return vec![source];
    };
    [
        DesktopRect {
            x: source.x,
            y: source.y,
            width: source.width,
            height: overlap.y - source.y,
        },
        DesktopRect {
            x: source.x,
            y: overlap.bottom(),
            width: source.width,
            height: source.bottom() - overlap.bottom(),
        },
        DesktopRect {
            x: source.x,
            y: overlap.y,
            width: overlap.x - source.x,
            height: overlap.height,
        },
        DesktopRect {
            x: overlap.right(),
            y: overlap.y,
            width: source.right() - overlap.right(),
            height: overlap.height,
        },
    ]
    .into_iter()
    .filter(|rect| rect.width > 0.0 && rect.height > 0.0)
    .collect()
}

/// Who this monitor draws right now: its own colony members, and the guest while it is out.
///
/// The one list decides three things at once, which is why it is a function rather than a filter
/// written twice: which sprites are baked, which the overlay keeps — anyone not here is evicted —
/// and which creatures a bubble or a menu may be drawn against.
pub(super) fn drawn_on_monitor(
    save: &SaveFile,
    monitor_id: formiga_core::MonitorId,
    fully_occluded: bool,
) -> Vec<&Creature> {
    save.creatures
        .iter()
        .chain(save.visitors.on_stage())
        .filter(|creature| {
            creature.state.surface.monitor_id == monitor_id
                && creature.state.arrival_delay_secs <= 0.0
                // Inside its house, behind the drawn curtain.
                && !creature.state.indoors
                && (!fully_occluded || creature.state.action == ActionKind::Dragged)
        })
        .collect()
}

impl OverlayRenderer {
    pub(super) fn update_occlusion_cache(&mut self, save: &SaveFile, windows: &[DesktopWindow]) {
        let rules = &save.settings.application_occlusion_rules;
        let fullscreen = save.settings.fullscreen_app_occlusion;
        let changed = !self.occlusion_initialized
            || self.occlusion_monitor_bounds != self.monitor.bounds
            || self.occlusion_fullscreen_enabled != fullscreen
            || self.occlusion_windows != windows
            || self.occlusion_rules != *rules;
        if !changed {
            return;
        }
        self.occlusion_rects =
            visible_occlusion_rects(self.monitor.bounds, windows, rules, fullscreen);
        self.occlusion_windows.clear();
        self.occlusion_windows.extend_from_slice(windows);
        self.occlusion_rules.clone_from(rules);
        self.occlusion_fullscreen_enabled = fullscreen;
        self.occlusion_monitor_bounds = self.monitor.bounds;
        self.occlusion_initialized = true;
    }

    pub(super) fn zone_vertices(&self, policy: &HabitatPolicy) -> Vec<ZoneVertex> {
        let mut vertices = Vec::new();
        for region in accessible_regions(policy, &self.monitor) {
            vertices.extend_from_slice(&self.zone_rect_vertices(region, [0.18, 0.72, 0.45, 0.20]));
        }
        for zone in policy.zones.iter().filter(|zone| {
            zone.enabled
                && zone.display == self.monitor.display_key
                && zone.kind == HabitatZoneKind::Excluded
        }) {
            let bounds = self.monitor.usable_bounds;
            let rect = formiga_core::DesktopRect {
                x: bounds.x + zone.normalized_bounds.x.clamp(0.0, 1.0) * bounds.width,
                y: bounds.y + zone.normalized_bounds.y.clamp(0.0, 1.0) * bounds.height,
                width: zone.normalized_bounds.width.clamp(0.0, 1.0) * bounds.width,
                height: zone.normalized_bounds.height.clamp(0.0, 1.0) * bounds.height,
            };
            if let Some(rect) = rect.intersection(bounds) {
                vertices
                    .extend_from_slice(&self.zone_rect_vertices(rect, [0.88, 0.30, 0.25, 0.28]));
            }
        }
        vertices.truncate(768);
        vertices
    }

    pub(super) fn occlusion_uniform(&self, rects: &[DesktopRect]) -> OcclusionUniform {
        let mut uniform = OcclusionUniform::zeroed();
        // The shader compares these with the fragment's own position, which is in drawable pixels.
        let to_drawable = self.monitor.scale_factor / self.render_divisor as f32;
        for (target, rect) in uniform.rects.iter_mut().zip(rects.iter()) {
            *target = [
                (rect.x - self.monitor.bounds.x) * to_drawable,
                (rect.y - self.monitor.bounds.y) * to_drawable,
                (rect.right() - self.monitor.bounds.x) * to_drawable,
                (rect.bottom() - self.monitor.bounds.y) * to_drawable,
            ];
        }
        uniform.metadata[0] = rects.len().min(MAX_OCCLUSION_RECTS) as u32;
        uniform
    }

    pub(super) fn zone_rect_vertices(
        &self,
        rect: formiga_core::DesktopRect,
        color: [f32; 4],
    ) -> [ZoneVertex; 6] {
        let left_px = self.snap((rect.x - self.monitor.bounds.x) * self.monitor.scale_factor);
        let right_px =
            self.snap((rect.right() - self.monitor.bounds.x) * self.monitor.scale_factor);
        let top_px = self.snap((rect.y - self.monitor.bounds.y) * self.monitor.scale_factor);
        let bottom_px =
            self.snap((rect.bottom() - self.monitor.bounds.y) * self.monitor.scale_factor);
        let left = left_px / self.layout.width as f32 * 2.0 - 1.0;
        let right = right_px / self.layout.width as f32 * 2.0 - 1.0;
        let top = 1.0 - top_px / self.layout.height as f32 * 2.0;
        let bottom = 1.0 - bottom_px / self.layout.height as f32 * 2.0;
        [
            ZoneVertex {
                position: [left, top],
                color,
            },
            ZoneVertex {
                position: [right, top],
                color,
            },
            ZoneVertex {
                position: [right, bottom],
                color,
            },
            ZoneVertex {
                position: [left, top],
                color,
            },
            ZoneVertex {
                position: [right, bottom],
                color,
            },
            ZoneVertex {
                position: [left, bottom],
                color,
            },
        ]
    }
}
