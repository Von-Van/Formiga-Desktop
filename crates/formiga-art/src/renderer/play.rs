//! How a companion looks at play (see [`formiga_core::play`]): the pose each move strikes, the
//! face it wears, and where the thing it is playing with is — tossed up out of the paws, bounced
//! off the ground, balanced on the head, swung, cast, held up on a string or set down to spin.
//!
//! Where a thing goes is worked out from where the frame holds things, moved by the move itself,
//! so a ball can go higher than the frame and a paper plane glide away across the desktop.

use super::*;
use formiga_core::ColonyObjectKind;
use formiga_core::play::{Play, PlayMove, Plaything as Thing};
use std::f32::consts::PI;

/// The game a creature is playing, if it is playing and not doing something bigger.
pub(super) fn play_shown(creature: &Creature) -> Option<&Play> {
    let state = &creature.state;
    (matches!(state.action, ActionKind::SoloPlay | ActionKind::Landing)
        && state.attention.is_none()
        && state.beat.is_none())
    .then_some(state.play.as_ref())
    .flatten()
}

/// The pose a move strikes. The ride down under an umbrella keeps the landing's own clip.
pub fn play_pose(play: &Play) -> Option<MealPose> {
    let (step, into, progress) = play.now();
    let gesture = |gesture: Gesture| Some(MealPose::Playing(BodyClip::Gesture(gesture), into));
    match step {
        PlayMove::PickUp | PlayMove::PutDown => gesture(Gesture::Pluck),
        PlayMove::KeepUp => Some(MealPose::Playing(
            BodyClip::Action(ActionKind::SoloPlay),
            into,
        )),
        PlayMove::TossHigh | PlayMove::Fish => gesture(Gesture::Watch),
        PlayMove::Bounce | PlayMove::Spin | PlayMove::Sail => gesture(Gesture::Crouch),
        PlayMove::Balance => gesture(Gesture::Balance),
        PlayMove::Swish | PlayMove::Fly => gesture(Gesture::Reach),
        PlayMove::Throw if progress < 0.3 => gesture(Gesture::Reach),
        PlayMove::Throw | PlayMove::Landed => gesture(Gesture::Watch),
        PlayMove::Cane => gesture(Gesture::Strut),
        PlayMove::Catch => gesture(Gesture::Cheer),
        PlayMove::Conduct | PlayMove::Twirl | PlayMove::Dance => gesture(Gesture::Bop),
        PlayMove::Boat => gesture(Gesture::Sit),
        PlayMove::Music | PlayMove::Stack => gesture(Gesture::Tend),
        PlayMove::Hug => gesture(Gesture::Worry),
        PlayMove::Topple => gesture(Gesture::Gasp),
        PlayMove::Ride => None,
    }
}

/// Whether a game is drawn calm: with motion reduced, the thing is held still in the paws
/// whatever the move, rather than tossed, bounced, swung or flown. The ride down under an umbrella
/// is the exception, being held still overhead already.
pub(super) fn calm(creature: &Creature, play: &Play) -> bool {
    creature.state.presence.reduced && play.now().0 != PlayMove::Ride
}

/// The face each move wears.
pub fn play_face(play: &Play) -> ExpressionKind {
    let (step, _, progress) = play.now();
    match step {
        PlayMove::PickUp | PlayMove::Spin | PlayMove::Sail => ExpressionKind::Curious,
        PlayMove::Fish | PlayMove::Stack => ExpressionKind::Focused,
        PlayMove::Balance => ExpressionKind::Determined,
        PlayMove::Topple if progress < 0.4 => ExpressionKind::Startled,
        PlayMove::Hug => ExpressionKind::Affectionate,
        PlayMove::Cane => ExpressionKind::Smug,
        PlayMove::PutDown | PlayMove::Landed => ExpressionKind::Content,
        _ => ExpressionKind::Joy,
    }
}

/// Where the eyes go: up after something thrown high or flown, down at something set going on
/// the ground, after a plane as it goes.
pub(super) fn play_gaze(creature: &Creature, play: &Play) -> Option<GazeDirection> {
    if calm(creature, play) {
        return None;
    }
    let forward = if creature.state.facing_right { 1 } else { -1 };
    let (step, _, progress) = play.now();
    match step {
        PlayMove::TossHigh | PlayMove::Balance | PlayMove::Ride => Some(GazeDirection::new(0, -1)),
        PlayMove::Fly => Some(GazeDirection::new(forward, -1)),
        PlayMove::Throw if progress >= 0.3 => Some(GazeDirection::new(forward, 0)),
        PlayMove::Landed => Some(GazeDirection::new(forward, 1)),
        PlayMove::Spin | PlayMove::Stack | PlayMove::Sail | PlayMove::Fish => {
            Some(GazeDirection::new(forward, 1))
        }
        _ => None,
    }
}

/// What the game is played with just now, as it is drawn: the thing itself, or for a stick, a
/// rod or a tower, the picture the move calls for.
pub fn play_held(play: &Play, own_colourway: u8) -> HeldThing {
    let (step, into, _) = play.now();
    let spin = ((play.elapsed * 8.0) as u32 % 4) as u8;
    let sticklike = play.shape == formiga_core::play::PlayShape::Stick;
    let found = matches!(play.thing, Thing::Trinket(_));
    match step {
        PlayMove::Fish => {
            return HeldThing::Play(PlayPicture::Rod((into / 0.7).fract() > 0.75));
        }
        PlayMove::Catch => return HeldThing::Play(PlayPicture::Fish),
        PlayMove::Stack => return HeldThing::Play(PlayPicture::Tower(play.tower())),
        PlayMove::Topple => return HeldThing::Play(PlayPicture::Toppled),
        PlayMove::Swish if sticklike && !found => {
            let angle = [0, 1, 2, 1][(into * 6.0) as usize % 4];
            return HeldThing::Play(PlayPicture::Stick(angle));
        }
        PlayMove::Conduct if sticklike && !found => {
            return HeldThing::Play(PlayPicture::Stick((into * 3.0) as u8 % 2));
        }
        PlayMove::Cane if !found => return HeldThing::Play(PlayPicture::Stick(3)),
        _ => {}
    }
    match play.thing {
        Thing::Toy(kind) => HeldThing::Toy {
            kind,
            colourway: own_colourway,
            spin,
        },
        Thing::Trinket(variant) => HeldThing::Trinket(variant),
        Thing::Belonging(kind) => match kind {
            ColonyObjectKind::YarnBall => HeldThing::Play(PlayPicture::YarnBall),
            ColonyObjectKind::Pebble => HeldThing::Play(PlayPicture::Pebble),
            ColonyObjectKind::Umbrella => HeldThing::Play(match step {
                PlayMove::Twirl | PlayMove::Ride => PlayPicture::UmbrellaOpen,
                PlayMove::Boat => PlayPicture::UmbrellaUpturned,
                _ => PlayPicture::UmbrellaClosed,
            }),
            ColonyObjectKind::Kite => HeldThing::Play(PlayPicture::Kite),
            ColonyObjectKind::Paper => HeldThing::Play(PlayPicture::Plane),
            ColonyObjectKind::Drum => HeldThing::Play(PlayPicture::Drum),
            ColonyObjectKind::MusicBox => HeldThing::Play(PlayPicture::MusicBox),
            ColonyObjectKind::Pillow | ColonyObjectKind::Blanket => {
                HeldThing::Play(PlayPicture::Pillow)
            }
            ColonyObjectKind::SpinningTop => HeldThing::Toy {
                kind: 1,
                colourway: 0,
                spin,
            },
            _ => HeldThing::Toy {
                kind: 0,
                colourway: 0,
                spin,
            },
        },
    }
}

/// What a game is played with, held still: the thing itself, not turning, and an umbrella or a
/// stick folded or upright rather than in whatever picture the move calls for.
fn calm_held(play: &Play, own_colourway: u8) -> HeldThing {
    let mut still = *play;
    // The first move of every game picks the thing up, and pictures it as itself.
    still.elapsed = 0.0;
    match play_held(&still, own_colourway) {
        HeldThing::Toy {
            kind, colourway, ..
        } => HeldThing::Toy {
            kind,
            colourway,
            spin: 0,
        },
        held => held,
    }
}

/// How long one toss up and back down takes, about: a move tosses as many whole ones as fit it.
const TOSS_SECS: f32 = 1.2;
/// How long one bounce off the ground and back up to the paw takes, about.
const BOUNCE_SECS: f32 = 0.8;
/// How far a thrown thing goes before it comes down, in frame pixels, and how far below where it
/// was held it comes to rest: on the ground, out ahead.
const THROWN_TO: (i32, i32) = (52, 10);

/// How far through the current arc of a move made of whole arcs, from 0 (in the paw) to 1 (back
/// in it): as many arcs of about `secs` each as fit the move, so its last one ends in the paw
/// however long or short the move came out.
fn arc(play: &Play, secs: f32) -> f32 {
    let (_, _, progress) = play.now();
    let arcs = (play.move_length() / secs).round().max(1.0);
    if progress >= 1.0 {
        return 0.0;
    }
    (progress * arcs).fract()
}

/// How far from where the frame holds things the thing is in this move, in frame pixels with the
/// frame facing right: up is negative.
pub fn play_offset(play: &Play) -> (i32, i32) {
    let (step, into, progress) = play.now();
    let wave = |rate: f32| (into * rate).sin();
    let round = |v: f32| v.round() as i32;
    match step {
        PlayMove::PickUp => (2, round(3.0 - 3.0 * progress)),
        PlayMove::PutDown => (3, round(4.0 * progress)),
        PlayMove::KeepUp => (0, 0),
        PlayMove::TossHigh => (2, -round((PI * arc(play, TOSS_SECS)).sin() * 18.0)),
        PlayMove::Bounce => {
            // Down to the ground and back up to the paw, starting and ending where it is held.
            let height = (PI * arc(play, BOUNCE_SECS)).sin();
            (2 + round(2.0 * height), round(8.0 * height))
        }
        PlayMove::Balance => (round(wave(6.0)) - 2, -17),
        PlayMove::Swish => (5, -3),
        PlayMove::Cane => (5, 6),
        PlayMove::Fish => (7, -1),
        PlayMove::Catch => (3, -8 - round(wave(9.0).abs() * 2.0)),
        PlayMove::Conduct => (3, -6),
        PlayMove::Twirl => (-1, -25),
        // Held right up overhead, arms up, on the way down.
        PlayMove::Ride => (-3, -31),
        // Under the body it is sat in.
        PlayMove::Boat => (-7, 8),
        PlayMove::Fly => (11 + round(wave(2.0) * 2.0), -20 + round(wave(3.0))),
        PlayMove::Throw if progress < 0.3 => (3, -4),
        PlayMove::Throw => {
            // Out of the paw, up a little and away, gliding down to the ground where it lands.
            let gone = (progress - 0.3) / 0.7;
            (
                3 + round(gone * (THROWN_TO.0 - 3) as f32),
                -4 + round(gone * (THROWN_TO.1 + 4) as f32) - round((gone * PI).sin() * 8.0),
            )
        }
        PlayMove::Landed => THROWN_TO,
        PlayMove::Spin => (9, 10),
        PlayMove::Music => (4, 3),
        PlayMove::Dance => (2, -2 + round(wave(8.0))),
        PlayMove::Hug => (1, 1),
        PlayMove::Stack => (9, 7),
        PlayMove::Topple => (9, 8),
        PlayMove::Sail => (9, 9 + round(wave(3.0))),
    }
}

/// The game's thing, where this frame and this move put it.
pub(super) fn play_placement(
    creature: &Creature,
    play: &Play,
    clip: BodyClip,
    baked: HeldPlacement,
) -> HeldPlacement {
    // With motion reduced the thing itself stays still in the paws, the hold the clip bakes,
    // and whatever the move would have done with it is left out.
    if calm(creature, play) {
        return baked.holding(calm_held(play, toy_colourway(&creature.appearance)));
    }
    let thing = play_held(play, toy_colourway(&creature.appearance));
    // Keeping it up off the paw follows the paw itself.
    if clip == BodyClip::Action(ActionKind::SoloPlay) {
        return baked.holding(thing);
    }
    let offset = baked.thing.offset();
    let (dx, dy) = play_offset(play);
    HeldPlacement::centred(
        thing,
        baked.at.x + offset.x + dx,
        baked.at.y + offset.y + dy,
    )
}

/// The string a flown kite is held by, from the paw to the kite, as its two ends in frame pixels
/// with the frame facing right: the paw's end first. Drawn as one light line behind the paw and
/// the kite, so the pose reads as flying it rather than watching something loose overhead.
pub fn held_tether(
    creature: &Creature,
    clip: BodyClip,
    baked: Option<HeldPlacement>,
) -> Option<(PixelPoint, PixelPoint)> {
    let baked = baked?;
    let play = play_shown(creature)?;
    // With motion reduced the kite is held still in the paws, and needs no string.
    if calm(creature, play)
        || play.now().0 != PlayMove::Fly
        || play_held(play, 0) != HeldThing::Play(PlayPicture::Kite)
        || clip == BodyClip::Action(ActionKind::SoloPlay)
    {
        return None;
    }
    let paw = baked.thing.offset();
    let kite = play_placement(creature, play, clip, baked);
    Some((
        PixelPoint {
            x: baked.at.x + paw.x,
            y: baked.at.y + paw.y,
        },
        // Where the kite's tail and string meet, at its lower point.
        PixelPoint {
            x: kite.at.x,
            y: kite.at.y + KITE_STRING_Y,
        },
    ))
}

/// Where a kite's string is tied, below the middle of its cell.
const KITE_STRING_Y: i32 = 2;

/// Whether something being played with or eaten is moving through the air on its own just now:
/// tossed, bounced, thrown, flown, or set down. Its owner may be standing still while it does,
/// and it still needs drawing as often as anything moving.
pub fn prop_in_motion(creature: &Creature) -> bool {
    if let Some(play) = play_shown(creature) {
        if calm(creature, play) {
            return false;
        }
        let (step, _, progress) = play.now();
        return match step {
            PlayMove::TossHigh | PlayMove::Bounce | PlayMove::Fly | PlayMove::Catch => true,
            PlayMove::Throw => progress >= 0.3,
            _ => false,
        };
    }
    super::meals::meal_shown(creature).is_some_and(|meal| {
        matches!(
            meal.step().0,
            formiga_core::meals::MealStep::TossCatch | formiga_core::meals::MealStep::SetDown
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::play::PlayShape;
    use formiga_core::{DesktopSnapshot, World};
    use rand::SeedableRng;

    fn player(thing: Thing) -> Creature {
        let mut creature = World::preview_adult(
            [33; 32],
            time::OffsetDateTime::UNIX_EPOCH,
            &DesktopSnapshot::default(),
        );
        creature.state.action = ActionKind::SoloPlay;
        creature.state.action_duration = 8.0;
        let mut rng = ChaCha12Rng::seed_from_u64(11);
        creature.state.play = Some(Play::plan(thing, 8.0, &mut rng));
        creature
    }

    /// Through a game, the paws hold the game's thing, or what its move calls for, and it goes
    /// where the move takes it rather than staying in the paws.
    #[test]
    fn a_game_holds_its_own_thing_and_moves_it_about() {
        for thing in [
            Thing::Toy(0),
            Thing::Belonging(ColonyObjectKind::Kite),
            Thing::Belonging(ColonyObjectKind::Umbrella),
            Thing::Trinket(16),
        ] {
            let mut creature = player(thing);
            let mut places = std::collections::BTreeSet::new();
            let mut t = 0.0;
            while t < 8.0 {
                creature.state.play.as_mut().unwrap().elapsed = t;
                let body = BodyPresentation::for_creature(&creature);
                let baked = CreatureRenderer::render_body_frame(
                    &creature.appearance,
                    body.clip,
                    body.frame,
                    false,
                )
                .held;
                let held = held_now(&creature, body.clip, baked)
                    .unwrap_or_else(|| panic!("{thing:?} has nothing in the paws at {t}"));
                if let Thing::Trinket(variant) = thing {
                    assert_eq!(held.thing, HeldThing::Trinket(variant));
                }
                places.insert((held.at.x, held.at.y));
                t += 0.25;
            }
            assert!(places.len() >= 4, "{thing:?} is held still");
        }
    }

    /// With motion reduced, switched on in the middle of a toss, the ball comes back to the paws
    /// and stays there, and the body keeps play's own calm hold, for the rest of the game.
    #[test]
    fn reduced_motion_holds_the_thing_still_in_the_paws() {
        let mut creature = player(Thing::Toy(0));
        let toss = (0..64)
            .map(|seed| Play::plan(Thing::Toy(0), 8.0, &mut ChaCha12Rng::seed_from_u64(seed)))
            .find_map(|play| {
                let start = play
                    .moves()
                    .iter()
                    .find(|(step, _)| *step == PlayMove::TossHigh)?
                    .1;
                Some((play, start))
            });
        let (play, start) = toss.expect("some game tosses a ball high");
        creature.state.play = Some(play);
        creature.state.presence.reduced = true;
        let mut t = start + 0.6;
        while t < play.length {
            creature.state.play.as_mut().unwrap().elapsed = t;
            let body = BodyPresentation::for_creature(&creature);
            assert_eq!(body.clip, BodyClip::Action(ActionKind::SoloPlay), "at {t}");
            let baked =
                CreatureRenderer::render_body_frame(&creature.appearance, body.clip, 0, true).held;
            let held = held_now(&creature, body.clip, baked).expect("held");
            assert_eq!(
                held,
                baked.unwrap().holding(HeldThing::Toy {
                    kind: 0,
                    colourway: toy_colourway(&creature.appearance),
                    spin: 0,
                }),
                "at {t}"
            );
            t += 0.2;
        }
    }

    /// Each shape is played with its own way: something round goes up and comes down, a stick is
    /// swung through its angles, a plane goes away and does not come back.
    #[test]
    fn each_shape_is_played_with_its_own_way() {
        let plan = |thing| Play::plan(thing, 8.0, &mut ChaCha12Rng::seed_from_u64(4));
        let round = plan(Thing::Toy(0));
        assert_eq!(round.shape, PlayShape::Round);
        let paper = plan(Thing::Belonging(ColonyObjectKind::Paper));
        let mut throw = paper;
        if let Some((_, start)) = paper.moves().iter().find(|(m, _)| *m == PlayMove::Throw) {
            throw.elapsed = start + 0.01;
            let near = play_offset(&throw).0;
            let end = paper
                .moves()
                .iter()
                .skip_while(|(m, s)| *m != PlayMove::Throw || s < start)
                .nth(1)
                .map_or(paper.length, |(_, s)| *s);
            throw.elapsed = end - 0.01;
            assert!(play_offset(&throw).0 > near + 30, "a plane glides away");
        }
    }

    /// However long a toss or a bounce comes out, it ends with the ball back where the paws hold
    /// it, so the next move starts from the catch rather than from mid-air.
    #[test]
    fn a_ball_is_back_in_the_paw_when_its_move_ends() {
        for seed in 0..60_u64 {
            for length in [3.0, 5.5, 9.0, 14.0] {
                let mut play =
                    Play::plan(Thing::Toy(0), length, &mut ChaCha12Rng::seed_from_u64(seed));
                let moves = play.moves().to_vec();
                for (index, (step, _)) in moves.iter().enumerate() {
                    if !matches!(step, PlayMove::TossHigh | PlayMove::Bounce) {
                        continue;
                    }
                    let end = moves.get(index + 1).map_or(play.length, |(_, next)| *next);
                    play.elapsed = end - 0.001;
                    let (_, dy) = play_offset(&play);
                    assert!(
                        dy.abs() <= 1,
                        "{step:?} ends {dy} pixels from the paw ({length}s, seed {seed})"
                    );
                }
            }
        }
    }

    /// A paper plane glides away and comes down on the ground ahead, and is still there, not
    /// back in the paws, when the game ends.
    #[test]
    fn a_paper_plane_lands_ahead_and_stays_there() {
        let mut play = Play::plan(
            Thing::Belonging(ColonyObjectKind::Paper),
            7.0,
            &mut ChaCha12Rng::seed_from_u64(3),
        );
        let landed = play
            .moves()
            .iter()
            .find(|(step, _)| *step == PlayMove::Landed)
            .map(|(_, start)| *start)
            .expect("a plane lands");
        play.elapsed = landed - 0.001;
        let gliding = play_offset(&play);
        play.elapsed = landed + 0.01;
        assert_eq!(play_offset(&play), gliding, "it lands where it glided to");
        play.elapsed = play.length - 0.01;
        assert_eq!(play_offset(&play), THROWN_TO);
        assert!(THROWN_TO.0 > 40 && THROWN_TO.1 > 0);
    }

    /// A kite is flown on a string from the paw to the kite, and only while it is flown.
    #[test]
    fn a_flown_kite_is_held_by_its_string() {
        let mut creature = player(Thing::Belonging(ColonyObjectKind::Kite));
        let mut flown = 0;
        let mut t = 0.0;
        while t < 8.0 {
            creature.state.play.as_mut().unwrap().elapsed = t;
            let body = BodyPresentation::for_creature(&creature);
            let baked = CreatureRenderer::render_body_frame(
                &creature.appearance,
                body.clip,
                body.frame,
                false,
            )
            .held;
            let tether = held_tether(&creature, body.clip, baked);
            let flying = creature.state.play.unwrap().now().0 == PlayMove::Fly;
            assert_eq!(tether.is_some(), flying, "at {t}");
            if let Some((paw, kite)) = tether {
                let held = held_now(&creature, body.clip, baked).unwrap();
                assert_eq!(kite.x, held.at.x);
                assert!(kite.y < paw.y, "the kite is up above the paw");
                flown += 1;
            }
            t += 0.1;
        }
        assert!(flown > 0, "the kite was never flown");
    }
}
