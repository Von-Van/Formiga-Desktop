//! Companions talking to each other: a short bubble when one greets another, asks a friend to
//! play, offers to share a meal, says goodnight, gets pounced on, catches a fish — and the
//! friend's answer a moment later. Each is said in the style that fits it, about what it is about
//! (see [`crate::talk`]).
//!
//! The pass watches what everyone has just started doing at the end of each tick, rolls dice of
//! its own, and writes nothing the simulation reads. A companion says something now and then, not
//! every time: each has a quiet spell after it speaks, and only a few bubbles are up at once.

use super::*;
use crate::play::{PlayMove, Plaything, own_toy};
use crate::talk::{Saying, TALK_SECS, TalkBubble, TalkStyle, TalkTopic};

/// How long a companion keeps quiet after it has said something, in seconds.
const QUIET_SECS: f32 = 9.0;
/// The most bubbles companions have up to each other at once.
const MOST_TALK: usize = 4;
/// How near a friend has to be to be talked to, in points along the ground.
const EARSHOT: f32 = 150.0;
/// How long an answer waits after what it answers, in seconds.
const ANSWER_SECS: f32 = 0.9;

/// What the pass last saw a companion doing, to tell when it starts something.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Seen {
    action: Option<ActionKind>,
    beat: Option<BeatKind>,
    play: Option<PlayMove>,
}

/// Where everyone is and what they have just started, for one tick's talk.
struct Speaker {
    id: CreatureId,
    at: Point,
    monitor: MonitorId,
    asleep: bool,
    started: Seen,
    toy: u8,
}

impl World {
    /// Bubbles companions have up to each other this frame, said already, oldest first.
    pub fn talk_bubbles(&self) -> impl Iterator<Item = &TalkBubble> {
        self.talk.iter().filter(|bubble| bubble.age >= 0.0)
    }

    /// Age, retire and start companions' talk. Called once, at the end of every tick.
    pub(super) fn carry_talk(&mut self, now: OffsetDateTime, dt: f32) {
        for bubble in &mut self.talk {
            bubble.age += dt;
        }
        for quiet in self.talk_quiet.values_mut() {
            *quiet -= dt;
        }
        let here: BTreeSet<CreatureId> = self.save.creatures.iter().map(|c| c.id).collect();
        self.talk
            .retain(|bubble| bubble.age < TALK_SECS && here.contains(&bubble.creature_id));
        self.talk_seen.retain(|id, _| here.contains(id));
        if !self.save.settings.visible || self.save.settings.paused {
            self.talk.clear();
            return;
        }
        let speakers: Vec<Speaker> = self
            .save
            .creatures
            .iter()
            .map(|creature| {
                let state = &creature.state;
                let seen = Seen {
                    action: Some(state.action),
                    beat: state.beat.map(|beat| beat.kind),
                    play: state.play.map(|play| play.now().0),
                };
                let before = self.talk_seen.insert(creature.id, seen).unwrap_or(seen);
                let started = Seen {
                    action: (before.action != seen.action)
                        .then_some(seen.action)
                        .flatten(),
                    beat: (before.beat != seen.beat).then_some(seen.beat).flatten(),
                    play: (before.play != seen.play).then_some(seen.play).flatten(),
                };
                Speaker {
                    id: creature.id,
                    at: state.position,
                    monitor: state.surface.monitor_id,
                    asleep: state.action == ActionKind::Sleep,
                    started,
                    toy: own_toy(&creature.appearance),
                }
            })
            .collect();
        let streams = SeedStream::new(self.save.colony_seed);
        for speaker in &speakers {
            if speaker.started == Seen::default() {
                continue;
            }
            let mut rng = streams.rng(
                "talk",
                speaker.id ^ (now.unix_timestamp_nanos() as u64 / 1_000_000).rotate_left(19),
            );
            // The nearest friend in earshot, awake or not.
            let friend = speakers
                .iter()
                .filter(|other| other.id != speaker.id && other.monitor == speaker.monitor)
                .map(|other| (other, (other.at.x - speaker.at.x).abs()))
                .filter(|(_, distance)| *distance <= EARSHOT)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(other, _)| other);
            let awake = friend.filter(|friend| !friend.asleep);
            let mut said: Option<(Saying, Option<(CreatureId, Saying)>)> = None;
            match speaker.started.action {
                Some(ActionKind::Greet) => {
                    let answer = if rng.random_bool(0.4) {
                        Saying::one(TalkStyle::Tender, TalkTopic::Heart)
                    } else {
                        Saying::one(TalkStyle::Chat, TalkTopic::Wave)
                    };
                    said = Some((
                        Saying::one(TalkStyle::Chat, TalkTopic::Wave),
                        awake.map(|friend| (friend.id, answer)),
                    ));
                }
                Some(ActionKind::SocialPlay) => {
                    let topic = Plaything::Toy(speaker.toy)
                        .shape()
                        .map_or(TalkTopic::Footprints, TalkTopic::of_shape);
                    said = Some((
                        Saying::one(TalkStyle::Excited, topic),
                        awake.map(|friend| {
                            (
                                friend.id,
                                Saying::one(TalkStyle::Excited, TalkTopic::Exclaim),
                            )
                        }),
                    ));
                }
                Some(ActionKind::Eat) if awake.is_some() && rng.random_bool(0.35) => {
                    said = Some((
                        Saying::two(TalkStyle::Chat, TalkTopic::Food, TalkTopic::Question),
                        awake.map(|friend| {
                            (friend.id, Saying::one(TalkStyle::Tender, TalkTopic::Heart))
                        }),
                    ));
                }
                Some(ActionKind::SoloPlay) if awake.is_some() && rng.random_bool(0.3) => {
                    let me = self.save.creatures.iter().find(|c| c.id == speaker.id);
                    let topic = me
                        .and_then(|c| c.state.play)
                        .map_or(TalkTopic::Ball, |play| TalkTopic::of_shape(play.shape));
                    let answer = if rng.random_bool(0.6) {
                        Saying::one(TalkStyle::Excited, TalkTopic::Exclaim)
                    } else {
                        Saying::one(TalkStyle::Thought, TalkTopic::Dots)
                    };
                    said = Some((
                        Saying::two(TalkStyle::Excited, topic, TalkTopic::Question),
                        awake.map(|friend| (friend.id, answer)),
                    ));
                }
                Some(ActionKind::Sleep) if awake.is_some() && rng.random_bool(0.4) => {
                    said = Some((
                        Saying::one(TalkStyle::Whisper, TalkTopic::Moon),
                        awake.map(|friend| {
                            (friend.id, Saying::one(TalkStyle::Whisper, TalkTopic::Heart))
                        }),
                    ));
                }
                _ => {}
            }
            match speaker.started.beat {
                Some(BeatKind::Startle) => {
                    said = Some((Saying::one(TalkStyle::Grumble, TalkTopic::Storm), None));
                }
                Some(BeatKind::Pounce) => {
                    said = Some((Saying::one(TalkStyle::Excited, TalkTopic::Sparkle), None));
                }
                Some(BeatKind::Stomp) => {
                    said = Some((Saying::one(TalkStyle::Shout, TalkTopic::No), None));
                }
                _ => {}
            }
            match speaker.started.play {
                Some(PlayMove::Catch) => {
                    said = Some((
                        Saying::two(TalkStyle::Excited, TalkTopic::Fish, TalkTopic::Exclaim),
                        awake.map(|friend| {
                            (
                                friend.id,
                                Saying::one(TalkStyle::Excited, TalkTopic::Sparkle),
                            )
                        }),
                    ));
                }
                Some(PlayMove::Topple) => {
                    said = Some((Saying::one(TalkStyle::Shout, TalkTopic::Exclaim), None));
                }
                Some(PlayMove::Ride) => {
                    said = Some((Saying::one(TalkStyle::Excited, TalkTopic::Umbrella), None));
                }
                Some(PlayMove::Dance | PlayMove::Music) => {
                    said = Some((Saying::one(TalkStyle::Song, TalkTopic::Note), None));
                }
                _ => {}
            }
            if let Some((saying, answer)) = said
                && self.say(speaker.id, saying, 0.0)
                && let Some((friend, answer)) = answer
            {
                self.say(friend, answer, -ANSWER_SECS);
            }
        }
    }

    /// Puts a bubble up over `creature_id`, starting at `age`, unless it is keeping quiet, is
    /// already answering the person at the desk, or the desktop has enough talk on it already.
    fn say(&mut self, creature_id: CreatureId, saying: Saying, age: f32) -> bool {
        let answering = age < 0.0;
        if !answering
            && self
                .talk_quiet
                .get(&creature_id)
                .is_some_and(|quiet| *quiet > 0.0)
        {
            return false;
        }
        if self
            .bubbles
            .iter()
            .any(|bubble| bubble.creature_id == creature_id)
        {
            return false;
        }
        self.talk.retain(|bubble| bubble.creature_id != creature_id);
        if self.talk.len() >= MOST_TALK {
            return false;
        }
        self.talk.push(TalkBubble {
            creature_id,
            saying,
            age,
        });
        self.talk_quiet.insert(creature_id, QUIET_SECS);
        true
    }
}
