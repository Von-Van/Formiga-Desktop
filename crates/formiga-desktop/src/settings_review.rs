//! Headless visual review of the real egui pages; no desktop capture or live colony access.
use super::*;
use formiga_core::*;
use std::collections::HashMap;

fn fixture() -> (SaveFile, Vec<MonitorInfo>) {
    let now = time::macros::datetime!(2026-09-14 12:00 UTC);
    let monitors = vec![MonitorInfo {
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
            height: 826.0,
        },
        scale_factor: 2.0,
        primary: true,
    }];
    let desktop = DesktopSnapshot {
        monitors: monitors.clone(),
        ..Default::default()
    };
    let mut world = World::new([25; 32], now - time::Duration::days(40), &desktop);
    world.tick(now, 0.05, &desktop);
    let c = &mut world.save.creatures[0];
    c.name = "Mallow".into();
    c.memory.times_petted = 42;
    c.memory.window_climbs = 18;
    c.memory.discoveries_found = 7;
    c.memory.descriptor_flags = ProfileDescriptor::Trusting.flag()
        | ProfileDescriptor::Playful.flag()
        | ProfileDescriptor::LovesHighPlaces.flag();
    world.save.companion.onboarding_complete = true;
    // Someone at the door and a full guest book, so the Journal page is reviewed with both.
    let friend = SharedCreatureSeed {
        source_colony_seed: [70; 32],
        source_generation: 1,
        design: Some(CreatureDesign::generated([70; 32], 1, None)),
    };
    world.invite_visitor(friend, now, &desktop).unwrap();
    for index in 0..MAX_GUEST_BOOK_ENTRIES {
        world.save.visitors.guest_book.push(GuestBookEntry {
            visited_at_utc: now - time::Duration::days((MAX_GUEST_BOOK_ENTRIES - index) as i64),
            name: format!("Wanderer {index}"),
            origin: CreatureOrigin {
                design: None,
                source_colony_seed: [index as u8; 32],
                source_generation: index as u8 % 4,
            },
            source: if index % 2 == 0 {
                VisitorSource::Wanderer
            } else {
                VisitorSource::Invited
            },
        });
    }
    // Two favorites kept from the book, one of them the friend visiting now.
    let visiting = world
        .save
        .visitors
        .guest
        .clone()
        .expect("a friend is visiting");
    world
        .save
        .visitors
        .keep_favorite(
            &visiting.creature.name,
            visiting.creature.origin,
            now - time::Duration::days(3),
        )
        .unwrap();
    let wanderer = world.save.visitors.guest_book[2].clone();
    world
        .save
        .visitors
        .keep_favorite(
            &wanderer.name,
            wanderer.origin,
            now - time::Duration::days(9),
        )
        .unwrap();
    world.save.companion.journal.push(JournalEntry {
        at: now,
        creature: None,
        moment: JournalMoment::Visit("Wanderer 0".into()),
    });
    // Every decoration there is, so each place on a house has something to choose from.
    world.save.home.unlocks.decorations = ShelterDecorationKind::ALL.to_vec();
    world.save.objects.objects = ColonyObjectKind::ALL
        .iter()
        .enumerate()
        .map(|(index, kind)| ColonyObject {
            id: index as u64,
            kind: *kind,
            role: kind.default_role(),
            ..Default::default()
        })
        .collect();
    (world.save, monitors)
}

#[test]
fn all_pages_render_with_bounded_resources_and_release_preview_images() {
    let (save, monitors) = fixture();
    let output_dir = std::env::var_os("FORMIGA_UI_REVIEW_DIR").map(std::path::PathBuf::from);
    if let Some(path) = &output_dir {
        std::fs::create_dir_all(path).unwrap();
    }
    // Every combination of theme and text size, at the widest and the narrowest window the app
    // allows — pairing them off would leave the hardest case, dark at the largest text in the
    // smallest window, rendered only by luck.
    let appearances = [
        AppearancePreferences::default(),
        AppearancePreferences {
            theme: ThemeChoice::Dark,
            text_scale: 100,
            sprite_outline: false,
        },
        AppearancePreferences {
            theme: ThemeChoice::Light,
            text_scale: 150,
            sprite_outline: true,
        },
        AppearancePreferences {
            theme: ThemeChoice::Dark,
            text_scale: 150,
            sprite_outline: true,
        },
    ];
    // `ThemeChoice::System` resolves through the platform's own appearance, which reports nothing
    // headlessly and falls back to the light theme, so it is verified natively rather than here.
    for (appearance, (width, height)) in appearances
        .into_iter()
        .flat_map(|appearance| [(appearance, (940, 720)), (appearance, (760, 560))])
    {
        for page in crate::clubhouse::journal::PAGES {
            let context = egui::Context::default();
            configure_style(&context, appearance);
            let mut clubhouse = Clubhouse::default();
            if page == SettingsTab::Studio {
                // The last candidate — the one a fresh page shows — came from a pasted code, so
                // the page is reviewed with the adopt-or-invite choice a shared creature offers.
                let shared = SharedCreatureSeed {
                    source_colony_seed: [17; 32],
                    source_generation: 0,
                    design: Some(CreatureDesign::generated([17; 32], 0, None)),
                };
                for index in 0..4 {
                    let seed = [index + 17; 32];
                    let shared = (index == 3).then_some(shared);
                    clubhouse.studio.push_preview(
                        &context,
                        GenerationPreview {
                            shared,
                            creature: match shared {
                                Some(shared) => World::from_shared_creature(
                                    shared,
                                    save.maximum_seen_utc,
                                    &DesktopSnapshot::default(),
                                )
                                .save
                                .creatures
                                .remove(0),
                                None => World::preview_adult(
                                    seed,
                                    save.maximum_seen_utc,
                                    &DesktopSnapshot::default(),
                                ),
                            },
                            source_seed: seed,
                            similarity: None,
                            summary: "A new companion with fresh memories.".into(),
                        },
                    );
                }
            }
            let mut settings = save.settings.clone();
            let mut tab = page;
            let mut names = BTreeMap::new();
            let mut selected = None;
            let mut error = None;
            let mut confirmation = None;
            let mut bulk = false;
            let mut textures = HashMap::new();
            for frame in 0..3 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width as f32, height as f32),
                    )),
                    time: Some(frame as f64),
                    ..Default::default()
                };
                let mut outcome = SettingsOutcome::default();
                let mut output = context.run_ui(input, |ui| {
                    draw_settings(
                        ui,
                        &mut settings,
                        &mut tab,
                        &mut error,
                        &save.settings,
                        "/example/colony.json",
                        &monitors,
                        &[],
                        false,
                        &UpdateStatus::Idle,
                        false,
                        &save.creatures,
                        &save.relationships,
                        &mut names,
                        &mut selected,
                        &mut clubhouse,
                        &save,
                        &mut confirmation,
                        &mut bulk,
                        &mut outcome,
                    )
                });
                apply_textures(&mut textures, &output.textures_delta);
                output.textures_delta.clear();
                assert!(outcome.applied.is_none());
                assert!(outcome.accept_creature_preview.is_none());
                if frame == 2 {
                    let bytes: usize = clubhouse
                        .texture_ids()
                        .iter()
                        .map(|id| textures[id].0.pixels.len() * 4)
                        .sum();
                    // The Home page is the heaviest: 416 KiB until 0.61.0 drew the houses a
                    // quarter larger and its row of the village grew by 47 KiB, and 463 KiB until
                    // 0.65.0 drew them a fifth larger again and it grew to 672x96, 77 KiB more.
                    assert!(bytes <= 524 * 1024, "UI artwork exceeded budget: {bytes}");
                    if let Some(path) = &output_dir {
                        let jobs = context.tessellate(output.shapes, output.pixels_per_point);
                        rasterize(&jobs, &textures, width, height)
                            .save(path.join(format!(
                                "{page:?}-{width}-{}-{}.png",
                                if appearance.theme == ThemeChoice::Dark {
                                    "charcoal"
                                } else {
                                    "cream"
                                },
                                appearance.text_scale
                            )))
                            .unwrap();
                    }
                }
            }
            clubhouse.release_images();
            assert!(clubhouse.texture_ids().is_empty());
        }
    }
}

type Textures = HashMap<egui::TextureId, (egui::ColorImage, egui::TextureOptions)>;
fn apply_textures(textures: &mut Textures, delta: &egui::TexturesDelta) {
    for (id, deltas) in &delta.set {
        for delta in deltas {
            let egui::ImageData::Color(image) = &delta.image;
            if let Some([left, top]) = delta.pos {
                let (target, _) = textures.get_mut(id).unwrap();
                for y in 0..image.size[1] {
                    for x in 0..image.size[0] {
                        target.pixels[(top + y) * target.size[0] + left + x] =
                            image.pixels[y * image.size[0] + x];
                    }
                }
            } else {
                textures.insert(*id, ((**image).clone(), delta.options));
            }
        }
    }
}
fn rasterize(
    jobs: &[egui::ClippedPrimitive],
    textures: &Textures,
    width: u32,
    height: u32,
) -> image::RgbaImage {
    let mut result = image::RgbaImage::from_pixel(width, height, image::Rgba(paper().to_array()));
    for job in jobs {
        let egui::epaint::Primitive::Mesh(mesh) = &job.primitive else {
            continue;
        };
        let (texture, options) = &textures[&mesh.texture_id];
        for triangle in mesh.indices.chunks_exact(3) {
            let v = [
                mesh.vertices[triangle[0] as usize],
                mesh.vertices[triangle[1] as usize],
                mesh.vertices[triangle[2] as usize],
            ];
            let area = cross(v[1].pos - v[0].pos, v[2].pos - v[0].pos);
            if area.abs() < 0.0001 {
                continue;
            }
            let left = v
                .iter()
                .map(|v| v.pos.x)
                .fold(f32::INFINITY, f32::min)
                .max(job.clip_rect.left())
                .max(0.0)
                .floor() as u32;
            let right = v
                .iter()
                .map(|v| v.pos.x)
                .fold(f32::NEG_INFINITY, f32::max)
                .min(job.clip_rect.right())
                .min(width as f32)
                .ceil() as u32;
            let top = v
                .iter()
                .map(|v| v.pos.y)
                .fold(f32::INFINITY, f32::min)
                .max(job.clip_rect.top())
                .max(0.0)
                .floor() as u32;
            let bottom = v
                .iter()
                .map(|v| v.pos.y)
                .fold(f32::NEG_INFINITY, f32::max)
                .min(job.clip_rect.bottom())
                .min(height as f32)
                .ceil() as u32;
            for y in top..bottom {
                for x in left..right {
                    let point = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
                    let a = cross(v[1].pos - point, v[2].pos - point) / area;
                    let b = cross(v[2].pos - point, v[0].pos - point) / area;
                    let c = 1.0 - a - b;
                    if a < -0.0001 || b < -0.0001 || c < -0.0001 {
                        continue;
                    }
                    let uv = v[0].uv.to_vec2() * a + v[1].uv.to_vec2() * b + v[2].uv.to_vec2() * c;
                    let sample = sample(
                        texture,
                        uv,
                        options.magnification == egui::TextureFilter::Linear,
                    );
                    let mut color = [0.0; 4];
                    for channel in 0..4 {
                        color[channel] = (f32::from(v[0].color[channel]) * a
                            + f32::from(v[1].color[channel]) * b
                            + f32::from(v[2].color[channel]) * c)
                            / 255.0
                            * sample[channel];
                    }
                    let dest = result.get_pixel_mut(x, y);
                    for channel in 0..3 {
                        dest[channel] = (color[channel]
                            + f32::from(dest[channel]) * (1.0 - color[3] / 255.0))
                            .clamp(0.0, 255.0) as u8;
                    }
                    dest[3] = 255;
                }
            }
        }
    }
    result
}
fn sample(image: &egui::ColorImage, uv: egui::Vec2, linear: bool) -> [f32; 4] {
    let px = uv.x * image.size[0] as f32 - 0.5;
    let py = uv.y * image.size[1] as f32 - 0.5;
    let fetch = |x: f32, y: f32| {
        image.pixels[(y.clamp(0.0, image.size[1] as f32 - 1.0) as usize) * image.size[0]
            + x.clamp(0.0, image.size[0] as f32 - 1.0) as usize]
            .to_array()
            .map(f32::from)
    };
    if !linear {
        return fetch(px.round(), py.round());
    }
    let a = fetch(px.floor(), py.floor());
    let b = fetch(px.floor() + 1.0, py.floor());
    let c = fetch(px.floor(), py.floor() + 1.0);
    let d = fetch(px.floor() + 1.0, py.floor() + 1.0);
    let fx = px - px.floor();
    let fy = py - py.floor();
    std::array::from_fn(|i| {
        (a[i] * (1.0 - fx) + b[i] * fx) * (1.0 - fy) + (c[i] * (1.0 - fx) + d[i] * fx) * fy
    })
}

fn cross(a: egui::Vec2, b: egui::Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

struct Harness {
    context: egui::Context,
    save: SaveFile,
    monitors: Vec<MonitorInfo>,
    draft: Settings,
    tab: SettingsTab,
    clubhouse: Clubhouse,
    selected: Option<CreatureId>,
    names: BTreeMap<CreatureId, String>,
    remove: Option<CreatureId>,
    bulk: bool,
    labels: Vec<(String, egui::Rect)>,
    /// Every filled or outlined rectangle drawn last frame, where it was drawn.
    rects: Vec<egui::Rect>,
    /// How big the window is.
    screen: egui::Vec2,
    /// Whatever the last frame drew past the edge of the area that shows it.
    cut: Vec<String>,
    textures: Textures,
    time: f64,
}
impl Harness {
    fn new(tab: SettingsTab) -> Self {
        let (save, monitors) = fixture();
        let context = egui::Context::default();
        configure_style(&context, AppearancePreferences::default());
        Self {
            context,
            draft: save.settings.clone(),
            selected: Some(save.creatures[0].id),
            save,
            monitors,
            tab,
            clubhouse: Clubhouse::default(),
            names: BTreeMap::new(),
            remove: None,
            bulk: false,
            labels: Vec::new(),
            rects: Vec::new(),
            // Tall enough for the whole Home page with a full village to arrange.
            screen: egui::vec2(1000.0, 2200.0),
            cut: Vec::new(),
            textures: Textures::new(),
            time: 0.0,
        }
    }

    /// What the menu's own artwork costs right now, in uploaded pixels.
    fn artwork_bytes(&self) -> usize {
        self.clubhouse
            .texture_ids()
            .iter()
            .map(|id| self.textures[id].0.pixels.len() * 4)
            .sum()
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> SettingsOutcome {
        self.time += 0.05;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.screen)),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let mut outcome = SettingsOutcome::default();
        let mut output = self.context.run_ui(input, |ui| {
            draw_settings(
                ui,
                &mut self.draft,
                &mut self.tab,
                &mut None,
                &self.save.settings,
                "/example/colony.json",
                &self.monitors,
                &[],
                false,
                &UpdateStatus::Idle,
                false,
                &self.save.creatures,
                &self.save.relationships,
                &mut self.names,
                &mut self.selected,
                &mut self.clubhouse,
                &self.save,
                &mut self.remove,
                &mut self.bulk,
                &mut outcome,
            );
        });
        self.labels.clear();
        self.rects.clear();
        self.cut = cut_off(&output.shapes);
        for shape in &output.shapes {
            collect_labels(&shape.shape, &mut self.labels);
            collect_rects(&shape.shape, &mut self.rects);
        }
        apply_textures(&mut self.textures, &output.textures_delta);
        output.textures_delta.clear();
        outcome
    }
    /// Where the Home page's preview drew something that can be picked out, last frame.
    fn shown(&mut self, picked: crate::clubhouse::arrange::Picked) -> egui::Rect {
        self.frame(Vec::new());
        self.frame(Vec::new());
        self.clubhouse
            .home
            .arrange
            .shown
            .iter()
            .find(|(candidate, _)| *candidate == picked)
            .unwrap_or_else(|| panic!("{picked:?} is not in the preview"))
            .1
    }

    /// Takes hold of whatever is at `from` and lets it go at `to`, a few frames along the way.
    fn drag(&mut self, from: egui::Pos2, to: egui::Pos2) -> SettingsOutcome {
        self.frame(vec![egui::Event::PointerMoved(from)]);
        self.frame(vec![egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }]);
        for step in 1..=4 {
            let t = step as f32 / 4.0;
            self.frame(vec![egui::Event::PointerMoved(from + (to - from) * t)]);
        }
        self.frame(vec![egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }])
    }

    fn click(&mut self, label: &str) -> SettingsOutcome {
        self.frame(Vec::new());
        self.frame(Vec::new());
        let point = self
            .labels
            .iter()
            .find(|(text, _)| text == label)
            .unwrap_or_else(|| {
                panic!(
                    "No visible label {label:?}; {:?}",
                    self.labels.iter().map(|x| &x.0).collect::<Vec<_>>()
                )
            })
            .1
            .center();
        self.click_at(point)
    }

    fn click_at(&mut self, point: egui::Pos2) -> SettingsOutcome {
        self.frame(vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ]);
        self.frame(vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }])
    }
}
fn collect_rects(shape: &egui::Shape, rects: &mut Vec<egui::Rect>) {
    match shape {
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                collect_rects(shape, rects);
            }
        }
        egui::Shape::Rect(rect) => rects.push(rect.rect),
        _ => {}
    }
}
fn collect_labels(shape: &egui::Shape, labels: &mut Vec<(String, egui::Rect)>) {
    match shape {
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                collect_labels(shape, labels);
            }
        }
        egui::Shape::Text(text) => labels.push((
            text.galley.text().to_owned(),
            text.galley.rect.translate(text.pos.to_vec2()),
        )),
        _ => {}
    }
}

#[test]
fn studio_clicks_preview_before_adoption_and_protect_replacement() {
    let mut h = Harness::new(SettingsTab::Studio);
    let shared = SharedCreatureSeed {
        source_colony_seed: [99; 32],
        source_generation: 2,
        design: None,
    };
    h.clubhouse.studio.seed_code = encode_creature_seed(shared.into());
    let outcome = h.click("Preview shared creature");
    assert_eq!(outcome.preview_shared, Some(shared));
    assert!(outcome.accept_creature_preview.is_none());
    let creature =
        World::from_shared_creature(shared, h.save.maximum_seen_utc, &DesktopSnapshot::default())
            .save
            .creatures
            .remove(0);
    h.clubhouse.studio.push_preview(
        &h.context,
        GenerationPreview {
            shared: Some(shared),
            creature,
            source_seed: shared.source_colony_seed,
            similarity: None,
            summary: "Exact shared companion".into(),
        },
    );
    // A full colony: adopting cannot replace it or silently remove an individual.
    while h.save.creatures.len() < formiga_core::MAX_COLONY_CREATURES {
        let extra = h.save.creatures[0].clone();
        let order = h.save.creatures.len() as u8;
        h.save.creatures.push(Creature {
            id: 700 + u64::from(order),
            colony_order: order,
            role: formiga_core::CreatureRole::Adult,
            ..extra
        });
    }
    assert!(
        h.click("Adopt into colony")
            .accept_creature_preview
            .is_none()
    );
    h.save.creatures.truncate(2);
    assert!(
        matches!(h.click("Adopt into colony").accept_creature_preview,Some(PreviewAcceptance::Shared { shared:s,replace:None }) if s == shared)
    );
    h.click("Replace a companion…");
    let id = h.save.creatures[0].id;
    h.save.creatures[0].kept = true;
    h.click("Replace Mallow and start fresh history for this companion");
    assert!(
        h.click("Confirm replacement")
            .accept_creature_preview
            .is_none()
    );
    h.save.creatures[0].kept = false;
    assert!(
        matches!(h.click("Confirm replacement").accept_creature_preview,Some(PreviewAcceptance::Shared { replace:Some(target), .. }) if target == id)
    );
}

#[test]
fn home_quiet_and_onboarding_controls_emit_the_expected_commands() {
    let mut h = Harness::new(SettingsTab::General);
    assert_eq!(h.click("30 minutes").quiet_minutes, Some(30));
    h.draft.reduce_motion = true;
    let (index, preset) = h.click("Save current preferences").save_mode.unwrap();
    assert_eq!(index, 0);
    assert!(preset.reduce_motion);
    h.tab = SettingsTab::Home;
    assert_eq!(
        h.click("Bottom left").home_corner,
        Some(HomeCorner::BottomLeft)
    );
    // A house picked out in the preview, and something hung on its roof.
    let founder = house_owners(&h.save.creatures, &[]).as_slice()[0];
    let house = h.shown(crate::clubhouse::arrange::Picked::House(founder));
    h.click_at(house.center());
    h.click("Nothing");
    assert_eq!(
        h.click("Roof star").set_decoration,
        Some((
            founder,
            DecorationSlot::Roof,
            Some(ShelterDecorationKind::RoofOrnament)
        ))
    );
    assert_eq!(h.click("Further").move_object, Some((0, 1)));
    h.tab = SettingsTab::Colony;
    h.save.companion.onboarding_complete = false;
    assert!(h.click("Skip the tour").complete_onboarding);
}

/// A pin is a promise that this moment will still be here. The rolling journal keeps only the
/// most recent sixty-four, so a kept moment has to outlive its own entry — and stay removable,
/// or a reader could lose one of their eight slots to something they can no longer see.
/// Both new exports reach the app the same way the creature card does: by asking for one on the
/// outcome. Nothing is rendered and no file is chosen here — the app opens the save dialog first,
/// and only then draws anything.
#[test]
fn the_sticker_and_colony_portrait_controls_ask_the_app_for_an_export() {
    use formiga_art::{DEFAULT_STICKER_SCALE, STICKER_SCALES, StickerClip};

    let mut h = Harness::new(SettingsTab::Home);
    let before = (h.save.clone(), h.draft.clone());
    assert!(h.click("Export colony portrait…").export_colony_card);

    h.tab = SettingsTab::Colony;
    let creature = h.save.creatures[0].id;
    // The clip is chosen right beside the button, and carried with the request.
    h.clubhouse.colony.sticker_clip = StickerClip::Dance;
    assert_eq!(
        h.click("Export sticker…").export_creature_sticker,
        Some((creature, StickerClip::Dance, DEFAULT_STICKER_SCALE))
    );
    // Both offered sizes are reachable at the size the window actually opens at.
    let [smaller, larger] = STICKER_SCALES;
    assert_eq!(DEFAULT_STICKER_SCALE, larger);
    h.click("4×");
    assert_eq!(
        h.click("Export sticker…").export_creature_sticker,
        Some((creature, StickerClip::Dance, smaller))
    );
    // Asking for a picture never changes the colony or the preferences being edited.
    assert!(h.save == before.0 && h.draft == before.1);
}

#[test]
fn a_kept_moment_outlives_the_journal_entry_it_was_taken_from() {
    let mut h = Harness::new(SettingsTab::Journal);
    let now = h.save.created_at_utc;
    let keeper = h.save.creatures[0].id;
    // One find, kept, and then a long run of ordinary moments on top of it. (An arrival would
    // be held on to as a milestone, so a find is what a pin has to outlast.)
    let first = JournalEntry {
        at: now,
        creature: Some(keeper),
        moment: JournalMoment::Discovery,
    };
    h.save.companion.journal.push(first.clone());
    assert!(h.save.companion.pin(&first));
    for index in 0..(MAX_JOURNAL_ENTRIES as i64 * 2) {
        h.save.companion.journal.push(JournalEntry {
            at: now + time::Duration::hours(index + 1),
            creature: Some(keeper),
            moment: JournalMoment::Ritual(RitualKind::Picnic),
        });
    }
    h.save.companion.normalize();
    assert!(
        !h.save.companion.journal.contains(&first),
        "the rolling journal has moved on past the kept moment"
    );
    assert_eq!(h.save.companion.pins.len(), 1, "but the pin is still held");
    // The page still shows it, in the words the journal would have used.
    let wanted = clubhouse::moment_text(&h.save, &first);
    h.frame(Vec::new());
    assert!(
        h.labels.iter().any(|(text, _)| *text == wanted),
        "a kept moment must still be shown: {:?}",
        h.labels.iter().map(|x| &x.0).collect::<Vec<_>>()
    );
    // And it can still be given up, so a slot is never lost to something invisible.
    let outcome = h.click("Unpin");
    assert!(
        outcome.unpin_moment.is_some_and(|entry| entry == first),
        "a kept moment stays removable once its entry is gone"
    );
}

#[test]
fn the_home_preview_shows_the_real_village_and_never_calls_the_colony_home() {
    let mut h = Harness::new(SettingsTab::Home);
    // Four companions, one of them a mini, and a corner full of belongings.
    h.save.home.active_since_utc = Some(h.save.created_at_utc);
    let before = (
        h.save.home.clone(),
        h.save.creatures.clone(),
        h.save.settings.clone(),
    );
    for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
        h.save.home.corner = corner;
        for scale in [2u8, 4] {
            h.save.settings.display_scale = scale;
            for preset in [
                HabitatPreset::EntireDesktop,
                HabitatPreset::BottomCorners,
                HabitatPreset::BottomEdge,
            ] {
                h.save.settings.habitat = HabitatPolicy {
                    preset,
                    zones: Vec::new(),
                };
                let outcome = h.frame(Vec::new());
                // Looking at the corner never asks the colony to come home, and never moves it.
                assert!(outcome.home_corner.is_none() && outcome.home_display.is_none());
                assert!(outcome.applied.is_none() && !outcome.gather);
            }
        }
    }
    // An inactive home still draws the page without asking for a visit.
    h.save.home.active_since_utc = None;
    let outcome = h.frame(Vec::new());
    assert!(!outcome.gather && outcome.home_corner.is_none());
    // Nothing the preview did changed the colony, the home, or the settings.
    h.save.home.corner = before.0.corner;
    h.save.settings = before.2;
    h.save.home.active_since_utc = before.0.active_since_utc;
    assert_eq!(h.save.creatures, before.1);
    assert_eq!(h.save.home, before.0);
    // The village atlas, the object atlas and the colony's own trinket sheet carry the corner
    // however large it grows: a tree with nothing on it, a tree with every keepsake the colony
    // can find hung on it, and half the colony gone all cost the same three textures.
    h.save.home.active_since_utc = Some(h.save.created_at_utc);
    h.frame(Vec::new());
    let full = h.clubhouse.texture_ids().len();
    h.save.companion.scrapbook = (0..TRINKET_VARIANTS)
        .map(|variant| ScrapbookRecord {
            variant,
            first_at: h.save.created_at_utc,
            finder: None,
            finder_name: String::new(),
        })
        .collect();
    h.frame(Vec::new());
    assert_eq!(h.clubhouse.texture_ids().len(), full);
    h.save.creatures.truncate(1);
    h.save.objects.objects.clear();
    h.save.companion.scrapbook.clear();
    h.frame(Vec::new());
    assert_eq!(h.clubhouse.texture_ids().len(), full);
}

#[test]
fn appearance_choices_reach_the_colony_and_stay_within_their_own_limits() {
    let mut h = Harness::new(SettingsTab::General);
    h.frame(Vec::new());
    for (theme, scale) in [
        (ThemeChoice::Dark, 150u8),
        (ThemeChoice::Light, 100),
        (ThemeChoice::System, 125),
    ] {
        let context = egui::Context::default();
        configure_style(
            &context,
            AppearancePreferences {
                theme,
                text_scale: scale,
                sprite_outline: theme == ThemeChoice::Dark,
            },
        );
        assert_eq!(
            crate::clubhouse::dark_interface(),
            theme == ThemeChoice::Dark,
            "{theme:?}"
        );
        // Body text always contrasts with the page it sits on, in either theme.
        let dark_theme = crate::clubhouse::dark_interface();
        let egui_theme = if dark_theme {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        let visuals = context.style_of(egui_theme).visuals.clone();
        let ink = visuals.override_text_color.unwrap();
        let paper = visuals.panel_fill;
        let distance = |a: egui::Color32, b: egui::Color32| {
            (i32::from(a.r()) - i32::from(b.r())).abs()
                + (i32::from(a.g()) - i32::from(b.g())).abs()
                + (i32::from(a.b()) - i32::from(b.b())).abs()
        };
        assert!(distance(ink, paper) > 300, "{theme:?} text is unreadable");
        // Text scaling reaches every style, and never runs away.
        let style = context.style_of(egui_theme);
        let body = style.text_styles[&egui::TextStyle::Body].size;
        assert!((body - 14.0 * f32::from(scale) / 100.0).abs() < 0.01);
        assert!((10.0..=24.0).contains(&body));
    }
    configure_style(&h.context, AppearancePreferences::default());
}

/// Every texture the settings window may hold at once. The scrapbook went from eight 16x16
/// drawings baked per creature to one 256x32 colony sheet that carries all sixteen trinkets and
/// their glint frames: one texture instead of eight, and 24 KiB more pixels, which is what moved
/// this from 416 to 432 KiB. In 0.59.0 every house got a cell of its own, so each can wear its
/// resident's curtain, and the Home page's daylit village grew from 128x128 to 256x128: 64 KiB
/// more, which moved this to 496 KiB. The object sheet then grew from eight cells to fourteen,
/// three hangout spots and three garden patches of 16x16 each: 6 KiB more, which is what moves
/// this to 500 KiB. In 0.60.0 the scrapbook became a Collection of 160 keepsakes, of which the
/// pages hold only the resting half of the sheet (160 KiB, 128 KiB more); the object sheet grew
/// to carry every garden at every stage, fifteen spots, fifteen ornaments and the village's
/// props (112 KiB, 98 KiB more); the Home page's village shrank to the one row it draws (128 KiB,
/// no change); and a companion trying something on shows four poses (36 KiB): 760 KiB. In 0.61.0
/// the houses were drawn a quarter larger, so the Home page's row of the village is 560x80
/// rather than 512x64 (175 KiB, 47 KiB more): 807 KiB. In 0.63.1 the creature studio stands its
/// large preview beside a village tree, one 64x64 texture shared by every candidate (16 KiB):
/// 823 KiB. In 0.65.0 the houses grew a fifth again and the Home page's row of the village with
/// them, to 672x96 (252 KiB, 77 KiB more): 900 KiB.
const ARTWORK_BUDGET: usize = 900 * 1024;

#[test]
fn opening_and_closing_the_menu_over_and_over_rebuilds_the_same_artwork_and_keeps_none_of_it() {
    let mut h = Harness::new(SettingsTab::Home);
    h.save.companion.scrapbook = (0..TRINKET_VARIANTS)
        .map(|variant| ScrapbookRecord {
            variant,
            first_at: h.save.created_at_utc,
            finder: Some(h.save.creatures[0].id),
            finder_name: h.save.creatures[0].name.clone(),
        })
        .collect();
    let mut rounds = Vec::new();
    for round in 0..8 {
        // Every page that has artwork on it: portraits, the village and its keepsakes, and a
        // studio full of candidates.
        for tab in [SettingsTab::Colony, SettingsTab::Home, SettingsTab::Studio] {
            h.tab = tab;
            if tab == SettingsTab::Colony {
                // Looking at each companion in turn is how all four portraits come to be drawn.
                for index in 0..h.save.creatures.len() {
                    h.selected = Some(h.save.creatures[index].id);
                    h.frame(Vec::new());
                    h.frame(Vec::new());
                }
            }
            if tab == SettingsTab::Studio {
                for index in 0..4 {
                    let seed = [40 + index; 32];
                    h.clubhouse.studio.push_preview(
                        &h.context,
                        GenerationPreview {
                            shared: None,
                            creature: World::preview_adult(
                                seed,
                                h.save.maximum_seen_utc,
                                &DesktopSnapshot::default(),
                            ),
                            source_seed: seed,
                            similarity: None,
                            summary: String::new(),
                        },
                    );
                }
            }
            h.frame(Vec::new());
            h.frame(Vec::new());
        }
        rounds.push((h.clubhouse.texture_ids().len(), h.artwork_bytes()));
        // Closing the menu gives all of it back.
        h.clubhouse.release_images();
        assert!(
            h.clubhouse.texture_ids().is_empty(),
            "round {round} kept artwork after closing"
        );
        assert_eq!(h.artwork_bytes(), 0);
    }
    eprintln!("settings artwork per opening: {rounds:?}");
    assert!(
        rounds.iter().all(|round| *round == rounds[0]),
        "the menu costs more each time it is opened: {rounds:?}"
    );
    let (textures, bytes) = rounds[0];
    assert!(
        bytes <= ARTWORK_BUDGET,
        "UI artwork exceeded budget: {bytes}"
    );
    // Four portraits, four candidate strips and the tree the large preview stands beside, the
    // village atlas, the object atlas, one sheet holding every trinket, and the last companion
    // looked at trying something on in four poses.
    assert_eq!(textures, 4 + 4 + 1 + 1 + 1 + 1 + 4);
}

#[test]
fn all_ui_artwork_together_fits_the_budget() {
    let mut h = Harness::new(SettingsTab::Home);
    for index in 0..4 {
        let seed = [30 + index; 32];
        h.clubhouse.studio.push_preview(
            &h.context,
            GenerationPreview {
                shared: None,
                creature: World::preview_adult(
                    seed,
                    h.save.maximum_seen_utc,
                    &DesktopSnapshot::default(),
                ),
                source_seed: seed,
                similarity: None,
                summary: String::new(),
            },
        );
    }
    let mut output = h.context.run_ui(egui::RawInput::default(), |ui| {
        for c in &h.save.creatures {
            h.clubhouse.shell.portrait(ui, c, 48.0);
        }
        h.clubhouse.home.show(
            ui,
            &mut h.clubhouse.shell,
            &h.save,
            &h.monitors,
            &mut SettingsOutcome::default(),
        );
    });
    let mut textures = HashMap::new();
    apply_textures(&mut textures, &output.textures_delta);
    output.textures_delta.clear();
    let total: usize = h
        .clubhouse
        .texture_ids()
        .iter()
        .map(|id| textures[id].0.pixels.len() * 4)
        .sum();
    assert!(
        total <= ARTWORK_BUDGET,
        "Combined UI artwork: {total} bytes"
    );
    h.clubhouse.release_images();
    assert!(h.clubhouse.texture_ids().is_empty());
}

/// Every pair of companions appears once, grouped by how they get along with the pair keeping its
/// distance first, and a pair that has never spent time together reads as getting acquainted.
#[test]
fn the_colony_view_lists_every_pair_once_by_how_they_get_along() {
    let (save, _) = fixture();
    let creatures = &save.creatures;
    assert!(creatures.len() >= 4, "a colony worth reading");
    let (a, b, c, d) = (
        creatures[0].id,
        creatures[1].id,
        creatures[2].id,
        creatures[3].id,
    );
    let record = |x, y, affinity, familiarity, playfulness, avoidance| {
        let mut relationship = CreatureRelationship::new(x, y).unwrap();
        relationship.affinity = affinity;
        relationship.familiarity = familiarity;
        relationship.playfulness = playfulness;
        relationship.avoidance = avoidance;
        relationship
    };
    let relationships = vec![
        record(a, b, 200, 180, 90, 0),
        record(a, c, 20, 30, 120, 10),
        record(b, c, 60, 60, 20, 170),
        record(a, d, 120, 90, 10, 5),
    ];
    let pairs = colony_standings(creatures, &relationships);
    let expected = creatures.len() * (creatures.len() - 1) / 2;
    assert_eq!(pairs.len(), expected);
    let mut seen = std::collections::BTreeSet::new();
    for pair in &pairs {
        assert!(
            seen.insert((pair.a.min(pair.b), pair.a.max(pair.b))),
            "a pair listed twice"
        );
    }
    let standing = |x: CreatureId, y: CreatureId| {
        pairs
            .iter()
            .find(|pair| (pair.a == x && pair.b == y) || (pair.a == y && pair.b == x))
            .map(|pair| pair.standing)
            .unwrap()
    };
    assert_eq!(standing(b, c), Standing::Distant);
    assert_eq!(standing(a, b), Standing::Close);
    assert_eq!(standing(a, d), Standing::Close);
    assert_eq!(standing(a, c), Standing::Playmates);
    assert_eq!(standing(c, d), Standing::Acquainting);
    // Grouped in order, and closest first within a group.
    assert!(pairs.windows(2).all(|w| w[0].standing <= w[1].standing));
    let close: Vec<_> = pairs
        .iter()
        .filter(|pair| pair.standing == Standing::Close)
        .collect();
    // Pairs are named in colony order, and the closest of the two close pairs comes first.
    assert_eq!((close[0].a, close[0].b), (a, b));
}

/// Today's recap reads only what was written down today, newest first, and says so when a full
/// journal may already have dropped some of today's earlier moments.
#[test]
fn today_recaps_only_what_was_recorded_today_and_admits_what_rolled_out() {
    let (mut save, _) = fixture();
    let offset = time::UtcOffset::UTC;
    let now = time::macros::datetime!(2026-09-14 18:00 UTC);
    let today_date = now.date();
    let creature = save.creatures[0].id;
    save.companion.journal.clear();
    save.companion.scrapbook.clear();
    // Yesterday's and today's moments.
    save.companion.journal.push(JournalEntry {
        at: now - time::Duration::days(1),
        creature: Some(creature),
        moment: JournalMoment::Discovery,
    });
    for hour in [9, 12, 15] {
        save.companion.journal.push(JournalEntry {
            at: time::macros::datetime!(2026-09-14 0:00 UTC) + time::Duration::hours(hour),
            creature: Some(creature),
            moment: JournalMoment::Discovery,
        });
    }
    save.companion.scrapbook.push(ScrapbookRecord {
        variant: 5,
        first_at: now - time::Duration::hours(2),
        finder: Some(creature),
        finder_name: "Mallow".into(),
    });
    let recap = clubhouse::today::today(&save, today_date, offset);
    assert_eq!(recap.moments.len(), 3);
    assert!(
        recap.moments.windows(2).all(|w| w[0].at >= w[1].at),
        "newest first"
    );
    assert_eq!(recap.found, vec![5]);
    assert!(
        !recap.rolled_out,
        "yesterday's entry is still here, so nothing of today was lost"
    );
    // A full journal that begins today may have lost some of today's earlier moments.
    save.companion
        .journal
        .retain(|entry| entry.at.date() == today_date);
    while save.companion.journal.len() < MAX_JOURNAL_ENTRIES {
        save.companion.journal.push(JournalEntry {
            at: now,
            creature: None,
            moment: JournalMoment::Ritual(RitualKind::Picnic),
        });
    }
    assert!(clubhouse::today::today(&save, today_date, offset).rolled_out);
    // Another day has nothing to say about this one.
    let tomorrow = clubhouse::today::today(&save, today_date.next_day().unwrap(), offset);
    assert!(tomorrow.moments.is_empty() && tomorrow.found.is_empty() && !tomorrow.rolled_out);
}

/// A companion's own ways are on its page, under the portrait: how it celebrates, then each habit
/// it has picked up in the order it picked them up. Picking one up is a journal line of its own.
#[test]
fn a_companions_own_ways_are_on_its_page_and_in_the_journal() {
    let mut h = Harness::new(SettingsTab::Colony);
    let creature = h.save.creatures[0].id;
    let name = h.save.creatures[0].name.clone();
    let celebration = Celebration::for_creature(&h.save.creatures[0]).label();
    h.frame(Vec::new());
    assert!(
        h.labels.iter().any(|(text, _)| text == celebration),
        "a companion with no habits yet still has its own celebration"
    );
    h.save.creatures[0].memory.habits = vec![Habit::CirclesBeforeNaps, Habit::WavesHello];
    h.frame(Vec::new());
    let wanted = format!("{celebration} · Turns in circles before a nap · Waves hello");
    assert!(
        h.labels.iter().any(|(text, _)| *text == wanted),
        "{:?}",
        h.labels.iter().map(|x| &x.0).collect::<Vec<_>>()
    );
    let entry = JournalEntry {
        at: h.save.created_at_utc,
        creature: Some(creature),
        moment: JournalMoment::Habit(Habit::WavesHello),
    };
    assert_eq!(
        clubhouse::moment_text(&h.save, &entry),
        format!("{name} picked up a little habit: waves hello")
    );
}

/// Anything the village has can be put down on its ground from the shelves on the Home page —
/// somewhere with room, in the widest gap left — and taken up again the same way. The page only
/// asks: the app is what changes the colony.
#[test]
fn things_are_put_down_on_the_ground_and_taken_up_again_from_the_shelves() {
    let mut h = Harness::new(SettingsTab::Home);
    let before = h.save.clone();
    for shelf in ["Gardens", "Hangout spots", "Ornaments"] {
        h.frame(Vec::new());
        assert!(
            h.labels.iter().any(|(text, _)| text == shelf),
            "the {shelf} shelf is on the page"
        );
    }
    assert_eq!(
        h.click("Nap cushion").set_hangout,
        Some((HangoutKind::Cushion, Some(0.5)))
    );
    assert_eq!(
        h.click("Flower bed").set_garden,
        Some((GardenKind::Flowers, Some(0.5)))
    );
    assert_eq!(
        h.click("Lamp post").set_ornament,
        Some((OrnamentKind::LampPost, Some(0.5)))
    );
    assert!(h.save == before, "asking never changes the colony itself");
    // With the cushion down in the middle, the next thing goes in the middle of a side.
    h.save.home.set_hangout(HangoutKind::Cushion, Some(0.5));
    let (kind, along) = h.click("Flower bed").set_garden.unwrap();
    assert_eq!(kind, GardenKind::Flowers);
    let along = along.unwrap();
    assert!(
        (along - 0.775).abs() < 1e-4 || (along - 0.225).abs() < 1e-4,
        "{along}"
    );
    assert_eq!(
        h.click("Nap cushion").set_hangout,
        Some((HangoutKind::Cushion, None))
    );
}

/// The village is arranged in its preview: a cottage carried along the row to stand further out,
/// something on the ground carried along it, a named palette chosen, and all of it put back as it
/// grew once that has been asked twice. The page only asks: the app is what changes the colony.
#[test]
fn the_village_is_arranged_in_its_preview() {
    use crate::clubhouse::arrange::Picked;
    let mut h = Harness::new(SettingsTab::Home);
    h.save.home.corner = HomeCorner::BottomLeft;
    // Two cottages to put in order: one of the minis grown up for the purpose.
    let grown = h
        .save
        .creatures
        .iter()
        .position(|creature| !creature.role.is_adult())
        .expect("the fixture has a mini");
    h.save.creatures[grown].role = CreatureRole::Adult;
    let owners = house_owners(&h.save.creatures, &[]);
    let owners = owners.as_slice().to_vec();
    assert_eq!(owners.len(), 3);
    h.save.home.set_hangout(HangoutKind::Cushion, Some(0.2));
    let before = h.save.clone();
    h.click("Arrange");
    // The first cottage, carried past the second.
    let first = h.shown(Picked::House(owners[1]));
    let second = h.shown(Picked::House(owners[2]));
    let outcome = h.drag(
        first.center(),
        egui::pos2(second.right() + 4.0, first.center().y),
    );
    assert_eq!(outcome.cottage_order, Some(vec![owners[2], owners[1]]));
    // The colony house always stands first, and cannot be carried anywhere.
    let colony = h.shown(Picked::House(owners[0]));
    let outcome = h.drag(
        colony.center(),
        egui::pos2(second.right() + 4.0, colony.center().y),
    );
    assert!(outcome.cottage_order.is_none());
    // The cushion, carried along the ground to the right.
    let cushion = h.shown(Picked::Ground(GroundItem::Hangout(HangoutKind::Cushion)));
    let outcome = h.drag(cushion.center(), cushion.center() + egui::vec2(60.0, 0.0));
    let (kind, along) = outcome
        .set_hangout
        .expect("the cushion was let go somewhere");
    assert_eq!(kind, HangoutKind::Cushion);
    assert!(along.is_some_and(|along| along > 0.2), "{along:?}");
    h.click("From the colony");
    assert_eq!(
        h.click("Autumn").village_palette,
        Some(Some(VillagePalette::Autumn))
    );
    assert!(h.save == before, "asking never changes the colony itself");

    // Nothing is arranged yet but the cushion, and a spot stays where it is, so there is nothing
    // to put back.
    let reset = "Put the village back as it grew…";
    assert!(!h.click(reset).reset_village);
    assert!(!h.labels.iter().any(|(text, _)| text == "Put back"));

    h.save.home.palette = Some(VillagePalette::Autumn);
    h.save
        .home
        .set_garden(GardenKind::Herbs, Some(0.3), h.save.created_at_utc);
    assert!(!h.click(reset).reset_village, "the first click only asks");
    assert!(!h.click("Keep them").reset_village);
    h.frame(Vec::new());
    assert!(!h.labels.iter().any(|(text, _)| text == "Put back"));
    h.click(reset);
    assert!(h.click("Put back").reset_village);
}

/// A postcard is chosen and written on the Home page: a scene picked, a caption typed, and the
/// export asked for with the caption made safe to write. The page draws nothing and keeps nothing:
/// the postcard is only ever the file it is exported to.
#[test]
fn a_postcard_is_chosen_and_captioned_from_the_home_page() {
    let mut h = Harness::new(SettingsTab::Home);
    h.frame(Vec::new());
    let before = h.artwork_bytes();
    assert_eq!(
        h.click("Export postcard…").export_postcard,
        Some((formiga_art::PostcardScene::Nap, String::new()))
    );
    h.click("A picnic");
    h.clubhouse.home.postcard_caption = "  Snacks \t for everyone\n".into();
    assert_eq!(
        h.click("Export postcard…").export_postcard,
        Some((
            formiga_art::PostcardScene::Picnic,
            "Snacks for everyone".to_owned()
        ))
    );
    assert!(h.save.creatures.len() > 1);
    assert_eq!(
        h.artwork_bytes(),
        before,
        "choosing a postcard uploads nothing"
    );
}

/// The last change to the colony can be taken back from the footer of any page, and only while
/// there is one: the page asks, and the app is what undoes it.
#[test]
fn the_last_change_is_offered_back_on_every_page() {
    for tab in [
        SettingsTab::Today,
        SettingsTab::Colony,
        SettingsTab::Home,
        SettingsTab::Journal,
    ] {
        let mut h = Harness::new(tab);
        h.frame(Vec::new());
        assert!(
            !h.labels.iter().any(|(text, _)| text.starts_with("Undo ")),
            "nothing to undo, nothing offered"
        );
        h.clubhouse.last_edit = Some("removing Poppy".into());
        let before = h.save.clone();
        assert!(h.click("Undo removing Poppy").undo_last_edit);
        assert!(h.save == before, "asking never changes the colony itself");
    }
}

/// Any house can be built as another type from the Home page, and given back its own: the colony
/// house as well as a cottage. The page only asks.
#[test]
fn a_house_is_built_as_another_type_from_the_home_page() {
    let mut h = Harness::new(SettingsTab::Home);
    h.save.home.shelter.style = ShelterStyle::Tent;
    for creature in &mut h.save.creatures {
        creature.behavior_seed[13] = 2;
        creature.behavior_seed[29] = 0;
    }
    let founder = house_owners(&h.save.creatures, &[]).as_slice()[0];
    let before = h.save.clone();
    // Picked out in the preview, whichever house it is.
    let house = h.shown(crate::clubhouse::arrange::Picked::House(founder));
    h.click_at(house.center());
    h.click("Tent");
    assert_eq!(
        h.click("Mushroom").house_style,
        Some((founder, Some(ShelterStyle::Mushroom)))
    );
    assert!(h.save == before, "asking never changes the colony itself");
    h.save
        .home
        .set_house_style(founder, Some(ShelterStyle::Mushroom));
    h.frame(Vec::new());
    h.click("Mushroom");
    assert_eq!(h.click("Its own · Tent").house_style, Some((founder, None)));
}

#[test]
fn pointing_at_something_to_wear_leaves_every_choice_where_it_was() {
    let mut h = Harness::new(SettingsTab::Colony);
    let mallow = h.save.creatures[0].id;
    // Half of what there is to wear has been found, so the chips are a mix of ones that can be
    // put on and ones still greyed out, and each find made into something can be worn as a pin.
    let made: Vec<AccessoryKind> = AccessoryKind::ALL.into_iter().step_by(2).collect();
    for kind in &made {
        h.save.companion.scrapbook.push(ScrapbookRecord {
            variant: kind.made_from(),
            first_at: time::macros::datetime!(2026-09-10 12:00 UTC),
            finder: Some(mallow),
            finder_name: "Mallow".into(),
        });
    }
    let pins: Vec<Accessory> = available_accessories(&h.save.companion.scrapbook)
        .into_iter()
        .filter(|accessory| matches!(accessory, Accessory::Pin(_)))
        .collect();
    h.click(&format!("Wear a find as a pin · {}", pins.len()));
    h.frame(vec![egui::Event::PointerGone]);
    h.frame(Vec::new());
    // Where every choice is drawn with the pointer nowhere near: the chips by their labels, and
    // the pins by their tiles, the only squares of their size on the page.
    let chip_labels: Vec<&str> = std::iter::once("Nothing")
        .chain(AccessoryKind::ALL.iter().map(|kind| kind.label()))
        .collect();
    let choices = |h: &Harness| {
        let chips: Vec<(String, egui::Rect)> = h
            .labels
            .iter()
            .filter(|(text, _)| chip_labels.contains(&text.as_str()))
            .cloned()
            .collect();
        let mut tiles: Vec<egui::Rect> = h
            .rects
            .iter()
            .filter(|rect| rect.size() == egui::vec2(32.0, 32.0))
            .copied()
            .collect();
        // Each tile is filled and then outlined.
        tiles.dedup();
        (chips, tiles)
    };
    let resting = choices(&h);
    assert_eq!(resting.0.len(), chip_labels.len(), "{:?}", resting.0);
    assert_eq!(resting.1.len(), pins.len(), "{:?}", resting.1);
    let chips = AccessoryKind::ALL.into_iter().map(|kind| {
        let chip = resting.0.iter().find(|(text, _)| text == kind.label());
        let point = chip.expect("every chip is drawn").1.center();
        (Accessory::Worn(kind), made.contains(&kind), point)
    });
    let tiles = pins
        .iter()
        .zip(&resting.1)
        .map(|(pin, tile)| (*pin, true, tile.center()));
    for (accessory, wearable, point) in chips.chain(tiles) {
        // The pointer rests on it for a few frames, the way a hand does before clicking.
        h.frame(vec![egui::Event::PointerMoved(point)]);
        for _ in 0..4 {
            h.frame(Vec::new());
            assert_eq!(
                choices(&h),
                resting,
                "pointing at {} moved the choices",
                accessory.label()
            );
        }
        let note = if wearable {
            format!("Trying on {} · click to put it on", accessory.label())
        } else {
            "Point at something to try it on".to_owned()
        };
        assert!(
            h.labels.iter().any(|(text, _)| *text == note),
            "{note:?} is not shown pointing at {}",
            accessory.label()
        );
        let outcome = h.click_at(point);
        let put_on = wearable.then_some((mallow, Some(accessory)));
        assert_eq!(outcome.set_accessory, put_on, "{}", accessory.label());
    }
}

/// Everything drawn past the edge of the area that shows it: the first text in each such shape,
/// where it reaches, and where it is cut off.
fn cut_off(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    shapes
        .iter()
        .filter_map(|clipped| {
            let bounds = clipped.shape.visual_bounding_rect();
            let clip = clipped.clip_rect;
            (bounds.is_positive()
                && (bounds.right() > clip.right() + 0.5 || bounds.left() < clip.left() - 0.5))
                .then(|| {
                    format!(
                        "{:?} at {:.0} spans {:.0}–{:.0}, shown only {:.0}–{:.0}",
                        first_text(&clipped.shape).unwrap_or_default(),
                        bounds.top(),
                        bounds.left(),
                        bounds.right(),
                        clip.left(),
                        clip.right()
                    )
                })
        })
        .collect()
}

/// The first text in `shape`, to say what was cut off.
fn first_text(shape: &egui::Shape) -> Option<String> {
    match shape {
        egui::Shape::Text(text) => Some(text.galley.text().chars().take(60).collect()),
        egui::Shape::Vec(shapes) => shapes.iter().find_map(first_text),
        _ => None,
    }
}

/// Nothing on any page is drawn past the edge of the part of the window it is shown in, at the
/// narrowest window the app allows and at its default width, at every size of text it offers.
/// egui widens a page to fit whatever is too wide for it, so one line that would not wrap took
/// the rest of the page past the window's edge with it, where the scroll area cut it off: at
/// larger text the Your colony page lost the right-hand end of each row of things to wear.
#[test]
fn no_page_is_drawn_past_the_edge_of_its_window() {
    let (mut save, monitors) = fixture();
    let finder = save.creatures[0].id;
    // Half of what there is to wear has been found, so the wardrobe has both kinds of chip.
    for kind in AccessoryKind::ALL.into_iter().step_by(2) {
        save.companion.scrapbook.push(ScrapbookRecord {
            variant: kind.made_from(),
            first_at: time::macros::datetime!(2026-09-10 12:00 UTC),
            finder: Some(finder),
            finder_name: "Mallow".into(),
        });
    }
    let context = egui::Context::default();
    let mut clubhouse = Clubhouse::default();
    for index in 0..4 {
        let seed = [index + 17; 32];
        clubhouse.studio.push_preview(
            &context,
            GenerationPreview {
                shared: None,
                creature: World::preview_adult(
                    seed,
                    save.maximum_seen_utc,
                    &DesktopSnapshot::default(),
                ),
                source_seed: seed,
                similarity: None,
                summary: "A new companion with fresh memories.".into(),
            },
        );
    }
    let mut cut = Vec::new();
    let mut time = 0.0;
    for text_scale in [100, 125, 150] {
        configure_style(
            &context,
            AppearancePreferences {
                text_scale,
                ..Default::default()
            },
        );
        for width in [760.0, 940.0] {
            for page in crate::clubhouse::journal::PAGES {
                let mut settings = save.settings.clone();
                let mut tab = page;
                let mut names = BTreeMap::new();
                let mut selected = None;
                let mut error = None;
                let mut confirmation = None;
                let mut bulk = false;
                for frame in 0..3 {
                    time += 1.0;
                    // Tall enough that every page is drawn whole, nothing left below the fold.
                    let input = egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 6000.0),
                        )),
                        time: Some(time),
                        ..Default::default()
                    };
                    let mut outcome = SettingsOutcome::default();
                    let mut output = context.run_ui(input, |ui| {
                        draw_settings(
                            ui,
                            &mut settings,
                            &mut tab,
                            &mut error,
                            &save.settings,
                            "/example/colony.json",
                            &monitors,
                            &[],
                            false,
                            &UpdateStatus::Idle,
                            false,
                            &save.creatures,
                            &save.relationships,
                            &mut names,
                            &mut selected,
                            &mut clubhouse,
                            &save,
                            &mut confirmation,
                            &mut bulk,
                            &mut outcome,
                        )
                    });
                    output.textures_delta.clear();
                    if frame < 2 {
                        continue;
                    }
                    for what in cut_off(&output.shapes) {
                        cut.push(format!(
                            "{page:?}, {width} wide, text at {text_scale}%: {what}"
                        ));
                    }
                }
            }
        }
    }
    assert!(cut.is_empty(), "{} cut off: {cut:#?}", cut.len());
}

/// Where the tour is: its header, and the page it has turned to.
fn tour_header(h: &Harness) -> Option<String> {
    h.labels
        .iter()
        .find(|(text, _)| text.starts_with("TOUR · "))
        .map(|(text, _)| text.clone())
}

/// The page each step of the tour is shown on, in order: the desktop basics and the Your colony
/// page, then every other page from the top of the cover's tabs to the bottom, and home again.
const TOURED: [SettingsTab; 19] = [
    SettingsTab::Today,
    SettingsTab::Today,
    SettingsTab::Colony,
    SettingsTab::Colony,
    SettingsTab::Colony,
    SettingsTab::Colony,
    SettingsTab::Colony,
    SettingsTab::Colony,
    SettingsTab::Colony,
    SettingsTab::Colony,
    SettingsTab::Studio,
    SettingsTab::Home,
    SettingsTab::Home,
    SettingsTab::Journal,
    SettingsTab::Habitat,
    SettingsTab::Applications,
    SettingsTab::General,
    SettingsTab::About,
    SettingsTab::Today,
];

#[test]
fn a_new_colony_is_shown_round_every_page_and_can_finish_the_tour() {
    assert_eq!(TOURED.len(), crate::clubhouse::tour::TourState::LENGTH);
    // Wherever the window opens, the tour turns to its own first page.
    let mut h = Harness::new(SettingsTab::Journal);
    h.save.companion.onboarding_complete = false;
    for (index, page) in TOURED.iter().enumerate() {
        h.frame(Vec::new());
        h.frame(Vec::new());
        assert_eq!(
            h.tab,
            *page,
            "step {} is shown on the wrong page",
            index + 1
        );
        assert_eq!(
            tour_header(&h),
            Some(format!("TOUR · {} OF {}", index + 1, TOURED.len()))
        );
        let last = index + 1 == TOURED.len();
        let outcome = h.click(if last { "Finish" } else { "Next" });
        assert_eq!(outcome.complete_onboarding, last, "step {}", index + 1);
    }
    // Finished, it does not start again, though this window's save has not been told yet.
    h.frame(Vec::new());
    assert_eq!(tour_header(&h), None);
    for page in crate::clubhouse::journal::PAGES {
        assert!(TOURED.contains(&page), "{page:?} is not on the tour");
    }
}

#[test]
fn the_tour_goes_back_a_step_and_can_be_skipped_from_any() {
    let mut h = Harness::new(SettingsTab::Colony);
    h.save.companion.onboarding_complete = false;
    for _ in 0..10 {
        h.click("Next");
    }
    h.frame(Vec::new());
    assert_eq!(h.tab, SettingsTab::Studio);
    assert_eq!(tour_header(&h).as_deref(), Some("TOUR · 11 OF 19"));
    h.click("Back");
    h.frame(Vec::new());
    assert_eq!(h.tab, SettingsTab::Colony);
    assert_eq!(tour_header(&h).as_deref(), Some("TOUR · 10 OF 19"));
    assert!(h.click("Skip the tour").complete_onboarding);
    h.frame(Vec::new());
    assert_eq!(tour_header(&h), None);
}

#[test]
fn the_tour_notices_a_companion_petted_carried_and_asked_for_something() {
    let mut h = Harness::new(SettingsTab::Colony);
    h.save.companion.onboarding_complete = false;
    let tried = |h: &Harness| {
        h.labels
            .iter()
            .any(|(text, _)| text == "Lovely — you've tried it!")
    };
    // Say hello, after the Today page's step. The fixture's companions have been petted before;
    // only a pet from now counts.
    h.click("Next");
    h.click("Next");
    h.frame(Vec::new());
    assert!(!tried(&h));
    h.save.creatures[1].memory.times_petted += 1;
    h.frame(Vec::new());
    assert!(tried(&h), "a pet went unnoticed");
    // Pick them up: a toss counts as much as setting one down.
    h.click("Next");
    h.frame(Vec::new());
    assert!(!tried(&h));
    h.save.creatures[0].memory.times_tossed += 1;
    h.frame(Vec::new());
    assert!(tried(&h), "a toss went unnoticed");
    // Ask for something: the app counts the menus opened on the desktop.
    h.click("Next");
    h.frame(Vec::new());
    assert!(!tried(&h));
    h.clubhouse.shell.tour.menus_opened += 1;
    h.frame(Vec::new());
    assert!(tried(&h), "a menu went unnoticed");
    // And the next step asks for nothing.
    h.click("Next");
    h.frame(Vec::new());
    assert!(!tried(&h));
}

#[test]
fn preferences_offers_the_tour_again() {
    let mut h = Harness::new(SettingsTab::General);
    h.frame(Vec::new());
    assert_eq!(
        tour_header(&h),
        None,
        "a colony that has taken the tour is not shown it again unasked"
    );
    h.click("Take the tour");
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert_eq!(h.tab, SettingsTab::Today);
    assert_eq!(tour_header(&h).as_deref(), Some("TOUR · 1 OF 19"));
}

#[test]
fn the_tour_waits_on_its_own_page_and_leads_back_to_it() {
    let mut h = Harness::new(SettingsTab::Colony);
    h.save.companion.onboarding_complete = false;
    h.frame(Vec::new());
    // Another page chosen from the cover, part way through.
    h.tab = SettingsTab::Journal;
    h.frame(Vec::new());
    assert!(
        h.labels
            .iter()
            .any(|(text, _)| text == "The tour is waiting on Today."),
        "{:?}",
        h.labels.iter().map(|(text, _)| text).collect::<Vec<_>>()
    );
    h.click("Back to the tour");
    h.frame(Vec::new());
    assert_eq!(h.tab, SettingsTab::Today);
    assert!(
        h.labels
            .iter()
            .any(|(text, _)| text == "Welcome to your colony")
    );
}

#[test]
fn every_step_of_the_tour_fits_the_smallest_window_at_the_largest_text() {
    let mut h = Harness::new(SettingsTab::Colony);
    h.save.companion.onboarding_complete = false;
    configure_style(
        &h.context,
        AppearancePreferences {
            text_scale: 150,
            ..Default::default()
        },
    );
    // The smallest window the app allows, drawn tall enough to show each page whole.
    h.screen = egui::vec2(760.0, 6000.0);
    for index in 0..TOURED.len() {
        h.frame(Vec::new());
        h.frame(Vec::new());
        assert!(h.cut.is_empty(), "step {}: {:#?}", index + 1, h.cut);
        if index + 1 < TOURED.len() {
            h.click("Next");
        }
    }
}

/// The window is a field notebook: a tab down the cover turns to its page, and one already turned
/// past turns back to it.
#[test]
fn a_tab_on_the_cover_turns_to_its_page_and_back() {
    let mut h = Harness::new(SettingsTab::Colony);
    h.click("Habitat");
    h.frame(Vec::new());
    assert_eq!(h.tab, SettingsTab::Habitat);
    assert!(
        h.labels
            .iter()
            .any(|(text, _)| text.starts_with("FIELD NOTES · Nº 06")),
        "{:?}",
        h.labels.iter().map(|x| &x.0).collect::<Vec<_>>()
    );
    h.click("Your colony");
    h.frame(Vec::new());
    assert_eq!(h.tab, SettingsTab::Colony);
}

/// Turning to another page turns the page for three quarters of a second, and not at all under
/// Reduce motion.
#[test]
fn a_page_turns_when_the_notebook_opens_at_another_unless_motion_is_reduced() {
    let mut h = Harness::new(SettingsTab::Colony);
    h.frame(Vec::new());
    assert!(
        h.clubhouse.page_turn.is_none(),
        "opening at a page turns nothing"
    );
    h.tab = SettingsTab::Journal;
    h.frame(Vec::new());
    assert!(
        h.clubhouse.page_turn.is_some(),
        "turning to another page turns it"
    );
    for _ in 0..20 {
        h.frame(Vec::new());
    }
    assert!(
        h.clubhouse.page_turn.is_none(),
        "a turn is over within a second"
    );

    h.save.settings.reduce_motion = true;
    h.tab = SettingsTab::About;
    h.frame(Vec::new());
    assert!(
        h.clubhouse.page_turn.is_none(),
        "under Reduce motion the page is simply there"
    );
}

/// At the size it was drawn for, the notebook is laid out as the design has it: a 196-point cover,
/// a 712-point page and the binding beyond it; narrower, the page's margins give way first.
#[test]
fn the_notebook_keeps_its_proportions_and_gives_up_margins_before_room() {
    let wide = crate::clubhouse::journal::Spread::of(940.0, 1.0);
    assert_eq!(wide.cover, 196.0);
    assert_eq!(940.0 - wide.cover - wide.binding, 712.0);
    assert_eq!((wide.margin, wide.left, wide.right), (50.0, 70.0, 30.0));
    let narrow = crate::clubhouse::journal::Spread::of(760.0, 1.5);
    assert!(narrow.left < wide.left && narrow.binding < wide.binding);
}

/// Every page of the notebook, at the largest text it offers, fits in the font atlas a single
/// page's worth of text needs: 2048 by 128 glyph pixels. The notebook's labels share one size, and
/// its page names stop growing before their glyphs would double the atlas.
#[test]
fn every_page_at_the_largest_text_fits_one_font_atlas() {
    let (save, monitors) = fixture();
    let context = egui::Context::default();
    configure_style(
        &context,
        AppearancePreferences {
            text_scale: 150,
            ..Default::default()
        },
    );
    let mut clubhouse = Clubhouse::default();
    let mut settings = save.settings.clone();
    let mut names = BTreeMap::new();
    let mut selected = None;
    let mut error = None;
    let mut confirmation = None;
    let mut bulk = false;
    let mut time = 0.0;
    for page in crate::clubhouse::journal::PAGES {
        let mut tab = page;
        for _ in 0..3 {
            time += 1.0;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(940.0, 720.0),
                )),
                time: Some(time),
                ..Default::default()
            };
            let mut outcome = SettingsOutcome::default();
            let mut output = context.run_ui(input, |ui| {
                draw_settings(
                    ui,
                    &mut settings,
                    &mut tab,
                    &mut error,
                    &save.settings,
                    "/example/colony.json",
                    &monitors,
                    &[],
                    false,
                    &UpdateStatus::Idle,
                    false,
                    &save.creatures,
                    &save.relationships,
                    &mut names,
                    &mut selected,
                    &mut clubhouse,
                    &save,
                    &mut confirmation,
                    &mut bulk,
                    &mut outcome,
                )
            });
            output.textures_delta.clear();
        }
    }
    let [width, height] = context.fonts(|fonts| fonts.font_image_size());
    assert!(
        width * height <= 2048 * 128,
        "the font atlas grew to {width}x{height}"
    );
}

fn key(key: egui::Key, modifiers: egui::Modifiers) -> Vec<egui::Event> {
    vec![
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        },
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        },
    ]
}

fn shown(h: &Harness, wanted: &str) -> bool {
    h.labels.iter().any(|(text, _)| text == wanted)
}

fn shown_starting(h: &Harness, wanted: &str) -> bool {
    h.labels.iter().any(|(text, _)| text.starts_with(wanted))
}

/// The notebook opens on Today, and a page can be turned to by number or one at a time from the
/// keyboard — but never while a text field has the keyboard.
#[test]
fn pages_turn_from_the_keyboard_but_not_while_typing() {
    let mut h = Harness::new(SettingsTab::default());
    assert_eq!(h.tab, SettingsTab::Today);
    h.frame(key(egui::Key::Num5, egui::Modifiers::COMMAND));
    assert_eq!(h.tab, SettingsTab::Journal);
    h.frame(key(egui::Key::CloseBracket, egui::Modifiers::COMMAND));
    assert_eq!(h.tab, SettingsTab::Habitat);
    h.frame(key(egui::Key::OpenBracket, egui::Modifiers::COMMAND));
    h.frame(key(egui::Key::OpenBracket, egui::Modifiers::COMMAND));
    assert_eq!(h.tab, SettingsTab::Home);
    // The first page goes no further back, and the last no further on.
    h.frame(key(egui::Key::Num1, egui::Modifiers::COMMAND));
    h.frame(key(egui::Key::OpenBracket, egui::Modifiers::COMMAND));
    assert_eq!(h.tab, SettingsTab::Today);
    h.frame(key(egui::Key::Num9, egui::Modifiers::COMMAND));
    h.frame(key(egui::Key::CloseBracket, egui::Modifiers::COMMAND));
    assert_eq!(h.tab, SettingsTab::About);
    // A number typed without the modifier is just a number.
    h.frame(key(egui::Key::Num2, egui::Modifiers::NONE));
    assert_eq!(h.tab, SettingsTab::About);
    // Typing in the journal's search field leaves the page where it is.
    h.frame(key(egui::Key::Num5, egui::Modifiers::COMMAND));
    h.frame(key(egui::Key::F, egui::Modifiers::COMMAND));
    h.frame(Vec::new());
    assert!(
        h.context.text_edit_focused(),
        "⌘F puts the keyboard in the search field"
    );
    h.frame(key(egui::Key::Num2, egui::Modifiers::COMMAND));
    assert_eq!(h.tab, SettingsTab::Journal);
}

/// Every tab can be reached with Tab and turned to with Enter or Space, so the notebook can be
/// used without a pointer.
#[test]
fn every_tab_can_be_reached_and_turned_to_from_the_keyboard() {
    let mut h = Harness::new(SettingsTab::Today);
    h.frame(Vec::new());
    let mut reached = std::collections::BTreeSet::new();
    for _ in 0..120 {
        h.frame(key(egui::Key::Tab, egui::Modifiers::NONE));
        let before = h.tab;
        h.frame(key(egui::Key::Enter, egui::Modifiers::NONE));
        if h.tab != before {
            reached.insert(crate::clubhouse::journal::page_number(h.tab));
            // Back to the first page, and on to find the next tab along.
            h.tab = SettingsTab::Today;
        }
        if reached.len() + 1 == crate::clubhouse::journal::PAGES.len() {
            break;
        }
    }
    assert_eq!(
        reached.into_iter().collect::<Vec<_>>(),
        (2..=crate::clubhouse::journal::PAGES.len()).collect::<Vec<_>>(),
        "every other page's tab is reachable from Today"
    );
}

/// A new colony's Today page says it has only just moved in, rather than showing empty boxes.
#[test]
fn a_brand_new_colony_is_welcomed_on_today_rather_than_shown_empty_sections() {
    let now = time::OffsetDateTime::now_utc();
    let desktop = DesktopSnapshot::default();
    let mut h = Harness::new(SettingsTab::Today);
    h.save = World::new([3; 32], now, &desktop).save;
    h.save.companion.onboarding_complete = true;
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(
        shown(&h, "Settling in"),
        "{:?}",
        h.labels.iter().map(|x| &x.0).collect::<Vec<_>>()
    );
    assert!(shown_starting(&h, "Nothing observed yet."));
    assert!(shown_starting(&h, "Day 1 of the colony."));
    h.click("Meet your companions");
    h.frame(Vec::new());
    assert_eq!(h.tab, SettingsTab::Colony);
}

/// Observations are only ever said with their evidence beside them.
#[test]
fn today_says_what_it_has_observed_and_why() {
    let mut h = Harness::new(SettingsTab::Today);
    let climber = h.save.creatures[0].name.clone();
    h.save.creatures[0].memory.ledge_seconds = 2 * 60 * 60 + 15 * 60;
    h.save.creatures[0].memory.window_climbs = 18;
    for other in h.save.creatures.iter_mut().skip(1) {
        other.memory.ledge_seconds = 0;
    }
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(shown(&h, &format!("{climber} prefers high places.")));
    assert!(shown_starting(&h, "2 h 15 min up on window ledges in all"));
    assert!(!shown_starting(&h, "Nothing observed yet."));
}

/// What is new is marked on the cover and the reader asked to have it counted as read only once
/// they turn to Today or the Journal; it stays on show while they read, and is let go of after.
#[test]
fn news_is_marked_until_it_is_read_and_stays_on_show_while_it_is() {
    let mut h = Harness::new(SettingsTab::Colony);
    let now = time::OffsetDateTime::now_utc();
    h.save.companion.journal_seen_until = h.save.companion.journal.iter().map(|e| e.at).max();
    h.save.companion.journal.push(JournalEntry {
        at: now,
        creature: None,
        moment: JournalMoment::Revisit("Wren".into(), 3),
    });
    let outcome = h.frame(Vec::new());
    assert!(
        !outcome.mark_journal_read,
        "nothing is read from another page"
    );
    assert!(shown(&h, "Today •"), "the Today tab carries its dot");
    assert!(shown(&h, "Journal •"));
    h.click("Today •");
    let outcome = h.frame(Vec::new());
    assert!(outcome.mark_journal_read);
    // The app marks it read; the page keeps showing what was new.
    assert!(h.save.companion.mark_read());
    h.frame(Vec::new());
    assert!(shown(&h, "NEW SINCE YOU LAST LOOKED"));
    assert!(shown(&h, "Wren came back to visit, for the third time"));
    assert!(shown(&h, "Today"), "the dot goes once it is read");
    // Turning elsewhere lets it go: coming back, there is nothing new.
    h.click("Your colony");
    h.click("Today");
    h.frame(Vec::new());
    assert!(!shown(&h, "NEW SINCE YOU LAST LOOKED"));
}

/// The journal can be searched and filtered by kind, says how much of it is showing, and offers
/// a way out of a search that matches nothing.
#[test]
fn the_journal_is_searched_filtered_and_cleared() {
    let mut h = Harness::new(SettingsTab::Journal);
    let name = h.save.creatures[0].name.clone();
    let at = h.save.created_at_utc + time::Duration::days(3);
    for (offset, moment) in [
        (0, JournalMoment::Discovery),
        (1, JournalMoment::Ritual(RitualKind::Dance)),
        (2, JournalMoment::Habit(Habit::LooksFoodOver)),
    ] {
        h.save.companion.journal.push(JournalEntry {
            at: at + time::Duration::hours(offset),
            creature: Some(h.save.creatures[0].id),
            moment,
        });
    }
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(shown(&h, "MILESTONES"));
    let dance = "The colony shared a dance";
    assert!(shown(&h, dance));
    h.clubhouse.journal.search = "treasure".into();
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(shown(&h, &format!("{name} found a little treasure")));
    assert!(!shown(&h, dance));
    assert!(shown_starting(&h, "Showing 1 of "));
    // A search that matches nothing says so, and clears in one click.
    h.clubhouse.journal.search = "zebra".into();
    h.click("Clear the search and filters");
    h.frame(Vec::new());
    assert!(h.clubhouse.journal.search.is_empty());
    assert!(shown(&h, dance));
    // By kind.
    h.click("Together");
    h.frame(Vec::new());
    assert_eq!(h.clubhouse.journal.kind, Some(MomentKind::Together));
    assert!(shown(&h, dance));
    assert!(!shown(&h, &format!("{name} found a little treasure")));
}

/// Empty journals and guest books say what will appear there and how, instead of nothing.
#[test]
fn an_empty_journal_and_guest_book_explain_themselves() {
    let mut h = Harness::new(SettingsTab::Journal);
    h.save.companion.journal.clear();
    h.save.companion.pins.clear();
    h.save.visitors = VisitorState::default();
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(shown(&h, "The story is just beginning"));
    assert!(shown(&h, "No visitors yet"));
}

/// A save that fails is shown calmly on every page, with something to do about it; it is the
/// app that clears it once a save works again.
#[test]
fn save_trouble_is_shown_with_ways_to_act_on_it() {
    for tab in [SettingsTab::Today, SettingsTab::Home, SettingsTab::About] {
        let mut h = Harness::new(tab);
        h.frame(Vec::new());
        assert!(!shown(&h, "SAVING HAS STOPPED FOR NOW"));
        h.clubhouse.recovery.save_trouble =
            Some("The disk is full, so the colony could not be written.".into());
        h.frame(Vec::new());
        assert!(shown(&h, "SAVING HAS STOPPED FOR NOW"));
        assert!(h.click("Try again now").retry_save);
        h.clubhouse.recovery.save_trouble = Some("x".into());
        assert!(h.click("Export a backup elsewhere…").export_colony);
    }
}

/// A returning visitor's line in the guest book says how often they have been.
#[test]
fn the_guest_book_remembers_how_often_a_visitor_has_come() {
    let mut h = Harness::new(SettingsTab::Journal);
    let again = h.save.visitors.guest_book[5].clone();
    h.save.visitors.guest_book.push(GuestBookEntry {
        visited_at_utc: again.visited_at_utc + time::Duration::days(1),
        ..again.clone()
    });
    h.click(&format!(
        "Visitors before this · {}",
        h.save.visitors.guest_book.len()
    ));
    h.frame(Vec::new());
    assert!(
        h.labels
            .iter()
            .any(|(text, _)| text.ends_with("· 2 visits in the book")),
        "{:?}",
        h.labels.iter().map(|x| &x.0).collect::<Vec<_>>()
    );
}

/// ⌘W closes the notebook from any page, as a native window would.
#[test]
fn command_w_asks_for_the_notebook_to_close() {
    let mut h = Harness::new(SettingsTab::Home);
    h.frame(Vec::new());
    assert!(!h.frame(Vec::new()).close_notebook);
    assert!(
        h.frame(key(egui::Key::W, egui::Modifiers::COMMAND))
            .close_notebook
    );
}

/// Today compares itself with the rest of the week only once yesterday was counted, each line
/// says what it rests on, and a pair who sought each other out is named.
#[test]
fn today_compares_itself_with_yesterday_once_yesterday_was_counted() {
    let mut h = Harness::new(SettingsTab::Today);
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(shown(&h, "TODAY, COMPARED"));
    assert!(shown_starting(
        &h,
        "The notebook began counting the colony's days today"
    ));
    let local = time::OffsetDateTime::now_utc().to_offset(clubhouse::local_offset());
    let day = local.date().to_julian_day();
    let (a, b) = (h.save.creatures[0].id, h.save.creatures[1].id);
    h.save.day_book.count_home(day - 1, 600);
    for a_began in [true, false, true] {
        h.save.day_book.count_pair(
            day,
            a,
            b,
            Some(a_began),
            formiga_core::SharedMomentKind::Greeting,
            true,
        );
    }
    // The page keeps what it read for half a minute; a new page state reads it at once.
    h.clubhouse.today = Default::default();
    h.frame(Vec::new());
    h.frame(Vec::new());
    let (first, second) = formiga_core::canonical_creature_pair(a, b).unwrap();
    let name = |id| {
        h.save
            .creatures
            .iter()
            .find(|creature| creature.id == id)
            .unwrap()
            .name
            .clone()
    };
    assert!(
        shown(
            &h,
            &format!(
                "{} and {} sought each other out 3 times today",
                name(first),
                name(second)
            )
        ),
        "{:?}",
        h.labels.iter().map(|(text, _)| text).collect::<Vec<_>>()
    );
    let (by_first, by_second) = if first == a { (2, 1) } else { (1, 2) };
    assert!(shown(
        &h,
        &format!(
            "{} went looking {}, {} {}.",
            name(first),
            if by_first == 2 { "twice" } else { "once" },
            name(second),
            if by_second == 2 { "twice" } else { "once" }
        )
    ));
}

/// The notebook follows the system's own appearance while it is open: when the preference is to
/// match the system and the system turns dark, the notebook applies it on its next frame, and the
/// palette with it; a fixed preference ignores the system. The native appearance switch itself
/// stays a manual check; this is the notebook's half of it.
#[test]
fn the_notebook_follows_the_system_into_the_dark_and_back() {
    let system = AppearancePreferences {
        theme: ThemeChoice::System,
        ..AppearancePreferences::default()
    };
    let light = Some(egui::Theme::Light);
    let dark = Some(egui::Theme::Dark);
    assert!(appearance_changed(None, None, system, light), "first frame");
    assert!(!appearance_changed(Some(system), light, system, light));
    assert!(
        appearance_changed(Some(system), light, system, dark),
        "the system turned dark"
    );
    let fixed = AppearancePreferences {
        theme: ThemeChoice::Light,
        ..AppearancePreferences::default()
    };
    assert!(!appearance_changed(Some(fixed), light, fixed, dark));
    // And applying it really does move the palette both ways.
    let context = egui::Context::default();
    for (theme, want_dark) in [(dark, true), (light, false), (dark, true)] {
        let mut input = egui::RawInput {
            system_theme: theme,
            ..egui::RawInput::default()
        };
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        ));
        let mut output = context.run_ui(input, |_| {});
        output.textures_delta.clear();
        configure_style(&context, system);
        assert_eq!(crate::clubhouse::dark_interface(), want_dark, "{theme:?}");
    }
    configure_style(&context, AppearancePreferences::default());
}
