//! `formiga-tools village-watch`: a colony kept at home and watched from outside, the way the
//! person at the desk sees it, with what it did counted up — how much of the time residents
//! stood about, how often they went somewhere, what they did, and how much of it together.
//!
//! Everything is read from what the overlay is given to draw (action, beat, place, depth), never
//! from the village's own bookkeeping, so the same numbers can be compared across any change to
//! how the village decides things.

use crate::fixture_desktop;
use anyhow::{Result, bail};
use formiga_art::{BodyClip, BodyPresentation};
use formiga_core::*;
use std::collections::BTreeMap;
use std::time::Instant;
use time::{Duration, OffsetDateTime};

const DT: f32 = 0.05;

struct Options {
    minutes: f32,
    seeds: u8,
    residents: Vec<usize>,
    furnished: bool,
    /// A friend invited over before the watch begins, staying the whole while.
    visitor: bool,
}

fn options(args: &[String]) -> Result<Options> {
    let mut options = Options {
        minutes: 15.0,
        seeds: 4,
        residents: vec![1, 3, 6],
        furnished: true,
        visitor: false,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .ok_or_else(|| anyhow::anyhow!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--minutes" => options.minutes = value()?.parse()?,
            "--seeds" => options.seeds = value()?.parse()?,
            "--residents" => {
                options.residents = value()?
                    .split(',')
                    .map(str::parse)
                    .collect::<Result<_, _>>()?;
            }
            "--bare" => options.furnished = false,
            "--visitor" => options.visitor = true,
            other => bail!("unknown argument {other}"),
        }
    }
    Ok(options)
}

/// What one resident is seen doing on one tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Seen {
    Indoors,
    /// Standing with nothing going on.
    Standing,
    Walking,
    Beat(BeatKind),
    Doing(ActionKind),
}

impl Seen {
    fn of(creature: &Creature) -> Self {
        let state = &creature.state;
        if state.indoors {
            Self::Indoors
        } else if let Some(beat) = state.beat
            && beat.kind != BeatKind::Carrying
        {
            Self::Beat(beat.kind)
        } else if state.velocity.x.abs() >= 1.0 || state.action == ActionKind::Traverse {
            Self::Walking
        } else if matches!(state.action, ActionKind::Homebound | ActionKind::Idle) {
            Self::Standing
        } else {
            Self::Doing(state.action)
        }
    }

    /// Something a resident does at or about another.
    fn social(self) -> bool {
        matches!(
            self,
            Self::Doing(ActionKind::Greet)
                | Self::Beat(
                    BeatKind::Notice
                        | BeatKind::Admiring
                        | BeatKind::ShowingOff
                        | BeatKind::Peek
                        | BeatKind::Pounce
                        | BeatKind::Startle
                        | BeatKind::Beg
                        | BeatKind::Strut
                )
        )
    }
}

#[derive(Default)]
struct Tally {
    /// Resident-ticks seen, and how many of them were each thing.
    ticks: u64,
    seen: BTreeMap<String, u64>,
    /// Things begun, by name.
    begun: BTreeMap<String, u64>,
    social_begun: u64,
    /// Stops somewhere at least a creature's width from the last stop.
    moves: u64,
    back_ticks: u64,
    /// Ticks, and on how many of them nobody, one, or two or more were up to something.
    frames: u64,
    busy: [u64; 3],
    /// Ticks on which this many residents were out of doors, from none to a full colony.
    out_of_doors: [u64; MAX_COLONY_CREATURES + 1],
    /// A resident begins the very thing it last began.
    repeats: u64,
    longest_stand: f32,
    stands: Vec<f32>,
    /// Ticks two residents stood within a creature's width and a half of one another, out of
    /// doors, at least one of them turned to the other.
    together_ticks: u64,
    /// Ticks a resident's walk was drawn side-on, from behind, and facing the screen, and how
    /// many times a walk changed between them without stopping.
    walked: [u64; 3],
    turns: u64,
    tick_nanos: u128,
    /// Face-cover episodes, the guest counted, that lasted a second or more, and the longest.
    cover_episodes: u64,
    worst_cover: f32,
    /// Seconds a guest spent visiting (after its hello, before its goodbye), standing at the
    /// spot it walked in to, standing beside a resident, and how many residents it went over to.
    guest_visiting: f32,
    guest_at_edge: f32,
    guest_with_residents: f32,
    guest_met: u64,
    guest_visits: u64,
}

fn name(seen: Seen) -> String {
    match seen {
        Seen::Beat(kind) => format!("{kind:?}"),
        Seen::Doing(action) => format!("{action:?}"),
        other => format!("{other:?}"),
    }
}

fn colony(seed: u8, residents: usize, furnished: bool) -> (World, OffsetDateTime) {
    let desktop = fixture_desktop();
    let created = OffsetDateTime::UNIX_EPOCH;
    let mut world = World::new([seed; 32], created, &desktop);
    let now = created + Duration::days(40) + Duration::hours(14);
    let mut next = seed.wrapping_mul(31).wrapping_add(7);
    while world.save.creatures.len() < residents {
        next = next.wrapping_add(13);
        world
            .add_designed_adult([next; 32], None, now, &desktop)
            .expect("the colony has room");
    }
    world.save.arrival_state.arrived = [true; 3];
    for creature in &mut world.save.creatures {
        creature.mini_arrivals.arrived = [true; 2];
        creature.state.arrival_delay_secs = 0.0;
    }
    if furnished {
        world.save.home.gardens = vec![
            GardenPatch {
                kind: GardenKind::Flowers,
                along: 0.2,
                planted_at_utc: Some(now - Duration::days(1)),
            },
            GardenPatch {
                kind: GardenKind::Strawberries,
                along: 0.75,
                planted_at_utc: Some(now - Duration::days(6)),
            },
        ];
        world.save.home.hangouts = vec![
            HangoutSpot {
                kind: HangoutKind::Bench,
                along: 0.45,
            },
            HangoutSpot {
                kind: HangoutKind::Cushion,
                along: 0.9,
            },
        ];
    }
    world.save.home.active_since_utc = Some(now);
    world.save.home.last_disappeared_utc = None;
    (world, now)
}

fn watch(seed: u8, residents: usize, options: &Options, tally: &mut Tally) {
    let desktop = fixture_desktop();
    let (mut world, start) = colony(seed, residents, options.furnished);
    let frame = 32.0 * f32::from(world.save.settings.display_scale) / 2.0;
    let total = (options.minutes * 60.0 / DT) as u64;
    // Two minutes for everybody to walk home before anything is counted.
    let warmup = (120.0 / DT) as u64;
    let mut last: BTreeMap<CreatureId, Seen> = BTreeMap::new();
    let mut last_begun: BTreeMap<CreatureId, Seen> = BTreeMap::new();
    let mut last_stop: BTreeMap<CreatureId, Point> = BTreeMap::new();
    let mut standing: BTreeMap<CreatureId, f32> = BTreeMap::new();
    let mut drawn: BTreeMap<CreatureId, usize> = BTreeMap::new();
    let mut cover: BTreeMap<(CreatureId, CreatureId), f32> = BTreeMap::new();
    if options.visitor {
        let friend = [seed.wrapping_mul(53).wrapping_add(29); 32];
        let shared = SharedCreatureSeed {
            source_colony_seed: friend,
            source_generation: 0,
            design: Some(CreatureDesign::generated(friend, 0, None)),
        };
        world
            .invite_visitor(shared, start, &desktop)
            .expect("nobody is visiting yet");
        tally.guest_visits += 1;
    }
    let clear =
        CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * f32::from(world.save.settings.display_scale)
            / desktop.monitors[0].scale_factor.max(1.0);
    let mut guest_met = 0;
    for tick in 0..warmup + total {
        let now = start + Duration::milliseconds((tick as f32 * DT * 1000.0) as i64);
        world.save.home.active_since_utc = Some(now - Duration::minutes(5));
        world.save.home.last_disappeared_utc = None;
        let began = Instant::now();
        world.tick(now, DT, &desktop);
        let spent = began.elapsed().as_nanos();
        world.drain_events().for_each(drop);
        if tick < warmup {
            continue;
        }
        tally.tick_nanos += spent;
        tally.frames += 1;
        let mut up_to_something = 0;
        let outside = world
            .save
            .creatures
            .iter()
            .filter(|creature| !creature.state.indoors)
            .count();
        tally.out_of_doors[outside.min(MAX_COLONY_CREATURES)] += 1;
        for creature in &world.save.creatures {
            let seen = Seen::of(creature);
            tally.ticks += 1;
            *tally.seen.entry(name(seen)).or_default() += 1;
            if creature.state.depth > 0.5 && !creature.state.indoors {
                tally.back_ticks += 1;
            }
            if !matches!(seen, Seen::Standing | Seen::Indoors) {
                up_to_something += 1;
            }
            // How its walk is drawn, as the overlay asks.
            if creature.state.presence.stepping && !creature.state.indoors {
                let view = match BodyPresentation::for_creature(creature).clip {
                    BodyClip::Gesture(Gesture::Stride | Gesture::Dash) => 0,
                    BodyClip::Gesture(Gesture::Depart) => 1,
                    _ => 2,
                };
                tally.walked[view] += 1;
                if drawn
                    .insert(creature.id, view)
                    .is_some_and(|was| was != view)
                {
                    tally.turns += 1;
                }
            } else {
                drawn.remove(&creature.id);
            }
            let was = last.insert(creature.id, seen);
            if was != Some(seen) {
                if !matches!(seen, Seen::Standing | Seen::Walking | Seen::Indoors) {
                    *tally.begun.entry(name(seen)).or_default() += 1;
                    if seen.social() {
                        tally.social_begun += 1;
                    }
                    // A look across at somebody is not a thing done twice.
                    if seen != Seen::Beat(BeatKind::Notice)
                        && last_begun.insert(creature.id, seen) == Some(seen)
                    {
                        tally.repeats += 1;
                    }
                }
                if was == Some(Seen::Walking) {
                    let here = creature.state.position;
                    let far = last_stop
                        .insert(creature.id, here)
                        .is_none_or(|before| (before.x - here.x).abs() >= frame);
                    if far {
                        tally.moves += 1;
                    }
                }
            }
            if seen == Seen::Standing {
                *standing.entry(creature.id).or_default() += DT;
            } else if let Some(stood) = standing.remove(&creature.id) {
                tally.longest_stand = tally.longest_stand.max(stood);
                tally.stands.push(stood);
            }
        }
        tally.busy[up_to_something.min(2)] += 1;
        let covered = world.covered_faces_with_guest(&desktop);
        let ended: Vec<_> = cover
            .keys()
            .copied()
            .filter(|pair| !covered.contains(pair))
            .collect();
        for pair in ended {
            if cover.remove(&pair).is_some_and(|run| run >= 1.0) {
                tally.cover_episodes += 1;
            }
        }
        for pair in covered {
            let run = cover.entry(pair).or_default();
            *run += DT;
            tally.worst_cover = tally.worst_cover.max(*run);
        }
        if let Some(guest) = world.save.visitors.guest.as_ref()
            && guest.on_stage
            && guest.visit.phase == VisitPhase::Visiting
        {
            let at = &guest.creature.state;
            tally.guest_visiting += DT;
            guest_met = guest_met.max(guest.visit.met.len());
            let settled = at.action != ActionKind::Traverse && at.velocity.x.abs() < 1.0;
            let spot = home_guest_position(
                &world.save.home,
                &colony_cottages(&world.save.creatures),
                &desktop.monitors,
                &world.save.settings.habitat,
                world.save.settings.display_scale,
            )
            .map(|(_, spot)| spot);
            if settled && spot.is_some_and(|spot| (at.position.x - spot.x).abs() < clear) {
                tally.guest_at_edge += DT;
            }
            let beside = world.save.creatures.iter().any(|creature| {
                !creature.state.indoors
                    && (creature.state.position.x - at.position.x).abs() <= frame * 1.5
                    && (creature.state.depth - at.depth).abs() < 3.0
            });
            if settled && beside {
                tally.guest_with_residents += DT;
            }
        }
        let out: Vec<&Creature> = world
            .save
            .creatures
            .iter()
            .filter(|creature| !creature.state.indoors && creature.state.velocity.x.abs() < 1.0)
            .collect();
        for (index, a) in out.iter().enumerate() {
            for b in &out[index + 1..] {
                let apart = b.state.position.x - a.state.position.x;
                let facing = (a.state.facing_right == (apart > 0.0))
                    || (b.state.facing_right == (apart < 0.0));
                if apart.abs() <= frame * 1.5
                    && (a.state.depth - b.state.depth).abs() < 3.0
                    && facing
                {
                    tally.together_ticks += 1;
                }
            }
        }
    }
    tally.guest_met += guest_met as u64;
    for run in cover.into_values() {
        if run >= 1.0 {
            tally.cover_episodes += 1;
        }
    }
    for stood in standing.into_values() {
        tally.longest_stand = tally.longest_stand.max(stood);
        tally.stands.push(stood);
    }
}

pub fn run(args: &[String]) -> Result<()> {
    let options = options(args)?;
    println!(
        "village-watch: {} min x {} seeds, {}",
        options.minutes,
        options.seeds,
        if options.furnished {
            "two gardens, a bench and a cushion"
        } else {
            "bare ground"
        }
    );
    for residents in options.residents.iter().copied() {
        let mut tally = Tally::default();
        for seed in 1..=options.seeds {
            watch(seed, residents, &options, &mut tally);
        }
        let share = |count: u64| 100.0 * count as f64 / tally.ticks.max(1) as f64;
        let resident_minutes = tally.ticks as f64 * f64::from(DT) / 60.0;
        let per_10 = |count: u64| 10.0 * count as f64 / resident_minutes.max(1e-9);
        println!("\n== {residents} resident(s) ==");
        let get = |key: &str| tally.seen.get(key).copied().unwrap_or(0);
        println!(
            "time: standing {:.1}%  walking {:.1}%  indoors {:.1}%  doing something {:.1}%",
            share(get("Standing")),
            share(get("Walking")),
            share(get("Indoors")),
            share(tally.ticks - get("Standing") - get("Walking") - get("Indoors")),
        );
        println!(
            "back from the front {:.1}% of the time; went somewhere {:.1} times per resident per 10 min",
            share(tally.back_ticks),
            per_10(tally.moves)
        );
        let mut stands = tally.stands.clone();
        stands.sort_by(f32::total_cmp);
        let median = stands.get(stands.len() / 2).copied().unwrap_or(0.0);
        println!(
            "standing spells: median {median:.0} s, longest {:.0} s",
            tally.longest_stand
        );
        let begun: u64 = tally.begun.values().sum();
        println!(
            "things begun per resident per 10 min: {:.1} ({} kinds), of them with or about somebody {:.1}; same thing twice running {:.0}%",
            per_10(begun),
            tally.begun.len(),
            per_10(tally.social_begun),
            100.0 * tally.repeats as f64 / begun.max(1) as f64
        );
        println!(
            "pairs standing together, one turned to the other: {:.2} at any moment",
            tally.together_ticks as f64 / tally.frames.max(1) as f64
        );
        let frames = tally.frames.max(1) as f64;
        println!(
            "up to something at once: nobody {:.0}%  one {:.0}%  two or more {:.0}%",
            100.0 * tally.busy[0] as f64 / frames,
            100.0 * tally.busy[1] as f64 / frames,
            100.0 * tally.busy[2] as f64 / frames
        );
        println!(
            "out of doors at once: {}",
            (0..=residents.min(MAX_COLONY_CREATURES))
                .map(|count| format!(
                    "{count}: {:.0}%",
                    100.0 * tally.out_of_doors[count] as f64 / frames
                ))
                .collect::<Vec<_>>()
                .join("  ")
        );
        let walked: u64 = tally.walked.iter().sum();
        let of_walking = |count: u64| 100.0 * count as f64 / walked.max(1) as f64;
        println!(
            "walks drawn: side-on {:.0}%  from behind {:.0}%  facing the screen {:.0}%; a walk turns {:.1} times a minute of walking",
            of_walking(tally.walked[0]),
            of_walking(tally.walked[1]),
            of_walking(tally.walked[2]),
            tally.turns as f64 / (walked as f64 * f64::from(DT) / 60.0).max(1e-9)
        );
        let mut kinds: Vec<(&String, &u64)> = tally.begun.iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(a.1));
        println!(
            "begun: {}",
            kinds
                .iter()
                .map(|(kind, count)| format!("{kind} {count}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!(
            "face-cover episodes of a second or more: {:.1} per hour; longest {:.1} s",
            tally.cover_episodes as f64 / (frames * f64::from(DT) / 3600.0).max(1e-9),
            tally.worst_cover
        );
        if tally.guest_visits > 0 {
            let visiting = f64::from(tally.guest_visiting).max(1e-9);
            println!(
                "guest: visiting {:.0} s a watch; at the spot it walked in to {:.0}%, beside a resident {:.0}%; went over to {:.1} residents a visit",
                visiting / tally.guest_visits as f64,
                100.0 * f64::from(tally.guest_at_edge) / visiting,
                100.0 * f64::from(tally.guest_with_residents) / visiting,
                tally.guest_met as f64 / tally.guest_visits as f64
            );
        }
        println!(
            "tick: {:.1} us (debug builds are several times slower than release)",
            tally.tick_nanos as f64 / frames / 1000.0
        );
    }
    Ok(())
}
