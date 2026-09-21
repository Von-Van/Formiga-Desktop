use super::*;

/// A full colony on a desktop of window ledges, with every appetite at its ceiling. Nothing
/// here is typical; it is the most reactive colony the generator can be asked for, which is
/// what the cadence claim has to survive.
pub(super) fn eager_colony(seed: [u8; 32]) -> (World, DesktopSnapshot, OffsetDateTime) {
    let created = datetime!(2026-01-01 0:00 UTC);
    let now = created + Duration::days(40);
    let mut desktop = desktop();
    desktop.window_sample = Some(WindowSample {
        monotonic_millis: 0,
        reliable: true,
    });
    for (key, x, y, width, z_order) in [
        (801_u64, 200.0, 600.0, 600.0, 2_u32),
        (802, 870.0, 600.0, 300.0, 1),
        (803, 320.0, 330.0, 420.0, 0),
    ] {
        desktop.windows.push(DesktopWindow {
            key,
            bounds: DesktopRect {
                x,
                y,
                width,
                height: 200.0,
            },
            z_order,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
    }
    let mut world = World::new(seed, created, &desktop);
    world.tick(now, 0.05, &desktop);
    let_colony_wander(&mut world, now);
    world.pending_home_greetings.clear();
    world.save.ritual.next_at_utc = now + Duration::days(30);
    // One art pixel per desktop point, so the gap and spacing numbers below read directly.
    world.save.settings.display_scale = 2;
    // Four, because one tick across forty days stages only the arrivals that tick earns. The
    // colony cap is higher, so the spacing bounds these colonies are held to are bounds for four
    // bodies on one floor rather than for a full village out on the desktop.
    assert_eq!(world.save.creatures.len(), 4);
    for (index, creature) in world.save.creatures.iter_mut().enumerate() {
        creature.state.arrival_delay_secs = 0.0;
        creature.state.drives = Drives::default();
        creature.personality.curiosity = 1.0;
        creature.personality.sociability = 1.0;
        creature.personality.playfulness = 1.0;
        creature.personality.boldness = 0.8;
        creature.personality.window_tolerance = 0.5;
        creature.state.action = ActionKind::Idle;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 100.0;
        if index < 2 {
            creature.state.surface = SurfaceAttachment {
                kind: SurfaceKind::WindowLedge,
                monitor_id: 1,
                window_key: Some(801),
                relative_x: 0.25 + index as f32 * 0.45,
            };
            creature.state.position = Point {
                x: 350.0 + index as f32 * 270.0,
                y: 600.0,
            };
        } else {
            creature.state.surface = SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id: 1,
                window_key: None,
                relative_x: 0.5,
            };
            creature.state.position = Point {
                x: 560.0 + (index - 2) as f32 * 110.0,
                y: 846.0,
            };
        }
    }
    world.tick(now, 0.05, &desktop);
    world.drain_events().for_each(drop);
    (world, desktop, now)
}

/// Whether a creature is inside an attention scene this tick rather than living its own life.
/// The runtime owns watchers that are still waiting for their cue but have no pose yet; the
/// pose covers the leaps whose contact belongs to the journey rather than to the plan.
fn in_an_attention_scene(world: &World, creature: &Creature) -> bool {
    world.attention.owns(creature.id) || creature.state.attention.is_some()
}

/// Nobody is left standing on geometry that is not there. Creatures still arriving, in the
/// air, or in someone's hand are excused; every other one has to be somewhere real.
fn everyone_stands_on_real_geometry(world: &World, desktop: &DesktopSnapshot, note: &str) {
    for creature in &world.save.creatures {
        if creature.state.arrival_delay_secs > 0.0
            || world.window_journeys.contains_key(&creature.id)
            || world.tosses.contains_key(&creature.id)
        {
            continue;
        }
        let monitor = desktop
            .monitors
            .iter()
            .find(|m| m.id == creature.state.surface.monitor_id)
            .unwrap_or_else(|| panic!("{note}: a creature is attached to a missing display"));
        assert!(
            monitor.bounds.contains(creature.state.position),
            "{note}: a creature stands outside every display"
        );
        if let Some(key) = creature.state.surface.window_key {
            assert!(
                desktop
                    .windows
                    .iter()
                    .any(|w| w.key == key && w.visible && !w.minimized),
                "{note}: a creature still claims window {key}"
            );
        }
    }
}

/// R02's cadence claim, measured rather than argued. The colony above is as reactive as one
/// can be, and the desktop below changes far more often than a real desk does. Ordinary life
/// still has to be what most of the colony is doing on most of the ticks; the per-creature
/// and colony cooldowns are what buy that quiet back.
#[test]
fn ordinary_quiet_life_still_fills_most_of_a_restless_desktop() {
    const STEPS: i64 = 2_400;
    let (mut world, mut desktop, now) = eager_colony([57; 32]);
    let mut scene_creature_ticks = 0_usize;
    let mut crowded_ticks = 0_usize;
    let mut longest_scene = 0_u32;
    let mut busiest = 0_u32;
    // Ticks in a scene so far, and the run the creature is in right now.
    let mut running: BTreeMap<CreatureId, (u32, u32)> = BTreeMap::new();
    for step in 1..=STEPS {
        // A window shoves itself across the screen every four seconds, alternating direction
        // so the desktop stays inside the display. No desk is ever this busy.
        if step % 80 == 0 {
            let turn = (step / 80) as usize;
            let shift = if turn.is_multiple_of(2) {
                110.0
            } else {
                -110.0
            };
            desktop.windows[turn % 3].bounds.x += shift;
        }
        desktop.window_sample.as_mut().unwrap().monotonic_millis = (step * 50) as u64;
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        let busy = world
            .save
            .creatures
            .iter()
            .filter(|c| in_an_attention_scene(&world, c))
            .count();
        scene_creature_ticks += busy;
        if busy * 2 > world.save.creatures.len() {
            crowded_ticks += 1;
        }
        for creature in &world.save.creatures {
            let entry = running.entry(creature.id).or_default();
            if in_an_attention_scene(&world, creature) {
                entry.0 += 1;
                entry.1 += 1;
            } else {
                entry.1 = 0;
            }
            longest_scene = longest_scene.max(entry.1);
            busiest = busiest.max(entry.0);
        }
    }
    let creature_ticks = STEPS as usize * world.save.creatures.len();
    let scene_share = scene_creature_ticks as f32 / creature_ticks as f32;
    let crowded_share = crowded_ticks as f32 / STEPS as f32;
    let busiest_share = busiest as f32 / STEPS as f32;
    // Measured on this fixture: sixteen percent of creature time, twenty-two percent for the
    // busiest single creature, more than half the colony caught up in one scene on six
    // percent of ticks, and no unbroken stretch longer than five and a half seconds. The
    // bounds below leave room for the catalogue to grow without letting it take the day.
    assert!(
        scene_share > 0.02,
        "the colony barely reacted at all: {scene_share}"
    );
    assert!(
        scene_share < 0.35,
        "creatures spent {:.0}% of their time in attention scenes",
        scene_share * 100.0
    );
    assert!(
        busiest_share < 0.4,
        "the busiest creature spent {:.0}% of its time in scenes",
        busiest_share * 100.0
    );
    assert!(
        crowded_share < 0.3,
        "most of the colony was caught up in a scene on {:.0}% of ticks",
        crowded_share * 100.0
    );
    assert!(
        longest_scene < 600,
        "one creature stayed inside a scene for {longest_scene} ticks without a break"
    );
}

/// The other half of R02's cadence claim. This gap is never completed — its far side shifts
/// under every attempt — and every scene that does start gathers whoever is nearby. Neither
/// failure nor an audience buys the colony more of the day, and a jump nobody finished is
/// never recorded as a landing.
#[test]
fn neither_repeated_failure_nor_an_audience_lets_attention_take_over_the_day() {
    const STEPS: i64 = 2_400;
    let (mut world, mut desktop, now) = eager_colony([58; 32]);
    // A ledge with a seventy point gap off each end: too wide for a route to cross and just
    // inside what a bold creature will try for itself. The high window off to the right is
    // too narrow to stand on and too far above to jump to, and exists only to keep giving
    // the colony something to look at, so every scene here is an attempt or an audience.
    desktop.windows[2] = DesktopWindow {
        key: 806,
        bounds: DesktopRect {
            x: 1_180.0,
            y: 200.0,
            width: 100.0,
            height: 140.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    };
    desktop.windows.push(DesktopWindow {
        key: 804,
        bounds: DesktopRect {
            x: 20.0,
            y: 600.0,
            width: 110.0,
            height: 200.0,
        },
        z_order: 3,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    for (index, creature) in world.save.creatures.iter_mut().enumerate() {
        creature.state.position = Point {
            x: [770.0, 230.0, 480.0, 570.0][index],
            y: 600.0,
        };
        creature.state.surface = SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: 1,
            window_key: Some(801),
            relative_x: (creature.state.position.x - 200.0) / 600.0,
        };
    }
    let mut attempts = 0_u32;
    let mut disrupted = 0_u32;
    let mut committed: Vec<WindowKey> = Vec::new();
    let mut nudge = 3.0_f32;
    let mut scene_creature_ticks = 0_usize;
    let mut landings = 0_u32;
    for step in 1..=STEPS {
        // Offer the gap as often as the rest of the system allows. Removing the once-a-minute
        // ledge interval leaves the cooldowns and the setback window to do the work alone.
        world.surface_memory.inspect_in = 0.0;
        if step % 80 == 0 {
            let turn = (step / 80) as usize;
            desktop.windows[2].bounds.x += if turn.is_multiple_of(2) {
                110.0
            } else {
                -110.0
            };
        }
        // Whichever ledge somebody has just committed to slides three points, alternating
        // direction so that it stays where it was: enough to lose the destination they were
        // promised, too little and too slow to read as a window anybody moved.
        if !committed.is_empty() {
            for window in &mut desktop.windows {
                if committed.contains(&window.key) {
                    window.bounds.x += nudge;
                    disrupted += 1;
                }
            }
            nudge = -nudge;
        }
        desktop.window_sample.as_mut().unwrap().monotonic_millis = (step * 50) as u64;
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        landings += world
            .drain_events()
            .filter(|e| {
                matches!(
                    e,
                    WorldEvent::ActionCompleted {
                        action: ActionKind::Landing,
                        ..
                    }
                )
            })
            .count() as u32;
        let was_committed = !committed.is_empty();
        committed = world
            .window_journeys
            .values()
            .filter(|j| matches!(j, WindowJourney::Gap(_)))
            .filter_map(|j| j.surface().window_key)
            .collect();
        attempts += u32::from(!committed.is_empty() && !was_committed);
        scene_creature_ticks += world
            .save
            .creatures
            .iter()
            .filter(|c| in_an_attention_scene(&world, c))
            .count();
    }
    let share = scene_creature_ticks as f32 / (STEPS as usize * world.save.creatures.len()) as f32;
    assert!(
        attempts >= 2,
        "the colony only tried the gap {attempts} times"
    );
    assert!(disrupted > 0, "no attempt was ever actually spoiled");
    assert_eq!(landings, 0, "a spoiled jump was recorded as a landing");
    // Measured on this fixture: six percent of creature time across two minutes, below the
    // sixteen percent of a colony whose desktop keeps rewarding it with something new.
    assert!(
        share < 0.3,
        "a colony that never succeeds spent {:.0}% of its time trying",
        share * 100.0
    );
    everyone_stands_on_real_geometry(&world, &desktop, "after a run of failed jumps");
}

/// R02 asks for adults and minis to be compared. In the simulation they are the same kind of
/// participant: the colony that arrives on the calendar is two of each, and a scene draws on
/// whoever is near and interested rather than on who is full size.
#[test]
fn minis_join_a_scene_on_the_same_terms_as_the_adults_they_live_with() {
    let (mut world, mut desktop, now) = eager_colony([59; 32]);
    assert_eq!(
        world
            .save
            .creatures
            .iter()
            .filter(|c| c.role.is_adult())
            .count(),
        2,
        "this colony is meant to be two adults and two minis"
    );
    let minis: Vec<_> = world
        .save
        .creatures
        .iter()
        .filter(|c| !c.role.is_adult())
        .map(|c| c.id)
        .collect();
    assert_eq!(minis.len(), 2);
    let mut joined = 0;
    for step in 1..=40 {
        if step == 1 {
            desktop.windows[0].bounds.x += 160.0;
        }
        desktop.window_sample.as_mut().unwrap().monotonic_millis = (step * 50) as u64;
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        joined += minis.iter().filter(|id| world.attention.owns(**id)).count();
    }
    assert!(
        joined > 0,
        "no mini was ever part of the scene its colony was watching"
    );
}

/// The familiar half of R02's last pair. A companion with no curiosity and no appetite for
/// company still looks up for someone it knows well; the same companion in a colony of
/// strangers keeps to itself.
#[test]
fn a_familiar_bond_recruits_a_companion_that_curiosity_alone_would_leave_out() {
    for familiar in [false, true] {
        let (mut world, mut desktop, now) = eager_colony([60; 32]);
        let actor = world.save.creatures[0].id;
        let watcher = world.save.creatures[2].id;
        for creature in world.save.creatures.iter_mut().skip(2) {
            creature.personality.curiosity = 0.0;
            creature.personality.sociability = 0.0;
        }
        for bond in &mut world.save.relationships {
            bond.affinity = if familiar && (bond.a == watcher || bond.b == watcher) {
                200
            } else {
                0
            };
            bond.avoidance = 0;
        }
        for step in 1..=6 {
            if step == 1 {
                desktop.windows[0].bounds.x += 160.0;
            }
            desktop.window_sample.as_mut().unwrap().monotonic_millis = (step * 50) as u64;
            world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        }
        assert!(
            world.attention.owns(actor),
            "the rider should react either way"
        );
        assert_eq!(
            world.attention.owns(watcher),
            familiar,
            "an uninterested companion looked up without a bond to explain it"
        );
    }
}

/// R03's session disruptions. A workspace switch takes every window away at once, a locked
/// screen makes the scan unreliable, a sleeping machine leaves a hole in the clock, and a
/// desktop past the window cap cannot be read at all. Each one has to end any scene in
/// progress, leave everybody on real geometry, record no success nobody earned, and replay
/// nothing when the desktop comes back.
#[test]
fn a_workspace_switch_a_lock_a_long_sleep_and_a_window_flood_recover_without_replay() {
    for disruption in 0..4 {
        let (mut world, mut desktop, now) = eager_colony([61; 32]);
        // A fourth window, too small to stand on and too far above anything to reach, so
        // that losing the desktop is a wholesale change of the kind a workspace switch is.
        desktop.windows.push(DesktopWindow {
            key: 807,
            bounds: DesktopRect {
                x: 200.0,
                y: 100.0,
                width: 100.0,
                height: 80.0,
            },
            z_order: 4,
            visible: true,
            minimized: false,
            application: None,
            application_name: None,
        });
        // Something worth watching, so there is a scene to interrupt.
        desktop.windows[0].bounds.x += 160.0;
        for step in 1..=6 {
            desktop.window_sample.as_mut().unwrap().monotonic_millis = (step * 50) as u64;
            world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        }
        assert!(
            world
                .save
                .creatures
                .iter()
                .any(|c| in_an_attention_scene(&world, c)),
            "disruption {disruption} had no scene to interrupt"
        );
        world.drain_events().for_each(drop);

        let restored = desktop.clone();
        let mut at = now + Duration::milliseconds(350);
        let mut millis = 400_u64;
        match disruption {
            0 => desktop.windows.clear(),
            1 => desktop.window_sample.as_mut().unwrap().reliable = false,
            2 => {
                // Six hours asleep. The colony is told the shelter has just gone, so the
                // home cycle does not answer the gap before the geometry does.
                at = now + Duration::hours(6);
                millis = 6 * 3_600 * 1_000;
                let_colony_wander(&mut world, at);
            }
            _ => {
                let template = desktop.windows[2].clone();
                for extra in 0..70 {
                    desktop.windows.push(DesktopWindow {
                        key: 900 + extra,
                        bounds: DesktopRect {
                            x: 40.0 + (extra % 10) as f32 * 130.0,
                            y: 60.0 + (extra / 10) as f32 * 24.0,
                            ..template.bounds
                        },
                        z_order: 20 + extra as u32,
                        ..template.clone()
                    });
                }
            }
        }
        let note = format!("disruption {disruption}");
        for step in 0..5 {
            desktop.window_sample.as_mut().unwrap().monotonic_millis = millis + step * 50;
            world.tick(
                at + Duration::milliseconds(step as i64 * 50),
                0.05,
                &desktop,
            );
            everyone_stands_on_real_geometry(&world, &desktop, &note);
        }
        millis += 200;
        let at = at + Duration::milliseconds(200);
        assert!(
            !world
                .save
                .creatures
                .iter()
                .any(|c| in_an_attention_scene(&world, c)),
            "{note}: a scene carried on through it"
        );
        assert!(
            !world.drain_events().any(|e| matches!(
                e,
                WorldEvent::ActionCompleted {
                    action: ActionKind::Landing | ActionKind::ClimbWindow,
                    ..
                }
            )),
            "{note}: an interrupted attempt was recorded as finished"
        );

        // The desktop comes back exactly as it was. That is a fresh baseline, not the scene
        // the colony was in the middle of before the interruption.
        desktop = restored;
        desktop.window_sample.as_mut().unwrap().monotonic_millis = millis + 50;
        world.tick(at + Duration::milliseconds(50), 0.05, &desktop);
        assert!(
            !world
                .save
                .creatures
                .iter()
                .any(|c| in_an_attention_scene(&world, c)),
            "{note}: the interrupted scene was replayed on the first scan back"
        );
        everyone_stands_on_real_geometry(&world, &desktop, &note);
    }
}

/// R03's window disruptions, one pass each. A rider keeps its footing when its window only
/// moves or grows; it gets put down safely when the window is minimised or closed; and while
/// something else is drawn over it, it is out of sight and out of every scene.
#[test]
fn a_moved_resized_minimised_closed_or_covered_window_always_leaves_its_rider_somewhere_safe() {
    for disruption in 0..5 {
        let (mut world, mut desktop, now) = eager_colony([62; 32]);
        let rider = world.save.creatures[0].id;
        match disruption {
            0 => desktop.windows[0].bounds.x += 240.0,
            1 => {
                desktop.windows[0].bounds.width += 260.0;
                desktop.windows[0].bounds.height += 120.0;
            }
            2 => desktop.windows[0].minimized = true,
            3 => {
                desktop.windows.remove(0);
            }
            _ => desktop.windows.push(DesktopWindow {
                key: 804,
                bounds: DesktopRect {
                    x: 240.0,
                    y: 520.0,
                    width: 520.0,
                    height: 200.0,
                },
                z_order: 0,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            }),
        }
        for step in 1..=30 {
            desktop.window_sample.as_mut().unwrap().monotonic_millis = (step * 50) as u64;
            world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
            everyone_stands_on_real_geometry(
                &world,
                &desktop,
                &format!("window disruption {disruption}"),
            );
        }
        let creature = world
            .save
            .creatures
            .iter()
            .find(|c| c.id == rider)
            .expect("the rider is still in the colony");
        match disruption {
            0 | 1 => assert_eq!(
                creature.state.surface.window_key,
                Some(801),
                "a window that only moved or grew still carries its rider"
            ),
            2 | 3 => {
                assert_eq!(
                    creature.state.surface.kind,
                    SurfaceKind::ScreenFloor,
                    "a lost window has to put its rider down"
                );
                assert_eq!(creature.state.surface.window_key, None);
            }
            _ => {
                assert_eq!(creature.state.surface.window_key, Some(801));
                assert!(
                    !world.attention.owns(rider),
                    "a creature nobody can see is not part of a scene"
                );
            }
        }
    }
}

/// R03's coordinate disruptions at the layer where creatures actually stand on things. The
/// second display is to the left of the origin and at a different scale; its ledges have to
/// carry a creature exactly like the ones on the primary display, and losing that display
/// has to put the creature back on geometry that still exists.
#[test]
fn ledges_at_negative_coordinates_and_another_scale_carry_creatures_normally() {
    let (mut world, mut desktop, now) = eager_colony([63; 32]);
    let mut second = desktop.monitors[0].clone();
    second.id = 2;
    second.display_key = DisplayKey([2; 16]);
    second.primary = false;
    second.bounds.x = -1_440.0;
    second.usable_bounds.x = -1_440.0;
    second.scale_factor = 1.0;
    desktop.monitors.push(second);
    desktop.windows.push(DesktopWindow {
        key: 805,
        bounds: DesktopRect {
            x: -1_100.0,
            y: 500.0,
            width: 420.0,
            height: 200.0,
        },
        z_order: 3,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let traveller = world.save.creatures[3].id;
    {
        let creature = creature_mut(&mut world.save.creatures, traveller).unwrap();
        creature.state.surface = SurfaceAttachment {
            kind: SurfaceKind::WindowLedge,
            monitor_id: 2,
            window_key: Some(805),
            relative_x: 0.5,
        };
        creature.state.position = Point {
            x: -890.0,
            y: 500.0,
        };
    }
    for step in 1..=20 {
        desktop.window_sample.as_mut().unwrap().monotonic_millis = (step * 50) as u64;
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
        everyone_stands_on_real_geometry(&world, &desktop, "on a display left of the origin");
    }
    let creature = world
        .save
        .creatures
        .iter()
        .find(|c| c.id == traveller)
        .unwrap();
    assert_eq!(creature.state.surface.monitor_id, 2);
    assert_eq!(creature.state.position.y, 500.0);
    assert!(creature.state.position.x < 0.0);

    // The same ledge moving on the far display carries its rider with it, negative
    // coordinates and a different scale factor notwithstanding.
    desktop.windows.last_mut().unwrap().bounds.x -= 120.0;
    desktop.window_sample.as_mut().unwrap().monotonic_millis = 1_050;
    world.tick(now + Duration::milliseconds(1_050), 0.05, &desktop);
    assert_eq!(
        world
            .save
            .creatures
            .iter()
            .find(|c| c.id == traveller)
            .unwrap()
            .state
            .position
            .x,
        -1_010.0
    );

    // Unplugging that display leaves the creature on the one that is left.
    desktop.monitors.pop();
    desktop.windows.pop();
    desktop.window_sample.as_mut().unwrap().monotonic_millis = 1_100;
    world.tick(now + Duration::milliseconds(1_100), 0.05, &desktop);
    everyone_stands_on_real_geometry(&world, &desktop, "after the far display was unplugged");
    assert_eq!(
        world
            .save
            .creatures
            .iter()
            .find(|c| c.id == traveller)
            .unwrap()
            .state
            .surface
            .monitor_id,
        1
    );
}
