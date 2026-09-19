use super::*;

/// One colony standing about on the desktop floor, awake, and bold enough to answer at once.
fn offer_world(seed: u8) -> (World, DesktopSnapshot, CreatureId) {
    let desktop = desktop();
    let mut world = two_creature_world([seed; 32], datetime!(2026-09-18 12:00 UTC));
    for creature in &mut world.save.creatures {
        creature.personality.boldness = 0.5;
    }
    let id = world.save.creatures[0].id;
    (world, desktop, id)
}

fn offer(kind: OfferKind, creature_id: CreatureId) -> WorldCommand {
    match kind {
        OfferKind::Snack => WorldCommand::OfferSnack { creature_id },
        OfferKind::Toy => WorldCommand::OfferToy { creature_id },
    }
}

fn accepted(world: &World, creature_id: CreatureId, kind: OfferKind) -> bool {
    let wanted = match kind {
        OfferKind::Snack => ActionKind::Eat,
        OfferKind::Toy => ActionKind::SoloPlay,
    };
    world
        .save
        .creatures
        .iter()
        .any(|creature| creature.id == creature_id && creature.state.action == wanted)
}

/// How many of `seeds` colonies took what was held out, once each, after `prepare`.
fn acceptances(seeds: u8, kind: OfferKind, prepare: impl Fn(&mut Creature)) -> usize {
    (1..=seeds)
        .filter(|seed| {
            let (mut world, desktop, id) = offer_world(*seed);
            prepare(&mut world.save.creatures[0]);
            assert!(world.handle_command(offer(kind, id), &desktop));
            accepted(&world, id, kind)
        })
        .count()
}

fn bubble(world: &World, creature_id: CreatureId) -> Option<BubbleIcon> {
    world
        .thought_bubbles()
        .iter()
        .find(|bubble| bubble.creature_id == creature_id)
        .map(|bubble| bubble.icon)
}

#[test]
fn sending_the_colony_home_starts_an_ordinary_gathering_without_waiting_out_the_cooldown() {
    let desktop = desktop();
    let mut world = two_creature_world([9; 32], datetime!(2026-09-18 12:00 UTC));
    // A gathering has only just ended, so the ordinary cycle would wait a quarter of an hour.
    let now = world.save.maximum_seen_utc;
    let_colony_wander(&mut world, now);
    world.tick(now, 0.05, &desktop);
    assert!(!world.save.home.is_active());

    assert!(world.handle_command(WorldCommand::SendHome, &desktop));
    assert!(world.save.home.is_active());
    assert!(
        world
            .drain_events()
            .any(|event| event == WorldEvent::HomeAppeared)
    );
    // Asking again while everyone is already home changes nothing.
    let started = world.save.home.active_since_utc;
    assert!(world.handle_command(WorldCommand::SendHome, &desktop));
    assert_eq!(world.save.home.active_since_utc, started);

    // It ends like any other gathering, and the cooldown follows it.
    let later = now + time::Duration::minutes(16);
    world.tick(later, 0.05, &desktop);
    assert!(!world.save.home.is_active());
    assert_eq!(world.save.home.last_disappeared_utc, Some(later));
}

#[test]
fn a_paused_colony_is_not_called_home() {
    let desktop = desktop();
    let mut world = two_creature_world([9; 32], datetime!(2026-09-18 12:00 UTC));
    let now = world.save.maximum_seen_utc;
    let_colony_wander(&mut world, now);
    world.save.settings.paused = true;
    assert!(!world.handle_command(WorldCommand::SendHome, &desktop));
    assert!(!world.save.home.is_active());
}

/// Every state in which holding something out is the wrong thing to do. The offer is not made:
/// it answers `false`, shows nothing, and leaves the creature exactly as it found it.
#[test]
fn an_offer_is_not_made_at_all_when_it_would_cut_something_short() {
    /// One wrong moment, arranged on a colony that is otherwise ready to be asked.
    type WrongMoment = fn(&mut World, &DesktopSnapshot, CreatureId);
    let cases: [(&str, WrongMoment); 10] = [
        ("hidden", |world, _, _| world.save.settings.visible = false),
        ("paused", |world, _, _| world.save.settings.paused = true),
        ("not arrived", |world, _, id| {
            creature_mut(&mut world.save.creatures, id)
                .unwrap()
                .state
                .arrival_delay_secs = 4.0;
        }),
        ("dragged", |world, desktop, id| {
            let cursor = world.save.creatures[0].state.position;
            assert!(world.handle_command(
                WorldCommand::BeginInteraction {
                    creature_id: id,
                    cursor
                },
                desktop
            ));
            assert!(world.handle_command(
                WorldCommand::UpdateInteraction {
                    cursor: Point {
                        x: cursor.x + 60.0,
                        y: cursor.y
                    },
                    velocity: Point::default(),
                },
                desktop
            ));
            assert!(world.is_dragging());
        }),
        ("tossed", |world, _, id| {
            world.tosses.insert(
                id,
                TossState {
                    elapsed: 0.0,
                    bounces: 0,
                    last_safe_position: world.save.creatures[0].state.position,
                    last_safe_surface: world.save.creatures[0].state.surface.clone(),
                },
            );
            creature_mut(&mut world.save.creatures, id)
                .unwrap()
                .state
                .action = ActionKind::Tossed;
        }),
        ("mid hop", |world, _, id| {
            let start = world.save.creatures[0].state.position;
            world.window_journeys.insert(
                id,
                WindowJourney::Hop(HopJourney {
                    start,
                    target: Point {
                        x: start.x + 40.0,
                        y: start.y,
                    },
                    surface: world.save.creatures[0].state.surface.clone(),
                    elapsed: 0.0,
                    duration: 1.0,
                }),
            );
        }),
        ("mid route", |world, _, id| {
            world.window_routes.insert(
                id,
                WindowRoutePlan {
                    geometry_hash: 0,
                    remaining: Default::default(),
                    repaired: false,
                },
            );
        }),
        ("climbing", |world, _, id| {
            creature_mut(&mut world.save.creatures, id)
                .unwrap()
                .state
                .action = ActionKind::ClimbWindow;
        }),
        ("squeezing", |world, _, id| {
            creature_mut(&mut world.save.creatures, id)
                .unwrap()
                .state
                .action = ActionKind::SqueezeWindow;
        }),
        ("hanging", |world, _, id| {
            creature_mut(&mut world.save.creatures, id)
                .unwrap()
                .state
                .action = ActionKind::Dangle;
        }),
    ];

    for (label, arrange) in cases {
        for kind in [OfferKind::Snack, OfferKind::Toy] {
            let (mut world, desktop, id) = offer_world(31);
            arrange(&mut world, &desktop, id);
            let before = world.save.creatures.clone();
            let bubbles = world.thought_bubbles().len();
            assert!(
                !world.handle_command(offer(kind, id), &desktop),
                "{label}: the offer should not have been made"
            );
            assert_eq!(
                world.save.creatures, before,
                "{label}: the colony changed anyway"
            );
            assert_eq!(
                world.thought_bubbles().len(),
                bubbles,
                "{label}: something was said anyway"
            );
            assert!(world.offers.is_empty(), "{label}: something was remembered");
        }
    }

    // Nobody by that name, either.
    let (mut world, desktop, _) = offer_world(31);
    assert!(!world.handle_command(offer(OfferKind::Snack, 424_242), &desktop));
    assert!(world.thought_bubbles().is_empty());
}

/// A ritual is the colony's own moment, and a snack does not call anyone out of it.
#[test]
fn a_creature_in_the_middle_of_a_colony_ritual_is_left_alone() {
    let created = datetime!(2026-09-18 12:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([44; 32], created);
    let now = world.save.maximum_seen_utc;
    world.save.ritual.next_at_utc = now;
    for creature in &mut world.save.creatures {
        creature.personality.boldness = 0.5;
        creature.state.action_duration = 0.01;
    }
    for _ in 0..6 {
        world.tick(now, 0.05, &desktop);
        if world.colony_plan.is_some() {
            break;
        }
    }
    let plan = world.colony_plan.as_ref().expect("a ritual started");
    let id = plan.participants[0].creature_id;
    let before = world.save.creatures.clone();
    assert!(!world.handle_command(offer(OfferKind::Snack, id), &desktop));
    assert!(!world.handle_command(offer(OfferKind::Toy, id), &desktop));
    assert!(world.colony_plan.is_some(), "the ritual carries on");
    assert_eq!(world.save.creatures, before);
    assert!(world.thought_bubbles().is_empty());
}

/// Whether a creature says yes follows what it needs and who it is, in the direction anyone at
/// the desk would expect, over enough colonies for one seed's mood not to decide it.
#[test]
fn what_a_creature_wants_decides_the_answer_across_many_colonies() {
    let seeds = 60;
    let hungry = acceptances(seeds, OfferKind::Snack, |creature| {
        creature.state.drives.energy = 0.15;
    });
    let sated = acceptances(seeds, OfferKind::Snack, |creature| {
        creature.state.drives.energy = 1.0;
    });
    assert!(
        hungry > sated + seeds as usize / 4,
        "hungry {hungry}, sated {sated} out of {seeds}"
    );

    let playful = acceptances(seeds, OfferKind::Toy, |creature| {
        creature.personality.playfulness = 1.0;
        creature.state.drives.boredom = 0.95;
    });
    let placid = acceptances(seeds, OfferKind::Toy, |creature| {
        creature.personality.playfulness = 0.0;
        creature.state.drives.boredom = 0.05;
    });
    assert!(
        playful > placid + seeds as usize / 4,
        "playful {playful}, placid {placid} out of {seeds}"
    );

    // A creature that has been thrown about trusts a held-out hand less than a petted one.
    let petted = acceptances(seeds, OfferKind::Snack, |creature| {
        creature.state.drives.energy = 0.6;
        creature.memory.times_petted = 60;
        creature.tendencies.cursor_trust = 100;
        creature.tendencies.sociability = 100;
    });
    let thrown = acceptances(seeds, OfferKind::Snack, |creature| {
        creature.state.drives.energy = 0.6;
        creature.memory.times_tossed = 60;
        creature.tendencies.cursor_trust = -100;
        creature.tendencies.sociability = -100;
    });
    assert!(
        petted > thrown,
        "petted {petted}, thrown {thrown} out of {seeds}"
    );

    // Worn out: neither is worth the trouble, and it says so the same way both times.
    for kind in [OfferKind::Snack, OfferKind::Toy] {
        let taken = acceptances(seeds, kind, |creature| {
            creature.state.drives.sleep_pressure = 0.95;
            creature.state.drives.energy = 0.1;
            creature.state.drives.boredom = 1.0;
            creature.personality.playfulness = 1.0;
        });
        assert_eq!(taken, 0, "an exhausted colony took {taken} of {seeds}");
    }
    let (mut world, desktop, id) = offer_world(7);
    creature_mut(&mut world.save.creatures, id)
        .unwrap()
        .state
        .drives
        .sleep_pressure = 0.95;
    assert!(world.handle_command(offer(OfferKind::Toy, id), &desktop));
    assert_eq!(bubble(&world, id), Some(BubbleIcon::Sleepy));

    // Neither answer is ever a foregone conclusion: an invitation can always be declined.
    let irresistible = acceptances(seeds, OfferKind::Snack, |creature| {
        creature.state.drives.energy = 0.0;
        creature.personality.sociability = 1.0;
        creature.personality.curiosity = 1.0;
        creature.tendencies.cursor_trust = 100;
    });
    assert!(irresistible < seeds as usize, "{irresistible} of {seeds}");
}

/// The same colony, asked the same thing in the same state, answers the same way every time.
#[test]
fn the_answer_is_the_creatures_own_and_the_same_for_the_same_seed() {
    for seed in [3_u8, 17, 58] {
        for kind in [OfferKind::Snack, OfferKind::Toy] {
            let run = || {
                let (mut world, desktop, id) = offer_world(seed);
                creature_mut(&mut world.save.creatures, id)
                    .unwrap()
                    .state
                    .drives
                    .energy = 0.4;
                assert!(world.handle_command(offer(kind, id), &desktop));
                (
                    world.save.creatures[0].state.action,
                    bubble(&world, id),
                    world.save.creatures[0].tendencies,
                )
            };
            assert_eq!(run(), run(), "seed {seed:?} answered differently");
        }
    }
}

/// What a creature accepted it gets to finish: the clip runs its own length and nothing the
/// colony does in the meantime takes it over.
#[test]
fn an_accepted_clip_plays_out_and_is_not_taken_over() {
    for (kind, action, icon) in [
        (OfferKind::Snack, ActionKind::Eat, BubbleIcon::Snack),
        (OfferKind::Toy, ActionKind::SoloPlay, BubbleIcon::Toy),
    ] {
        let (mut world, desktop, id) = offer_world(12);
        let now = world.save.maximum_seen_utc;
        // A ritual is overdue, and the companion keeps reaching the boundary that would start it.
        world.save.ritual.next_at_utc = now;
        world.save.creatures[1].state.action_duration = 0.2;
        {
            let creature = creature_mut(&mut world.save.creatures, id).unwrap();
            creature.state.drives.energy = 0.05;
            creature.state.drives.boredom = 1.0;
            creature.personality.playfulness = 1.0;
        }
        assert!(world.handle_command(offer(kind, id), &desktop));
        assert!(accepted(&world, id, kind), "{action:?} was not accepted");
        assert_eq!(bubble(&world, id), Some(icon));
        let duration = world.save.creatures[0].state.action_duration;
        assert!((5.0..=9.0).contains(&duration), "{duration} is not a meal");

        let mut elapsed = 0.0;
        while world.save.creatures[0].state.action == action {
            world.tick(now, 0.05, &desktop);
            elapsed += 0.05;
            assert!(elapsed < duration + 1.0, "{action:?} outstayed its clip");
        }
        assert!(
            elapsed >= duration,
            "{action:?} was cut off at {elapsed} of {duration}"
        );
        // The ritual really was waiting on it: it gathers everyone once the clip is done.
        let mut gathered = world.colony_plan.is_some();
        for _ in 0..400 {
            if gathered {
                break;
            }
            world.tick(now, 0.05, &desktop);
            gathered = world.colony_plan.is_some();
        }
        assert!(gathered, "no ritual was being held up by {action:?}");
    }
}

/// No is a whole answer. It costs the creature nothing and the person nothing.
#[test]
fn a_decline_is_a_small_look_away_and_nothing_else() {
    let (mut world, mut desktop, id) = offer_world(23);
    desktop.cursor.available = true;
    desktop.cursor.position = Point {
        x: world.save.creatures[0].state.position.x + 40.0,
        y: world.save.creatures[0].state.position.y,
    };
    {
        let creature = creature_mut(&mut world.save.creatures, id).unwrap();
        // Content, unplayful, and facing the hand.
        creature.state.drives.energy = 1.0;
        creature.state.drives.boredom = 0.0;
        creature.state.drives.arousal = 0.0;
        creature.personality.playfulness = 0.0;
        creature.personality.sociability = 0.0;
        creature.personality.curiosity = 0.0;
        creature.tendencies.cursor_trust = -100;
        creature.state.facing_right = true;
    }
    let before = world.save.creatures[0].clone();
    assert!(world.handle_command(offer(OfferKind::Toy, id), &desktop));
    let after = &world.save.creatures[0];
    assert!(!accepted(&world, id, OfferKind::Toy));
    // A toy it cannot see the point of is baffling rather than unwelcome.
    assert_eq!(bubble(&world, id), Some(BubbleIcon::Question));
    assert_eq!(after.state.action, before.state.action);
    assert_eq!(after.state.action_elapsed, before.state.action_elapsed);
    assert_eq!(after.state.drives, before.state.drives);
    assert_eq!(after.state.position, before.state.position);
    assert!(!after.state.facing_right, "it looked away from the hand");
    assert_eq!(after.memory.play_sessions, before.memory.play_sessions);
    assert_eq!(after.memory.times_petted, before.memory.times_petted);
    assert_eq!(after.memory.times_tossed, before.memory.times_tossed);
    assert_eq!(
        after.memory.sleep_interruptions,
        before.memory.sleep_interruptions
    );
    // Nothing is held against the person who asked; being asked is still a kind thing.
    assert!(after.tendencies.cursor_trust > before.tendencies.cursor_trust);
    assert_eq!(after.tendencies.play, before.tendencies.play);
    assert_eq!(
        after.tendencies.sleep_security,
        before.tendencies.sleep_security
    );
}

/// Clicking the same item over and over is a nudge, not a feast.
#[test]
fn offering_the_same_thing_over_and_over_never_produces_back_to_back_meals() {
    let (mut world, desktop, id) = offer_world(5);
    let start = world.save.maximum_seen_utc;
    creature_mut(&mut world.save.creatures, id)
        .unwrap()
        .state
        .drives
        .energy = 0.0;
    assert!(world.handle_command(offer(OfferKind::Snack, id), &desktop));
    assert!(accepted(&world, id, OfferKind::Snack));

    let mut meals = 1;
    let mut seconds = 0_i64;
    for round in 0..40 {
        // Half a minute of the world going by between clicks, and the creature stays hungry.
        seconds += 30;
        for step in 0..600 {
            world.tick(
                start + Duration::seconds(seconds - 30) + Duration::milliseconds(step * 50),
                0.05,
                &desktop,
            );
        }
        creature_mut(&mut world.save.creatures, id)
            .unwrap()
            .state
            .drives
            .energy = 0.0;
        let before = world.save.creatures[0].state.clone();
        assert!(world.handle_command(offer(OfferKind::Snack, id), &desktop));
        let after = &world.save.creatures[0].state;
        let eating = after.action == ActionKind::Eat;
        if before.action == ActionKind::Eat {
            assert!(
                eating && after.action_elapsed == before.action_elapsed,
                "round {round} served a second helping on top of the first"
            );
        } else if eating {
            meals += 1;
        }
    }
    // Half an hour of a hungry creature being clicked at is a handful of meals, not forty.
    assert!(meals < 20, "{meals} meals in half an hour");

    // And two clicks in the same breath only ever answer once.
    let (mut world, desktop, id) = offer_world(5);
    creature_mut(&mut world.save.creatures, id)
        .unwrap()
        .state
        .drives
        .energy = 0.0;
    assert!(world.handle_command(offer(OfferKind::Snack, id), &desktop));
    let first = world.save.creatures[0].state.clone();
    for _ in 0..6 {
        assert!(world.handle_command(offer(OfferKind::Snack, id), &desktop));
        assert_eq!(world.save.creatures[0].state.action, first.action);
        assert_eq!(
            world.save.creatures[0].state.action_elapsed, first.action_elapsed,
            "the meal was restarted"
        );
    }
}

/// A creature that still needs its sleep sleeps through the whole thing; a rested one can be
/// woken, and being woken is the same event it always is.
#[test]
fn a_sleeping_creature_is_only_woken_when_it_has_had_its_rest() {
    let desktop = desktop();
    let (mut world, _, id) = offer_world(19);
    {
        let creature = creature_mut(&mut world.save.creatures, id).unwrap();
        creature.state.action = ActionKind::Sleep;
        creature.state.action_duration = 40.0;
        creature.state.drives.sleep_pressure = 0.7;
        creature.state.drives.energy = 0.1;
    }
    let before = world.save.creatures[0].clone();
    assert!(world.handle_command(offer(OfferKind::Snack, id), &desktop));
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Sleep);
    assert_eq!(world.save.creatures[0].state.drives, before.state.drives);
    assert_eq!(bubble(&world, id), Some(BubbleIcon::Sleepy));
    assert!(
        !world
            .drain_events()
            .any(|event| matches!(event, WorldEvent::SleepInterrupted { .. })),
        "its sleep was not disturbed"
    );

    // Rested, ravenous, and trusting: it wakes for this one.
    let (mut world, _, id) = offer_world(19);
    {
        let creature = creature_mut(&mut world.save.creatures, id).unwrap();
        creature.state.action = ActionKind::Sleep;
        creature.state.action_elapsed = 30.0;
        creature.state.action_duration = 40.0;
        creature.state.drives.sleep_pressure = 0.05;
        creature.state.drives.energy = 0.0;
        creature.tendencies.cursor_trust = 100;
    }
    world.sleep_elapsed.insert(id, 30.0);
    assert!(world.handle_command(offer(OfferKind::Snack, id), &desktop));
    assert!(accepted(&world, id, OfferKind::Snack));
    let events: Vec<_> = world.drain_events().collect();
    assert!(events.contains(&WorldEvent::SleepInterrupted {
        creature_id: id,
        elapsed_seconds: 30
    }));
    assert!(events.contains(&WorldEvent::CreatureWoke { creature_id: id }));
    assert!(!world.sleep_elapsed.contains_key(&id));
}

#[test]
fn a_snack_offered_at_home_is_eaten_at_the_doorstep_and_then_the_resident_settles_again() {
    let desktop = desktop();
    let mut world = two_creature_world([9; 32], datetime!(2026-09-18 12:00 UTC));
    let now = world.save.maximum_seen_utc;
    let id = world.save.creatures[0].id;
    for creature in &mut world.save.creatures {
        creature.personality.boldness = 0.9;
        creature.state.drives.energy = 0.0;
        creature.tendencies.cursor_trust = 100;
    }
    // On the way home is not somewhere anything can be enjoyed.
    assert!(world.handle_command(WorldCommand::SendHome, &desktop));
    world.tick(now, 0.05, &desktop);
    assert!(!world.handle_command(offer(OfferKind::Snack, id), &desktop));

    finish_home_approach(&mut world, now, &desktop);
    creature_mut(&mut world.save.creatures, id)
        .unwrap()
        .state
        .drives
        .energy = 0.0;
    assert!(world.handle_command(offer(OfferKind::Snack, id), &desktop));
    assert_eq!(bubble(&world, id), Some(BubbleIcon::Snack));
    world.tick(now, 0.05, &desktop);
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Eat);
    for _ in 0..400 {
        world.tick(now, 0.05, &desktop);
    }
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Homebound);
    assert!(!world.handle_command(
        WorldCommand::OfferSnack {
            creature_id: 424_242
        },
        &desktop
    ));
}

/// A visitor answers like anyone else, and the colony remembers nothing about it.
#[test]
fn a_guest_answers_for_itself_and_leaves_no_trace() {
    let desktop = desktop();
    let now = datetime!(2026-09-18 12:00 UTC);
    let mut world = two_creature_world([5; 32], now);
    let mut creature = world.save.creatures[0].clone();
    creature.id = 77;
    creature.personality.boldness = 0.9;
    creature.personality.playfulness = 1.0;
    creature.state.drives.boredom = 1.0;
    creature.tendencies = LearnedTendencies::default();
    creature.memory = CreatureMemory::default();
    // The houses come out first: a gathering beginning is what decides who is visiting.
    assert!(world.handle_command(WorldCommand::SendHome, &desktop));
    let mut visitor = Visitor::new(creature, VisitorSource::Wanderer, None);
    visitor.on_stage = true;
    visitor.visit.phase = VisitPhase::Visiting;
    visitor.visit.greeted = true;
    visitor.visit.beat_remaining = 60.0;
    world.save.visitors.guest = Some(visitor);

    assert!(world.handle_command(offer(OfferKind::Toy, 77), &desktop));
    let guest = world.save.visitors.on_stage().expect("the guest is out");
    assert_eq!(guest.state.action, ActionKind::SoloPlay);
    assert_eq!(bubble(&world, 77), Some(BubbleIcon::Toy));
    assert_eq!(guest.tendencies, LearnedTendencies::default());
    assert_eq!(guest.memory, CreatureMemory::default());
    assert!(
        !world
            .drain_events()
            .any(|event| matches!(event, WorldEvent::OfferAnswered { .. })),
        "a stranger's visit teaches the colony nothing"
    );

    // The visit waits while the guest enjoys the toy, then carries on with its own calm beat.
    let now = world.save.maximum_seen_utc;
    let mut enjoyed = 0;
    for _ in 0..400 {
        world.tick(now, 0.05, &desktop);
        let guest = world.save.visitors.on_stage().expect("still visiting");
        if guest.state.action == ActionKind::SoloPlay {
            enjoyed += 1;
        }
    }
    assert!(enjoyed >= 40, "the toy was enjoyed for {enjoyed} ticks");
    let guest = world
        .save
        .visitors
        .on_stage()
        .expect("the guest is still out");
    assert_ne!(
        guest.state.action,
        ActionKind::SoloPlay,
        "and then put down"
    );
    assert_eq!(guest.tendencies, LearnedTendencies::default());
    assert_eq!(guest.memory, CreatureMemory::default());
}

/// Being asked kindly is a good thing to have happen, bounded like every other learned thing and
/// reversible by the contrary experience.
#[test]
fn offers_nudge_trust_within_the_same_bounds_as_everything_else() {
    let desktop = desktop();
    let (mut world, _, id) = offer_world(41);
    creature_mut(&mut world.save.creatures, id)
        .unwrap()
        .state
        .drives
        .energy = 0.0;
    let before = world.save.creatures[0].tendencies;
    assert!(world.handle_command(offer(OfferKind::Snack, id), &desktop));
    let after = world.save.creatures[0].tendencies;
    assert!(after.cursor_trust > before.cursor_trust);
    assert!(
        after.cursor_trust - before.cursor_trust <= 3,
        "one offer is a nudge, not a conversion"
    );

    for accepted in [true, false] {
        for _ in 0..200 {
            world.events.push(WorldEvent::OfferAnswered {
                creature_id: id,
                kind: OfferKind::Toy,
                accepted,
            });
        }
    }
    world.project_events(world.save.maximum_seen_utc);
    assert_eq!(world.save.creatures[0].tendencies.cursor_trust, 100);
    assert_eq!(world.save.creatures[0].tendencies.sociability, 100);
    assert_eq!(
        LearnedTendencies::utility(world.save.creatures[0].tendencies.cursor_trust),
        0.35
    );

    // And it comes back: being thrown about undoes it.
    for _ in 0..40 {
        world.events.push(WorldEvent::DragEnded {
            creature_id: id,
            outcome: DragReleaseKind::Tossed {
                velocity: Point::default(),
            },
        });
    }
    world.project_events(world.save.maximum_seen_utc);
    assert!(world.save.creatures[0].tendencies.cursor_trust < 0);
}

/// A timid creature takes a beat before it answers, and reduced motion answers straight away,
/// calmly, with the same words.
#[test]
fn a_timid_creature_thinks_about_it_and_reduced_motion_answers_at_once() {
    let desktop = desktop();
    let (mut world, _, id) = offer_world(11);
    let now = world.save.maximum_seen_utc;
    {
        let creature = creature_mut(&mut world.save.creatures, id).unwrap();
        creature.personality.boldness = 0.05;
        creature.state.drives.energy = 0.0;
        creature.tendencies.cursor_trust = 100;
        creature.state.facing_right = true;
    }
    assert!(world.handle_command(offer(OfferKind::Snack, id), &desktop));
    assert_eq!(bubble(&world, id), Some(BubbleIcon::Ellipsis));
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Idle);
    for _ in 0..16 {
        world.tick(now, 0.05, &desktop);
    }
    assert_eq!(bubble(&world, id), Some(BubbleIcon::Snack));
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Eat);

    // The same creature, with motion reduced: no beat, no turning about, the same answer.
    let (mut world, mut desktop, id) = offer_world(11);
    desktop.cursor.available = true;
    desktop.cursor.position = Point {
        x: world.save.creatures[0].state.position.x - 40.0,
        y: world.save.creatures[0].state.position.y,
    };
    world.save.settings.reduce_motion = true;
    {
        let creature = creature_mut(&mut world.save.creatures, id).unwrap();
        creature.personality.boldness = 0.05;
        creature.state.drives.energy = 0.0;
        creature.tendencies.cursor_trust = 100;
        creature.state.facing_right = true;
    }
    assert!(world.handle_command(offer(OfferKind::Snack, id), &desktop));
    assert_eq!(bubble(&world, id), Some(BubbleIcon::Snack));
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Eat);
    assert!(
        world.save.creatures[0].state.facing_right,
        "a calm answer does not swing the body about"
    );
    assert_eq!(world.save.creatures[0].state.velocity, Point::default());
}

/// Hours of a desktop being handled: offers, pets and throws all at once, for several colonies.
/// Nothing panics, nobody leaves the habitat, and nothing the moment keeps grows without bound.
#[test]
fn offers_amid_pets_and_throws_leave_the_colony_whole() {
    let desktop = desktop();
    let created = datetime!(2026-05-01 0:00 UTC);
    for seed in [2_u8, 29, 63, 91] {
        let mut world = two_creature_world([seed; 32], created);
        let colony = world.save.creatures.len();
        let start = world.save.maximum_seen_utc;
        for step in 1..=7_200_i64 {
            let now = start + Duration::milliseconds(step * 50);
            world.tick(now, 0.05, &desktop);
            let index = (step as usize / 7) % colony;
            let id = world.save.creatures[index].id;
            let cursor = world.save.creatures[index].state.position;
            match step % 11 {
                0 => {
                    world.handle_command(offer(OfferKind::Snack, id), &desktop);
                }
                3 => {
                    world.handle_command(offer(OfferKind::Toy, id), &desktop);
                }
                5 => {
                    // A pet: press and release without going anywhere.
                    world.handle_command(
                        WorldCommand::BeginInteraction {
                            creature_id: id,
                            cursor,
                        },
                        &desktop,
                    );
                    world.handle_command(
                        WorldCommand::EndInteraction {
                            cursor,
                            velocity: Point::default(),
                        },
                        &desktop,
                    );
                }
                8 => {
                    // A throw: press, drag, and let go at speed.
                    world.handle_command(
                        WorldCommand::BeginInteraction {
                            creature_id: id,
                            cursor,
                        },
                        &desktop,
                    );
                    let moved = Point {
                        x: cursor.x + 120.0,
                        y: cursor.y - 60.0,
                    };
                    world.handle_command(
                        WorldCommand::UpdateInteraction {
                            cursor: moved,
                            velocity: Point {
                                x: 600.0,
                                y: -200.0,
                            },
                        },
                        &desktop,
                    );
                    world.handle_command(
                        WorldCommand::EndInteraction {
                            cursor: moved,
                            velocity: Point {
                                x: 600.0,
                                y: -200.0,
                            },
                        },
                        &desktop,
                    );
                }
                _ => {}
            }
            assert!(
                world.thought_bubbles().len() <= crate::world::bubbles::MAX_BUBBLES,
                "seed {seed}: bubbles outgrew their cap at step {step}"
            );
            assert!(
                world.offers.len() <= colony,
                "seed {seed}: offers outgrew the colony at step {step}"
            );
            world.drain_events().for_each(drop);
        }
        for creature in &world.save.creatures {
            assert!(
                desktop.monitors.iter().any(|monitor| habitat_contains(
                    &world.save.settings.habitat,
                    monitor,
                    creature.state.position
                )) || world.window_journeys.contains_key(&creature.id)
                    || world.tosses.contains_key(&creature.id),
                "seed {seed}: somebody ended up outside the habitat"
            );
            assert!(creature.tendencies.cursor_trust.abs() <= 100);
        }
        // Whatever was handed out, a meal was still a meal and a game still a game, counted once
        // each by the action that finished rather than by the asking.
        let plays: u32 = world
            .save
            .creatures
            .iter()
            .map(|creature| creature.memory.play_sessions)
            .sum();
        assert!(plays < 7_200, "seed {seed}: {plays} play sessions");
    }
}

/// A resident in the middle of one of its small doorstep moments is still at home and still
/// within reach: the offer takes over from the fidget rather than being silently refused.
#[test]
fn an_offer_takes_over_from_a_doorstep_fidget() {
    let desktop = desktop();
    let mut world = two_creature_world([9; 32], datetime!(2026-09-18 12:00 UTC));
    let now = world.save.maximum_seen_utc;
    let id = world.save.creatures[0].id;
    world.save.creatures[0].state.drives.energy = 0.0;
    assert!(world.handle_command(WorldCommand::SendHome, &desktop));
    finish_home_approach(&mut world, now, &desktop);
    assert!(world.begin_home_moment(id, ActionKind::Drink, 30.0));
    world.tick(now, 0.05, &desktop);
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Drink);

    assert!(
        world.handle_command(WorldCommand::OfferSnack { creature_id: id }, &desktop),
        "a fidgeting resident can still be offered something"
    );
}
