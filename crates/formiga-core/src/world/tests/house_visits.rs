use super::*;

fn state_of(world: &World, id: CreatureId) -> CreatureState {
    world
        .save
        .creatures
        .iter()
        .find(|creature| creature.id == id)
        .expect("still in the colony")
        .state
        .clone()
}

#[test]
fn a_household_away_in_a_house_is_left_alone_and_comes_back_out_where_it_went_in() {
    let created = datetime!(2026-10-06 9:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([81; 32], created);
    let (away, here) = (world.save.creatures[0].id, world.save.creatures[1].id);
    for creature in &mut world.save.creatures {
        creature.state.action_duration = 0.1;
    }
    let went_in = state_of(&world, away).position;
    assert_eq!(world.begin_house_visit(&[away], &desktop), vec![away]);
    assert!(world.away_in_a_house(away) && !world.away_in_a_house(here));
    assert!(state_of(&world, away).indoors);
    let mut now = created + Duration::hours(2);
    let start = state_of(&world, here).position;
    let mut others_moved = false;
    for _ in 0..1_200 {
        now += Duration::milliseconds(50);
        world.tick(now, 0.05, &desktop);
        others_moved |= state_of(&world, here).position != start;
        assert_eq!(state_of(&world, away).position, went_in, "nobody moved it");
        assert!(state_of(&world, away).indoors);
    }
    assert!(others_moved, "the world went on for everybody else");
    assert!(!state_of(&world, here).indoors);
    // Nothing is offered to it, and a gathering leaves it where it is.
    assert!(!world.handle_command(WorldCommand::OfferSnack { creature_id: away }, &desktop));
    world.handle_command(WorldCommand::GatherCreatures, &desktop);
    assert_eq!(state_of(&world, away).position, went_in);
    assert!(world.end_house_visit());
    assert!(!world.away_in_a_house(away));
    let back = state_of(&world, away);
    assert!(!back.indoors);
    assert_eq!(back.position, went_in);
    assert_eq!(back.action, ActionKind::Idle);
    assert!(!world.end_house_visit(), "nobody is away any more");
}

#[test]
fn only_companions_who_are_here_and_free_can_be_lent_and_one_visit_is_open_at_a_time() {
    let created = datetime!(2026-10-06 9:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([82; 32], created);
    let (held, free) = (world.save.creatures[0].id, world.save.creatures[1].id);
    let cursor = state_of(&world, held).position;
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id: held,
            cursor,
        },
        &desktop,
    ));
    assert_eq!(
        world.begin_house_visit(&[held, free, 999_999, free], &desktop),
        vec![free],
        "not the one in hand, nobody unknown, and nobody twice"
    );
    world.handle_command(WorldCommand::CancelInteraction, &desktop);
    // Opening another house closes the first: its household comes back out.
    assert_eq!(world.begin_house_visit(&[held], &desktop), vec![held]);
    assert!(!world.away_in_a_house(free) && world.away_in_a_house(held));
    assert!(!state_of(&world, free).indoors);
}

#[test]
fn whoever_the_house_does_not_have_just_now_is_out_on_the_desktop_and_can_go_back_in() {
    let created = datetime!(2026-10-06 9:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([83; 32], created);
    let (keeper, friend) = (world.save.creatures[0].id, world.save.creatures[1].id);
    // The friend is lent for the visit but waits out on the desktop until it comes over.
    assert_eq!(world.begin_house_visit(&[keeper], &desktop), vec![keeper]);
    assert!(
        !world.settle_house_visit(&[keeper], &desktop),
        "nothing to change"
    );
    assert!(world.settle_house_visit(&[keeper, friend], &desktop));
    assert!(world.away_in_a_house(friend) && state_of(&world, friend).indoors);
    // The owner sends the keeper out: it is back where it went in, and the friend stays.
    let went_in = state_of(&world, keeper).position;
    assert!(world.settle_house_visit(&[friend], &desktop));
    assert!(!world.away_in_a_house(keeper) && world.away_in_a_house(friend));
    let out = state_of(&world, keeper);
    assert!(!out.indoors);
    assert_eq!(out.position, went_in);
    // Somebody in the owner's hand waits there until it is put down.
    let cursor = out.position;
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id: keeper,
            cursor,
        },
        &desktop,
    ));
    assert!(!world.settle_house_visit(&[keeper, friend], &desktop));
    assert!(!world.away_in_a_house(keeper));
    world.handle_command(WorldCommand::CancelInteraction, &desktop);
    assert!(world.settle_house_visit(&[keeper, friend], &desktop));
    assert!(world.end_house_visit());
    assert!(!world.away_in_a_house(keeper) && !world.away_in_a_house(friend));
}

#[test]
fn time_inside_a_house_is_written_once_in_desktops_own_moment() {
    let created = datetime!(2026-10-06 9:00 UTC);
    let mut world = two_creature_world([83; 32], created);
    let keeper = world.save.creatures[0].id;
    let now = created + Duration::hours(2);
    assert!(world.note_house_visit(keeper, now));
    assert!(
        !world.note_house_visit(keeper, now),
        "the same visit read again"
    );
    assert!(
        !world.note_house_visit(keeper, now + Duration::hours(1)),
        "two visits close together are one line"
    );
    assert!(world.note_house_visit(keeper, now + Duration::hours(7)));
    assert!(
        !world.note_house_visit(424_242, now + Duration::hours(8)),
        "nobody keeps that house"
    );
    let written: Vec<_> = world
        .save
        .companion
        .journal
        .iter()
        .filter(|entry| entry.moment == JournalMoment::HouseVisit)
        .collect();
    assert_eq!(written.len(), 2);
    assert!(written.iter().all(|entry| entry.creature == Some(keeper)));
    assert_eq!(JournalMoment::HouseVisit.kind(), MomentKind::Village);
    assert!(!JournalMoment::HouseVisit.is_noteworthy());
}
