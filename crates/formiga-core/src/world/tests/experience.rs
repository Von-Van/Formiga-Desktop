use super::*;

#[test]
fn minute_observations_pause_while_hidden_or_paused() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut active = World::new([83; 32], created, &desktop);
    active.tick(created, OBSERVATION_INTERVAL_SECS, &desktop);
    assert_eq!(
        active.save.creatures[0]
            .memory
            .favorite_display
            .map(|favorite| favorite.confidence),
        Some(1)
    );

    for (visible, paused) in [(false, false), (true, true)] {
        let mut inactive = World::new([84; 32], created, &desktop);
        inactive.save.settings.visible = visible;
        inactive.save.settings.paused = paused;
        inactive.tick(created, OBSERVATION_INTERVAL_SECS, &desktop);
        assert!(inactive.save.creatures[0].memory.favorite_display.is_none());
    }
}

#[test]
fn contrary_experiences_reverse_tendencies_and_badges_persist_until_viewed() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = World::new([85; 32], created, &desktop);
    let creature_id = world.save.creatures[0].id;
    for _ in 0..12 {
        world
            .events
            .push(WorldEvent::CreaturePetted { creature_id });
    }
    world.project_events(created);
    assert_eq!(world.save.creatures[0].tendencies.cursor_trust, 36);
    assert!(world.save.creatures[0].memory.profile_revision > 0);
    assert!(world.save.creatures[0].memory.viewed_profile_revision == 0);
    assert!(world.drain_events().any(|event| matches!(
        event,
        WorldEvent::ProfileChanged {
            new_descriptor: Some(ProfileDescriptor::Trusting),
            show_milestone: true,
            ..
        }
    )));
    for _ in 0..10 {
        world.events.push(WorldEvent::DragEnded {
            creature_id,
            outcome: DragReleaseKind::Tossed {
                velocity: Point::default(),
            },
        });
    }
    world.project_events(created + Duration::hours(1));
    assert!(world.save.creatures[0].tendencies.cursor_trust < 0);
    assert!(world.drain_events().any(|event| matches!(
        event,
        WorldEvent::ProfileChanged {
            new_descriptor: Some(ProfileDescriptor::Wary),
            show_milestone: false,
            ..
        }
    )));

    world.save.creatures[0]
        .memory
        .milestone_cooldown_active_seconds = 12 * 60 * 60 - 60;
    world.events.push(WorldEvent::ObservationElapsed {
        creature_id,
        display: desktop.monitors[0].display_key,
        region: 0,
        on_ledge: false,
        riding_window: false,
        nearby_creature: None,
        active_seconds: 60,
    });
    world.project_events(created + Duration::days(2));
    world.drain_events().for_each(drop);
    for _ in 0..18 {
        world
            .events
            .push(WorldEvent::CreaturePetted { creature_id });
    }
    world.project_events(created + Duration::days(2));
    assert!(world.drain_events().any(|event| matches!(
        event,
        WorldEvent::ProfileChanged {
            new_descriptor: Some(ProfileDescriptor::Social),
            show_milestone: true,
            ..
        }
    )));
    assert!(world.mark_profile_viewed(creature_id));
    assert_eq!(
        world.save.creatures[0].memory.viewed_profile_revision,
        world.save.creatures[0].memory.profile_revision
    );
}
