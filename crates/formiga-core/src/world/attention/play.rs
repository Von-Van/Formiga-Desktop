//! Small social scenes promoted from visible ordinary encounters. No game-start timer or history.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Copycat(ActionKind),
    Stare,
    Chase,
    Parade,
    Dance,
    Pile,
    Leapfrog,
    KeepAway,
    Tug,
    Tag,
    JumpContest,
    Hush,
    Prank,
    FollowLeader,
    Hill,
    Race,
    Lava,
    HideAndSeek,
}

impl Kind {
    /// Scenes about someone who is not taking part, such as a companion asleep.
    fn solo(self) -> bool {
        matches!(self, Self::Hush | Self::Prank)
    }
}

/// One temporary plaything. It exists only while a scene does, is always in someone's hands, and
/// is drawn with the existing carried-object quad, so it adds no artwork and no draw call.
#[derive(Clone, Copy, Debug)]
pub(super) struct Prop {
    pub holder: CreatureId,
    /// Hand-offs so far; a bounded number keeps a tussle from running forever.
    pub handoffs: u8,
    pub remaining: f32,
}

impl Kind {
    /// How long an ordinary encounter must hold before it becomes an invitation.
    fn invitation_seconds(self) -> f32 {
        match self {
            Self::Copycat(_) => 0.6,
            Self::Dance => 0.9,
            Self::Chase | Self::Leapfrog | Self::KeepAway | Self::Tug | Self::Tag => 1.0,
            Self::JumpContest => 1.2,
            Self::Hush | Self::Prank => 1.3,
            Self::FollowLeader => 1.3,
            Self::Hill => 1.5,
            Self::Race => 1.4,
            Self::Lava => 1.2,
            Self::HideAndSeek => 1.6,
            Self::Parade => 1.4,
            Self::Pile => 1.5,
            Self::Stare => 2.5,
        }
    }

    /// Scenes that travel need room, and never begin with reduced motion enabled.
    pub(super) fn travels(self) -> bool {
        !matches!(self, Self::Copycat(_) | Self::Stare)
    }

    fn length(self) -> f32 {
        match self {
            Self::Copycat(_) => 3.2,
            Self::JumpContest => 18.0,
            Self::Tug => 6.5,
            Self::Prank => 7.5,
            Self::Hush => 8.5,
            Self::Hill => 10.0,
            Self::FollowLeader => 11.0,
            Self::Dance => 7.0,
            Self::Pile => 8.0,
            Self::Leapfrog => 10.0,
            Self::Race => 16.0,
            Self::Lava => 12.0,
            Self::HideAndSeek => 20.0,
            _ => 9.0,
        }
    }

    /// The appetite an invitation asks of the creature receiving it.
    fn appetite(self) -> f32 {
        match self {
            Self::Pile => 0.3,
            Self::Stare | Self::Parade => 0.4,
            Self::Copycat(_) | Self::Dance => 0.5,
            Self::Chase | Self::Leapfrog | Self::KeepAway | Self::Tug | Self::Tag => 0.6,
            Self::JumpContest => 0.65,
            Self::Hush => 0.35,
            Self::Prank => 0.7,
            Self::FollowLeader => 0.45,
            Self::Hill => 0.6,
            Self::Race => 0.6,
            Self::Lava => 0.5,
            Self::HideAndSeek => 0.55,
        }
    }

    /// Games whose whole point is how the members feel about where they are standing. These set
    /// their own mood; the shared wind-down still speaks for everyone at the end.
    fn expressive(self) -> bool {
        matches!(self, Self::Lava | Self::HideAndSeek)
    }

    /// Whose side of the scene the audience is on. Usually whoever is leading it — the runner,
    /// the one who is "it", the one holding the best spot. In hide and seek it is the seeker,
    /// because a row of companions staring at the hiding place would give the hider away.
    fn watched_role(self) -> PlayRole {
        match self {
            Self::HideAndSeek => PlayRole::Follow,
            _ => PlayRole::Lead,
        }
    }

    /// Games whose members leave the shared plan for a moment to cross a gap of their own.
    pub(super) fn hops(self) -> bool {
        matches!(self, Self::JumpContest | Self::Race | Self::Lava)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum PlayRole {
    /// Symmetric participants: imitation, contests, dancing.
    #[default]
    Partner,
    /// The runner of a chase or the front of a procession.
    Lead,
    Follow,
    /// A resting creature a pile forms around; it is never commandeered.
    Anchor,
}

#[derive(Clone, Copy)]
struct Encounter {
    pair: [CreatureId; 2],
    kind: Kind,
    seconds: f32,
}

#[derive(Clone, Copy)]
pub(super) struct Session {
    pub(super) kind: Kind,
    pub(super) origin: Origin,
    pub(super) members: [Option<CreatureId>; MAX_PARTICIPANTS],
    pub(super) roles: [PlayRole; MAX_PARTICIPANTS],
    pub(super) count: usize,
    pub(super) elapsed: f32,
    turn: usize,
    pub(super) turn_started: f32,
    pub(super) ends_at: f32,
    /// Index of the member who breaks eye contact first.
    loser: usize,
    distracted: bool,
    /// Bounded role swaps, so a chase cannot ping-pong forever.
    pub(super) swaps: u8,
    pub(super) seed: u64,
    /// The one temporary plaything a scene may involve.
    pub(super) prop: Option<Prop>,
    /// A creature the scene is about without taking part, such as someone asleep.
    pub(super) focus: Option<CreatureId>,
    /// A few points a leader has just passed through, or the one place a seeker last saw someone.
    /// Positions only, held only while the scene lasts.
    pub(super) trail: [Option<Point>; 4],
    /// The window a race runs to. Other scenes leave it empty.
    pub(super) goal: Option<WindowKey>,
}

impl Session {
    pub(super) fn with_role(
        &self,
        role: PlayRole,
    ) -> impl Iterator<Item = (usize, CreatureId)> + '_ {
        self.members
            .iter()
            .enumerate()
            .filter(move |(index, _)| self.roles[*index] == role)
            .filter_map(|(index, id)| id.map(|id| (index, id)))
    }
}

/// A refused invitation, so the same pair is not asked again immediately.
#[derive(Clone, Copy)]
struct Refusal {
    pair: [CreatureId; 2],
    remaining: f32,
}

/// A declined invitation is respected for this long before the same pair is considered again.
const DECLINED_SECONDS: f32 = 75.0;

/// The moves a copy chain can pass along. One is picked per scene, from the scene's own seed.
const COPIED_FLOURISHES: [Gesture; 5] = [
    Gesture::Cheer,
    Gesture::Bop,
    Gesture::Reach,
    Gesture::Crouch,
    Gesture::Balance,
];

/// How long a creature can hold a look: temperament and wakefulness, not a coin flip.
pub(super) fn composure(c: &Creature) -> f32 {
    (c.personality.boldness * 0.4
        + c.personality.curiosity * 0.2
        + c.personality.playfulness * 0.1
        + c.state.drives.energy * 0.2
        - c.state.drives.sleep_pressure * 0.3)
        .clamp(0.0, 1.0)
}

#[derive(Default)]
pub(super) struct PlayRuntime {
    encounter: Option<Encounter>,
    pub(super) session: Option<Session>,
    pub(super) cooldown: f32,
    refusals: [Option<Refusal>; MAX_PARTICIPANTS],
}

impl PlayRuntime {
    pub(super) fn reset(&mut self) {
        self.encounter = None;
        self.session = None;
        self.cooldown = 12.0;
        self.refusals = Default::default();
    }

    pub(super) fn invitation_ready(&self) -> bool {
        self.encounter
            .is_some_and(|e| e.seconds >= e.kind.invitation_seconds())
    }

    fn refused(&self, pair: [CreatureId; 2]) -> bool {
        self.refusals
            .iter()
            .flatten()
            .any(|r| (r.pair == pair || r.pair == [pair[1], pair[0]]) && r.remaining > 0.0)
    }

    fn refuse(&mut self, pair: [CreatureId; 2], seconds: f32) {
        let slot = self
            .refusals
            .iter()
            .position(Option::is_none)
            .unwrap_or_else(|| {
                (0..MAX_PARTICIPANTS)
                    .min_by(|&a, &b| {
                        let remaining = |i: usize| self.refusals[i].map_or(0.0, |r| r.remaining);
                        remaining(a).total_cmp(&remaining(b))
                    })
                    .unwrap()
            });
        self.refusals[slot] = Some(Refusal {
            pair,
            remaining: seconds,
        });
    }

    fn decay(&mut self, dt: f32) {
        self.cooldown = (self.cooldown - dt).max(0.0);
        for slot in &mut self.refusals {
            if let Some(refusal) = slot {
                refusal.remaining -= dt;
                if refusal.remaining <= 0.0 {
                    *slot = None;
                }
            }
        }
    }
}

impl World {
    pub(super) fn play_available(&self, c: &Creature, desktop: &DesktopSnapshot) -> bool {
        self.play_ready(c, desktop, false)
    }

    /// `following` allows a creature already walking after a companion to be the one who invites,
    /// so ordinary following can become play rather than competing with it.
    fn play_ready(&self, c: &Creature, desktop: &DesktopSnapshot, following: bool) -> bool {
        c.state.arrival_delay_secs <= 0.0
            && c.state.drives.energy > 0.35
            && c.state.drives.sleep_pressure < 0.7
            && c.personality.sociability > 0.35
            && (!self.bond_plans.contains_key(&c.id)
                || (following && c.state.action == ActionKind::Follow))
            && !self.window_journeys.contains_key(&c.id)
            && !self.tosses.contains_key(&c.id)
            && !self
                .interaction
                .as_ref()
                .is_some_and(|i| i.creature_id == c.id)
            && point_exposed(
                head_point(c, &self.save.settings, desktop),
                c.state.surface.window_key,
                desktop,
            )
            && desktop.monitors.iter().any(|m| {
                m.id == c.state.surface.monitor_id
                    && habitat_contains(&self.save.settings.habitat, m, c.state.position)
            })
    }

    fn play_pair(&self, a: &Creature, b: &Creature, desktop: &DesktopSnapshot) -> bool {
        self.play_pair_spaced(a, b, desktop, 46.0)
    }

    pub(super) fn play_pair_spaced(
        &self,
        a: &Creature,
        b: &Creature,
        desktop: &DesktopSnapshot,
        nearest: f32,
    ) -> bool {
        // Measured against the frame, unclamped: the clamp that used to be here left the band
        // narrower than the creatures themselves at the largest display scale.
        let scale = super::super::spacing::creature_frame_width(
            a,
            self.save.settings.display_scale,
            desktop,
        ) / CREATURE_ART_WIDTH;
        a.id != b.id
            && a.state.surface.monitor_id == b.state.surface.monitor_id
            && a.state.surface.window_key == b.state.surface.window_key
            && (a.state.position.y - b.state.position.y).abs() < 4.0
            && (nearest * scale..=180.0 * scale)
                .contains(&a.state.position.distance(b.state.position))
            && !self.save.relationships.iter().any(|r| {
                ((r.a == a.id && r.b == b.id) || (r.a == b.id && r.b == a.id)) && r.avoidance >= 128
            })
    }

    /// Whether an invited creature takes up the offer: temperament, needs, and the existing bond.
    fn accepts(&self, invitee: &Creature, initiator: &Creature, kind: Kind) -> bool {
        let bond = self.save.relationships.iter().find(|r| {
            (r.a == invitee.id && r.b == initiator.id) || (r.b == invitee.id && r.a == initiator.id)
        });
        let appetite = invitee.personality.playfulness * 0.4
            + invitee.personality.sociability * 0.3
            + invitee.state.drives.energy * 0.2
            + invitee.state.drives.boredom * 0.1
            + bond.map_or(0.0, |b| f32::from(b.affinity) / 255.0 * 0.2)
            - invitee.state.drives.sleep_pressure * 0.4
            - bond.map_or(0.0, |b| f32::from(b.avoidance) / 255.0 * 0.3);
        appetite >= kind.appetite()
    }

    pub(super) fn observe_play(&mut self, desktop: &DesktopSnapshot, dt: f32) {
        self.attention.play.decay(dt);
        if self.attention.play.cooldown > 0.0
            || !self.attention.plans.is_empty()
            || self.attention.colony_cooldown > 0.0
            || self.geometry_observer.signals().next().is_some()
            || self.cursor_observer.cue().is_some()
            || self.display_attention.has_pending()
        {
            self.attention.play.encounter = None;
            return;
        }
        let mut candidate = None;
        let travelling = !self.save.settings.reduce_motion;
        for a in self.save.creatures.iter().take(MAX_PARTICIPANTS) {
            if !self.play_ready(a, desktop, true) {
                continue;
            }
            let following = self
                .bond_plans
                .get(&a.id)
                .filter(|_| a.state.action == ActionKind::Follow)
                .map(|plan| plan.target);
            for b in self.save.creatures.iter().take(MAX_PARTICIPANTS) {
                let receptive = matches!(
                    b.state.action,
                    ActionKind::Idle | ActionKind::Perch | ActionKind::InspectScreen
                );
                let chasing = following == Some(b.id);
                let sleeping = b.state.action == ActionKind::Sleep;
                if !self.play_available(b, desktop)
                    || !self.play_pair_spaced(a, b, desktop, if chasing { 24.0 } else { 46.0 })
                    || (!receptive && !chasing && !sleeping)
                    || self.attention.cooldowns.contains_key(&a.id)
                    || self.attention.cooldowns.contains_key(&b.id)
                    || self.attention.play.refused([a.id, b.id])
                {
                    continue;
                }
                // A solo flourish with enough playful company gathers a circle; a greeting is
                // imitated instead.
                let gathering = travelling
                    && a.state.action == ActionKind::SoloPlay
                    && self
                        .save
                        .creatures
                        .iter()
                        .filter(|c| {
                            c.id != a.id
                                && c.personality.playfulness > 0.55
                                && self.play_available(c, desktop)
                                && self.play_pair(a, c, desktop)
                        })
                        .count()
                        >= 2;
                let kind = if chasing && travelling {
                    // Lively pursuit becomes a chase; a calmer one becomes a procession.
                    Some(
                        if a.personality.playfulness > 0.55 && b.personality.playfulness > 0.5 {
                            if a.state.drives.energy > 0.5 && b.state.drives.energy > 0.5 {
                                Kind::Chase
                            } else {
                                Kind::FollowLeader
                            }
                        } else {
                            Kind::Parade
                        },
                    )
                } else if travelling
                    && b.state.action == ActionKind::Sleep
                    && a.personality.sociability > 0.5
                    && matches!(
                        a.state.action,
                        ActionKind::Idle | ActionKind::Perch | ActionKind::InspectScreen
                    )
                {
                    // A companion is asleep: creep around them, or try to wake them.
                    Some(
                        if a.personality.playfulness > 0.7 && b.state.drives.sleep_pressure < 0.55 {
                            Kind::Prank
                        } else {
                            Kind::Hush
                        },
                    )
                } else if travelling
                    && a.state.action == ActionKind::Sprint
                    && a.personality.playfulness > 0.5
                    && b.personality.playfulness > 0.5
                    && b.state.drives.energy > 0.5
                {
                    // One of them is dashing about and the other is game: tag.
                    Some(Kind::Tag)
                } else if travelling
                    && receptive
                    && b.state.surface.window_key.is_some()
                    && a.personality.playfulness > 0.55
                    && a.personality.boldness > 0.45
                    && matches!(a.state.action, ActionKind::Idle | ActionKind::Perch)
                    && self.hill_spot(b, desktop).is_some_and(|spot| {
                        (b.state.position.x - spot.x).abs() < 40.0
                            && (a.state.position.x - spot.x).abs() > 60.0
                    })
                {
                    // A companion has the best spot on this ledge; try to take it.
                    Some(Kind::Hill)
                } else if travelling
                    && receptive
                    && a.state.surface.window_key.is_some()
                    && a.personality.playfulness > 0.6
                    && b.personality.playfulness > 0.6
                    && a.state.drives.energy > 0.5
                    && b.state.drives.energy > 0.5
                    && matches!(a.state.action, ActionKind::Idle | ActionKind::Perch)
                    && self.race_destination(a, b, 2, desktop).is_some()
                {
                    // A window away across the desktop that both of them could reach: a race.
                    Some(Kind::Race)
                } else if travelling
                    && receptive
                    && a.state.surface.window_key.is_some()
                    && a.personality.playfulness > 0.6
                    && b.personality.playfulness > 0.6
                    && a.state.drives.energy > 0.5
                    && matches!(a.state.action, ActionKind::Idle | ActionKind::Perch)
                    && self.gap_candidate(a, desktop).is_some()
                    && self.attention.setback(a.id).is_none()
                {
                    // A gap both of them could try, with company to watch: take turns.
                    Some(Kind::JumpContest)
                } else if travelling
                    && receptive
                    && a.personality.playfulness > 0.55
                    && b.personality.playfulness > 0.55
                    && matches!(a.state.action, ActionKind::Idle | ActionKind::Perch)
                    && self.lava_ready(a, b, desktop)
                {
                    // Standing at the end of a ledge with a long way down: the floor is lava.
                    Some(Kind::Lava)
                } else if travelling
                    && a.state.action == ActionKind::SocialPlay
                    && b.personality.playfulness > 0.6
                    && a.personality.playfulness > 0.6
                    && b.state.drives.energy > 0.5
                {
                    // Playing with a toy beside a playful companion: they want it too.
                    Some(Kind::KeepAway)
                } else if matches!(a.state.action, ActionKind::Greet | ActionKind::SoloPlay)
                    && b.personality.playfulness > 0.55
                    && b.state.facing_right == (a.state.position.x > b.state.position.x)
                {
                    Some(if gathering {
                        Kind::Dance
                    } else if travelling
                        && a.state.action == ActionKind::SoloPlay
                        && a.personality.playfulness > 0.65
                        && b.personality.playfulness > 0.65
                    {
                        Kind::Tug
                    } else {
                        Kind::Copycat(a.state.action)
                    })
                } else if matches!(
                    a.state.action,
                    ActionKind::Idle | ActionKind::Perch | ActionKind::InspectScreen
                ) && a.state.facing_right == (b.state.position.x > a.state.position.x)
                    && b.state.facing_right == (a.state.position.x > b.state.position.x)
                    && a.personality.curiosity > 0.5
                    && b.personality.curiosity > 0.5
                    && a.personality.boldness > 0.25
                    && b.personality.boldness > 0.25
                {
                    Some(Kind::Stare)
                } else if travelling
                    && receptive
                    && a.state.facing_right == b.state.facing_right
                    && a.state.facing_right == (b.state.position.x > a.state.position.x)
                    && a.personality.playfulness > 0.6
                    && b.personality.playfulness > 0.6
                    && a.state.drives.energy > 0.5
                    && matches!(
                        a.state.action,
                        ActionKind::Idle | ActionKind::Perch | ActionKind::InspectScreen
                    )
                    && self.play_pair_spaced(a, b, desktop, 24.0)
                    && a.state.position.distance(b.state.position) < 110.0
                {
                    // Two playful companions lined up along a surface, one behind the other.
                    Some(Kind::Leapfrog)
                } else if travelling
                    && matches!(b.state.action, ActionKind::Idle | ActionKind::Perch)
                    && b.state.action_elapsed >= 3.0
                    && a.personality.sociability > 0.6
                    && a.state.drives.social_need > 0.35
                    && matches!(
                        a.state.action,
                        ActionKind::Idle | ActionKind::Perch | ActionKind::InspectScreen
                    )
                {
                    // Settling beside a companion who is already resting.
                    Some(Kind::Pile)
                } else if travelling
                    && receptive
                    && a.state.surface.window_key.is_some()
                    && a.personality.playfulness > 0.55
                    && b.personality.playfulness > 0.55
                    && b.personality.curiosity > 0.55
                    && a.state.drives.energy > 0.45
                    && b.state.drives.energy > 0.45
                    && matches!(a.state.action, ActionKind::Idle | ActionKind::Perch)
                    && self.hiding_spot(a, b.state.position, desktop).is_some()
                {
                    // Somewhere on this surface to get out of sight: count, and come looking.
                    Some(Kind::HideAndSeek)
                } else {
                    None
                };
                if let Some(kind) = kind {
                    candidate = Some(Encounter {
                        pair: [a.id, b.id],
                        kind,
                        seconds: dt,
                    });
                    break;
                }
            }
            if candidate.is_some() {
                break;
            }
        }
        if let Some(e) = &mut candidate
            && let Some(previous) = self.attention.play.encounter
            && previous.pair == e.pair
            && previous.kind == e.kind
        {
            e.seconds += previous.seconds;
        }
        self.attention.play.encounter = candidate;
    }

    pub(super) fn try_play(&mut self, desktop: &DesktopSnapshot) {
        if !self.attention.play.invitation_ready() || !self.attention.plans.is_empty() {
            return;
        }
        let e = self.attention.play.encounter.take().unwrap();
        let Some(initiator) = self.save.creatures.iter().find(|c| c.id == e.pair[0]) else {
            return;
        };
        let Some(invitee) = self.save.creatures.iter().find(|c| c.id == e.pair[1]) else {
            return;
        };
        // An invitation may be declined; that pair is then left alone for a while.
        if !self.accepts(invitee, initiator, e.kind) {
            self.attention.play.refuse(e.pair, DECLINED_SECONDS);
            self.attention.play.cooldown = 12.0;
            return;
        }
        // The creature that started it leads a dance, but follows in a chase or a procession.
        let roles = match e.kind {
            // The one being followed leads; the one already on the spot holds it.
            Kind::Chase | Kind::Parade | Kind::FollowLeader | Kind::Hill => [
                PlayRole::Follow,
                PlayRole::Lead,
                PlayRole::Follow,
                PlayRole::Follow,
            ],
            Kind::Pile => [
                PlayRole::Partner,
                PlayRole::Anchor,
                PlayRole::Partner,
                PlayRole::Partner,
            ],
            Kind::Dance => [
                PlayRole::Lead,
                PlayRole::Partner,
                PlayRole::Partner,
                PlayRole::Partner,
            ],
            // Whoever started it has the toy or is "it"; the others answer.
            Kind::KeepAway | Kind::Tag | Kind::JumpContest => [
                PlayRole::Lead,
                PlayRole::Follow,
                PlayRole::Follow,
                PlayRole::Follow,
            ],
            // The one who proposed it hides; the others count and then look.
            Kind::HideAndSeek => [
                PlayRole::Lead,
                PlayRole::Follow,
                PlayRole::Follow,
                PlayRole::Follow,
            ],
            // The companion ahead crouches; the one behind takes the first leap.
            Kind::Leapfrog | Kind::Tug => [
                PlayRole::Follow,
                PlayRole::Lead,
                PlayRole::Follow,
                PlayRole::Follow,
            ],
            _ => [PlayRole::Partner; MAX_PARTICIPANTS],
        };
        self.surface_memory.next_origin = self.surface_memory.next_origin.wrapping_add(1);
        let origin = Origin::Surface(self.surface_memory.next_origin);
        let members: Vec<_> = if e.kind.solo() {
            vec![e.pair[0]]
        } else {
            e.pair.to_vec()
        };
        for (index, id) in members.iter().copied().enumerate() {
            self.join_play(id, e.pair[1 - index.min(1)], origin, desktop);
        }
        self.recruit_attention_observers(e.pair[0], origin, desktop);
        for p in self.attention.plans.values_mut() {
            p.seconds = 9.0;
        }
        // The less composed of the pair blinks first, and a steadier pair stares for longer.
        let held: Vec<f32> = e
            .pair
            .iter()
            .map(|id| {
                let c = self.save.creatures.iter().find(|c| c.id == *id).unwrap();
                composure(c) + self.ambient_rng.random_range(0.0..0.08)
            })
            .collect();
        let loser = usize::from(held[1] < held[0]);
        self.attention.play.session = Some(Session {
            kind: e.kind,
            origin,
            members: if e.kind.solo() {
                [Some(e.pair[0]), None, None, None]
            } else {
                [Some(e.pair[0]), Some(e.pair[1]), None, None]
            },
            roles,
            count: if e.kind.solo() { 1 } else { 2 },
            elapsed: 0.0,
            turn: 1,
            turn_started: 0.0,
            ends_at: if e.kind == Kind::Stare {
                (3.5 + held[loser] * 3.5).clamp(3.5, 7.0)
            } else {
                e.kind.length()
            },
            loser,
            distracted: false,
            swaps: 0,
            seed: self.ambient_rng.random_range(0..u64::MAX),
            prop: matches!(e.kind, Kind::KeepAway | Kind::Tug).then_some(Prop {
                holder: e.pair[0],
                handoffs: 0,
                remaining: e.kind.length(),
            }),
            focus: matches!(e.kind, Kind::Hush | Kind::Prank).then_some(e.pair[1]),
            trail: [None; 4],
            goal: None,
        });
        self.attention.colony_cooldown = 15.0;
    }

    pub(super) fn join_play(
        &mut self,
        id: CreatureId,
        target: CreatureId,
        origin: Origin,
        desktop: &DesktopSnapshot,
    ) {
        let c = self.save.creatures.iter().find(|c| c.id == id).unwrap();
        let target = head_point(
            self.save.creatures.iter().find(|c| c.id == target).unwrap(),
            &self.save.settings,
            desktop,
        );
        let bounds = desktop
            .windows
            .iter()
            .find(|w| Some(w.key) == c.state.surface.window_key)
            .map(|w| w.bounds);
        let plan = Reaction {
            origin,
            role: Role::Play {
                stage: Stage::Notice,
                gesture: ActionKind::InspectScreen,
                bounds,
                hopping: false,
                pose: None,
            },
            target,
            emotion: AttentionEmotion::Curious,
            elapsed: 0.0,
            seconds: 9.0,
            delay: 0.0,
            travel_elapsed: 0.0,
            walk: None,
            display_walk: None,
            surface: (c.state.surface.monitor_id, c.state.surface.window_key),
            action: ActionKind::InspectScreen,
            cue: None,
            ride: None,
        };
        // Switching from spectator or follower to participant drops its earlier plan.
        self.bond_plans.remove(&id);
        self.begin_attention(id, plan);
    }

    fn finish_play(&mut self, session: Session) {
        let ids: Vec<_> = self
            .attention
            .plans
            .iter()
            .filter_map(|(&id, p)| (p.origin == session.origin).then_some(id))
            .collect();
        for id in ids {
            if let Some(p) = self.attention.plans.remove(&id)
                && let Some(c) = creature_mut(&mut self.save.creatures, id)
            {
                release(c, p);
            }
            self.attention.cooldowns.insert(id, 35.0);
        }
        self.attention.play.encounter = None;
        self.attention.play.cooldown = 45.0;
    }

    pub(super) fn advance_play(&mut self, desktop: &DesktopSnapshot, dt: f32) {
        let Some(mut s) = self.attention.play.session.take() else {
            return;
        };
        let turns = s.kind.hops();
        let valid = (!s.kind.travels() || !self.save.settings.reduce_motion)
            && s.members.iter().flatten().all(|id| {
                // A competitor between turns has no plan of its own for a moment.
                self.attention.plans.get(id).map_or(turns, |p| {
                    (matches!(p.role, Role::Play { .. }) || turns)
                        // A member standing in the lava keeps its plan just long enough for the
                        // round to end around it.
                        && (self.reaction_valid(*id, p, desktop)
                            || (s.kind == Kind::Lava
                                && self.save.creatures.iter().any(|c| {
                                    c.id == *id && c.state.surface.window_key.is_none()
                                })))
                }) && self
                    .save
                    .creatures
                    .iter()
                    .any(|c| c.id == *id && c.state.drives.energy > 0.25)
            });
        if !valid {
            self.finish_play(s);
            return;
        }
        s.elapsed += dt;
        if s.elapsed >= s.ends_at {
            self.finish_play(s);
            return;
        }
        self.settle_leaps();
        if matches!(s.kind, Kind::Copycat(_))
            && s.elapsed - s.turn_started >= 1.6
            && s.count < MAX_PARTICIPANTS
        {
            let previous = s.members[s.turn].unwrap();
            let actor = self
                .save
                .creatures
                .iter()
                .find(|c| c.id == previous)
                .unwrap();
            let candidate = self.attention.plans.iter().find_map(|(&id, p)| {
                let c = self.save.creatures.iter().find(|c| c.id == id)?;
                (matches!(p.role, Role::Observer { .. })
                    && p.origin == s.origin
                    && p.walk.is_none()
                    && c.personality.playfulness > 0.7
                    && self.play_available(c, desktop)
                    && self.play_pair(actor, c, desktop)
                    && !s.members.contains(&Some(id)))
                .then_some(id)
            });
            if let Some(id) = candidate {
                self.join_play(id, previous, s.origin, desktop);
                s.members[s.count] = Some(id);
                s.turn = s.count;
                s.count += 1;
                s.turn_started = s.elapsed;
                s.ends_at = (s.elapsed + 3.2).min(7.0);
            }
        }
        // Something happening nearby breaks the nearer contestant's concentration.
        if s.kind == Kind::Stare && !s.distracted && s.elapsed < s.ends_at - 1.4 {
            let cue = self
                .geometry_observer
                .signals()
                .map(|signal| signal.bounds)
                .next()
                .map(|bounds| (bounds, true))
                .or_else(|| {
                    self.cursor_observer.cue().map(|cue| {
                        (
                            DesktopRect {
                                x: cue.point.x,
                                y: cue.point.y,
                                width: 0.0,
                                height: 0.0,
                            },
                            false,
                        )
                    })
                });
            if let Some((bounds, _)) = cue
                && let Some((index, _)) = s
                    .members
                    .iter()
                    .enumerate()
                    .filter_map(|(index, id)| {
                        let c = self.save.creatures.iter().find(|c| Some(c.id) == *id)?;
                        Some((index, distance_to_rect(c.state.position, bounds)))
                    })
                    .filter(|(_, distance)| *distance <= 320.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
            {
                s.loser = index;
                s.distracted = true;
                s.ends_at = s.elapsed + 1.4;
            }
        }
        // Travelling games steer their own members; running out of room winds the scene down.
        if let Some(prop) = &mut s.prop {
            prop.remaining -= dt;
        }
        // A game restates each pose it wants every tick, so none outlives the moment it was for,
        // including a member the game has nothing more to say to as it winds down.
        for id in s.members.iter().flatten() {
            if let Some(plan) = self.attention.plans.get_mut(id)
                && let Role::Play { pose, .. } = &mut plan.role
            {
                *pose = None;
            }
        }
        let continuing = match s.kind {
            Kind::Chase => self.advance_chase(&mut s, desktop),
            Kind::Leapfrog => self.advance_leapfrog(&mut s, desktop),
            Kind::KeepAway => self.advance_keep_away(&mut s, desktop),
            Kind::Tug => self.advance_tug(&mut s, desktop, dt),
            Kind::Tag => self.advance_tag(&mut s, desktop),
            Kind::Hush => self.advance_hush(&mut s, desktop),
            Kind::FollowLeader => self.advance_follow_leader(&mut s, desktop, dt),
            Kind::Hill => self.advance_hill(&mut s, desktop),
            Kind::Prank => self.advance_prank(&mut s, desktop, dt),
            Kind::JumpContest => self.advance_jump_contest(&mut s, desktop),
            Kind::Race => self.advance_race(&mut s, desktop),
            Kind::Lava => self.advance_lava(&mut s, desktop),
            Kind::HideAndSeek => self.advance_hide_and_seek(&mut s, desktop),
            Kind::Parade => self.advance_parade(&mut s, desktop),
            Kind::Dance => self.advance_dance(&mut s, desktop),
            Kind::Pile => self.advance_pile(&mut s, desktop),
            _ => true,
        };
        if !continuing {
            s.ends_at = s.ends_at.min(s.elapsed + 1.4);
        }
        let quorum = usize::from(!s.kind.solo()) + 1;
        if s.members.iter().flatten().count() < quorum {
            self.finish_play(s);
            return;
        }
        s.turn = s.turn.min(MAX_PARTICIPANTS - 1);
        // What the audience is actually here for: whoever holds the plaything, else whoever is
        // leading — the runner, the one who is "it", the one holding the best spot — and only
        // then whichever member is taking its turn. All three change hands mid-scene, and the
        // gaze has to change with them or the watchers are looking at the wrong creature.
        let Some(focus) = s
            .prop
            .map(|prop| prop.holder)
            .or_else(|| s.with_role(s.kind.watched_role()).next().map(|(_, id)| id))
            .or(s.members[s.turn])
            .or(s.members[0])
        else {
            self.finish_play(s);
            return;
        };
        let focus_point = head_point(
            self.save.creatures.iter().find(|c| c.id == focus).unwrap(),
            &self.save.settings,
            desktop,
        );
        let recovery = s.elapsed >= s.ends_at - 1.4;
        // A long game must not outlive its own audience: a sixteen-second race whose watchers
        // were released at nine would be won in front of nobody. Everyone in the scene is kept
        // until it ends, and expires shortly after it rather than in the middle of it.
        for plan in self
            .attention
            .plans
            .values_mut()
            .filter(|p| p.origin == s.origin)
        {
            plan.seconds = plan.seconds.max(plan.elapsed + 1.5);
        }
        for (index, id) in s
            .members
            .iter()
            .enumerate()
            .filter_map(|(i, id)| id.map(|id| (i, id)))
        {
            // Gesture games look at each other; travelling ones already chose where to look.
            let target = match s.kind {
                Kind::Stare | Kind::Copycat(_) if s.kind == Kind::Stare || index == s.turn => {
                    let other = s.members[if index == 0 { 1 } else { index - 1 }].unwrap_or(focus);
                    head_point(
                        self.save.creatures.iter().find(|c| c.id == other).unwrap(),
                        &self.save.settings,
                        desktop,
                    )
                }
                Kind::Stare | Kind::Copycat(_) => focus_point,
                _ => self.attention.plans[&id].target,
            };
            let resting =
                self.save
                    .creatures
                    .iter()
                    .find(|c| c.id == id)
                    .map_or(ActionKind::Idle, |c| {
                        if c.state.surface.window_key.is_some() {
                            ActionKind::Perch
                        } else {
                            ActionKind::Idle
                        }
                    });
            let p = self.attention.plans.get_mut(&id).unwrap();
            let Role::Play {
                stage,
                gesture,
                pose,
                ..
            } = &mut p.role
            else {
                continue;
            };
            *stage = if recovery {
                Stage::Recover(Outcome::Completed)
            } else if s.elapsed - s.turn_started < 0.5 {
                Stage::Prepare
            } else {
                Stage::Act
            };
            *gesture = match s.kind {
                Kind::Copycat(action)
                    if index == s.turn && (0.5..1.6).contains(&(s.elapsed - s.turn_started)) =>
                {
                    action
                }
                Kind::Stare if recovery && index != s.loser => ActionKind::Greet,
                Kind::Copycat(_) | Kind::Stare => ActionKind::InspectScreen,
                // A travelling scene settles where it stands when it winds down, though whoever
                // ended up with the toy still has it.
                _ if recovery => {
                    if s.prop.is_some_and(|prop| prop.holder == id) {
                        ActionKind::PresentDiscovery
                    } else {
                        resting
                    }
                }
                _ => *gesture,
            };
            // Nothing steers a contest of looks or a copy chain, so their poses are struck here:
            // whoever blinks hides its eyes while the others cheer, and every imitation of the
            // move is the same unmistakable flourish, passed along the chain.
            match s.kind {
                Kind::Stare if recovery => {
                    *pose = Some(if index == s.loser {
                        Gesture::Cover
                    } else {
                        Gesture::Cheer
                    });
                }
                Kind::Copycat(action) if *gesture == action => {
                    *pose =
                        Some(COPIED_FLOURISHES[(s.seed % COPIED_FLOURISHES.len() as u64) as usize]);
                }
                _ => {}
            }
            if recovery {
                p.walk = None;
            }
            p.emotion = if s.kind == Kind::Stare && recovery && index == s.loser {
                AttentionEmotion::Averting
            } else if s.kind.expressive() {
                // How it feels about where it is standing is the scene, including how the round
                // ended; leave the game's own choice alone.
                p.emotion
            } else if recovery || *gesture != ActionKind::InspectScreen {
                AttentionEmotion::Enjoying
            } else {
                AttentionEmotion::Curious
            };
            p.target = target;
        }
        // Watchers of a contest look between the two, then at whoever breaks first.
        let breaker = s.members[s.loser].unwrap_or(focus);
        for (index, p) in self
            .attention
            .plans
            .values_mut()
            .filter(|p| p.origin == s.origin && matches!(p.role, Role::Observer { .. }))
            .enumerate()
        {
            p.role = Role::Observer {
                actor: if s.kind != Kind::Stare {
                    focus
                } else if recovery {
                    breaker
                } else {
                    s.members[((s.elapsed / 0.9) as usize + index) % 2].unwrap_or(focus)
                },
            };
        }
        self.attention.play.session = Some(s);
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(in super::super) fn scene(copy: bool) -> (World, DesktopSnapshot, OffsetDateTime) {
        let (mut world, desktop, now) = super::super::tests::scene();
        world.save.settings.display_scale = 2;
        world.clear_attention();
        world.attention.play.cooldown = 0.0;
        world.surface_memory.inspect_in = 100.0;
        for (i, c) in world.save.creatures.iter_mut().enumerate() {
            c.state.surface.window_key = Some(701);
            c.state.surface.kind = SurfaceKind::WindowLedge;
            c.state.surface.relative_x = (160.0 + i as f32 * 80.0) / 600.0;
            c.state.position = Point {
                x: 360.0 + i as f32 * 80.0,
                y: 600.0,
            };
            c.state.action = if i == 0 && copy {
                ActionKind::Greet
            } else {
                ActionKind::Perch
            };
            c.state.action_duration = 100.0;
            c.state.action_elapsed = 0.0;
            c.state.facing_right = i == 0;
            c.personality.boldness = 0.7;
            c.personality.curiosity = 0.8;
            c.personality.sociability = 0.9;
            c.personality.playfulness = if i == 3 { 0.0 } else { 1.0 };
            c.state.drives = Drives::default();
        }
        (world, desktop, now)
    }

    pub(in super::super) fn tick(
        w: &mut World,
        d: &mut DesktopSnapshot,
        now: OffsetDateTime,
        step: u64,
    ) {
        d.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
        w.tick(now + Duration::milliseconds(step as i64 * 50), 0.05, d);
    }

    /// What the audience of a scene actually did, gathered tick by tick: who watched, where each
    /// of them looked, how they felt about it, and anything a person at the screen would notice
    /// going wrong — a watcher quietly made into a player, a gaze resting on nobody, one scene
    /// told from two origins. Every game reads its audience through this, so "the watchers
    /// reacted" means the same thing in all of them.
    #[derive(Default, Debug)]
    pub(in super::super) struct Audience {
        /// Where each watcher looked, in whole points, with repeats collapsed.
        pub(in super::super) looked: BTreeMap<CreatureId, Vec<i32>>,
        /// How each watcher felt, with repeats collapsed.
        pub(in super::super) felt: BTreeMap<CreatureId, Vec<AttentionEmotion>>,
        /// The participants each watcher was given to follow, with repeats collapsed. Every scene
        /// hands its audience whoever opened it for the first tick, before the game itself names
        /// the one it wants watched, so two entries is a scene that never changed its mind.
        pub(in super::super) followed: BTreeMap<CreatureId, Vec<CreatureId>>,
        /// Watchers that saw the scene out with a greeting of their own.
        pub(in super::super) cheered: BTreeSet<CreatureId>,
        /// Watchers the scene later invited in, which is a promotion rather than a fault.
        pub(in super::super) joined: BTreeSet<CreatureId>,
        /// Watchers looking somewhere other than at the companion they were given.
        pub(in super::super) strayed: BTreeSet<CreatureId>,
        /// Watchers listed among the members while still holding a watcher's plan.
        pub(in super::super) conscripted: BTreeSet<CreatureId>,
        /// Watchers showing nothing at all once their own notice window had passed.
        pub(in super::super) blank: BTreeSet<CreatureId>,
        /// Anyone in the scene holding a plan from a different origin.
        pub(in super::super) split: BTreeSet<CreatureId>,
    }

    impl Audience {
        /// Take down what the audience of the running scene is doing this tick.
        pub(in super::super) fn note(&mut self, w: &World) {
            let Some(s) = w.attention.play.session else {
                return;
            };
            let members: Vec<CreatureId> = s.members.iter().flatten().copied().collect();
            for (&id, plan) in &w.attention.plans {
                let Some(creature) = w.save.creatures.iter().find(|c| c.id == id) else {
                    continue;
                };
                if plan.origin != s.origin {
                    self.split.insert(id);
                    continue;
                }
                let Role::Observer { actor } = plan.role else {
                    if self.looked.contains_key(&id) {
                        self.joined.insert(id);
                    }
                    continue;
                };
                if members.contains(&id) {
                    self.conscripted.insert(id);
                }
                note_change(self.followed.entry(id).or_default(), actor);
                if !members.contains(&actor) {
                    self.strayed.insert(id);
                }
                let Some(pose) = creature.state.attention else {
                    // A watcher shows nothing while it is still looking up; after that, an empty
                    // pose would mean nobody is watching at all.
                    if plan.elapsed - plan.delay - plan.travel_elapsed > 0.0 {
                        self.blank.insert(id);
                    }
                    continue;
                };
                note_change(self.looked.entry(id).or_default(), pose.target.x as i32);
                note_change(self.felt.entry(id).or_default(), pose.emotion);
                // The gaze is on the companion it was given, wherever that companion has got to.
                // A single tick of their running is worth a little slack, and a watcher covering
                // its eyes turns that same gaze around rather than choosing someone else.
                let aimed = w.save.creatures.iter().any(|other| {
                    let mirrored = 2.0 * creature.state.position.x - pose.target.x;
                    other.id == actor
                        && ((other.state.position.x - pose.target.x).abs() <= 16.0
                            || (other.state.position.x - mirrored).abs() <= 16.0)
                });
                if !aimed {
                    self.strayed.insert(id);
                }
                if creature.state.action == ActionKind::Greet {
                    self.cheered.insert(id);
                }
            }
        }

        /// The watchers that sat the whole scene out, in creature order.
        pub(in super::super) fn watchers(&self) -> Vec<CreatureId> {
            self.looked
                .keys()
                .filter(|id| !self.joined.contains(id))
                .copied()
                .collect()
        }

        /// Where one watcher looked, or a failure saying the scene had no audience at all.
        pub(in super::super) fn looked_at(&self, id: CreatureId) -> &[i32] {
            self.looked
                .get(&id)
                .unwrap_or_else(|| panic!("{id} watched nothing: {self:?}"))
        }

        /// How one watcher felt, in order, or a failure saying nobody was watching.
        pub(in super::super) fn felt_by(&self, id: CreatureId) -> &[AttentionEmotion] {
            self.felt
                .get(&id)
                .unwrap_or_else(|| panic!("{id} felt nothing: {self:?}"))
        }

        /// Nothing the audience did was out of order: no stray gaze, no watcher quietly made a
        /// player, no blank stare, and one origin for the whole scene.
        pub(in super::super) fn coherent(&self) -> bool {
            self.strayed.is_empty()
                && self.conscripted.is_empty()
                && self.blank.is_empty()
                && self.split.is_empty()
        }
    }

    /// Keep a value only when it differs from the last, so a sequence reads as what changed.
    fn note_change<T: PartialEq>(seen: &mut Vec<T>, value: T) {
        if seen.last() != Some(&value) {
            seen.push(value);
        }
    }

    /// Nobody is left holding a pose, a plan, or a half-finished reaction from a scene that is
    /// over.
    pub(in super::super) fn everyone_settled(w: &World) -> bool {
        w.save.creatures.iter().all(|c| c.state.attention.is_none())
            && !w
                .attention
                .plans
                .values()
                .any(|p| matches!(p.role, Role::Play { .. } | Role::Observer { .. }))
    }

    #[test]
    fn ordinary_gesture_becomes_a_finite_copy_chain_with_a_remaining_spectator() {
        let (mut w, mut d, now) = scene(true);
        let mut actors = BTreeSet::new();
        let mut copied = BTreeSet::new();
        let mut started = false;
        let mut finished = false;
        for step in 1..180 {
            tick(&mut w, &mut d, now, step);
            if let Some(s) = w.attention.play.session {
                started = true;
                assert!(s.count <= 4);
                assert!(w.attention.plans.values().all(|p| p.origin == s.origin));
                for (&id, p) in &w.attention.plans {
                    if let Role::Play { gesture, .. } = p.role {
                        actors.insert(id);
                        if gesture == ActionKind::Greet {
                            copied.insert(id);
                        }
                    }
                }
                assert!(matches!(
                    w.attention.plans[&w.save.creatures[3].id].role,
                    Role::Observer { .. }
                ));
            } else if started {
                finished = true;
                break;
            }
        }
        assert!(started && finished);
        assert_eq!(actors.len(), 3);
        assert_eq!(copied.len(), 2);
        assert!(
            !copied.contains(&w.save.creatures[0].id),
            "the original gesture must not restart itself"
        );
        assert!(w.attention.plans.is_empty());
        assert!(w.attention.play.cooldown > 40.0);
        assert!(std::mem::size_of::<PlayRuntime>() <= 512);
        let restored = World::from_save(
            serde_json::from_str(&serde_json::to_string(&w.save).unwrap()).unwrap(),
        );
        assert!(restored.attention.play.session.is_none());
        assert!(restored.attention.play.encounter.is_none());
    }

    #[test]
    fn sustained_mutual_gaze_starts_a_contest_and_someone_breaks_eye_contact() {
        let (mut w, mut d, now) = scene(false);
        for step in 1..40 {
            tick(&mut w, &mut d, now, step);
        }
        assert!(w.attention.play.session.is_none(), "a glance is not a game");
        let mut started = false;
        let mut blinked = false;
        for step in 40..200 {
            tick(&mut w, &mut d, now, step);
            started |= w
                .attention
                .play
                .session
                .is_some_and(|s| s.kind == Kind::Stare);
            blinked |= w.save.creatures.iter().any(|c| {
                c.state
                    .attention
                    .is_some_and(|p| p.emotion == AttentionEmotion::Averting)
            });
        }
        assert!(started && blinked);
        assert!(w.attention.play.session.is_none());
    }

    #[test]
    fn the_less_composed_contestant_blinks_and_watchers_look_between_the_pair() {
        let (mut w, mut d, now) = scene(false);
        // A sleepy, low-energy creature holds a look less well than a rested one.
        w.save.creatures[1].state.drives.energy = 0.5;
        w.save.creatures[1].state.drives.sleep_pressure = 0.6;
        let (steady, blinker) = (w.save.creatures[0].id, w.save.creatures[1].id);
        let mut watched = BTreeSet::new();
        let mut averted = Vec::new();
        let mut watched_breaker = false;
        for step in 1..220 {
            tick(&mut w, &mut d, now, step);
            let Some(session) = w.attention.play.session else {
                continue;
            };
            assert_eq!(session.members[session.loser], Some(blinker));
            if let Some(pose) = w.save.creatures[2].state.attention {
                watched.insert(pose.target.x as i32);
                watched_breaker |= session.elapsed >= session.ends_at - 1.4
                    && pose.target.x as i32 == w.save.creatures[1].state.position.x as i32;
            }
            for c in &w.save.creatures[..2] {
                if c.state
                    .attention
                    .is_some_and(|p| p.emotion == AttentionEmotion::Averting)
                {
                    averted.push(c.id);
                }
            }
        }
        assert!(averted.iter().all(|id| *id == blinker) && !averted.is_empty());
        assert!(!averted.contains(&steady));
        assert!(watched.len() >= 2, "watchers look between both contestants");
        assert!(watched_breaker, "and at whoever breaks first");
    }

    #[test]
    fn something_happening_nearby_ends_a_contest_early() {
        let (mut w, mut d, now) = scene(false);
        let mut neighbor = d.windows[0].clone();
        neighbor.key = 702;
        neighbor.z_order = 1;
        neighbor.bounds = DesktopRect {
            x: 200.0,
            y: 500.0,
            width: 100.0,
            height: 200.0,
        };
        d.windows.push(neighbor);
        let mut started_at = None;
        let mut ended_at = None;
        for step in 1..260 {
            if w.attention.play.session.is_some() && started_at.is_none() {
                started_at = Some(step);
                // A window shifts beside the creature on the left, in plain sight.
                d.windows[1].bounds.x += 30.0;
            }
            tick(&mut w, &mut d, now, step);
            if let Some(session) = w.attention.play.session {
                if session.distracted {
                    assert_eq!(session.members[session.loser], Some(w.save.creatures[0].id));
                    assert!(session.ends_at <= session.elapsed + 1.4);
                }
            } else if started_at.is_some() && ended_at.is_none() {
                ended_at = Some(step);
            }
        }
        let (started, ended) = (started_at.unwrap(), ended_at.unwrap());
        assert!(
            ended - started < 60,
            "a distracted contest ends well before its full length"
        );
        assert!(
            !w.attention
                .plans
                .values()
                .any(|p| matches!(p.role, Role::Play { .. }))
        );
    }

    #[test]
    fn interruption_releases_every_role_and_cannot_record_a_completed_game() {
        for mode in 0..5 {
            let (mut w, mut d, now) = scene(true);
            for step in 1..20 {
                tick(&mut w, &mut d, now, step);
            }
            assert!(w.attention.play.session.is_some());
            w.drain_events().for_each(drop);
            match mode {
                0 => w.save.settings.paused = true,
                1 => w.save.settings.visible = false,
                2 => w.save.creatures[0].state.drives.sleep_pressure = 0.95,
                3 => d.windows[0].bounds.x += 10.0,
                _ => d.window_sample.as_mut().unwrap().reliable = false,
            }
            tick(&mut w, &mut d, now, 20);
            assert!(w.attention.play.session.is_none(), "mode {mode}");
            assert!(
                !w.attention
                    .plans
                    .values()
                    .any(|p| matches!(p.role, Role::Play { .. } | Role::Observer { .. }))
            );
            assert!(!w.drain_events().any(|e| matches!(
                e,
                WorldEvent::ActionCompleted { .. } | WorldEvent::BondInteraction { .. }
            )));
        }
    }

    #[test]
    fn reduced_motion_keeps_the_social_response_planted_and_calm() {
        let (mut w, mut d, now) = scene(true);
        w.save.settings.reduce_motion = true;
        let mut positions = None;
        let mut started = false;
        for step in 1..120 {
            tick(&mut w, &mut d, now, step);
            if w.attention.play.session.is_some() {
                started = true;
                let positions = positions.get_or_insert_with(|| {
                    w.save
                        .creatures
                        .iter()
                        .map(|c| c.state.position)
                        .collect::<Vec<_>>()
                });
                for (i, c) in w.save.creatures.iter().enumerate() {
                    assert_eq!(c.state.position, positions[i]);
                    assert_eq!(c.state.action, ActionKind::InspectScreen);
                }
            }
        }
        assert!(started);
    }

    #[test]
    fn solitary_gestures_and_avoidant_pairs_do_not_invent_a_partner() {
        for solitary in [false, true] {
            let (mut w, mut d, now) = scene(true);
            if solitary {
                w.save.creatures.truncate(1);
            } else {
                for r in &mut w.save.relationships {
                    r.avoidance = 255;
                }
            }
            for step in 1..80 {
                tick(&mut w, &mut d, now, step);
            }
            assert!(w.attention.play.session.is_none());
        }
    }

    /// Nothing steers a contest of looks or a copy chain, so their poses come with the scene
    /// itself: whoever blinks first hides its eyes while the rest cheer, and every creature that
    /// takes up the copied move strikes the same unmistakable flourish, which is what makes a
    /// chain of imitations read as one.
    #[test]
    fn a_contest_of_looks_and_a_copy_chain_strike_their_own_poses() {
        let (mut w, mut d, now) = scene(false);
        let mut poses = super::super::tests::Poses::default();
        let mut loser = None;
        for step in 1..220 {
            poses.tick(&mut w, |w| tick(w, &mut d, now, step));
            if let Some(s) = w.attention.play.session.filter(|s| s.kind == Kind::Stare) {
                loser = loser.or(s.members[s.loser]);
            }
        }
        let loser = loser.expect("a contest of looks needs someone to blink");
        assert_eq!(poses.by(loser), [Gesture::Cover], "{poses:?}");
        assert!(
            poses
                .struck
                .iter()
                .any(|(id, struck)| *id != loser && struck.contains(&Gesture::Cheer)),
            "nobody enjoyed winning: {poses:?}"
        );

        let (mut w, mut d, now) = scene(true);
        let mut poses = super::super::tests::Poses::default();
        for step in 1..220 {
            poses.tick(&mut w, |w| tick(w, &mut d, now, step));
        }
        let copied: Vec<_> = poses.struck.values().flatten().copied().collect();
        assert!(copied.len() >= 2, "the move was never passed on: {poses:?}");
        assert!(
            COPIED_FLOURISHES.contains(&copied[0]) && copied.iter().all(|pose| *pose == copied[0]),
            "a copy chain is one move, not several: {poses:?}"
        );
    }
}
