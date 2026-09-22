use super::*;
use crate::world::bubbles::{BUBBLE_SECS, MAX_BUBBLES};

#[test]
fn a_bubble_grows_in_holds_and_shrinks_out_then_is_gone() {
    let desktop = desktop();
    let now = datetime!(2026-09-18 12:00 UTC);
    let mut world = World::new([3; 32], now, &desktop);
    let id = world.save.creatures[0].id;
    world.show_bubble(id, BubbleIcon::Heart);
    assert_eq!(world.thought_bubbles().len(), 1);
    assert_eq!(
        world.thought_bubbles()[0].growth(false),
        BubbleGrowth::Small
    );

    let mut seen = vec![BubbleGrowth::Small];
    let mut elapsed = 0.0;
    while !world.thought_bubbles().is_empty() {
        world.tick(now, 0.05, &desktop);
        elapsed += 0.05;
        if let Some(bubble) = world.thought_bubbles().first() {
            let growth = bubble.growth(false);
            if seen.last() != Some(&growth) {
                seen.push(growth);
            }
        }
        assert!(
            elapsed < BUBBLE_SECS + 0.5,
            "a bubble never outstays its welcome"
        );
    }
    assert_eq!(
        seen,
        [
            BubbleGrowth::Small,
            BubbleGrowth::Medium,
            BubbleGrowth::Full,
            BubbleGrowth::Medium,
            BubbleGrowth::Small
        ]
    );
    assert!(elapsed >= BUBBLE_SECS - 0.05);
}

#[test]
fn asking_again_holds_the_bubble_open_instead_of_popping_it_afresh() {
    let desktop = desktop();
    let now = datetime!(2026-09-18 12:00 UTC);
    let mut world = World::new([3; 32], now, &desktop);
    let id = world.save.creatures[0].id;
    world.show_bubble(id, BubbleIcon::Heart);
    for _ in 0..40 {
        world.tick(now, 0.05, &desktop);
    }
    let late = world.thought_bubbles()[0].age;
    world.show_bubble(id, BubbleIcon::Heart);
    let held = world.thought_bubbles()[0];
    assert_eq!(world.thought_bubbles().len(), 1, "never two over one head");
    assert!(held.age < late, "the same answer stays up longer");
    assert_eq!(
        held.growth(false),
        BubbleGrowth::Full,
        "without shrinking first"
    );

    world.show_bubble(id, BubbleIcon::Decline);
    let changed = world.thought_bubbles()[0];
    assert_eq!(changed.icon, BubbleIcon::Decline);
    assert_eq!(changed.growth(false), BubbleGrowth::Full);
}

#[test]
fn bubbles_are_bounded_skip_the_growing_under_reduced_motion_and_vanish_with_the_colony() {
    let desktop = desktop();
    let now = datetime!(2026-09-18 12:00 UTC);
    let mut world = two_creature_world([3; 32], now);
    let ids: Vec<_> = world.save.creatures.iter().map(|c| c.id).collect();
    for round in 0..(MAX_BUBBLES as u64 + 3) {
        world.show_bubble(9_000 + round, BubbleIcon::Hello);
    }
    assert_eq!(world.thought_bubbles().len(), MAX_BUBBLES);
    // Nobody by those names lives here, so the next tick clears them.
    world.tick(now, 0.05, &desktop);
    assert!(world.thought_bubbles().is_empty());

    world.show_bubble(ids[0], BubbleIcon::Snack);
    assert_eq!(world.thought_bubbles()[0].growth(true), BubbleGrowth::Full);

    world.save.settings.visible = false;
    world.tick(now, 0.05, &desktop);
    assert!(
        world.thought_bubbles().is_empty(),
        "a hidden colony shows nothing"
    );
}

/// A companion dropping off says so. Whichever way it got there — the quiet moment a resident
/// picks for itself at its own door, a cushion somebody put down, or an ordinary sleep out on
/// the desktop — falling asleep is one `ActionStarted`, and the bubble follows it, so a resident
/// standing still because it is asleep is not mistaken for one standing still with nothing to do.
#[test]
fn dropping_off_says_so_over_the_one_who_fell_asleep() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = super::home::settled_colony([11; 32], 4, created, &desktop);
    assert!(world.send_home(&desktop));
    for _ in 0..4_000 {
        world.tick(created, 0.05, &desktop);
        let Some(sleeper) = world
            .save
            .creatures
            .iter()
            .find(|creature| creature.state.action == ActionKind::Sleep)
        else {
            continue;
        };
        let sleeper = sleeper.id;
        let bubbles: Vec<_> = world
            .thought_bubbles()
            .iter()
            .map(|bubble| (bubble.creature_id, bubble.icon))
            .collect();
        assert!(
            bubbles.contains(&(sleeper, BubbleIcon::Sleepy)),
            "the one who fell asleep says so: {bubbles:?}"
        );
        assert!(
            world.save.creatures.iter().all(|creature| {
                creature.state.action == ActionKind::Sleep
                    || !bubbles.contains(&(creature.id, BubbleIcon::Sleepy))
            }),
            "nobody still on their feet is saying it: {bubbles:?}"
        );
        return;
    }
    panic!("nobody in the village ever napped");
}
