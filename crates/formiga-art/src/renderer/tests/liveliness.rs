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
    assert_eq!(
        BodyPresentation::for_creature(&creature).clip,
        BodyClip::Action(ActionKind::Traverse),
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
