use super::ui::*;
use super::*;
use formiga_core::{ApplicationKey, DisplayKey, MonitorInfo, Point, World};

fn window(
    key: u64,
    z_order: u32,
    bounds: DesktopRect,
    application: Option<ApplicationKey>,
) -> DesktopWindow {
    DesktopWindow {
        key,
        bounds,
        z_order,
        visible: true,
        minimized: false,
        application,
        application_name: None,
    }
}

#[test]
fn multi_creature_presentation_capacity_and_stall_recovery_are_bounded() {
    // Four creatures, four dwellings and the two trees that bookend them, every keepsake
    // hung between the pair, one bubble, and the colony's eight belongings.
    assert_eq!(
        INITIAL_VERTEX_CAPACITY,
        4 * 18 + 6 * 6 + usize::from(formiga_core::TRINKET_VARIANTS) * 6 + 6 + 8 * 6
    );
    assert_eq!(
        expanded_vertex_capacity(INITIAL_VERTEX_CAPACITY, INITIAL_VERTEX_CAPACITY),
        None
    );
    assert_eq!(
        expanded_vertex_capacity(INITIAL_VERTEX_CAPACITY, INITIAL_VERTEX_CAPACITY + 1),
        Some(512)
    );
    assert!(!surface_stalls_require_recovery(2));
    assert!(surface_stalls_require_recovery(3));
}

#[test]
fn selected_window_only_occludes_where_it_is_not_covered() {
    let selected = ApplicationKey::MacBundleId("example.selected".into());
    let rule = ApplicationOcclusionRule {
        application: selected.clone(),
        display_name: "Selected".into(),
        enabled: true,
    };
    let windows = [
        window(
            1,
            0,
            DesktopRect {
                x: 50.0,
                y: 0.0,
                width: 50.0,
                height: 100.0,
            },
            None,
        ),
        window(
            2,
            1,
            DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            Some(selected),
        ),
    ];
    let rects = visible_occlusion_rects(
        DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 200.0,
        },
        &windows,
        &[rule],
        false,
    );
    assert!(
        rects
            .iter()
            .any(|rect| rect.contains(Point { x: 25.0, y: 50.0 }))
    );
    assert!(
        !rects
            .iter()
            .any(|rect| rect.contains(Point { x: 75.0, y: 50.0 }))
    );
}

#[test]
fn disabled_rule_does_not_occlude() {
    let application = ApplicationKey::MacBundleId("example.selected".into());
    let windows = [window(
        1,
        0,
        DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        Some(application.clone()),
    )];
    let rule = ApplicationOcclusionRule {
        application,
        display_name: "Selected".into(),
        enabled: false,
    };
    assert!(visible_occlusion_rects(windows[0].bounds, &windows, &[rule], false).is_empty());
}

#[test]
fn fullscreen_window_occludes_by_default_without_an_application_rule() {
    let monitor = DesktopRect {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };
    let windows = [window(1, 0, monitor, None)];
    assert_eq!(
        visible_occlusion_rects(monitor, &windows, &[], true),
        vec![monitor]
    );
}

#[test]
fn fullscreen_occlusion_can_be_disabled() {
    let monitor = DesktopRect {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };
    let windows = [window(1, 0, monitor, None)];
    assert!(visible_occlusion_rects(monitor, &windows, &[], false).is_empty());
}

#[test]
fn maximized_work_area_window_is_not_treated_as_fullscreen() {
    let monitor = DesktopRect {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };
    let windows = [window(
        1,
        0,
        DesktopRect {
            x: 0.0,
            y: 24.0,
            width: 1920.0,
            height: 1016.0,
        },
        None,
    )];
    assert!(visible_occlusion_rects(monitor, &windows, &[], true).is_empty());
}

#[test]
fn fullscreen_window_only_occludes_its_own_monitor() {
    let left = DesktopRect {
        x: -1920.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };
    let right = DesktopRect {
        x: 0.0,
        y: 0.0,
        width: 2560.0,
        height: 1440.0,
    };
    let windows = [window(1, 0, left, None)];
    assert_eq!(
        visible_occlusion_rects(left, &windows, &[], true),
        vec![left]
    );
    assert!(visible_occlusion_rects(right, &windows, &[], true).is_empty());
}

#[test]
fn a_desktop_full_of_chosen_windows_still_hands_the_shader_a_list_it_can_hold() {
    let monitor = DesktopRect {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };
    let chosen = ApplicationKey::MacBundleId("example.chosen".into());
    // Forty tall windows from the chosen application, with one band lying across all of them,
    // so each contributes two visible pieces and the list would run to eighty.
    let mut windows: Vec<_> = (0..40)
        .map(|index| {
            window(
                index as u64 + 2,
                10 + index,
                DesktopRect {
                    x: index as f32 * 40.0,
                    y: 0.0,
                    width: 30.0,
                    height: 1080.0,
                },
                Some(chosen.clone()),
            )
        })
        .collect();
    windows.push(window(
        1,
        0,
        DesktopRect {
            x: 0.0,
            y: 500.0,
            width: 1920.0,
            height: 80.0,
        },
        None,
    ));
    let rule = ApplicationOcclusionRule {
        application: chosen,
        display_name: "Chosen".into(),
        enabled: true,
    };
    let rects = visible_occlusion_rects(monitor, &windows, &[rule], false);
    assert_eq!(rects.len(), MAX_OCCLUSION_RECTS);
    // The shader reads a fixed array, and the list is cut to exactly what fits in it.
    assert_eq!(OcclusionUniform::zeroed().rects.len(), MAX_OCCLUSION_RECTS);
    // A crowd of narrow windows is still a crowd of narrow windows: it never adds up to
    // covering the display, which is what would stop the colony being drawn at all.
    assert!(!rects_cover(monitor, &rects));
}

#[test]
fn every_motion_and_readability_choice_bakes_the_same_atlas_at_the_same_cost() {
    let world = World::new(
        [19; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &formiga_core::DesktopSnapshot::default(),
    );
    let creature = &world.save.creatures[0];
    let choices = [(false, false), (true, false), (false, true), (true, true)];
    let first: Vec<_> = choices
        .iter()
        .map(|&(reduce_motion, outline)| build_atlas_pixels(creature, reduce_motion, outline))
        .collect();
    for (choice, atlas) in choices.iter().zip(&first) {
        let bytes = atlas.body_pixels.len() + atlas.face_pixels.len();
        assert_eq!(bytes, 1_529_856, "{choice:?} costs {bytes} bytes");
        assert_eq!(atlas.face_anchors.len(), total_animation_frames() as usize);
    }
    // Thrown away and baked again, twice over: the same atlas, byte for byte, every time.
    for round in 1..3 {
        for (choice, previous) in choices.iter().zip(&first) {
            let atlas = build_atlas_pixels(creature, choice.0, choice.1);
            assert_eq!(
                atlas.body_pixels, previous.body_pixels,
                "{choice:?} {round}"
            );
            assert_eq!(
                atlas.face_pixels, previous.face_pixels,
                "{choice:?} {round}"
            );
        }
    }
    // The eight slots the face atlas has always ended with are still there and still the
    // same size, so switching the overlay to the colony sheet changed nothing per creature.
    assert_eq!(trinket_atlas_slot(0), face_slot_count());
    assert_eq!(trinket_atlas_slot(7) + 1, face_slot_count() + 8);
    assert_eq!(trinket_atlas_slot(8), trinket_atlas_slot(0));
}

/// Every variant the catalogue has — including the eight the simulation cannot pick yet —
/// samples a real cell of the colony sheet, on both of its frames.
#[test]
fn every_trinket_in_the_catalogue_samples_its_own_cell_of_the_colony_sheet() {
    let mut seen = std::collections::BTreeSet::new();
    for variant in 0..formiga_core::TRINKET_VARIANTS {
        for body_frame in 0..4_u8 {
            let frame = trinket_frame(body_frame);
            let (x, y, width, height) =
                formiga_art::TrinketAtlasRenderer::cell_rect(variant, frame);
            assert_eq!((width, height), (TRINKET_CELL, TRINKET_CELL));
            assert!(
                x + width <= TRINKET_ATLAS_WIDTH && y + height <= TRINKET_ATLAS_HEIGHT,
                "variant {variant} samples outside the sheet"
            );
            seen.insert((variant, x, y));
        }
    }
    // Sixteen variants, each on a rest cell and a glint cell.
    assert_eq!(seen.len(), usize::from(formiga_core::TRINKET_VARIANTS) * 2);
    // The presentation clip rests for half of itself and twinkles for the other half.
    assert_eq!(trinket_frame(0), formiga_art::TRINKET_FRAME_REST);
    assert_eq!(trinket_frame(1), formiga_art::TRINKET_FRAME_REST);
    assert_eq!(trinket_frame(2), formiga_art::TRINKET_FRAME_GLINT);
    assert_eq!(trinket_frame(3), formiga_art::TRINKET_FRAME_GLINT);
}

/// The village atlas is one texture: a cell for every house by day and another for it lit
/// after dark, and one tree cell both trees come from. No two lots may sample the same cell,
/// and none may sample off the sheet.
#[test]
fn every_village_cell_including_the_tree_has_its_own_place_on_the_one_atlas() {
    let cell = SHELTER_SIZE as f32 / VILLAGE_ATLAS_SIZE as f32;
    let mut cells: Vec<VillageCell> = (0..VILLAGE_HOUSES)
        .flat_map(|slot| [false, true].map(|lit| VillageCell::House { slot, lit }))
        .collect();
    cells.push(VillageCell::Tree);
    let mut seen = std::collections::BTreeSet::new();
    for kind in cells {
        let (u, v) = village_cell(kind);
        assert!(
            (0.0..=1.0 - cell).contains(&u) && (0.0..=1.0 - cell).contains(&v),
            "{kind:?} samples off the sheet"
        );
        assert!(
            seen.insert(((u / cell).round() as u8, (v / cell).round() as u8)),
            "{kind:?} shares a cell"
        );
        // By day everything is in the top half, which is all the Home page holds.
        if !matches!(kind, VillageCell::House { lit: true, .. }) {
            assert!(v + cell <= 0.5, "{kind:?} is not in the daylit half");
        }
    }
    assert_eq!(seen.len(), VILLAGE_HOUSES * 2 + 1);
    assert!(
        tree_is_mirrored(formiga_core::TreeEnd::Inward)
            && !tree_is_mirrored(formiga_core::TreeEnd::Outward),
        "the inward bookend is the outward one read the other way round"
    );
    assert_eq!(
        formiga_art::VILLAGE_ATLAS_SIZE,
        SHELTER_SIZE * 4,
        "sixteen cells, one 256x256 texture"
    );
}

/// A colony draws exactly the keepsakes it has found, once each, split between the two trees,
/// and every one of them lands inside its own tree's cell — never beside it, never twice.
#[test]
fn the_trees_draw_one_quad_for_each_found_keepsake_and_keep_it_on_their_own_branches() {
    let mut world = ui_world();
    for found in [0_u8, 1, 8, formiga_core::TRINKET_VARIANTS] {
        world.save.companion.scrapbook = (0..found)
            .map(|variant| formiga_core::ScrapbookRecord {
                variant,
                first_at: world.save.created_at_utc,
                finder: None,
                finder_name: String::new(),
            })
            .collect();
        // The same find recorded twice is still one thing on one branch.
        if found > 0 {
            let repeat = world.save.companion.scrapbook[0].clone();
            world.save.companion.scrapbook.push(repeat);
        }
        let drawn = found_trinkets(&world.save);
        assert_eq!(
            drawn.len(),
            usize::from(found),
            "{found} found should hang {found} keepsakes"
        );
        let mut per_tree = std::collections::BTreeMap::new();
        for scale in [2.0_f32, 3.0, 4.0] {
            let half = TRINKET_CELL as f32 * scale / 2.0;
            for variant in &drawn {
                let (end, anchor) =
                    formiga_art::trinket_place(*variant).expect("a found keepsake has a place");
                *per_tree.entry(end).or_insert(0_usize) += 1;
                let (x, y) = hung_trinket_centre(500.0, 400.0, anchor, scale);
                // The tree's own cell, measured from its contact point at (500, 400).
                let cell = SHELTER_SIZE as f32 * scale;
                assert!(
                    x - half >= 500.0 - cell / 2.0 && x + half <= 500.0 + cell / 2.0,
                    "variant {variant} at {scale}x hangs off the side of its tree"
                );
                assert!(
                    y - half >= 400.0 - cell && y + half <= 400.0,
                    "variant {variant} at {scale}x hangs off the top or foot of its tree"
                );
            }
        }
        // The everyday finds fill the outward tree first; the conditional ones only ever go
        // on the inward one, so neither can be asked to carry more than its eight anchors.
        let hung = |end| per_tree.get(&end).copied().unwrap_or(0) / 3;
        assert_eq!(
            hung(formiga_core::TreeEnd::Outward),
            usize::from(found.min(formiga_core::TRINKETS_PER_TREE)),
        );
        assert_eq!(
            hung(formiga_core::TreeEnd::Inward),
            usize::from(found.saturating_sub(formiga_core::TRINKETS_PER_TREE)),
        );
    }
    // A save from a catalogue this build does not have keeps its record and hangs nothing.
    world.save.companion.scrapbook = vec![formiga_core::ScrapbookRecord {
        variant: 200,
        first_at: world.save.created_at_utc,
        finder: None,
        finder_name: String::new(),
    }];
    assert!(found_trinkets(&world.save).is_empty());
}

#[test]
fn layered_atlas_matches_the_baked_budget_per_creature() {
    let desktop = formiga_core::DesktopSnapshot {
        monitors: vec![MonitorInfo {
            id: 1,
            display_key: DisplayKey([1; 16]),
            bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 1440.0,
                height: 900.0,
            },
            usable_bounds: DesktopRect {
                x: 0.0,
                y: 24.0,
                width: 1440.0,
                height: 836.0,
            },
            scale_factor: 2.0,
            primary: true,
        }],
        ..Default::default()
    };
    let world = World::new([7; 32], time::OffsetDateTime::UNIX_EPOCH, &desktop);
    let started = std::time::Instant::now();
    let atlas = build_atlas_pixels(&world.save.creatures[0], false, false);
    let bake_time = started.elapsed();
    let total_bytes = atlas.body_pixels.len() + atlas.face_pixels.len();
    eprintln!("layered atlas: {total_bytes} bytes, baked in {bake_time:?}");
    // 92 action frames and 38 gesture frames: ten columns by thirteen rows of 48px bodies,
    // plus the unchanged face atlas. Raised deliberately from 1,437,696 bytes in 0.57.1,
    // where the twelfth row was already full. The habits' stretch took four of the six
    // spare slots in the thirteenth row in 0.59.0, and the two the rest loop grew into in
    // 0.59.5 were the last of them, so neither cost any bytes. The row is now full: the
    // next clip to want a frame has to find it in one that is already baked.
    assert_eq!(total_animation_frames(), 130);
    assert_eq!(total_bytes, 1_529_856);
    // Tripled in 0.58.0 so the pose vocabulary has somewhere to grow: the budget is what
    // stops a creature costing more than a creature should, not what stops it having poses.
    assert!(total_bytes <= 4_500_000, "atlas uses {total_bytes} bytes");
    // A full colony of six: 9,179,136 bytes, held to the same 1.5 MiB a creature the budget
    // for four once set.
    assert!(
        total_bytes * formiga_core::MAX_COLONY_CREATURES < 9_437_184,
        "a full colony's atlases exceed 9 MiB"
    );
    assert!(total_bytes < atlas.body_pixels.len() * 3);
    // The optional outline is baked into the same atlas: no extra texture, no extra frame,
    // and the same bytes. It touches only pixels the creature itself does not occupy.
    let outlined = build_atlas_pixels(&world.save.creatures[0], false, true);
    assert_eq!(
        outlined.body_pixels.len() + outlined.face_pixels.len(),
        total_bytes
    );
    assert_eq!(outlined.face_anchors, atlas.face_anchors);
    let (mut added, mut changed) = (0, 0);
    for (plain, edged) in atlas
        .body_pixels
        .chunks_exact(4)
        .zip(outlined.body_pixels.chunks_exact(4))
    {
        match (plain[3] > 16, plain == edged) {
            (true, same) => changed += usize::from(!same),
            (false, false) => added += 1,
            _ => {}
        }
    }
    assert_eq!(
        changed, 0,
        "an outline never touches the creature's own pixels"
    );
    assert!(added > 0, "an outline does appear around the creature");
    assert_eq!(atlas.face_anchors.len(), total_animation_frames() as usize);
    assert_eq!(
        atlas_slot(ActionKind::Tossed, 2),
        atlas_slot(ActionKind::Dragged, 2)
    );
    assert_eq!(
        atlas_slot(ActionKind::SqueezeWindow, 2),
        atlas_slot(ActionKind::Traverse, 2)
    );
    // Gestures follow the action clips, and every baked frame owns exactly one slot.
    assert_eq!(atlas_slot(formiga_core::Gesture::Cheer, 0), 92);
    let mut slots = BTreeSet::new();
    for clip in BodyClip::baked() {
        for frame in 0..AnimationSpec::for_clip(clip).frames {
            let slot = atlas_slot(clip, frame);
            assert!(slot < total_animation_frames(), "{clip:?} {frame}");
            assert!(slots.insert(slot), "{clip:?} {frame} shares slot {slot}");
        }
    }
    assert_eq!(slots.len(), total_animation_frames() as usize);
    assert_eq!(creature_horizontal_scale(ActionKind::SqueezeWindow), 0.72);
    assert_eq!(creature_horizontal_scale(ActionKind::Traverse), 1.0);
    if !cfg!(debug_assertions) {
        assert!(
            bake_time < std::time::Duration::from_millis(75),
            "release atlas bake took {bake_time:?}"
        );
    }
}

// -------------------------------------------------------------------------------------
// On-desktop UI: icon bubbles, the right-click menu, and whoever is visiting.
// -------------------------------------------------------------------------------------

/// One Retina monitor with a menu bar's worth of inset at the top.
fn ui_desktop() -> formiga_core::DesktopSnapshot {
    formiga_core::DesktopSnapshot {
        monitors: vec![MonitorInfo {
            id: 1,
            display_key: DisplayKey([3; 16]),
            bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 1440.0,
                height: 900.0,
            },
            usable_bounds: DesktopRect {
                x: 0.0,
                y: 24.0,
                width: 1440.0,
                height: 836.0,
            },
            scale_factor: 2.0,
            primary: true,
        }],
        ..Default::default()
    }
}

/// The single place these tests build a guest, so the shape of `Visitor` only has to be
/// followed in one spot.
fn test_guest(creature: Creature) -> formiga_core::Visitor {
    let mut guest =
        formiga_core::Visitor::new(creature, formiga_core::VisitorSource::Wanderer, None);
    guest.on_stage = true;
    guest
}

fn ui_world() -> World {
    World::new([11; 32], time::OffsetDateTime::UNIX_EPOCH, &ui_desktop())
}

const DRAWABLE: PhysicalSize<u32> = PhysicalSize::new(1440, 900);

/// A quad's pixel rectangle, back out of clip space: left, top, right, bottom.
fn quad_pixels(quad: &[Vertex]) -> (f32, f32, f32, f32) {
    let to_x = |value: f32| (value + 1.0) / 2.0 * DRAWABLE.width as f32;
    let to_y = |value: f32| (1.0 - value) / 2.0 * DRAWABLE.height as f32;
    (
        to_x(quad[0].position[0]),
        to_y(quad[0].position[1]),
        to_x(quad[1].position[0]),
        to_y(quad[5].position[1]),
    )
}

/// Positions and UVs make a round trip through clip space, so they are compared at
/// sub-pixel tolerance rather than bit for bit.
#[track_caller]
fn close(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < 0.01,
        "{what}: {actual} is not {expected}"
    );
}

/// The atlas rect a quad samples, back out of its UVs: x, y, width, height.
fn quad_sprite(quad: &[Vertex]) -> (f32, f32, f32, f32) {
    let left = quad[0].uv[0] * UI_ATLAS_WIDTH as f32;
    let right = quad[1].uv[0] * UI_ATLAS_WIDTH as f32;
    let top = quad[0].uv[1] * UI_ATLAS_HEIGHT as f32;
    let bottom = quad[5].uv[1] * UI_ATLAS_HEIGHT as f32;
    (left, top.min(bottom), right - left, (bottom - top).abs())
}

#[test]
fn the_crown_of_a_head_is_the_frame_it_draws_not_the_box_it_is_drawn_in() {
    let world = ui_world();
    let creature = &world.save.creatures[0];
    let atlas = build_atlas_pixels(creature, false, false);
    assert_eq!(atlas.silhouette.len(), total_animation_frames() as usize);
    for clip in BodyClip::baked() {
        for frame in 0..AnimationSpec::for_clip(clip).frames {
            let (top, bottom) = atlas.silhouette[atlas_slot(clip, frame) as usize];
            assert!(top < bottom, "{clip:?} frame {frame} draws nothing");
            assert!(u32::from(bottom) <= FRAME_SIZE);
            // A creature never fills its own frame to the very top edge, which is exactly
            // why a bubble hangs off this row rather than off the frame.
            assert!(
                top > 0,
                "{clip:?} frame {frame} touches the frame's top edge"
            );
        }
    }
    // Mirroring a frame moves no row, so one silhouette serves a creature facing either way.
    let baked = atlas.silhouette[atlas_slot(ActionKind::Idle, 0) as usize];
    for facing_right in [false, true] {
        let mirrored = CreatureRenderer::render_composited_frame(
            &creature.appearance,
            BodyClip::from(ActionKind::Idle),
            0,
            facing_right,
            false,
            FaceRenderState {
                expression: formiga_art::ExpressionKind::Neutral,
                eyelids: formiga_art::EyelidPose::Open,
                gaze: formiga_art::GazeDirection::new(0, 0),
            },
        );
        let bounds = mirrored.alpha_bounds().expect("the creature is drawn");
        // The composited frame carries the face too, which may reach a row above the body
        // on its own; the crown is never lower than the body's own first row.
        assert!(bounds.1 as u8 <= baked.0, "facing_right {facing_right}");
        assert!(bounds.1 > 0);
    }
}

#[test]
fn a_smaller_creature_keeps_its_bubble_as_close_to_its_head_as_an_adult() {
    let world = ui_world();
    let adult = world.save.creatures[0].clone();
    let mut mini = adult.clone();
    mini.appearance.logical_size = adult.appearance.logical_size / 2;
    let slot = atlas_slot(ActionKind::Idle, 0) as usize;
    let adult_top = build_atlas_pixels(&adult, false, false).silhouette[slot].0;
    let mini_top = build_atlas_pixels(&mini, false, false).silhouette[slot].0;
    assert!(
        mini_top > adult_top,
        "a smaller creature's crown is further down its frame ({mini_top} vs {adult_top})"
    );
    // Both bubbles sit the same distance above their own crown, so neither floats.
    for (name, top) in [("adult", adult_top), ("mini", mini_top)] {
        let scale = 3.0;
        let head_top = 400.0 + f32::from(top) * scale;
        let (_, y) = bubble_origin(700.0, head_top, scale, DRAWABLE);
        assert_eq!(
            y + BUBBLE_ANCHOR.1 as f32 * scale,
            head_top - scale,
            "{name}: the anchor pixel belongs one art pixel above the crown"
        );
    }
}

#[test]
fn a_bubble_is_centred_on_the_head_with_its_tail_a_pixel_clear_of_it() {
    for scale in [2.0_f32, 3.0, 4.0] {
        let (x, y) = bubble_origin(700.0, 400.0, scale, DRAWABLE);
        // Centred: the tail column is the creature's own centre column.
        assert_eq!(x + BUBBLE_ANCHOR.0 as f32 * scale, 700.0);
        // The tail's tip is the cell's row 14, so exactly one art pixel of daylight is left
        // between the tip and the crown.
        let tip_bottom = y + 15.0 * scale;
        assert_eq!(
            400.0 - tip_bottom,
            scale,
            "one art pixel of gap at {scale}x"
        );
        let quad = ui_atlas_quad(
            UiAtlasRenderer::bubble(
                formiga_core::BubbleIcon::Heart,
                formiga_core::BubbleGrowth::Full,
            ),
            x,
            y,
            scale,
            false,
            1.0,
            DRAWABLE,
        );
        let (left, top, right, bottom) = quad_pixels(&quad);
        close(right - left, BUBBLE_CELL.0 as f32 * scale, "bubble width");
        close(bottom - top, BUBBLE_CELL.1 as f32 * scale, "bubble height");
        close(left, x, "bubble left");
        close(top, y, "bubble top");
        let sprite = quad_sprite(&quad);
        close(sprite.2, BUBBLE_CELL.0 as f32, "bubble sprite width");
        close(sprite.3, BUBBLE_CELL.1 as f32, "bubble sprite height");
    }
}

#[test]
fn a_bubble_at_the_top_or_the_edge_of_a_display_stays_on_it() {
    let scale = 4.0;
    let width = BUBBLE_CELL.0 as f32 * scale;
    let height = BUBBLE_CELL.1 as f32 * scale;
    // A creature with its head at the very top of the display.
    let (_, y) = bubble_origin(700.0, 6.0, scale, DRAWABLE);
    assert_eq!(y, 0.0);
    assert!(y + height <= DRAWABLE.height as f32);
    // And one pressed against either side.
    let (left, _) = bubble_origin(2.0, 400.0, scale, DRAWABLE);
    assert_eq!(left, 0.0);
    let (right, _) = bubble_origin(DRAWABLE.width as f32 - 2.0, 400.0, scale, DRAWABLE);
    assert_eq!(right, DRAWABLE.width as f32 - width);
    // Every growth step is the same cell, so growing never pushes a clamped bubble off.
    for growth in [
        formiga_core::BubbleGrowth::Small,
        formiga_core::BubbleGrowth::Medium,
        formiga_core::BubbleGrowth::Full,
    ] {
        let rect = UiAtlasRenderer::bubble(formiga_core::BubbleIcon::Snack, growth);
        assert_eq!((rect.width, rect.height), BUBBLE_CELL);
    }
}

fn placed_menu(below: bool) -> (MenuLayout, [MenuIcon; 4], MenuPlacement) {
    use crate::creature_menu::{LocalRect, MenuAnchor, MenuTarget, menu_items, place};
    let items = menu_items(MenuTarget::Member, false);
    let layout = MenuLayout::new(&items);
    let scale = 3.0;
    let usable = LocalRect {
        x: 0.0,
        y: 48.0,
        width: DRAWABLE.width as f32,
        height: DRAWABLE.height as f32 - 48.0,
    };
    let head_top = if below { usable.y + 2.0 } else { 500.0 };
    let placement = place(
        &layout,
        MenuAnchor {
            centre_x: 700.0,
            head_top,
            foot_bottom: head_top + 36.0 * scale,
            art_scale: scale,
            grid: 1.0,
            usable,
            monitor_origin: Point { x: 0.0, y: 0.0 },
            scale_factor: 2.0,
        },
    );
    assert_eq!(placement.below, below);
    (layout, items, placement)
}

#[test]
fn an_open_menu_draws_one_frame_one_quad_per_cell_and_one_label_tab() {
    let (layout, items, placement) = placed_menu(false);
    let view = |hovered| MenuView {
        creature_id: 1,
        items: &items,
        layout: &layout,
        placement,
        hovered,
        side: None,
    };
    // Nothing hovered: the frame and its four cells, and no tab.
    assert_eq!(menu_quads(view(None), DRAWABLE).len(), 6 * 5);
    let hovered = menu_quads(view(Some(2)), DRAWABLE);
    assert_eq!(hovered.len(), 6 * 6);

    // The frame quad is the atlas's own four-cell frame, at the placed top-left.
    let expected = UiAtlasRenderer::menu_frame(4).expect("four cells have a frame");
    let sprite = quad_sprite(&hovered[..6]);
    close(sprite.0, expected.x as f32, "frame sprite x");
    close(sprite.1, expected.y as f32, "frame sprite y");
    close(sprite.2, expected.width as f32, "frame sprite width");
    close(sprite.3, expected.height as f32, "frame sprite height");
    let (left, top, right, bottom) = quad_pixels(&hovered[..6]);
    close(left, placement.x, "frame left");
    close(top, placement.y, "frame top");
    close(
        right - left,
        layout.size().0 as f32 * placement.art_scale,
        "frame width",
    );
    close(
        bottom - top,
        formiga_art::MENU_STRIP_HEIGHT as f32 * placement.art_scale,
        "frame height",
    );

    // Each cell quad lands on its own cell, and only the hovered one is the hovered sprite.
    let body = placement.body_rect();
    for index in 0..4 {
        let quad = &hovered[6 * (index + 1)..6 * (index + 2)];
        let cell = layout.cell(index).expect("four cells");
        let (left, top, right, bottom) = quad_pixels(quad);
        close(
            left,
            body.x + cell.x as f32 * placement.art_scale,
            "cell left",
        );
        close(
            top,
            body.y + cell.y as f32 * placement.art_scale,
            "cell top",
        );
        let side = formiga_art::MENU_CELL as f32 * placement.art_scale;
        close(right - left, side, "cell width");
        close(bottom - top, side, "cell height");
        close(
            quad_sprite(quad).0,
            UiAtlasRenderer::menu_icon(items[index], index == 2).x as f32,
            &format!("cell {index} samples the wrong icon"),
        );
    }
}

#[test]
fn the_label_tab_hangs_under_the_hovered_cell_and_inside_the_strip() {
    let (layout, items, placement) = placed_menu(false);
    for hovered in 0..4 {
        let quads = menu_quads(
            MenuView {
                creature_id: 1,
                items: &items,
                layout: &layout,
                placement,
                hovered: Some(hovered),
                side: None,
            },
            DRAWABLE,
        );
        let tab = &quads[6 * 5..];
        let (left, top, right, bottom) = quad_pixels(tab);
        let sprite = UiAtlasRenderer::menu_label(items[hovered]);
        close(quad_sprite(tab).0, sprite.x as f32, "tab sprite x");
        close(
            right - left,
            sprite.width as f32 * placement.art_scale,
            "tab width",
        );
        close(
            bottom - top,
            formiga_art::LABEL_TAB_HEIGHT as f32 * placement.art_scale,
            "tab height",
        );
        // Below the strip, never overlapping it, and never hanging off its sides.
        let frame = placement.frame_rect();
        assert!(top >= frame.bottom(), "the tab must clear the strip");
        assert!(left >= frame.x - 0.01 && right <= frame.right() + 0.01);
        // And under its own cell.
        let cell = layout.cell(hovered).expect("four cells");
        let cell_centre = placement.body_rect().x
            + (cell.x as f32 + formiga_art::MENU_CELL as f32 / 2.0) * placement.art_scale;
        assert!(
            (left..=right).contains(&cell_centre),
            "the tab for cell {hovered} is not under it"
        );
    }
}

/// The strip of moments beside a menu is its plain tray, one quad per cell, and the hovered
/// cell's label hung level with the menu's own labels; the menu's quads come first, unchanged.
#[test]
fn the_strip_beside_a_menu_draws_its_plain_tray_cells_and_label() {
    let (layout, items, placement) = placed_menu(false);
    let side_layout = MenuLayout::new(&[MenuIcon::Picnic, MenuIcon::Dance, MenuIcon::Nap]);
    let usable = LocalRect {
        x: 0.0,
        y: 0.0,
        width: DRAWABLE.width as f32,
        height: DRAWABLE.height as f32,
    };
    let side = crate::creature_menu::place_beside(&side_layout, placement, usable);
    let alone = menu_quads(
        MenuView {
            creature_id: 1,
            items: &items,
            layout: &layout,
            placement,
            hovered: None,
            side: None,
        },
        DRAWABLE,
    );
    let quads = menu_quads(
        MenuView {
            creature_id: 1,
            items: &items,
            layout: &layout,
            placement,
            hovered: None,
            side: Some(SideView {
                layout: &side_layout,
                placement: side,
                hovered: Some(1),
            }),
        },
        DRAWABLE,
    );
    assert!(
        bytemuck::cast_slice::<Vertex, u8>(&quads[..alone.len()])
            == bytemuck::cast_slice::<Vertex, u8>(&alone),
        "the menu's own quads are unchanged"
    );
    let strip = &quads[alone.len()..];
    // The tray, three cells, and one label.
    assert_eq!(strip.len(), 6 * 5);
    let tray = UiAtlasRenderer::menu_frame_plain(3).expect("three cells have a tray");
    let sprite = quad_sprite(&strip[..6]);
    close(sprite.0, tray.x as f32, "tray sprite x");
    close(sprite.1, tray.y as f32, "tray sprite y");
    let (left, top, _, _) = quad_pixels(&strip[..6]);
    close(left, side.x, "tray left");
    close(top, side.y, "tray top");
    let label = UiAtlasRenderer::menu_label(MenuIcon::Dance);
    close(
        quad_sprite(&strip[6 * 4..]).0,
        label.x as f32,
        "label sprite x",
    );
    let (_, label_top, _, _) = quad_pixels(&strip[6 * 4..]);
    close(label_top, side.label_y, "label hangs level with the menu's");
}

#[test]
fn a_menu_under_a_creature_flips_only_its_frame() {
    let (layout, items, placement) = placed_menu(true);
    let quads = menu_quads(
        MenuView {
            creature_id: 1,
            items: &items,
            layout: &layout,
            placement,
            hovered: Some(0),
            side: None,
        },
        DRAWABLE,
    );
    // The frame samples the same sprite upside down, which points the notch up instead.
    let frame = &quads[..6];
    assert!(
        frame[0].uv[1] > frame[5].uv[1],
        "the frame is not flipped, so the notch still points down"
    );
    close(
        quad_sprite(frame).0,
        UiAtlasRenderer::menu_frame(4).expect("frame").x as f32,
        "flipped frame sprite x",
    );
    // The cells and the tab are not flipped, and the cells start a notch further down.
    for index in 1..6 {
        let quad = &quads[6 * index..6 * (index + 1)];
        assert!(quad[0].uv[1] < quad[5].uv[1], "quad {index} is upside down");
    }
    let cells_top = quad_pixels(&quads[6..12]).1;
    let strip_top = quad_pixels(frame).1;
    close(
        cells_top - strip_top,
        (formiga_art::MENU_NOTCH_HEIGHT + 2) as f32 * placement.art_scale,
        "the cells start a notch and a border below the flipped strip",
    );
}

#[test]
fn the_overlay_draws_whoever_is_visiting_and_forgets_it_the_moment_it_leaves() {
    let mut world = ui_world();
    let mut guest = World::preview_adult([42; 32], time::OffsetDateTime::UNIX_EPOCH, &ui_desktop());
    guest.state.surface.monitor_id = 1;
    guest.state.arrival_delay_secs = 0.0;
    let guest_id = guest.id;
    world.save.visitors.guest = Some(test_guest(guest));
    let drawn = |save: &SaveFile| -> Vec<CreatureId> {
        drawn_on_monitor(save, 1, false)
            .iter()
            .map(|creature| creature.id)
            .collect()
    };
    let with_guest = drawn(&world.save);
    assert!(
        with_guest.contains(&guest_id),
        "a guest on stage is drawn like anyone else"
    );
    assert_eq!(with_guest.last(), Some(&guest_id), "and drawn last, on top");

    // The sprite cache keeps exactly whoever is on that list, so stepping back inside is
    // enough to evict the guest's atlas — this is the overlay's own `retain`, verbatim.
    let mut sprites: BTreeSet<CreatureId> = with_guest.iter().copied().collect();
    world
        .save
        .visitors
        .guest
        .as_mut()
        .expect("a guest")
        .on_stage = false;
    let without_guest = drawn(&world.save);
    assert!(!without_guest.contains(&guest_id));
    sprites.retain(|id| without_guest.contains(id));
    assert!(
        !sprites.contains(&guest_id),
        "the guest's atlas is released"
    );
    assert_eq!(sprites.len(), without_guest.len());

    // And leaving for good is the same again.
    world.save.visitors.guest = None;
    assert_eq!(drawn(&world.save), without_guest);
    // A guest is never mistaken for a colony member.
    assert!(!world.save.creatures.iter().any(|c| c.id == guest_id));
}

#[test]
fn a_fully_covered_monitor_drops_the_guest_too_unless_it_is_being_carried() {
    let mut world = ui_world();
    let mut guest = World::preview_adult([43; 32], time::OffsetDateTime::UNIX_EPOCH, &ui_desktop());
    guest.state.surface.monitor_id = 1;
    guest.state.arrival_delay_secs = 0.0;
    let guest_id = guest.id;
    world.save.visitors.guest = Some(test_guest(guest));
    assert!(drawn_on_monitor(&world.save, 1, true).is_empty());
    world
        .save
        .visitors
        .guest
        .as_mut()
        .expect("a guest")
        .creature
        .state
        .action = ActionKind::Dragged;
    assert_eq!(
        drawn_on_monitor(&world.save, 1, true)
            .iter()
            .map(|c| c.id)
            .collect::<Vec<_>>(),
        vec![guest_id]
    );
}

/// The rope a friend tows a sleeper on reaches from the hand to the sleeper without a gap,
/// hangs a little below the straight line between them, sits on the art-pixel grid, and has
/// its shade under every pixel of it.
#[test]
fn a_tow_rope_hangs_between_its_ends_without_a_gap() {
    let px = 2.0;
    for (from, to) in [
        ((400.0, 700.0), (310.0, 730.0)),
        ((100.0, 500.0), (190.0, 510.0)),
        ((50.0, 50.0), (51.0, 52.0)),
    ] {
        let pixels = super::rope_pixels(from, to, px);
        let rope: Vec<(f32, f32)> = pixels
            .iter()
            .filter(|(_, shade)| !shade)
            .map(|(at, _)| *at)
            .collect();
        let shade: Vec<(f32, f32)> = pixels
            .iter()
            .filter(|(_, shade)| *shade)
            .map(|(at, _)| *at)
            .collect();
        assert_eq!(rope.len(), shade.len());
        for ((x, y), under) in rope.iter().zip(&shade) {
            assert_eq!(*under, (*x, y + px), "the shade sits under the rope");
            assert_eq!((x % px, y % px), (0.0, 0.0), "off the pixel grid");
        }
        let cell = |(x, y): (f32, f32)| ((x / px) as i32, (y / px) as i32);
        let covers = |point: (f32, f32)| {
            let target = ((point.0 / px).floor() as i32, (point.1 / px).floor() as i32);
            rope.iter().any(|at| cell(*at) == target)
        };
        assert!(covers(from) && covers(to), "the rope stops short of an end");
        for pair in rope.windows(2) {
            let (a, b) = (cell(pair[0]), cell(pair[1]));
            assert!(
                (a.0 - b.0).abs() <= 1 && (a.1 - b.1).abs() <= 1,
                "a gap in the rope between {a:?} and {b:?}"
            );
        }
        if (to.0 - from.0).abs() > 20.0 {
            let middle = rope[rope.len() / 2];
            let straight = (from.1 + to.1) / 2.0;
            assert!(middle.1 > straight, "the rope does not sag");
        }
    }
}
