use super::*;
use crate::talk::{Saying, TALK_SECS, TalkStyle, TalkTopic};

const TICK: f32 = 0.05;

/// Nothing a colony does depends on what its companions say to each other, and over a few
/// minutes of wandering they do say things.
#[test]
fn talk_changes_nothing_the_colony_does() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let run = |wipe: bool| {
        let mut world = World::new([65; 32], created, &desktop);
        let now = created + Duration::hours(1);
        let_colony_wander(&mut world, now);
        let mut said = 0;
        for tick in 0..6_000 {
            if wipe {
                world.talk.clear();
            }
            world.tick(now + Duration::milliseconds(50 * tick), TICK, &desktop);
            said += world.talk_bubbles().count();
        }
        let trace = world
            .save
            .creatures
            .iter()
            .map(|creature| (creature.state.position, creature.state.action))
            .collect::<Vec<_>>();
        (trace, said)
    };
    let (kept, said) = run(false);
    let (wiped, _) = run(true);
    assert_eq!(kept, wiped);
    assert!(said > 0, "nobody said anything in five minutes");
}

/// A greeting is a hello, and the friend greeted says hello or sends a heart back a moment later;
/// then the greeter keeps quiet for a while.
#[test]
fn a_hello_is_answered_and_then_there_is_quiet() {
    let created = datetime!(2026-01-01 12:00 UTC);
    let desktop = desktop();
    let mut world = World::new([66; 32], created, &desktop);
    if world.save.creatures.len() < 2 {
        let mut friend = world.save.creatures[0].clone();
        friend.id += 1;
        world.save.creatures.push(friend);
    }
    let (a, b) = (world.save.creatures[0].id, world.save.creatures[1].id);
    let at = world.save.creatures[0].state.position;
    world.save.creatures[1].state.position = Point {
        x: at.x + 60.0,
        ..at
    };
    world.save.creatures[1].state.surface = world.save.creatures[0].state.surface.clone();
    let now = created;
    world.carry_talk(now, TICK);
    world.save.creatures[0].state.action = ActionKind::Greet;
    world.save.creatures[1].state.action = ActionKind::Idle;
    world.carry_talk(now, TICK);
    let hello = world
        .talk_bubbles()
        .find(|bubble| bubble.creature_id == a)
        .expect("a greeting says hello");
    assert_eq!(hello.saying.style, TalkStyle::Chat);
    assert_eq!(hello.saying.topic, TalkTopic::Wave);
    assert!(
        world.talk_bubbles().all(|bubble| bubble.creature_id != b),
        "the answer waits a moment"
    );
    for _ in 0..30 {
        world.carry_talk(now, TICK);
    }
    assert!(
        world.talk_bubbles().any(|bubble| bubble.creature_id == b),
        "and then comes"
    );
    // Greeting again straight away says nothing new.
    for _ in 0..40 {
        world.carry_talk(now, TICK);
    }
    world.save.creatures[0].state.action = ActionKind::Idle;
    world.carry_talk(now, TICK);
    world.save.creatures[0].state.action = ActionKind::Greet;
    world.carry_talk(now, TICK);
    assert!(world.talk_bubbles().all(|bubble| bubble.creature_id != a));
}

const NOON: OffsetDateTime = datetime!(2026-01-01 12:00 UTC);

/// `count` companions standing idle in a row on the floor, `gap` points apart, looked at once by
/// the talk pass so that nothing counts as just started.
fn row_of(count: usize, gap: f32, seed: u8) -> World {
    let desktop = desktop();
    let mut world = World::new([seed; 32], NOON, &desktop);
    world.save.creatures.truncate(1);
    while world.save.creatures.len() < count {
        let mut friend = world.save.creatures[0].clone();
        friend.id = world.save.creatures.last().expect("one at least").id + 1;
        world.save.creatures.push(friend);
    }
    let first = world.save.creatures[0].state.clone();
    for (index, creature) in world.save.creatures.iter_mut().enumerate() {
        let state = &mut creature.state;
        state.position = Point {
            x: 300.0 + gap * index as f32,
            ..first.position
        };
        state.surface = first.surface.clone();
        state.action = ActionKind::Idle;
        state.velocity = Point::default();
        state.attention = None;
        state.beat = None;
        state.play = None;
        state.indoors = false;
    }
    world.action_choices.clear();
    world.talk.clear();
    world.talk_quiet.clear();
    world.carry_talk(NOON, TICK);
    world
}

fn ids(world: &World) -> Vec<CreatureId> {
    world.save.creatures.iter().map(|c| c.id).collect()
}

fn saying_of(world: &World, id: CreatureId) -> Option<Saying> {
    world
        .talk_bubbles()
        .find(|bubble| bubble.creature_id == id)
        .map(|bubble| bubble.saying)
}

fn talk_for(world: &mut World, seconds: f32) {
    for _ in 0..(seconds / TICK).round() as usize {
        world.carry_talk(NOON, TICK);
    }
}

/// A greeting is answered by the friend it was meant for, not by whoever happens to stand
/// nearest, and never by a companion up on another window.
#[test]
fn a_hello_is_answered_by_the_friend_it_was_for() {
    let mut world = row_of(3, 40.0, 70);
    let [a, b, c] = ids(&world)[..] else {
        unreachable!()
    };
    world.action_choices.insert(
        a,
        ActionChoice {
            action: ActionKind::Greet,
            target_creature: Some(c),
            target_point: None,
        },
    );
    world.save.creatures[0].state.action = ActionKind::Greet;
    world.carry_talk(NOON, TICK);
    assert!(saying_of(&world, a).is_some(), "a greeting says hello");
    talk_for(&mut world, 1.5);
    assert!(saying_of(&world, c).is_some(), "the friend greeted answers");
    assert!(
        saying_of(&world, b).is_none(),
        "the one in between does not"
    );

    // With no friend in mind, the nearest one on the same ground answers; one up on a window
    // just above does not, however near.
    let mut world = row_of(3, 40.0, 71);
    let [_, b, c] = ids(&world)[..] else {
        unreachable!()
    };
    world.save.creatures[1].state.surface.window_key = Some(77);
    world.carry_talk(NOON, TICK);
    world.save.creatures[0].state.action = ActionKind::Greet;
    world.carry_talk(NOON, TICK);
    talk_for(&mut world, 1.5);
    assert!(
        saying_of(&world, b).is_none(),
        "on another window, out of the talk"
    );
    assert!(
        saying_of(&world, c).is_some(),
        "the nearest on the floor answers"
    );
}

/// An answer still waiting to be said is dropped if the one it answers is no longer there to hear
/// it, or the one answering has fallen asleep.
#[test]
fn an_answer_that_no_longer_makes_sense_is_not_said() {
    for change in ["walked off", "fell asleep", "went behind a house"] {
        let mut world = row_of(2, 40.0, 72);
        let [a, b] = ids(&world)[..] else {
            unreachable!()
        };
        world.save.creatures[0].state.action = ActionKind::Greet;
        world.carry_talk(NOON, TICK);
        assert!(saying_of(&world, a).is_some());
        match change {
            "walked off" => world.save.creatures[1].state.position.x += 400.0,
            "fell asleep" => world.save.creatures[1].state.action = ActionKind::Sleep,
            _ => world.behind_houses.push(a),
        }
        talk_for(&mut world, 1.5);
        assert!(
            world
                .talk
                .iter()
                .all(|bubble| bubble.creature_id != b || bubble.answering != Some(a)),
            "{change}: no answer"
        );
    }
}

/// A companion asked something while it is still answering somebody else finishes what it is
/// saying first, and answers once that has gone; a third question meanwhile is not queued.
#[test]
fn a_companion_finishes_its_line_before_its_next_answer() {
    let mut world = row_of(3, 40.0, 73);
    let [_, b, c] = ids(&world)[..] else {
        unreachable!()
    };
    // A greets B, who answers.
    world.save.creatures[0].state.action = ActionKind::Greet;
    world.carry_talk(NOON, TICK);
    talk_for(&mut world, 1.0);
    let first = saying_of(&world, b).expect("B answers A");
    // C, the other side of B, asks B to play while B is still talking.
    world.save.creatures[2].state.position.x = world.save.creatures[1].state.position.x + 30.0;
    world.save.creatures[2].state.action = ActionKind::SocialPlay;
    world.action_choices.insert(
        c,
        ActionChoice {
            action: ActionKind::SocialPlay,
            target_creature: Some(b),
            target_point: None,
        },
    );
    world.carry_talk(NOON, TICK);
    assert!(saying_of(&world, c).is_some(), "C asks");
    // B's first answer stays up to the end of its time, unbroken.
    let mut shown = 0.0;
    while saying_of(&world, b) == Some(first) {
        world.carry_talk(NOON, TICK);
        shown += TICK;
        assert!(shown < TALK_SECS, "B's line ends in its own time");
    }
    assert!(
        shown > TALK_SECS - 1.0 - 2.0 * TICK,
        "B's first line was not cut off ({shown} s more)"
    );
    // Then its answer to C comes up.
    talk_for(&mut world, 0.2);
    assert_eq!(
        saying_of(&world, b).map(|saying| saying.topic),
        Some(TalkTopic::Exclaim),
        "and then B answers C"
    );
}

/// Goodnight is said once a companion has lain down, never while it is still walking to bed, and
/// a friend walking to bed still answers.
#[test]
fn goodnight_waits_until_the_companion_has_settled() {
    let mut early = 0;
    let mut settled = 0;
    for seed in 0..40_u8 {
        let mut world = row_of(2, 40.0, seed);
        let [a, _] = ids(&world)[..] else {
            unreachable!()
        };
        world.save.creatures[0].state.action = ActionKind::Sleep;
        world.save.creatures[0].state.velocity.x = 30.0;
        for tick in 0..10 {
            world.carry_talk(NOON + Duration::milliseconds(50 * tick), TICK);
            if saying_of(&world, a).is_some_and(|saying| saying.topic == TalkTopic::Moon) {
                early += 1;
            }
        }
        world.save.creatures[0].state.velocity.x = 0.0;
        world.carry_talk(NOON + Duration::seconds(1), TICK);
        if saying_of(&world, a).is_some_and(|saying| saying.topic == TalkTopic::Moon) {
            settled += 1;
        }
    }
    assert_eq!(early, 0, "no goodnight on the way to bed");
    assert!(settled > 0, "goodnight once lain down ({settled} of 40)");

    // Walking to bed, a friend is awake enough to answer a hello.
    let mut world = row_of(2, 40.0, 74);
    let [_, b] = ids(&world)[..] else {
        unreachable!()
    };
    world.save.creatures[1].state.action = ActionKind::Sleep;
    world.save.creatures[1].state.velocity.x = -30.0;
    world.carry_talk(NOON, TICK);
    world.save.creatures[0].state.action = ActionKind::Greet;
    world.carry_talk(NOON, TICK);
    talk_for(&mut world, 1.5);
    assert!(saying_of(&world, b).is_some());
}

/// A companion out of sight behind a house says nothing, and what it was saying is not drawn
/// while it is there.
#[test]
fn a_companion_behind_a_house_keeps_its_talk_to_itself() {
    let mut world = row_of(2, 40.0, 75);
    let [a, _] = ids(&world)[..] else {
        unreachable!()
    };
    world.behind_houses.push(a);
    world.save.creatures[0].state.action = ActionKind::Greet;
    world.carry_talk(NOON, TICK);
    assert!(world.talk.iter().all(|bubble| bubble.creature_id != a));
    // Saying something as it goes behind one.
    let mut world = row_of(2, 40.0, 75);
    world.save.creatures[0].state.action = ActionKind::Greet;
    world.carry_talk(NOON, TICK);
    assert!(saying_of(&world, a).is_some());
    world.behind_houses.push(a);
    assert!(saying_of(&world, a).is_none(), "hidden with its speaker");
    world.behind_houses.clear();
    assert!(saying_of(&world, a).is_some(), "back when it comes out");
}

/// A ride down under an umbrella is remarked on only when it is what is drawn: not while a small
/// beat of its own is shown in its place.
#[test]
fn a_play_moment_is_remarked_on_only_when_it_is_shown() {
    for beat in [false, true] {
        let mut world = row_of(1, 40.0, 76);
        let [a] = ids(&world)[..] else { unreachable!() };
        let state = &mut world.save.creatures[0].state;
        state.action = ActionKind::Landing;
        state.play = Some(crate::play::Play::umbrella_ride(3.0));
        if beat {
            state.beat = Some(Beat::new(BeatKind::Notice, 2.0));
        }
        world.carry_talk(NOON, TICK);
        let said = saying_of(&world, a).map(|saying| saying.topic);
        if beat {
            assert_eq!(
                said, None,
                "the ride is not shown, so nothing is said of it"
            );
        } else {
            assert_eq!(said, Some(TalkTopic::Umbrella));
        }
    }
}

/// A pair at play is a chase or a game of tag, with no thing in it: the invitation shows
/// pawprints, not a toy that never appears.
#[test]
fn an_invitation_to_play_shows_pawprints() {
    let mut world = row_of(2, 40.0, 77);
    let [a, _] = ids(&world)[..] else {
        unreachable!()
    };
    world.save.creatures[0].state.action = ActionKind::SocialPlay;
    world.carry_talk(NOON, TICK);
    assert_eq!(
        saying_of(&world, a).map(|saying| saying.topic),
        Some(TalkTopic::Footprints)
    );
}

/// The room over a companion answering the person at the desk is kept: a neighbour starts
/// nothing there, and an answer due there waits until the person's answer has gone.
#[test]
fn an_answer_to_the_person_keeps_the_room_over_it() {
    let mut world = row_of(3, 40.0, 78);
    let [a, b, c] = ids(&world)[..] else {
        unreachable!()
    };
    // C is answering the person at the desk, right beside B.
    world.save.creatures[2].state.position.x = world.save.creatures[1].state.position.x + 20.0;
    world.bubbles.push(ThoughtBubble {
        creature_id: c,
        icon: BubbleIcon::Snack,
        age: 0.0,
    });
    // B, next to it, starts a game: nothing is said.
    world.save.creatures[1].state.action = ActionKind::SocialPlay;
    world.carry_talk(NOON, TICK);
    assert!(world.talk.iter().all(|bubble| bubble.creature_id != b));
    // A, further off, greets B: A says hello, and B's answer waits for the room.
    world.save.creatures[0].state.position.x -= 100.0;
    world.carry_talk(NOON, TICK);
    world.save.creatures[0].state.action = ActionKind::Greet;
    world.action_choices.insert(
        a,
        ActionChoice {
            action: ActionKind::Greet,
            target_creature: Some(b),
            target_point: None,
        },
    );
    world.carry_talk(NOON, TICK);
    assert!(saying_of(&world, a).is_some());
    talk_for(&mut world, 1.5);
    assert!(saying_of(&world, b).is_none(), "B waits");
    assert!(world.talk.iter().any(|bubble| bubble.creature_id == b));
    // Once the person's answer has gone, B answers.
    world.bubbles.clear();
    talk_for(&mut world, 0.2);
    assert!(saying_of(&world, b).is_some(), "then B answers");
}
