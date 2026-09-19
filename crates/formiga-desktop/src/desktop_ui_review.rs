//! A review sheet for the on-desktop UI: icon bubbles and the right-click creature menu, drawn
//! over real creature sprites at every display scale so a person can look at the result.
//!
//! The overlay's own placement is arithmetic on a GPU, which the vertex tests in `gpu.rs` already
//! pin down. What they cannot answer is whether it *looks* right: whether the bubble's tail
//! actually reaches the crown of a head rather than floating above it, whether the notch points at
//! the creature, whether the label tab lands under the cell the cursor is on, and whether anything
//! is clipped. This composites the very same sprites with the very same offsets and writes a PNG.
//!
//! ```sh
//! FORMIGA_UI_REVIEW_DIR=/tmp/formiga-ui cargo test -p formiga-desktop --bin formiga \
//!     desktop_ui_review
//! ```
//!
//! The sheet is three bands, 2x at the top then 3x then 4x, marked in the left gutter by that
//! many small squares. Each band holds six scenes, left to right:
//!
//! 1. an adult with a full heart bubble,
//! 2. the same adult with a half-grown snack bubble,
//! 3. a small creature with a question bubble — its menu and bubble hug its lower head,
//! 4. a colony member's menu with "profile" hovered, which leaves the notch in plain sight,
//! 5. a visitor's menu with "stay" hovered, whose tab sits over the notch as the artwork intends,
//! 6. a menu with no room above it, flipped under the creature with its notch pointing up.
//!
//! The first three sit on light paper and the last three on a dark desktop, so both contrasts are
//! checked at once. Two short ticks in each scene's margins mark the crown of that creature's
//! head: the bubble's tail tip belongs one art pixel above that line, never on or below it.

use crate::creature_menu::{MENU_NOTCH_GAP, MENU_RISE, MenuTarget, menu_items};
use formiga_art::{
    BUBBLE_ANCHOR, BUBBLE_CELL, BodyClip, Canvas, CreatureRenderer, ExpressionKind, EyelidPose,
    FRAME_SIZE, FaceRenderState, FramePlacement, GazeDirection, MENU_BODY_HEIGHT, MENU_CELL,
    MENU_NOTCH_HEIGHT, MENU_STRIP_HEIGHT, MenuLayout, Rgba, SpriteRect, UiAtlasRenderer,
};
use formiga_core::{ActionKind, BubbleGrowth, BubbleIcon, Creature, DesktopSnapshot, World};

/// One scene's box, in art pixels. Tall enough for a strip above the head *and* a flipped one
/// hanging under the feet, so no scene ever clips its own menu.
const SCENE: (i32, i32) = (90, 140);
/// The art row a creature's feet stand on inside its scene.
const GROUND: i32 = 86;
/// The gutter that carries each band's scale marker, in art pixels.
const GUTTER: i32 = 6;
const SCENES: usize = 6;
const SCALES: [i32; 3] = [2, 3, 4];

const PAPER: Rgba = Rgba::new(238, 235, 228, 255);
const DESK: Rgba = Rgba::new(34, 32, 42, 255);
const GUIDE: Rgba = Rgba::new(214, 92, 108, 255);
const MARK: Rgba = Rgba::new(96, 106, 130, 255);

fn face() -> FaceRenderState {
    FaceRenderState {
        expression: ExpressionKind::Content,
        eyelids: EyelidPose::Open,
        gaze: GazeDirection::new(0, 0),
    }
}

fn desktop() -> DesktopSnapshot {
    DesktopSnapshot::default()
}

fn adult(seed: u8) -> Creature {
    World::preview_adult([seed; 32], time::OffsetDateTime::UNIX_EPOCH, &desktop())
}

/// Source-over, because `Canvas::set` replaces rather than blends and a bubble's paper is
/// deliberately a little translucent.
fn over(source: Rgba, under: Rgba) -> Rgba {
    let alpha = u32::from(source.a);
    let mix = |s: u8, u: u8| ((u32::from(s) * alpha + u32::from(u) * (255 - alpha)) / 255) as u8;
    Rgba::new(
        mix(source.r, under.r),
        mix(source.g, under.g),
        mix(source.b, under.b),
        under.a.max(source.a),
    )
}

/// Blit part of a canvas at a whole-pixel zoom, optionally upside down — the overlay's own
/// nearest sampling and its own vertical UV flip, in software.
fn blit(
    sheet: &mut Canvas,
    source: &Canvas,
    rect: SpriteRect,
    left: i32,
    top: i32,
    scale: i32,
    flip_vertically: bool,
) {
    for y in 0..rect.height as i32 * scale {
        for x in 0..rect.width as i32 * scale {
            let source_y = if flip_vertically {
                rect.height as i32 - 1 - y / scale
            } else {
                y / scale
            };
            let pixel = source.get(rect.x as i32 + x / scale, rect.y as i32 + source_y);
            if pixel.a > 0 {
                sheet.set(left + x, top + y, over(pixel, sheet.get(left + x, top + y)));
            }
        }
    }
}

fn whole(canvas: &Canvas) -> SpriteRect {
    SpriteRect {
        x: 0,
        y: 0,
        width: canvas.width(),
        height: canvas.height(),
    }
}

/// The creature as the overlay draws it, plus the two rows the UI hangs off: the crown of its
/// head and its lowest drawn pixel, in art pixels from the frame's top edge.
fn creature_frame(creature: &Creature) -> (Canvas, i32, i32) {
    let clip = BodyClip::from(ActionKind::Idle);
    // The silhouette comes from the body frame, exactly as `build_atlas_pixels` measures it.
    let body = CreatureRenderer::render_body_frame(&creature.appearance, clip, 0, false);
    let (_, top, _, bottom) = body
        .canvas
        .alpha_bounds()
        .unwrap_or((0, 0, 0, FRAME_SIZE - 1));
    let picture = CreatureRenderer::render_composited_frame(
        &creature.appearance,
        clip,
        0,
        creature.state.facing_right,
        false,
        face(),
    );
    (picture, top as i32, bottom as i32 + 1)
}

struct Scene<'a> {
    creature: &'a Creature,
    dark: bool,
    bubble: Option<(BubbleIcon, BubbleGrowth)>,
    menu: Option<(MenuTarget, bool, usize, bool)>,
}

/// Draw one scene, using the production offsets for every piece.
fn draw_scene(
    sheet: &mut Canvas,
    atlas: &Canvas,
    scene: &Scene<'_>,
    origin: (i32, i32),
    scale: i32,
) {
    let (origin_x, origin_y) = origin;
    sheet.fill_rect(
        origin_x,
        origin_y,
        SCENE.0 * scale,
        SCENE.1 * scale,
        if scene.dark { DESK } else { PAPER },
    );

    let (picture, silhouette_top, silhouette_bottom) = creature_frame(scene.creature);
    let baseline = CreatureRenderer::resting_baseline(&scene.creature.appearance, false);
    let frame_top = GROUND + FramePlacement::for_action(ActionKind::Idle, baseline).origin_y;
    let head_top = frame_top + silhouette_top;
    let foot_bottom = frame_top + silhouette_bottom;
    let centre = SCENE.0 / 2;
    let art = |x: i32, y: i32| (origin_x + x * scale, origin_y + y * scale);

    // Two short ticks in the margins, marking the crown. The artwork itself is never crossed.
    for x in [0, SCENE.0 - 5] {
        let (tick_x, tick_y) = art(x, head_top);
        sheet.fill_rect(tick_x, tick_y, 5 * scale, scale.max(1), GUIDE);
    }

    let (creature_x, creature_y) = art(centre - FRAME_SIZE as i32 / 2, frame_top);
    blit(
        sheet,
        &picture,
        whole(&picture),
        creature_x,
        creature_y,
        scale,
        false,
    );

    if let Some((icon, growth)) = scene.bubble {
        let rect = UiAtlasRenderer::bubble(icon, growth);
        // The anchor pixel lands on the art pixel directly above the crown.
        let (x, y) = art(
            centre - BUBBLE_ANCHOR.0 as i32,
            head_top - 1 - BUBBLE_ANCHOR.1 as i32,
        );
        blit(sheet, atlas, rect, x, y, scale, false);
    }

    let Some((target, can_stay, hovered, below)) = scene.menu else {
        return;
    };
    let items = menu_items(target, can_stay);
    let layout = MenuLayout::new(&items);
    let strip_x = centre - layout.notch_x();
    let strip_y = if below {
        foot_bottom + MENU_NOTCH_GAP
    } else {
        head_top - MENU_RISE
    };
    let body_y = strip_y + if below { MENU_NOTCH_HEIGHT as i32 } else { 0 };

    let frame = UiAtlasRenderer::menu_frame(items.len() as u8).expect("four cells have a frame");
    let (x, y) = art(strip_x, strip_y);
    blit(sheet, atlas, frame, x, y, scale, below);
    for (index, icon) in items.iter().enumerate() {
        let cell = layout.cell(index).expect("four cells");
        let (x, y) = art(strip_x + cell.x, body_y + cell.y);
        blit(
            sheet,
            atlas,
            UiAtlasRenderer::menu_icon(*icon, index == hovered),
            x,
            y,
            scale,
            false,
        );
    }
    let tab = layout.label_tab(hovered).expect("a hovered cell has a tab");
    let (x, y) = art(strip_x + tab.x, strip_y + tab.y);
    blit(
        sheet,
        atlas,
        UiAtlasRenderer::menu_label(items[hovered]),
        x,
        y,
        scale,
        false,
    );
}

fn save_review(filename: &str, sheet: &Canvas) {
    if let Some(directory) = std::env::var_os("FORMIGA_UI_REVIEW_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let file = std::fs::File::create(directory.join(filename)).unwrap();
        let mut encoder = png::Encoder::new(file, sheet.width(), sheet.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&sheet.rgba_bytes())
            .unwrap();
    }
}

#[test]
fn bubbles_and_menus_sit_on_real_creatures_at_every_display_scale() {
    let atlas = UiAtlasRenderer::render();
    let tall = adult(11);
    let other = adult(29);
    let mut small = adult(47);
    small.appearance.logical_size = tall.appearance.logical_size / 2;

    let scenes = [
        Scene {
            creature: &tall,
            dark: false,
            bubble: Some((BubbleIcon::Heart, BubbleGrowth::Full)),
            menu: None,
        },
        Scene {
            creature: &tall,
            dark: false,
            bubble: Some((BubbleIcon::Snack, BubbleGrowth::Medium)),
            menu: None,
        },
        Scene {
            creature: &small,
            dark: false,
            bubble: Some((BubbleIcon::Question, BubbleGrowth::Full)),
            menu: None,
        },
        Scene {
            creature: &other,
            dark: true,
            bubble: None,
            // The far cell, so the notch is not hidden behind its own label tab.
            menu: Some((MenuTarget::Member, false, 3, false)),
        },
        Scene {
            creature: &other,
            dark: true,
            bubble: None,
            menu: Some((MenuTarget::Guest, true, 2, false)),
        },
        Scene {
            creature: &tall,
            dark: true,
            bubble: None,
            menu: Some((MenuTarget::Member, false, 0, true)),
        },
    ];

    let width = (GUTTER + SCENE.0 * SCENES as i32) * SCALES[SCALES.len() - 1];
    let height: i32 = SCALES.iter().map(|scale| SCENE.1 * scale).sum();
    let mut sheet = Canvas::new(width as u32, height as u32);
    sheet.fill_rect(0, 0, width, height, PAPER);

    let mut band_y = 0;
    for scale in SCALES {
        // Each scene paints its own light or dark ground, so the band only has to clear itself.
        sheet.fill_rect(0, band_y, width, SCENE.1 * scale, PAPER);
        // As many small squares in the gutter as the band's scale.
        for step in 0..scale {
            sheet.fill_rect(
                scale,
                band_y + scale * (2 + step * 4),
                scale * 2,
                scale * 2,
                MARK,
            );
        }
        for (index, scene) in scenes.iter().enumerate() {
            draw_scene(
                &mut sheet,
                &atlas,
                scene,
                ((GUTTER + index as i32 * SCENE.0) * scale, band_y),
                scale,
            );
        }
        band_y += SCENE.1 * scale;
    }

    save_review("desktop-ui.png", &sheet);

    // The sheet is only worth looking at if every piece actually landed on it. A bubble's tail
    // tip belongs one art pixel above the crown, so the crown's own row stays the creature's.
    for scale in SCALES {
        let (picture, silhouette_top, _) = creature_frame(&tall);
        let baseline = CreatureRenderer::resting_baseline(&tall.appearance, false);
        let head_top = GROUND
            + FramePlacement::for_action(ActionKind::Idle, baseline).origin_y
            + silhouette_top;
        let bubble_top = head_top - 1 - BUBBLE_ANCHOR.1 as i32;
        assert!(
            bubble_top >= 0,
            "a bubble at {scale}x is clipped by the top of its scene"
        );
        assert!(
            bubble_top + BUBBLE_CELL.1 as i32 <= head_top,
            "the bubble cell reaches past the crown"
        );
        assert!(
            head_top - MENU_RISE >= 0,
            "a menu at {scale}x does not fit above the head in its scene"
        );
        assert!(picture.alpha_bounds().is_some(), "the creature is blank");
    }
    // Nothing was drawn outside the sheet: the widest scene at the largest scale still fits.
    assert!((GUTTER + SCENE.0 * SCENES as i32) * SCALES[2] <= width);
    let layout = MenuLayout::new(&menu_items(MenuTarget::Guest, true));
    assert!(
        layout.size().0 as i32 <= SCENE.0,
        "a four-cell strip is wider than a scene"
    );
    assert_eq!(
        layout.size().1,
        MENU_STRIP_HEIGHT,
        "the strip is not the height the sheet reserved"
    );
    assert_eq!(MENU_BODY_HEIGHT, 2 * 2 + MENU_CELL);
    // And the sheet is not accidentally blank.
    let painted = sheet
        .pixels()
        .iter()
        .filter(|pixel| **pixel != PAPER && pixel.a > 0)
        .count();
    assert!(
        painted > (width * height) as usize / 10,
        "the review sheet is mostly empty"
    );
}
