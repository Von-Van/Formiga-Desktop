use super::*;
use crate::trinkets::{TrinketCondition, trinket_info, trinkets_for};
use crate::world::discovery::{
    ColonyView, DiscoveryCircumstances, after_dark, beside_close_friend, choose_trinket_variant,
    high_up, mid_ride, plaything_variant,
};
use std::collections::BTreeSet;
use time::macros::offset;

const LEDGE: WindowKey = 808;
const SHELF: WindowKey = 809;

/// One companion standing in the middle of a window ledge at `ledge_y`, on the plain single
/// monitor the rest of these tests use. The habitat floor is at y 846.
fn ledge_world(ledge_y: f32) -> (World, DesktopSnapshot) {
    let mut desktop = desktop();
    desktop.windows.push(DesktopWindow {
        key: LEDGE,
        bounds: DesktopRect {
            x: 200.0,
            y: ledge_y,
            width: 600.0,
            height: 100.0,
        },
        visible: true,
        minimized: false,
        z_order: 0,
        application: None,
        application_name: None,
    });
    let mut world = World::new([19; 32], datetime!(2026-01-01 0:00 UTC), &desktop);
    let creature = &mut world.save.creatures[0];
    creature.state.surface = SurfaceAttachment {
        kind: SurfaceKind::WindowLedge,
        monitor_id: 1,
        window_key: Some(LEDGE),
        relative_x: 0.5,
    };
    creature.state.position = Point {
        x: 500.0,
        y: ledge_y,
    };
    creature.state.arrival_delay_secs = 0.0;
    (world, desktop)
}

/// The one circumstance under test and nothing else.
fn only(condition: TrinketCondition) -> DiscoveryCircumstances {
    DiscoveryCircumstances {
        night: condition == TrinketCondition::Night,
        high_tier: condition == TrinketCondition::HighTier,
        mid_ride: condition == TrinketCondition::MidRide,
        beside_close_friend: condition == TrinketCondition::BesideCloseFriend,
    }
}

fn every_circumstance() -> DiscoveryCircumstances {
    DiscoveryCircumstances {
        night: true,
        high_tier: true,
        mid_ride: true,
        beside_close_friend: true,
    }
}

/// A scrapbook that has already seen these variants.
fn found(variants: &[u8]) -> Vec<ScrapbookRecord> {
    variants
        .iter()
        .map(|variant| ScrapbookRecord {
            variant: *variant,
            first_at: datetime!(2026-01-01 0:00 UTC),
            finder: None,
            finder_name: String::from("Someone"),
        })
        .collect()
}

#[test]
fn with_nothing_special_going_on_only_the_everyday_trinkets_turn_up() {
    let mut rng = ChaCha12Rng::from_seed([7; 32]);
    let mut plain = ChaCha12Rng::from_seed([7; 32]);
    for _ in 0..512 {
        let variant = choose_trinket_variant(&mut rng, DiscoveryCircumstances::default(), &[]);
        assert!(variant < 8, "variant {variant} needs a circumstance");
        // The everyday sequence is the one the colony has always had: one plain draw over the
        // original eight per find, in the same order, with nothing else taken off the stream.
        assert_eq!(variant, plain.random_range(0..8_u8));
    }
}

#[test]
fn a_find_leaves_the_ambient_stream_where_an_everyday_draw_would_whatever_was_true() {
    let scrapbook = found(&[8, 10, 12]);
    for mask in 0..16_u8 {
        let circumstances = DiscoveryCircumstances {
            night: mask & 1 != 0,
            high_tier: mask & 2 != 0,
            mid_ride: mask & 4 != 0,
            beside_close_friend: mask & 8 != 0,
        };
        let mut rng = ChaCha12Rng::from_seed([13; 32]);
        let mut plain = ChaCha12Rng::from_seed([13; 32]);
        for step in 0..128 {
            choose_trinket_variant(&mut rng, circumstances, &scrapbook);
            plain.random_range(0..8_u8);
            // Whether a circumstance held depends on the clock and on where the windows happen to
            // be. Neither is allowed a say in how far the ambient stream has moved, so the next
            // draw is the same one it would always have been.
            assert_eq!(
                rng.random::<u64>(),
                plain.random::<u64>(),
                "{circumstances:?} diverged at find {step}"
            );
        }
    }
}

#[test]
fn after_dark_reads_the_hour_on_the_users_own_clock() {
    // The same instant is the middle of the afternoon in one place and the small hours in
    // another, and the rule follows the wall clock rather than UTC.
    let midday = datetime!(2026-03-01 12:00 UTC);
    assert!(!after_dark(midday));
    assert!(after_dark(midday.to_offset(offset!(+9))), "21:00 is dark");
    assert!(after_dark(midday.to_offset(offset!(-8))), "04:00 is dark");
    assert!(!after_dark(midday.to_offset(offset!(-4))), "08:00 is not");
    for hour in 0..24_i64 {
        let local = datetime!(2026-03-01 0:00 UTC) + Duration::hours(hour);
        assert_eq!(after_dark(local), !(6..20).contains(&hour), "{hour}");
    }
    // Wider than the late-night ritual's own 22..05, which is about somebody still being awake
    // rather than about it being dark, so that window sits inside this one.
    for hour in [22_i64, 23, 0, 4] {
        assert!(after_dark(
            datetime!(2026-03-01 0:00 UTC) + Duration::hours(hour)
        ));
    }
}

#[test]
fn a_ledge_is_high_when_there_is_a_long_way_down() {
    let (high, desktop) = ledge_world(300.0);
    assert!(high_up(
        &high.save.creatures[0],
        &desktop,
        &high.save.settings
    ));

    // The same ledge a hand's breadth above the floor is not somewhere high up.
    let (low, low_desktop) = ledge_world(800.0);
    assert!(!high_up(
        &low.save.creatures[0],
        &low_desktop,
        &low.save.settings
    ));

    // Standing on the floor is never high, however far the screen falls away elsewhere.
    let (mut floor, floor_desktop) = ledge_world(300.0);
    floor.save.creatures[0].state.surface.kind = SurfaceKind::ScreenFloor;
    floor.save.creatures[0].state.surface.window_key = None;
    assert!(!high_up(
        &floor.save.creatures[0],
        &floor_desktop,
        &floor.save.settings
    ));

    // A window opening just underneath takes the height away: what counts is the nearest thing
    // that would catch a fall, not the distance to the ground.
    let (stacked, mut shelved) = ledge_world(300.0);
    shelved.windows.push(DesktopWindow {
        key: SHELF,
        bounds: DesktopRect {
            x: 200.0,
            y: 420.0,
            width: 600.0,
            height: 100.0,
        },
        visible: true,
        minimized: false,
        z_order: 1,
        application: None,
        application_name: None,
    });
    assert!(!high_up(
        &stacked.save.creatures[0],
        &shelved,
        &stacked.save.settings
    ));
}

#[test]
fn a_ride_counts_while_the_window_moves_and_as_it_settles() {
    let (mut world, mut desktop) = ledge_world(400.0);
    desktop.window_sample = Some(WindowSample {
        monotonic_millis: 0,
        reliable: true,
    });
    let id = world.save.creatures[0].id;
    world
        .ride_memory
        .update(&world.save.creatures, &desktop, 0.05, true);

    // Standing on a window that is not going anywhere is not a ride, even though a rider is
    // handed a neutral stance the first time it is seen on one.
    assert!(!world.ride_memory.riding(id));
    assert!(!mid_ride(
        &world.save.creatures[0],
        ActionKind::Perch,
        &world.ride_memory
    ));

    // A find is chosen at an action boundary, and the ride is an ordinary action that ends at
    // one, so the ride settling is exactly the moment this is true.
    assert!(mid_ride(
        &world.save.creatures[0],
        ActionKind::RideWindow,
        &world.ride_memory
    ));

    // The window actually moving underneath counts on its own, whatever they were just doing.
    desktop.window_sample.as_mut().unwrap().monotonic_millis = 250;
    desktop.windows[0].bounds.x += 120.0;
    world
        .ride_memory
        .update(&world.save.creatures, &desktop, 0.05, true);
    assert!(world.ride_memory.riding(id));
    assert!(mid_ride(
        &world.save.creatures[0],
        ActionKind::Perch,
        &world.ride_memory
    ));
}

#[test]
fn a_close_friend_has_to_be_both_close_and_near() {
    let (mut world, _) = ledge_world(400.0);
    let mut companion = world.save.creatures[0].clone();
    companion.id = world.save.creatures[0].id ^ 1;
    companion.state.position = Point { x: 540.0, y: 400.0 };
    world.save.creatures.push(companion);
    let (a, b) = (world.save.creatures[0].id, world.save.creatures[1].id);
    let mut relationship = CreatureRelationship::new(a, b).expect("two distinct companions");
    relationship.affinity = 112;

    let beside = |world: &World, relationship: CreatureRelationship| {
        beside_close_friend(
            &world.save.creatures[0],
            ColonyView {
                creatures: &world.save.creatures,
                relationships: &[relationship],
            },
        )
    };
    assert!(beside(&world, relationship), "a close friend, within reach");

    // The journal calls 112 a friendship. A well-known acquaintance standing just as near is a
    // different thing and does not turn up the same keepsake.
    let mut acquaintance = relationship;
    acquaintance.affinity = 111;
    acquaintance.familiarity = 255;
    assert!(!beside(&world, acquaintance));

    // Two creature-widths, not "somewhere on the same ledge".
    world.save.creatures[1].state.position.x = 700.0;
    assert!(!beside(&world, relationship));
    world.save.creatures[1].state.position.x = 540.0;

    // The same ledge of the same display: a friend on the window below is not beside anybody.
    world.save.creatures[1].state.surface.window_key = Some(SHELF);
    assert!(!beside(&world, relationship));
    world.save.creatures[1].state.surface.window_key = Some(LEDGE);

    world.save.creatures[1].state.surface.monitor_id = 2;
    assert!(!beside(&world, relationship));
    world.save.creatures[1].state.surface.monitor_id = 1;

    // Somebody who has not arrived yet is not standing anywhere.
    world.save.creatures[1].state.arrival_delay_secs = 5.0;
    assert!(!beside(&world, relationship));
    world.save.creatures[1].state.arrival_delay_secs = 0.0;
    assert!(beside(&world, relationship));
}

#[test]
fn a_keepsake_never_turns_up_without_the_circumstance_it_belongs_to() {
    for condition in TrinketCondition::ALL {
        if condition == TrinketCondition::Anywhere {
            continue;
        }
        let mut rng = ChaCha12Rng::from_seed([37; 32]);
        let mut seen = BTreeSet::new();
        for _ in 0..4096 {
            seen.insert(choose_trinket_variant(&mut rng, only(condition), &[]));
        }
        let conditional: BTreeSet<u8> = seen.iter().copied().filter(|v| *v >= 8).collect();
        let expected: BTreeSet<u8> = trinkets_for(condition).map(|t| t.variant).collect();
        assert_eq!(conditional, expected, "{condition:?}");
        assert!(
            (0..8).all(|variant| seen.contains(&variant)),
            "{condition:?} still leaves room for an ordinary find"
        );
    }
}

#[test]
fn a_circumstance_makes_its_keepsake_likely_without_promising_one() {
    let mut rng = ChaCha12Rng::from_seed([23; 32]);
    let mut conditional = 0;
    let draws = 8192;
    for _ in 0..draws {
        conditional +=
            usize::from(choose_trinket_variant(&mut rng, only(TrinketCondition::Night), &[]) >= 8);
    }
    // About half, by design: a circumstance should make a keepsake likely and never promise one.
    let share = conditional as f32 / draws as f32;
    assert!((0.45..0.55).contains(&share), "{conditional} of {draws}");
}

#[test]
fn an_empty_scrapbook_slot_fills_before_one_already_filled() {
    let scrapbook = found(&[8]);
    let mut rng = ChaCha12Rng::from_seed([29; 32]);
    let (mut refound, mut fresh) = (0, 0);
    for _ in 0..8192 {
        match choose_trinket_variant(&mut rng, only(TrinketCondition::Night), &scrapbook) {
            8 => refound += 1,
            9 => fresh += 1,
            _ => {}
        }
    }
    // Three times in four the pick prefers the empty slot, so the scrapbook fills rather than
    // handing back the same keepsake; the fourth time either of them can turn up.
    assert!(
        fresh > refound * 4,
        "{fresh} fresh against {refound} refound"
    );
    assert!(refound > 0, "a keepsake can still be found twice");

    // With both already in the book there is nothing to favour, and they share the slot evenly.
    let complete = found(&[8, 9]);
    let mut rng = ChaCha12Rng::from_seed([29; 32]);
    let (mut eight, mut nine) = (0, 0);
    for _ in 0..8192 {
        match choose_trinket_variant(&mut rng, only(TrinketCondition::Night), &complete) {
            8 => eight += 1,
            9 => nine += 1,
            _ => {}
        }
    }
    assert!(eight > 0 && nine > 0);
    assert!(
        (eight as f32 / nine as f32 - 1.0).abs() < 0.2,
        "{eight} against {nine}"
    );
}

#[test]
fn every_trinket_in_the_catalogue_can_be_found() {
    let mut rng = ChaCha12Rng::from_seed([31; 32]);
    let mut seen = BTreeSet::new();
    for _ in 0..8192 {
        seen.insert(choose_trinket_variant(&mut rng, every_circumstance(), &[]));
    }
    let catalogue: BTreeSet<u8> = (0..TRINKET_VARIANTS).collect();
    assert_eq!(seen, catalogue);
}

#[test]
fn a_games_toy_is_always_an_everyday_one_and_is_picked_from_the_scene() {
    let mut seen = BTreeSet::new();
    for seed in 0..512_u64 {
        let variant = plaything_variant(seed);
        assert!(variant < 8, "a toy is never a keepsake: {variant}");
        assert_eq!(
            variant,
            plaything_variant(seed),
            "the same scene carries the same toy"
        );
        seen.insert(variant);
    }
    assert_eq!(seen.len(), 8, "every everyday toy is reachable");
}

/// A companion on a high ledge whose ride has just ended, with no close friendships anywhere, so
/// the only circumstances that can hold are being high up, being mid-ride, and whatever the
/// machine's own clock says about the hour.
#[test]
fn a_find_in_the_right_circumstances_reaches_the_scrapbook_with_its_trinket_and_finder() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let (mut world, desktop) = ledge_world(300.0);
    let now = created + Duration::days(30);
    world.tick(now, 0.05, &desktop);
    let_colony_wander(&mut world, now);
    world.save.ritual.next_at_utc = now + Duration::days(9);
    world.drain_events().for_each(drop);

    let mut at = now;
    let mut keepsake = None;
    for step in 0..600_i64 {
        at = now + Duration::seconds(step * 10);
        // Nobody here is close enough to anybody to make a friendship keepsake possible.
        world
            .save
            .relationships
            .iter_mut()
            .for_each(|relationship| relationship.affinity = 0);
        for creature in &mut world.save.creatures {
            creature.state.arrival_delay_secs = 0.0;
            creature.state.surface = SurfaceAttachment {
                kind: SurfaceKind::WindowLedge,
                monitor_id: 1,
                window_key: Some(LEDGE),
                relative_x: 0.5,
            };
            creature.state.position = Point { x: 500.0, y: 300.0 };
            creature.state.action = ActionKind::RideWindow;
            creature.state.action_elapsed = 9.0;
            creature.state.action_duration = 3.0;
        }
        world.discovery_remaining = 0.0;
        world.tick(at, 0.05, &desktop);
        let Some(finder) = world
            .save
            .creatures
            .iter()
            .find(|creature| creature.state.action == ActionKind::PresentDiscovery)
        else {
            continue;
        };
        if finder.state.activity_variant >= 8 {
            keepsake = Some((
                finder.id,
                finder.name.clone(),
                finder.state.activity_variant,
            ));
            break;
        }
    }
    let (finder_id, finder_name, variant) =
        keepsake.expect("a keepsake turns up where its circumstance holds");
    let info = trinket_info(variant).expect("a catalogued trinket");
    let held = match info.condition {
        TrinketCondition::Anywhere | TrinketCondition::HighTier | TrinketCondition::MidRide => true,
        TrinketCondition::Night => after_dark(local_time_or_utc(at)),
        TrinketCondition::BesideCloseFriend => false,
    };
    assert!(held, "{info:?} turned up without its circumstance");

    // Let the presentation finish, which is what the scrapbook is written from.
    for creature in &mut world.save.creatures {
        if creature.id == finder_id {
            creature.state.action_elapsed = creature.state.action_duration + 1.0;
        } else {
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = 600.0;
        }
    }
    let completed = at + Duration::seconds(5);
    world.tick(completed, 0.05, &desktop);

    let record = world
        .save
        .companion
        .scrapbook
        .iter()
        .find(|record| record.variant == variant)
        .expect("the find is in the scrapbook")
        .clone();
    assert_eq!(record.finder, Some(finder_id));
    assert_eq!(record.finder_name, finder_name);
    assert!(record.first_at >= now && record.first_at <= completed);

    // What, when, who. Nothing about the hour, the height, the window, or who was standing there.
    let fields: BTreeSet<String> = serde_json::to_value(&record)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(
        fields,
        BTreeSet::from_iter(["variant", "first_at", "finder", "finder_name"].map(String::from))
    );
    let discoveries = world
        .save
        .companion
        .journal
        .iter()
        .filter(|entry| entry.moment == JournalMoment::Discovery)
        .count();
    assert_eq!(discoveries, 1, "the journal says a find happened, once");

    // A second find by the same companion within six hours is still the same quiet moment, and
    // the scrapbook keeps the first date rather than the latest one.
    let first_at = record.first_at;
    world.save.companion.remember_discovery(
        variant,
        finder_id,
        String::from("Somebody else"),
        completed + Duration::hours(1),
    );
    world.save.companion.record(
        &WorldEvent::ActionCompleted {
            creature_id: finder_id,
            action: ActionKind::PresentDiscovery,
        },
        completed + Duration::hours(1),
    );
    assert_eq!(
        world
            .save
            .companion
            .scrapbook
            .iter()
            .find(|record| record.variant == variant)
            .map(|record| (record.first_at, record.finder_name.clone())),
        Some((first_at, finder_name))
    );
    assert_eq!(
        world
            .save
            .companion
            .journal
            .iter()
            .filter(|entry| entry.moment == JournalMoment::Discovery)
            .count(),
        1,
        "journal throttling still applies"
    );
}
