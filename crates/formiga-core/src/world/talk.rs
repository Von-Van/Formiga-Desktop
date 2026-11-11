//! Companions talking to each other: a short bubble when one greets another, asks a friend to
//! play, offers to share a meal, says goodnight, gets pounced on, catches a fish — and the
//! friend's answer a moment later. Each is said in the style that fits it, about what it is about
//! (see [`crate::talk`]).
//!
//! The pass watches what everyone has just started doing at the end of each tick, rolls dice of
//! its own, and writes nothing the simulation reads. A companion says something now and then, not
//! every time: each has a quiet spell after it speaks, and only a few bubbles are up at once.

use super::*;
use crate::play::PlayMove;
use crate::talk::{Saying, TALK_SECS, TalkBubble, TalkStyle, TalkTopic};

/// How long a companion keeps quiet after it has said something, in seconds.
const QUIET_SECS: f32 = 9.0;
/// The most bubbles companions have up to each other at once.
const MOST_TALK: usize = 4;
/// How near a friend has to be to be talked to, in points along the ground.
const EARSHOT: f32 = 150.0;
/// How long an answer waits after what it answers, in seconds.
const ANSWER_SECS: f32 = 0.9;
/// How near a companion answering the person at the desk keeps the room over its head, in points
/// each way: near enough for a second bubble to run into its answer. Nobody that near starts
/// saying something, and an answer due there waits until the person's answer has gone.
const ANSWER_ROOM: f32 = 64.0;

/// What the pass last saw a companion doing, to tell when it starts something.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Seen {
    action: Option<ActionKind>,
    beat: Option<BeatKind>,
    /// The move of a game it is playing, while the game is what is drawn.
    play: Option<PlayMove>,
    /// Asleep where it lay down, not still on its way to bed.
    settled: bool,
}

/// Where everyone is and what they have just started, for one tick's talk.
struct Speaker {
    id: CreatureId,
    at: Point,
    monitor: MonitorId,
    /// The window it stands on, or `None` on the floor: only companions on the same ground talk.
    ground: Option<WindowKey>,
    /// Asleep, having got to where it sleeps. A companion still walking to bed answers.
    asleep: bool,
    /// Drawn where the person can see it: outdoors, and not behind a house in the village.
    in_sight: bool,
    started: Seen,
    /// The companion what it is doing is aimed at: the friend it walked over to greet or to play
    /// with, or to sleep beside.
    toward: Option<CreatureId>,
}

impl Speaker {
    /// Whether `other` can hear what this one says: in sight, on the same ground, and in earshot.
    fn heard_by(&self, other: &Speaker) -> bool {
        other.id != self.id
            && other.in_sight
            && other.monitor == self.monitor
            && other.ground == self.ground
            && (other.at.x - self.at.x).abs() <= EARSHOT
    }
}

impl World {
    /// Bubbles companions have up to each other this frame, said already, oldest first. A
    /// companion out of sight behind a house in the village keeps what it is saying to itself
    /// until it comes out, so a bubble never floats over a house as if a neighbour had said it.
    pub fn talk_bubbles(&self) -> impl Iterator<Item = &TalkBubble> {
        self.talk
            .iter()
            .filter(|bubble| bubble.age >= 0.0 && self.talker_in_sight(bubble.creature_id))
    }

    /// Whether a companion is drawn where the person can see it.
    fn talker_in_sight(&self, creature_id: CreatureId) -> bool {
        !self.behind_a_house(creature_id)
            && self
                .save
                .creatures
                .iter()
                .any(|creature| creature.id == creature_id && !creature.state.indoors)
    }

    /// Whether a companion stands in the room kept over one answering the person at the desk
    /// (see [`ANSWER_ROOM`]), its own included.
    fn crowds_an_answer(&self, creature_id: CreatureId) -> bool {
        let place = |id: CreatureId| {
            self.save
                .creatures
                .iter()
                .find(|creature| creature.id == id)
                .map(|creature| (creature.state.surface.monitor_id, creature.state.position))
        };
        let Some((monitor, at)) = place(creature_id) else {
            return false;
        };
        self.bubbles
            .iter()
            .filter_map(|bubble| place(bubble.creature_id))
            .any(|(other_monitor, other)| {
                other_monitor == monitor
                    && (other.x - at.x).abs() < ANSWER_ROOM
                    && (other.y - at.y).abs() < ANSWER_ROOM
            })
    }

    /// Age, retire and start companions' talk. Called once, at the end of every tick.
    pub(super) fn carry_talk(&mut self, now: OffsetDateTime, dt: f32) {
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
                let previous = self.talk_seen.get(&creature.id).copied();
                // Asleep once it has got to bed, and still asleep if its breathing slides it
                // along afterwards.
                let settled = state.action == ActionKind::Sleep
                    && (previous.is_some_and(|seen| seen.settled) || !state.walking_to_sleep());
                let seen = Seen {
                    action: Some(state.action),
                    beat: state.beat.map(|beat| beat.kind),
                    play: state.play_shown().map(|play| play.now().0),
                    settled,
                };
                self.talk_seen.insert(creature.id, seen);
                let before = previous.unwrap_or(seen);
                let started = Seen {
                    action: (before.action != seen.action)
                        .then_some(seen.action)
                        .flatten(),
                    beat: (before.beat != seen.beat).then_some(seen.beat).flatten(),
                    play: (before.play != seen.play).then_some(seen.play).flatten(),
                    settled: settled && !before.settled,
                };
                let toward = self
                    .action_choices
                    .get(&creature.id)
                    .filter(|choice| choice.action == state.action)
                    .and_then(|choice| choice.target_creature);
                Speaker {
                    id: creature.id,
                    at: state.position,
                    monitor: state.surface.monitor_id,
                    ground: state.surface.window_key,
                    asleep: settled,
                    in_sight: !state.indoors && !self.behind_houses.contains(&creature.id),
                    started,
                    toward,
                }
            })
            .collect();
        self.age_talk(dt, &speakers);
        let streams = SeedStream::new(self.save.colony_seed);
        for speaker in &speakers {
            if speaker.started == Seen::default() || !speaker.in_sight {
                continue;
            }
            let mut rng = streams.rng(
                "talk",
                speaker.id ^ (now.unix_timestamp_nanos() as u64 / 1_000_000).rotate_left(19),
            );
            // Whoever it is talking to: the friend it walked over to, if it walked over to
            // anybody, and they are the only one who answers; otherwise the nearest in earshot.
            let friend = match speaker.toward {
                Some(target) => speakers
                    .iter()
                    .find(|other| other.id == target)
                    .filter(|other| speaker.heard_by(other)),
                None => speakers
                    .iter()
                    .filter(|other| speaker.heard_by(other))
                    .min_by(|a, b| {
                        (a.at.x - speaker.at.x)
                            .abs()
                            .total_cmp(&(b.at.x - speaker.at.x).abs())
                    }),
            };
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
                    // A game with a thing in it is talked about with that thing; a chase or a
                    // game of tag, which is all a pair at play is, with pawprints.
                    let topic = self
                        .save
                        .creatures
                        .iter()
                        .find(|c| c.id == speaker.id)
                        .and_then(|c| c.state.play_shown())
                        .map_or(TalkTopic::Footprints, |play| {
                            TalkTopic::of_shape(play.shape)
                        });
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
                        Saying::one(TalkStyle::Chat, TalkTopic::Food),
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
                        Saying::one(TalkStyle::Excited, topic),
                        awake.map(|friend| (friend.id, answer)),
                    ));
                }
                _ => {}
            }
            // Goodnight once it has lain down, not as it sets off for bed.
            if speaker.started.settled && awake.is_some() && rng.random_bool(0.4) {
                said = Some((
                    Saying::one(TalkStyle::Whisper, TalkTopic::Moon),
                    awake.map(|friend| {
                        (friend.id, Saying::one(TalkStyle::Whisper, TalkTopic::Heart))
                    }),
                ));
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
                        Saying::one(TalkStyle::Excited, TalkTopic::Fish),
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
                && self.say(speaker.id, saying, None)
                && let Some((friend, answer)) = answer
            {
                self.say(friend, answer, Some(speaker.id));
            }
        }
    }

    /// Ages every bubble. An answer still waiting is said only while it still makes sense — the
    /// one it answers there, and the one answering awake, in sight and in earshot of them — and
    /// waits while the room over it is kept for an answer to the person at the desk, for as long
    /// as a bubble lasts at most.
    fn age_talk(&mut self, dt: f32, speakers: &[Speaker]) {
        let find = |id: CreatureId| speakers.iter().find(|speaker| speaker.id == id);
        self.talk.retain(|bubble| {
            let Some(asked_by) = bubble.answering.filter(|_| bubble.age < 0.0) else {
                return true;
            };
            match (find(asked_by), find(bubble.creature_id)) {
                (Some(asker), Some(answerer)) => {
                    asker.in_sight && !answerer.asleep && asker.heard_by(answerer)
                }
                _ => false,
            }
        });
        let crowded: BTreeSet<CreatureId> = self
            .talk
            .iter()
            .filter(|bubble| bubble.age < 0.0 && bubble.age + dt >= 0.0)
            .map(|bubble| bubble.creature_id)
            .filter(|id| self.crowds_an_answer(*id))
            .collect();
        for bubble in &mut self.talk {
            if bubble.age < 0.0 && crowded.contains(&bubble.creature_id) {
                bubble.held += dt;
            } else {
                bubble.age += dt;
            }
        }
        self.talk
            .retain(|bubble| bubble.age < TALK_SECS && bubble.held < TALK_SECS);
    }

    /// Puts a bubble up over `creature_id`, unless it is keeping quiet, is already answering the
    /// person at the desk, stands where the room is kept for somebody who is, or the desktop has
    /// enough talk on it already. An answer to `answering` comes a moment after what it answers,
    /// and never cuts off what the one answering is saying: it waits for that to finish, and only
    /// one answer waits at a time.
    fn say(
        &mut self,
        creature_id: CreatureId,
        saying: Saying,
        answering: Option<CreatureId>,
    ) -> bool {
        let answer = answering.is_some();
        if !answer
            && (self
                .talk_quiet
                .get(&creature_id)
                .is_some_and(|quiet| *quiet > 0.0)
                || self.crowds_an_answer(creature_id))
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
        let mut age = if answer { -ANSWER_SECS } else { 0.0 };
        for bubble in self.talk.iter().filter(|b| b.creature_id == creature_id) {
            if !answer || bubble.age < 0.0 {
                return false;
            }
            age = age.min(bubble.age - TALK_SECS);
        }
        if self.talk.len() >= MOST_TALK {
            return false;
        }
        self.talk.push(TalkBubble {
            creature_id,
            saying,
            age,
            answering,
            held: 0.0,
        });
        self.talk_quiet.insert(creature_id, QUIET_SECS);
        true
    }
}
