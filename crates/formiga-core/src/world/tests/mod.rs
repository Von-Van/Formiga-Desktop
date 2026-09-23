use super::arrivals::*;
use super::experience::*;
use super::*;
use time::macros::datetime;

mod ambient;
mod arrivals;
mod bonds;
mod bubbles;
mod colony_management;
mod companion;
mod discovery;
mod experience;
mod habits;
mod hangouts;
mod home;
mod interaction;
mod journeys;
mod misc;
mod moments;
mod objects_and_decorations;
mod offers;
mod perches;
mod rituals;
mod spacing;
mod topology_and_attention;
mod tows;
mod undo;
mod village;
mod village_life;
mod visitors;

pub(super) fn desktop() -> DesktopSnapshot {
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

pub(super) fn let_colony_wander(world: &mut World, now: OffsetDateTime) {
    world.save.home.active_since_utc = None;
    world.save.home.last_disappeared_utc = Some(now);
}

fn two_creature_world(seed: [u8; 32], created: OffsetDateTime) -> World {
    let desktop = desktop();
    let mut world = World::new(seed, created, &desktop);
    let now = created + Duration::hours(1);
    world.tick(now, 0.05, &desktop);
    let_colony_wander(&mut world, now);
    world.pending_home_greetings.clear();
    assert_eq!(world.save.creatures.len(), 2);
    for (index, creature) in world.save.creatures.iter_mut().enumerate() {
        creature.state.position = Point {
            x: 500.0 + index as f32 * 60.0,
            y: 846.0,
        };
        creature.state.surface = SurfaceAttachment {
            kind: SurfaceKind::ScreenFloor,
            monitor_id: 1,
            window_key: None,
            relative_x: 0.5,
        };
        creature.state.action = ActionKind::Idle;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 100.0;
    }
    world
}

fn finish_home_approach(world: &mut World, now: OffsetDateTime, desktop: &DesktopSnapshot) {
    for _ in 0..3_000 {
        world.tick(now, 0.05, desktop);
        if world.save.creatures.iter().all(|creature| {
            creature.state.arrival_delay_secs <= 0.0 && world.resting_at_home(creature.id)
        }) {
            return;
        }
    }
    panic!("colony did not finish walking home");
}

/// Walks the colony home and holds it perfectly still: no passive moments, so a test about
/// where everyone stands sees exactly that.
fn finish_home_approach_still(world: &mut World, now: OffsetDateTime, desktop: &DesktopSnapshot) {
    let reduce_motion = world.save.settings.reduce_motion;
    world.save.settings.reduce_motion = true;
    finish_home_approach(world, now, desktop);
    world.save.settings.reduce_motion = reduce_motion;
}
