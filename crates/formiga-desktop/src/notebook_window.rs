//! Where the notebook window was, so it opens there again: its position, its size and whether it
//! was zoomed to fill the screen. Kept in `notebook-window.json` beside the colony rather than in
//! the colony file, because it belongs to this computer's displays and not to the colony: a backup
//! carried to another computer should not carry a window position with it.
//!
//! A remembered place is only used while it still makes sense. A display that has gone, been
//! rearranged, or changed resolution can leave the remembered spot off every screen, or a size
//! larger than any of them; the window then opens centred on the main display, as large as it
//! was but no larger than that display allows.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use winit::dpi::{LogicalSize, PhysicalPosition};

/// The smallest the notebook may be, in points. Every page is laid out to fit it.
pub const MIN_SIZE: (f64, f64) = (760.0, 560.0);
/// The size it opens at the first time, in points.
pub const DEFAULT_SIZE: (f64, f64) = (940.0, 720.0);

/// The window as it was last left: its outer top-left corner in physical pixels on the desktop,
/// its inner size in points, and whether it was zoomed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NotebookGeometry {
    pub x: i32,
    pub y: i32,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub maximized: bool,
}

/// One display as winit sees it: physical pixels on the shared desktop, and its scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub primary: bool,
}

impl Screen {
    fn logical_size(&self) -> (f64, f64) {
        let scale = self.scale.max(0.5);
        (
            f64::from(self.width) / scale,
            f64::from(self.height) / scale,
        )
    }

    /// How much of `rect` (physical x, y, width, height) lies on this screen, in square pixels.
    fn overlap(&self, rect: (i64, i64, i64, i64)) -> i64 {
        let (x, y, w, h) = rect;
        let left = x.max(i64::from(self.x));
        let top = y.max(i64::from(self.y));
        let right = (x + w).min(i64::from(self.x) + i64::from(self.width));
        let bottom = (y + h).min(i64::from(self.y) + i64::from(self.height));
        (right - left).max(0) * (bottom - top).max(0)
    }
}

/// Where and how large to open the notebook.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// `None` leaves it to the system, which only happens with no displays known at all.
    pub position: Option<PhysicalPosition<i32>>,
    pub size: LogicalSize<f64>,
    pub maximized: bool,
}

/// How much of the window's title strip has to be on a display for the remembered spot to count
/// as reachable: enough to see it and drag it by, in points.
const GRIP: (f64, f64) = (120.0, 24.0);
/// What is left round the notebook when a display is too small for its remembered size.
const BREATHING_ROOM: f64 = 48.0;

/// Decide where the notebook opens, given what was remembered and the displays there are now.
pub fn place(saved: Option<NotebookGeometry>, screens: &[Screen]) -> Placement {
    let main = screens
        .iter()
        .find(|screen| screen.primary)
        .or_else(|| screens.first());
    let wanted = saved.map_or(DEFAULT_SIZE, |geometry| {
        let finite = |value: f64, fallback: f64| {
            if value.is_finite() && value > 0.0 {
                value
            } else {
                fallback
            }
        };
        (
            finite(geometry.width, DEFAULT_SIZE.0),
            finite(geometry.height, DEFAULT_SIZE.1),
        )
    });
    let fit = |screen: &Screen| {
        let (room_w, room_h) = screen.logical_size();
        (
            wanted
                .0
                .min(room_w - BREATHING_ROOM)
                .max(MIN_SIZE.0.min(room_w)),
            wanted
                .1
                .min(room_h - BREATHING_ROOM)
                .max(MIN_SIZE.1.min(room_h)),
        )
    };
    // The remembered spot, if its title strip is still somewhere a person can reach.
    if let Some(geometry) = saved {
        let home = screens
            .iter()
            .max_by_key(|screen| {
                let scale = screen.scale.max(0.5);
                screen.overlap((
                    i64::from(geometry.x),
                    i64::from(geometry.y),
                    (wanted.0 * scale) as i64,
                    (wanted.1 * scale) as i64,
                ))
            })
            .filter(|screen| {
                let scale = screen.scale.max(0.5);
                let grip = (
                    i64::from(geometry.x),
                    i64::from(geometry.y),
                    (wanted.0.min(GRIP.0 * 3.0) * scale) as i64,
                    (GRIP.1 * scale) as i64,
                );
                screens.iter().any(|candidate| {
                    candidate.overlap(grip) >= ((GRIP.0 * GRIP.1) * scale * scale) as i64
                })
            });
        if let Some(screen) = home {
            let (width, height) = fit(screen);
            return Placement {
                position: Some(PhysicalPosition::new(geometry.x, geometry.y)),
                size: LogicalSize::new(width, height),
                maximized: geometry.maximized,
            };
        }
    }
    // Otherwise centred on the main display.
    let Some(screen) = main else {
        return Placement {
            position: None,
            size: LogicalSize::new(wanted.0, wanted.1),
            maximized: false,
        };
    };
    let (width, height) = fit(screen);
    let scale = screen.scale.max(0.5);
    let x = screen.x + ((f64::from(screen.width) - width * scale) / 2.0).max(0.0) as i32;
    let y = screen.y + ((f64::from(screen.height) - height * scale) / 2.0).max(0.0) as i32;
    Placement {
        position: Some(PhysicalPosition::new(x, y)),
        size: LogicalSize::new(width, height),
        maximized: saved.is_some_and(|geometry| geometry.maximized),
    }
}

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("notebook-window.json")
}

/// What was remembered, if anything readable was. A file that cannot be read is simply ignored:
/// the notebook opens where it would have the first time.
pub fn load(data_dir: &Path) -> Option<NotebookGeometry> {
    let bytes = std::fs::read(path(data_dir)).ok()?;
    if bytes.len() > 4096 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

/// Remember where the notebook is. Written beside the colony and swapped into place, so a
/// half-written file is never read back.
pub fn save(data_dir: &Path, geometry: NotebookGeometry) -> std::io::Result<()> {
    let target = path(data_dir);
    let temporary = target.with_extension("json.tmp");
    std::fs::write(&temporary, serde_json::to_vec(&geometry)?)?;
    std::fs::rename(temporary, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retina() -> Screen {
        Screen {
            x: 0,
            y: 0,
            width: 2880,
            height: 1800,
            scale: 2.0,
            primary: true,
        }
    }

    fn side_monitor() -> Screen {
        Screen {
            x: 2880,
            y: 0,
            width: 1920,
            height: 1080,
            scale: 1.0,
            primary: false,
        }
    }

    #[test]
    fn a_first_opening_is_centred_on_the_main_display_at_the_default_size() {
        let placement = place(None, &[side_monitor(), retina()]);
        assert_eq!(placement.size, LogicalSize::new(940.0, 720.0));
        assert_eq!(placement.position, Some(PhysicalPosition::new(500, 180)));
        assert!(!placement.maximized);
    }

    #[test]
    fn a_remembered_place_on_a_display_that_is_still_there_is_kept() {
        let saved = NotebookGeometry {
            x: 3000,
            y: 100,
            width: 1000.0,
            height: 800.0,
            maximized: false,
        };
        let placement = place(Some(saved), &[retina(), side_monitor()]);
        assert_eq!(placement.position, Some(PhysicalPosition::new(3000, 100)));
        assert_eq!(placement.size, LogicalSize::new(1000.0, 800.0));
        let zoomed = place(
            Some(NotebookGeometry {
                maximized: true,
                ..saved
            }),
            &[retina(), side_monitor()],
        );
        assert!(zoomed.maximized);
    }

    #[test]
    fn a_display_that_has_gone_sends_the_notebook_back_to_the_main_one() {
        let saved = NotebookGeometry {
            x: 3000,
            y: 100,
            width: 1000.0,
            height: 800.0,
            maximized: true,
        };
        let placement = place(Some(saved), &[retina()]);
        let position = placement.position.expect("placed");
        assert!(position.x < 2880 && position.y < 1800);
        assert_eq!(placement.size, LogicalSize::new(1000.0, 800.0));
        assert!(placement.maximized, "zoom is kept; only the place changes");
    }

    #[test]
    fn a_window_dragged_almost_off_screen_comes_back_and_a_huge_one_shrinks_to_fit() {
        // Only a sliver of the title strip on screen is not enough to grab it by.
        let saved = NotebookGeometry {
            x: 2860,
            y: -10,
            width: 900.0,
            height: 700.0,
            maximized: false,
        };
        let placement = place(Some(saved), &[retina()]);
        assert_ne!(placement.position, Some(PhysicalPosition::new(2860, -10)));
        // Remembered from a larger display: no larger than this one, and never under the minimum.
        let huge = NotebookGeometry {
            x: 0,
            y: 0,
            width: 4000.0,
            height: 3000.0,
            maximized: false,
        };
        let small = Screen {
            width: 1280,
            height: 720,
            scale: 1.0,
            ..retina()
        };
        let placement = place(Some(huge), &[small]);
        assert_eq!(placement.size, LogicalSize::new(1232.0, 672.0));
        let tiny = Screen {
            width: 700,
            height: 500,
            scale: 1.0,
            ..retina()
        };
        let placement = place(Some(huge), &[tiny]);
        assert_eq!(placement.size, LogicalSize::new(700.0, 500.0));
    }

    #[test]
    fn nonsense_in_the_file_is_ignored_rather_than_believed() {
        let directory =
            std::env::temp_dir().join(format!("formiga-notebook-geometry-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        assert_eq!(load(&directory), None);
        std::fs::write(path(&directory), b"{not json").unwrap();
        assert_eq!(load(&directory), None);
        let geometry = NotebookGeometry {
            x: -40,
            y: 30,
            width: 800.0,
            height: 600.0,
            maximized: false,
        };
        save(&directory, geometry).unwrap();
        assert_eq!(load(&directory), Some(geometry));
        let silly = NotebookGeometry {
            width: f64::NAN,
            height: -5.0,
            ..geometry
        };
        assert_eq!(
            place(Some(silly), &[retina()]).size,
            LogicalSize::new(940.0, 720.0)
        );
        let _ = std::fs::remove_dir_all(directory);
    }
}
