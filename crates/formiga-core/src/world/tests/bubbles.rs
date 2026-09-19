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
