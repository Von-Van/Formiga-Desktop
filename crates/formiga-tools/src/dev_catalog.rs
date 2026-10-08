//! `formiga-tools dev catalog`: everything a player can come across, as Formiga's own code lists it.
//!
//!   cargo run -p formiga-tools -- dev catalog
//!
//! One JSON document: every kind of thing (decorations, finds, accessories, souvenirs, bodies…)
//! with each item's name, the identifier a save keeps, its description and hint where it has
//! them, how a player comes by it, and where its name is shown on screen and whether it fits.
//! The developer toolkit's `inspect` and `audit content` read this rather than the source, so
//! nothing about the game is worked out twice.
//!
//! How items arrive is taken from the code that decides it. Village pieces and colony objects
//! are watched arriving: a number of colonies are lived day by day through the simulation and
//! the day each piece first turns up is kept. Finds and wonders turn up only while companions are
//! out on the desktop, which a day-by-day run does not reach, so for those the circumstance and
//! hint the catalogue gives are reported instead. Accessories are checked against the rule that
//! decides them: worn only once the find each is made from has been found.

use epaint::{Color32, FontFamily, FontId, Fonts, TextOptions, text::FontDefinitions};
use formiga_core::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use time::{Duration, OffsetDateTime};

/// How many colonies are lived through to watch village pieces and objects arrive, and for how
/// long. Every village piece has arrived in every colony well before this.
pub(crate) const REACH_COLONIES: u8 = 24;
pub(crate) const REACH_DAYS: i64 = 240;
const REACH_START: OffsetDateTime = time::macros::datetime!(2026-01-05 09:00 UTC);

/// Where the village's ground pieces are named on screen: the tiles of the Village page's
/// "Put something down" catalogue (formiga-desktop/src/clubhouse/arrange.rs, `ground_catalogue`).
/// Each tile is 96 by 78; the name is laid out in the Small text style, wrapped to the tile less
/// four points either side, and sits with its foot four points above the tile's bottom edge.
/// The picture above it is 40 high and centred 26 below the tile's top, so its foot is 46 down.
/// The name has the 28 points between the two.
pub(crate) const GROUND_TILE_WRAP: f32 = 96.0 - 8.0;
pub(crate) const GROUND_TILE_NAME_ROOM: f32 = 78.0 - 4.0 - (26.0 + 20.0);
/// The Small text style, and the largest text size the settings offer
/// (formiga-desktop/src/settings.rs, the style set up for every window).
pub(crate) const SMALL_TEXT: f32 = 10.5;
pub(crate) const TEXT_SCALES: [u8; 2] = [100, 150];

pub fn catalog() -> Value {
    let reach = Reach::watch();
    let mut text = NameText::new();
    let kinds = vec![
        decorations(&reach),
        village_kind(
            "hangout",
            "HangoutKind",
            &HangoutKind::ALL,
            &HangoutKind::STARTING,
            VillageItem::Hangout,
            HangoutKind::label,
            HangoutKind::description,
            &reach,
            &mut text,
            "home.hangouts[].kind",
            "ColonyObjectRenderer::render_atlas, at ColonyObjectRenderer::hangout_cell",
        ),
        village_kind(
            "garden",
            "GardenKind",
            &GardenKind::ALL,
            &GardenKind::STARTING,
            VillageItem::Garden,
            GardenKind::label,
            GardenKind::description,
            &reach,
            &mut text,
            "home.gardens[].kind",
            "ColonyObjectRenderer::render_atlas, at ColonyObjectRenderer::garden_cell for each stage",
        ),
        village_kind(
            "ornament",
            "OrnamentKind",
            &OrnamentKind::ALL,
            &OrnamentKind::STARTING,
            VillageItem::Ornament,
            OrnamentKind::label,
            OrnamentKind::description,
            &reach,
            &mut text,
            "home.ornaments[].kind",
            "ColonyObjectRenderer::render_atlas, at ColonyObjectRenderer::ornament_cell",
        ),
        objects(&reach),
        house_styles(),
        palettes(),
        trinkets(),
        accessories(),
        souvenirs(),
        wonders(),
        body_plans(),
        ear_styles(),
        archetypes(),
        habits(),
    ];
    json!({
        "success": true,
        "save_version": SAVE_VERSION,
        "reach": {
            "colonies": REACH_COLONIES,
            "days": REACH_DAYS,
            "about": "Village pieces and colony objects are watched arriving in this many \
                      colonies, each lived day by day for this many days.",
        },
        "names_shown": [{
            "where": "the Village page's \"Put something down\" tiles",
            "kinds": ["hangout", "garden", "ornament"],
            "wrap": GROUND_TILE_WRAP,
            "room": GROUND_TILE_NAME_ROOM,
            "text_size": SMALL_TEXT,
            "text_scales": TEXT_SCALES,
        }, {
            "where": "Formiga Home, which cuts a find's or souvenir's name to this many letters",
            "kinds": ["trinket", "souvenir"],
            "letters": formiga_home_contract::limits::MAX_ITEM_NAME_CHARS,
        }],
        "kinds": kinds,
    })
}

/// Where a new name would be shown, and whether it fits there: what `catalog` says of each
/// ground piece's name, for one that does not exist yet.
pub fn fits(name: &str) -> Value {
    json!({ "success": true, "name": name, "shown": NameText::new().ground_tile(name) })
}

/// One kind of thing, and what is true of every item of it.
#[allow(clippy::too_many_arguments)]
fn kind(
    key: &str,
    type_name: &str,
    defined_in: &str,
    saved_in: &[&str],
    drawn_by: &str,
    comes_by: &str,
    items: Vec<Value>,
) -> Value {
    json!({
        "kind": key,
        "type": type_name,
        "defined_in": defined_in,
        "saved_in": saved_in,
        "drawn_by": drawn_by,
        "comes_by": comes_by,
        "count": items.len(),
        "items": items,
    })
}

const MODEL: &str = "crates/formiga-core/src/model.rs";

fn decorations(reach: &Reach) -> Value {
    let items = ShelterDecorationKind::ALL
        .iter()
        .map(|&kind| {
            let item = VillageItem::Decoration(kind);
            json!({
                "id": format!("{kind:?}"),
                "name": kind.label(),
                "number": kind.index(),
                "saved_as": serde_json::to_value(kind).unwrap_or_default(),
                "group": kind.slot().label(),
                "starting": ShelterDecorationKind::STARTING.contains(&kind),
                "reach": reach.village(item),
            })
        })
        .collect();
    kind(
        "decoration",
        "ShelterDecorationKind",
        MODEL,
        &["home.unlocks.decorations", "home.dressing[].decorations"],
        "ShelterRenderer::render_with_decorations (the colony house) and \
         ShelterRenderer::render_village (every house)",
        "Three to start with; then the village gains a new piece every day or two \
         (VillageUnlocks), and it hangs itself on the colony house if its place there is free.",
        items,
    )
}

#[allow(clippy::too_many_arguments)]
fn village_kind<K: Copy + std::fmt::Debug + PartialEq>(
    key: &str,
    type_name: &str,
    all: &[K],
    starting: &[K],
    wrap: fn(K) -> VillageItem,
    label: fn(K) -> &'static str,
    description: fn(K) -> &'static str,
    reach: &Reach,
    text: &mut NameText,
    saved_in: &str,
    drawn_by: &str,
) -> Value {
    let items = all
        .iter()
        .enumerate()
        .map(|(number, &kind)| {
            let item = wrap(kind);
            json!({
                "id": format!("{kind:?}"),
                "name": label(kind),
                "number": number,
                "saved_as": saved_village(item),
                "about": description(kind),
                "starting": starting.contains(&kind),
                "reach": reach.village(item),
                "shown": text.ground_tile(label(kind)),
            })
        })
        .collect();
    kind(
        key,
        type_name,
        MODEL,
        &[&format!("home.unlocks.{key}s"), saved_in],
        drawn_by,
        "Three to start with; then the village gains a new piece every day or two \
         (VillageUnlocks), and it is put down from the Village page.",
        items,
    )
}

/// The name a save keeps for a village piece: the variant inside `VillageItem`'s wrapping.
fn saved_village(item: VillageItem) -> Value {
    match serde_json::to_value(item).unwrap_or_default() {
        Value::Object(map) => map.into_iter().next().map(|(_, v)| v).unwrap_or_default(),
        other => other,
    }
}

fn objects(reach: &Reach) -> Value {
    let items = ColonyObjectKind::ALL
        .iter()
        .map(|&kind| {
            json!({
                "id": format!("{kind:?}"),
                "name": kind.label(),
                "number": kind.index(),
                "saved_as": serde_json::to_value(kind).unwrap_or_default(),
                "group": format!("{:?}", kind.default_role()),
                "reach": reach.object(kind),
            })
        })
        .collect();
    kind(
        "object",
        "ColonyObjectKind",
        MODEL,
        &["objects.objects[].kind"],
        "ColonyObjectRenderer::render_atlas, at ColonyObjectRenderer::object_cell",
        &format!(
            "Arrives on its own every few days, picked at random from the kinds the colony does \
             not have yet, until it has {MAX_COLONY_OBJECTS}. A colony never gives one up, so \
             each colony only ever has {MAX_COLONY_OBJECTS} of the {} kinds.",
            ColonyObjectKind::ALL.len()
        ),
        items,
    )
}

fn house_styles() -> Value {
    let items = ShelterStyle::ALL
        .iter()
        .enumerate()
        .map(|(number, &style)| {
            json!({
                "id": format!("{style:?}"),
                "name": style.label(),
                "number": number,
                "saved_as": serde_json::to_value(style).unwrap_or_default(),
            })
        })
        .collect();
    kind(
        "house-style",
        "ShelterStyle",
        MODEL,
        &["home.shelter.style", "home.house_styles[].style"],
        "ShelterRenderer::render and ShelterRenderer::render_village",
        "Always available: the colony's own comes from its seed, and any house can be changed \
         to any of them from the Village page.",
        items,
    )
}

fn palettes() -> Value {
    let items = VillagePalette::ALL
        .iter()
        .enumerate()
        .map(|(number, &palette)| {
            json!({
                "id": format!("{palette:?}"),
                "name": palette.label(),
                "number": number,
                "saved_as": serde_json::to_value(palette).unwrap_or_default(),
            })
        })
        .collect();
    kind(
        "palette",
        "VillagePalette",
        MODEL,
        &["home.palette"],
        "ColonyHome::drawn_shelter, then the shelter renderers",
        "Always available from the Village page.",
        items,
    )
}

fn trinkets() -> Value {
    let items = all_trinkets()
        .map(|info| {
            json!({
                "id": info.variant.to_string(),
                "name": info.name,
                "number": info.variant,
                "saved_as": info.variant,
                "about": info.description,
                "hint": info.hint,
                "group": info.condition.label(),
                "condition": format!("{:?}", info.condition),
                "made_into": AccessoryKind::made_by(info.variant).map(|kind| format!("{kind:?}")),
            })
        })
        .collect();
    kind(
        "trinket",
        "TrinketInfo (by variant number)",
        "crates/formiga-core/src/trinkets.rs",
        &[
            "companion.scrapbook[].variant",
            "creatures[].accessory.Pin",
            "home.keepsakes.hooks",
        ],
        "draw_trinket (formiga-art), resting and glinting",
        "Found by a companion out on the desktop, in the circumstance its group names: an \
         everyday find on any ordinary day, the others only when that circumstance holds.",
        items,
    )
}

fn accessories() -> Value {
    let items = AccessoryKind::ALL
        .iter()
        .map(|&kind| {
            let find = kind.made_from();
            let found = [ScrapbookRecord {
                variant: find,
                first_at: OffsetDateTime::UNIX_EPOCH,
                finder: None,
                finder_name: String::new(),
            }];
            let worn = Accessory::Worn(kind);
            json!({
                "id": format!("{kind:?}"),
                "name": kind.label(),
                "number": kind.index(),
                "saved_as": serde_json::to_value(worn).unwrap_or_default(),
                "group": format!("{:?}", kind.place()),
                "made_from": {
                    "find": find,
                    "name": trinket_info(find).map(|info| info.name),
                },
                "rule": {
                    "worn_before_its_find": worn.available(&[]),
                    "worn_once_found": worn.available(&found),
                },
            })
        })
        .collect();
    kind(
        "accessory",
        "AccessoryKind",
        "crates/formiga-core/src/accessories.rs",
        &["creatures[].accessory.Worn"],
        "CreatureRenderer::render_dressed_composited_frame, with AccessoryArt::resolve",
        "Made from a find: it can be worn once the colony has found the find it is made from \
         (Accessory::available).",
        items,
    )
}

fn souvenirs() -> Value {
    let items = Souvenir::ALL
        .iter()
        .enumerate()
        .map(|(number, &souvenir)| {
            json!({
                "id": souvenir.id(),
                "variant": format!("{souvenir:?}"),
                "name": souvenir.name(),
                "number": number,
                "saved_as": serde_json::to_value(souvenir).unwrap_or_default(),
                "about": souvenir.description(),
            })
        })
        .collect();
    kind(
        "souvenir",
        "Souvenir",
        "crates/formiga-core/src/souvenirs.rs",
        &["trips.souvenirs[].souvenir"],
        "draw_souvenir (formiga-art)",
        "Brought back from a trip to Formiga Hill, by a story or a fairground game there.",
        items,
    )
}

fn wonders() -> Value {
    let items = WonderKind::ALL
        .iter()
        .map(|&kind| {
            json!({
                "id": format!("{kind:?}"),
                "name": kind.label(),
                "number": kind.index(),
                "saved_as": serde_json::to_value(kind).unwrap_or_default(),
                "about": kind.description(),
                "hint": kind.hint(),
            })
        })
        .collect();
    kind(
        "wonder",
        "WonderKind",
        "crates/formiga-core/src/wonders.rs",
        &["companion.wonders[].kind"],
        "WonderRenderer::render (formiga-art), one strip of frames",
        "Come across by a companion out on the desktop; ones not yet seen are picked three \
         times in four, and the seesaw needs a friend along.",
        items,
    )
}

const DESIGN: &str = "crates/formiga-core/src/design.rs";

fn body_plans() -> Value {
    let items = BodyPlan::ALL
        .iter()
        .enumerate()
        .map(|(number, &plan)| {
            json!({
                "id": format!("{plan:?}"),
                "name": plan.label(),
                "number": number,
                "saved_as": serde_json::to_value(plan).unwrap_or_default(),
            })
        })
        .collect();
    kind(
        "body-plan",
        "BodyPlan",
        DESIGN,
        &[
            "creatures[].appearance.design.body",
            "creatures[].origin.design.body",
        ],
        "CreatureRenderer::render_composited_frame",
        "Drawn for a companion when it is born (CreatureDesign::drawn). Its number is written \
         into share codes and trips, so the order of the list must never change.",
        items,
    )
}

fn ear_styles() -> Value {
    let items = EarStyle::ALL
        .iter()
        .enumerate()
        .map(|(number, &ears)| {
            json!({
                "id": format!("{ears:?}"),
                "name": format!("{ears:?} ears"),
                "number": number,
                "saved_as": serde_json::to_value(ears).unwrap_or_default(),
                "unnamed": true,
            })
        })
        .collect();
    kind(
        "ear-style",
        "EarStyle",
        DESIGN,
        &[
            "creatures[].appearance.design.ears",
            "creatures[].origin.design.ears",
        ],
        "CreatureRenderer::render_composited_frame",
        "Drawn for a companion when it is born. Never named on screen. Its number is written \
         into share codes and trips, so the order of the list must never change.",
        items,
    )
}

fn archetypes() -> Value {
    let items = BodyArchetype::ALL
        .iter()
        .map(|&archetype| {
            let bodies: Vec<String> = BodyPlan::ALL
                .iter()
                .filter(|plan| archetype.allows(**plan))
                .map(|plan| format!("{plan:?}"))
                .collect();
            json!({
                "id": format!("{archetype:?}"),
                "name": archetype.label(),
                "number": archetype.number(),
                "saved_as": archetype.number(),
                "bodies": bodies,
            })
        })
        .collect();
    kind(
        "archetype",
        "BodyArchetype",
        DESIGN,
        &["creatures[].appearance.design.archetype"],
        "CreatureRenderer::render_composited_frame (through the recipe it shapes)",
        "Drawn first when a companion is born, with its own odds; it shapes the rest of the \
         recipe. Named only on review sheets.",
        items,
    )
}

fn habits() -> Value {
    let items = Habit::ALL
        .iter()
        .enumerate()
        .map(|(number, &habit)| {
            json!({
                "id": format!("{habit:?}"),
                "name": habit.label(),
                "number": number,
                "saved_as": serde_json::to_value(habit).unwrap_or_default(),
            })
        })
        .collect();
    kind(
        "habit",
        "Habit",
        "crates/formiga-core/src/habits.rs",
        &["creatures[].memory.habits"],
        "the creature's own animation, through the action it plays",
        "Picked up by a companion from what happens around it (habit_to_learn, learning_chance).",
        items,
    )
}

/// Village pieces and colony objects as they arrive in colonies lived through day by day.
pub(crate) struct Reach {
    /// For each item: the day it arrived in each colony it arrived in.
    village: BTreeMap<String, Vec<i64>>,
    objects: BTreeMap<String, Vec<i64>>,
}

impl Reach {
    pub(crate) fn watch() -> Self {
        let desktop = crate::fixture_desktop();
        let mut village: BTreeMap<String, Vec<i64>> = BTreeMap::new();
        let mut objects: BTreeMap<String, Vec<i64>> = BTreeMap::new();
        for colony in 0..REACH_COLONIES {
            let mut seed = [colony; 32];
            seed[0] = 0xF0;
            let mut world = World::new(seed, REACH_START, &desktop);
            let mut seen_village: Vec<VillageItem> = Vec::new();
            let mut seen_objects: Vec<ColonyObjectKind> = Vec::new();
            for day in 0..=REACH_DAYS {
                if day > 0 {
                    world.tick(REACH_START + Duration::days(day), 0.05, &desktop);
                }
                for item in VillageItem::all() {
                    if world.save.home.unlocks.has(item) && !seen_village.contains(&item) {
                        seen_village.push(item);
                        village.entry(village_key(item)).or_default().push(day);
                    }
                }
                for object in &world.save.objects.objects {
                    if !seen_objects.contains(&object.kind) {
                        seen_objects.push(object.kind);
                        objects
                            .entry(format!("{:?}", object.kind))
                            .or_default()
                            .push(day);
                    }
                }
            }
        }
        Self { village, objects }
    }

    fn village(&self, item: VillageItem) -> Value {
        reached(self.village.get(&village_key(item)))
    }

    fn object(&self, kind: ColonyObjectKind) -> Value {
        reached(self.objects.get(&format!("{kind:?}")))
    }
}

fn village_key(item: VillageItem) -> String {
    format!("{item:?}")
}

fn reached(days: Option<&Vec<i64>>) -> Value {
    let days = days.cloned().unwrap_or_default();
    json!({
        "colonies": days.len(),
        "of": REACH_COLONIES,
        "first_day": days.iter().min(),
        "last_day": days.iter().max(),
    })
}

/// Names measured in egui's own font, the one every Formiga window draws its text in.
struct NameText {
    fonts: Fonts,
}

impl NameText {
    fn new() -> Self {
        Self {
            fonts: Fonts::new(TextOptions::default(), FontDefinitions::default()),
        }
    }

    /// How a ground piece's name sits in its catalogue tile at each text size.
    fn ground_tile(&mut self, name: &str) -> Value {
        let sizes: Vec<Value> = TEXT_SCALES
            .iter()
            .map(|&scale| {
                let size = SMALL_TEXT * f32::from(scale) / 100.0;
                let mut view = self.fonts.with_pixels_per_point(1.0);
                let galley = view.layout(
                    name.to_owned(),
                    FontId::new(size, FontFamily::Proportional),
                    Color32::WHITE,
                    GROUND_TILE_WRAP,
                );
                // A word too long for the tile is broken partway through: egui wraps a word
                // that cannot fit on a line of its own at any letter.
                let longest = name
                    .split_whitespace()
                    .map(|word| {
                        view.layout_no_wrap(
                            word.to_owned(),
                            FontId::new(size, FontFamily::Proportional),
                            Color32::WHITE,
                        )
                        .size()
                        .x
                    })
                    .fold(0.0_f32, f32::max);
                let height = galley.size().y;
                json!({
                    "text_scale": scale,
                    "lines": galley.rows.len(),
                    "height": (height * 10.0).round() / 10.0,
                    "room": GROUND_TILE_NAME_ROOM,
                    "word_broken": longest > GROUND_TILE_WRAP,
                    "fits": height <= GROUND_TILE_NAME_ROOM && longest <= GROUND_TILE_WRAP,
                })
            })
            .collect();
        json!([{ "where": "ground tile", "sizes": sizes }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tile measurements above are copied from the Desktop app, which this crate cannot
    /// depend on. If these lines change there, change the constants here to match.
    #[test]
    fn the_ground_tile_is_measured_as_the_app_draws_it() {
        let arrange = include_str!("../../formiga-desktop/src/clubhouse/arrange.rs");
        assert!(arrange.contains("egui::vec2(96.0, 78.0)"), "tile size");
        assert!(arrange.contains("rect.top() + 26.0"), "picture centre");
        assert!(arrange.contains("egui::vec2(40.0, 40.0)"), "picture size");
        assert!(arrange.contains("rect.width() - 8.0"), "name wrap");
        assert!(
            arrange.contains("rect.bottom() - 4.0 - name.size().y"),
            "name foot"
        );
        let settings = include_str!("../../formiga-desktop/src/settings.rs");
        assert!(
            settings.contains("(egui::TextStyle::Small, 10.5)"),
            "small text"
        );
        assert!(
            settings.contains("text_scale.clamp(100, 150)"),
            "text scales"
        );
    }

    #[test]
    fn every_kind_lists_every_item_once() {
        let catalog = catalog();
        for kind in catalog["kinds"].as_array().unwrap() {
            let items = kind["items"].as_array().unwrap();
            let mut ids: Vec<&str> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), items.len(), "{}", kind["kind"]);
        }
    }
}
