use super::*;

#[test]
fn quiet_mode_holds_home_beyond_normal_cycle_and_expires_after_relaunch() {
    let now = datetime!(2026-09-14 12:00 UTC);
    let desktop = desktop();
    let mut world = World::new([7; 32], now, &desktop);
    let settings = world.save.settings.clone();
    world.set_quiet_mode(30, now);
    world.tick(now + Duration::minutes(20), 0.05, &desktop);
    assert!(world.save.home.is_active());
    assert!(!world.try_start_colony_plan(now + Duration::minutes(20), &desktop));
    assert_eq!(world.save.settings, settings);
    let mut restored = World::from_save(world.save);
    restored.tick(now + Duration::minutes(31), 0.05, &desktop);
    assert!(restored.save.companion.quiet_until.is_none());
    assert!(!restored.save.home.is_active());
    assert_eq!(restored.save.settings, settings);
    restored.set_quiet_mode(60, now + Duration::minutes(32));
    restored.set_quiet_mode(0, now + Duration::minutes(33));
    assert!(restored.save.companion.quiet_until.is_none());
    assert_eq!(restored.save.settings, settings);
}

/// The calendar itself is covered where it lives, in the schedule's own tests. This is the
/// part the world owns: applying a routine once, leaving a manual choice alone, and never
/// touching visibility, pause, or the separate quiet expiry. It uses a routine that is in
/// force at every hour, so the result does not depend on the timezone the test runs in.
#[test]
fn an_opt_in_schedule_applies_a_routine_once_and_yields_to_a_manual_choice() {
    let created = datetime!(2026-09-14 0:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([31; 32], created);
    let work = BehaviorPreset {
        habitat: HabitatPolicy::default(),
        window_ledges: false,
        cursor_reactions: false,
        reduce_motion: true,
    };
    let relax = BehaviorPreset {
        habitat: HabitatPolicy::default(),
        window_ledges: true,
        cursor_reactions: true,
        reduce_motion: false,
    };
    world.save.companion.modes = [Some(work.clone()), Some(relax.clone())];
    world.save.companion.schedule = RoutineSchedule {
        enabled: true,
        transitions: vec![ScheduledTransition {
            days: 0b1111111,
            minute: 0,
            preset: 0,
        }],
        ..RoutineSchedule::default()
    };
    world.tick(datetime!(2026-09-16 10:00 UTC), 0.05, &desktop);
    assert!(world.save.settings.reduce_motion);
    assert!(!world.save.settings.window_ledges);
    assert_eq!(world.save.companion.schedule.applied, Some(0));
    // A manual choice holds until the next change comes round.
    relax.apply(&mut world.save.settings);
    world.override_routine();
    world.tick(datetime!(2026-09-16 14:00 UTC), 0.05, &desktop);
    assert!(
        world.save.settings.window_ledges,
        "a manual choice is respected between transitions"
    );
    assert!(world.save.companion.schedule.overridden);
    // The next scheduled change takes the routine back and ends the override.
    world.save.companion.schedule.applied = Some(1);
    world.tick(datetime!(2026-09-16 18:30 UTC), 0.05, &desktop);
    assert_eq!(world.save.companion.schedule.applied, Some(0));
    assert!(!world.save.companion.schedule.overridden);
    assert!(world.save.settings.reduce_motion);
    // Visibility, pause, and the quiet expiry are never a schedule's business.
    world.save.settings.paused = true;
    world.save.settings.visible = false;
    world.save.companion.quiet_until = Some(datetime!(2030-01-01 0:00 UTC));
    world.save.companion.schedule.applied = Some(1);
    world.tick(datetime!(2026-09-17 9:30 UTC), 0.05, &desktop);
    assert_eq!(world.save.companion.schedule.applied, Some(0));
    assert!(world.save.settings.paused && !world.save.settings.visible);
    assert!(world.save.companion.quiet_until.is_some());
    // A routine that was never saved changes nothing, and is not recorded as applied.
    world.save.companion.modes[0] = None;
    world.save.companion.schedule.applied = Some(1);
    relax.apply(&mut world.save.settings);
    world.tick(datetime!(2026-09-17 10:00 UTC), 0.05, &desktop);
    assert!(
        !world.save.settings.reduce_motion,
        "an unsaved routine is not applied"
    );
    assert_eq!(world.save.companion.schedule.applied, Some(1));
    // Neither does one whose habitat no longer leaves anywhere to stand.
    world.save.companion.modes[0] = Some(BehaviorPreset {
        habitat: HabitatPolicy {
            preset: HabitatPreset::Custom,
            zones: Vec::new(),
        },
        ..work.clone()
    });
    world.tick(datetime!(2026-09-17 10:30 UTC), 0.05, &desktop);
    assert!(!world.save.settings.reduce_motion);
    assert_eq!(world.save.companion.schedule.applied, Some(1));
    // Handing the routine back to the schedule takes effect at once.
    world.save.companion.modes[0] = Some(work);
    world.resume_routine(datetime!(2026-09-17 11:00 UTC));
    assert_eq!(world.save.companion.schedule.applied, Some(0));
    assert!(!world.save.companion.schedule.overridden);
    assert!(world.save.settings.reduce_motion);
    // A schedule nobody turned on never touches anything.
    world.save.companion.schedule.enabled = false;
    world.save.companion.schedule.applied = None;
    relax.apply(&mut world.save.settings);
    world.tick(datetime!(2026-09-17 12:00 UTC), 0.05, &desktop);
    assert_eq!(world.save.companion.schedule.applied, None);
    assert!(!world.save.settings.reduce_motion);
}
