//! How a companion carries itself between the big moments: feet that agree with where it is
//! going, a rest that holds still and shifts at its own pace, and eyes that blink and look about.
use super::*;
use formiga_core::{DesktopSnapshot, Presence, PresenceMark, World};

/// A companion as the world has it after a tick or two: measured, standing, on the floor.
fn measured(seed: u8) -> Creature {
    let mut creature = World::preview_adult(
        [seed; 32],
        time::OffsetDateTime::UNIX_EPOCH,
        &DesktopSnapshot::default(),
    );
    creature.state.attention = None;
    creature.state.beat = None;
    creature.state.flourish = None;
    creature.state.facing_right = true;
    creature.state.presence = Presence {
        since: 30.0,
        last: Some(PresenceMark {
            x: creature.state.position.x,
            depth: 0.0,
            window: None,
        }),
        ..Presence::default()
    };
    creature
}

/// A walk going nowhere is drawn resting, and a greeting still walking over is drawn walking;
/// a companion the world has never measured is drawn exactly as its action says.
#[test]
fn the_body_drawn_agrees_with_the_feet() {
    let mut creature = measured(41);
    creature.state.action = ActionKind::Follow;
    let body = BodyPresentation::for_creature(&creature);
    assert_eq!(
        body.clip,
        BodyClip::Action(ActionKind::Idle),
        "standing by a friend"
    );
    creature.state.presence.stepping = true;
    assert_eq!(
        BodyPresentation::for_creature(&creature).clip,
        BodyClip::Action(ActionKind::Follow)
    );
    creature.state.action = ActionKind::Greet;
    assert!(
        matches!(
            BodyPresentation::for_creature(&creature).clip,
            BodyClip::Action(ActionKind::Traverse) | BodyClip::Gesture(Gesture::Skip)
        ),
        "walking over to say hello"
    );
    creature.state.presence.stepping = false;
    assert_eq!(
        BodyPresentation::for_creature(&creature).clip,
        BodyClip::Action(ActionKind::Greet),
        "and saying it once there"
    );
    creature.state.action = ActionKind::Traverse;
    creature.state.presence = Presence::default();
    assert_eq!(
        BodyPresentation::for_creature(&creature).clip,
        BodyClip::Action(ActionKind::Traverse),
        "unmeasured, as the action says"
    );
}

/// A walk's frames follow the ground covered, one after another in order, rather than the clock.
#[test]
fn the_steps_follow_the_ground() {
    let mut creature = measured(43);
    creature.state.action = ActionKind::Traverse;
    creature.state.presence.stepping = true;
    let frames: Vec<u8> = (0..12)
        .map(|stride| {
            creature.state.presence.stride = stride as f32 + 0.5;
            // However long it has been walking.
            creature.state.action_elapsed = 3.0;
            BodyPresentation::for_creature(&creature).frame
        })
        .collect();
    for pair in frames.windows(2) {
        assert_eq!(pair[1], (pair[0] + 1) % 6, "{frames:?}");
    }
}

/// The rest a companion holds: a settle as it comes to rest, then stillness on the plain
/// settle broken by beats of the authored loop, never the same loop going round, and a calm
/// companion holding still for longer than a lively one.
#[test]
fn a_rest_holds_still_and_shifts_at_its_own_pace() {
    let creature = measured(47);
    let motion = MotionSignature::for_creature(&creature);
    let frames: Vec<u8> = (0..180)
        .map(|at| motion.rest_frame(at as f32 / 3.0))
        .collect();
    assert_eq!(frames[0], 1, "it comes to rest with a settle");
    assert_eq!(frames[3], 0, "and then holds still");
    for beat in 0..6 {
        assert!(
            frames.contains(&beat),
            "beat {beat} of the rest shows: {frames:?}"
        );
    }
    assert!(
        (0..frames.len() - 6).any(|at| frames[at] != frames[at + 6]),
        "not a six-frame loop"
    );
    let stillness = |motion: MotionSignature| {
        let frames: Vec<u8> = (0..600)
            .map(|at| motion.rest_frame(at as f32 / 3.0))
            .collect();
        frames.iter().filter(|frame| **frame == 0).count()
    };
    let calm = MotionSignature {
        calm: 1.0,
        ..motion
    };
    let lively = MotionSignature {
        calm: 0.0,
        ..motion
    };
    assert!(
        stillness(calm) > stillness(lively) + 60,
        "calm holds still longer"
    );
}

/// When each blink starts and how long its lids stay shut, over `secs` of a creature's clock.
fn blinks(creature: &mut Creature, secs: f32) -> Vec<(f32, f32)> {
    let mut found = Vec::new();
    let mut shut_since = None;
    for step in 0..(secs * 50.0) as u32 {
        let clock = step as f32 / 50.0;
        creature.state.presence.clock = clock;
        let face = CreatureRenderer::resolve_face_state(creature, CursorSnapshot::default(), true);
        match (face.eyelids == EyelidPose::Closed, shut_since) {
            (true, None) => shut_since = Some(clock),
            (false, Some(since)) => {
                found.push((since, clock - since));
                shut_since = None;
            }
            _ => {}
        }
    }
    found
}

/// Blinks come at irregular intervals, now and then two together, and a content companion
/// blinks slowly some of the time.
#[test]
fn blinks_are_irregular_and_sometimes_slow() {
    let mut creature = measured(53);
    creature.state.action = ActionKind::Idle;
    creature.state.drives.comfort = 0.3;
    creature.state.drives.boredom = 0.2;
    creature.state.drives.sleep_pressure = 0.1;
    let quick = blinks(&mut creature, 240.0);
    assert!((30..=100).contains(&quick.len()), "{} blinks", quick.len());
    let gaps: Vec<f32> = quick.windows(2).map(|pair| pair[1].0 - pair[0].0).collect();
    let shortest = gaps.iter().copied().fold(f32::MAX, f32::min);
    let longest = gaps.iter().copied().fold(0.0, f32::max);
    assert!(shortest < 0.6, "now and then two together: {shortest}");
    assert!(longest > 5.0, "now and then a long while: {longest}");
    assert!(
        quick.iter().all(|(_, shut)| *shut < 0.25),
        "neutral blinks are quick"
    );
    creature.state.drives.comfort = 0.9;
    let content = blinks(&mut creature, 240.0);
    assert!(
        content.iter().any(|(_, shut)| *shut > 0.3),
        "a content companion blinks slowly"
    );
}

/// With nothing to look at, a resting companion's eyes look out and now and then about; a
/// walking one looks where it is going; one waiting on a friend watches them; and under reduced
/// motion they look out as they always have.
#[test]
fn eyes_wander_when_nothing_has_them() {
    let gazes = |creature: &mut Creature, colony: &[Creature], reduce_motion: bool| {
        (0..1_200)
            .map(|step| {
                creature.state.presence.clock = step as f32 / 10.0;
                CreatureRenderer::resolve_face_state_among(
                    creature,
                    colony,
                    CursorSnapshot::default(),
                    true,
                    reduce_motion,
                )
                .gaze
            })
            .collect::<Vec<_>>()
    };
    let mut creature = measured(59);
    creature.state.action = ActionKind::Idle;
    let resting = gazes(&mut creature, &[], false);
    let away = resting
        .iter()
        .filter(|gaze| **gaze != GazeDirection::default())
        .count();
    assert!(
        (60..=700).contains(&away),
        "looks about some of the time: {away} of 1200"
    );
    assert!(
        gazes(&mut creature, &[], true)
            .iter()
            .all(|gaze| *gaze == GazeDirection::default()),
        "reduced motion keeps the eyes still"
    );
    creature.state.presence.stepping = true;
    let ahead = gazes(&mut creature, &[], false)
        .iter()
        .filter(|gaze| **gaze == GazeDirection::new(1, 0))
        .count();
    assert!(ahead > 600, "a walker looks where it is going: {ahead}");
    creature.state.presence.stepping = false;
    // Just stopped beside its friend: settling, not washing.
    creature.state.presence.since = 0.5;
    creature.state.action = ActionKind::Follow;
    let mut friend = measured(61);
    friend.id = creature.id + 1;
    friend.state.position.x = creature.state.position.x - 80.0;
    friend.state.surface = creature.state.surface.clone();
    let colony = [creature.clone(), friend];
    let watching = gazes(&mut creature, &colony, false)
        .iter()
        .filter(|gaze| gaze.x == -1)
        .count();
    assert!(
        watching > 900,
        "keeps an eye on the friend it is with: {watching}"
    );
}

/// A rest has room for things a companion does with itself — washing, shaking itself off,
/// sniffing the air — as its temperament leans, and none of them under reduced motion.
#[test]
fn a_rest_has_moments_of_its_own() {
    let motion = MotionSignature::for_creature(&measured(67));
    let own = |motion: MotionSignature, fidgets: bool| {
        let mut seen = std::collections::HashMap::new();
        for step in 0..3_000 {
            if let RestMoment::Own(gesture, _) = motion.rest_moment(step as f32 / 10.0, fidgets) {
                *seen.entry(gesture).or_insert(0) += 1;
            }
        }
        seen
    };
    assert!(
        own(motion, false).is_empty(),
        "nothing of its own when asked not to"
    );
    let seen = own(motion, true);
    let total: i32 = seen.values().sum();
    assert!(
        (150..=1_500).contains(&total),
        "some of a rest is its own, not most: {total} of 3000"
    );
    let washer = MotionSignature {
        calm: 1.0,
        energy: 0.0,
        curiosity: 0.0,
        ..motion
    };
    let shaker = MotionSignature {
        calm: 0.0,
        energy: 1.0,
        curiosity: 0.0,
        ..motion
    };
    let count = |seen: &std::collections::HashMap<Gesture, i32>, gesture| {
        seen.get(&gesture).copied().unwrap_or(0)
    };
    let (washes, shakes) = (own(washer, true), own(shaker, true));
    assert!(count(&washes, Gesture::Groom) > count(&shakes, Gesture::Groom));
    assert!(count(&shakes, Gesture::Shake) > count(&washes, Gesture::Shake));
}

/// Naps are taken different ways — curled, sprawled, loafed — a trusting companion sprawling
/// more than a wary one, and a long nap now and then turning over into another way.
#[test]
fn naps_are_taken_different_ways() {
    let motion = MotionSignature::for_creature(&measured(71));
    let ways = |motion: MotionSignature| {
        let mut seen = std::collections::HashMap::new();
        for nap in 0..400 {
            *seen
                .entry(motion.nap_pose(nap as f32 * 37.0, 1.0, 30.0, true))
                .or_insert(0) += 1;
        }
        seen
    };
    let seen = ways(motion);
    assert_eq!(seen.len(), 3, "curled, sprawled and loafed: {seen:?}");
    let trusting = ways(MotionSignature {
        trusting: 1.0,
        ..motion
    });
    let wary = ways(MotionSignature {
        trusting: 0.0,
        ..motion
    });
    let sprawled = |seen: &std::collections::HashMap<Option<Gesture>, i32>| {
        seen.get(&Some(Gesture::Sprawl)).copied().unwrap_or(0)
    };
    assert!(sprawled(&trusting) > sprawled(&wary) + 40);
    let turned = (0..400)
        .filter(|nap| {
            let began = *nap as f32 * 37.0;
            motion.nap_pose(began, 1.0, 40.0, true) != motion.nap_pose(began, 39.0, 40.0, true)
        })
        .count();
    assert!(
        (40..=250).contains(&turned),
        "{turned} of 400 long naps turn over"
    );
    assert!(
        (0..400).all(|nap| {
            let began = nap as f32 * 37.0;
            motion.nap_pose(began, 1.0, 40.0, false) == motion.nap_pose(began, 39.0, 40.0, false)
        }),
        "never under reduced motion"
    );
}

/// The three ways of sleeping and the three ways of walking are each drawn differently, on every
/// body: a sprawl lies lower and wider than a curl, a loaf sits taller, and a skip bounces up
/// where a trudge hangs low.
#[test]
fn every_body_sleeps_and_walks_in_more_than_one_way() {
    let creature = measured(73);
    let mut bodies = vec![creature.appearance.clone()];
    for plan in formiga_core::BodyPlan::ALL {
        let mut appearance = creature.appearance.clone();
        let mut design = formiga_core::CreatureDesign::modular([29; 32], 0, None);
        design.body = plan;
        appearance.design = Some(design);
        bodies.push(appearance);
    }
    for family in [
        BodyFamily::Blob,
        BodyFamily::Hopper,
        BodyFamily::SoftQuadruped,
    ] {
        bodies.push(genome(family));
    }
    for genome in &bodies {
        let bounds = |clip: BodyClip, frame: u8| {
            CreatureRenderer::render_body_frame(genome, clip, frame, false)
                .canvas
                .alpha_bounds()
                .expect("drawn")
        };
        let curl = bounds(BodyClip::Action(ActionKind::Sleep), 0);
        let sprawl = bounds(BodyClip::Gesture(Gesture::Sprawl), 0);
        let loaf = bounds(BodyClip::Gesture(Gesture::Loaf), 0);
        let width = |(left, _, right, _): (u32, u32, u32, u32)| right - left;
        let height = |(_, top, _, bottom): (u32, u32, u32, u32)| bottom - top;
        assert!(
            width(sprawl) > width(curl) || height(sprawl) < height(curl),
            "{:?}: a sprawl is longer or lower than a curl",
            genome.design.map(|design| design.body)
        );
        assert_ne!(
            (sprawl, loaf, curl.1),
            (curl, curl, loaf.1),
            "three different ways to sleep"
        );
        // A body bounces up off its legs in a skip, and carries itself low in a trudge.
        let highest = |clip: BodyClip| {
            (0..6)
                .map(|frame| bounds(clip, frame).1)
                .min()
                .expect("six frames")
        };
        assert!(
            highest(BodyClip::Gesture(Gesture::Skip)) < highest(BodyClip::Gesture(Gesture::Trudge)),
            "{:?}: a skip carries itself higher than a trudge",
            genome.design.map(|design| design.body)
        );
    }
}

/// Up from a nap, a companion stretches or shakes itself before it rests, and it washes with
/// its eyes shut.
#[test]
fn waking_and_washing_show() {
    let mut creature = measured(79);
    creature.state.action = ActionKind::Idle;
    creature.state.action_elapsed = 0.3;
    creature.state.presence.since = 0.3;
    creature.state.presence.after = Some((ActionKind::Sleep, 0.3));
    let body = BodyPresentation::for_creature(&creature);
    assert!(
        matches!(
            body.clip,
            BodyClip::Gesture(Gesture::Stretch | Gesture::Shake)
        ),
        "{body:?}"
    );
    creature.state.presence.reduced = true;
    assert_eq!(
        BodyPresentation::for_creature(&creature).clip,
        BodyClip::Action(ActionKind::Idle),
        "not under reduced motion"
    );
    creature.state.presence.reduced = false;
    creature.state.presence.after = None;
    let motion = MotionSignature::for_creature(&creature);
    let washing = (0..3_000).map(|step| step as f32 / 10.0).find(|at| {
        matches!(
            motion.rest_moment(*at, true),
            RestMoment::Own(Gesture::Groom, _)
        )
    });
    if let Some(at) = washing {
        creature.state.action_elapsed = at;
        creature.state.presence.since = at;
        let face =
            CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
        assert_eq!(face.eyelids, EyelidPose::Closed, "eyes shut to wash");
    }
}

/// Each chore has its own pose — watering, picking, tidying, holding up what was grown — rather
/// than all reaching, a pet is leaned into rather than waved at, and eating and drinking, and
/// playing alone and with a friend, move differently.
#[test]
fn what_a_companion_is_doing_shows() {
    use formiga_core::{Beat, BeatKind};
    let mut creature = measured(83);
    creature.state.action = ActionKind::Idle;
    let doing = |creature: &mut Creature, kind: BeatKind| {
        creature.state.beat = Some(Beat {
            elapsed: 0.5,
            ..Beat::new(kind, 3.0)
        });
        BodyPresentation::for_creature(creature).clip
    };
    let chores = [
        (BeatKind::Watering, BodyClip::Gesture(Gesture::Pour)),
        (BeatKind::Picking, BodyClip::Gesture(Gesture::Pluck)),
        (BeatKind::Retrieve, BodyClip::Gesture(Gesture::Pluck)),
        (BeatKind::TidyLeaves, BodyClip::Gesture(Gesture::Tend)),
        (BeatKind::AdjustFlap, BodyClip::Gesture(Gesture::Tend)),
        (BeatKind::FluffCushion, BodyClip::Gesture(Gesture::Tend)),
        (
            BeatKind::ShowingOff,
            BodyClip::Action(ActionKind::PresentDiscovery),
        ),
    ];
    for (kind, clip) in chores {
        assert_eq!(doing(&mut creature, kind), clip, "{kind:?}");
    }
    creature.state.beat = None;
    creature.state.action = ActionKind::PetReaction;
    assert_eq!(
        BodyPresentation::for_creature(&creature).clip,
        BodyClip::Gesture(Gesture::Bask)
    );
    let face = CreatureRenderer::resolve_face_state(&creature, CursorSnapshot::default(), false);
    assert_eq!(face.eyelids, EyelidPose::Closed, "eyes shut under the hand");
    let genome = &creature.appearance;
    let differ = |a: ActionKind, b: ActionKind| {
        (0..4).any(|frame| {
            Pose::new(genome, BodyClip::Action(a), frame, false).lean
                != Pose::new(genome, BodyClip::Action(b), frame, false).lean
                || Pose::new(genome, BodyClip::Action(a), frame, false).crouch
                    != Pose::new(genome, BodyClip::Action(b), frame, false).crouch
        })
    };
    assert!(differ(ActionKind::Eat, ActionKind::Drink));
    assert!(differ(ActionKind::SoloPlay, ActionKind::SocialPlay));
    // Drinking tips the head back; eating dips it forward.
    let leans = |action| {
        (0..4)
            .map(|frame| Pose::new(genome, BodyClip::Action(action), frame, false).lean)
            .collect::<Vec<_>>()
    };
    assert!(leans(ActionKind::Drink).iter().any(|lean| *lean < 0));
    assert!(leans(ActionKind::Eat).iter().all(|lean| *lean >= 0));
}

/// A companion a friend is walking over to watches it come.
#[test]
fn a_friend_coming_over_is_watched() {
    let mut creature = measured(89);
    creature.state.action = ActionKind::Idle;
    creature.state.presence.since = 0.5;
    let mut friend = measured(97);
    friend.id = creature.id + 1;
    friend.state.surface = creature.state.surface.clone();
    friend.state.position.x = creature.state.position.x + 150.0;
    friend.state.action = ActionKind::Greet;
    friend.state.presence.stepping = true;
    friend.state.facing_right = false;
    let colony = [creature.clone(), friend];
    let watched = (0..600)
        .filter(|step| {
            creature.state.presence.clock = *step as f32 / 10.0;
            CreatureRenderer::resolve_face_state_among(
                &creature,
                &colony,
                CursorSnapshot::default(),
                true,
                false,
            )
            .gaze
            .x == 1
        })
        .count();
    assert!(watched > 540, "watching its friend come: {watched} of 600");
}
