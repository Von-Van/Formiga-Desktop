use super::home::settled_colony;
use super::*;
use crate::world::village_life::{self, GardenTask, Plan, VillageActivity, max_indoors};

/// Every beat anybody starts over the next `ticks`, with who, what, and when, in seconds.
fn record_beats(
    world: &mut World,
    now: OffsetDateTime,
    desktop: &DesktopSnapshot,
    ticks: usize,
) -> Vec<(CreatureId, BeatKind, f32)> {
    let mut started = Vec::new();
    let mut last: BTreeMap<CreatureId, (BeatKind, f32)> = BTreeMap::new();
    for tick in 0..ticks {
        world.tick(now, 0.05, desktop);
        world.save.home.active_since_utc = Some(now);
        for creature in &world.save.creatures {
            let Some(beat) = creature.state.beat else {
                last.remove(&creature.id);
                continue;
            };
            let fresh = last
                .insert(creature.id, (beat.kind, beat.elapsed))
                .is_none_or(|(kind, elapsed)| kind != beat.kind || elapsed > beat.elapsed);
            if fresh {
                started.push((creature.id, beat.kind, tick as f32 * 0.05));
            }
        }
    }
    started
}

/// One companion yawns; the nearest friend looks over and a moment later yawns too; now and then
/// it goes on to a third, who holds out first. Never further than that, and never all at once.
#[test]
fn a_yawn_travels_to_a_friend_and_now_and_then_to_a_third_who_holds_out_first() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let (mut caught, mut resisted) = (0, 0);
    for seed in 0..40_u8 {
        let mut world = settled_colony([seed; 32], 4, created, &desktop);
        world.beats.yawn_now();
        let started = record_beats(&mut world, created, &desktop, 200);
        let yawns: Vec<(CreatureId, f32)> = started
            .iter()
            .filter(|(_, kind, _)| *kind == BeatKind::Yawn)
            .map(|(id, _, at)| (*id, *at))
            .collect();
        assert!(!yawns.is_empty(), "seed {seed}: nobody yawned");
        assert!(
            yawns.len() <= 3,
            "seed {seed}: a yawn went round {} times",
            yawns.len()
        );
        let (first, first_at) = yawns[0];
        assert!(first_at < 0.1, "the first yawn starts straight away");
        for (index, (id, at)) in yawns.iter().enumerate().skip(1) {
            assert_ne!(*id, first, "seed {seed}: the first caught its own yawn");
            // Staggered: a look first, and the yawn a moment after it.
            assert!(
                *at >= yawns[index - 1].1 + 1.0,
                "seed {seed}: two yawns on top of one another"
            );
            let looked = started
                .iter()
                .any(|(who, kind, when)| who == id && *kind == BeatKind::Notice && when < at);
            assert!(
                looked,
                "seed {seed}: caught a yawn without looking over first"
            );
        }
        if yawns.len() >= 2 {
            caught += 1;
        }
        if yawns.len() == 3 {
            let (third, third_at) = yawns[2];
            let resist = started
                .iter()
                .find(|(who, kind, _)| *who == third && *kind == BeatKind::ResistYawn)
                .map(|(.., at)| *at)
                .expect("the third holds out before giving in");
            assert!(resist < third_at);
            resisted += 1;
        }
        assert!(
            started.iter().all(|(_, kind, _)| matches!(
                kind,
                BeatKind::Yawn | BeatKind::Notice | BeatKind::ResistYawn
            )),
            "seed {seed}: something other than a yawn went round: {started:?}"
        );
    }
    assert!(caught >= 20, "a yawn is usually caught: {caught} of 40");
    assert!(
        resisted >= 2,
        "a yawn sometimes reaches a third: {resisted} of 40"
    );
    assert!(resisted < caught, "and more often stops at two");
}

/// A companion yawning stands still for it, out on the desktop as much as at home, and picks up
/// whatever it was doing afterwards.
#[test]
fn a_yawn_is_had_standing_still_and_nothing_yawns_in_a_still_or_hidden_colony() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = two_creature_world([3; 32], created);
    let now = created + Duration::hours(1);
    let id = world.save.creatures[0].id;
    world.save.creatures[0].state.beat = Some(Beat::new(BeatKind::Yawn, 1.0));
    let before = world.save.creatures[0].state.clone();
    for _ in 0..10 {
        world.tick(now, 0.05, &desktop);
    }
    let creature = world.save.creatures.iter().find(|c| c.id == id).unwrap();
    assert_eq!(creature.state.position, before.position);
    assert_eq!(creature.state.action, before.action);
    assert_eq!(creature.state.action_elapsed, before.action_elapsed);
    for _ in 0..20 {
        world.tick(now, 0.05, &desktop);
    }
    assert!(
        world.save.creatures.iter().all(|c| c.state.beat.is_none()),
        "a yawn ends by itself"
    );

    for hold in 0..3 {
        let mut world = settled_colony([5; 32], 3, created, &desktop);
        match hold {
            0 => world.save.settings.reduce_motion = true,
            1 => world.save.settings.paused = true,
            _ => world.save.settings.visible = false,
        }
        world.beats.yawn_now();
        let started = record_beats(&mut world, created, &desktop, 100);
        assert!(started.is_empty(), "case {hold}: {started:?}");
        assert_eq!(world.beats.queued(), 0);
    }
}

/// However long the village is left to itself, somebody is always outside to be seen: a lone
/// companion never goes in at all, and a bigger village never has more than two in at once.
/// Whoever is in has its own house's curtain drawn, and nobody else's.
#[test]
fn somebody_is_always_outside_and_the_curtain_drawn_is_the_right_one() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut went_in = 0;
    for members in [1, 2, 3, 4, 6] {
        let mut world = settled_colony([80 + members as u8; 32], members, created, &desktop);
        let mut most_in = 0;
        for _ in 0..30_000 {
            world.tick(created, 0.05, &desktop);
            world.save.home.active_since_utc = Some(created);
            let inside: Vec<&Creature> = world
                .save
                .creatures
                .iter()
                .filter(|creature| creature.state.indoors)
                .collect();
            most_in = most_in.max(inside.len());
            assert!(
                inside.len() <= max_indoors(members),
                "{} of {members} indoors at once",
                inside.len()
            );
            assert!(inside.len() < members, "the village looked empty");
            let mut drawn: Vec<usize> = inside
                .iter()
                .map(|creature| {
                    house_slot_for(
                        creature,
                        &world.save.creatures,
                        &world.save.home.cottage_order,
                    )
                })
                .collect();
            drawn.sort_unstable();
            drawn.dedup();
            let occupancy: Vec<usize> = world
                .house_occupancy()
                .iter()
                .map(|house| house.slot)
                .collect();
            assert_eq!(
                occupancy, drawn,
                "the curtains drawn are the houses somebody is in"
            );
            for house in world.house_occupancy() {
                let napping = inside.iter().any(|creature| {
                    creature.state.action == ActionKind::Sleep
                        && house_slot_for(
                            creature,
                            &world.save.creatures,
                            &world.save.home.cottage_order,
                        ) == house.slot
                });
                assert_eq!(house.napping, napping);
            }
        }
        if members == 1 {
            assert_eq!(most_in, 0, "a lone companion stays in sight");
        }
        if most_in > 0 {
            went_in += 1;
        }
    }
    assert!(
        went_in >= 2,
        "residents use their houses: {went_in} villages went in"
    );
}

/// A village with three patches coming along: flowers just sprouting, herbs grown, and
/// vegetables at their fullest. Flowers are watered and looked at closely, herbs and vegetables
/// picked and eaten, and a vegetable proudly shown to a friend, who looks on pleased.
#[test]
fn gardens_are_tended_for_what_is_growing_in_them() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([91; 32], 4, created, &desktop);
    let home = &mut world.save.home;
    assert!(home.set_garden(GardenKind::Flowers, Some(0.1), created));
    assert!(home.set_garden(GardenKind::Herbs, Some(0.5), created - Duration::hours(10)));
    assert!(home.set_garden(
        GardenKind::Vegetables,
        Some(0.9),
        created - Duration::hours(24)
    ));
    assert_eq!(
        world
            .save
            .home
            .garden(GardenKind::Flowers)
            .unwrap()
            .stage(created),
        GardenStage::Sprout
    );
    assert_eq!(
        world
            .save
            .home
            .garden(GardenKind::Herbs)
            .unwrap()
            .stage(created),
        GardenStage::Grown
    );
    assert_eq!(
        world
            .save
            .home
            .garden(GardenKind::Vegetables)
            .unwrap()
            .stage(created),
        GardenStage::Bounty
    );
    let mut tasks: BTreeSet<(u8, &'static str)> = BTreeSet::new();
    let mut admired = false;
    let mut ate_what_was_picked = false;
    let mut picking: BTreeMap<CreatureId, bool> = BTreeMap::new();
    for _ in 0..80_000 {
        world.tick(created, 0.05, &desktop);
        world.save.home.active_since_utc = Some(created);
        for (id, activity) in &world.village_life {
            let Plan::Garden { patch, task, .. } = activity.plan else {
                continue;
            };
            let name = match task {
                GardenTask::Water => "water",
                GardenTask::Inspect => {
                    assert_eq!(
                        patch,
                        GardenKind::Flowers,
                        "only a sprout is looked at closely"
                    );
                    "inspect"
                }
                GardenTask::Pick => {
                    assert!(patch.edible(), "picked something that is not to eat");
                    "pick"
                }
                GardenTask::Show { friend } => {
                    assert_eq!(patch, GardenKind::Vegetables, "shown off from its fullest");
                    assert_ne!(friend, *id);
                    "show"
                }
            };
            tasks.insert((patch.index(), name));
            let creature = world.save.creatures.iter().find(|c| c.id == *id).unwrap();
            if let Some(beat) = creature.state.beat {
                match beat.kind {
                    BeatKind::Watering => {
                        assert_eq!(beat.held, Some(VillageProp::WateringCan));
                    }
                    BeatKind::Picking => {
                        assert_eq!(beat.held, Some(VillageProp::Produce(patch)));
                        picking.insert(*id, true);
                    }
                    _ => {}
                }
            }
            if creature.state.action == ActionKind::Eat && picking.remove(id).is_some() {
                ate_what_was_picked = true;
            }
        }
        admired |= world.save.creatures.iter().any(|creature| {
            creature
                .state
                .beat
                .is_some_and(|beat| beat.kind == BeatKind::Admiring)
        });
    }
    let names: BTreeSet<&str> = tasks.iter().map(|(_, name)| *name).collect();
    for task in ["water", "inspect", "pick", "show"] {
        assert!(names.contains(task), "nobody came to {task}: {tasks:?}");
    }
    assert!(ate_what_was_picked, "something picked was eaten");
    assert!(admired, "a friend looked on at what was shown");
}

/// A resident climbs up onto its own house in a little hop, sits up on the roof — its feet on
/// the top of the house, however tall that type of house is — and hops back down.
#[test]
fn a_resident_sits_up_on_its_own_roof_and_comes_back_down() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([93; 32], 3, created, &desktop);
    let ground = world.save.creatures[0].state.position.y;
    let scale = f32::from(world.save.settings.display_scale) / desktop.monitors[0].scale_factor;
    let mut sat = BTreeSet::new();
    let mut came_down = 0;
    let mut aloft: BTreeSet<CreatureId> = BTreeSet::new();
    for _ in 0..80_000 {
        world.tick(created, 0.05, &desktop);
        world.save.home.active_since_utc = Some(created);
        let styles = world.save.home.house_style_list(&world.save.creatures);
        for creature in &world.save.creatures {
            let on_roof = world
                .village_life
                .get(&creature.id)
                .and_then(|activity| match activity.plan {
                    Plan::Roof { slot, top, .. } => Some((slot, top)),
                    _ => None,
                });
            if let Some((slot, top)) = on_roof
                && creature
                    .state
                    .beat
                    .is_some_and(|beat| beat.kind == BeatKind::RoofSit)
            {
                let height = house_roof_height(&world.save.home.shelter, styles[slot], slot == 0);
                assert_eq!(creature.state.position, top);
                assert!((top.y - (ground - height * scale)).abs() < 0.01);
                assert_eq!(creature.state.action, ActionKind::Perch);
                assert_eq!(
                    slot,
                    house_slot_for(
                        creature,
                        &world.save.creatures,
                        &world.save.home.cottage_order
                    ),
                    "up on somebody else's roof"
                );
                sat.insert(creature.id);
                aloft.insert(creature.id);
            } else if on_roof.is_none() && aloft.remove(&creature.id) {
                assert_eq!(creature.state.position.y, ground, "left up on the roof");
                came_down += 1;
            }
        }
    }
    assert!(!sat.is_empty(), "nobody ever sat on a roof");
    assert!(came_down > 0, "nobody came down again");
}

/// Stands a plan up for one resident, as if the village had chosen it.
fn start_plan(world: &mut World, creature_id: CreatureId, plan: Plan) {
    world
        .village_life
        .insert(creature_id, VillageActivity::new(plan));
}

/// A leaf drifts down onto a face: a start, a shake to get it off, and it carries on — and the
/// nearest resident looks over at it.
#[test]
fn a_leaf_on_the_face_is_a_start_a_shake_and_carrying_on() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([95; 32], 3, created, &desktop);
    let id = world.save.creatures[1].id;
    start_plan(&mut world, id, Plan::LeafOnFace);
    let mut heights = Vec::new();
    let mut beat_seen = false;
    let mut noticed = false;
    for _ in 0..200 {
        world.tick(created, 0.05, &desktop);
        if let Some((prop, at)) = world.loose_props(&desktop.monitors).first().copied() {
            assert_eq!(prop, VillageProp::Leaf);
            heights.push(at.y);
        }
        let creature = world.save.creatures.iter().find(|c| c.id == id).unwrap();
        beat_seen |= creature
            .state
            .beat
            .is_some_and(|beat| beat.kind == BeatKind::LeafOnFace);
        noticed |= world.save.creatures.iter().any(|other| {
            other.id != id
                && other
                    .state
                    .beat
                    .is_some_and(|beat| beat.kind == BeatKind::Notice && beat.look.is_some())
        });
        if !world.village_life.contains_key(&id) {
            break;
        }
    }
    assert!(beat_seen, "no start and no shake");
    assert!(noticed, "nobody looked over");
    assert!(!world.village_life.contains_key(&id), "never carried on");
    assert!(
        world.loose_props(&desktop.monitors).is_empty(),
        "the leaf was left in the air"
    );
    // Down onto the face, and then off it down to the ground.
    let lowest = heights.iter().copied().fold(f32::MIN, f32::max);
    assert!(
        heights[0] < heights[heights.len() / 3],
        "the leaf came down"
    );
    assert!(lowest <= world.save.creatures[1].state.position.y + 0.01);
}

/// A snack gets away: eaten, dropped with a start, chased to where it stopped rolling, picked up
/// and eaten on with.
#[test]
fn a_dropped_snack_is_chased_picked_up_and_eaten_on_with() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([96; 32], 3, created, &desktop);
    let id = world.save.creatures[0].id;
    let from = world.save.creatures[0].state.position;
    let to = Point {
        x: from.x + 50.0,
        y: from.y,
    };
    start_plan(
        &mut world,
        id,
        Plan::DroppedSnack {
            before: 1.0,
            from,
            to,
            eat: 2.0,
        },
    );
    let mut actions: Vec<ActionKind> = Vec::new();
    let mut beats: Vec<BeatKind> = Vec::new();
    let mut apple: Vec<f32> = Vec::new();
    for _ in 0..600 {
        world.tick(created, 0.05, &desktop);
        let creature = world.save.creatures.iter().find(|c| c.id == id).unwrap();
        if actions.last() != Some(&creature.state.action) {
            actions.push(creature.state.action);
        }
        if let Some(beat) = creature.state.beat
            && beats.last() != Some(&beat.kind)
        {
            beats.push(beat.kind);
        }
        if let Some((prop, at)) = world.loose_props(&desktop.monitors).first().copied() {
            assert_eq!(prop, VillageProp::Apple);
            // Along the ground, with a bounce or two.
            assert!(at.y <= from.y + 0.01 && at.y >= from.y - 5.0);
            apple.push(at.x);
        }
        if !world.village_life.contains_key(&id) {
            break;
        }
    }
    assert!(
        !world.village_life.contains_key(&id),
        "never finished its snack"
    );
    assert_eq!(beats, [BeatKind::DroppedSnack, BeatKind::Retrieve]);
    let eat = actions.iter().position(|a| *a == ActionKind::Eat).unwrap();
    let chase = actions
        .iter()
        .position(|a| *a == ActionKind::Traverse)
        .unwrap();
    let again = actions.iter().rposition(|a| *a == ActionKind::Eat).unwrap();
    assert!(eat < chase && chase < again, "{actions:?}");
    assert!(
        apple.windows(2).all(|pair| pair[1] >= pair[0] - 0.01),
        "rolled one way"
    );
    assert!(
        (apple.last().unwrap() - to.x).abs() < 0.5,
        "stopped where it rolled to"
    );
    let creature = world.save.creatures.iter().find(|c| c.id == id).unwrap();
    assert!((creature.state.position.x - to.x).abs() < 20.0, "chased it");
}

/// Sat down just off the cushion, a start at finding the ground, a shuffle across onto it, and a
/// nap there.
#[test]
fn a_missed_cushion_is_shuffled_onto_and_napped_on() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([97; 32], 3, created, &desktop);
    world.save.home.set_hangout(HangoutKind::Cushion, Some(0.5));
    let cottages = colony_cottage_list(&world.save.creatures);
    let (_, _, cushion) = home_hangout_positions(
        &world.save.home,
        cottages.as_slice(),
        &desktop.monitors,
        &world.save.settings.habitat,
        world.save.settings.display_scale,
    )[0];
    let id = world.save.creatures[0].id;
    let beside = Point {
        x: cushion.x - 14.0,
        y: cushion.y,
    };
    start_plan(
        &mut world,
        id,
        Plan::MissedCushion {
            beside,
            cushion,
            nap: 3.0,
        },
    );
    let mut missed_at = None;
    let mut slept_at = None;
    for _ in 0..2_000 {
        world.tick(created, 0.05, &desktop);
        let creature = world.save.creatures.iter().find(|c| c.id == id).unwrap();
        if creature
            .state
            .beat
            .is_some_and(|beat| beat.kind == BeatKind::MissedCushion)
        {
            missed_at = Some(creature.state.position);
        }
        if creature.state.action == ActionKind::Sleep {
            slept_at = Some(creature.state.position);
        }
        if !world.village_life.contains_key(&id) {
            break;
        }
    }
    assert_eq!(missed_at, Some(beside), "sat down beside the cushion");
    assert_eq!(slept_at, Some(cushion), "and napped on it");
}

/// Every type of house has its own chore, and while it is being done the house is told so.
#[test]
fn each_house_has_its_own_chore_and_answers_to_it() {
    assert_eq!(
        village_life::chore_for(ShelterStyle::Tent),
        BeatKind::AdjustFlap
    );
    assert_eq!(
        village_life::chore_for(ShelterStyle::PillowFort),
        BeatKind::FluffCushion
    );
    assert_eq!(
        village_life::chore_for(ShelterStyle::Mushroom),
        BeatKind::InspectCap
    );
    assert_eq!(
        village_life::chore_for(ShelterStyle::LeafHouse),
        BeatKind::TidyLeaves
    );
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([98; 32], 3, created, &desktop);
    let id = world.save.creatures[2].id;
    let slot = house_slot_for(
        &world.save.creatures[2],
        &world.save.creatures,
        &world.save.home.cottage_order,
    );
    let at = world.save.creatures[2].state.position;
    start_plan(
        &mut world,
        id,
        Plan::Chore {
            slot,
            stand: at,
            face_right: true,
            chore: BeatKind::FluffCushion,
        },
    );
    let mut progress = Vec::new();
    for _ in 0..200 {
        world.tick(created, 0.05, &desktop);
        if let Some(motion) = world.house_motions().first().copied() {
            assert_eq!(motion.slot, slot);
            assert_eq!(motion.chore, BeatKind::FluffCushion);
            progress.push(motion.progress);
        }
        if !world.village_life.contains_key(&id) {
            break;
        }
    }
    assert!(!progress.is_empty(), "the house never heard about it");
    assert!(progress.windows(2).all(|pair| pair[1] >= pair[0]));
    assert!(world.house_motions().is_empty());
}

/// Picking up somebody sitting on its roof takes it off the roof, and it comes down to the ground
/// afterwards; somebody indoors is out of reach until it comes out.
#[test]
fn a_roof_sitter_can_be_picked_up_and_somebody_indoors_cannot() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([99; 32], 3, created, &desktop);
    let ground = world.save.creatures[0].state.position.y;
    let (sitter, inside) = (world.save.creatures[0].id, world.save.creatures[1].id);
    let at = world.save.creatures[0].state.position;
    let top = Point {
        x: at.x,
        y: at.y - 40.0,
    };
    start_plan(
        &mut world,
        sitter,
        Plan::Roof {
            slot: 0,
            beside: at,
            top,
            stay: 30.0,
        },
    );
    let door = world.save.creatures[1].state.position;
    start_plan(
        &mut world,
        inside,
        Plan::Indoors {
            slot: 1,
            door,
            stay: 30.0,
            nap: true,
        },
    );
    for _ in 0..40 {
        world.tick(created, 0.05, &desktop);
    }
    let creature = |world: &World, id| {
        world
            .save
            .creatures
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .clone()
    };
    assert_eq!(creature(&world, sitter).state.position, top);
    assert!(creature(&world, inside).state.indoors);
    assert_eq!(
        world.house_occupancy(),
        [HouseOccupancy {
            slot: 1,
            napping: true
        }]
    );
    assert!(!world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id: inside,
            cursor: door,
        },
        &desktop,
    ));
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id: sitter,
            cursor: top,
        },
        &desktop,
    ));
    assert!(!world.village_life.contains_key(&sitter));
    assert!(world.handle_command(
        WorldCommand::EndInteraction {
            cursor: top,
            velocity: Point::default(),
        },
        &desktop,
    ));
    for _ in 0..400 {
        world.tick(created, 0.05, &desktop);
        world.save.home.active_since_utc = Some(created);
    }
    assert_eq!(
        creature(&world, sitter).state.position.y,
        ground,
        "came down again"
    );
}

/// Something turned up about the village — in the garden, doing a chore, up on the roof — goes in
/// the scrapbook like any other find, and the visit carries on.
#[test]
fn a_find_at_home_goes_in_the_scrapbook() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    let mut world = settled_colony([100; 32], 4, created, &desktop);
    world
        .save
        .home
        .set_garden(GardenKind::Flowers, Some(0.3), created);
    let before = world.save.companion.scrapbook.len();
    let mut presented = 0;
    for _ in 0..80_000 {
        world.tick(created, 0.05, &desktop);
        world.save.home.active_since_utc = Some(created);
        presented += world
            .drain_events()
            .filter(|event| {
                matches!(
                    event,
                    WorldEvent::ActionCompleted {
                        action: ActionKind::PresentDiscovery,
                        ..
                    }
                )
            })
            .count();
    }
    assert!(presented > 0, "nothing turned up about the village");
    assert!(world.save.companion.scrapbook.len() > before);
    assert!(world.save.home.is_active());
}

/// Ending a visit, pausing and hiding each bring everybody outside and down onto the ground, with
/// nothing left in their hands.
#[test]
fn every_way_of_ending_a_visit_brings_everyone_out_and_down() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    for case in 0..3 {
        let mut world = settled_colony([101; 32], 3, created, &desktop);
        let ground = world.save.creatures[0].state.position.y;
        let (sitter, inside) = (world.save.creatures[0].id, world.save.creatures[1].id);
        let at = world.save.creatures[0].state.position;
        start_plan(
            &mut world,
            sitter,
            Plan::Roof {
                slot: 0,
                beside: at,
                top: Point {
                    x: at.x,
                    y: at.y - 40.0,
                },
                stay: 30.0,
            },
        );
        let door = world.save.creatures[1].state.position;
        start_plan(
            &mut world,
            inside,
            Plan::Indoors {
                slot: 1,
                door,
                stay: 30.0,
                nap: false,
            },
        );
        for _ in 0..40 {
            world.tick(created, 0.05, &desktop);
        }
        match case {
            0 => world.dismiss_home(created, false),
            1 => world.save.settings.paused = true,
            _ => world.save.settings.visible = false,
        }
        world.tick(created, 0.05, &desktop);
        assert!(world.village_life.is_empty(), "case {case}");
        assert!(world.house_occupancy().is_empty());
        for creature in &world.save.creatures {
            assert!(!creature.state.indoors, "case {case}");
            assert!(creature.state.beat.is_none(), "case {case}");
            assert_eq!(creature.state.position.y, ground, "case {case}");
        }
    }
}

/// A plan made for the village as it stood is let go if the village changes under it — the
/// houses moved to the other corner, or a resident's cottage moved along the row — and the
/// resident is put back outside, on the ground, rather than going on with a plan for houses that
/// are no longer where it thinks they are.
#[test]
fn a_plan_for_the_village_as_it_was_is_let_go_when_the_village_changes() {
    let created = datetime!(2026-01-01 0:00 UTC);
    let desktop = desktop();
    for change in 0..2 {
        let mut world = settled_colony([102; 32], 4, created, &desktop);
        let ground = world.save.creatures[0].state.position.y;
        let (sitter, inside) = (world.save.creatures[0].id, world.save.creatures[1].id);
        let at = world.save.creatures[0].state.position;
        start_plan(
            &mut world,
            sitter,
            Plan::Roof {
                slot: 0,
                beside: at,
                top: Point {
                    x: at.x,
                    y: at.y - 40.0,
                },
                stay: 30.0,
            },
        );
        let door = world.save.creatures[1].state.position;
        start_plan(
            &mut world,
            inside,
            Plan::Indoors {
                slot: 1,
                door,
                stay: 30.0,
                nap: true,
            },
        );
        for _ in 0..40 {
            world.tick(created, 0.05, &desktop);
            world.save.home.active_since_utc = Some(created);
        }
        assert!(world.village_life.contains_key(&sitter));
        assert!(!world.house_occupancy().is_empty());
        match change {
            0 => world.save.home.corner = HomeCorner::BottomRight,
            _ => {
                let owners = house_owners(&world.save.creatures, &world.save.home.cottage_order);
                let mut order = owners.as_slice()[1..].to_vec();
                order.reverse();
                let creatures = world.save.creatures.clone();
                world.save.home.arrange_cottages(order, &creatures);
            }
        }
        world.tick(created, 0.05, &desktop);
        assert!(world.house_occupancy().is_empty(), "case {change}");
        assert!(!world.village_life.contains_key(&inside), "case {change}");
        for creature in &world.save.creatures {
            assert!(!creature.state.indoors, "case {change}");
        }
        let sitting = world
            .save
            .creatures
            .iter()
            .find(|creature| creature.id == sitter)
            .unwrap();
        if change == 0 {
            // The whole village moved: everybody's plan goes.
            assert!(
                world.village_life.is_empty(),
                "the corner changed under the plans"
            );
            assert_eq!(sitting.state.position.y, ground);
        } else {
            // Only the cottages moved: the colony house, and whoever is on its roof, stay put.
            assert!(world.village_life.contains_key(&sitter));
            assert!(sitting.state.position.y < ground);
        }
    }
}
