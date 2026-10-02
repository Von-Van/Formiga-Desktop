//! A sample colony and the snapshot it travels as, for Hill's previews and tests and for this
//! crate's own: one of everything the format has to carry — every recipe edition, a companion from
//! before recipes, a little one, something worn, something pinned, habits, and bonds of every
//! strength. It is built through Desktop's own `World`, so it is always a colony Desktop could
//! really have, and it is the same every time.

use crate::{SessionId, TravelSnapshot, project_colony};
use formiga_core::*;
use time::OffsetDateTime;
use time::macros::datetime;

/// When the sample colony was made, and its snapshot taken.
pub const MADE: OffsetDateTime = datetime!(2026-10-02 9:30 UTC);

/// The sample snapshot's session.
pub const SESSION: &str = "5eed5eed5eed5eed5eed5eed5eed5eed";

/// One display, 1440 by 900 points at 2x, as the sample colony was made on.
pub fn desktop() -> DesktopSnapshot {
    DesktopSnapshot {
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
                height: 826.0,
            },
            scale_factor: 2.0,
            primary: true,
        }],
        ..DesktopSnapshot::default()
    }
}

/// The sample colony; `seed` picks one of many such colonies, each with one of everything.
pub fn colony(seed: u8) -> SaveFile {
    let desktop = desktop();
    let mut world = World::new([seed; 32], MADE, &desktop);
    let source = |n: u8| [seed.wrapping_mul(31).wrapping_add(n); 32];
    let modular = CreatureDesign::modular(source(2), 0, None);
    // Not every recipe drawn today happens to have details, so find one that does.
    let detailed = (4..=u8::MAX)
        .map(|n| CreatureDesign::detailed(source(n), 0).0)
        .find(|design| design.edition() == Edition::Details)
        .expect("a recipe with details");
    let designs = [
        // Today's generator, with details.
        Some(detailed),
        // An archetype without details, as 0.62 drew them.
        Some(CreatureDesign::drawn(source(3), 0).0),
        // A plain modular recipe, as 0.55 drew them.
        Some(modular),
        // Classic parts on a modular recipe.
        Some(CreatureDesign {
            classic: ClassicParts {
                face: 2,
                tail: 1,
                ..ClassicParts::default()
            },
            ..modular
        }),
    ];
    for (n, design) in designs.into_iter().enumerate() {
        if world.save.creatures.len() >= MAX_COLONY_CREATURES - 1 {
            break;
        }
        world
            .add_designed_adult(source(10 + n as u8), design, MADE, &desktop)
            .expect("room for another adult");
    }
    let mut save = world.save.clone();
    // The oldest kind of companion, from before recipes: its look is its genome alone.
    let oldest = save.creatures.len() - 1;
    save.creatures[oldest].appearance.design = None;
    save.creatures[oldest].origin.design = None;
    save.creatures[oldest].temperament = None;
    // Dressed, pinned, and in the habit of things.
    save.creatures[0].accessory = Some(Accessory::Worn(AccessoryKind::FlowerCrown));
    save.creatures[0].memory.habits = vec![Habit::WavesHello, Habit::CirclesBeforeNaps];
    save.creatures[1].accessory = Some(Accessory::Pin(17));
    save.creatures[1].memory.habits = vec![Habit::LooksFoodOver];
    // A little one of the first adult, drawn at its share of the adult's size.
    if save.creatures.len() < MAX_COLONY_CREATURES {
        let parent = save.creatures[0].clone();
        let mut mini = parent.clone();
        mini.id = parent.id ^ 0x5a5a_5a5a;
        mini.name = "Pip".to_owned();
        mini.role = CreatureRole::Mini {
            parent_id: parent.id,
        };
        mini.display_scale_percent = 62;
        mini.appearance.logical_size = size_after_parent(&parent, 62);
        mini.accessory = None;
        mini.memory.habits.clear();
        mini.colony_order = save.creatures.len() as u8;
        save.creatures.push(mini);
    }
    // Every pair, at every strength.
    save.relationships.clear();
    let ids: Vec<_> = save.creatures.iter().map(|creature| creature.id).collect();
    for (index, a) in ids.iter().enumerate() {
        for (offset, b) in ids[index + 1..].iter().enumerate() {
            let mut bond = CreatureRelationship::new(*a, *b).unwrap();
            let step = ((index * 7 + offset * 13) % 6) as u8;
            bond.affinity = step * 40;
            bond.familiarity = 255 - step * 30;
            bond.playfulness = step * 20;
            bond.avoidance = (5 - step) * 9;
            save.relationships.push(bond);
        }
    }
    save
}

/// The sample colony as it travels.
pub fn snapshot() -> TravelSnapshot {
    project_colony(
        &colony(3),
        SessionId::parse(SESSION).expect("the sample session is a session id"),
        MADE,
        "sample colony",
    )
    .expect("the sample colony can travel")
}
