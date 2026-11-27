//! A visitor the colony has met comes back pixel for pixel the creature it was: drawn from the
//! colony file alone after it has been written and read back, every frame of how it stands,
//! walks, greets and plays is the same picture it was while it was here.

use formiga_art::CreatureRenderer;
use formiga_core::*;
use time::Duration;
use time::macros::datetime;

fn desktop() -> DesktopSnapshot {
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

#[test]
fn a_visitor_met_before_is_drawn_pixel_for_pixel_after_a_save_and_load() {
    let desktop = desktop();
    for (colony, friend) in [([41_u8; 32], [77_u8; 32]), ([42; 32], [5; 32])] {
        let mut now = datetime!(2026-11-27 9:00 UTC);
        let mut world = World::new(colony, now, &desktop);
        let code = SharedCreatureSeed {
            source_colony_seed: friend,
            source_generation: 0,
            design: Some(CreatureDesign::generated(friend, 0, None)),
        };
        world.invite_visitor(code, now, &desktop).unwrap();
        let mut met = None;
        for _ in 0..(180.0 / 0.05) as i64 {
            now += Duration::milliseconds(50);
            world.tick(now, 0.05, &desktop);
            world.drain_events().for_each(drop);
            if let Some(guest) = &world.save.visitors.guest
                && guest.visit.greeted
            {
                met = Some(guest.creature.clone());
                break;
            }
        }
        let met = met.expect("the friend said hello");
        assert!(world.save.visitors.known(&met.origin).is_some());

        let bytes = serde_json::to_vec_pretty(&world.save).unwrap();
        let mut reopened = World::from_save(decode(&bytes).unwrap());
        reopened.save.visitors.guest = None;
        let back = reopened
            .remembered_visitor(&met.origin, now + Duration::days(3))
            .expect("remembered");
        assert_eq!(back.name, met.name);
        assert_eq!(back.appearance, met.appearance);
        for action in [
            ActionKind::Idle,
            ActionKind::Traverse,
            ActionKind::Greet,
            ActionKind::SoloPlay,
            ActionKind::Perch,
        ] {
            for frame in 0..4 {
                for facing in [false, true] {
                    let before =
                        CreatureRenderer::render_frame(&met.appearance, action, frame, facing);
                    let after =
                        CreatureRenderer::render_frame(&back.appearance, action, frame, facing);
                    assert!(before == after, "{action:?} frame {frame} differs");
                }
            }
        }
    }
}
