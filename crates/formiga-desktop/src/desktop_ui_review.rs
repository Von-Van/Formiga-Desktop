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
use crate::review_sheet::over;
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
    crate::review_sheet::save("FORMIGA_UI_REVIEW_DIR", filename, sheet);
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

/// One strip drawn where `placement` puts it — frame, cells and the hovered cell's label — at its
/// own scale, which need not be the creature's.
fn draw_strip(
    sheet: &mut Canvas,
    atlas: &Canvas,
    items: &[formiga_art::MenuIcon],
    placement: crate::creature_menu::MenuPlacement,
    hovered: usize,
) {
    let layout = MenuLayout::new(items);
    let scale = placement.art_scale as i32;
    let (left, top) = (placement.x as i32, placement.y as i32);
    let frame = UiAtlasRenderer::menu_frame(items.len() as u8).expect("a frame");
    blit(sheet, atlas, frame, left, top, scale, placement.below);
    let body = placement.body_rect();
    for (index, icon) in items.iter().enumerate() {
        let cell = layout.cell(index).expect("a cell");
        blit(
            sheet,
            atlas,
            UiAtlasRenderer::menu_icon(*icon, index == hovered),
            body.x as i32 + cell.x * scale,
            body.y as i32 + cell.y * scale,
            scale,
            false,
        );
    }
    let tab = layout.label_tab(hovered).expect("a tab");
    blit(
        sheet,
        atlas,
        UiAtlasRenderer::menu_label(items[hovered]),
        left + tab.x * scale,
        top + tab.y * scale,
        scale,
        false,
    );
}

/// Where a menu goes over a creature drawn in `clip` and `frame` at `scale`, standing with its
/// centre at `centre_x` and its feet on `ground`, in the sheet's own physical pixels.
fn anchor_over(
    creature: &Creature,
    clip: BodyClip,
    frame: u8,
    (centre_x, ground): (i32, i32),
    scale: i32,
    menu_scale: f32,
    usable: crate::creature_menu::LocalRect,
) -> crate::creature_menu::MenuAnchor {
    let bounds = |clip: BodyClip, frame: u8| {
        let body = CreatureRenderer::render_body_frame(&creature.appearance, clip, frame, false);
        let (_, top, _, bottom) = body.canvas.alpha_bounds().expect("drawn");
        (top as i32, bottom as i32 + 1)
    };
    let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
    let frame_top =
        ground + FramePlacement::for_action(ActionKind::Idle, baseline).origin_y * scale;
    let (top, bottom) = bounds(clip, frame);
    let (settled, _) = bounds(BodyClip::from(ActionKind::Idle), 0);
    crate::creature_menu::MenuAnchor {
        centre_x: centre_x as f32,
        head_top: (frame_top + top * scale) as f32,
        foot_bottom: (frame_top + bottom * scale) as f32,
        ground: ground as f32,
        settled_head_top: (frame_top + settled * scale) as f32,
        art_scale: menu_scale,
        grid: 1.0,
        usable,
        monitor_origin: formiga_core::Point { x: 0.0, y: 0.0 },
        scale_factor: 1.0,
    }
}

/// Two pictures of the open strip, drawn with the desktop's own placement code:
///
/// - `menu-fidgets.png`: one companion through six moments of standing about — the settle, two
///   beats of its rest, a wash, a stretch and a shake — with Snack hovered. The top row places
///   the strip from each frame's own crown, as it used to be placed, and the bottom row as an
///   open menu now does, from the feet. The red ticks are where the strip's top sat when it
///   opened.
/// - `menu-small.png`: a Small companion on a high-density display at its real size in physical
///   pixels, with its strip at the companion's own scale on the left and at the size it is drawn
///   at now on the right, beside a Medium companion's for comparison.
#[test]
fn a_menu_holds_still_over_fidgets_and_is_big_enough_at_small() {
    use crate::creature_menu::{CreatureMenu, LocalRect, place};
    let atlas = UiAtlasRenderer::render();
    let creature = adult(29);
    let items = menu_items(MenuTarget::Member, false);
    let layout = MenuLayout::new(&items);
    let picture = |clip: BodyClip, frame: u8| {
        CreatureRenderer::render_composited_frame(
            &creature.appearance,
            clip,
            frame,
            true,
            false,
            face(),
        )
    };
    let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
    let origin_y = FramePlacement::for_action(ActionKind::Idle, baseline).origin_y;

    let poses = [
        (BodyClip::from(ActionKind::Idle), 0),
        (BodyClip::from(ActionKind::Idle), 2),
        (BodyClip::from(ActionKind::Idle), 4),
        (BodyClip::Gesture(formiga_core::Gesture::Groom), 1),
        (BodyClip::Gesture(formiga_core::Gesture::Stretch), 2),
        (BodyClip::Gesture(formiga_core::Gesture::Shake), 1),
    ];
    let scale = 3;
    let (scene_w, scene_h) = (80 * scale, 100 * scale);
    let ground = 90 * scale;
    let width = scene_w * poses.len() as i32;
    let mut sheet = Canvas::new(width as u32, (scene_h * 2) as u32);
    sheet.fill_rect(0, 0, width, scene_h * 2, DESK);
    let usable = LocalRect {
        x: 0.0,
        y: 0.0,
        width: width as f32,
        height: (scene_h * 2) as f32,
    };
    let mut menu = CreatureMenu::new(7, MenuTarget::Member, false, 1, false);
    let mut opened = [None, None];
    for (index, (clip, frame)) in poses.into_iter().enumerate() {
        for row in 0..2 {
            let left = index as i32 * scene_w;
            let top = row * scene_h;
            let centre = (left + scene_w / 2, top + ground);
            let anchor = anchor_over(&creature, clip, frame, centre, scale, scale as f32, usable);
            let placement = if row == 0 {
                place(&layout, anchor)
            } else {
                menu.attach(Some(anchor));
                menu.placement().expect("placed")
            };
            let opened = *opened[row as usize].get_or_insert(placement.y as i32 - top);
            let body = picture(clip, frame);
            blit(
                &mut sheet,
                &body,
                whole(&body),
                centre.0 - FRAME_SIZE as i32 / 2 * scale,
                centre.1 + origin_y * scale,
                scale,
                false,
            );
            draw_strip(&mut sheet, &atlas, &items, placement, 0);
            // Ticks in the margins at the strip's top when it opened.
            for x in [left, left + scene_w - 4 * scale] {
                sheet.fill_rect(x, top + opened, 4 * scale, scale, GUIDE);
            }
            if row == 1 {
                assert_eq!(
                    placement.y as i32 - top,
                    opened,
                    "the open strip holds still"
                );
            }
        }
    }
    save_review("menu-fidgets.png", &sheet);

    // Small on a Retina display: two physical pixels an art pixel, and half-resolution drawing.
    let factor = 2.0;
    let (scene_w, scene_h) = (260, 220);
    let mut small = Canvas::new((scene_w * 3) as u32, scene_h as u32);
    small.fill_rect(0, 0, scene_w * 3, scene_h, DESK);
    let usable = LocalRect {
        x: 0.0,
        y: 0.0,
        width: (scene_w * 3) as f32,
        height: scene_h as f32,
    };
    let scenes = [
        (2, 2.0),
        (2, crate::gpu::ui_menu_scale(2, factor, 2.0)),
        (3, crate::gpu::ui_menu_scale(3, factor, 1.0)),
    ];
    for (index, (scale, strip)) in scenes.into_iter().enumerate() {
        let left = index as i32 * scene_w;
        let centre = (left + scene_w / 2, scene_h - 8);
        let clip = BodyClip::from(ActionKind::Idle);
        let anchor = anchor_over(&creature, clip, 0, centre, scale, strip, usable);
        let body = picture(clip, 0);
        blit(
            &mut small,
            &body,
            whole(&body),
            centre.0 - FRAME_SIZE as i32 / 2 * scale,
            centre.1 + origin_y * scale,
            scale,
            false,
        );
        draw_strip(&mut small, &atlas, &items, place(&layout, anchor), 0);
    }
    save_review("menu-small.png", &small);
}

/// `village-stroll.png`: a companion strolling from the front of the village up the path to the
/// middle of it, a walking frame a column, at its real footing. The top row changes to its
/// smaller set-back drawing the moment it crosses the line, as it used to, whatever its feet are
/// doing; the bottom row as it does now, on the next planted step. A red tick marks the column
/// it crosses the line in, and a grey dot each frame with a foot just down.
#[test]
fn the_village_size_changes_on_a_planted_step_in_a_stroll() {
    let mut creature = adult(29);
    creature.state.action = ActionKind::Traverse;
    creature.state.facing_right = true;
    creature.state.presence = formiga_core::Presence {
        stepping: true,
        last: Some(formiga_core::PresenceMark {
            x: 0.0,
            depth: 0.0,
            window: None,
        }),
        ..Default::default()
    };
    let scale = 4;
    let columns = 8;
    let (cell_w, cell_h) = (50 * scale, 56 * scale);
    let mut sheet = Canvas::new((cell_w * columns) as u32, (cell_h * 2) as u32);
    sheet.fill_rect(0, 0, cell_w * columns, cell_h * 2, PAPER);
    let line = formiga_core::SET_BACK_DEPTH;
    let mut shown = None;
    let mut crossed = false;
    for column in 0..columns {
        creature.state.depth = line - 0.5 + column as f32 * 0.2;
        creature.state.presence.stride = column as f32;
        let now = formiga_art::shown_set_back(&creature, true, shown);
        shown = Some(now);
        let body = formiga_art::BodyPresentation::for_creature(&creature);
        for (row, set_back) in [creature.state.set_back(), now].into_iter().enumerate() {
            let mut appearance = creature.appearance.clone();
            if set_back {
                appearance.logical_size = formiga_core::set_back_size(appearance.logical_size);
            }
            let picture = CreatureRenderer::render_composited_frame(
                &appearance,
                body.clip,
                body.frame,
                true,
                false,
                face(),
            );
            let baseline = CreatureRenderer::resting_baseline(&appearance, false);
            let origin_y = FramePlacement::for_action(ActionKind::Traverse, baseline).origin_y;
            let (left, top) = (column * cell_w, row as i32 * cell_h);
            let ground = top + 50 * scale;
            blit(
                &mut sheet,
                &picture,
                whole(&picture),
                left + cell_w / 2 - FRAME_SIZE as i32 / 2 * scale,
                ground + origin_y * scale,
                scale,
                false,
            );
            sheet.fill_rect(left, ground, cell_w, scale, MARK);
            if !crossed && creature.state.set_back() {
                sheet.fill_rect(left, top, scale, cell_h, GUIDE);
            }
            if formiga_art::footfall_frame(&creature.appearance, body.frame) {
                sheet.fill_rect(
                    left + cell_w / 2 - scale,
                    ground + 2 * scale,
                    2 * scale,
                    2 * scale,
                    MARK,
                );
            }
        }
        crossed |= creature.state.set_back();
    }
    save_review("village-stroll.png", &sheet);
}
