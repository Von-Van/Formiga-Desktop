use super::*;

/// Nothing the simulation keeps about the moment may grow without bound. Under a desktop that
/// keeps shoving windows about, every runtime map stays inside the colony it belongs to, and a
/// scene that is cut short leaves no plan, no route, and no reserved landing behind it.
#[test]
fn every_runtime_map_stays_inside_the_colony_and_a_cut_short_scene_leaves_nothing() {
    let created = datetime!(2026-05-01 0:00 UTC);
    let mut desktop = desktop();
    desktop.window_sample = Some(WindowSample {
        monotonic_millis: 0,
        reliable: true,
    });
    for index in 0..4 {
        desktop.windows.push(DesktopWindow {
            key: 950 + index,
            bounds: DesktopRect {
                x: 120.0 + index as f32 * 300.0,
                y: 560.0 + (index % 2) as f32 * 60.0,
                width: 260.0,
                height: 240.0,
            },
            z_order: index as u32,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
    }
    let mut world = two_creature_world([88; 32], created);
    let now = created + Duration::hours(1);
    let colony = world.save.creatures.len();
    let mut peak = (0, 0, 0, 0);
    for step in 1..=4_000 {
        // A restless desktop: something moves every half second, and every so often a scene
        // is cut short under the colony's feet.
        if step % 10 == 0 {
            let index = (step / 10 % 4) as usize;
            desktop.windows[index].bounds.x += if step % 20 == 0 { 24.0 } else { -24.0 };
        }
        if step % 400 == 0 {
            world.save.settings.paused = true;
        }
        if step % 400 == 20 {
            world.save.settings.paused = false;
        }
        desktop.window_sample.as_mut().unwrap().monotonic_millis = step as u64 * 50;
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        let (plans, cooldowns) = world.attention.held();
        peak = (
            peak.0.max(plans),
            peak.1.max(cooldowns),
            peak.2.max(world.window_routes.len()),
            peak.3.max(world.window_journeys.len()),
        );
        for map in [
            plans,
            cooldowns,
            world.window_routes.len(),
            world.window_journeys.len(),
            world.tosses.len(),
        ] {
            assert!(
                map <= colony,
                "a runtime map outgrew the colony at step {step}"
            );
        }
        assert!(
            world.save.companion.journal.len() <= MAX_JOURNAL_ENTRIES,
            "the journal outgrew its cap at step {step}"
        );
    }
    assert!(peak.0 > 0, "the colony did have scenes to clean up after");
    // Cut everything short at once, the way hiding the colony does.
    world.save.settings.visible = false;
    for step in 4_001..=4_040 {
        desktop.window_sample.as_mut().unwrap().monotonic_millis = step as u64 * 50;
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
    }
    assert_eq!(world.attention.held().0, 0, "no plan survives the cut");
    assert!(
        world.window_journeys.is_empty(),
        "and no journey does either"
    );
    assert!(
        world
            .save
            .creatures
            .iter()
            .all(|c| c.state.attention.is_none()),
        "and nobody is left holding a pose"
    );
}

#[test]
fn passive_activity_actions_have_distinct_state_outcomes() {
    let desktop = desktop();
    let creature = World::new([61; 32], datetime!(2026-01-01 0:00 UTC), &desktop)
        .save
        .creatures
        .remove(0);
    let context = BehaviorContext {
        cursor_safe: true,
        ambience: DesktopAmbience::default(),
        nearest_creature_distance: None,
        nearest_creature_position: None,
        nearest_creature_id: None,
        bond: None,
        on_window_ledge: false,
        reachable_window_ledge: false,
        window_changed_nearby: false,
        objects: ObjectUtility::default(),
        hour_utc: 12,
    };

    let mut eating = creature.clone();
    eating.state.action = ActionKind::Eat;
    eating.state.drives.energy = 0.25;
    let energy_before = eating.state.drives.energy;
    update_drives(&mut eating, 1.0);
    execute_action(&mut eating, &desktop, context, 1.0, None, None);
    assert!(eating.state.drives.energy > energy_before);

    let mut drinking = creature.clone();
    drinking.state.action = ActionKind::Drink;
    drinking.state.drives.comfort = 0.2;
    drinking.state.drives.arousal = 0.8;
    execute_action(&mut drinking, &desktop, context, 1.0, None, None);
    assert!(drinking.state.drives.comfort > 0.2);
    assert!(drinking.state.drives.arousal < 0.8);

    let mut sprinting = creature;
    sprinting.state.action = ActionKind::Sprint;
    sprinting.state.facing_right = true;
    let start_x = sprinting.state.position.x;
    let walking_speed = 24.0 + sprinting.personality.activity * 34.0;
    execute_action(&mut sprinting, &desktop, context, 0.5, None, None);
    assert!(sprinting.state.position.x - start_x > walking_speed * 0.5 * 2.0);
}

#[test]
fn stale_monitor_ids_rebind_all_arrived_creatures_instead_of_hiding_them() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([97; 32], created);
    for creature in &mut world.save.creatures {
        creature.state.surface.monitor_id = u64::MAX;
    }
    keep_creatures_in_habitat(
        &mut world.save.creatures,
        &desktop,
        &world.save.settings.habitat,
        &[],
    );
    assert_eq!(world.save.creatures.len(), 2);
    assert!(
        world
            .save
            .creatures
            .iter()
            .all(|creature| creature.state.surface.monitor_id == desktop.monitors[0].id)
    );
}
