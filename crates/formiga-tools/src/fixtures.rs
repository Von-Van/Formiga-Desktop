//! The desktop and the reference companions the review sheets are drawn on.

use formiga_core::*;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

/// Three reference creatures, one from each of the blob, hopper, and soft-quadruped families:
/// the first seed of each that grows into that family, on a plain modular recipe, so the review
/// sheets that compare poses keep the same three subjects whatever the generator now mixes in.
pub(crate) fn reference_creatures() -> Vec<Creature> {
    let desktop = fixture_desktop();
    [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ]
    .into_iter()
    .map(|family| {
        (0_u64..1_000)
            .find_map(|index| {
                let mut seed = [0_u8; 32];
                seed.copy_from_slice(&Sha256::digest(format!(
                    "formiga-reference-{family:?}-{index}"
                )));
                let mut creature = World::new(seed, OffsetDateTime::UNIX_EPOCH, &desktop)
                    .save
                    .creatures
                    .remove(0);
                // Classic parts have their own sheet; these subjects stay as they have been.
                apply_creature_design(&mut creature, Some(CreatureDesign::modular(seed, 0, None)));
                (creature.appearance.family == family).then_some(creature)
            })
            .expect("a deterministic reference seed exists for every family")
    })
    .collect()
}

pub(crate) fn fixture_desktop() -> DesktopSnapshot {
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
        cursor: CursorSnapshot {
            position: Point { x: 720.0, y: 420.0 },
            velocity: Point::default(),
            available: true,
        },
        ..DesktopSnapshot::default()
    }
}
