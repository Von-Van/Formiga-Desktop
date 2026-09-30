use super::*;
use crate::world::antics as antic;

/// Two companions standing about a little apart, with the second given `axes` and `tension` as
/// its temperament, of `kind`.
fn pair_with(kind: TemperamentKind, axes: Axes, tension: Option<Tension>) -> World {
    let mut world = two_creature_world([151; 32], datetime!(2026-09-29 12:00 UTC));
    world.save.creatures[1].temperament = Some(Temperament {
        kind,
        axes,
        tension,
    });
    world
}

fn beat_of(world: &World, index: usize) -> Option<BeatKind> {
    world.save.creatures[index].state.beat.map(|beat| beat.kind)
}

fn bubble_of(world: &World, index: usize) -> Option<BubbleIcon> {
    let id = world.save.creatures[index].id;
    world
        .thought_bubbles()
        .iter()
        .find(|bubble| bubble.creature_id == id)
        .map(|bubble| bubble.icon)
}

/// Something happens, and the colony gets a moment to make something of it.
fn happen(world: &mut World, event: WorldEvent) {
    let now = world.save.maximum_seen_utc;
    world.events.push(event);
    world.project_events(now);
    world.tick(now, 0.05, &desktop());
}

#[test]
fn a_jealous_companion_huffs_when_its_neighbour_is_petted() {
    let jealous = Axes {
        affection: 0.95,
        feistiness: 0.9,
        ..Axes::MIDDLING
    };
    let mut huffed = 0;
    for attempt in 0..8_u8 {
        let mut world = pair_with(TemperamentKind::Sweetheart, jealous, None);
        world.antics = antic::Antics::new(&SeedStream::new([attempt; 32]));
        assert!(world.save.creatures[1].traits().contains(&Trait::Jealous));
        let petted = world.save.creatures[0].id;
        happen(
            &mut world,
            WorldEvent::CreaturePetted {
                creature_id: petted,
            },
        );
        if beat_of(&world, 1) == Some(BeatKind::Huff) {
            huffed += 1;
            assert_eq!(bubble_of(&world, 1), Some(BubbleIcon::Jealous));
            // Nose in the air at whoever got the pet, and a while to itself afterwards.
            let beat = world.save.creatures[1].state.beat.unwrap();
            assert!(beat.look.is_some());
        }
    }
    // Usually, not always.
    assert!((4..8).contains(&huffed), "{huffed} of 8");

    // Somebody who does not mind lets it go.
    let mut calm = pair_with(TemperamentKind::Sweetheart, Axes::MIDDLING, None);
    let petted = calm.save.creatures[0].id;
    happen(
        &mut calm,
        WorldEvent::CreaturePetted {
            creature_id: petted,
        },
    );
    assert_eq!(beat_of(&calm, 1), None);
}

#[test]
fn a_grump_grumbles_at_a_pet_and_a_soft_one_gives_in_to_it() {
    let world = pair_with(
        TemperamentKind::Grump,
        Axes {
            feistiness: 0.9,
            playfulness: 0.1,
            affection: 0.3,
            ..Axes::MIDDLING
        },
        None,
    );
    assert_eq!(
        antic::pet_bubble(&world.save.creatures[1]),
        (BubbleIcon::Grumble, false)
    );
    let world = pair_with(
        TemperamentKind::Grump,
        Axes {
            feistiness: 0.9,
            playfulness: 0.1,
            affection: 0.8,
            ..Axes::MIDDLING
        },
        Some(Tension::GrumpyButAffectionate),
    );
    assert_eq!(
        antic::pet_bubble(&world.save.creatures[1]),
        (BubbleIcon::Grumble, true)
    );
    assert_eq!(
        antic::pet_bubble(&world.save.creatures[0]),
        (BubbleIcon::Heart, false)
    );
}

#[test]
fn a_dramatic_companion_swoons_at_a_fright_and_a_jumpy_one_hides() {
    let dramatic = pair_with(
        TemperamentKind::Showoff,
        Axes {
            boldness: 0.1,
            energy: 0.8,
            social: 0.8,
            ..Axes::MIDDLING
        },
        Some(Tension::DramaticButCowardly),
    );
    let mut world = dramatic;
    let id = world.save.creatures[1].id;
    happen(
        &mut world,
        WorldEvent::WindowReaction {
            creature_id: id,
            action: ActionKind::ReactToWindow,
        },
    );
    assert_eq!(beat_of(&world, 1), Some(BeatKind::Swoon));
    assert_eq!(bubble_of(&world, 1), Some(BubbleIcon::Swoon));

    let mut world = pair_with(
        TemperamentKind::Explorer,
        Axes {
            boldness: 0.85,
            energy: 0.7,
            ..Axes::MIDDLING
        },
        Some(Tension::BraveButNervous),
    );
    let id = world.save.creatures[1].id;
    happen(
        &mut world,
        WorldEvent::WindowReaction {
            creature_id: id,
            action: ActionKind::ReactToWindow,
        },
    );
    assert_eq!(beat_of(&world, 1), Some(BeatKind::Peek));
}

#[test]
fn a_food_lover_begs_when_somebody_nearby_is_eating() {
    let mut begged = 0;
    for attempt in 0..8_u8 {
        let mut world = pair_with(
            TemperamentKind::Lazybones,
            Axes {
                energy: 0.05,
                ..Axes::MIDDLING
            },
            None,
        );
        world.antics = antic::Antics::new(&SeedStream::new([attempt; 32]));
        assert!(
            world.save.creatures[1]
                .traits()
                .contains(&Trait::FoodMotivated)
        );
        let eater = world.save.creatures[0].id;
        happen(
            &mut world,
            WorldEvent::ActionStarted {
                creature_id: eater,
                action: ActionKind::Eat,
            },
        );
        if beat_of(&world, 1) == Some(BeatKind::Beg) {
            begged += 1;
            assert_eq!(bubble_of(&world, 1), Some(BubbleIcon::Snack));
        }
    }
    assert!((3..8).contains(&begged), "{begged} of 8");
}

#[test]
fn a_show_off_strikes_a_pose_after_a_climb_or_a_find() {
    let mut posed = 0;
    for attempt in 0..8_u8 {
        let mut world = pair_with(
            TemperamentKind::Showoff,
            Axes {
                social: 0.9,
                boldness: 0.9,
                ..Axes::MIDDLING
            },
            None,
        );
        world.antics = antic::Antics::new(&SeedStream::new([attempt; 32]));
        let id = world.save.creatures[1].id;
        let action = if attempt % 2 == 0 {
            ActionKind::ClimbWindow
        } else {
            ActionKind::PresentDiscovery
        };
        happen(
            &mut world,
            WorldEvent::ActionCompleted {
                creature_id: id,
                action,
            },
        );
        if beat_of(&world, 1) == Some(BeatKind::Strut) {
            posed += 1;
            assert_eq!(bubble_of(&world, 1), Some(BubbleIcon::Sparkle));
        }
    }
    assert!((3..8).contains(&posed), "{posed} of 8");
}

#[test]
fn a_troublemaker_pounces_on_a_resting_friend_who_jumps_as_it_lands() {
    let mut world = pair_with(
        TemperamentKind::Troublemaker,
        Axes {
            playfulness: 0.9,
            impulsiveness: 0.9,
            ..Axes::MIDDLING
        },
        None,
    );
    world.antics.prank_now();
    let now = world.save.maximum_seen_utc;
    let desktop = desktop();
    world.tick(now, 0.05, &desktop);
    assert_eq!(beat_of(&world, 1), Some(BeatKind::Pounce));
    // The friend has not noticed yet.
    assert_eq!(beat_of(&world, 0), None);
    for _ in 0..22 {
        world.tick(now, 0.05, &desktop);
    }
    assert_eq!(beat_of(&world, 0), Some(BeatKind::Startle));
    assert_eq!(bubble_of(&world, 0), Some(BubbleIcon::Surprise));
}

#[test]
fn a_stubborn_one_stamps_its_foot_when_it_says_no() {
    let mut world = pair_with(
        TemperamentKind::Sweetheart,
        Axes {
            feistiness: 0.2,
            impulsiveness: 0.1,
            ..Axes::MIDDLING
        },
        Some(Tension::GentleButStubborn),
    );
    let id = world.save.creatures[1].id;
    world.antics.cue(antic::Cue::Declined(id));
    let now = world.save.maximum_seen_utc;
    world.tick(now, 0.05, &desktop());
    assert_eq!(beat_of(&world, 1), Some(BeatKind::Stomp));
}

/// A companion has a while to itself after a moment, reduced motion keeps the bubble and leaves
/// out the pose, and a hidden or paused colony has none at all.
#[test]
fn moments_rest_between_them_and_keep_to_the_settings() {
    let dramatic = || {
        pair_with(
            TemperamentKind::Showoff,
            Axes {
                boldness: 0.1,
                energy: 0.8,
                social: 0.8,
                ..Axes::MIDDLING
            },
            Some(Tension::DramaticButCowardly),
        )
    };
    let fright = |world: &mut World| {
        let id = world.save.creatures[1].id;
        happen(
            world,
            WorldEvent::WindowReaction {
                creature_id: id,
                action: ActionKind::ReactToWindow,
            },
        );
    };
    let mut world = dramatic();
    fright(&mut world);
    assert_eq!(beat_of(&world, 1), Some(BeatKind::Swoon));
    // Out of the swoon, and straight into another fright: it is still getting over the last.
    let now = world.save.maximum_seen_utc;
    for _ in 0..80 {
        world.tick(now, 0.05, &desktop());
    }
    assert_eq!(beat_of(&world, 1), None);
    fright(&mut world);
    assert_eq!(beat_of(&world, 1), None);

    let mut world = dramatic();
    world.save.settings.reduce_motion = true;
    fright(&mut world);
    assert_eq!(beat_of(&world, 1), None);
    assert_eq!(bubble_of(&world, 1), Some(BubbleIcon::Swoon));

    for (visible, paused) in [(false, false), (true, true)] {
        let mut world = dramatic();
        world.save.settings.visible = visible;
        world.save.settings.paused = paused;
        fright(&mut world);
        assert_eq!(beat_of(&world, 1), None);
        assert_eq!(world.antics.waiting(), 0);
    }
}

/// Offered a snack, a food-lover takes it far more often than a picky eater does.
#[test]
fn a_food_lover_takes_what_a_picky_eater_turns_down() {
    let takes = |axes: Axes, kind: TemperamentKind| {
        let mut taken = 0;
        for seed in 0..40_u8 {
            let desktop = desktop();
            let mut world = two_creature_world([seed; 32], datetime!(2026-09-29 12:00 UTC));
            let id = world.save.creatures[0].id;
            let creature = creature_mut(&mut world.save.creatures, id).unwrap();
            creature.temperament = Some(Temperament {
                kind,
                axes,
                tension: None,
            });
            creature.state.drives.energy = 0.6;
            world.handle_command(WorldCommand::OfferSnack { creature_id: id }, &desktop);
            // A careful one thinks it over before it answers.
            let now = world.save.maximum_seen_utc;
            let mut answered = false;
            for _ in 0..80 {
                world.tick(now, 0.05, &desktop);
                answered |= world
                    .drain_events()
                    .any(|event| matches!(event, WorldEvent::OfferAnswered { accepted: true, .. }));
            }
            taken += usize::from(answered);
        }
        taken
    };
    let hungry = takes(
        Axes {
            energy: 0.05,
            ..Axes::MIDDLING
        },
        TemperamentKind::Lazybones,
    );
    let picky = takes(
        Axes {
            impulsiveness: 0.05,
            feistiness: 0.9,
            playfulness: 0.3,
            ..Axes::MIDDLING
        },
        TemperamentKind::Scholar,
    );
    assert!(hungry > picky + 8, "food-lover {hungry}, picky {picky}");
}

/// Greeting a visitor, a guardian keeps an eye on it, a shy one hides behind its paws, a show-off
/// shows off and a grump huffs; reduced motion keeps every bubble and leaves out every pose.
#[test]
fn residents_greet_a_visitor_in_character() {
    let cases = [
        (
            TemperamentKind::Guardian,
            Axes {
                suspicion: 0.9,
                boldness: 0.8,
                affection: 0.8,
                ..Axes::MIDDLING
            },
            Gesture::Watch,
            BubbleIcon::Watching,
        ),
        (
            TemperamentKind::Wallflower,
            Axes {
                social: 0.1,
                boldness: 0.1,
                ..Axes::MIDDLING
            },
            Gesture::Peek,
            BubbleIcon::Blush,
        ),
        (
            TemperamentKind::Showoff,
            Axes {
                social: 0.9,
                boldness: 0.9,
                affection: 0.1,
                ..Axes::MIDDLING
            },
            Gesture::Strut,
            BubbleIcon::Sparkle,
        ),
        (
            TemperamentKind::Grump,
            Axes {
                feistiness: 0.9,
                playfulness: 0.1,
                social: 0.2,
                ..Axes::MIDDLING
            },
            Gesture::Huff,
            BubbleIcon::Grumble,
        ),
    ];
    for (kind, axes, gesture, bubble) in cases {
        let world = pair_with(kind, axes, None);
        let creature = &world.save.creatures[1];
        let answer = super::super::visitors::resident_answer(creature, 0, false);
        assert_eq!(answer.gesture, Some(gesture), "{kind:?}");
        assert_eq!(answer.bubble, Some(bubble), "{kind:?}");
        let calm = super::super::visitors::resident_answer(creature, 0, true);
        assert_eq!(calm.gesture, None, "{kind:?}");
        assert_eq!(calm.bubble, Some(bubble), "{kind:?}");
    }
}
