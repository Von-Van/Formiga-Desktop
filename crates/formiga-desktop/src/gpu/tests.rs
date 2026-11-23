use super::ui::*;
use super::*;
use formiga_art::TALK_BUBBLE;
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
    // Four creatures, each on its shadow, four dwellings and the two trees that bookend them, the
    // sixteen keepsakes the trees hold between them, one bubble, and the colony's eight belongings.
    assert_eq!(
        INITIAL_VERTEX_CAPACITY,
        4 * 24 + 6 * 6 + formiga_core::TREE_HOOKS * 6 + 6 + 8 * 6
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

/// What one creature's atlases would cost if no two of its frames or faces came out alike: a cell
/// for every one. Nothing can cost more, so this is what the budgets are held to.
fn full_layout_bytes() -> usize {
    let body = total_animation_frames().div_ceil(ATLAS_COLUMNS) * ATLAS_COLUMNS * FRAME_SIZE;
    let face = face_slot_count().div_ceil(FACE_ATLAS_COLUMNS) * FACE_ATLAS_COLUMNS;
    (body * FRAME_SIZE * 4 + face * FACE_FRAME_SIZE * FACE_FRAME_SIZE * 4) as usize
}

/// Every motion and readability choice bakes one atlas, byte for byte the same each time, with a
/// cell for every frame and face. An outline never makes two frames alike or apart, so it costs
/// what the plain atlas costs; reduced motion holds each clip still, so its frames share far
/// fewer cells.
#[test]
fn every_motion_and_readability_choice_bakes_the_same_atlas_every_time() {
    let world = World::new(
        [19; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &formiga_core::DesktopSnapshot::default(),
    );
    let creature = &world.save.creatures[0];
    let choices = [(false, false), (true, false), (false, true), (true, true)];
    let first: Vec<_> = choices
        .iter()
        .map(|&(reduce_motion, outline)| {
            build_atlas_pixels(creature, reduce_motion, outline, None, 1)
        })
        .collect();
    let cost = |atlas: &AtlasPixels| atlas.body_pixels.len() + atlas.face_pixels.len();
    for (choice, atlas) in choices.iter().zip(&first) {
        assert_eq!(atlas.body_cells.len(), total_animation_frames() as usize);
        assert_eq!(atlas.face_cells.len(), face_slot_count() as usize);
        assert_eq!(atlas.face_anchors.len(), total_animation_frames() as usize);
        assert_eq!(atlas.hands.len(), total_animation_frames() as usize);
        assert!(cost(atlas) <= full_layout_bytes(), "{choice:?}");
    }
    assert_eq!(cost(&first[0]), cost(&first[2]));
    assert_eq!(cost(&first[1]), cost(&first[3]));
    assert!(
        cost(&first[1]) < cost(&first[0]) / 2,
        "reduced motion costs {} bytes against {}",
        cost(&first[1]),
        cost(&first[0])
    );
    // Thrown away and baked again, twice over: the same atlas, byte for byte, every time.
    for round in 1..3 {
        for (choice, previous) in choices.iter().zip(&first) {
            let atlas = build_atlas_pixels(creature, choice.0, choice.1, None, 1);
            assert_eq!(
                atlas.body_pixels, previous.body_pixels,
                "{choice:?} {round}"
            );
            assert_eq!(atlas.body_cells, previous.body_cells, "{choice:?} {round}");
            assert_eq!(
                atlas.face_pixels, previous.face_pixels,
                "{choice:?} {round}"
            );
            assert_eq!(atlas.face_cells, previous.face_cells, "{choice:?} {round}");
        }
    }
}

/// Frames and faces that come out alike share a cell, so every one has to be found again where
/// its slot says: each baked frame and every face, drawn afresh, is exactly the cell it points
/// at, with reduced motion and without.
#[test]
fn every_frame_and_face_is_drawn_from_a_cell_that_holds_it() {
    let world = World::new(
        [7; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &formiga_core::DesktopSnapshot::default(),
    );
    let creature = &world.save.creatures[0];
    let cell_of = |pixels: &[u8], width: u32, cell: u16, columns: u32, size: u32| {
        let (x, y) = (
            u32::from(cell) % columns * size,
            u32::from(cell) / columns * size,
        );
        (0..size)
            .flat_map(|row| {
                let start = (((y + row) * width + x) * 4) as usize;
                pixels[start..start + (size * 4) as usize].to_vec()
            })
            .collect::<Vec<u8>>()
    };
    for reduce_motion in [false, true] {
        let atlas = build_atlas_pixels(creature, reduce_motion, false, None, 1);
        for clip in BodyClip::baked() {
            for frame in 0..AnimationSpec::for_clip(clip).frames {
                let drawn = CreatureRenderer::render_dressed_body_frame(
                    &creature.appearance,
                    None,
                    clip,
                    frame,
                    reduce_motion,
                );
                let cell = atlas.body_cells[atlas_slot(clip, frame) as usize];
                assert_eq!(
                    cell_of(
                        &atlas.body_pixels,
                        atlas.body_width,
                        cell,
                        ATLAS_COLUMNS,
                        FRAME_SIZE
                    ),
                    drawn.canvas.rgba_bytes(),
                    "{clip:?} frame {frame}, reduced motion {reduce_motion}"
                );
            }
        }
        for expression in formiga_art::ExpressionKind::ALL {
            for eyelids in formiga_art::EyelidPose::ALL {
                for gaze in (-1..=1)
                    .flat_map(|y| (-1..=1).map(move |x| formiga_art::GazeDirection::new(x, y)))
                {
                    let state = FaceRenderState {
                        expression,
                        eyelids,
                        gaze,
                    };
                    let cell = atlas.face_cells[face_atlas_slot(state) as usize];
                    assert_eq!(
                        cell_of(
                            &atlas.face_pixels,
                            atlas.face_width,
                            cell,
                            FACE_ATLAS_COLUMNS,
                            FACE_FRAME_SIZE
                        ),
                        CreatureRenderer::render_face_frame(&creature.appearance, state)
                            .rgba_bytes(),
                        "{state:?}"
                    );
                }
            }
        }
    }
}

/// With motion reduced each clip bakes one cell, shared by every frame of it, and that cell is the
/// clip's still frame: what it holds there is what the moving clip holds on that frame, the snack
/// at the mouth and the cup tipped to it rather than the first frame of either.
#[test]
fn reduced_motion_bakes_each_clip_as_one_cell_of_its_still_frame() {
    let world = World::new(
        [7; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &formiga_core::DesktopSnapshot::default(),
    );
    let creature = &world.save.creatures[0];
    let reduced = build_atlas_pixels(creature, true, false, None, 1);
    let moving = build_atlas_pixels(creature, false, false, None, 1);
    for clip in BodyClip::baked() {
        let still = atlas_slot(clip, AnimationSpec::still_frame(clip)) as usize;
        for frame in 0..AnimationSpec::for_clip(clip).frames {
            let slot = atlas_slot(clip, frame) as usize;
            assert_eq!(
                reduced.body_cells[slot], reduced.body_cells[still],
                "{clip:?} frame {frame} has a cell of its own under reduced motion"
            );
        }
        assert_eq!(
            reduced.held[still].map(|held| held.thing),
            moving.held[still].map(|held| held.thing),
            "{clip:?} holds still on another frame than its still frame"
        );
    }
    let cells = |atlas: &AtlasPixels| {
        atlas
            .body_cells
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
    };
    eprintln!(
        "reduced motion: {} body cells, {} bytes; moving: {} body cells, {} bytes",
        cells(&reduced),
        reduced.body_pixels.len() + reduced.face_pixels.len(),
        cells(&moving),
        moving.body_pixels.len() + moving.face_pixels.len(),
    );
    assert!(cells(&reduced) <= BodyClip::baked().count());
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

/// The village atlas is one texture: a cell for every house by day, with somebody at home and
/// without, the same again lit after dark, one tree cell both trees come from, and below it the
/// tree leaning either way in the wind and the tents' blown-out pennants. No two lots may sample
/// the same cell, and none may sample off the sheet: the wind costs the texture nothing.
#[test]
fn every_village_cell_including_the_tree_has_its_own_place_on_the_one_atlas() {
    let (cell_u, cell_v) = (
        SHELTER_SIZE as f32 / VILLAGE_ATLAS_WIDTH as f32,
        SHELTER_SIZE as f32 / VILLAGE_ATLAS_HEIGHT as f32,
    );
    let mut cells: Vec<VillageCell> = (0..formiga_art::VILLAGE_HOUSES)
        .flat_map(|slot| {
            [(false, false), (false, true), (true, false), (true, true)].map(|(lit, occupied)| {
                VillageCell::House {
                    slot,
                    lit,
                    occupied,
                }
            })
        })
        .collect();
    cells.push(VillageCell::Tree);
    let wind = [
        VillageCell::TreeInWind {
            leaning_right: true,
        },
        VillageCell::TreeInWind {
            leaning_right: false,
        },
        VillageCell::Pennants,
    ];
    cells.extend(wind);
    let mut seen = std::collections::BTreeSet::new();
    for kind in cells {
        let (u, v) = village_cell(kind);
        assert!(
            (0.0..=1.0 - cell_u).contains(&u) && (0.0..=1.0 - cell_v).contains(&v),
            "{kind:?} samples off the sheet"
        );
        assert!(
            seen.insert(((u / cell_u).round() as u8, (v / cell_v).round() as u8)),
            "{kind:?} shares a cell"
        );
        // By day everything is in the top half, which is all the Home page holds.
        if !matches!(kind, VillageCell::House { lit: true, .. }) && !wind.contains(&kind) {
            assert!(v + cell_v <= 0.5, "{kind:?} is not in the daylit half");
        }
    }
    assert_eq!(seen.len(), formiga_art::VILLAGE_HOUSES * 4 + 4);
    // Every slot's blown-out pennant has its own square inside the pennants' cell.
    let (px, py) = ShelterRenderer::village_cell(VillageCell::Pennants);
    let mut patches = std::collections::BTreeSet::new();
    for slot in 0..formiga_art::VILLAGE_HOUSES {
        for lit in [false, true] {
            let (x, y) = ShelterRenderer::pennant_patch(slot, lit);
            assert!(
                (px..=px + SHELTER_SIZE - formiga_art::PENNANT_PATCH).contains(&x)
                    && (py..=py + SHELTER_SIZE - formiga_art::PENNANT_PATCH).contains(&y),
                "slot {slot}'s pennant falls outside its cell"
            );
            assert!(
                patches.insert((x, y)),
                "slot {slot}'s pennant shares a square"
            );
        }
    }
    // In a gust both trees lean the way the pennants fly: the inward one, read mirrored, from
    // the cell that leans the other way. Still, both are the one tree cell.
    for end in formiga_core::TreeEnd::BOTH {
        assert_eq!(tree_cell(end, false), VillageCell::Tree);
    }
    assert_eq!(
        tree_cell(formiga_core::TreeEnd::Outward, true),
        VillageCell::TreeInWind {
            leaning_right: true
        }
    );
    assert_eq!(
        tree_cell(formiga_core::TreeEnd::Inward, true),
        VillageCell::TreeInWind {
            leaning_right: false
        }
    );
    assert!(
        tree_is_mirrored(formiga_core::TreeEnd::Inward)
            && !tree_is_mirrored(formiga_core::TreeEnd::Outward),
        "the inward bookend is the outward one read the other way round"
    );
    assert_eq!(
        (VILLAGE_ATLAS_WIDTH, VILLAGE_ATLAS_HEIGHT),
        (SHELTER_SIZE * 7, SHELTER_SIZE * 4),
        "twenty-eight cells, one 672x384 texture"
    );
}

/// A colony hangs sixteen of its keepsakes at most, once each, split between the two trees, and
/// every one of them lands inside its own tree's cell — never beside it, never twice.
#[test]
fn the_trees_draw_one_quad_for_each_hung_keepsake_and_keep_it_on_their_own_branches() {
    let mut world = ui_world();
    for found in [0_u8, 1, 8, 16, formiga_core::TRINKET_VARIANTS] {
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
        let hung = formiga_art::hung_trinkets(&world.save.home, &world.save.companion.scrapbook);
        let expected = usize::from(found).min(formiga_core::TREE_HOOKS);
        assert_eq!(hung.len(), expected, "{found} found should hang {expected}");
        let mut variants: Vec<u8> = hung.iter().map(|(variant, ..)| *variant).collect();
        variants.dedup();
        assert_eq!(variants.len(), hung.len(), "something hangs twice");
        let mut per_tree = std::collections::BTreeMap::new();
        for scale in [2.0_f32, 3.0, 4.0] {
            let half = TRINKET_CELL as f32 * scale / 2.0;
            for (variant, end, anchor) in &hung {
                *per_tree.entry(*end).or_insert(0_usize) += 1;
                let (x, y) = hung_trinket_centre(500.0, 400.0, *anchor, scale);
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
                // And on the very pixel of the atlas's tree cell its anchor names: the tree's
                // own smaller cell sits in the middle of the village cell, on its foot.
                let (inset_x, inset_y) = formiga_art::TREE_INSET;
                assert_eq!(
                    (
                        (x - (500.0 - cell / 2.0)) / scale,
                        (y - (400.0 - cell)) / scale
                    ),
                    ((inset_x + anchor.x) as f32, (inset_y + anchor.y) as f32),
                    "variant {variant} at {scale}x is not on its anchor"
                );
            }
        }
        // The first eight hooks are the outward tree's and the next eight the inward one's.
        let on = |end| per_tree.get(&end).copied().unwrap_or(0) / 3;
        assert_eq!(on(formiga_core::TreeEnd::Outward), expected.min(8));
        assert_eq!(
            on(formiga_core::TreeEnd::Inward),
            expected.saturating_sub(8)
        );
    }
    // A save from a catalogue this build does not have keeps its record and hangs nothing.
    world.save.companion.scrapbook = vec![formiga_core::ScrapbookRecord {
        variant: 200,
        first_at: world.save.created_at_utc,
        finder: None,
        finder_name: String::new(),
    }];
    assert!(
        formiga_art::hung_trinkets(&world.save.home, &world.save.companion.scrapbook).is_empty()
    );
}

/// A finer atlas is the same picture at more pixels: the same cells, faces, anchors and
/// silhouettes, every art pixel a square of texture pixels, and each frame in the same place in
/// it, so a frame fills the very quad on the screen it filled before.
#[test]
fn a_finer_atlas_is_the_same_picture_at_more_pixels() {
    let world = World::new(
        [19; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &formiga_core::DesktopSnapshot::default(),
    );
    let creature = &world.save.creatures[0];
    let plain = build_atlas_pixels(creature, false, true, None, 1);
    let fine = build_atlas_pixels(creature, false, true, None, 3);
    assert_eq!((plain.detail, fine.detail), (1, 3));
    assert_eq!(
        (fine.body_width, fine.body_height),
        (plain.body_width * 3, plain.body_height * 3)
    );
    assert_eq!(
        (fine.face_width, fine.face_height),
        (plain.face_width * 3, plain.face_height * 3)
    );
    assert_eq!(fine.body_cells, plain.body_cells);
    assert_eq!(fine.face_cells, plain.face_cells);
    assert_eq!(fine.face_anchors, plain.face_anchors);
    assert_eq!(fine.hands, plain.hands);
    assert_eq!(fine.silhouette, plain.silhouette);
    let pixel = |pixels: &[u8], width: u32, x: u32, y: u32| {
        let at = ((y * width + x) * 4) as usize;
        pixels[at..at + 4].to_vec()
    };
    for y in 0..fine.body_height {
        for x in 0..fine.body_width {
            assert_eq!(
                pixel(&fine.body_pixels, fine.body_width, x, y),
                pixel(&plain.body_pixels, plain.body_width, x / 3, y / 3)
            );
        }
    }
    for cell in [0, 7, 23] {
        assert_eq!(
            cell_uv(
                cell,
                ATLAS_COLUMNS,
                FRAME_SIZE,
                plain.body_width,
                plain.body_height
            ),
            cell_uv(
                cell,
                ATLAS_COLUMNS,
                FRAME_SIZE * 3,
                fine.body_width,
                fine.body_height
            )
        );
        assert_eq!(
            cell_uv(
                cell,
                FACE_ATLAS_COLUMNS,
                FACE_FRAME_SIZE,
                plain.face_width,
                plain.face_height
            ),
            cell_uv(
                cell,
                FACE_ATLAS_COLUMNS,
                FACE_FRAME_SIZE * 3,
                fine.face_width,
                fine.face_height
            )
        );
    }
}

/// A reshaped companion is baked from Formiga Farm's finer painting: the same cells, anchors and
/// silhouettes as at one pixel to the unit, so it stands, moves and is picked up exactly as
/// before, with each frame painted finer in the very place the frame stands. The finer painting
/// may add a tuft or a curl of fur the plain frame has no room for, but never further out than two
/// of its pixels, and leaves nothing of the plain frame out.
#[test]
fn a_sculpted_form_is_baked_finer_where_its_frame_stands() {
    use formiga_core::forms::{Plan, Sculpt};
    let world = World::new(
        [19; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &formiga_core::DesktopSnapshot::default(),
    );
    let mut creature = world.save.creatures[0].clone();
    creature.appearance.sculpt = Some(Sculpt::starter(Plan::CompactQuadruped));
    let plain = build_atlas_pixels(&creature, false, true, None, 1);
    let fine = build_atlas_pixels(&creature, false, true, None, 3);
    assert_eq!(fine.body_width, plain.body_width * 3);
    assert_eq!(fine.face_width, plain.face_width * 3);
    assert_eq!(fine.face_cells, plain.face_cells);
    assert_eq!(fine.face_anchors, plain.face_anchors);
    assert_eq!(fine.silhouette, plain.silhouette);
    // A frame's cell, wherever packing put it, read as the alpha at one of its pixels.
    let alpha = |atlas: &AtlasPixels, slot: usize, size: u32, x: u32, y: u32| {
        let cell = u32::from(atlas.body_cells[slot]);
        let (left, top) = (cell % ATLAS_COLUMNS * size, cell / ATLAS_COLUMNS * size);
        atlas.body_pixels[(((top + y) * atlas.body_width + left + x) * 4 + 3) as usize]
    };
    let mut finer = false;
    for slot in 0..plain.body_cells.len() {
        for y in 0..FRAME_SIZE * 3 {
            for x in 0..FRAME_SIZE * 3 {
                let painted = alpha(&fine, slot, FRAME_SIZE * 3, x, y);
                let (px, py) = (x / 3, y / 3);
                finer |= painted != alpha(&plain, slot, FRAME_SIZE, px, py);
                if painted == 0 {
                    continue;
                }
                let near = (px.saturating_sub(2)..=(px + 2).min(FRAME_SIZE - 1)).any(|nx| {
                    (py.saturating_sub(2)..=(py + 2).min(FRAME_SIZE - 1))
                        .any(|ny| alpha(&plain, slot, FRAME_SIZE, nx, ny) > 0)
                });
                assert!(
                    near,
                    "frame {slot} is painted at ({x}, {y}), away from its frame"
                );
            }
        }
        for py in 0..FRAME_SIZE {
            for px in 0..FRAME_SIZE {
                if alpha(&plain, slot, FRAME_SIZE, px, py) == 0 {
                    continue;
                }
                let kept = (0..9).any(|i| {
                    alpha(&fine, slot, FRAME_SIZE * 3, px * 3 + i % 3, py * 3 + i / 3) > 0
                });
                assert!(kept, "frame {slot} leaves out its pixel ({px}, {py})");
            }
        }
    }
    assert!(finer, "the fine atlas is painted, not enlarged");
}

/// A companion is pixel art at one pixel to the unit, and is baked at one whatever its size on the
/// screen. Only a sculpted form is baked finer, and never finer than its screen shows it.
#[test]
fn only_a_sculpted_form_is_baked_finer_and_never_finer_than_its_screen_shows_it() {
    use formiga_core::forms::{Plan, Sculpt};
    let world = World::new(
        [19; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &formiga_core::DesktopSnapshot::default(),
    );
    let mut appearance = world.save.creatures[0].appearance.clone();
    for display_scale in 1..=4 {
        assert_eq!(baked_detail(&appearance, display_scale, 1), 1);
    }
    appearance.sculpt = Some(Sculpt::starter(Plan::Floater));
    for display_scale in 1..=4 {
        let detail = baked_detail(&appearance, display_scale, 1);
        assert!((1..=u32::from(display_scale)).contains(&detail));
        assert!(detail <= formiga_art::forms::FINEST_DETAIL);
    }
}

/// On a Retina display at an even size the overlay draws at half resolution, so a sculpted form is
/// baked at the overlay's pixels, not the screen's: plain at Small, and twice as fine at Large.
#[test]
fn a_sculpted_form_is_baked_no_finer_than_a_half_resolution_overlay_draws_it() {
    use formiga_core::forms::{Plan, Sculpt};
    let world = World::new(
        [19; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &formiga_core::DesktopSnapshot::default(),
    );
    let mut appearance = world.save.creatures[0].appearance.clone();
    appearance.sculpt = Some(Sculpt::starter(Plan::CompactQuadruped));
    assert_eq!(baked_detail(&appearance, 2, 2), 1);
    assert_eq!(baked_detail(&appearance, 4, 2), 2);
    assert_eq!(baked_detail(&appearance, 3, 1), 3);
    assert_eq!(baked_detail(&appearance, 4, 1), 4);
}

/// The overlay centres a find held up, a watering can and anything picked on the paw each frame
/// draws, so the atlas keeps that paw for every body slot, exactly as the frame reports it, beside
/// the face anchor it keeps the same way.
#[test]
fn the_atlas_keeps_each_frames_paw_for_every_body_slot() {
    let world = World::new(
        [23; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &formiga_core::DesktopSnapshot::default(),
    );
    let creature = &world.save.creatures[0];
    let atlas = build_atlas_pixels(creature, false, false, None, 1);
    assert_eq!(atlas.hands.len(), total_animation_frames() as usize);
    assert_eq!(atlas.hands.len(), atlas.face_anchors.len());
    for clip in BodyClip::baked() {
        for frame in 0..AnimationSpec::for_clip(clip).frames {
            let rendered =
                CreatureRenderer::render_body_frame(&creature.appearance, clip, frame, false);
            let slot = atlas_slot(clip, frame) as usize;
            assert_eq!(atlas.hands[slot], rendered.hands, "{clip:?} frame {frame}");
            assert_eq!(atlas.face_anchors[slot], rendered.face_anchor);
        }
    }
}

/// Through a whole watering beat, ticked as the desktop ticks it, the can is on the paw on every
/// frame and the water is drawn only while the pour has the can tipped over, its quad centred on
/// the tipped can's spout whichever way the companion faces and however large it is drawn.
#[test]
fn the_water_runs_from_the_cans_spout_only_while_the_pour_tips_it() {
    use formiga_art::{BodyPresentation, PropSprite};
    use formiga_core::{Beat, BeatKind, Gesture, VillageProp};
    let world = World::new(
        [23; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &formiga_core::DesktopSnapshot::default(),
    );
    let mut creature = world.save.creatures[0].clone();
    creature.state.action = ActionKind::Idle;
    let hands = (300.0, 400.0);
    let tipped = ColonyObjectRenderer::prop_cell(PropSprite::WateringCanTipped);
    let upright = ColonyObjectRenderer::prop_cell(PropSprite::WateringCan);
    let water = ColonyObjectRenderer::prop_cell(PropSprite::Water);
    let (spout_x, spout_y) = PropSprite::WateringCanTipped
        .spout()
        .expect("the tipped can has a spout");
    let half = COLONY_OBJECT_SIZE as f32 / 2.0;
    for facing_right in [true, false] {
        for display_scale in [1_u8, 2, 3] {
            creature.state.facing_right = facing_right;
            let mut beat = Beat::new(BeatKind::Watering, 5.0);
            beat.held = Some(VillageProp::WateringCan);
            let (mut poured, mut level) = (0, 0);
            while beat.elapsed <= beat.length {
                creature.state.beat = Some(beat);
                let shown = BodyPresentation::for_creature(&creature);
                assert_eq!(shown.clip, BodyClip::Gesture(Gesture::Pour));
                let [can, running] = village_held_cells(
                    VillageProp::WateringCan,
                    shown.clip,
                    shown.frame,
                    hands,
                    shown.facing_right,
                    display_scale,
                );
                let (can_cell, can_centre) = can.expect("the can is always in its paws");
                assert_eq!(can_centre, hands, "the can is on the paw the frame draws");
                // The pour leans into the can on its second and third frames and is level on the
                // first and last.
                let tips = matches!(shown.frame % 4, 1 | 2);
                assert_eq!(running.is_some(), tips, "frame {}", shown.frame);
                assert_eq!(can_cell, if tips { tipped } else { upright });
                if let Some((cell, centre)) = running {
                    poured += 1;
                    assert_eq!(cell, water);
                    let unit = f32::from(display_scale);
                    let across = (spout_x as f32 - half) * unit;
                    let spout = (
                        can_centre.0 + if shown.facing_right { across } else { -across },
                        can_centre.1 + (spout_y as f32 - half) * unit,
                    );
                    close(centre.0, spout.0, "water centre x at the spout");
                    close(centre.1, spout.1, "water centre y at the spout");
                } else {
                    level += 1;
                }
                beat.elapsed += 0.05;
            }
            assert!(poured > 0 && level > 0, "it pours and comes level");
        }
    }
    // Nothing else the village puts in a paw pours, and the can pours on no other clip's frames.
    for clip in BodyClip::baked() {
        for frame in 0..AnimationSpec::for_clip(clip).frames {
            for prop in [
                VillageProp::WateringCan,
                VillageProp::Apple,
                VillageProp::Leaf,
                VillageProp::Produce(formiga_core::GardenKind::Vegetables),
            ] {
                let [held, running] = village_held_cells(prop, clip, frame, hands, true, 2);
                assert!(held.is_some());
                let pours = prop == VillageProp::WateringCan
                    && clip == BodyClip::Gesture(Gesture::Pour)
                    && matches!(frame, 1 | 2);
                assert_eq!(running.is_some(), pours, "{prop:?} {clip:?} frame {frame}");
            }
        }
    }
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
    let atlas = build_atlas_pixels(&world.save.creatures[0], false, false, None, 1);
    let bake_time = started.elapsed();
    let total_bytes = atlas.body_pixels.len() + atlas.face_pixels.len();
    eprintln!("layered atlas: {total_bytes} bytes, baked in {bake_time:?}");
    // 110 action frames — six of them the climb, a stroke with each paw, two the toss's own and
    // the last two a squeeze's own, pinched — and 130 gesture frames — three of them the mantle
    // a climb tops out with, eight the seat and the sled wonders brought in 0.66.0, twenty-eight a
    // companion's own repertoire (a wash, a shake, a sniff, two more ways to sleep and two more
    // ways to walk), sixteen the chores and a pet leaned into, fourteen the nibble, chomp, chew and
    // gulp of a meal, and two a hop's leap through the air — and sixteen more for breathing:
    // eight frames each for sitting on a window and on the doorstep where each had one pose, and
    // four for each way of sleeping where each had two —
    // and fifteen expressions' faces in every eyelid pose
    // and gaze. Until 0.62.0 every one had a cell of its own, ten columns of 48px bodies and
    // twenty-seven of 16px faces to a row, and the face texture ended in eight trinket cells
    // nothing read: 1,824,768 bytes with the temperament poses and their faces. Since 0.62.0 a
    // frame or face that comes out the same as one already baked shares its cell — a held pose,
    // eyes shut whichever way they would be looking — and the trinket cells are gone.
    assert_eq!(total_animation_frames(), 240);
    // 1,244,160 until 0.66.0, when the walk, the run, the greeting and the meal stopped repeating
    // frames they had shared and the texture grew by a row of ten cells, to 1,336,320;
    // 1,244,160 again from 0.66.1, whose scoot along on a leaf sled repeats two of its four
    // frames where pedalling a bike repeated none, which brings it back under that row; and
    // 1,520,640 with the repertoire, three rows more, 1,612,800 with the chores and the pet, and
    // 1,704,960 with the four ways a meal is eaten (fourteen frames), and still that with the
    // climb's two more frames, the mantle's three and the toss's two, which fit in cells the last
    // row had spare, and with the leap's two, which take the two cells being carried gave up when
    // it stopped bobbing and came to repeat its frames; 1,797,120 with the breathing, one row
    // more (a seated breath's second loop is its first but for the tail), and still that with a
    // squeeze's two pinched frames, which fill cells the last row had spare; and 1,889,280 once
    // the marks over a greeting, a game with a friend and a cheer drift and fade across their
    // frames, so fewer of those frames come out alike: one row more again.
    assert_eq!(total_bytes, 1_889_280);
    assert_eq!(full_layout_bytes(), 2_626_560);
    // Tripled in 0.58.0 so the pose vocabulary has somewhere to grow: the budget is what
    // stops a creature costing more than a creature should, not what stops it having poses.
    assert!(full_layout_bytes() <= 4_500_000);
    // A full colony of six like this one: 9,676,800 bytes. Even with no two frames alike it
    // would be 13,547,520, which is why the ceiling was raised from 10 MiB to 15 MiB in 0.62.0,
    // and to 16 MiB in 0.68.0, when the animation pass's new clips (climb, mantle, toss, leap,
    // squeeze, breathing) put the no-two-frames-alike colony just past 15 MiB.
    assert!(
        full_layout_bytes() * formiga_core::MAX_COLONY_CREATURES < 16 * 1024 * 1024,
        "a full colony's atlases could exceed 16 MiB"
    );
    assert!(total_bytes < atlas.body_pixels.len() * 3);
    // The optional outline is baked into the same atlas: no extra texture, no extra frame,
    // and the same bytes. It touches only pixels the creature itself does not occupy.
    let outlined = build_atlas_pixels(&world.save.creatures[0], false, true, None, 1);
    assert_eq!(
        outlined.body_pixels.len() + outlined.face_pixels.len(),
        total_bytes
    );
    assert_eq!(outlined.body_cells, atlas.body_cells);
    assert_eq!(outlined.face_anchors, atlas.face_anchors);
    assert_eq!(outlined.hands, atlas.hands);
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
    // A toss has two frames of its own, after every other action's.
    assert_eq!(
        atlas_slot(ActionKind::Tossed, 0),
        atlas_slot(ActionKind::PresentDiscovery, 3) + 1
    );
    // A squeeze has two frames of its own, after every other action's. Appended clips from other
    // changes move this and the gesture offset below by their frame counts and nothing else.
    assert_eq!(
        atlas_slot(ActionKind::SqueezeWindow, 0),
        atlas_slot(ActionKind::Tossed, 1) + 1
    );
    // Gestures follow the action clips, and every baked frame owns exactly one slot.
    assert_eq!(atlas_slot(formiga_core::Gesture::Cheer, 0), 110);
    let mut slots = BTreeSet::new();
    for clip in BodyClip::baked() {
        for frame in 0..AnimationSpec::for_clip(clip).frames {
            let slot = atlas_slot(clip, frame);
            assert!(slot < total_animation_frames(), "{clip:?} {frame}");
            assert!(slots.insert(slot), "{clip:?} {frame} shares slot {slot}");
        }
    }
    assert_eq!(slots.len(), total_animation_frames() as usize);
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
    let atlas = build_atlas_pixels(creature, false, false, None, 1);
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
    let adult_top = build_atlas_pixels(&adult, false, false, None, 1).silhouette[slot].0;
    let mini_top = build_atlas_pixels(&mini, false, false, None, 1).silhouette[slot].0;
    assert!(
        mini_top > adult_top,
        "a smaller creature's crown is further down its frame ({mini_top} vs {adult_top})"
    );
    // Both bubbles sit the same distance above their own crown, so neither floats.
    let size = (TALK_BUBBLE.0 as f32, TALK_BUBBLE.1 as f32);
    for (name, top) in [("adult", adult_top), ("mini", mini_top)] {
        let scale = 3.0;
        let head_top = 400.0 + f32::from(top) * scale;
        let y = talk_origin(700.0, (head_top, head_top + 120.0), scale, DRAWABLE, size).y;
        assert_eq!(
            y + size.1 * scale,
            head_top - scale,
            "{name}: the tail's tip belongs one art pixel above the crown"
        );
    }
}

/// A bubble hangs off the highest crown its clip reaches, not off each frame's own, so over a
/// bouncing greeting or a hop it holds still while the head moves under it, and its tail stays
/// clear of the head at the top of every bounce.
#[test]
fn a_bubble_holds_still_over_a_bobbing_clip() {
    let world = ui_world();
    let atlas = build_atlas_pixels(&world.save.creatures[0], false, false, None, 1);
    let size = (TALK_BUBBLE.0 as f32, TALK_BUBBLE.1 as f32);
    let scale = 3.0;
    let frame_top = 300.0;
    for clip in [
        BodyClip::Action(ActionKind::Greet),
        BodyClip::Action(ActionKind::Idle),
        BodyClip::Gesture(formiga_core::Gesture::Cheer),
    ] {
        let tops: Vec<u8> = (0..AnimationSpec::for_clip(clip).frames)
            .map(|frame| atlas.silhouette[atlas_slot(clip, frame) as usize].0)
            .collect();
        let crown = clip_crown(&atlas.silhouette, clip).expect("baked");
        assert_eq!(crown, *tops.iter().min().unwrap(), "{clip:?}");
        let placed: Vec<f32> = tops
            .iter()
            .map(|top| {
                let foot_bottom = frame_top + 47.0 * scale;
                let head_top = frame_top + f32::from(crown) * scale;
                let y = talk_origin(700.0, (head_top, foot_bottom), scale, DRAWABLE, size).y;
                // The tail's tip is above this frame's own crown too.
                assert!(y + size.1 * scale < frame_top + f32::from(*top) * scale);
                y
            })
            .collect();
        assert!(placed.windows(2).all(|pair| pair[0] == pair[1]), "{clip:?}");
    }
    // The greeting does bob, which is why this matters.
    let greet: Vec<u8> = (0..4)
        .map(|frame| atlas.silhouette[atlas_slot(ActionKind::Greet, frame) as usize].0)
        .collect();
    assert!(greet.iter().any(|top| *top != greet[0]), "{greet:?}");
}

#[test]
fn a_bubble_is_centred_on_the_head_with_its_tail_a_pixel_clear_of_it() {
    let frame = TalkSheet::frame(
        formiga_core::talk::TalkStyle::Chat,
        formiga_core::BubbleGrowth::Full,
    );
    let size = (
        (frame.width / TALK_DETAIL) as f32,
        (frame.height / TALK_DETAIL) as f32,
    );
    for scale in [2.0_f32, 3.0, 4.0] {
        let TalkPlacement { x, y, below } =
            talk_origin(700.0, (400.0, 520.0), scale, DRAWABLE, size);
        assert!(!below);
        // Centred: the tail's column is the creature's own centre column.
        assert_eq!(x + (size.0 / 2.0).floor() * scale, 700.0);
        // Exactly one art pixel of daylight between the tip and the crown.
        assert_eq!(400.0 - (y + size.1 * scale), scale, "one art pixel of gap");
        let quad = talk_quad(frame, x, y, scale, false, 1.0, DRAWABLE);
        let (left, top, right, bottom) = quad_pixels(&quad);
        close(right - left, size.0 * scale, "bubble width");
        close(bottom - top, size.1 * scale, "bubble height");
        close(left, x, "bubble left");
        close(top, y, "bubble top");
    }
}

#[test]
fn a_bubble_at_the_top_of_a_display_hangs_under_its_creature_tail_up() {
    let scale = 4.0;
    let size = (TALK_BUBBLE.0 as f32, TALK_BUBBLE.1 as f32);
    let height = size.1 * scale;
    // A creature with its head at the very top of the display: no room above, so the bubble
    // goes one art pixel under its lowest drawn row, turned over so the tail points up at it,
    // rather than pressed against the top edge over its face.
    let (head_top, foot_bottom) = (6.0, 6.0 + 40.0 * scale);
    let placement = talk_origin(700.0, (head_top, foot_bottom), scale, DRAWABLE, size);
    assert!(placement.below);
    assert_eq!(placement.y, foot_bottom + scale, "one art pixel of gap");
    assert!(placement.y + height <= DRAWABLE.height as f32);
    // Its picture goes on the body under the tail, as far from the tail as it sits upright.
    let upright = TalkPlacement {
        below: false,
        ..placement
    };
    let (_, up) = upright.icon_at();
    let (_, down) = placement.icon_at();
    assert_eq!(up, TALK_ICON_AT.1 as f32);
    assert_eq!(down + TALK_ICON as f32, size.1 - up);
    // Drawn upside down: the same rows of the sheet, with the top and bottom swapped.
    let frame = TalkSheet::frame(
        formiga_core::talk::TalkStyle::Chat,
        formiga_core::BubbleGrowth::Full,
    );
    let flipped = talk_quad(frame, placement.x, placement.y, scale, true, 1.0, DRAWABLE);
    let straight = talk_quad(frame, placement.x, placement.y, scale, false, 1.0, DRAWABLE);
    assert_eq!(flipped[0].uv[1], straight[5].uv[1]);
    assert_eq!(flipped[5].uv[1], straight[0].uv[1]);
    // With room above, the same creature lower down keeps its bubble over its head.
    let lower = talk_origin(700.0, (400.0, 560.0), scale, DRAWABLE, size);
    assert!(!lower.below);
    // A display with no room either way keeps the bubble on it, against the top.
    let tiny = PhysicalSize::new(1200, 100);
    let cramped = talk_origin(700.0, (6.0, 90.0), scale, tiny, size);
    assert!(!cramped.below);
    assert_eq!(cramped.y, 0.0);
}

#[test]
fn half_resolution_never_throws_away_a_talk_bubbles_finer_pixels() {
    // On a Retina display at display scale 2, an art pixel is two physical pixels and a bubble's
    // texture pixel one: halving would merge every two of them, so the overlay stays at full.
    assert_eq!(render_divisor(true, 2.0, 2), 1);
    assert_eq!(render_divisor(true, 2.0, 4), 2);
    assert_eq!(render_divisor(true, 2.0, 3), 1);
    assert_eq!(render_divisor(true, 1.0, 4), 1);
    assert_eq!(render_divisor(false, 2.0, 4), 1);
}

#[test]
fn a_growing_bubble_stays_on_the_side_its_whole_bubble_takes() {
    let scale = 4.0;
    let full = (TALK_BUBBLE.0 as f32, TALK_BUBBLE.1 as f32);
    let small = TalkSheet::frame(
        formiga_core::talk::TalkStyle::Chat,
        formiga_core::BubbleGrowth::Small,
    );
    let small = (
        small.width as f32 / TALK_DETAIL as f32,
        small.height as f32 / TALK_DETAIL as f32,
    );
    // Near the top: a small first step would fit above the head, but the whole bubble goes
    // under the feet, so every step does.
    let extent = (40.0, 40.0 + 40.0 * scale);
    let whole = talk_origin(700.0, extent, scale, DRAWABLE, full);
    assert!(whole.below);
    assert!(
        !talk_origin(700.0, extent, scale, DRAWABLE, small).below,
        "on its own the small step would go above"
    );
    let step = talk_origin_beside(whole, 700.0, extent, scale, DRAWABLE, small);
    assert!(step.below);
    assert_eq!(step.y, whole.y, "hung from the same row under the feet");
    // With room above, every step goes above, its tail on the same row as the whole bubble's.
    let extent = (400.0, 560.0);
    let whole = talk_origin(700.0, extent, scale, DRAWABLE, full);
    let step = talk_origin_beside(whole, 700.0, extent, scale, DRAWABLE, small);
    assert!(!whole.below && !step.below);
    assert_eq!(step.y + small.1 * scale, whole.y + full.1 * scale);
}

#[test]
fn a_milestone_bubble_on_a_drawable_narrower_than_itself_does_not_panic() {
    assert_eq!(milestone_centre_x(700.0, 120.0, 1440.0), 700.0);
    assert_eq!(milestone_centre_x(10.0, 120.0, 1440.0), 60.0);
    assert_eq!(milestone_centre_x(1435.0, 120.0, 1440.0), 1380.0);
    // A surface recovered with no size, or one narrower than the bubble.
    assert_eq!(milestone_centre_x(0.0, 120.0, 0.0), 60.0);
    assert_eq!(milestone_centre_x(50.0, 120.0, 100.0), 60.0);
}

#[test]
fn a_bubble_at_the_edge_of_a_display_stays_on_it() {
    let scale = 4.0;
    let size = (TALK_BUBBLE.0 as f32, TALK_BUBBLE.1 as f32);
    let width = size.0 * scale;
    let left = talk_origin(2.0, (400.0, 560.0), scale, DRAWABLE, size).x;
    assert_eq!(left, 0.0);
    let right = talk_origin(
        DRAWABLE.width as f32 - 2.0,
        (400.0, 560.0),
        scale,
        DRAWABLE,
        size,
    )
    .x;
    assert_eq!(right, DRAWABLE.width as f32 - width);
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
            ground: head_top + 36.0 * scale,
            settled_head_top: head_top,
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
        creature_id: Some(1),
        items: &items,
        layout: &layout,
        placement,
        hovered,
        unavailable: None,
        side: None,
    };
    // Nothing hovered: the frame and its four cells, and no tab.
    assert_eq!(menu_quads(view(None), DRAWABLE).len(), 6 * 5);
    // An item that could not be done: "not now" under it, in place of any hovered label.
    let not_now = menu_quads(
        MenuView {
            unavailable: Some(0),
            ..view(Some(2))
        },
        DRAWABLE,
    );
    assert_eq!(not_now.len(), 6 * 6);
    let tab = UiAtlasRenderer::menu_unavailable_label();
    let sprite = quad_sprite(&not_now[not_now.len() - 6..]);
    close(sprite.0, tab.x as f32, "not-now sprite x");
    close(sprite.2, tab.width as f32, "not-now sprite width");
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
                creature_id: Some(1),
                items: &items,
                layout: &layout,
                placement,
                hovered: Some(hovered),
                unavailable: None,
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
            creature_id: Some(1),
            items: &items,
            layout: &layout,
            placement,
            hovered: None,
            unavailable: None,
            side: None,
        },
        DRAWABLE,
    );
    let quads = menu_quads(
        MenuView {
            creature_id: Some(1),
            items: &items,
            layout: &layout,
            placement,
            hovered: None,
            unavailable: None,
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
            creature_id: Some(1),
            items: &items,
            layout: &layout,
            placement,
            hovered: Some(0),
            unavailable: None,
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

/// A companion standing back in the village is drawn after every house as far back as it or
/// further, its own house's doorstep included, and before every house nearer the front.
#[test]
fn a_companion_back_in_the_village_is_drawn_between_the_houses() {
    let quad = |depth: f32| VillageQuad {
        depth,
        vertices: [Vertex::zeroed(); 6],
    };
    // Back to front, as `village_vertices` orders them: the trees at the front come last.
    let quads = [
        quad(8.0),
        quad(7.0),
        quad(6.0),
        quad(3.0),
        quad(2.0),
        quad(0.0),
        quad(0.0),
    ];
    assert_eq!(quads_behind(&quads, 4.5), 3, "on the path");
    assert_eq!(
        quads_behind(&quads, 7.0),
        2,
        "on the doorstep of the house set back seven"
    );
    assert_eq!(quads_behind(&quads, 8.0), 1, "on the furthest doorstep");
    assert_eq!(quads_behind(&quads, 0.5), 5, "a step up from the front");
}

/// A sculpted companion baked finely is a tall atlas. On a graphics card whose textures stop
/// short of it, it is baked as finely as the card can hold instead, and the cells, faces and
/// silhouettes it is drawn by stay the same.
#[test]
fn a_fine_atlas_too_big_for_the_graphics_card_is_baked_as_finely_as_it_holds() {
    use formiga_core::forms::{Plan, Sculpt};
    let world = World::new(
        [19; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &formiga_core::DesktopSnapshot::default(),
    );
    let mut creature = world.save.creatures[0].clone();
    creature.appearance.sculpt = Some(Sculpt::starter(Plan::CompactQuadruped));
    let bake = |detail| build_atlas_pixels(&creature, false, true, None, detail);
    let finest = bake_fitting(u32::MAX, 3, bake);
    assert_eq!(finest.detail, 3);
    // A card that holds a little less than the finest bake.
    let largest = finest.body_height.max(finest.body_width) - 1;
    let fitted = bake_fitting(largest, 3, bake);
    assert!(fitted.detail < 3);
    assert!(fitted.fits(largest));
    assert_eq!(fitted.face_cells, finest.face_cells);
    assert_eq!(fitted.silhouette, finest.silhouette);
}

/// The habitat editor draws every red excluded area however many green regions the desktop is
/// cut into; it is the green that gives way when the buffer is full.
#[test]
fn the_habitat_editor_keeps_red_areas_when_green_ones_fill_its_buffer() {
    let green = ZoneVertex {
        position: [0.0, 0.0],
        color: [0.18, 0.72, 0.45, 0.20],
    };
    let red = ZoneVertex {
        position: [0.0, 0.0],
        color: [0.88, 0.30, 0.25, 0.28],
    };
    let kept = keep_excluded_zones(vec![green; 6 * 200], vec![red; 6 * 32]);
    assert_eq!(kept.len(), MAX_ZONE_VERTICES);
    assert_eq!(kept.iter().filter(|v| v.color == red.color).count(), 6 * 32);
    // Red is drawn last, over the green.
    assert!(
        kept[MAX_ZONE_VERTICES - 6 * 32..]
            .iter()
            .all(|v| v.color == red.color)
    );
    // Nothing is cut when it all fits.
    assert_eq!(keep_excluded_zones(vec![green; 12], vec![red; 6]).len(), 18);
}

/// Clicks resolve against the order the overlay draws in: further back in the village first,
/// everyone as deep as each other in colony order, so swapping two residents' depths swaps which
/// one is in front. And whoever is far enough back to be drawn smaller has its mask read smaller.
#[test]
fn clicks_share_the_overlays_order_and_size() {
    let created = time::macros::datetime!(2026-01-01 12:00 UTC);
    let world = World::new([71; 32], created, &formiga_core::DesktopSnapshot::default());
    let mut first = world.save.creatures[0].clone();
    let mut second = first.clone();
    second.id = first.id + 1;
    let order = |a: &Creature, b: &Creature| {
        back_to_front([a, b])
            .into_iter()
            .map(|creature| creature.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(order(&first, &second), [first.id, second.id]);
    first.state.depth = 0.0;
    second.state.depth = 0.5;
    assert_eq!(order(&first, &second), [second.id, first.id]);
    (first.state.depth, second.state.depth) = (second.state.depth, first.state.depth);
    assert_eq!(order(&first, &second), [first.id, second.id]);
    assert!(drawn_set_back(formiga_core::SET_BACK_DEPTH, true));
    assert!(!drawn_set_back(formiga_core::SET_BACK_DEPTH, false));
    assert!(!drawn_set_back(formiga_core::SET_BACK_DEPTH - 0.01, true));
}

/// A menu's strip keeps the companion's own scale where that is big enough, and is never drawn
/// smaller than one and a half points an art pixel: a Small companion on a high-density display
/// gets a strip as big as a Large one's rather than one too small to hit, on whole drawable
/// pixels so it stays crisp. The companion itself is drawn at its own size regardless.
#[test]
fn a_small_companions_menu_is_still_big_enough_to_use() {
    // Small, Medium and Large on a plain display: the companion's own scale.
    for scale in [2, 3, 4] {
        assert_eq!(menu_scale(scale, 1.0, 1.0), f32::from(scale));
    }
    // On a Retina display Small is drawn at half resolution, so the strip goes up to a whole
    // number of drawable pixels: 4, two points an art pixel.
    assert_eq!(menu_scale(2, 2.0, 2.0), 4.0);
    assert_eq!(menu_scale(3, 2.0, 1.0), 3.0);
    assert_eq!(menu_scale(4, 2.0, 2.0), 4.0);
    for (display_scale, factor, grid) in
        [(2, 2.0, 2.0), (2, 1.5, 1.0), (3, 2.0, 1.0), (2, 1.25, 1.0)]
    {
        let scale = menu_scale(display_scale, factor, grid);
        assert!(
            scale / factor >= MENU_MIN_POINTS,
            "{display_scale} at {factor}x"
        );
        assert_eq!(scale % grid, 0.0, "on the drawable's grid");
        let cell_points = formiga_art::MENU_CELL as f32 * scale / factor;
        assert!(cell_points >= 21.0, "{cell_points} points a cell");
    }
}

// -------------------------------------------------------------------------------------
// Contact shadows.
// -------------------------------------------------------------------------------------

/// The Retina drawable `ui_desktop` lays out on, full resolution.
const SHADOW_LAYOUT: PhysicalSize<u32> = PhysicalSize::new(2880, 1800);

/// A quad's pixel rectangle on `SHADOW_LAYOUT`: left, top, right, bottom.
fn shadow_rect(quad: &[Vertex; 6]) -> (f32, f32, f32, f32) {
    let to_x = |value: f32| (value + 1.0) / 2.0 * SHADOW_LAYOUT.width as f32;
    let to_y = |value: f32| (1.0 - value) / 2.0 * SHADOW_LAYOUT.height as f32;
    (
        to_x(quad[0].position[0]),
        to_y(quad[0].position[1]),
        to_x(quad[1].position[0]),
        to_y(quad[5].position[1]),
    )
}

/// What the overlay works out for a companion's shadow, laid out the way `vertices_for` lays
/// out its body: the shadow quad if any, and the frame's left edge, top edge and contact line.
fn drawn_shadow(
    creature: &Creature,
    atlas: &AtlasPixels,
    grounds: &ShadowGrounds<'_>,
    display_scale: u8,
) -> (Option<[Vertex; 6]>, f32, f32, f32) {
    let unit = f32::from(display_scale);
    let monitor = grounds.monitor;
    let drawn = creature
        .state
        .drawn_position(unit / monitor.scale_factor.max(1.0));
    let local_x = (drawn.x - monitor.bounds.x) * monitor.scale_factor;
    let contact_y = (drawn.y - monitor.bounds.y) * monitor.scale_factor;
    let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
    let placement = FramePlacement::for_creature(creature, baseline);
    let frame_top = contact_y + placement.origin_y as f32 * unit;
    let BodyPresentation {
        clip,
        frame,
        facing_right,
    } = BodyPresentation::for_creature(creature);
    let slot = atlas_slot(clip, frame) as usize;
    let quad = grounds
        .spot(
            creature,
            creature.state.position,
            atlas.silhouette[slot].1,
            (local_x, contact_y),
            frame_top,
            display_scale,
        )
        .and_then(|spot| {
            shadow_quad(
                atlas.feet[slot],
                facing_right,
                spot,
                SHADOW_LAYOUT,
                1.0,
                |value| value,
            )
        });
    let frame_left = local_x - FRAME_SIZE as f32 * unit / 2.0;
    (quad, frame_left, frame_top, contact_y)
}

fn standing(creature: &Creature, action: ActionKind, frame: u8, x: f32, y: f32) -> Creature {
    let mut creature = creature.clone();
    creature.state.action = action;
    creature.state.action_elapsed = 0.0;
    creature.state.attention = None;
    creature.state.beat = None;
    creature.state.play = None;
    creature.state.flourish = None;
    creature.state.position = Point { x, y };
    creature.state.depth = 0.0;
    // Far enough into the clip to show `frame`.
    let spec = AnimationSpec::for_action(action);
    creature.state.action_elapsed = f32::from(frame) / f32::from(spec.fps.max(1));
    creature
}

/// Every companion on its feet stands on exactly one shadow, whatever it is doing on the
/// ground, and nobody being carried has one.
#[test]
fn every_companion_on_its_feet_has_one_shadow_and_none_while_carried() {
    let world = ui_world();
    let monitor = &ui_desktop().monitors[0];
    let settings = &world.save.settings;
    let grounds = ShadowGrounds {
        monitor,
        render_divisor: 1,
        windows: &[],
        settings,
        village: None,
    };
    let floor = 856.0;
    for creature in &world.save.creatures {
        let atlas = build_atlas_pixels(creature, false, false, None, 1);
        for action in [
            ActionKind::Idle,
            ActionKind::Traverse,
            ActionKind::Sprint,
            ActionKind::Sleep,
            ActionKind::Eat,
            ActionKind::Greet,
            ActionKind::Landing,
            ActionKind::PetReaction,
        ] {
            for frame in 0..AnimationSpec::for_action(action).frames {
                let on_feet = standing(creature, action, frame, 600.0, floor);
                let (quad, ..) = drawn_shadow(&on_feet, &atlas, &grounds, 3);
                assert!(quad.is_some(), "{action:?} frame {frame} has no shadow");
            }
        }
        let carried = standing(creature, ActionKind::Dragged, 0, 600.0, 400.0);
        assert!(
            drawn_shadow(&carried, &atlas, &grounds, 3).0.is_none(),
            "a companion being carried casts no shadow"
        );
    }
}

/// A shadow is never wider than the frame it is under draws, either way round, at any frame of
/// any clip, and never reaches up past the feet: its top row is the feet's own bottom row, on the
/// line they stand on, and a frame that lifts its feet off that line keeps it there, smaller.
#[test]
fn a_shadow_stays_within_the_silhouette_and_never_climbs_above_the_feet() {
    let world = ui_world();
    let creature = &world.save.creatures[0];
    let atlas = build_atlas_pixels(creature, false, false, None, 1);
    let unit = 3.0;
    let (frame_left, contact_y) = (600.0, 900.0);
    let baseline = CreatureRenderer::resting_baseline(&creature.appearance, false);
    let contact_row = FRAME_SIZE as i32 - baseline as i32;
    let mut drawn = 0;
    for clip in BodyClip::baked() {
        for frame in 0..AnimationSpec::for_clip(clip).frames {
            let slot = atlas_slot(clip, frame) as usize;
            let body =
                CreatureRenderer::render_body_frame(&creature.appearance, clip, frame, false)
                    .canvas;
            let (left, _, right, bottom) = body.alpha_bounds().expect("the frame draws something");
            let lift = (contact_row - (bottom as i32 + 1)).max(0) as f32;
            for facing_right in [true, false] {
                let spot = ShadowSpot {
                    frame_centre_x: frame_left + FRAME_SIZE as f32 * unit / 2.0,
                    ground_y: contact_y,
                    lift,
                    unit,
                    ground: ShadowGround::Desktop,
                };
                let Some(quad) = shadow_quad(
                    atlas.feet[slot],
                    facing_right,
                    spot,
                    SHADOW_LAYOUT,
                    1.0,
                    |value| value,
                ) else {
                    continue;
                };
                drawn += 1;
                let (from, to) = if facing_right {
                    (left, right + 1)
                } else {
                    (FRAME_SIZE - right - 1, FRAME_SIZE - left)
                };
                let (shadow_left, shadow_top, shadow_right, _) = shadow_rect(&quad);
                let what = format!("{clip:?} frame {frame} facing right {facing_right}");
                assert!(
                    shadow_left >= frame_left + from as f32 * unit - 0.01
                        && shadow_right <= frame_left + to as f32 * unit + 0.01,
                    "{what}: the shadow is wider than the silhouette"
                );
                close(shadow_top, contact_y - unit, &what);
            }
        }
    }
    assert!(
        drawn > total_animation_frames() as usize,
        "hardly any frame had a shadow"
    );
}

/// Thrown, a companion's shadow stays on whatever it would come down on — the ledge under it,
/// or the floor where the ledge is covered or not under it at all — and shrinks the higher it
/// is, until it is gone.
#[test]
fn in_the_air_a_shadow_stays_on_the_ground_below_and_shrinks_with_height() {
    let world = ui_world();
    let monitor = &ui_desktop().monitors[0];
    let ledge = DesktopRect {
        x: 400.0,
        y: 600.0,
        width: 400.0,
        height: 200.0,
    };
    let windows = [window(1, 1, ledge, None)];
    let grounds = ShadowGrounds {
        monitor,
        render_divisor: 1,
        windows: &windows,
        settings: &world.save.settings,
        village: None,
    };
    let creature = &world.save.creatures[0];
    let atlas = build_atlas_pixels(creature, false, false, None, 1);
    let unit = 3.0;
    let ledge_line = ledge.y * monitor.scale_factor;
    let mut last_width = f32::INFINITY;
    let mut gone = false;
    for rise in (0..=60).map(|step| step as f32 * 4.0) {
        let thrown = standing(creature, ActionKind::Tossed, 0, 600.0, ledge.y - rise);
        match drawn_shadow(&thrown, &atlas, &grounds, 3).0 {
            Some(quad) => {
                assert!(!gone, "a shadow came back higher up, at {rise} points");
                let (left, top, right, _) = shadow_rect(&quad);
                close(top, ledge_line - unit, "on the ledge below");
                assert!(right - left <= last_width + 0.01, "it grew going up");
                last_width = right - left;
            }
            None => gone = true,
        }
    }
    assert!(gone, "a companion high above the ledge still had a shadow");

    // Off the end of the ledge, the floor is what is below.
    let floor_line = 856.0 * monitor.scale_factor;
    let beside = standing(creature, ActionKind::Tossed, 0, 300.0, 820.0);
    let (quad, ..) = drawn_shadow(&beside, &atlas, &grounds, 3);
    close(
        shadow_rect(&quad.expect("low over the floor")).1,
        floor_line - unit,
        "on the floor",
    );

    // A ledge behind another window is not somewhere to come down.
    let covered = [
        window(2, 0, DesktopRect { y: 500.0, ..ledge }, None),
        window(1, 1, ledge, None),
    ];
    let behind = ShadowGrounds {
        windows: &covered,
        ..grounds
    };
    let above = standing(creature, ActionKind::Tossed, 0, 600.0, 495.0);
    let (quad, ..) = drawn_shadow(&above, &atlas, &behind, 3);
    close(
        shadow_rect(&quad.expect("just over the front window")).1,
        500.0 * monitor.scale_factor - unit,
        "on the front window, not the one behind it",
    );

    // Hanging by its hands from something low, the shadow is on the floor under its feet, and
    // smaller than standing there would make it.
    let hanging = standing(creature, ActionKind::Dangle, 0, 300.0, 780.0);
    let quad = drawn_shadow(&hanging, &atlas, &grounds, 3)
        .0
        .expect("hanging a little off the floor");
    let standing_there = standing(creature, ActionKind::Idle, 0, 300.0, 856.0);
    let full = drawn_shadow(&standing_there, &atlas, &grounds, 3)
        .0
        .expect("standing on the floor");
    let ((left, top, right, _), (full_left, _, full_right, _)) =
        (shadow_rect(&quad), shadow_rect(&full));
    close(top, floor_line - unit, "under a hanging companion");
    assert!(right - left < full_right - full_left);
}

/// On the village's grass a shadow is drawn as the ground shades itself: three rows, all of
/// them on the grass behind the line the feet stand on.
#[test]
fn on_the_village_grass_a_shadow_lies_back_across_it() {
    let world = ui_world();
    let monitor = &ui_desktop().monitors[0];
    let floor_line = 856.0 * monitor.scale_factor;
    let grounds = ShadowGrounds {
        monitor,
        render_divisor: 1,
        windows: &[],
        settings: &world.save.settings,
        village: Some(VillageStrip {
            left: 1000.0,
            right: 1600.0,
            line_y: floor_line,
        }),
    };
    let creature = &world.save.creatures[0];
    let atlas = build_atlas_pixels(creature, false, false, None, 1);
    let unit = 3.0;
    let at_home = standing(creature, ActionKind::Idle, 0, 650.0, 856.0);
    let (quad, ..) = drawn_shadow(&at_home, &atlas, &grounds, 3);
    let (_, top, _, bottom) = shadow_rect(&quad.expect("a shadow at home"));
    close(top, floor_line - 3.0 * unit, "village shadow top");
    close(bottom, floor_line, "village shadow foot");
    // Out on the desktop beside the village, it is the desktop's two rows again.
    let out = standing(creature, ActionKind::Idle, 0, 200.0, 856.0);
    let (quad, ..) = drawn_shadow(&out, &atlas, &grounds, 3);
    let (_, top, _, bottom) = shadow_rect(&quad.expect("a shadow out on the desktop"));
    close(bottom - top, 2.0 * unit, "desktop shadow height");
}
