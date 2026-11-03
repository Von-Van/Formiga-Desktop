use super::*;
use crate::talk::{TalkStyle, TalkTopic};

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
