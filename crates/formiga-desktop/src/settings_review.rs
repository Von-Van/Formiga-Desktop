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
    // Mallow eating up on a window and a friend munching on the ground, so Right now is
    // reviewed naming a meal wherever it is eaten, the longest of them wrapping.
    let appetite = meals::Appetite {
        picky: false,
        greedy: false,
        sleepy: false,
        favourite: meals::Food::Apple,
    };
    for (index, food, window) in [
        (0, meals::Food::Soup, Some(7)),
        (1, meals::Food::Breadstick, None),
    ] {
        if let Some(creature) = world.save.creatures.get_mut(index) {
            creature.state.action = ActionKind::Eat;
            creature.state.meal = Some(meals::Meal::plan_seeded(food, 8.0, appetite, 3));
            creature.state.surface.window_key = window;
        }
    }
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
            let context = notebook_context();
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
                            drawn: None,
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
                    // The Home page is the heaviest, with its 672x96 row of the village and the
                    // ground under it, at most 640x48.
                    assert!(bytes <= 644 * 1024, "UI artwork exceeded budget: {bytes}");
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
    /// The last frame as a picture, when one was asked for with `picture`.
    pictured: Option<image::RgbaImage>,
    picture_next: bool,
    /// What the last frame told screen readers, once `context.enable_accesskit()` asks it to.
    access: Option<egui::accesskit::TreeUpdate>,
}
impl Harness {
    fn new(tab: SettingsTab) -> Self {
        let (save, monitors) = fixture();
        let context = notebook_context();
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
            pictured: None,
            picture_next: false,
            access: None,
        }
    }

    /// The window as it is now, drawn as the review pictures are.
    fn picture(&mut self) -> image::RgbaImage {
        self.picture_next = true;
        self.frame(Vec::new());
        self.pictured.take().expect("the frame was pictured")
    }

    /// Turns the mouse wheel over the page by `points`, downward, and lets the scroll settle.
    fn scroll_page(&mut self, points: f32) {
        let over = egui::pos2(self.screen.x * 0.6, self.screen.y * 0.5);
        self.frame(vec![egui::Event::PointerMoved(over)]);
        self.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -points),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
        }]);
        for _ in 0..40 {
            self.frame(Vec::new());
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
        self.access = output.platform_output.accesskit_update.take();
        self.labels.clear();
        self.rects.clear();
        self.cut = cut_off(&output.shapes);
        for shape in &output.shapes {
            collect_labels(&shape.shape, &mut self.labels);
            collect_rects(&shape.shape, &mut self.rects);
        }
        apply_textures(&mut self.textures, &output.textures_delta);
        output.textures_delta.clear();
        if std::mem::take(&mut self.picture_next) {
            let jobs = self
                .context
                .tessellate(output.shapes, output.pixels_per_point);
            self.pictured = Some(rasterize(
                &jobs,
                &self.textures,
                self.screen.x as u32,
                self.screen.y as u32,
            ));
        }
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

    /// Where the top of `label` was drawn last frame.
    fn text_top(&self, label: &str) -> f32 {
        self.labels
            .iter()
            .find(|(text, _)| text == label)
            .unwrap_or_else(|| panic!("No visible label {label:?}"))
            .1
            .top()
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
            drawn: None,
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
    h.click("Routines");
    let (index, preset) = h.click("Save current preferences").save_mode.unwrap();
    assert_eq!(index, 0);
    assert!(preset.reduce_motion);
    h.tab = SettingsTab::Home;
    h.clubhouse
        .shell
        .sections
        .open_section(SettingsTab::Home, "where");
    assert_eq!(
        h.click("Bottom left").home_corner,
        Some(HomeCorner::BottomLeft)
    );
    assert_eq!(
        h.click("Always on the desktop").village_stays_out,
        Some(true)
    );
    h.save.home.stays_out = true;
    assert_eq!(h.click("Come and go").village_stays_out, Some(false));
    h.save.home.stays_out = false;
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
    h.clubhouse
        .shell
        .sections
        .open_section(SettingsTab::Home, "belongings");
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
    h.clubhouse
        .shell
        .sections
        .open_section(SettingsTab::Home, "pictures");
    assert!(h.click("Export colony portrait…").export_colony_card);

    h.tab = SettingsTab::Colony;
    let creature = h.save.creatures[0].id;
    h.click("Care & share");
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
    // And it can still be given up, so a slot is never lost to something invisible: its pin
    // stays drawn on a kept moment.
    let spot = h
        .clubhouse
        .journal
        .pin_spots
        .iter()
        .find(|(at, _)| *at == first.at)
        .map(|(_, rect)| rect.center())
        .expect("the kept moment's pin");
    let outcome = h.click_at(spot);
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
        let context = notebook_context();
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
        assert!((body - 16.0 * f32::from(scale) / 100.0).abs() < 0.01);
        assert!((10.0..=24.0).contains(&body));
    }
    configure_style(&h.context, AppearancePreferences::default());
}

/// Every texture the settings window may hold at once. The largest are the Home page's row of the
/// village, 672x96 with a cell for each house so each can wear its resident's curtain (252 KiB),
/// and the ground it stands on, its two daylit layers only as long as the village (120 KiB at
/// most);
/// the Collection of 160 keepsakes, of which the pages hold only the resting half of the sheet
/// (160 KiB); the object sheet, with every garden at every stage, fifteen hangout spots, fifteen
/// ornaments and the village's props (112 KiB); a companion trying something on, in four poses
/// (36 KiB), and everything there is to wear pictured on that companion, one strip of twenty boxes
/// (40 KiB); and the village tree the creature studio stands its large preview beside, one 64x64
/// texture shared by every candidate (16 KiB).
const ARTWORK_BUDGET: usize = 1060 * 1024;

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
                // Looking at each companion in turn is how all four portraits come to be drawn,
                // and opening the last one's wardrobe how it comes to try something on.
                h.clubhouse.shell.sections.close(tab);
                for index in 0..h.save.creatures.len() {
                    h.selected = Some(h.save.creatures[index].id);
                    h.frame(Vec::new());
                    h.frame(Vec::new());
                }
                h.clubhouse.shell.sections.open_section(tab, "wardrobe");
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
                            drawn: None,
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
    // village atlas, the ground it stands on, the object atlas, one sheet holding every trinket,
    // and the last companion looked at trying something on in four poses, with everything to
    // wear pictured on it, caught in its newest habit on its page, and its portrait there playing
    // what it is doing.
    assert_eq!(textures, 4 + 4 + 1 + 1 + 1 + 1 + 1 + 4 + 1 + 1 + 1);
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
                drawn: None,
            },
        );
    }
    let mut output = h.context.run_ui(egui::RawInput::default(), |ui| {
        for c in &h.save.creatures {
            let _ = h.clubhouse.shell.portrait_texture(ui.ctx(), c);
        }
        h.clubhouse.home.show(
            ui,
            &mut h.clubhouse.shell,
            &h.save,
            &h.monitors,
            crate::clubhouse::FormigaHomeView::default(),
            &mut h.clubhouse.colony,
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
        h.labels
            .iter()
            .any(|(text, _)| *text == format!("{celebration}.")),
        "a companion with no habits yet still has its own celebration"
    );
    h.save.creatures[0].memory.habits = vec![Habit::CirclesBeforeNaps, Habit::WavesHello];
    h.frame(Vec::new());
    let wanted = format!("{celebration}. Turns in circles before a nap. Waves hello.");
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
    h.clubhouse
        .shell
        .sections
        .open_section(SettingsTab::Home, "ground");
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
    h.clubhouse
        .shell
        .sections
        .open_section(SettingsTab::Home, "where");
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
    h.clubhouse
        .shell
        .sections
        .open_section(SettingsTab::Home, "pictures");
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
    h.click("Something to wear");
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
            .filter(|rect| rect.size() == egui::vec2(36.0, 36.0))
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

#[test]
fn the_ruling_is_where_the_writing_is_however_far_the_page_is_scrolled() {
    use crate::clubhouse::journal::{RuleGrid, ruled_lines};
    // The same page, scrolled: every line has moved up with the writing, the first is a whole
    // line below the top of the writing, and the window is still ruled from top to bottom.
    let (top, bottom, pitch) = (40.0, 640.0, 24.0);
    for scrolled in [0.0, 13.0, 100.0, 1000.0] {
        let grid = RuleGrid {
            top: top - scrolled,
            pitch,
        };
        let lines = ruled_lines(grid, top, bottom);
        assert!(!lines.is_empty());
        assert!(
            lines[0] >= top && lines[0] < top + pitch + 0.01,
            "{scrolled}"
        );
        assert!(lines[0] >= grid.top + pitch - 0.01, "{scrolled}");
        for line in &lines {
            let pitches = (line - grid.top) / pitch;
            assert!(
                (pitches - pitches.round()).abs() < 1e-3,
                "{scrolled}: {line}"
            );
            assert!(*line >= top && *line < bottom);
        }
    }
    // Pictures of pages scrolled part of the way down, for review.
    if let Some(path) = std::env::var_os("FORMIGA_UI_REVIEW_DIR").map(std::path::PathBuf::from) {
        std::fs::create_dir_all(&path).unwrap();
        for tab in [
            SettingsTab::Colony,
            SettingsTab::Journal,
            SettingsTab::General,
        ] {
            let mut h = Harness::new(tab);
            h.screen = egui::vec2(940.0, 720.0);
            h.frame(Vec::new());
            h.scroll_page(115.0);
            h.picture()
                .save(path.join(format!("{tab:?}-940-cream-100-scrolled.png")))
                .unwrap();
        }
        // A section open, as it reads on its own.
        for (tab, section) in [
            (SettingsTab::Colony, "care"),
            (SettingsTab::General, "desktop"),
        ] {
            let mut h = Harness::new(tab);
            h.screen = egui::vec2(940.0, 720.0);
            h.clubhouse.shell.sections.open_section(tab, section);
            h.frame(Vec::new());
            h.picture()
                .save(path.join(format!("{tab:?}-940-cream-100-{section}.png")))
                .unwrap();
        }
        // The wardrobe and the Collection with everything found, so every choice has its picture
        // and every keepsake its colours; the wardrobe trying something on.
        for (section, theme) in [
            ("wardrobe", ThemeChoice::Light),
            ("wardrobe", ThemeChoice::Dark),
            ("collection", ThemeChoice::Light),
        ] {
            // The collection is kept with the village's keepsakes.
            let mut h = Harness::new(if section == "wardrobe" {
                SettingsTab::Colony
            } else {
                SettingsTab::Home
            });
            // Tall enough for the page's list of sections to be on it.
            h.screen = egui::vec2(940.0, 1400.0);
            configure_style(
                &h.context,
                AppearancePreferences {
                    theme,
                    ..AppearancePreferences::default()
                },
            );
            h.save.companion.scrapbook = (0..TRINKET_VARIANTS)
                .map(|variant| ScrapbookRecord {
                    variant,
                    first_at: h.save.created_at_utc,
                    finder: Some(h.save.creatures[0].id),
                    finder_name: h.save.creatures[0].name.clone(),
                })
                .collect();
            h.frame(Vec::new());
            h.click(if section == "wardrobe" {
                "Something to wear"
            } else {
                "The collection"
            });
            h.frame(Vec::new());
            if section == "wardrobe" {
                let chip = h
                    .labels
                    .iter()
                    .find(|(text, _)| text == AccessoryKind::MushroomCap.label())
                    .map(|(_, rect)| rect.center())
                    .expect("the toadstool cap is a choice");
                h.frame(vec![egui::Event::PointerMoved(chip)]);
                h.frame(Vec::new());
            }
            h.frame(Vec::new());
            h.picture()
                .save(path.join(format!("{:?}-940-{theme:?}-100-{section}.png", h.tab)))
                .unwrap();
            configure_style(&h.context, AppearancePreferences::default());
        }
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
fn a_section_opens_on_its_own_and_goes_back_to_where_the_page_was_left() {
    let mut h = Harness::new(SettingsTab::Colony);
    h.screen = egui::vec2(940.0, 1000.0);
    h.frame(Vec::new());
    h.frame(Vec::new());
    // The page opens on who the companion is, with a line for each of the rest.
    assert!(shown(&h, "Mallow"));
    assert!(shown(&h, "Something to wear"));
    assert!(!shown(&h, "Save name"), "the name is in its own section");
    h.scroll_page(400.0);
    let list = h.text_top("Care & share");
    // Opened, the section stands alone under a way back, and the companion's summary is gone.
    h.click("Care & share");
    h.frame(Vec::new());
    assert!(shown(&h, "Copy seed"));
    assert!(shown(&h, "\u{2039} Your colony"));
    assert!(!shown(&h, "Something to wear"));
    // Back finds the list where it was left.
    h.click("\u{2039} Your colony");
    h.frame(Vec::new());
    assert!(!shown(&h, "Copy seed"));
    assert!((h.text_top("Care & share") - list).abs() < 1.0);
    // Escape goes back the same way, but not while something is being typed.
    h.click("Care & share");
    h.frame(Vec::new());
    assert!(shown(&h, "Save name"));
    h.frame(key(egui::Key::Escape, egui::Modifiers::NONE));
    h.frame(Vec::new());
    assert!(!shown(&h, "Save name"));
    assert_eq!(h.clubhouse.shell.sections.open(SettingsTab::Colony), None);
    // Other pages keep sections of their own.
    h.tab = SettingsTab::About;
    h.screen = egui::vec2(940.0, 1400.0);
    h.click("Privacy");
    h.tab = SettingsTab::Colony;
    h.frame(Vec::new());
    assert!(
        shown(&h, "Something to wear"),
        "Your colony is still on its list"
    );
    // A step of the tour pointing into a section opens it.
    h.save.companion.onboarding_complete = false;
    h.clubhouse.shell.take_the_tour();
    while h.clubhouse.shell.tour.pointing_at() != Some(TourMark::Share) {
        h.click("Next");
    }
    h.frame(Vec::new());
    assert!(shown(&h, "Copy seed"), "the tour opened Share");
}

/// The sections each page keeps beyond its first view, by the keys the page opens them by.
fn sections_of(page: SettingsTab) -> &'static [&'static str] {
    match page {
        SettingsTab::Colony => &["wardrobe", "together", "care"],
        SettingsTab::Home => &["ground", "where", "pictures", "collection", "belongings"],
        SettingsTab::General => &["desktop", "routines", "notebook"],
        SettingsTab::About => &["backups", "apps", "updates", "privacy", "kept"],
        _ => &[],
    }
}

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
    let context = notebook_context();
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
                drawn: None,
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
            // Every page as it opens, and every section of it opened in turn.
            let views = crate::clubhouse::journal::PAGES
                .into_iter()
                .flat_map(|page| {
                    std::iter::once((page, None))
                        .chain(sections_of(page).iter().map(move |key| (page, Some(*key))))
                });
            for (page, section) in views {
                match section {
                    Some(key) => clubhouse.shell.sections.open_section(page, key),
                    None => clubhouse.shell.sections.close(page),
                }
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
                            "{page:?} {section:?}, {width} wide, text at {text_scale}%: {what}"
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
    SettingsTab::Studio,
    SettingsTab::Home,
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
    for _ in 0..9 {
        h.click("Next");
    }
    h.frame(Vec::new());
    assert_eq!(h.tab, SettingsTab::Studio);
    assert_eq!(tour_header(&h).as_deref(), Some("TOUR · 10 OF 19"));
    h.click("Back");
    h.frame(Vec::new());
    assert_eq!(h.tab, SettingsTab::Colony);
    assert_eq!(tour_header(&h).as_deref(), Some("TOUR · 9 OF 19"));
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
fn creature_size_is_chosen_by_its_word_or_its_picture_and_keeps_its_words() {
    let open = |h: &mut Harness| {
        h.clubhouse
            .shell
            .sections
            .open_section(SettingsTab::General, "desktop");
        h.frame(Vec::new());
        h.frame(Vec::new());
    };
    let mut h = Harness::new(SettingsTab::General);
    h.context.enable_accesskit();
    open(&mut h);
    // Each word chooses its size, and a screen reader hears the three words, one chosen.
    for (scale, name) in [(2, "Small"), (4, "Large"), (3, "Medium")] {
        h.click(name);
        h.frame(Vec::new());
        assert_eq!(h.draft.display_scale, scale, "{name}");
        let access = h.access.as_ref().expect("accesskit is on");
        for (other, label) in [(2, "Small"), (3, "Medium"), (4, "Large")] {
            let node = access
                .nodes
                .iter()
                .map(|(_, node)| node)
                .find(|node| node.label() == Some(label))
                .unwrap_or_else(|| panic!("no {label} for screen readers"));
            assert_eq!(node.role(), egui::accesskit::Role::Button, "{label}");
            assert_eq!(
                node.toggled(),
                Some(if other == scale {
                    egui::accesskit::Toggled::True
                } else {
                    egui::accesskit::Toggled::False
                }),
                "{label}"
            );
        }
    }
    // The companion beside a word is part of its choice: Small's, just left of the word.
    let small = h
        .labels
        .iter()
        .find(|(text, _)| text == "Small")
        .expect("Small is shown")
        .1;
    h.click_at(egui::pos2(small.left() - 6.0, small.bottom() - 4.0));
    h.frame(Vec::new());
    assert_eq!(h.draft.display_scale, 2);
    // The portrait is the one the notebook keeps: choosing again draws nothing new.
    let artwork = h.artwork_bytes();
    let textures = h.clubhouse.texture_ids().len();
    h.click("Large");
    h.frame(Vec::new());
    assert_eq!(h.draft.display_scale, 4);
    assert_eq!(h.artwork_bytes(), artwork);
    assert_eq!(h.clubhouse.texture_ids().len(), textures);
    // With nobody in the colony yet, the words alone still choose.
    let mut empty = Harness::new(SettingsTab::General);
    empty.save.creatures.clear();
    open(&mut empty);
    empty.click("Small");
    empty.frame(Vec::new());
    assert_eq!(empty.draft.display_scale, 2);
    assert!(empty.clubhouse.texture_ids().is_empty());
    // Pictures of the row for review, in both themes and at the largest text.
    if let Some(path) = std::env::var_os("FORMIGA_UI_REVIEW_DIR").map(std::path::PathBuf::from) {
        std::fs::create_dir_all(&path).unwrap();
        for appearance in [
            AppearancePreferences::default(),
            AppearancePreferences {
                theme: ThemeChoice::Dark,
                text_scale: 150,
                sprite_outline: false,
            },
        ] {
            let mut h = Harness::new(SettingsTab::General);
            h.screen = egui::vec2(940.0, 1200.0);
            configure_style(&h.context, appearance);
            open(&mut h);
            h.picture()
                .save(path.join(format!(
                    "General-940-{:?}-{}-creature-size.png",
                    appearance.theme, appearance.text_scale
                )))
                .unwrap();
        }
    }
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
    let context = notebook_context();
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
    // The pencil hand's two weights, laid out by egui as any font is, cost a second strip.
    assert!(
        width * height <= 2048 * 256,
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
        shown(&h, "Settling in!"),
        "{:?}",
        h.labels.iter().map(|x| &x.0).collect::<Vec<_>>()
    );
    assert!(shown_starting(&h, "Nothing I can vouch for yet."));
    assert!(shown_starting(&h, "Day 1!"));
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
    assert!(shown(&h, &format!("{climber} loves high places!")));
    assert!(shown_starting(
        &h,
        "I've clocked 2 h 15 min up on window ledges"
    ));
    assert!(!shown_starting(&h, "Nothing I can vouch for yet."));
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
    assert!(shown(&h, "NEW"), "the highlight is stamped as new");
    assert!(shown(&h, "Wren came back to visit, for the third time"));
    assert!(shown(&h, "Today"), "the dot goes once it is read");
    // Turning elsewhere lets it go: coming back, there is nothing new.
    h.click("Your colony");
    h.click("Today");
    h.frame(Vec::new());
    assert!(!shown(&h, "NEW"));
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
    // The search and the filters wait behind the magnifier, and ⌘F brings them out.
    assert!(!shown(&h, "Every kind"));
    h.frame(key(egui::Key::F, egui::Modifiers::COMMAND));
    h.frame(Vec::new());
    assert!(shown(&h, "Every kind"));
    assert!(h.clubhouse.journal.searching);
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

/// An empty journal is the colony on its plate and one line, with no empty boxes.
#[test]
fn an_empty_journal_and_guest_book_explain_themselves() {
    let mut h = Harness::new(SettingsTab::Journal);
    h.save.companion.journal.clear();
    h.save.companion.pins.clear();
    h.save.visitors = VisitorState::default();
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(
        h.labels.iter().any(|(text, _)| text.starts_with("Day ")
            && text.ends_with("Nothing written down yet; I'll jot things here as they happen.")),
        "{:?}",
        h.labels.iter().map(|x| &x.0).collect::<Vec<_>>()
    );
    assert!(shown(&h, "RIGHT NOW"), "the colony on its plate");
    for empty in ["THE GUEST BOOK", "No visitors yet", "Nothing found yet"] {
        assert!(!shown(&h, empty), "{empty} on an empty journal");
    }
}

/// Formiga Home is offered for the house picked out on the Home page only while it is installed:
/// to open it, or, while it is the one open, to bring its household back out. While another
/// house is open it is shown but cannot be opened.
#[test]
fn a_picked_house_offers_formiga_home_only_while_it_is_installed() {
    use crate::clubhouse::{FormigaHomeView, arrange::Picked};
    let mut h = Harness::new(SettingsTab::Home);
    let owners: Vec<CreatureId> =
        formiga_core::house_owners(&h.save.creatures, &h.save.home.cottage_order)
            .as_slice()
            .to_vec();
    let keeper = owners[0];
    h.clubhouse.home.arrange.picked = Some(Picked::House(keeper));
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(!shown(&h, "Open in Formiga Home"), "nothing without Home");
    h.clubhouse.formiga_home = FormigaHomeView {
        installed: true,
        open: None,
        can_open: true,
    };
    assert_eq!(h.click("Open in Formiga Home").open_house, Some(keeper));
    h.clubhouse.formiga_home = FormigaHomeView {
        installed: true,
        open: Some(keeper),
        can_open: false,
    };
    assert!(h.click("Bring them back out").bring_household_back);
    if let Some(other) = owners.get(1) {
        h.clubhouse.home.arrange.picked = Some(Picked::House(*other));
        let outcome = h.click("Open in Formiga Home");
        assert_eq!(outcome.open_house, None, "one house at a time");
    }
}

/// Formiga Farm is offered only while it is installed: on a companion's page to reshape it, and
/// in the studio to draw a new one. While a companion is open in it, its page says so instead,
/// and a sketch drawn there is adopted with the design Farm drew it in.
#[test]
fn formiga_farm_is_offered_only_while_it_is_installed() {
    use crate::clubhouse::FormigaFarmView;
    use crate::farm::FarmRequest;
    use formiga_core::forms::{Design, Form, Plan, Sculpt};
    let mut h = Harness::new(SettingsTab::Colony);
    h.screen = egui::vec2(760.0, 6000.0);
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(
        !shown(&h, "Reshape in Formiga Farm"),
        "nothing without Farm"
    );
    let installed = FormigaFarmView {
        installed: true,
        open: None,
        can_open: true,
    };
    h.clubhouse.formiga_farm = installed;
    let id = h.selected.expect("a companion's page");
    h.click("Care & share");
    assert_eq!(
        h.click("Reshape in Formiga Farm").open_farm,
        Some(FarmRequest::Reshape(id))
    );
    h.clubhouse.formiga_farm = FormigaFarmView {
        installed: true,
        open: Some(FarmRequest::Reshape(id)),
        can_open: false,
    };
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(!shown(&h, "Reshape in Formiga Farm"), "open there already");

    h.tab = SettingsTab::Studio;
    h.clubhouse.formiga_farm = FormigaFarmView::default();
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(
        !shown(&h, "Draw one in Formiga Farm"),
        "nothing without Farm"
    );
    h.clubhouse.formiga_farm = installed;
    assert_eq!(
        h.click("Draw one in Formiga Farm").open_farm,
        Some(FarmRequest::Draw)
    );
    let seed = [9; 32];
    let mut creature =
        World::preview_adult(seed, h.save.maximum_seen_utc, &DesktopSnapshot::default());
    let design = Design {
        form: Form::Sculpted {
            sculpt: Sculpt::starter(Plan::Floater),
        },
        face: creature.appearance.face,
    };
    formiga_farm_contract::apply_design(&mut creature, &design);
    h.clubhouse.studio.push_preview(
        &h.context,
        GenerationPreview {
            shared: None,
            creature: creature.clone(),
            source_seed: seed,
            similarity: None,
            summary: String::new(),
            drawn: Some(Box::new(design.clone())),
        },
    );
    h.save.creatures.truncate(2);
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(shown(&h, "Floater"), "named by its own plan");
    assert_eq!(
        h.click("Adopt into colony").accept_creature_preview,
        Some(PreviewAcceptance::Drawn {
            source_seed: seed,
            recipe: creature.appearance.design,
            design: Box::new(design),
            replace: None,
        })
    );
}

/// The journal has no place for souvenirs until one comes home from Formiga Hill. Then each is
/// shown, newest first, with Desktop's own name and line for it, and its picture is given back
/// with the rest of the artwork when the menu closes.
#[test]
fn souvenirs_appear_in_the_journal_only_once_one_has_come_home() {
    let mut h = Harness::new(SettingsTab::Journal);
    // An otherwise empty journal, so the section is on screen without scrolling.
    h.save.companion.journal.clear();
    h.save.companion.pins.clear();
    h.save.companion.scrapbook.clear();
    h.save.visitors = VisitorState::default();
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(!shown_starting(&h, "SOUVENIRS"));
    let textures = h.clubhouse.texture_ids().len();
    let at = h.save.created_at_utc + time::Duration::days(2);
    h.save.trips.souvenirs = vec![
        SouvenirRecord {
            souvenir: Souvenir::PicnicRibbon,
            brought_home_at_utc: at,
        },
        SouvenirRecord {
            souvenir: Souvenir::FairTicket,
            brought_home_at_utc: at + time::Duration::days(1),
        },
    ];
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(shown(&h, "SOUVENIRS · 2 FROM FORMIGA HILL"));
    let top = |name: &str| {
        h.labels
            .iter()
            .find(|(text, _)| text == name)
            .map(|(_, rect)| rect.top())
            .unwrap_or_else(|| panic!("{name} is not shown"))
    };
    assert!(
        top(Souvenir::FairTicket.name()) < top(Souvenir::PicnicRibbon.name()),
        "the newest comes first"
    );
    assert!(shown(&h, Souvenir::PicnicRibbon.description()));
    assert!(shown_starting(&h, "Brought home from Formiga Hill · "));
    assert_eq!(
        h.clubhouse.texture_ids().len(),
        textures + 1,
        "one picture holds every souvenir"
    );
    h.clubhouse.release_images();
    assert!(h.clubhouse.texture_ids().is_empty());
}

/// The journal has no place for Formiga Battle's cups until a keepsake comes home from a match.
/// Then each trophy and medal is shown, newest first, under the Handler's name, the cups won and
/// the points, and their picture is given back with the rest of the artwork.
#[test]
fn battle_cups_appear_in_the_journal_only_once_one_has_come_home() {
    use formiga_core::{BattleHandlerCard, BattleKeepsake, BattleKeepsakeRecord, BattlePennant};
    let mut h = Harness::new(SettingsTab::Journal);
    h.save.companion.journal.clear();
    h.save.companion.pins.clear();
    h.save.companion.scrapbook.clear();
    h.save.visitors = VisitorState::default();
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(!shown_starting(&h, "CUPS"));
    let textures = h.clubhouse.texture_ids().len();
    let at = h.save.created_at_utc + time::Duration::days(2);
    h.save.battle.keepsakes = vec![
        BattleKeepsakeRecord {
            keepsake: BattleKeepsake::BackyardCupTrophy,
            earned_at_utc: at,
        },
        BattleKeepsakeRecord {
            keepsake: BattleKeepsake::BackyardCupMedal,
            earned_at_utc: at + time::Duration::days(1),
        },
    ];
    h.save.battle.record.circuit_points = 15;
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(shown(&h, "CUPS · 2 FROM FORMIGA BATTLE"));
    assert!(shown(&h, "1 cup won · 15 circuit points"));
    let top = |h: &Harness, name: &str| {
        h.labels
            .iter()
            .find(|(text, _)| text == name)
            .map(|(_, rect)| rect.top())
            .unwrap_or_else(|| panic!("{name} is not shown"))
    };
    assert!(
        top(&h, BattleKeepsake::BackyardCupMedal.name())
            < top(&h, BattleKeepsake::BackyardCupTrophy.name()),
        "the newest comes first"
    );
    assert!(shown(&h, BattleKeepsake::BackyardCupTrophy.description()));
    assert!(shown_starting(&h, "Won in Formiga Battle · "));
    assert_eq!(
        h.clubhouse.texture_ids().len(),
        textures + 1,
        "one picture holds every cup"
    );
    h.save.battle.record.handler = Some(BattleHandlerCard {
        name: "Jo".to_owned(),
        club: None,
        pennant: BattlePennant {
            shape: "swallowtail".to_owned(),
            field: "forest".to_owned(),
            symbol: "acorn".to_owned(),
            trim: "brass".to_owned(),
        },
    });
    h.frame(Vec::new());
    assert!(shown(&h, "Handler Jo · 1 cup won · 15 circuit points"));
    if let Some(path) = std::env::var_os("FORMIGA_UI_REVIEW_DIR").map(std::path::PathBuf::from) {
        std::fs::create_dir_all(&path).unwrap();
        h.screen = egui::vec2(940.0, 1200.0);
        h.frame(Vec::new());
        h.picture()
            .save(path.join("Journal-940-cream-100-cups.png"))
            .unwrap();
    }
    h.clubhouse.release_images();
    assert!(h.clubhouse.texture_ids().is_empty());
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
    h.screen = egui::vec2(1000.0, 4000.0);
    let again = h.save.visitors.guest_book[5].clone();
    h.save.visitors.guest_book.push(GuestBookEntry {
        visited_at_utc: again.visited_at_utc + time::Duration::days(1),
        ..again.clone()
    });
    // The latest few are shown under the book's heading, and the rest when asked for.
    h.frame(Vec::new());
    assert!(shown(
        &h,
        &format!(
            "VISITORS BEFORE THIS · {}",
            h.save.visitors.guest_book.len()
        )
    ));
    let codes = |h: &Harness| h.labels.iter().filter(|(t, _)| t == "Copy code").count();
    let before = codes(&h);
    h.click(&format!("Show all {}", h.save.visitors.guest_book.len()));
    h.frame(Vec::new());
    assert!(
        codes(&h) > before,
        "the rest of the book is shown once asked for"
    );
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
    assert!(shown(&h, "VS. THE REST OF THE WEEK"));
    assert!(shown_starting(&h, "I only started counting days today"));
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
                "{} and {} went looking for each other 3 times today!",
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
            "{} {}, {} {}.",
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
    let context = notebook_context();
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

/// The About page lists Formiga's other apps: what each is, whether it is here or where to get
/// it, a newer one when one is out, and a button for the two the colony travels to. Farm is
/// listed only once it is installed, since it is not offered anywhere yet.
#[test]
fn the_about_page_lists_formigas_apps() {
    use crate::clubhouse::{AppListing, AppOpen};
    let listing = |name, installed: Option<&str>, offered, open| AppListing {
        name,
        blurb: "What it is.",
        how: "How it is opened.".to_owned(),
        installed: installed.map(|version| Some(version.to_owned())),
        newer: None,
        offered,
        open,
    };
    let mut h = Harness::new(SettingsTab::About);
    h.clubhouse.apps = vec![
        listing(
            "Formiga Hill",
            Some("0.67.14"),
            true,
            Some((AppOpen::Hill, true)),
        ),
        listing("Formiga Home", None, true, None),
        listing("Formiga Farm", None, false, None),
        listing(
            "Formiga Battle",
            Some("0.67.11"),
            true,
            Some((AppOpen::Battle, false)),
        ),
    ];
    h.clubhouse.apps[3].newer = Some(("0.68.0".to_owned(), "https://example.invalid".to_owned()));
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(
        shown(&h, "A newer Formiga Battle is out"),
        "said on the list"
    );
    h.click("Formiga's apps");
    h.frame(Vec::new());
    assert!(shown(&h, "Installed · 0.67.14"));
    assert!(shown(&h, "Get Formiga Home at formigaworld.com"));
    assert!(
        !shown(&h, "Formiga Farm"),
        "Farm is not listed until it is installed"
    );
    assert!(shown(
        &h,
        "Formiga Battle 0.68.0 is out. This computer has 0.67.11."
    ));
    assert_eq!(h.click("Go to Formiga Hill").open_app, Some(AppOpen::Hill));
    assert_eq!(
        h.click("Play Formiga Battle").open_app,
        None,
        "not while the colony cannot go"
    );

    h.clubhouse.apps[2].installed = Some(None);
    h.frame(Vec::new());
    assert!(shown(&h, "Formiga Farm"), "listed once it is installed");
    // A picture of the section, for review.
    if let Some(path) = std::env::var_os("FORMIGA_UI_REVIEW_DIR").map(std::path::PathBuf::from) {
        std::fs::create_dir_all(&path).unwrap();
        h.screen = egui::vec2(940.0, 1100.0);
        h.frame(Vec::new());
        h.picture()
            .save(path.join("About-940-cream-100-apps.png"))
            .unwrap();
    }
}

/// The colophon's title is Echo Fox Mode's switch: a click asks for the mode, and while the
/// colony is foxes the page says so and another click asks for everybody back.
#[test]
fn the_colophon_title_switches_echo_fox_mode() {
    let mut h = Harness::new(SettingsTab::About);
    let title = format!("Formiga {APP_VERSION}");
    assert_eq!(h.click(&title).echo_fox, Some(true));
    for creature in &mut h.save.creatures {
        creature.appearance.echo_fox = true;
    }
    assert_eq!(h.click(&title).echo_fox, Some(false));
    assert!(
        h.labels
            .iter()
            .any(|(text, _)| text.starts_with("Echo Fox Mode")),
        "the page says the mode is on"
    );
}

/// The Today page of a colony with a full day behind it, pictured whole for review when
/// `FORMIGA_UI_REVIEW_DIR` is set: a new friendship still unread, a find, a shared moment and a
/// returning visitor today, and a pair the day book counted seeking each other out.
#[test]
fn a_full_day_is_laid_out_on_today() {
    let mut h = Harness::new(SettingsTab::Today);
    let now = time::OffsetDateTime::now_utc();
    let local = now.to_offset(clubhouse::local_offset());
    let midnight = local.date().midnight().assume_offset(local.offset());
    let at = |minutes: i64| (midnight + time::Duration::minutes(minutes)).min(now);
    let (a, b, c) = (
        h.save.creatures[0].id,
        h.save.creatures[1].id,
        h.save.creatures[2].id,
    );
    h.save.companion.journal_seen_until = Some(at(0));
    for (minutes, creature, moment) in [
        (10 * 60 + 31, Some(c), JournalMoment::Discovery),
        (
            12 * 60 + 15,
            None,
            JournalMoment::Ritual(RitualKind::Picnic),
        ),
        (14 * 60 + 2, Some(a), JournalMoment::Friendship(b)),
        (16 * 60 + 40, None, JournalMoment::Revisit("Pip".into(), 3)),
    ] {
        h.save.companion.journal.push(JournalEntry {
            at: at(minutes),
            creature,
            moment,
        });
    }
    h.save.companion.scrapbook.push(ScrapbookRecord {
        variant: 9,
        first_at: at(10 * 60 + 31),
        finder: Some(c),
        finder_name: h.save.creatures[2].name.clone(),
    });
    let day = local.date().to_julian_day();
    h.save.day_book.count_home(day - 1, 600);
    for a_began in [true, true, true, true, false, false] {
        h.save.day_book.count_pair(
            day,
            a,
            b,
            Some(a_began),
            formiga_core::SharedMomentKind::Greeting,
            true,
        );
    }
    h.screen = egui::vec2(1152.0, 1700.0);
    h.frame(Vec::new());
    h.frame(Vec::new());
    let picture = h.picture();
    assert!(shown(&h, "TODAY'S HIGHLIGHT"));
    if let Some(path) = std::env::var_os("FORMIGA_UI_REVIEW_DIR").map(std::path::PathBuf::from) {
        std::fs::create_dir_all(&path).unwrap();
        picture.save(path.join("Today-full-day.png")).unwrap();
    }
}

/// Right now keeps each companion's name on the line above the ground and its portrait level
/// with the others', however long what it is doing runs and however big the text: a long
/// activity wraps downward into the ground, never up past it. Pictured for review when
/// `FORMIGA_UI_REVIEW_DIR` is set.
#[test]
fn a_long_activity_in_right_now_stays_below_the_ground_line() {
    let mut raised = Vec::new();
    for (text_scale, width) in [(100, 940.0), (150, 760.0)] {
        let mut h = Harness::new(SettingsTab::Today);
        configure_style(
            &h.context,
            AppearancePreferences {
                text_scale,
                ..Default::default()
            },
        );
        for (index, creature) in h.save.creatures.iter_mut().enumerate() {
            creature.state.surface.window_key = None;
            creature.state.action = [
                ActionKind::Idle,
                ActionKind::Traverse,
                ActionKind::PetReaction,
                ActionKind::Homebound,
            ][index % 4];
        }
        h.screen = egui::vec2(width, 1400.0);
        h.frame(Vec::new());
        h.frame(Vec::new());
        let picture = h.picture();
        let tops: Vec<f32> = h
            .save
            .creatures
            .iter()
            .map(|creature| h.text_top(&creature.name))
            .collect();
        if let Some(path) = std::env::var_os("FORMIGA_UI_REVIEW_DIR").map(std::path::PathBuf::from)
        {
            std::fs::create_dir_all(&path).unwrap();
            picture
                .save(path.join(format!("Today-right-now-{text_scale}.png")))
                .unwrap();
        }
        if tops.iter().any(|top| (top - tops[0]).abs() >= 0.5) {
            raised.push(format!("text at {text_scale}%: {tops:?}"));
        }
    }
    assert!(raised.is_empty(), "every name sits on one line: {raised:?}");
}

#[test]
fn applying_a_notebook_change_keeps_a_pause_made_from_the_tray() {
    // The notebook opened on the colony's preferences and its size was changed, not applied.
    let saved = Settings::default();
    let mut draft = saved.clone();
    draft.display_scale = 4;
    // Meanwhile the tray paused the colony and switched launch at login on.
    let mut current = saved.clone();
    current.paused = true;
    current.launch_at_login = true;
    let rebased = rebase_preferences(&saved, &draft, &current);
    assert!(rebased.paused, "Apply would have un-paused the colony");
    assert!(rebased.launch_at_login);
    assert_eq!(
        rebased.display_scale, 4,
        "the notebook's own change is kept"
    );
    assert_eq!(pending_changes(&current, &rebased), vec!["size"]);
}

#[test]
fn a_habitat_applied_from_the_editor_is_not_left_waiting_to_apply() {
    let saved = Settings::default();
    let mut current = saved.clone();
    current.habitat.preset = HabitatPreset::Custom;
    // The editor hands its result to the notebook's draft as it finishes.
    let mut draft = saved.clone();
    draft.habitat = current.habitat.clone();
    let rebased = rebase_preferences(&saved, &draft, &current);
    assert_eq!(rebased, current);
    assert!(pending_changes(&current, &rebased).is_empty());
}

#[test]
fn a_change_made_in_the_notebook_wins_over_the_same_one_made_elsewhere() {
    let saved = Settings::default();
    let mut draft = saved.clone();
    draft.paused = true;
    let mut current = saved.clone();
    current.reduce_motion = true;
    let rebased = rebase_preferences(&saved, &draft, &current);
    assert!(rebased.paused && rebased.reduce_motion);
    // Unchanged in both: the colony's value, which is the same.
    assert_eq!(rebased.visible, current.visible);
}

/// A companion's profile offers a snack and a toy as plain buttons, the same two offers the
/// strip over it on the desktop makes, so they can be reached from the keyboard: Tab to one and
/// press Enter. What came of it is written on the profile, for that companion only.
#[test]
fn snacks_and_toys_can_be_offered_from_the_profile_by_keyboard() {
    let mut h = Harness::new(SettingsTab::Colony);
    let mallow = h.save.creatures[0].id;
    assert_eq!(
        h.click("Offer a snack").offer,
        Some((mallow, OfferKind::Snack))
    );
    assert_eq!(h.click("Offer a toy").offer, Some((mallow, OfferKind::Toy)));
    // From the keyboard: Tab along until the snack button has focus, then Enter.
    let target = h.text_top("Offer a snack");
    let key = |key: egui::Key, pressed: bool| egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Default::default(),
    };
    let mut reached = false;
    for _ in 0..200 {
        h.frame(vec![key(egui::Key::Tab, true), key(egui::Key::Tab, false)]);
        let focused = h
            .context
            .memory(|memory| memory.focused())
            .and_then(|id| h.context.read_response(id))
            .map(|response| response.rect);
        if focused.is_some_and(|rect| {
            rect.top() <= target
                && target <= rect.bottom()
                && h.labels
                    .iter()
                    .any(|(text, at)| text == "Offer a snack" && rect.contains(at.center()))
        }) {
            reached = true;
            break;
        }
    }
    assert!(reached, "Tab never reached Offer a snack");
    let outcome = h.frame(vec![
        key(egui::Key::Enter, true),
        key(egui::Key::Enter, false),
    ]);
    assert_eq!(
        outcome.offer,
        Some((mallow, OfferKind::Snack)),
        "Enter offers it"
    );
    // The answer, in words, on this companion's profile and nobody else's.
    h.clubhouse.colony.offer_note = Some((mallow, "Mallow took the snack.".into()));
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(
        h.labels
            .iter()
            .any(|(text, _)| text == "Mallow took the snack.")
    );
    if let Some(directory) = std::env::var_os("FORMIGA_UI_REVIEW_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        h.picture()
            .save(directory.join("profile-offers.png"))
            .unwrap();
    }
    h.selected = Some(h.save.creatures[1].id);
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(
        !h.labels
            .iter()
            .any(|(text, _)| text == "Mallow took the snack.")
    );
}

/// A wonder the colony has found is put out on the desktop from the journal and put away again,
/// and no more than three can be out.
#[test]
fn wonders_are_put_out_and_put_away_from_the_journal() {
    let mut h = Harness::new(SettingsTab::Journal);
    h.screen = egui::vec2(940.0, 4000.0);
    let now = h.save.maximum_seen_utc;
    let found = [
        WonderKind::Chair,
        WonderKind::Fountain,
        WonderKind::Seesaw,
        WonderKind::Hammock,
    ];
    for kind in found {
        h.save.companion.wonders.push(formiga_core::WonderRecord {
            kind,
            first_at: now,
            finder: None,
            finder_name: "Mallow".into(),
            goes: 1,
        });
    }
    assert_eq!(
        h.click("Put it out").put_out_wonder,
        Some(WonderKind::Chair)
    );
    let out = |kind| formiga_core::PlacedWonder {
        kind,
        display: DisplayKey([1; 16]),
        along: 0.5,
    };
    h.save.companion.placed_wonders = vec![out(WonderKind::Chair)];
    assert_eq!(h.click("Put away").put_away_wonder, Some(WonderKind::Chair));
    // With three out, the rest cannot be put out until one is put away.
    h.save.companion.placed_wonders = vec![
        out(WonderKind::Chair),
        out(WonderKind::Fountain),
        out(WonderKind::Seesaw),
    ];
    assert_eq!(h.click("Put it out").put_out_wonder, None);
}

/// A colony `day` days old, for picturing the notebook through a colony's life: on day 1 nothing
/// but the founders' arrival; by day 10 a first friendship, a find, a habit and a few shared
/// moments; by day 110 a colony with a history. Everything in it is something the colony would
/// have recorded itself.
fn aged_fixture(day: i64) -> SaveFile {
    use time::Duration;
    // Dated from the real clock, as the notebook reads it, so day 1 is today.
    let now = time::OffsetDateTime::now_utc();
    let (_, monitors) = fixture();
    let desktop = DesktopSnapshot {
        monitors,
        ..Default::default()
    };
    let founded = now - Duration::days(day - 1) - Duration::minutes(1);
    let mut world = World::new([25; 32], founded, &desktop);
    world.tick(now, 0.05, &desktop);
    let save = &mut world.save;
    save.companion.onboarding_complete = true;
    for (creature, name) in save
        .creatures
        .iter_mut()
        .zip(["Mallow", "Tofu", "Pebble", "Poppy"])
    {
        creature.name = name.into();
    }
    if day < 10 {
        return world.save;
    }
    let ids: Vec<CreatureId> = save.creatures.iter().map(|c| c.id).collect();
    let (a, b, c) = (ids[0], ids[1], ids[2 % ids.len()]);
    let on = |days: i64, hour: i64| founded + Duration::days(days) + Duration::hours(hour);
    let scale = |n: i64| (n * day / 10) as u32;
    let mallow = &mut save.creatures[0];
    mallow.memory.times_petted = scale(4);
    mallow.memory.discoveries_found = scale(1);
    mallow.memory.window_climbs = scale(2);
    mallow.memory.habits = vec![Habit::LooksFoodOver];
    let mut moments = vec![
        (on(2, 4), Some(a), JournalMoment::Friendship(b)),
        (on(3, 2), Some(c), JournalMoment::Discovery),
        (
            on(4, 6),
            Some(a),
            JournalMoment::Habit(Habit::LooksFoodOver),
        ),
        (on(6, 1), None, JournalMoment::Ritual(RitualKind::GroupNap)),
        (on(8, 5), Some(b), JournalMoment::Discovery),
    ];
    save.companion.scrapbook.push(ScrapbookRecord {
        variant: 9,
        first_at: on(3, 2),
        finder: Some(c),
        finder_name: save.creatures[2 % save.creatures.len()].name.clone(),
    });
    if day >= 100 {
        moments.extend([
            (on(14, 3), None, JournalMoment::Visit("Pip".into())),
            (on(21, 2), Some(c), JournalMoment::Friendship(a)),
            (
                on(30, 4),
                Some(a),
                JournalMoment::Preference(ProfileDescriptor::LovesHighPlaces),
            ),
            (on(41, 1), Some(b), JournalMoment::Wonder(WonderKind::Chair)),
            (on(55, 7), None, JournalMoment::Trip),
            (on(70, 2), Some(a), JournalMoment::Discovery),
            (on(90, 5), None, JournalMoment::Revisit("Pip".into(), 2)),
            (
                on(day - 2, 6),
                Some(a),
                JournalMoment::Habit(Habit::WavesHello),
            ),
            (
                on(day - 1, 2),
                None,
                JournalMoment::Ritual(RitualKind::Picnic),
            ),
        ]);
        save.creatures[0].memory.habits.push(Habit::WavesHello);
        save.companion.scrapbook.push(ScrapbookRecord {
            variant: 3,
            first_at: on(70, 2),
            finder: Some(a),
            finder_name: "Mallow".into(),
        });
    }
    for (at, creature, moment) in moments {
        save.companion.journal.push(JournalEntry {
            at,
            creature,
            moment,
        });
    }
    save.companion.journal.sort_by_key(|entry| entry.at);
    save.companion.journal_seen_until = save.companion.journal.last().map(|entry| entry.at);
    let (low, high) = (a.min(b), a.max(b));
    match save
        .relationships
        .iter_mut()
        .find(|r| (r.a, r.b) == (low, high))
    {
        Some(bond) => bond.affinity = 140,
        None => save.relationships.push(CreatureRelationship {
            a: low,
            b: high,
            affinity: 140,
            familiarity: 160,
            playfulness: 40,
            avoidance: 0,
        }),
    }
    if let Some(pair) = tally_mut_or_insert(&mut save.tallies, a, b) {
        pair.tally.shared_rests = scale(1) as u16;
        pair.tally.greetings = scale(3) as u16;
        pair.tally.sought = [scale(2) as u16, scale(1) as u16];
        pair.tally.memory = Some(PairMemory {
            at: now - Duration::hours(4),
            kind: SharedMomentKind::Rest,
        });
    }
    world.save
}

impl Harness {
    /// The notebook open on `tab` over a colony of its own rather than the usual fixture.
    fn over(tab: SettingsTab, save: SaveFile) -> Self {
        let mut h = Self::new(tab);
        h.selected = Some(save.creatures[0].id);
        h.draft = save.settings.clone();
        h.save = save;
        h
    }
}

/// Today, Your colony and the Journal of a colony on its first day, its tenth and its hundred
/// and tenth, in both themes, at both text sizes, in the widest and narrowest windows: what the
/// notebook is judged on. Each is drawn whole, and pictured when `FORMIGA_UI_REVIEW_DIR` is set.
#[test]
fn the_notebook_through_a_colonys_life() {
    let output = std::env::var_os("FORMIGA_UI_REVIEW_DIR").map(std::path::PathBuf::from);
    let mut cut = Vec::new();
    for day in [1, 10, 110] {
        let save = aged_fixture(day);
        // Your colony both as it opens, on everyone, and turned to the first companion.
        for (tab, everyone) in [
            (SettingsTab::Today, false),
            (SettingsTab::Colony, true),
            (SettingsTab::Colony, false),
            (SettingsTab::Journal, false),
        ] {
            let name = if everyone {
                "Everyone".to_owned()
            } else {
                format!("{tab:?}")
            };
            for (theme, text_scale, width) in [
                (ThemeChoice::Light, 100, 940.0),
                (ThemeChoice::Light, 150, 760.0),
                (ThemeChoice::Dark, 100, 940.0),
                (ThemeChoice::Dark, 150, 760.0),
            ] {
                let mut h = Harness::over(tab, save.clone());
                if everyone {
                    h.selected = None;
                }
                configure_style(
                    &h.context,
                    AppearancePreferences {
                        theme,
                        text_scale,
                        sprite_outline: false,
                    },
                );
                h.screen = egui::vec2(width, 720.0);
                h.frame(Vec::new());
                h.frame(Vec::new());
                if !h.cut.is_empty() {
                    cut.push(format!(
                        "day {day}, {name} at {width} and {text_scale}%: {:?}",
                        h.cut
                    ));
                }
                for unwanted in ["Apply changes", "Revert", "Preferences are saved"] {
                    assert!(!shown(&h, unwanted), "{unwanted} under {tab:?}");
                }
                if let Some(path) = &output {
                    std::fs::create_dir_all(path).unwrap();
                    h.picture()
                        .save(path.join(format!(
                            "life-day{day}-{name}-{width}-{}-{text_scale}.png",
                            if theme == ThemeChoice::Dark {
                                "charcoal"
                            } else {
                                "cream"
                            },
                        )))
                        .unwrap();
                }
            }
        }
    }
    assert!(cut.is_empty(), "{cut:#?}");
}

/// Apply and Revert are under the pages with preferences on them, and nowhere else unless a
/// preference is waiting to be applied.
#[test]
fn apply_and_revert_are_only_under_the_preferences() {
    for tab in crate::clubhouse::journal::PAGES {
        let mut h = Harness::new(tab);
        h.frame(Vec::new());
        h.frame(Vec::new());
        let preference_page = matches!(
            tab,
            SettingsTab::General | SettingsTab::Habitat | SettingsTab::Applications
        );
        assert_eq!(shown(&h, "Apply changes"), preference_page, "{tab:?}");
        // A preference changed and not yet applied follows the reader to every page.
        h.draft.reduce_motion = !h.draft.reduce_motion;
        h.frame(Vec::new());
        h.frame(Vec::new());
        assert!(shown(&h, "Apply changes"), "{tab:?} with a change waiting");
    }
}

/// A companion's page and the colony's are written as a notebook, not a register: no specimen
/// numbers, no figure numbers, no body type, and what it is doing in the notebook's own words.
#[test]
fn a_companion_is_not_a_specimen() {
    let mut h = Harness::new(SettingsTab::Colony);
    h.save.creatures[0].state.action = ActionKind::SoloPlay;
    h.save.creatures[0].state.surface.window_key = None;
    h.frame(Vec::new());
    h.frame(Vec::new());
    for unwanted in [
        "Specimen Nº",
        "Fig. Nº",
        "Body:",
        "specimens observed",
        "SPECIMEN Nº",
    ] {
        assert!(
            !h.labels.iter().any(|(text, _)| text.contains(unwanted)),
            "{unwanted} on the page"
        );
    }
    assert!(shown(&h, "Playing with a toy."));
    for action in ActionKind::ALL {
        let label = activity_label(action);
        assert!(
            label.starts_with(|c: char| c.is_lowercase()),
            "{action:?} reads {label:?}"
        );
        assert_ne!(
            label,
            crate::clubhouse::words(&format!("{action:?}")).to_lowercase(),
            "{action:?} is named as the code names it"
        );
    }
    let mut colony = h.save.creatures.clone();
    for creature in &mut colony {
        creature.state.action = ActionKind::Traverse;
    }
    let count = crate::clubhouse::today::count_word(colony.len());
    assert_eq!(
        crate::clubhouse::journal::colony_observation(&colony),
        format!("{count} of them, all awake.")
    );
    colony[0].state.action = ActionKind::Sleep;
    assert_eq!(
        crate::clubhouse::journal::colony_observation(&colony),
        format!("{count} of them, one of them napping.")
    );
}

/// No date in the notebook is written as an ISO date.
#[test]
fn every_date_on_the_journal_is_written_one_way() {
    let mut h = Harness::over(SettingsTab::Journal, aged_fixture(110));
    h.screen = egui::vec2(940.0, 6000.0);
    h.frame(Vec::new());
    h.frame(Vec::new());
    let iso = |text: &str| {
        text.as_bytes()
            .windows(10)
            .any(|w| w[4] == b'-' && w[7] == b'-' && w[..4].iter().all(u8::is_ascii_digit))
    };
    let dated: Vec<&String> = h
        .labels
        .iter()
        .map(|(text, _)| text)
        .filter(|text| iso(text))
        .collect();
    assert!(dated.is_empty(), "{dated:?}");
}

/// The companion is the first thing on its page, at every size: its portrait and its name in the
/// first screen, beside its temperament and traits, at the widest window and smallest text and at
/// the narrowest and largest.
#[test]
fn the_companion_is_above_the_fold() {
    for (text_scale, width) in [(100, 940.0), (150, 760.0)] {
        let mut h = Harness::new(SettingsTab::Colony);
        configure_style(
            &h.context,
            AppearancePreferences {
                text_scale,
                ..Default::default()
            },
        );
        h.screen = egui::vec2(width, 560.0);
        h.frame(Vec::new());
        h.frame(Vec::new());
        let phrase = h.save.creatures[0].temperament_phrase();
        assert!(shown(&h, &phrase), "{phrase} at {width} and {text_scale}%");
        let fold = h.screen.y - 60.0;
        let top = h.text_top(&phrase);
        assert!(
            top < fold,
            "the phrase is at {top}, under the fold at {fold}"
        );
        for gone in ["TALLY · LIFE HERE", "days together"] {
            assert!(!shown(&h, gone), "{gone} is still on the page");
        }
    }
}

/// A companion is chosen by its face, and the arrow keys and ⌘[ ⌘] turn between companions;
/// past the last one, ⌘] turns the page.
#[test]
fn companions_are_chosen_by_face_and_turned_from_the_keyboard() {
    let mut h = Harness::new(SettingsTab::Colony);
    h.screen = egui::vec2(940.0, 720.0);
    let ids: Vec<CreatureId> = h.save.creatures.iter().map(|c| c.id).collect();
    assert!(ids.len() >= 2, "the fixture has company");
    let second = h.save.creatures[1].name.clone();
    // The face's name sits under it; clicking the name's face turns to it.
    h.frame(Vec::new());
    h.frame(Vec::new());
    let at = h
        .labels
        .iter()
        .find(|(text, _)| *text == second)
        .expect("a face in the strip")
        .1;
    h.click_at(at.center() - egui::vec2(0.0, 20.0));
    h.frame(Vec::new());
    assert_eq!(h.selected, Some(ids[1]));
    h.frame(key(egui::Key::ArrowLeft, egui::Modifiers::NONE));
    assert_eq!(h.selected, Some(ids[0]), "left turns back");
    h.frame(key(egui::Key::CloseBracket, egui::Modifiers::COMMAND));
    assert_eq!(h.selected, Some(ids[1]), "⌘] turns on");
    h.selected = ids.last().copied();
    h.frame(key(egui::Key::CloseBracket, egui::Modifiers::COMMAND));
    assert_eq!(h.selected, ids.last().copied());
    assert_eq!(h.tab, SettingsTab::Studio, "past the last, the page turns");
    // Before the first companion is everyone together, and before that the page turns back.
    h.tab = SettingsTab::Colony;
    h.selected = Some(ids[0]);
    h.frame(key(egui::Key::OpenBracket, egui::Modifiers::COMMAND));
    assert_eq!(h.selected, None, "⌘[ from the first turns to everyone");
    h.frame(key(egui::Key::OpenBracket, egui::Modifiers::COMMAND));
    assert_eq!(h.tab, SettingsTab::Today);
}

/// A companion that has just arrived still has a page: its portrait, who it is, that it arrived
/// today, and a line saying more will be written — no empty boxes.
#[test]
fn a_new_companion_still_has_a_page() {
    let save = aged_fixture(1);
    let name = save.creatures[0].name.clone();
    let mut h = Harness::over(SettingsTab::Colony, save);
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(shown(&h, "Arrived today"));
    assert!(shown(
        &h,
        &format!("Still getting to know {name}. I'll write things here as I see them.")
    ));
}

/// A companion's page writes about it in the notebook's voice: the latest thing seen, dated,
/// first; its friends as faces with a word under each, which turn to that friend; its firsts;
/// and its little ways. Nothing is written as a field name.
#[test]
fn a_companions_page_speaks() {
    let save = aged_fixture(110);
    let mallow = save.creatures[0].id;
    let mut h = Harness::over(SettingsTab::Colony, save);
    h.screen = egui::vec2(940.0, 1400.0);
    h.frame(Vec::new());
    h.frame(Vec::new());
    let texts: Vec<&String> = h.labels.iter().map(|(text, _)| text).collect();
    assert!(
        texts.iter().any(|t| t.starts_with("Napped beside Tofu ")),
        "the newest shared moment comes first: {texts:?}"
    );
    assert!(texts.iter().any(|t| t.starts_with("Joined ")), "{texts:?}");
    assert!(
        texts
            .iter()
            .any(|t| t.contains("first friend Tofu") && t.contains("first find")),
        "{texts:?}"
    );
    for field in ["Favorite region:", "Bond:", "Closest friend:", "Habits:"] {
        assert!(
            !texts.iter().any(|t| t.starts_with(field)),
            "{field} on the page"
        );
    }
    assert!(shown(&h, "Friends"));
    assert!(shown(&h, "cozy"), "Tofu's word: {texts:?}");
    let word = h.labels.iter().find(|(t, _)| t == "cozy").unwrap().1;
    h.click_at(word.center() - egui::vec2(0.0, 24.0));
    h.frame(Vec::new());
    assert!(
        h.selected.is_some_and(|id| id != mallow),
        "a friend's face turns to that friend"
    );
}

/// A find the journal wrote down is named where the scrapbook kept it as the same companion's
/// first of its kind, within a minute.
#[test]
fn a_find_is_named_in_the_journal() {
    let mut h = Harness::new(SettingsTab::Journal);
    let finder = h.save.creatures[0].id;
    let at = h.save.maximum_seen_utc;
    let entry = JournalEntry {
        at,
        creature: Some(finder),
        moment: JournalMoment::Discovery,
    };
    let key = (0..=u8::MAX)
        .find(|v| trinket_info(*v).is_some_and(|info| info.name == "Key"))
        .unwrap();
    assert_eq!(
        clubhouse::moment_text(&h.save, &entry),
        "Mallow found a little treasure"
    );
    h.save.companion.scrapbook.push(ScrapbookRecord {
        variant: key,
        first_at: at + time::Duration::seconds(20),
        finder: Some(finder),
        finder_name: "Mallow".into(),
    });
    assert_eq!(
        clubhouse::moment_text(&h.save, &entry),
        "Mallow found a key"
    );
}

/// Everything that changes or shares a companion is in one place at the foot of its page, and the
/// collection is with the village's keepsakes; the page itself is for reading.
#[test]
fn the_controls_step_back_to_the_foot_of_the_page() {
    let mut h = Harness::new(SettingsTab::Colony);
    h.screen = egui::vec2(940.0, 3000.0);
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(shown(&h, "Something to wear"));
    assert!(shown(&h, "Care & share"));
    for gone in [
        "Name and where they like to be",
        "Share Mallow",
        "Keeping and starting over",
        "The collection",
        "Save name",
    ] {
        assert!(!shown(&h, gone), "{gone} is still on the companion's page");
    }
    assert!(
        h.text_top("Care & share") > h.text_top("Something to wear"),
        "Care & share is at the foot"
    );
    h.click("Care & share");
    h.frame(Vec::new());
    for there in ["Save name", "Copy seed", "Export creature card…"] {
        assert!(shown(&h, there), "{there} is under Care & share");
    }
    h.tab = SettingsTab::Home;
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(shown(&h, "The collection"), "the collection is on Home");
}

/// A companion is renamed where its name is written: click the name, type, and Enter keeps it;
/// Escape leaves it as it was.
#[test]
fn a_companion_is_renamed_where_its_name_is_written() {
    let mut h = Harness::new(SettingsTab::Colony);
    h.screen = egui::vec2(940.0, 1200.0);
    let id = h.save.creatures[0].id;
    h.frame(Vec::new());
    h.frame(Vec::new());
    let heading = |h: &Harness| {
        h.labels
            .iter()
            .filter(|(text, _)| text == "Mallow")
            .max_by(|a, b| a.1.height().total_cmp(&b.1.height()))
            .expect("the name heading")
            .1
    };
    h.click_at(heading(&h).center());
    h.frame(Vec::new());
    assert!(shown(
        &h,
        "Enter keeps the new name; Esc leaves it as it was."
    ));
    let select_all = egui::Event::Key {
        key: egui::Key::A,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    };
    h.frame(vec![
        select_all.clone(),
        egui::Event::Text("Biscuit".into()),
    ]);
    let outcome = h.frame(key(egui::Key::Enter, egui::Modifiers::NONE));
    assert_eq!(outcome.rename_creature, Some((id, "Biscuit".into())));
    // Escape leaves the name as it was.
    h.frame(Vec::new());
    h.click_at(heading(&h).center());
    h.frame(vec![select_all, egui::Event::Text("Nope".into())]);
    let outcome = h.frame(key(egui::Key::Escape, egui::Modifiers::NONE));
    h.frame(Vec::new());
    assert_eq!(outcome.rename_creature, None);
    assert!(h.clubhouse.colony.renaming.is_none());
}

/// Your colony opens on everyone together: what each is doing, and the pairs who go together as
/// faces with a word between. Any face there, or on Today, turns to that companion's page.
#[test]
fn the_colony_opens_on_everyone_and_every_face_turns_to_its_page() {
    let save = aged_fixture(110);
    let mallow = save.creatures[0].id;
    let mut h = Harness::over(SettingsTab::Colony, save);
    h.selected = None;
    h.screen = egui::vec2(940.0, 1600.0);
    h.frame(Vec::new());
    h.frame(Vec::new());
    assert!(shown(&h, "Your colony"));
    assert!(shown(&h, "RIGHT NOW"));
    assert!(shown(&h, "WHO GOES WITH WHOM"));
    assert!(
        h.labels
            .iter()
            .any(|(t, _)| t.starts_with("· ") && t.ends_with(" ·")),
        "a pair with a word between"
    );
    assert!(
        !shown(&h, "Care & share"),
        "no companion's controls on the spread"
    );
    // A face on the plate turns to that companion.
    // Mallow's name is in the strip at the top, then on the plate, then among the pairs.
    let mut names: Vec<egui::Rect> = h
        .labels
        .iter()
        .filter(|(t, _)| t == "Mallow")
        .map(|(_, rect)| *rect)
        .collect();
    names.sort_by(|a, b| a.top().total_cmp(&b.top()));
    let name = names[1];
    h.click_at(name.center() - egui::vec2(0.0, 30.0));
    h.frame(Vec::new());
    assert_eq!(h.selected, Some(mallow));
    // On Today, a face in the log or the highlight turns to the companion's page too.
    h.tab = SettingsTab::Today;
    h.selected = None;
    let outcome_before = h.tab;
    h.clubhouse.shell.open_companion = Some(mallow);
    h.frame(Vec::new());
    assert_eq!(outcome_before, SettingsTab::Today);
    assert_eq!(h.tab, SettingsTab::Colony);
    assert_eq!(h.selected, Some(mallow));
}

/// Turning away from the Journal and back finds it where it was left.
#[test]
fn the_journal_reopens_where_it_was_left() {
    let mut h = Harness::over(SettingsTab::Journal, aged_fixture(110));
    h.screen = egui::vec2(940.0, 720.0);
    h.frame(Vec::new());
    h.frame(Vec::new());
    h.scroll_page(300.0);
    let first = |h: &Harness| {
        h.labels
            .iter()
            .filter(|(_, rect)| rect.top() > 60.0)
            .min_by(|a, b| a.1.top().total_cmp(&b.1.top()))
            .map(|(text, rect)| (text.clone(), rect.top()))
    };
    let left_at = first(&h);
    h.tab = SettingsTab::Today;
    h.frame(Vec::new());
    h.frame(Vec::new());
    h.tab = SettingsTab::Journal;
    for _ in 0..30 {
        h.frame(Vec::new());
    }
    assert_eq!(first(&h), left_at);
}

/// The Journal reads as days: a day with one moment is one line with its day where the time would
/// be, a moment is pinned from the glyph at its line's end, and in a narrow window at large text
/// the time goes above a moment that would otherwise wrap in beside it.
#[test]
fn the_journal_reads_as_a_diary() {
    let save = aged_fixture(110);
    let mut h = Harness::over(SettingsTab::Journal, save);
    h.screen = egui::vec2(940.0, 4000.0);
    h.frame(Vec::new());
    h.frame(Vec::new());
    let texts: Vec<&String> = h.labels.iter().map(|(t, _)| t).collect();
    assert!(
        texts
            .iter()
            .any(|t| t.contains(" · ") && t.split(" · ").nth(1).is_some_and(|c| c.len() == 5)),
        "a folded day writes its day beside its time: {texts:?}"
    );
    // Pinning, from the glyph at the end of a moment's line.
    let (at, spot) = h.clubhouse.journal.pin_spots[0];
    let outcome = h.click_at(spot.center());
    assert!(
        outcome.pin_moment.is_some_and(|entry| entry.at == at),
        "the glyph pins its moment"
    );
    // Narrow and large: the time over the moment, not beside it.
    let mut h = Harness::over(SettingsTab::Journal, aged_fixture(110));
    configure_style(
        &h.context,
        AppearancePreferences {
            text_scale: 150,
            ..Default::default()
        },
    );
    h.screen = egui::vec2(760.0, 4000.0);
    h.frame(Vec::new());
    h.frame(Vec::new());
    let line = "The colony took the train to Formiga Hill and came home";
    let text = h.labels.iter().find(|(t, _)| t == line).expect(line).1;
    assert!(
        !h.labels.iter().any(|(t, rect)| t.len() <= 20
            && t.contains(':')
            && (rect.center().y - text.center().y).abs() < 4.0
            && rect.right() < text.left()),
        "no time beside a moment that would wrap"
    );
}

/// The newest days' best moments are taped into the Journal as pictures with their caption, and
/// the scrapbook's finds are pasted in like snapshots.
#[test]
fn the_journal_has_pictures_taped_in() {
    let mut save = aged_fixture(110);
    let now = time::OffsetDateTime::now_utc();
    let (a, b) = (save.creatures[0].id, save.creatures[1].id);
    save.companion.journal.push(JournalEntry {
        at: now - time::Duration::minutes(5),
        creature: Some(a),
        moment: JournalMoment::Friendship(b),
    });
    let caption = clubhouse::moment_text(&save, save.companion.journal.last().unwrap());
    let mut h = Harness::over(SettingsTab::Journal, save);
    h.screen = egui::vec2(940.0, 4000.0);
    h.frame(Vec::new());
    h.frame(Vec::new());
    let written = h.labels.iter().filter(|(t, _)| *t == caption).count();
    assert!(
        written >= 2,
        "the moment and its picture's caption: {written}"
    );
    assert!(shown(&h, "THE SCRAPBOOK · 2 FOUND"));
    let before = h.artwork_bytes();
    h.clubhouse.release_images();
    assert!(h.clubhouse.texture_ids().is_empty());
    assert!(before > 0);
    if let Some(path) = std::env::var_os("FORMIGA_UI_REVIEW_DIR").map(std::path::PathBuf::from) {
        std::fs::create_dir_all(&path).unwrap();
        h.screen = egui::vec2(940.0, 1600.0);
        h.picture().save(path.join("Journal-pictures.png")).unwrap();
    }
}
