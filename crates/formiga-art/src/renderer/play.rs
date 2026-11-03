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
        PlayMove::Throw => gesture(Gesture::Watch),
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
        PlayMove::PutDown => ExpressionKind::Content,
        _ => ExpressionKind::Joy,
    }
}

/// Where the eyes go: up after something thrown high or flown, down at something set going on
/// the ground, after a plane as it goes.
pub(super) fn play_gaze(creature: &Creature, play: &Play) -> Option<GazeDirection> {
    let forward = if creature.state.facing_right { 1 } else { -1 };
    let (step, _, progress) = play.now();
    match step {
        PlayMove::TossHigh | PlayMove::Balance | PlayMove::Ride => Some(GazeDirection::new(0, -1)),
        PlayMove::Fly => Some(GazeDirection::new(forward, -1)),
        PlayMove::Throw if progress >= 0.3 => Some(GazeDirection::new(forward, 0)),
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
        PlayMove::TossHigh => (2, -round((PI * (into / 1.2).fract()).sin() * 18.0)),
        PlayMove::Bounce => (4, round(8.0 - (PI * (into / 0.8).fract()).sin() * 10.0)),
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
            let gone = (progress - 0.3) / 0.7;
            (6 + round(gone * 46.0), -6 - round((gone * PI).sin() * 5.0))
        }
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
}
