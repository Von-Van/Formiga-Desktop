use super::home::settled_colony;
use super::*;
use crate::world::village_company::{Call, Kind, Memory, Purpose, Reply, reply_of};
use crate::world::village_life::{GardenTask, Plan, VillageActivity};

/// One fiftieth-of-a-second step of a visit home that never runs out.
fn step(world: &mut World, now: OffsetDateTime, desktop: &DesktopSnapshot) {
    world.tick(now, 0.05, desktop);
    world.save.home.active_since_utc = Some(now);
}

/// Nobody starts anything of its own: only what a test stands up happens.
fn hold_turns(world: &mut World) {
    let ids: Vec<CreatureId> = world.save.creatures.iter().map(|c| c.id).collect();
    for id in ids {
        world.home_moment_timers.insert(id, 1.0e6);
    }
}

fn state(world: &World, id: CreatureId) -> CreatureState {
    world
        .save
        .creatures
        .iter()
        .find(|creature| creature.id == id)
        .expect("the colony is whole")
        .state
        .clone()
}

/// Sends `from` over to `on`, as if it had chosen to.
fn call_on(world: &mut World, from: CreatureId, on: CreatureId, purpose: Purpose) {
    let at = state(world, from).position;
    world.village_life.insert(
        from,
        VillageActivity::new(Plan::Call(Call {
            on,
            purpose,
            stay: 10.0,
            to: at,
            to_depth: 0.0,
        })),
    );
}

/// A colony of three home and awake, with everybody as glad of company as `sociability`, and
/// nobody starting anything of its own.
fn village(seed: u8, sociability: f32, desktop: &DesktopSnapshot) -> (World, OffsetDateTime) {
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut world = settled_colony([seed; 32], 3, created, desktop);
    for creature in &mut world.save.creatures {
        creature.personality.sociability = sociability;
        creature.personality.playfulness = 0.9;
        creature.state.drives.sleep_pressure = 0.0;
        creature.state.drives.energy = 0.9;
    }
    hold_turns(&mut world);
    // A few seconds for everybody to be walking the village as it ordinarily does.
    for _ in 0..100 {
        step(&mut world, created, desktop);
        hold_turns(&mut world);
    }
    (world, created)
}

fn frame(world: &World, desktop: &DesktopSnapshot) -> f32 {
    crate::world::spacing::frame_width(
        world.save.settings.display_scale,
        desktop.monitors[0].scale_factor,
    )
}

/// Somebody who goes over to a neighbour looks across first; the neighbour stops and looks
/// back; the two say hello face to face, a creature's width apart, and keep one another
/// company; and then both are let go, with nothing left over.
#[test]
fn a_call_is_looked_back_at_answered_and_kept_company_with() {
    let desktop = desktop();
    let mut welcomes = 0;
    for seed in 0..8_u8 {
        let (mut world, now) = village(seed, 1.0, &desktop);
        let (caller, friend) = (world.save.creatures[0].id, world.save.creatures[2].id);
        call_on(&mut world, caller, friend, Purpose::Hello);
        let frame = frame(&world, &desktop);
        let (mut looked_back, mut greeted, mut together) = (false, [false; 2], 0.0_f32);
        let mut ended_at = None;
        for tick in 0..1_600 {
            step(&mut world, now, &desktop);
            hold_turns(&mut world);
            let (a, b) = (state(&world, caller), state(&world, friend));
            let answering = world
                .village_life
                .get(&friend)
                .is_some_and(|activity| matches!(activity.plan, Plan::Answer(_)));
            if tick == 1 {
                assert!(answering, "seed {seed}: nobody stopped for the caller");
            }
            if answering && b.beat.is_some_and(|beat| beat.kind == BeatKind::Notice) {
                // Stopped where it stood, turned to whoever is coming.
                looked_back = true;
                if (a.position.x - b.position.x).abs() > 1.0 {
                    assert_eq!(b.facing_right, a.position.x > b.position.x, "seed {seed}");
                }
            }
            greeted[0] |= a.action == ActionKind::Greet;
            greeted[1] |= b.action == ActionKind::Greet;
            let met = world
                .village_life
                .get(&caller)
                .is_some_and(VillageActivity::keeps_still);
            if met && answering {
                let apart = (a.position.x - b.position.x).abs();
                assert!(
                    apart >= frame * 0.85 && apart <= frame * 1.3,
                    "seed {seed}: together {apart} apart"
                );
                assert_eq!(a.facing_right, b.position.x > a.position.x);
                together += 0.05;
            }
            if !world.village_life.contains_key(&caller)
                && !world.village_life.contains_key(&friend)
            {
                ended_at = Some(tick);
                break;
            }
        }
        assert!(looked_back, "seed {seed}: the neighbour never looked back");
        assert!(ended_at.is_some(), "seed {seed}: the call never ended");
        assert!(greeted[0], "seed {seed}: the caller never said hello");
        for id in [caller, friend] {
            let after = state(&world, id);
            assert!(after.attention.is_none(), "seed {seed}: left sitting");
            assert!(!after.indoors);
        }
        assert!(
            world.village_memory.met(caller, friend),
            "seed {seed}: the two forgot they had just met"
        );
        if greeted[1] {
            welcomes += 1;
            assert!(together >= 6.0, "seed {seed}: together only {together}s");
        }
    }
    assert!(
        welcomes >= 5,
        "glad of company, and welcomed {welcomes} of 8"
    );
}

/// A game of chase: the caller crouches and runs after its friend, catches it, runs back with
/// the friend after it, and the two end face to face on the ground, in sight, with the game
/// over inside a minute.
#[test]
fn a_game_of_chase_runs_there_and_back_and_lets_both_go() {
    let desktop = desktop();
    let mut games = 0;
    for seed in 0..8_u8 {
        let (mut world, now) = village(seed, 1.0, &desktop);
        let (caller, friend) = (world.save.creatures[0].id, world.save.creatures[1].id);
        call_on(&mut world, caller, friend, Purpose::Play);
        let ground = state(&world, caller).position.y;
        let (mut ran, mut crouched, mut caught, mut laughed) =
            ([false; 2], [false; 2], false, [false; 2]);
        let mut over = false;
        for _ in 0..1_200 {
            step(&mut world, now, &desktop);
            hold_turns(&mut world);
            for (index, id) in [caller, friend].into_iter().enumerate() {
                let seen = state(&world, id);
                ran[index] |= seen.action == ActionKind::Sprint;
                laughed[index] |= seen.action == ActionKind::SocialPlay;
                crouched[index] |= seen.beat.is_some_and(|beat| beat.kind == BeatKind::Pounce);
                caught |=
                    index == 1 && seen.beat.is_some_and(|beat| beat.kind == BeatKind::Startle);
                assert_eq!(
                    seen.position.y, ground,
                    "seed {seed}: a chase left the ground"
                );
            }
            if !world.village_life.contains_key(&caller)
                && !world.village_life.contains_key(&friend)
            {
                over = true;
                break;
            }
        }
        assert!(over, "seed {seed}: the game never ended");
        if !ran[1] {
            // Not in the mood: nobody runs after somebody who said no.
            assert!(!ran[0], "seed {seed}: chased a friend who had said no");
            continue;
        }
        games += 1;
        assert!(
            ran[0] && crouched[0],
            "seed {seed}: the caller never gave chase"
        );
        assert!(caught, "seed {seed}: nobody was caught");
        assert!(crouched[1], "seed {seed}: the friend never took its turn");
        assert!(laughed[0] && laughed[1], "seed {seed}: no laugh at the end");
        // A second or two to stop, and nobody is left standing behind a house.
        for _ in 0..200 {
            step(&mut world, now, &desktop);
            hold_turns(&mut world);
        }
    }
    assert!(
        games >= 5,
        "playful, and only {games} of 8 games were played"
    );
}

/// How somebody takes being called on is its own: the sociable and close welcome it, the sleepy
/// mostly do not and say so with a yawn, the shy say yes from behind their paws, and a grump
/// says no with a huff.
#[test]
fn a_reply_is_the_residents_own() {
    let desktop = desktop();
    let (world, _) = village(3, 0.5, &desktop);
    let mut rng = SeedStream::new([9; 32]).rng("test", 0);
    let mut tally = |change: &dyn Fn(&mut Creature), purpose: Purpose, bond: f32| {
        let mut creature = world.save.creatures[0].clone();
        creature.temperament = Some(crate::Temperament {
            kind: TemperamentKind::Explorer,
            ..creature.temperament()
        });
        change(&mut creature);
        let mut seen = (0, 0, 0, Vec::new());
        for _ in 0..400 {
            match reply_of(&creature, purpose, bond, &mut rng) {
                (Reply::Warm, _) => seen.0 += 1,
                (Reply::Shy, _) => seen.1 += 1,
                (Reply::Decline, sulk) => {
                    seen.2 += 1;
                    if !seen.3.contains(&sulk) {
                        seen.3.push(sulk);
                    }
                }
            }
        }
        seen
    };
    let (warm, shy, _, _) = tally(&|c| c.personality.sociability = 0.9, Purpose::Hello, 0.8);
    assert!(warm >= 360 && shy == 0, "a friend welcomed {warm} of 400");

    let (warm, shy, no, sulks) = tally(
        &|c| {
            c.personality.sociability = 0.9;
            c.state.drives.sleep_pressure = 0.9;
        },
        Purpose::Play,
        0.0,
    );
    assert!(
        no > warm + shy,
        "sleepy, and said no only {no} of 400 times"
    );
    assert_eq!(sulks, [Some(BeatKind::Yawn)], "a sleepy no is a yawn");

    let (warm, shy, _, _) = tally(&|c| c.personality.sociability = 0.1, Purpose::Hello, 0.5);
    assert!(warm == 0 && shy > 200, "shy, and welcomed {warm} openly");

    let (.., no, sulks) = tally(
        &|c| {
            c.temperament = Some(crate::Temperament {
                kind: TemperamentKind::Grump,
                ..c.temperament()
            });
            c.personality.sociability = 0.2;
        },
        Purpose::Hello,
        0.0,
    );
    assert!(no > 100, "a grump said no only {no} of 400 times");
    assert_eq!(sulks, [Some(BeatKind::Huff)], "a grump's no is a huff");
}

/// What a village does of an afternoon, counted from its plans.
#[derive(Default)]
struct Afternoon {
    calls: usize,
    looks_on: usize,
    most_about: usize,
    most_mishaps: usize,
    two_about: usize,
    begun: BTreeMap<CreatureId, usize>,
    longest_plan: f32,
}

/// Lets a village of `members` live `minutes` of an ordinary visit, after `prepare`.
fn afternoon(seed: u8, members: usize, minutes: f32, prepare: impl Fn(&mut Creature)) -> Afternoon {
    let desktop = desktop();
    let created = datetime!(2026-01-01 0:00 UTC);
    let mut world = settled_colony([seed; 32], members, created, &desktop);
    for (index, creature) in world.save.creatures.iter_mut().enumerate() {
        // Settled colonies are one companion several times over: tell them apart.
        creature.behavior_seed[5] = index as u8;
        prepare(creature);
    }
    let mut seen = Afternoon::default();
    let mut had: BTreeMap<CreatureId, (std::mem::Discriminant<Plan>, f32)> = BTreeMap::new();
    for _ in 0..(minutes * 60.0 / 0.05) as usize {
        step(&mut world, created, &desktop);
        let about = world
            .village_life
            .values()
            .filter(|activity| !activity.plan.indoors() && !activity.plan.answering())
            .count();
        seen.most_about = seen.most_about.max(about);
        seen.two_about += usize::from(about >= 2);
        let mishaps = world
            .village_life
            .values()
            .filter(|activity| activity.plan.mishap())
            .count();
        seen.most_mishaps = seen.most_mishaps.max(mishaps);
        // One to a roof, a mini and its big version sharing a house included.
        let mut roofs: Vec<usize> = world
            .village_life
            .values()
            .filter_map(|activity| match activity.plan {
                Plan::Roof { slot, .. } => Some(slot),
                _ => None,
            })
            .collect();
        roofs.sort_unstable();
        assert!(
            roofs.windows(2).all(|pair| pair[0] != pair[1]),
            "two are up on one roof"
        );
        had.retain(|id, _| world.village_life.contains_key(id));
        for (id, activity) in &world.village_life {
            let kind = std::mem::discriminant(&activity.plan);
            match had.get_mut(id) {
                Some((before, held)) if *before == kind => {
                    *held += 0.05;
                    seen.longest_plan = seen.longest_plan.max(*held);
                }
                _ => {
                    had.insert(*id, (kind, 0.0));
                    *seen.begun.entry(*id).or_default() += 1;
                    seen.calls += usize::from(matches!(activity.plan, Plan::Call(_)));
                    seen.looks_on += usize::from(matches!(activity.plan, Plan::LookOn(_)));
                }
            }
        }
        for creature in &world.save.creatures {
            assert!(
                creature.state.position.x.is_finite() && creature.state.depth >= 0.0,
                "somebody is nowhere"
            );
        }
    }
    // However it is left, the village can be closed up: everybody out, down and on its feet.
    world.end_village_life();
    assert!(world.village_life.is_empty());
    for creature in &world.save.creatures {
        assert!(!creature.state.indoors);
    }
    seen
}

/// Several residents are about the village at once — never more than its size allows, and one
/// mishap at a time — everybody gets turns, and no plan goes on for ever. About half of a
/// village of six is indoors at any time, so two are about together less than they would be
/// with everybody out, and the longest plan is a long nap indoors.
#[test]
fn several_residents_are_about_at_once_and_everybody_gets_a_turn() {
    for seed in [21_u8, 22] {
        let seen = afternoon(seed, 6, 20.0, |creature| {
            creature.state.drives.sleep_pressure = 0.2;
        });
        assert!(
            seen.most_about >= 2 && seen.most_about <= 3,
            "seed {seed}: {} about at once",
            seen.most_about
        );
        assert!(
            seen.two_about > 300,
            "seed {seed}: two were about together for only {} ticks",
            seen.two_about
        );
        assert!(seen.most_mishaps <= 1, "seed {seed}: two mishaps at once");
        assert_eq!(
            seen.begun.len(),
            6,
            "seed {seed}: somebody never had a turn"
        );
        assert!(
            seen.begun.values().all(|begun| *begun >= 3),
            "seed {seed}: turns were {:?}",
            seen.begun.values().collect::<Vec<_>>()
        );
        assert!(
            seen.longest_plan < 300.0,
            "seed {seed}: one plan went on for {}s",
            seen.longest_plan
        );
    }
}

/// A village of one has nobody to call on, and goes on with a life of its own all the same.
#[test]
fn a_village_of_one_keeps_itself_busy_and_calls_on_nobody() {
    let seen = afternoon(31, 1, 15.0, |_| {});
    assert_eq!(seen.calls + seen.looks_on, 0);
    assert_eq!(seen.most_about, 1);
    assert!(
        seen.begun.values().sum::<usize>() >= 3,
        "a lone resident did {:?} things in a quarter of an hour",
        seen.begun
    );
}

/// The sociable call on their neighbours far more than those who keep to themselves, and the
/// curious are the ones who stop to look on.
#[test]
fn who_seeks_company_and_who_looks_on_is_down_to_what_they_are_like() {
    let (mut sociable, mut solitary, mut curious, mut incurious) = (0, 0, 0, 0);
    // A full village: with about half of it indoors at any time, three or so are out to call
    // on one another and to be looked on at.
    for seed in [41_u8, 42, 43] {
        sociable += afternoon(seed, 6, 12.0, |c| {
            c.personality.sociability = 0.95;
            c.personality.playfulness = 0.9;
        })
        .calls;
        solitary += afternoon(seed, 6, 12.0, |c| {
            c.personality.sociability = 0.05;
            c.personality.playfulness = 0.05;
            c.personality.activity = 0.3;
        })
        .calls;
        curious += afternoon(seed, 6, 12.0, |c| c.personality.curiosity = 1.0).looks_on;
        incurious += afternoon(seed, 6, 12.0, |c| c.personality.curiosity = 0.0).looks_on;
    }
    assert!(sociable >= 6, "the sociable made only {sociable} calls");
    assert!(
        sociable >= solitary * 2,
        "{sociable} calls from the sociable, {solitary} from the solitary"
    );
    assert!(
        curious > incurious,
        "{curious} looks on from the curious, {incurious} from the incurious"
    );
}

/// Whichever of two keeping company is picked up, the other is let go a moment later; and a
/// colony paused or hidden in the middle of it has everybody back on their own feet.
#[test]
fn company_ends_cleanly_when_one_is_picked_up_or_the_colony_is_stilled() {
    let desktop = desktop();
    for case in 0..4 {
        // A seed whose neighbour welcomes the call.
        let (mut world, now, caller, friend) = (0..12_u8)
            .find_map(|seed| {
                let (mut world, now) = village(seed, 1.0, &desktop);
                let (caller, friend) = (world.save.creatures[0].id, world.save.creatures[2].id);
                call_on(&mut world, caller, friend, Purpose::Hello);
                for _ in 0..1_000 {
                    step(&mut world, now, &desktop);
                    hold_turns(&mut world);
                    let sat = |id| {
                        state(&world, id)
                            .attention
                            .is_some_and(|pose| pose.gesture == Some(Gesture::Sit))
                    };
                    if sat(caller) && sat(friend) {
                        return Some((world, now, caller, friend));
                    }
                }
                None
            })
            .expect("somebody welcomes a call");
        match case {
            0 | 1 => {
                let taken = if case == 0 { caller } else { friend };
                let at = state(&world, taken).position;
                assert!(world.handle_command(
                    WorldCommand::BeginInteraction {
                        creature_id: taken,
                        cursor: at,
                    },
                    &desktop
                ));
                assert!(!world.village_life.contains_key(&taken));
                let left = if case == 0 { friend } else { caller };
                for _ in 0..40 {
                    step(&mut world, now, &desktop);
                    hold_turns(&mut world);
                }
                assert!(
                    !world.village_life.contains_key(&left),
                    "case {case}: left waiting for somebody carried off"
                );
                assert!(state(&world, left).attention.is_none(), "case {case}");
            }
            2 => world.save.settings.paused = true,
            _ => world.save.settings.visible = false,
        }
        if case >= 2 {
            world.tick(now, 0.05, &desktop);
            assert!(world.village_life.is_empty(), "case {case}");
            for id in [caller, friend] {
                assert!(state(&world, id).attention.is_none(), "case {case}");
                assert!(state(&world, id).beat.is_none(), "case {case}");
            }
        }
    }
}

/// What the village remembers: who has lately met, what was done last, a run of one thing
/// leading to another that ends in a rest, and a job to go back to — once.
#[test]
fn the_village_remembers_a_little_and_not_for_long() {
    let mut memory = Memory::default();
    let mut rng = SeedStream::new([4; 32]).rng("test", 0);
    memory.note_met(7, 3, 100.0);
    assert!(memory.met(3, 7) && !memory.met(3, 8));
    memory.age(99.0);
    assert!(memory.met(7, 3));
    memory.age(2.0);
    assert!(!memory.met(7, 3), "a meeting is forgotten in time");

    let nap = Plan::Indoors {
        slot: 0,
        door: Point::default(),
        depth: 0.0,
        stay: 1.0,
        nap: true,
    };
    // One thing leads to another, but never more than a few in a row.
    let mut longest = 0;
    let mut run = 0;
    let mut followed = 0;
    for _ in 0..400 {
        if memory.finished(1, &nap, false, &mut rng) {
            run += 1;
            followed += 1;
            assert_eq!(memory.recent(1).bent(), Some(Kind::Chore));
            assert!(memory.eager(1));
        } else {
            run = 0;
            assert_eq!(memory.recent(1).bent(), None);
        }
        longest = longest.max(run);
        assert_eq!(memory.recent(1).last, Some(Kind::Indoors));
    }
    assert!(
        followed > 100,
        "a nap led on to a tidy only {followed} times"
    );
    assert!(longest <= 2, "{longest} things one straight after another");

    let patch = Plan::Garden {
        patch: GardenKind::Flowers,
        stand: Point::default(),
        face_right: true,
        task: GardenTask::Water,
    };
    memory.interrupted(2, patch);
    assert!(memory.eager(2));
    assert_eq!(memory.resume(2), Some(patch));
    assert_eq!(memory.resume(2), None, "gone back to once");
    memory.interrupted(2, patch);
    memory.age(60.0);
    assert_eq!(memory.resume(2), None, "and not after too long");
    memory.interrupted(2, Plan::LeafOnFace);
    assert_eq!(memory.resume(2), None, "a mishap is not gone back to");
    memory.forget(2);
    memory.clear();
    assert!(!memory.eager(1));
}

/// A stroll does not put a resident's turn off: left to itself for a few minutes, everybody in
/// a small village that is always out strolling has done something.
#[test]
fn a_stroll_does_not_put_a_residents_turn_off() {
    let desktop = desktop();
    let created = datetime!(2026-01-01 0:00 UTC);
    for seed in [51_u8, 52, 53] {
        let mut world = settled_colony([seed; 32], 3, created, &desktop);
        let mut did: BTreeSet<CreatureId> = BTreeSet::new();
        for _ in 0..6_000 {
            step(&mut world, created, &desktop);
            did.extend(world.village_life.keys().copied());
            did.extend(world.home_moments.keys().copied());
        }
        assert_eq!(
            did.len(),
            3,
            "seed {seed}: somebody did nothing in five minutes"
        );
    }
}
