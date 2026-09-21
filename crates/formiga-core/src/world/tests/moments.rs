use super::home::settled_colony;
use super::*;
use crate::world::home::HOME_DURATION;

/// A village of `members` at home, every one of them glad to be asked to anything.
fn willing_village(seed: u8, members: usize) -> (World, DesktopSnapshot, OffsetDateTime) {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([seed; 32], members, created, &desktop);
    for creature in &mut world.save.creatures {
        creature.personality.sociability = 1.0;
        creature.personality.playfulness = 1.0;
        creature.state.drives.energy = 0.6;
        creature.state.drives.sleep_pressure = 0.5;
    }
    (world, desktop, created)
}

fn invite(world: &mut World, desktop: &DesktopSnapshot, moment: VillageMoment) -> bool {
    let host = world.save.creatures[0].id;
    world.handle_command(
        WorldCommand::InviteVillageMoment {
            creature_id: host,
            moment,
        },
        desktop,
    )
}

/// Ticks until the moment under way has gathered everyone and begun, and returns how long that
/// took, in ticks.
fn until_together(world: &mut World, desktop: &DesktopSnapshot, now: OffsetDateTime) -> usize {
    for tick in 0..2_000 {
        if world
            .village_moment
            .as_ref()
            .is_some_and(|plan| plan.together)
        {
            return tick;
        }
        world.tick(now, 0.05, desktop);
    }
    panic!("the village never gathered");
}

/// Asked for a picnic, the whole village walks to one line on its own ground, faces the middle,
/// eats and drinks together, and then goes back to its afternoon, closer for it and with the
/// picnic in the journal.
#[test]
fn an_invited_picnic_gathers_the_village_and_hands_it_back() {
    let (mut world, desktop, now) = willing_village(81, 4);
    assert_eq!(
        world.available_village_moments(),
        [
            VillageMoment::Picnic,
            VillageMoment::Dance,
            VillageMoment::Nap
        ]
    );
    assert!(invite(&mut world, &desktop, VillageMoment::Picnic));
    assert_eq!(world.village_moment(), Some(VillageMoment::Picnic));
    assert!(
        !world
            .available_village_moments()
            .contains(&VillageMoment::Picnic),
        "the moment under way is not offered again"
    );
    world.drain_events().for_each(drop);
    until_together(&mut world, &desktop, now);
    world.tick(now, 0.05, &desktop);

    let plan = world.village_moment.clone().expect("under way");
    assert_eq!(plan.places.len(), 4, "everyone was glad to come");
    let cottages = colony_cottage_list(&world.save.creatures);
    let commons = home_commons(
        &world.save.home,
        cottages.as_slice(),
        &desktop.monitors,
        &world.save.settings.habitat,
        world.save.settings.display_scale,
    )
    .unwrap();
    let (low, high) = commons.standing_span();
    let clear = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * commons.scale;
    for pair in plan.places.windows(2) {
        assert!(pair[1].spot.x - pair[0].spot.x >= clear - 0.01, "{pair:?}");
    }
    let mut eating = 0;
    for place in &plan.places {
        assert!((low - 0.01..=high + 0.01).contains(&place.spot.x));
        assert_eq!(place.spot.y, commons.ground_y);
        let creature = world
            .save
            .creatures
            .iter()
            .find(|creature| creature.id == place.creature_id)
            .unwrap();
        assert_eq!(creature.state.position, place.spot);
        assert_eq!(creature.state.action, place.action);
        assert!(matches!(place.action, ActionKind::Eat | ActionKind::Drink));
        eating += usize::from(place.action == ActionKind::Eat);
        if (plan.centre.x - place.spot.x).abs() > 0.5 {
            assert_eq!(
                creature.state.facing_right,
                plan.centre.x > place.spot.x,
                "facing the middle of the line"
            );
        }
    }
    assert_eq!(eating, 2, "snacks and drinks by turns");

    let mut events = Vec::new();
    for _ in 0..(15.0 / 0.05) as usize {
        world.tick(now, 0.05, &desktop);
        events.extend(world.drain_events());
    }
    assert_eq!(world.village_moment(), None);
    assert!(events.contains(&WorldEvent::RitualCompleted {
        kind: RitualKind::Picnic
    }));
    let pairs = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                WorldEvent::BondInteraction {
                    experience: RelationshipExperience::Greeting,
                    ..
                }
            )
        })
        .count();
    assert_eq!(pairs, 6, "every pair who shared it");
    assert!(
        world
            .save
            .companion
            .journal
            .iter()
            .any(|entry| entry.moment == JournalMoment::Ritual(RitualKind::Picnic))
    );
    for creature in &world.save.creatures {
        assert!(
            matches!(
                creature.state.action,
                ActionKind::Homebound | ActionKind::Traverse
            ),
            "{:?} went back to its afternoon",
            creature.state.action
        );
    }
    assert!(world.save.home.is_active(), "and the houses are still out");
}

/// A dance is danced, each dancer on its own beat, and finished with each dancer's own way of
/// celebrating. Reduced motion never offers one.
#[test]
fn a_dance_ends_with_every_dancer_celebrating_its_own_way() {
    let (mut world, desktop, now) = willing_village(82, 3);
    assert!(invite(&mut world, &desktop, VillageMoment::Dance));
    until_together(&mut world, &desktop, now);
    let mut bopped = 0;
    let mut cheered = 0;
    while world.village_moment().is_some() {
        world.tick(now, 0.05, &desktop);
        for creature in &world.save.creatures {
            if !world.in_village_moment(creature.id)
                || creature.state.action != ActionKind::SocialPlay
            {
                continue;
            }
            match creature.state.attention.and_then(|pose| pose.gesture) {
                Some(Gesture::Bop) => bopped += 1,
                Some(Gesture::Cheer) => cheered += 1,
                other => panic!("a dancer showed {other:?}"),
            }
        }
    }
    assert!(bopped > cheered * 4, "{bopped} {cheered}");
    assert!(cheered >= 3 * 20, "every dancer's own finish: {cheered}");
    for creature in &world.save.creatures {
        assert_eq!(
            creature.state.attention, None,
            "the dance leaves no pose behind"
        );
    }

    let (mut still, desktop, _) = willing_village(82, 3);
    still.save.settings.reduce_motion = true;
    assert_eq!(
        still.available_village_moments(),
        [VillageMoment::Picnic, VillageMoment::Nap]
    );
    assert!(!invite(&mut still, &desktop, VillageMoment::Dance));
}

/// Reduced motion keeps the colony where it stands, so a shared moment is shared in place.
#[test]
fn reduced_motion_shares_a_moment_where_everyone_stands() {
    let (mut world, desktop, now) = willing_village(83, 3);
    world.save.settings.reduce_motion = true;
    let before: Vec<Point> = world
        .save
        .creatures
        .iter()
        .map(|creature| creature.state.position)
        .collect();
    assert!(invite(&mut world, &desktop, VillageMoment::Nap));
    until_together(&mut world, &desktop, now);
    for _ in 0..40 {
        world.tick(now, 0.05, &desktop);
    }
    let mut sleeping = 0;
    for (creature, at) in world.save.creatures.iter().zip(before) {
        assert_eq!(creature.state.position, at, "nobody moved");
        if world.in_village_moment(creature.id) {
            assert_eq!(creature.state.action, ActionKind::Sleep);
            sleeping += 1;
        }
    }
    assert!(sleeping >= 2);
}

/// Everyone answers for themselves. A companion already asleep stays asleep, and a moment that
/// fewer than two want is not had at all: the one who would have come looks round for company.
#[test]
fn a_moment_too_few_want_is_not_had() {
    let (mut world, desktop, _) = willing_village(84, 3);
    for creature in world.save.creatures.iter_mut().skip(1) {
        creature.state.action = ActionKind::Sleep;
    }
    assert!(!invite(&mut world, &desktop, VillageMoment::Picnic));
    assert_eq!(world.village_moment(), None);
    let bubbles: Vec<_> = world
        .thought_bubbles()
        .iter()
        .map(|bubble| (bubble.creature_id, bubble.icon))
        .collect();
    let host = world.save.creatures[0].id;
    assert!(
        bubbles.contains(&(host, BubbleIcon::Question)),
        "{bubbles:?}"
    );
    for sleeper in world.save.creatures.iter().skip(1) {
        assert!(bubbles.contains(&(sleeper.id, BubbleIcon::Sleepy)));
        assert_eq!(sleeper.state.action, ActionKind::Sleep, "left asleep");
    }
}

/// A moment can be stopped, and hiding or pausing the colony ends one too. A moment cut short
/// is not written down and brings nobody closer.
#[test]
fn a_moment_cut_short_is_neither_written_down_nor_a_bond() {
    for ending in 0..3 {
        let (mut world, desktop, now) = willing_village(85, 3);
        assert!(invite(&mut world, &desktop, VillageMoment::Nap));
        until_together(&mut world, &desktop, now);
        for _ in 0..40 {
            world.tick(now, 0.05, &desktop);
        }
        world.drain_events().for_each(drop);
        match ending {
            0 => assert!(world.handle_command(WorldCommand::StopVillageMoment, &desktop)),
            1 => world.save.settings.visible = false,
            _ => world.save.settings.paused = true,
        }
        world.tick(now, 0.05, &desktop);
        assert_eq!(world.village_moment(), None, "ending {ending}");
        let events: Vec<_> = world.drain_events().collect();
        assert!(events.contains(&WorldEvent::RitualInterrupted {
            kind: RitualKind::GroupNap
        }));
        assert!(!events.iter().any(|event| matches!(
            event,
            WorldEvent::BondInteraction { .. } | WorldEvent::RitualCompleted { .. }
        )));
        assert!(
            !world
                .save
                .companion
                .journal
                .iter()
                .any(|entry| entry.moment == JournalMoment::Ritual(RitualKind::GroupNap))
        );
        for creature in &world.save.creatures {
            assert_ne!(creature.state.action, ActionKind::Sleep, "everyone woke");
        }
    }
}

/// Nothing is offered unless the houses are out, in sight, and not on hold, and a moment asked
/// for when that has stopped being true is refused, whatever the menu showed.
#[test]
fn a_moment_is_offered_only_while_the_village_is_out() {
    let (mut world, desktop, now) = willing_village(86, 3);
    world.save.settings.paused = true;
    assert!(world.available_village_moments().is_empty());
    assert!(!invite(&mut world, &desktop, VillageMoment::Picnic));
    world.save.settings.paused = false;
    world.dismiss_home(now, false);
    assert!(world.available_village_moments().is_empty());
    assert!(!invite(&mut world, &desktop, VillageMoment::Picnic));
    // A colony of one has nobody to share anything with.
    let (mut alone, desktop, _) = willing_village(86, 1);
    assert!(alone.available_village_moments().is_empty());
    assert!(!invite(&mut alone, &desktop, VillageMoment::Nap));
}

/// The houses stay out for a moment under way, however long they have already been out, and go
/// once it is over.
#[test]
fn the_houses_wait_for_a_moment_to_finish() {
    let (mut world, desktop, now) = willing_village(87, 2);
    assert!(invite(&mut world, &desktop, VillageMoment::Picnic));
    until_together(&mut world, &desktop, now);
    world.save.home.active_since_utc = Some(now - HOME_DURATION - Duration::minutes(1));
    world.tick(now, 0.05, &desktop);
    assert!(world.save.home.is_active());
    while world.village_moment().is_some() {
        world.tick(now, 0.05, &desktop);
    }
    world.tick(now, 0.05, &desktop);
    assert!(
        !world.save.home.is_active(),
        "and then go as they would have"
    );
}

/// A pet is answered in the middle of a moment, and the moment carries on. Picking a companion
/// up sends the houses away, as it always has, and the moment goes with them. Holding something
/// out to one takes just that one out, and the others carry on while there are still two.
#[test]
fn a_pet_leaves_a_moment_alone_and_a_pick_up_ends_the_visit() {
    let (mut world, desktop, now) = willing_village(88, 3);
    assert!(invite(&mut world, &desktop, VillageMoment::Picnic));
    until_together(&mut world, &desktop, now);
    let first = world.save.creatures[0].id;
    let at = world.save.creatures[0].state.position;
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id: first,
            cursor: at,
        },
        &desktop,
    ));
    world.tick(now, 0.05, &desktop);
    assert!(
        world.in_village_moment(first),
        "a press that may yet be a pet leaves it in the moment"
    );
    world.handle_command(
        WorldCommand::EndInteraction {
            cursor: at,
            velocity: Point::default(),
        },
        &desktop,
    );
    world.tick(now, 0.05, &desktop);
    assert!(
        world.in_village_moment(first),
        "petted, and still picnicking"
    );
    assert!(world.save.home.is_active());

    // Something held out mid-picnic is turned down: it is busy eating already.
    let second = world.save.creatures[1].id;
    assert!(!world.begin_home_moment(second, ActionKind::Eat, 4.0));
    assert!(world.in_village_moment(second));

    // Picked up: the houses go, and the moment with them.
    world.drain_events().for_each(drop);
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id: first,
            cursor: at,
        },
        &desktop,
    ));
    world.handle_command(
        WorldCommand::UpdateInteraction {
            cursor: Point {
                x: at.x + 60.0,
                y: at.y - 40.0,
            },
            velocity: Point::default(),
        },
        &desktop,
    );
    world.tick(now, 0.05, &desktop);
    assert_eq!(world.village_moment(), None);
    assert!(!world.save.home.is_active());
    assert!(world.drain_events().any(|event| event
        == WorldEvent::RitualInterrupted {
            kind: RitualKind::Picnic
        }));
}

/// Something held out while the village is still gathering is taken, and that companion steps out
/// of the moment; the others carry on while there are still two.
#[test]
fn something_held_out_on_the_way_is_taken_and_the_rest_carry_on() {
    let (mut world, desktop, now) = willing_village(89, 3);
    assert!(invite(&mut world, &desktop, VillageMoment::Picnic));
    world.tick(now, 0.05, &desktop);
    assert!(!world.village_moment.as_ref().unwrap().together);
    let second = world.save.creatures[1].id;
    assert!(world.begin_home_moment(second, ActionKind::Eat, 4.0));
    world.tick(now, 0.05, &desktop);
    assert!(!world.in_village_moment(second));
    assert_eq!(world.village_moment(), Some(VillageMoment::Picnic));
    assert_eq!(world.village_moment.as_ref().unwrap().places.len(), 2);
}
