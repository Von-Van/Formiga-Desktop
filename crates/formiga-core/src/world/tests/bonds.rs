use super::*;

#[test]
fn four_creatures_have_exactly_six_canonical_bond_records() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([54; 32], created, &desktop);
    world.tick(created + Duration::days(31), 0.05, &desktop);

    assert_eq!(world.save.creatures.len(), 4);
    assert_eq!(world.save.relationships.len(), MAX_RELATIONSHIPS);
    let pairs: BTreeSet<_> = world
        .save
        .relationships
        .iter()
        .map(|relationship| (relationship.a, relationship.b))
        .collect();
    assert_eq!(pairs.len(), MAX_RELATIONSHIPS);
    assert!(pairs.iter().all(|(a, b)| a < b));
    for creature in &world.save.creatures {
        assert_eq!(
            pairs
                .iter()
                .filter(|(a, b)| *a == creature.id || *b == creature.id)
                .count(),
            3
        );
    }
}

#[test]
fn five_calm_minutes_project_into_one_compact_bond_update() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([55; 32], created);
    let a = world.save.creatures[0].id;
    let b = world.save.creatures[1].id;
    let relationship = relationship_mut_or_insert(&mut world.save.relationships, a, b).unwrap();
    relationship.avoidance = 3;
    let before = *relationship;

    for _ in 0..5 {
        world.sample_observations(OBSERVATION_INTERVAL_SECS, &desktop);
        world.project_events(created + Duration::hours(1));
    }

    let after = *relationship_between(&world.save.relationships, a, b).unwrap();
    assert_eq!(after.familiarity, before.familiarity + 1);
    assert_eq!(after.avoidance, before.avoidance - 1);
    assert!(world.drain_events().any(|event| matches!(
        event,
        WorldEvent::BondInteraction {
            experience: RelationshipExperience::CalmProximity,
            ..
        }
    )));
    assert_eq!(
        world.calm_proximity_seconds.get(&(a.min(b), a.max(b))),
        Some(&0)
    );
}

#[test]
fn every_targeted_sequence_handles_moved_and_unavailable_companions() {
    // The fixture monitor is 2x and the default display scale is 3, so a creature draws 72 points
    // wide and shoulder to shoulder is 57.6 of them.
    const FRAME_WIDTH: f32 = 72.0;
    let clear = FRAME_WIDTH * super::super::spacing::FACE_CLEAR_RATIO + BOND_SETTLE;
    let created = datetime!(2026-01-01 0:00 UTC);
    let world = two_creature_world([56; 32], created);
    let actor = world.save.creatures[0].clone();
    let mut target = world.save.creatures[1].clone();
    let target_id = target.id;
    let social_actions = [
        ActionKind::Follow,
        ActionKind::Sleep,
        ActionKind::PresentDiscovery,
        ActionKind::SocialPlay,
        ActionKind::Greet,
        ActionKind::InspectScreen,
        ActionKind::ReactToWindow,
    ];

    target.state.position = Point { x: 710.0, y: 846.0 };
    let moved = vec![actor.clone(), target.clone()];
    // Standing alongside is the face-clear distance; showing a discovery and following trail a
    // little further back than that.
    assert_eq!(
        bond_target_point(&actor, &moved, target_id, ActionKind::Greet, FRAME_WIDTH),
        Some(Point {
            x: 710.0 - clear,
            y: 846.0
        })
    );
    assert_eq!(
        bond_target_point(
            &actor,
            &moved,
            target_id,
            ActionKind::SocialPlay,
            FRAME_WIDTH
        ),
        Some(Point {
            x: 710.0 - clear,
            y: 846.0
        })
    );
    assert_eq!(
        bond_target_point(&actor, &moved, target_id, ActionKind::Follow, FRAME_WIDTH),
        Some(Point {
            x: 710.0 - (FRAME_WIDTH * super::super::spacing::FACE_CLEAR_RATIO * 1.25 + BOND_SETTLE),
            y: 846.0
        })
    );
    assert_eq!(
        bond_target_point(
            &actor,
            &moved,
            target_id,
            ActionKind::PresentDiscovery,
            FRAME_WIDTH
        ),
        Some(Point {
            x: 710.0 - (FRAME_WIDTH * super::super::spacing::FACE_CLEAR_RATIO * 1.1 + BOND_SETTLE),
            y: 846.0
        })
    );
    let mut actor_on_right = actor.clone();
    actor_on_right.state.position.x = 760.0;
    assert_eq!(
        bond_target_point(
            &actor_on_right,
            &moved,
            target_id,
            ActionKind::Greet,
            FRAME_WIDTH
        ),
        Some(Point {
            x: 710.0 + clear,
            y: 846.0
        })
    );

    target.state.action = ActionKind::Sleep;
    let sleeping = vec![actor.clone(), target.clone()];
    assert!(
        bond_target_point(&actor, &sleeping, target_id, ActionKind::Sleep, FRAME_WIDTH).is_some()
    );
    for action in [
        ActionKind::Follow,
        ActionKind::PresentDiscovery,
        ActionKind::SocialPlay,
        ActionKind::Greet,
        ActionKind::InspectScreen,
    ] {
        assert_eq!(
            bond_target_point(&actor, &sleeping, target_id, action, FRAME_WIDTH),
            None
        );
    }

    target.state.action = ActionKind::Homebound;
    let homebound = vec![actor.clone(), target.clone()];
    for action in social_actions {
        assert_eq!(
            bond_target_point(&actor, &homebound, target_id, action, FRAME_WIDTH),
            None
        );
    }

    target.state.action = ActionKind::Tossed;
    let tossed = vec![actor.clone(), target.clone()];
    assert!(
        bond_target_point(
            &actor,
            &tossed,
            target_id,
            ActionKind::ReactToWindow,
            FRAME_WIDTH
        )
        .is_some()
    );
    for action in social_actions
        .into_iter()
        .filter(|action| *action != ActionKind::ReactToWindow)
    {
        assert_eq!(
            bond_target_point(&actor, &tossed, target_id, action, FRAME_WIDTH),
            None
        );
    }

    target.state.action = ActionKind::ClimbWindow;
    let climbing = vec![actor.clone(), target.clone()];
    assert_eq!(
        bond_target_point(
            &actor,
            &climbing,
            target_id,
            ActionKind::InspectScreen,
            FRAME_WIDTH
        ),
        Some(target.state.position)
    );

    let removed = vec![actor.clone()];
    for action in social_actions {
        assert_eq!(
            bond_target_point(&actor, &removed, target_id, action, FRAME_WIDTH),
            None
        );
        assert_eq!(
            bond_target_point(&actor, &removed, u64::MAX, action, FRAME_WIDTH),
            None
        );
    }
}

#[test]
fn a_bond_plan_tracks_motion_then_cancels_if_the_target_cannot_participate() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut moving = two_creature_world([57; 32], created);
    let actor = moving.save.creatures[0].id;
    let target = moving.save.creatures[1].id;
    moving.save.creatures[1].state.position = Point { x: 820.0, y: 846.0 };
    moving.save.creatures[0].state.action = ActionKind::Follow;
    moving.action_choices.insert(
        actor,
        ActionChoice {
            action: ActionKind::Follow,
            target_creature: Some(target),
            target_point: Some(Point { x: 600.0, y: 846.0 }),
        },
    );
    moving.bond_plans.insert(
        actor,
        BondPlan {
            target,
            final_action: ActionKind::Greet,
            experience: RelationshipExperience::Greeting,
            approaching: true,
        },
    );
    moving.tick(created + Duration::hours(1), 0.05, &desktop);
    // A follower trails the target by a quarter more than shoulder to shoulder.
    assert_eq!(
        moving.action_choices[&actor].target_point,
        Some(Point {
            x: 820.0 - (72.0 * super::super::spacing::FACE_CLEAR_RATIO * 1.25 + BOND_SETTLE),
            y: 846.0
        })
    );

    moving.save.creatures[1].state.action = ActionKind::Sleep;
    moving.tick(created + Duration::hours(1), 0.05, &desktop);
    assert_eq!(moving.save.creatures[0].state.action, ActionKind::Idle);
    assert!(!moving.action_choices.contains_key(&actor));
    assert!(!moving.bond_plans.contains_key(&actor));

    let mut removed = two_creature_world([58; 32], created);
    let actor = removed.save.creatures[0].id;
    let target = removed.save.creatures[1].id;
    removed.save.creatures[0].state.action = ActionKind::Greet;
    removed.action_choices.insert(
        actor,
        ActionChoice {
            action: ActionKind::Greet,
            target_creature: Some(target),
            target_point: Some(removed.save.creatures[1].state.position),
        },
    );
    removed.bond_plans.insert(
        actor,
        BondPlan {
            target,
            final_action: ActionKind::Greet,
            experience: RelationshipExperience::Greeting,
            approaching: false,
        },
    );
    removed.save.creatures.remove(1);
    removed.tick(created + Duration::hours(1), 0.05, &desktop);
    assert_eq!(removed.save.creatures[0].state.action, ActionKind::Idle);
    assert!(!removed.action_choices.contains_key(&actor));
    assert!(!removed.bond_plans.contains_key(&actor));
}

#[test]
fn follow_then_greet_updates_the_pair_only_after_the_sequence_completes() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([59; 32], created);
    let actor = world.save.creatures[0].id;
    let target = world.save.creatures[1].id;
    world.save.creatures[0].state.position = Point { x: 300.0, y: 846.0 };
    world.save.creatures[1].state.position = Point { x: 600.0, y: 846.0 };
    let before = *relationship_between(&world.save.relationships, actor, target).unwrap();
    world.save.creatures[0].state.action = ActionKind::Follow;
    world.save.creatures[0].state.action_elapsed = 1.0;
    world.save.creatures[0].state.action_duration = 1.0;
    world.action_choices.insert(
        actor,
        ActionChoice {
            action: ActionKind::Follow,
            target_creature: Some(target),
            target_point: Some(world.save.creatures[1].state.position),
        },
    );
    world.bond_plans.insert(
        actor,
        BondPlan {
            target,
            final_action: ActionKind::Greet,
            experience: RelationshipExperience::Greeting,
            approaching: true,
        },
    );

    world.tick(created + Duration::hours(1), 0.01, &desktop);
    assert_eq!(world.save.creatures[0].state.action, ActionKind::Greet);
    assert!(!world.bond_plans[&actor].approaching);
    assert_eq!(
        relationship_between(&world.save.relationships, actor, target),
        Some(&before)
    );

    let duration = world.save.creatures[0].state.action_duration;
    world.save.creatures[0].state.action_elapsed = duration;
    world.tick(created + Duration::hours(1), 0.01, &desktop);
    let after = relationship_between(&world.save.relationships, actor, target).unwrap();
    assert_eq!(after.affinity, before.affinity.saturating_add(2));
    assert_eq!(after.familiarity, before.familiarity.saturating_add(1));
    assert!(!world.bond_plans.contains_key(&actor));
}

#[test]
fn a_tossed_preferred_companion_prompts_a_concerned_reaction() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([60; 32], created);
    let actor = world.save.creatures[0].id;
    let target = world.save.creatures[1].id;
    let before = *relationship_between(&world.save.relationships, actor, target).unwrap();
    world.save.creatures[0].state.action = ActionKind::Sleep;
    world.save.creatures[0].state.action_elapsed = 30.0;
    world.save.creatures[0].state.action_duration = 300.0;
    world.save.creatures[1].state.action = ActionKind::Tossed;
    world.save.creatures[1].state.action_duration = 100.0;

    world.tick(created + Duration::hours(1), 0.01, &desktop);
    assert_eq!(
        world.save.creatures[0].state.action,
        ActionKind::ReactToWindow
    );
    assert_eq!(
        world.bond_plans[&actor].experience,
        RelationshipExperience::ConcernedAfterToss
    );
    assert!(world.drain_events().any(|event| matches!(
        event,
        WorldEvent::SleepInterrupted { creature_id, .. } if creature_id == actor
    )));

    world.save.creatures[1].state.action = ActionKind::Landing;
    let duration = world.save.creatures[0].state.action_duration;
    world.save.creatures[0].state.action_elapsed = duration;
    world.tick(created + Duration::hours(1), 0.01, &desktop);
    let after = relationship_between(&world.save.relationships, actor, target).unwrap();
    assert_eq!(after.affinity, before.affinity.saturating_add(2));
    assert_eq!(after.familiarity, before.familiarity.saturating_add(1));
}

#[test]
fn shelter_return_reuses_greeting_actions_and_adds_no_body_clip() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([62; 32], created);
    let now = created + Duration::hours(1);
    world.save.home.active_since_utc = Some(now);
    for creature in &mut world.save.creatures {
        creature.state.action = ActionKind::Homebound;
    }
    world.dismiss_home(now, false);
    for creature in &mut world.save.creatures {
        creature.state.action_elapsed = creature.state.action_duration;
    }

    world.tick(now, 0.01, &desktop);
    assert!(world.bond_plans.values().any(|plan| {
        plan.final_action == ActionKind::Greet
            && plan.experience == RelationshipExperience::HomecomingGreeting
    }));
    assert!(world.save.creatures.iter().any(|creature| {
        matches!(
            creature.state.action,
            ActionKind::Greet | ActionKind::Follow
        )
    }));
    // Coming home costs the atlas nothing: every pose it asks for is one that already has a
    // body clip baked for it.
    assert!(
        world
            .bond_plans
            .values()
            .map(|plan| plan.final_action)
            .chain(world.save.creatures.iter().map(|c| c.state.action))
            .all(|action| ActionKind::BODY_CLIPS.contains(&action))
    );
}

#[test]
fn discovery_watch_steal_and_squabble_schedule_existing_targeted_actions() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let now = created + Duration::hours(1);

    let mut gift = two_creature_world([63; 32], created);
    let gift_actor = gift.save.creatures[0].id;
    let gift_target = gift.save.creatures[1].id;
    let relationship =
        relationship_mut_or_insert(&mut gift.save.relationships, gift_actor, gift_target).unwrap();
    relationship.affinity = 200;
    relationship.familiarity = 200;
    relationship.avoidance = 0;
    gift.discovery_remaining = 0.0;
    gift.save.creatures[0].state.action_elapsed = 1.0;
    gift.save.creatures[0].state.action_duration = 1.0;
    gift.tick(now, 0.01, &desktop);
    let gift_plan = gift.bond_plans[&gift_actor];
    assert_eq!(gift_plan.target, gift_target);
    assert_eq!(gift_plan.final_action, ActionKind::PresentDiscovery);
    assert_eq!(
        gift_plan.experience,
        RelationshipExperience::BroughtDiscovery
    );

    for (seed, target_action, scores, expected, expected_action, attempts) in [
        (
            64,
            ActionKind::ClimbWindow,
            (100, 100, 0, 0),
            RelationshipExperience::WatchedClimb,
            ActionKind::InspectScreen,
            64,
        ),
        (
            65,
            ActionKind::SoloPlay,
            (100, 100, 200, 0),
            RelationshipExperience::StoleToy,
            ActionKind::SocialPlay,
            64,
        ),
        (
            67,
            ActionKind::Idle,
            (20, 100, 100, 120),
            RelationshipExperience::Squabble,
            ActionKind::SocialPlay,
            1_024,
        ),
    ] {
        let mut world = two_creature_world([seed; 32], created);
        let actor = world.save.creatures[0].id;
        let target = world.save.creatures[1].id;
        let relationship =
            relationship_mut_or_insert(&mut world.save.relationships, actor, target).unwrap();
        relationship.affinity = scores.0;
        relationship.familiarity = scores.1;
        relationship.playfulness = scores.2;
        relationship.avoidance = scores.3;
        world.discovery_remaining = f32::MAX;

        let mut scheduled = None;
        for _ in 0..attempts {
            world.action_choices.clear();
            world.bond_plans.clear();
            world.save.creatures[0].state.action = ActionKind::Idle;
            world.save.creatures[0].state.action_elapsed = 1.0;
            world.save.creatures[0].state.action_duration = 1.0;
            world.save.creatures[1].state.action = target_action;
            world.save.creatures[1].state.action_elapsed = 0.0;
            world.save.creatures[1].state.action_duration = 100.0;
            world.tick(now, 0.01, &desktop);
            scheduled = world
                .bond_plans
                .get(&actor)
                .copied()
                .filter(|plan| plan.experience == expected);
            if scheduled.is_some() {
                break;
            }
        }
        let plan = scheduled.expect("seeded scheduler should eventually choose the rare bond");
        assert_eq!(plan.target, target);
        assert_eq!(plan.final_action, expected_action);
        assert_eq!(world.action_choices[&actor].target_creature, Some(target));
    }
}
