use super::*;

const CUES: [ActionKind; 8] = [
    ActionKind::Eat,
    ActionKind::Sleep,
    ActionKind::Greet,
    ActionKind::SoloPlay,
    ActionKind::Drink,
    ActionKind::SocialPlay,
    ActionKind::InvestigateCursor,
    ActionKind::Idle,
];

fn out_on_the_desktop(seed: u8) -> (World, DesktopSnapshot, OffsetDateTime) {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([seed; 32], created, &desktop);
    let now = created + Duration::hours(1);
    let_colony_wander(&mut world, now);
    world.save.ritual.next_at_utc = now + Duration::days(1);
    (world, desktop, now)
}

/// Moment after moment of every kind: a companion picks up two habits at most, never two for the
/// same kind of moment, keeps each one once it has it, and each is announced exactly once.
#[test]
fn habits_are_picked_up_one_per_kind_of_moment_and_kept() {
    let (mut world, ..) = out_on_the_desktop(71);
    let mut rng = SeedStream::new([71; 32]).rng("habit-test", 0);
    let mut events = Vec::new();
    let creature = &mut world.save.creatures[0];
    let revision = creature.memory.profile_revision;
    for round in 0..6_000 {
        let before = creature.memory.habits.clone();
        cue_habit(
            creature,
            CUES[round % CUES.len()],
            &mut rng,
            true,
            false,
            &mut events,
        );
        assert!(
            creature.memory.habits.starts_with(&before),
            "a habit, once had, is kept"
        );
    }
    let habits = creature.memory.habits.clone();
    assert_eq!(habits.len(), MAX_HABITS, "{habits:?}");
    assert_ne!(habits[0].cue(), habits[1].cue());
    let announced: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            WorldEvent::HabitLearned { creature_id, habit } => {
                assert_eq!(*creature_id, creature.id);
                Some(*habit)
            }
            _ => None,
        })
        .collect();
    assert_eq!(announced, habits);
    assert_eq!(creature.memory.profile_revision, revision + 2);
}

/// A moment at the door teaches nothing, and an ordinary action has no habit to show.
#[test]
fn only_real_moments_teach_and_only_cued_actions_show_a_habit() {
    let (mut world, ..) = out_on_the_desktop(72);
    let mut rng = SeedStream::new([72; 32]).rng("habit-test", 0);
    let mut events = Vec::new();
    let creature = &mut world.save.creatures[0];
    for round in 0..4_000 {
        cue_habit(
            creature,
            CUES[round % CUES.len()],
            &mut rng,
            false,
            false,
            &mut events,
        );
    }
    assert!(creature.memory.habits.is_empty());
    assert!(events.is_empty());
    creature.memory.habits = vec![Habit::LooksFoodOver, Habit::StretchesBeforeNaps];
    for action in ActionKind::ALL {
        let mut shown = 0;
        for _ in 0..40 {
            cue_habit(creature, action, &mut rng, false, false, &mut events);
            if let Some(flourish) = creature.state.flourish {
                assert_eq!(flourish.action, action);
                assert_eq!(flourish.started_at, None, "it waits to come to a stop");
                assert!(creature.memory.habits.contains(&flourish.habit));
                shown += 1;
            }
        }
        let has_one = HabitCue::for_action(action).is_some_and(|cue| {
            creature
                .memory
                .habits
                .iter()
                .any(|habit| habit.cue() == cue)
        });
        if has_one {
            // Most times, not every time.
            assert!((20..40).contains(&shown), "{action:?} shown {shown} of 40");
        } else {
            assert_eq!(shown, 0, "{action:?}");
        }
    }
}

/// Reduced motion stops the flourish being drawn, as it stops every body pose, but the companion
/// still has its habits and can still pick one up.
#[test]
fn reduced_motion_keeps_habits_and_hides_the_flourish() {
    let (mut world, ..) = out_on_the_desktop(73);
    let mut rng = SeedStream::new([73; 32]).rng("habit-test", 0);
    let mut events = Vec::new();
    let creature = &mut world.save.creatures[0];
    for round in 0..6_000 {
        cue_habit(
            creature,
            CUES[round % CUES.len()],
            &mut rng,
            true,
            true,
            &mut events,
        );
        assert_eq!(creature.state.flourish, None);
    }
    assert_eq!(creature.memory.habits.len(), MAX_HABITS);
}

/// A companion that has its habit does it where it stops, holds still while it does, and then
/// carries on with the very action it opened.
#[test]
fn a_habit_is_done_standing_still_at_the_start_of_its_action() {
    let (mut world, desktop, now) = out_on_the_desktop(74);
    world.save.creatures[0].memory.habits = vec![Habit::PlayBows];
    let mut rng = SeedStream::new([74; 32]).rng("habit-test", 0);
    let mut events = Vec::new();
    let creature = &mut world.save.creatures[0];
    creature.state.position = Point { x: 600.0, y: 846.0 };
    creature.state.velocity = Point { x: 40.0, y: 0.0 };
    creature.state.action = ActionKind::SoloPlay;
    creature.state.action_elapsed = 0.0;
    creature.state.action_duration = 6.0;
    while creature.state.flourish.is_none() {
        cue_habit(
            creature,
            ActionKind::SoloPlay,
            &mut rng,
            false,
            false,
            &mut events,
        );
    }
    let mut shown: Vec<(f32, f32)> = Vec::new();
    for _ in 0..80 {
        world.tick(now, 0.05, &desktop);
        let creature = &world.save.creatures[0];
        assert_eq!(creature.state.action, ActionKind::SoloPlay);
        match creature.state.flourish {
            Some(flourish) => shown.push((
                flourish
                    .progress(&creature.state)
                    .expect("a game alone starts where it is"),
                creature.state.position.x,
            )),
            None => break,
        }
    }
    assert!(
        (19..=21).contains(&shown.len()),
        "a one-second bow at 20 ticks a second: {}",
        shown.len()
    );
    assert!(
        shown.windows(2).all(|pair| pair[0].1 == pair[1].1),
        "it holds still while it bows: {shown:?}"
    );
    let creature = &world.save.creatures[0];
    assert_eq!(creature.state.flourish, None);
    assert_eq!(creature.state.action, ActionKind::SoloPlay, "and plays on");
}

/// A nap that begins with a walk to a pillow stretches once it is there, not on the way.
#[test]
fn a_habit_waits_for_the_walk_its_action_begins_with() {
    let (mut world, desktop, now) = out_on_the_desktop(78);
    world.save.creatures[0].memory.habits = vec![Habit::StretchesBeforeNaps];
    let mut rng = SeedStream::new([78; 32]).rng("habit-test", 0);
    let mut events = Vec::new();
    let creature_id = world.save.creatures[0].id;
    let creature = &mut world.save.creatures[0];
    creature.state.position = Point { x: 600.0, y: 846.0 };
    creature.state.action = ActionKind::Sleep;
    creature.state.action_elapsed = 0.0;
    creature.state.action_duration = 30.0;
    while creature.state.flourish.is_none() {
        cue_habit(
            creature,
            ActionKind::Sleep,
            &mut rng,
            false,
            false,
            &mut events,
        );
    }
    world.action_choices.insert(
        creature_id,
        ActionChoice {
            action: ActionKind::Sleep,
            target_creature: None,
            target_point: Some(Point { x: 700.0, y: 846.0 }),
        },
    );
    let mut walked_while_waiting = false;
    let mut started_at_x = None;
    for _ in 0..200 {
        world.tick(now, 0.05, &desktop);
        let creature = &world.save.creatures[0];
        match creature.state.flourish {
            Some(flourish) if flourish.started_at.is_none() => {
                walked_while_waiting |= creature.state.velocity.x.abs() > 1.0;
            }
            Some(_) => {
                started_at_x.get_or_insert(creature.state.position.x);
            }
            None => break,
        }
    }
    assert!(walked_while_waiting, "it walked to its pillow first");
    let x = started_at_x.expect("and stretched once it got there");
    assert!((x - 700.0).abs() <= 5.0, "stretched at {x}");
}

/// Something taking over from the action ends its flourish on the spot.
#[test]
fn an_interrupted_action_ends_its_flourish() {
    let (mut world, desktop, now) = out_on_the_desktop(75);
    let creature = &mut world.save.creatures[0];
    creature.state.action = ActionKind::Eat;
    creature.state.action_elapsed = 0.0;
    creature.state.action_duration = 8.0;
    creature.state.velocity = Point::default();
    creature.state.flourish = Some(Flourish {
        habit: Habit::LooksFoodOver,
        action: ActionKind::Eat,
        started_at: Some(0.0),
    });
    creature.state.action = ActionKind::Drink;
    world.tick(now, 0.05, &desktop);
    assert_eq!(world.save.creatures[0].state.flourish, None);
}

/// A new habit is a moment worth keeping: it goes in the journal at once, and the colony is
/// written straight away rather than at the next checkpoint.
#[test]
fn a_new_habit_is_written_in_the_journal() {
    let (mut world, _, now) = out_on_the_desktop(76);
    let creature_id = world.save.creatures[0].id;
    let event = WorldEvent::HabitLearned {
        creature_id,
        habit: Habit::StretchesBeforeNaps,
    };
    assert_eq!(event.save_urgency(), SaveUrgency::Prompt);
    world.events.push(event);
    world.project_events(now);
    let entry = world.save.companion.journal.last().expect("an entry");
    assert_eq!(entry.creature, Some(creature_id));
    assert_eq!(
        entry.moment,
        JournalMoment::Habit(Habit::StretchesBeforeNaps)
    );
}

/// Over ordinary days every companion comes to do things its own way: a first habit within its
/// first hours of company, a second some while after, and never more than two.
#[test]
fn companions_pick_up_habits_over_their_first_day() {
    // An hour of moments at the rates a colony of five was measured living out on the desktop:
    // several naps, a few snacks and drinks, greetings and games.
    let hour: Vec<ActionKind> = [
        [ActionKind::Sleep; 6].as_slice(),
        &[ActionKind::Eat, ActionKind::Eat, ActionKind::Drink],
        &[ActionKind::Greet; 3],
        &[ActionKind::SoloPlay, ActionKind::SocialPlay],
    ]
    .concat();
    let mut first = Vec::new();
    let mut second = Vec::new();
    for seed in 0..40_u8 {
        let (mut world, ..) = out_on_the_desktop(seed);
        let mut rng = SeedStream::new([seed; 32]).rng("habit-test", 0);
        let mut events = Vec::new();
        let creature = &mut world.save.creatures[0];
        let (mut learned_first, mut learned_second) = (None, None);
        for hours in 1..=72 {
            for action in &hour {
                cue_habit(creature, *action, &mut rng, true, false, &mut events);
            }
            let count = creature.memory.habits.len();
            if count >= 1 && learned_first.is_none() {
                learned_first = Some(hours);
            }
            if count >= 2 && learned_second.is_none() {
                learned_second = Some(hours);
            }
            assert!(count <= MAX_HABITS);
        }
        first.push(learned_first.unwrap_or(99));
        second.push(learned_second.unwrap_or(99));
    }
    first.sort_unstable();
    second.sort_unstable();
    eprintln!("hours to a first habit: {first:?}");
    eprintln!("hours to a second habit: {second:?}");
    let median = |hours: &[u32]| hours[hours.len() / 2];
    assert!(
        (1..=6).contains(&median(&first)),
        "median first habit after {} hours",
        median(&first)
    );
    assert!(
        (8..=48).contains(&median(&second)),
        "median second habit after {} hours",
        median(&second)
    );
    assert!(
        first.iter().filter(|hours| **hours <= 24).count() >= 36,
        "nearly everyone has a habit by the end of a day of company"
    );
}
