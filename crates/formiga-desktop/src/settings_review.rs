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
        for page in [
            SettingsTab::Colony,
            SettingsTab::Studio,
            SettingsTab::Home,
            SettingsTab::Journal,
            SettingsTab::Habitat,
            SettingsTab::General,
            SettingsTab::Applications,
            SettingsTab::About,
        ] {
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
                    clubhouse.push_preview(
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
                    assert!(bytes <= 416 * 1024, "UI artwork exceeded budget: {bytes}");
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
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                // Tall enough for the whole Home page with a full village to arrange.
                egui::vec2(1000.0, 2200.0),
            )),
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
    h.clubhouse.seed_code = encode_creature_seed(shared.into());
    let outcome = h.click("Preview shared creature");
    assert_eq!(outcome.preview_shared, Some(shared));
    assert!(outcome.accept_creature_preview.is_none());
    let creature =
        World::from_shared_creature(shared, h.save.maximum_seen_utc, &DesktopSnapshot::default())
            .save
            .creatures
            .remove(0);
    h.clubhouse.push_preview(
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
    assert!(h.click("Skip introduction").complete_onboarding);
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
    h.clubhouse.sticker_clip = StickerClip::Dance;
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
    // One arrival, kept, and then a long run of ordinary moments on top of it.
    let first = JournalEntry {
        at: now,
        creature: Some(keeper),
        moment: JournalMoment::Arrival,
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
        !h.save
            .companion
            .journal
            .iter()
            .any(|entry| entry.at == first.at),
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
/// no change); and a companion trying something on shows four poses (36 KiB): 760 KiB.
const ARTWORK_BUDGET: usize = 760 * 1024;

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
                    h.clubhouse.push_preview(
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
    // Four portraits, four candidate strips, the village atlas, the object atlas, one sheet
    // holding every trinket, and the last companion looked at trying something on in four poses.
    assert_eq!(textures, 4 + 4 + 1 + 1 + 1 + 4);
}

#[test]
fn all_ui_artwork_together_fits_the_budget() {
    let mut h = Harness::new(SettingsTab::Home);
    for index in 0..4 {
        let seed = [30 + index; 32];
        h.clubhouse.push_preview(
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
            h.clubhouse.portrait(ui, c, 48.0);
        }
        h.clubhouse
            .home(ui, &h.save, &h.monitors, &mut SettingsOutcome::default());
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
    let recap = clubhouse::today(&save, today_date, offset);
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
    assert!(clubhouse::today(&save, today_date, offset).rolled_out);
    // Another day has nothing to say about this one.
    let tomorrow = clubhouse::today(&save, today_date.next_day().unwrap(), offset);
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
    h.clubhouse.postcard_caption = "  Snacks \t for everyone\n".into();
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
    for tab in [SettingsTab::Colony, SettingsTab::Home, SettingsTab::Journal] {
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
        clubhouse.push_preview(
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
            for page in [
                SettingsTab::Colony,
                SettingsTab::Studio,
                SettingsTab::Home,
                SettingsTab::Journal,
                SettingsTab::Habitat,
                SettingsTab::General,
                SettingsTab::Applications,
                SettingsTab::About,
            ] {
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
                    for clipped in &output.shapes {
                        let bounds = clipped.shape.visual_bounding_rect();
                        let clip = clipped.clip_rect;
                        if bounds.is_positive()
                            && (bounds.right() > clip.right() + 0.5
                                || bounds.left() < clip.left() - 0.5)
                        {
                            cut.push(format!(
                                "{page:?}, {width} wide, text at {text_scale}%: {:?} at {:.0} \
                                 spans {:.0}–{:.0}, shown only {:.0}–{:.0}",
                                first_text(&clipped.shape).unwrap_or_default(),
                                bounds.top(),
                                bounds.left(),
                                bounds.right(),
                                clip.left(),
                                clip.right()
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(cut.is_empty(), "{} cut off: {cut:#?}", cut.len());
}
