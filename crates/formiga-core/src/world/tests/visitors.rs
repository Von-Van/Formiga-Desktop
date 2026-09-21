use super::super::visitors::wanderer_due;
use super::*;

/// A colony of two grown companions, standing at their houses with the village out.
fn colony_of_two(seed: [u8; 32], created: OffsetDateTime) -> World {
    let desktop = desktop();
    let mut world = World::new(seed, created, &desktop);
    let mut friend = World::preview_adult([99; 32], created, &desktop);
    friend.colony_order = 1;
    friend.name = "Pebble".into();
    world.save.creatures.push(friend);
    let mut world = World::from_save(world.save);
    for creature in &mut world.save.creatures {
        creature.state.arrival_delay_secs = 0.0;
    }
    world
}

/// Start a fresh gathering the way the ordinary cycle does, so the visitor count advances.
fn gather(world: &mut World, now: OffsetDateTime, desktop: &DesktopSnapshot) {
    world.dismiss_home(now, false);
    world.tick(now, 0.0, desktop);
    world.drain_events().for_each(drop);
    assert!(world.handle_command(WorldCommand::SendHome, desktop));
}

/// Live `seconds` of colony time in fiftieth-of-a-second steps, and return where the clock got to.
fn run(
    world: &mut World,
    from: OffsetDateTime,
    seconds: f32,
    desktop: &DesktopSnapshot,
) -> OffsetDateTime {
    let steps = (seconds / 0.05).round() as i64;
    for step in 1..=steps {
        world.tick(from + Duration::milliseconds(step * 50), 0.05, desktop);
        world.drain_events().for_each(drop);
    }
    from + Duration::milliseconds(steps * 50)
}

fn phase(world: &World) -> Option<VisitPhase> {
    world
        .save
        .visitors
        .guest
        .as_ref()
        .map(|guest| guest.visit.phase)
}

/// The colony seed of a colony whose very next gathering brings someone by.
fn seed_expecting_a_visitor() -> [u8; 32] {
    (0..=u8::MAX)
        .map(|byte| [byte; 32])
        .find(|seed| wanderer_due(*seed, 0))
        .expect("some colony meets a visitor at its first counted gathering")
}

#[test]
fn a_guest_leaves_the_desktop_when_the_home_goes() {
    let desktop = desktop();
    let now = datetime!(2026-09-18 12:00 UTC);
    let mut world = two_creature_world([5; 32], now);
    let mut creature = world.save.creatures[0].clone();
    creature.id = 77;
    let mut guest = Visitor::new(creature, VisitorSource::Wanderer, None);
    guest.on_stage = true;
    world.save.visitors.guest = Some(guest);
    assert!(world.handle_command(WorldCommand::SendHome, &desktop));
    assert!(world.save.home.is_active());
    assert_eq!(
        world.save.visitors.on_stage().map(|guest| guest.id),
        Some(77)
    );

    world.dismiss_home(now, false);
    assert!(world.save.visitors.on_stage().is_none());
}

#[test]
fn a_wanderer_comes_by_one_gathering_in_four_with_no_drought_and_no_run() {
    const BLOCKS: u32 = 500;
    for byte in 0..64_u8 {
        let seed = [byte; 32];
        let visits: Vec<u32> = (0..BLOCKS * 4).filter(|o| wanderer_due(seed, *o)).collect();
        // Exactly one gathering in every block of four, so the rate really is one in four.
        assert_eq!(visits.len() as u32, BLOCKS, "colony {byte}");
        for block in 0..BLOCKS {
            assert_eq!(
                visits.iter().filter(|o| **o / 4 == block).count(),
                1,
                "colony {byte}, block {block}"
            );
        }
        let mut run_length = 1;
        for pair in visits.windows(2) {
            let gap = pair[1] - pair[0];
            assert!((1..=7).contains(&gap), "colony {byte} waited {gap}");
            run_length = if gap == 1 { run_length + 1 } else { 1 };
            assert!(run_length <= 2, "colony {byte} had {run_length} in a row");
        }
    }
    // One colony's rhythm is its own, and it is the same every time it is asked.
    let rhythm = |seed| (0..64).map(|o| wanderer_due(seed, o)).collect::<Vec<_>>();
    assert_eq!(rhythm([3; 32]), rhythm([3; 32]));
    assert_ne!(rhythm([3; 32]), rhythm([200; 32]));
}

#[test]
fn wanderers_are_varied_deterministic_and_never_a_colony_members_double() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let members: BTreeSet<_> = colony_of_two([11; 32], created)
        .save
        .creatures
        .iter()
        .map(|creature| creature.id)
        .collect();
    let names: BTreeSet<_> = colony_of_two([11; 32], created)
        .save
        .creatures
        .iter()
        .map(|creature| creature.name.clone())
        .collect();
    let mut designs = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut small = 0;
    let mut met = 0;
    let mut first = Vec::new();
    for ordinal in 0..400_u32 {
        let mut world = colony_of_two([11; 32], created);
        world.save.visitors.gatherings = ordinal;
        world.visitor_home_appeared(created);
        let Some(guest) = world.save.visitors.guest else {
            continue;
        };
        met += 1;
        assert!(!members.contains(&guest.creature.id), "a member's double");
        assert!(!names.contains(&guest.creature.name), "a member's name");
        assert!(
            guest.creature.appearance.design.is_some(),
            "a plain visitor"
        );
        designs.insert(
            guest
                .creature
                .appearance
                .design
                .map(CreatureDesign::to_bytes),
        );
        ids.insert(guest.creature.id);
        small += u32::from(guest.creature.display_scale_percent < 100);
        if ordinal < 40 {
            first.push(guest.creature.id);
        }
    }
    assert_eq!(met, 100, "one gathering in four brings someone by");
    assert_eq!(designs.len(), met, "every visitor looks like itself");
    assert_eq!(ids.len(), met, "no two visitors share an identity");
    // Mostly grown-up visitors, with the occasional small one: neither is a rarity.
    assert!((5..=45).contains(&small), "{small} of {met} were small");

    // The same colony meets the same visitors in the same order, every time.
    let mut again = Vec::new();
    for ordinal in 0..40_u32 {
        let mut world = colony_of_two([11; 32], created);
        world.save.visitors.gatherings = ordinal;
        world.visitor_home_appeared(created);
        if let Some(guest) = world.save.visitors.guest {
            again.push(guest.creature.id);
        }
    }
    assert_eq!(first, again);
}

#[test]
fn a_visit_arrives_after_the_colony_settles_says_hello_once_and_leaves_before_the_end() {
    let desktop = desktop();
    let created = datetime!(2026-04-02 9:00 UTC);
    let mut world = colony_of_two(seed_expecting_a_visitor(), created);
    let mut now = run(&mut world, created, 40.0, &desktop);
    // Everyone is home and settled, and nobody is visiting yet.
    world.save.visitors.gatherings = 0;
    assert!(world.save.visitors.on_stage().is_none());

    gather(&mut world, now, &desktop);
    let start = now;
    let spots: Vec<_> = world
        .save
        .creatures
        .iter()
        .map(|creature| (creature.id, creature.state.position))
        .collect();
    let guest_spot = home_guest_position(
        &world.save.home,
        &colony_cottages(&world.save.creatures),
        world.save.objects.objects.len(),
        &desktop.monitors,
        &world.save.settings.habitat,
        world.save.settings.display_scale,
    )
    .expect("the village has room for a guest");

    let mut guest_id = 0;
    let mut hellos = 0;
    let mut walked_in = false;
    let mut standing = false;
    let mut left_at = None;
    let mut lived = 0;
    for step in 1..=17_600_i64 {
        lived = step;
        now = start + Duration::milliseconds(step * 50);
        world.tick(now, 0.05, &desktop);
        world.drain_events().for_each(drop);
        let seconds = (step as f32) * 0.05;
        hellos += u32::from(
            world
                .thought_bubbles()
                .iter()
                .any(|bubble| bubble.icon == BubbleIcon::Hello && bubble.age < 0.05),
        );
        let phase = phase(&world).expect("the guest stays until it has gone");
        // Nobody turns up at the instant the houses do.
        if seconds < 12.0 {
            assert!(
                world.save.visitors.on_stage().is_none(),
                "someone arrived with the residents at {seconds}s"
            );
        }
        if let Some(guest) = world.save.visitors.on_stage() {
            guest_id = guest.id;
            // A guest is always standing somewhere the habitat allows.
            assert!(
                habitat_contains(
                    &world.save.settings.habitat,
                    &desktop.monitors[0],
                    guest.state.position
                ),
                "the guest stepped outside the habitat at {seconds}s"
            );
            walked_in |= phase == VisitPhase::ArrivingWalk;
            // The hello is said where the guest walked in to; the walk round the houses comes
            // afterwards, and has a test of its own.
            if phase == VisitPhase::Greeting {
                assert_eq!(guest.state.position, guest_spot.1, "at {seconds}s");
                standing = true;
            }
        } else if phase == VisitPhase::Gone {
            left_at.get_or_insert(seconds);
            break;
        }
        // The residents keep their places at the houses whatever the guest does.
        for (id, spot) in &spots {
            let resident = world
                .save
                .creatures
                .iter()
                .find(|creature| creature.id == *id)
                .expect("the colony is whole");
            // Residents walk their own commons while a guest is here — what they must not do is
            // leave the village to follow it about.
            assert!(
                (resident.state.position.x - spot.x).abs() <= VILLAGE_SPAN_LIMIT * 4.0
                    && (resident.state.position.y - spot.y).abs() <= 1.0,
                "{} left the village at {seconds}s",
                resident.name
            );
        }
    }
    assert!(walked_in, "the guest never walked in along the floor");
    assert!(standing, "the guest never stood at the guest spot");
    assert_eq!(hellos, 1, "a visitor says hello once");
    let left_at = left_at.expect("the guest leaves");
    assert!(
        (700.0..900.0).contains(&left_at),
        "the guest left at {left_at}s, not before the gathering ended"
    );
    assert!(world.save.home.is_active(), "the houses outlast the guest");

    // The same gathering without a visitor, to say what the visit itself changed. Standing
    // together at the houses builds familiarity either way; a guest must add nothing to it.
    let mut quiet = colony_of_two(seed_expecting_a_visitor(), created);
    run(&mut quiet, created, 40.0, &desktop);
    // One gathering in four brings someone by, so the next three do not.
    quiet.save.visitors.gatherings = 1;
    gather(&mut quiet, start, &desktop);
    run(&mut quiet, start, lived as f32 * 0.05, &desktop);
    assert!(
        quiet.save.visitors.guest.is_none(),
        "the control had a guest"
    );
    assert_eq!(
        world.save.relationships, quiet.save.relationships,
        "a visit wrote a bond of its own"
    );
    let bonds_of = |world: &World| {
        world
            .save
            .creatures
            .iter()
            .map(|creature| (creature.tendencies, creature.memory.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        bonds_of(&world),
        bonds_of(&quiet),
        "a visit taught the colony something"
    );
    assert!(
        world
            .save
            .relationships
            .iter()
            .all(|bond| !bond.contains(guest_id)),
        "a visitor is never a bond of the colony's"
    );
    let visits: Vec<_> = world
        .save
        .companion
        .journal
        .iter()
        .filter(|entry| matches!(entry.moment, JournalMoment::Visit(_)))
        .collect();
    assert_eq!(visits.len(), 1);
    assert_eq!(visits[0].creature, None);
    assert_eq!(world.save.visitors.guest_book.len(), 1);
    assert_eq!(
        world.save.visitors.guest_book[0].source,
        VisitorSource::Wanderer
    );

    // And when the houses go, the visitor is gone with them rather than left standing.
    world.dismiss_home(now, false);
    assert!(world.save.visitors.guest.is_none());
    assert!(world.save.visitors.on_stage().is_none());
}

#[test]
fn a_guest_answers_the_hello_and_so_does_the_colony_without_leaving_its_places() {
    let desktop = desktop();
    let created = datetime!(2026-04-02 9:00 UTC);
    let mut world = colony_of_two(seed_expecting_a_visitor(), created);
    let now = run(&mut world, created, 40.0, &desktop);
    world.save.visitors.gatherings = 0;
    // One timid companion and one playful one, so both answers are on show.
    world.save.creatures[0].personality.playfulness = 0.95;
    world.save.creatures[0].personality.sociability = 0.95;
    world.save.creatures[1].personality.playfulness = 0.1;
    world.save.creatures[1].personality.boldness = 0.1;
    gather(&mut world, now, &desktop);
    let mut answered = BTreeSet::new();
    let mut gestures = Vec::new();
    let mut greeted = false;
    for step in 1..=1_200_i64 {
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        world.drain_events().for_each(drop);
        greeted |= world
            .save
            .visitors
            .guest
            .as_ref()
            .is_some_and(|guest| guest.visit.greeted);
        for creature in &world.save.creatures {
            if let Some(pose) = creature.state.attention {
                answered.insert(creature.id);
                gestures.push(pose.gesture);
            }
        }
    }
    assert!(greeted, "the guest never said hello");
    assert_eq!(answered.len(), 2, "the colony answered in ones and twos");
    // The playful one bounces where it stands; the timid one only looks.
    assert!(gestures.contains(&Some(Gesture::Bop)));
    assert!(gestures.contains(&None));
    // Their answers are staggered: they do not all turn on the same tick.
    let together = world
        .save
        .creatures
        .iter()
        .filter(|creature| creature.state.attention.is_some())
        .count();
    assert!(together <= 2);
}

#[test]
fn dismissing_the_houses_early_takes_the_guest_with_it() {
    let desktop = desktop();
    let created = datetime!(2026-04-02 9:00 UTC);
    let mut world = colony_of_two(seed_expecting_a_visitor(), created);
    let now = run(&mut world, created, 40.0, &desktop);
    world.save.visitors.gatherings = 0;
    gather(&mut world, now, &desktop);
    let now = run(&mut world, now, 45.0, &desktop);
    assert!(world.save.visitors.on_stage().is_some(), "someone came by");

    world.dismiss_home(now, true);
    assert!(world.save.visitors.on_stage().is_none());
    assert!(world.save.visitors.guest.is_none(), "a wanderer goes home");
    // It really was here, so it is in the book even though it was cut short.
    assert_eq!(world.save.visitors.guest_book.len(), 1);

    // A wanderer that never got as far as the door leaves no trace at all.
    let mut world = colony_of_two(seed_expecting_a_visitor(), created);
    let now = run(&mut world, created, 40.0, &desktop);
    world.save.visitors.gatherings = 0;
    gather(&mut world, now, &desktop);
    let now = run(&mut world, now, 5.0, &desktop);
    assert!(world.save.visitors.guest.is_some());
    assert!(world.save.visitors.on_stage().is_none());
    world.dismiss_home(now, true);
    assert!(world.save.visitors.guest.is_none());
    assert!(world.save.visitors.guest_book.is_empty());
    assert!(
        world
            .save
            .companion
            .journal
            .iter()
            .all(|entry| !matches!(entry.moment, JournalMoment::Visit(_)))
    );
}

#[test]
fn pausing_hiding_and_reduced_motion_each_leave_the_visit_somewhere_defined() {
    let desktop = desktop();
    let created = datetime!(2026-04-02 9:00 UTC);

    // Paused: the guest holds exactly where it was, and nothing about it moves on.
    let mut world = colony_of_two(seed_expecting_a_visitor(), created);
    let now = run(&mut world, created, 40.0, &desktop);
    world.save.visitors.gatherings = 0;
    gather(&mut world, now, &desktop);
    let now = run(&mut world, now, 45.0, &desktop);
    let held = world.save.visitors.guest.clone().expect("someone came by");
    world.save.settings.paused = true;
    run(&mut world, now, 60.0, &desktop);
    assert_eq!(world.save.visitors.guest, Some(held));

    // Hidden: the colony still lives its day, and the guest simply is not drawn.
    let mut world = colony_of_two(seed_expecting_a_visitor(), created);
    let now = run(&mut world, created, 40.0, &desktop);
    world.save.visitors.gatherings = 0;
    world.save.settings.visible = false;
    gather(&mut world, now, &desktop);
    run(&mut world, now, 60.0, &desktop);
    assert!(world.save.visitors.guest.is_some());
    assert!(world.thought_bubbles().is_empty());

    // Reduced motion: no walk in, no walk out, and the hello is said standing still.
    let mut world = colony_of_two(seed_expecting_a_visitor(), created);
    let now = run(&mut world, created, 40.0, &desktop);
    world.save.visitors.gatherings = 0;
    world.save.settings.reduce_motion = true;
    gather(&mut world, now, &desktop);
    let spot = home_guest_position(
        &world.save.home,
        &colony_cottages(&world.save.creatures),
        world.save.objects.objects.len(),
        &desktop.monitors,
        &world.save.settings.habitat,
        world.save.settings.display_scale,
    )
    .expect("the village has room")
    .1;
    let mut seen = Vec::new();
    for step in 1..=17_600_i64 {
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        world.drain_events().for_each(drop);
        // No walk in, no walk out, and no walk around the houses either: a still visit is
        // never given a tour to begin with.
        assert!(
            world
                .save
                .visitors
                .guest
                .as_ref()
                .is_none_or(|guest| guest.visit.stops.is_empty())
        );
        if let Some(guest) = world.save.visitors.on_stage() {
            assert_eq!(guest.state.position, spot, "a still guest stays put");
            assert!(
                guest
                    .state
                    .attention
                    .is_none_or(|pose| pose.gesture.is_none())
            );
        }
        seen.push(phase(&world));
        if phase(&world) == Some(VisitPhase::Gone) {
            break;
        }
    }
    assert!(seen.contains(&Some(VisitPhase::Greeting)));
    assert!(seen.contains(&Some(VisitPhase::Gone)));
    assert!(!seen.contains(&Some(VisitPhase::ArrivingWalk)));
    assert!(!seen.contains(&Some(VisitPhase::LeavingWalk)));
    assert!(
        world
            .save
            .creatures
            .iter()
            .all(|creature| creature.state.attention.is_none_or(|p| p.gesture.is_none()))
    );
}

#[test]
fn a_guest_stays_inside_a_habitat_that_is_redrawn_underneath_it() {
    let desktop = desktop();
    let created = datetime!(2026-04-02 9:00 UTC);
    let mut world = colony_of_two(seed_expecting_a_visitor(), created);
    let now = run(&mut world, created, 40.0, &desktop);
    world.save.visitors.gatherings = 0;
    gather(&mut world, now, &desktop);
    let now = run(&mut world, now, 45.0, &desktop);
    assert!(world.save.visitors.on_stage().is_some());

    // The whole display is put out of bounds: the guest leaves the desktop at once.
    world.save.settings.habitat.zones.push(HabitatZone {
        id: 1,
        display: DisplayKey([1; 16]),
        normalized_bounds: DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        kind: HabitatZoneKind::Excluded,
        enabled: true,
    });
    run(&mut world, now, 0.5, &desktop);
    assert!(world.save.visitors.on_stage().is_none());
    // It was really here, so it is remembered rather than quietly forgotten.
    assert_eq!(world.save.visitors.guest_book.len(), 1);
}

#[test]
fn an_invitation_is_refused_for_a_member_or_while_someone_is_already_visiting() {
    let desktop = desktop();
    let created = datetime!(2026-04-02 9:00 UTC);
    let mut world = colony_of_two([31; 32], created);
    run(&mut world, created, 5.0, &desktop);
    let resident = SharedCreatureSeed::from(world.save.creatures[0].origin);
    assert_eq!(
        world.invite_visitor(resident, created, &desktop),
        Err(VisitorError::AlreadyHome)
    );
    assert!(world.save.visitors.guest.is_none());

    let friend = SharedCreatureSeed {
        source_colony_seed: [77; 32],
        source_generation: 1,
        design: Some(CreatureDesign::generated([77; 32], 1, None)),
    };
    assert_eq!(world.invite_visitor(friend, created, &desktop), Ok(()));
    let guest = world
        .save
        .visitors
        .guest
        .clone()
        .expect("a friend is coming");
    assert_eq!(guest.source, VisitorSource::Invited);
    assert_eq!(
        guest.stays_until_utc,
        Some(world.save.maximum_seen_utc + Duration::hours(24))
    );
    // A code that decodes to the guest's own appearance is a friend we already have over.
    let another = SharedCreatureSeed {
        source_colony_seed: [78; 32],
        source_generation: 0,
        design: None,
    };
    assert_eq!(
        world.invite_visitor(another, created, &desktop),
        Err(VisitorError::GuestPresent)
    );
    // The code the guest hands back recreates exactly the creature that turned up.
    let code = world.visitor_share_code().expect("a guest has a code");
    assert_eq!(decode_creature_seed(&code), Ok(friend));
}

#[test]
fn an_invited_friend_stays_a_day_over_several_gatherings_and_survives_a_relaunch() {
    let desktop = desktop();
    let created = datetime!(2026-04-02 9:00 UTC);
    let mut world = colony_of_two([31; 32], created);
    let mut now = run(&mut world, created, 40.0, &desktop);
    let friend = SharedCreatureSeed {
        source_colony_seed: [77; 32],
        source_generation: 0,
        design: Some(CreatureDesign::generated([77; 32], 0, None)),
    };
    // Inviting calls everyone home, so the friend turns up now rather than in half an hour.
    world.dismiss_home(now, false);
    world.tick(now, 0.0, &desktop);
    assert_eq!(world.invite_visitor(friend, now, &desktop), Ok(()));
    assert!(world.save.home.is_active(), "a gathering starts at once");
    let name = world
        .save
        .visitors
        .guest
        .as_ref()
        .unwrap()
        .creature
        .name
        .clone();
    assert!(
        world.save.creatures.iter().all(|c| c.name != name),
        "a guest never borrows a resident's name"
    );

    now = run(&mut world, now, 40.0, &desktop);
    assert!(world.save.visitors.on_stage().is_some());
    assert_eq!(world.save.visitors.guest_book.len(), 1, "signed once");

    // Put the colony away mid-visit and open it again: the friend is still staying.
    let mut world = World::from_save(world.save.clone());
    let guest = world.save.visitors.guest.clone().expect("still visiting");
    assert_eq!(guest.source, VisitorSource::Invited);
    assert!(!guest.on_stage, "a reopened colony has nobody mid-scene");
    assert_eq!(guest.visit.phase, VisitPhase::Waiting);
    assert!(guest.signed);

    // Three more gatherings through the day, and the friend is at the houses for each.
    for round in 0..3 {
        now += Duration::minutes(31);
        gather(&mut world, now, &desktop);
        now = run(&mut world, now, 40.0, &desktop);
        assert!(
            world.save.visitors.on_stage().is_some(),
            "the friend missed gathering {round}"
        );
        assert_eq!(world.save.visitors.guest_book.len(), 1, "one visit, once");
    }

    // The day is up: the friend simply does not come back, and is not written down twice.
    now += Duration::hours(24);
    gather(&mut world, now, &desktop);
    run(&mut world, now, 40.0, &desktop);
    assert!(
        world
            .save
            .visitors
            .guest
            .as_ref()
            .is_none_or(|guest| guest.source == VisitorSource::Wanderer)
    );
    assert_eq!(world.save.visitors.guest_book.len(), 1);
}

#[test]
fn winding_the_clock_back_never_lengthens_a_stay() {
    let desktop = desktop();
    let created = datetime!(2026-04-02 9:00 UTC);
    let mut world = colony_of_two([31; 32], created);
    let now = run(&mut world, created, 40.0, &desktop);
    let friend = SharedCreatureSeed {
        source_colony_seed: [77; 32],
        source_generation: 0,
        design: None,
    };
    assert_eq!(world.invite_visitor(friend, now, &desktop), Ok(()));
    let until = world.save.visitors.guest.as_ref().unwrap().stays_until_utc;
    assert_eq!(
        until,
        Some(world.save.maximum_seen_utc + Duration::hours(24))
    );

    // A day of colony time passes, and then the machine's clock is wound back a week.
    let later = now + Duration::hours(25);
    world.tick(later, 0.05, &desktop);
    let rolled_back = now - Duration::days(7);
    world.tick(rolled_back, 0.05, &desktop);
    assert_eq!(world.save.maximum_seen_utc, later);
    gather(&mut world, rolled_back, &desktop);
    // The stay is measured on the colony's own timeline, which never runs backwards.
    assert!(
        world
            .save
            .visitors
            .guest
            .as_ref()
            .is_none_or(|guest| guest.source == VisitorSource::Wanderer),
        "a wound-back clock kept the friend here"
    );
}

#[test]
fn the_guest_book_keeps_the_newest_two_dozen_in_order() {
    let created = datetime!(2026-04-02 9:00 UTC);
    let mut state = VisitorState::default();
    for index in 0..80_u32 {
        state.sign(GuestBookEntry {
            visited_at_utc: created + Duration::hours(i64::from(index)),
            name: format!("Guest {index}"),
            origin: CreatureOrigin {
                design: None,
                source_colony_seed: [index as u8; 32],
                source_generation: 0,
            },
            source: if index % 2 == 0 {
                VisitorSource::Wanderer
            } else {
                VisitorSource::Invited
            },
        });
        assert!(state.guest_book.len() <= MAX_GUEST_BOOK_ENTRIES);
    }
    assert_eq!(state.guest_book.len(), MAX_GUEST_BOOK_ENTRIES);
    assert_eq!(state.guest_book[0].name, "Guest 56");
    assert_eq!(
        state.guest_book[MAX_GUEST_BOOK_ENTRIES - 1].name,
        "Guest 79"
    );
    assert!(
        state
            .guest_book
            .windows(2)
            .all(|pair| pair[0].visited_at_utc < pair[1].visited_at_utc),
        "oldest first, newest last"
    );
    // Every line still recreates the visitor it names.
    for entry in &state.guest_book {
        assert_eq!(
            decode_creature_seed(&encode_creature_seed(entry.origin)),
            Ok(SharedCreatureSeed::from(entry.origin))
        );
    }
}

#[test]
fn asking_a_visitor_to_stay_keeps_its_place_and_a_full_colony_cannot() {
    let desktop = desktop();
    let created = datetime!(2026-04-02 9:00 UTC);
    let mut world = colony_of_two(seed_expecting_a_visitor(), created);
    let now = run(&mut world, created, 40.0, &desktop);
    world.save.visitors.gatherings = 0;
    gather(&mut world, now, &desktop);
    let now = run(&mut world, now, 45.0, &desktop);
    let guest = world.save.visitors.guest.clone().expect("someone came by");
    let standing = guest.creature.state.position;
    assert!(world.visitor_can_stay());

    let creature_id = world
        .ask_visitor_to_stay(now, &desktop)
        .expect("there is room");
    let member = world
        .save
        .creatures
        .iter()
        .find(|creature| creature.id == creature_id)
        .expect("the new companion joined");
    // Exactly the creature that was standing there, standing where it already stood.
    assert_eq!(member.state.position, standing);
    assert_eq!(member.origin, guest.creature.origin);
    assert_eq!(member.appearance, guest.creature.appearance);
    assert_eq!(member.personality, guest.creature.personality);
    // A fresh history of its own, not the guest's.
    assert_eq!(member.memory, CreatureMemory::default());
    assert_eq!(member.tendencies, LearnedTendencies::default());
    assert!(world.save.visitors.guest.is_none());
    assert!(!world.visitor_can_stay());
    assert!(world.visitor_share_code().is_none());
    assert_eq!(world.save.visitors.guest_book.len(), 1);
    assert!(
        world
            .thought_bubbles()
            .iter()
            .any(|bubble| bubble.creature_id == creature_id && bubble.icon == BubbleIcon::Stay)
    );

    // A colony with no room can only copy the code and wave the visitor off.
    let mut world = colony_of_two(seed_expecting_a_visitor(), created);
    while world.save.creatures.len() < MAX_COLONY_CREATURES {
        let order = world.save.creatures.len() as u8;
        let mut extra = World::preview_adult([order + 40; 32], created, &desktop);
        extra.colony_order = order;
        extra.name = format!("Spare {order}");
        world.save.creatures.push(extra);
    }
    let now = run(&mut world, created, 40.0, &desktop);
    world.save.visitors.gatherings = 0;
    gather(&mut world, now, &desktop);
    let now = run(&mut world, now, 45.0, &desktop);
    assert!(world.save.visitors.guest.is_some());
    assert!(!world.visitor_can_stay());
    assert!(world.ask_visitor_to_stay(now, &desktop).is_err());
    assert!(
        world.save.visitors.guest.is_some(),
        "the guest is still here"
    );
    assert!(world.visitor_share_code().is_some());
}

#[test]
fn a_guest_can_be_petted_but_never_picked_up() {
    let desktop = desktop();
    let created = datetime!(2026-04-02 9:00 UTC);
    let mut world = colony_of_two(seed_expecting_a_visitor(), created);
    let now = run(&mut world, created, 40.0, &desktop);
    world.save.visitors.gatherings = 0;
    gather(&mut world, now, &desktop);
    let now = run(&mut world, now, 45.0, &desktop);
    let guest_id = world.save.visitors.on_stage().expect("someone came by").id;
    let standing = world.save.visitors.on_stage().unwrap().state.position;

    // A click is a pet, answered with a heart.
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id: guest_id,
            cursor: standing,
        },
        &desktop,
    ));
    assert!(world.handle_command(
        WorldCommand::EndInteraction {
            cursor: standing,
            velocity: Point::default(),
        },
        &desktop,
    ));
    let guest = world.save.visitors.on_stage().expect("still visiting");
    assert_eq!(guest.state.action, ActionKind::PetReaction);
    assert_eq!(guest.state.position, standing);
    assert!(
        world
            .thought_bubbles()
            .iter()
            .any(|bubble| bubble.creature_id == guest_id && bubble.icon == BubbleIcon::Heart)
    );

    // A drag is a pet too: a guest is not ours to carry, and the houses stay out.
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id: guest_id,
            cursor: standing,
        },
        &desktop,
    ));
    assert!(world.handle_command(
        WorldCommand::UpdateInteraction {
            cursor: Point { x: 700.0, y: 300.0 },
            velocity: Point {
                x: 900.0,
                y: -400.0
            },
        },
        &desktop,
    ));
    assert!(!world.is_dragging());
    assert!(world.save.home.is_active(), "a guest never sends home away");
    assert!(world.handle_command(
        WorldCommand::EndInteraction {
            cursor: Point { x: 700.0, y: 300.0 },
            velocity: Point {
                x: 900.0,
                y: -400.0
            },
        },
        &desktop,
    ));
    let guest = world.save.visitors.on_stage().expect("still visiting");
    assert_eq!(guest.state.position, standing, "the guest was not carried");
    assert_eq!(guest.state.action, ActionKind::PetReaction);
    assert!(world.save.home.is_active());

    // Petting leaves no mark on the colony's own history.
    assert!(
        world
            .save
            .creatures
            .iter()
            .all(|creature| creature.memory.times_petted == 0)
    );
    // And the visit picks up again where it left off.
    run(&mut world, now, 4.0, &desktop);
    assert_ne!(
        world
            .save
            .visitors
            .on_stage()
            .expect("still visiting")
            .state
            .action,
        ActionKind::PetReaction
    );
}

/// Every rule a stop on a guest's walk around the village has to keep, checked against the
/// village exactly as it stands right now.
fn every_stop_is_somewhere_a_guest_may_stand(world: &World, desktop: &DesktopSnapshot) {
    let guest = world
        .save
        .visitors
        .guest
        .as_ref()
        .expect("someone is visiting");
    let cottages = colony_cottages(&world.save.creatures);
    let policy = &world.save.settings.habitat;
    let display_scale = world.save.settings.display_scale;
    let monitor = &desktop.monitors[0];
    let scale = f32::from(display_scale) / monitor.scale_factor.max(1.0);
    let clear = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * scale;
    let places = home_object_positions(
        &world.save.home,
        &cottages,
        &desktop.monitors,
        policy,
        display_scale,
    );
    for stop in &guest.visit.stops {
        assert!(
            habitat_contains(policy, monitor, stop.at),
            "{stop:?} is outside the habitat"
        );
        for slot in 0..=cottages.len() {
            if let Some((_, house)) = home_dwelling_position(
                &world.save.home,
                slot,
                &cottages,
                &desktop.monitors,
                policy,
                display_scale,
            ) {
                let kind = if slot == 0 {
                    DwellingKind::Main
                } else {
                    cottages[slot - 1]
                };
                let reach =
                    (kind.width() / 2.0 + CREATURE_FRAME_WIDTH / 2.0 - REST_WALL_SLIVER) * scale;
                assert!(
                    (stop.at.x - house.x).abs() >= reach,
                    "{stop:?} stands over house {slot}'s doorway"
                );
            }
            let residents = world.save.creatures.len().max(1);
            if let Some((_, resting)) = home_resting_position(
                &world.save.home,
                slot,
                residents,
                &cottages,
                &desktop.monitors,
                policy,
                display_scale,
            ) {
                assert!(
                    (stop.at.x - resting.x).abs() >= clear,
                    "{stop:?} crowds the resident at door {slot}"
                );
            }
        }
        for (_, thing) in places[..world.save.objects.objects.len()].iter().flatten() {
            assert!(
                (stop.at.x - thing.x).abs() >= OBJECT_WIDTH / 2.0 * scale,
                "{stop:?} stands on a belonging"
            );
        }
    }
}

/// The stops a guest is touring right now, once it has any.
fn tour_of(world: &World) -> Vec<TourStop> {
    world
        .save
        .visitors
        .guest
        .as_ref()
        .map(|guest| guest.visit.stops.clone())
        .unwrap_or_default()
}

#[test]
fn a_guest_walks_the_village_and_goes_over_to_every_resident_in_turn() {
    let desktop = desktop();
    // The same display, narrower: a village that moves underneath a guest halfway through.
    let mut moved = desktop.clone();
    moved.monitors[0].bounds.width = 1300.0;
    moved.monitors[0].usable_bounds.width = 1300.0;

    let created = datetime!(2026-04-02 9:00 UTC);
    let mut world = colony_of_two(seed_expecting_a_visitor(), created);
    let now = run(&mut world, created, 40.0, &desktop);
    world.save.visitors.gatherings = 0;
    // One playful companion and one timid one, so both kinds of answer are on show.
    world.save.creatures[0].personality.playfulness = 0.95;
    world.save.creatures[0].personality.sociability = 0.95;
    world.save.creatures[1].personality.playfulness = 0.1;
    world.save.creatures[1].personality.boldness = 0.1;
    gather(&mut world, now, &desktop);
    let mut doors: Vec<_> = world
        .save
        .creatures
        .iter()
        .map(|creature| (creature.id, creature.state.position))
        .collect();

    let mut first = Vec::new();
    let mut after_the_move = Vec::new();
    let mut stood = BTreeSet::new();
    let mut greeted = BTreeSet::new();
    let mut answered = BTreeSet::new();
    let mut gestures = Vec::new();
    let mut rounds = 0;
    let mut walked = false;
    let mut left_at = None;
    for step in 1..=17_600_i64 {
        let seconds = step as f32 * 0.05;
        let desktop = if seconds < 300.0 { &desktop } else { &moved };
        world.tick(now + Duration::milliseconds(step * 50), 0.05, desktop);
        world.drain_events().for_each(drop);
        let guest = world
            .save
            .visitors
            .guest
            .as_ref()
            .expect("the guest stays until it has gone");
        if guest.visit.phase == VisitPhase::Gone {
            left_at = Some(seconds);
            break;
        }
        if guest.visit.phase == VisitPhase::Visiting {
            // The stops always belong to the village as it stands right now.
            let spot = home_guest_position(
                &world.save.home,
                &colony_cottages(&world.save.creatures),
                world.save.objects.objects.len(),
                &desktop.monitors,
                &world.save.settings.habitat,
                world.save.settings.display_scale,
            )
            .expect("the village has room for a guest");
            assert_eq!(
                guest.visit.planned,
                Some(spot.1),
                "the tour was not worked out again at {seconds}s"
            );
            assert!(guest.visit.stops.len() <= MAX_TOUR_STOPS);
            assert!(guest.visit.met.len() <= MAX_COLONY_CREATURES);
            assert!(guest.visit.answers.len() <= 2 * MAX_COLONY_CREATURES);
            if let TourMoment::Greeting(creature_id) = guest.visit.moment {
                greeted.insert(creature_id);
                // That resident answers the hello said at this stop, in its own time.
                if let Some(pose) = world
                    .save
                    .creatures
                    .iter()
                    .find(|creature| creature.id == creature_id)
                    .and_then(|creature| creature.state.attention)
                {
                    answered.insert(creature_id);
                    gestures.push(pose.gesture);
                }
            }
            rounds = rounds.max(guest.visit.answers.len());
            walked |= guest.creature.state.action == ActionKind::Traverse;
            if guest.creature.state.action != ActionKind::Traverse {
                stood.insert(guest.creature.state.position.x as i32);
            }
            every_stop_is_somewhere_a_guest_may_stand(&world, desktop);
            if seconds < 300.0 {
                if first.is_empty() {
                    first = tour_of(&world);
                }
            } else if after_the_move.is_empty() {
                after_the_move = tour_of(&world);
            }
        }
        // A narrower display moves everybody's door, and the colony walks to its new one. Once
        // it has settled the doors are noted again; either side of that nobody budges,
        // whatever the guest does.
        if (300.0..320.0).contains(&seconds) {
            doors = world
                .save
                .creatures
                .iter()
                .map(|creature| (creature.id, creature.state.position))
                .collect();
            continue;
        }
        for (creature_id, door) in &doors {
            let resident = world
                .save
                .creatures
                .iter()
                .find(|creature| creature.id == *creature_id)
                .expect("the colony is whole");
            assert!(
                (resident.state.position.x - door.x).abs() <= VILLAGE_SPAN_LIMIT * 4.0
                    && (resident.state.position.y - door.y).abs() <= 1.0,
                "{} left the village at {seconds}s",
                resident.name
            );
        }
    }

    // The guest really does walk the housing area rather than standing to one side of it.
    assert!(first.len() > 1, "the guest was given nowhere to go");
    assert!(walked, "the guest never walked anywhere");
    assert!(
        stood.len() >= first.len(),
        "the guest stood in {} places on a tour of {}",
        stood.len(),
        first.len()
    );
    // The village moved out from under it, and the walk was worked out again around it.
    assert!(
        !after_the_move.is_empty(),
        "the tour survived a narrower display"
    );
    assert_ne!(first, after_the_move, "a moved village kept the old walk");
    // And the guest goes over to each resident in turn rather than talking to one of them.
    assert_eq!(greeted.len(), 2, "the guest went over to {greeted:?}");
    assert_eq!(answered.len(), 2, "a resident never answered");
    // Two answers for the hello itself and one more each for being gone over to.
    assert_eq!(rounds, 4, "the colony answered {rounds} times in all");
    // Each in its own temperament: the playful one bounces, the timid one only looks.
    assert!(gestures.contains(&Some(Gesture::Bop)));
    assert!(gestures.contains(&None));

    let left_at = left_at.expect("the guest leaves");
    assert!(
        (700.0..900.0).contains(&left_at),
        "the guest left at {left_at}s"
    );
    assert!(world.save.home.is_active(), "the houses outlast the guest");
    // One visit, written down once, however far the guest walked while it was here.
    assert_eq!(world.save.visitors.guest_book.len(), 1);
    assert_eq!(
        world
            .save
            .companion
            .journal
            .iter()
            .filter(|entry| matches!(entry.moment, JournalMoment::Visit(_)))
            .count(),
        1
    );

    // The same colony meets the same guest, and it tours the village the same way.
    let mut again = colony_of_two(seed_expecting_a_visitor(), created);
    let now = run(&mut again, created, 40.0, &desktop);
    again.save.visitors.gatherings = 0;
    gather(&mut again, now, &desktop);
    run(&mut again, now, 60.0, &desktop);
    assert_eq!(tour_of(&again), first);
}

/// A favorite is kept apart from the guest book: it outlasts the book moving on, it is never the
/// same visitor twice, a full list waits for one to be forgotten, and inviting it again brings back
/// exactly the visitor it was kept from.
#[test]
fn a_favorite_visitor_outlasts_the_guest_book_and_comes_back_exactly_as_it_was() {
    let desktop = desktop();
    let created = datetime!(2026-04-02 9:00 UTC);
    let mut world = colony_of_two([31; 32], created);
    run(&mut world, created, 5.0, &desktop);
    let friend = SharedCreatureSeed {
        source_colony_seed: [77; 32],
        source_generation: 1,
        design: Some(CreatureDesign::generated([77; 32], 1, None)),
    };
    assert_eq!(world.invite_visitor(friend, created, &desktop), Ok(()));
    let visitor = world.save.visitors.guest.clone().expect("a friend came");
    let origin = visitor.creature.origin;
    let visitors = &mut world.save.visitors;
    assert!(!visitors.is_favorite(&origin));
    assert_eq!(
        visitors.keep_favorite(&visitor.creature.name, origin, created),
        Ok(())
    );
    assert!(visitors.is_favorite(&origin));
    assert_eq!(
        visitors.keep_favorite("Someone else", origin, created),
        Err(FavoriteError::AlreadyKept)
    );
    // The book moves on past the visit the favorite came from; the favorite stays.
    for index in 0..(MAX_GUEST_BOOK_ENTRIES as u8 + 6) {
        visitors.sign(GuestBookEntry {
            visited_at_utc: created,
            name: format!("Passer-by {index}"),
            origin: CreatureOrigin {
                design: None,
                source_colony_seed: [index; 32],
                source_generation: 0,
            },
            source: VisitorSource::Wanderer,
        });
    }
    assert_eq!(visitors.guest_book.len(), MAX_GUEST_BOOK_ENTRIES);
    assert!(visitors.is_favorite(&origin));
    // A full list keeps what it has until one is forgotten.
    for index in 1..MAX_FAVORITE_VISITORS as u8 {
        let other = CreatureOrigin {
            design: None,
            source_colony_seed: [200 + index; 32],
            source_generation: 0,
        };
        assert_eq!(visitors.keep_favorite("Friend", other, created), Ok(()));
    }
    let one_more = CreatureOrigin {
        design: None,
        source_colony_seed: [250; 32],
        source_generation: 0,
    };
    assert_eq!(
        visitors.keep_favorite("One more", one_more, created),
        Err(FavoriteError::Full)
    );
    assert_eq!(visitors.favorites.len(), MAX_FAVORITE_VISITORS);
    assert!(visitors.is_favorite(&origin));
    // Kept through a save, and never twice even if a file says so.
    let mut saved: VisitorState =
        serde_json::from_value(serde_json::to_value(&*visitors).unwrap()).unwrap();
    saved.favorites.push(saved.favorites[0].clone());
    saved.normalize();
    assert_eq!(saved.favorites, visitors.favorites);
    // Invited again, it is the same visitor it was kept from.
    world.save.visitors.guest = None;
    let favorite = world.save.visitors.favorites[0].clone();
    assert_eq!(
        world.invite_visitor(SharedCreatureSeed::from(favorite.origin), created, &desktop),
        Ok(())
    );
    let again = world
        .save
        .visitors
        .guest
        .clone()
        .expect("the favorite came back");
    assert_eq!(again.creature.appearance, visitor.creature.appearance);
    assert_eq!(again.creature.personality, visitor.creature.personality);
    assert_eq!(again.creature.name, favorite.name);
    // Forgetting a favorite while it visits leaves the visit alone.
    assert!(world.save.visitors.forget_favorite(&origin));
    assert!(!world.save.visitors.forget_favorite(&origin));
    assert!(world.save.visitors.guest.is_some());
}
