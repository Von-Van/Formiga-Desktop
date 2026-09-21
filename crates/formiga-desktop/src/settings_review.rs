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
    world.save.companion.journal.push(JournalEntry {
        at: now,
        creature: None,
        moment: JournalMoment::Visit("Wanderer 0".into()),
    });
    world.save.home.decorations.decorations = ShelterDecorationKind::ALL.to_vec();
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
                egui::vec2(1000.0, 1400.0),
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
        for shape in &output.shapes {
            collect_labels(&shape.shape, &mut self.labels);
        }
        apply_textures(&mut self.textures, &output.textures_delta);
        output.textures_delta.clear();
        outcome
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
    // Four companions: adopting cannot replace the colony or silently remove an individual.
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
    assert_eq!(h.click("Leaf").hidden_decorations, Some(1));
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
/// their glint frames: one texture instead of eight, and 24 KiB more pixels, which is what moves
/// this from 416 to 432 KiB. Nothing else on any page grew.
const ARTWORK_BUDGET: usize = 432 * 1024;

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
    // Four portraits, four candidate strips, the village atlas, the object atlas, and one sheet
    // holding every trinket — sixteen of them now, for the price of one texture.
    assert_eq!(textures, 4 + 4 + 1 + 1 + 1);
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
