//! Wonders turning up, being gone to, played on, and going again.
//!
//! Every ten to twenty minutes of visible time something to play on appears somewhere a companion
//! can get to, and whoever it turned up for heads straight over: along the ledge or the floor it
//! is already on, down off a window, or up onto one by the same hop and climb the colony always
//! uses. While the houses are out it only ever turns up on the village's own ground, so the
//! screen stays as calm as the village means it to be. A wonder for two sends both at once.
//!
//! Once there, each kind is a short script of where its players stand or sit relative to it and
//! what pose they strike: a crouch before a hop up, a seat, a ride along and back, a wobble on the
//! rope, a cheer at the end, a hop back down. The first time a kind turns up its finder stops to
//! marvel at it first, and the notebook writes it down; that, and a count of goes, is all a save
//! ever keeps. Picking a player up, on the way or in the middle of it, and anything else that
//! would leave the wonder somewhere it no longer belongs, sends it away at once.
//!
//! The wonder owns its players' feet while it lasts, exactly as a plan about the village does:
//! the colony's ordinary choices pass them by, and every other scene, beat and antic leaves them
//! be. All of it runs on a stream of its own, so a wonder never shifts any other choice.

use super::attention::point_exposed;
use super::*;
use crate::tuning::WONDERS;
use crate::wonders::{WonderKind, WonderRecord, WonderSeats, WonderView};

/// Runtime state: when the next wonder is due, and the one out now.
pub(super) struct Wonders {
    next_in: f32,
    rng: ChaCha12Rng,
    active: Option<Active>,
    /// One of its players was picked up: the rest are let go on the next tick, when there is a
    /// desktop to put them down on.
    grabbed: bool,
    /// The kind the next one must be, for tests that need a particular wonder.
    #[cfg(test)]
    forced: Option<WonderKind>,
}

impl Wonders {
    pub(super) fn new(streams: &SeedStream) -> Self {
        let mut rng = streams.rng("wonders", 0);
        let next_in = rng.random_range(WONDERS.interval_secs);
        Self {
            next_in,
            rng,
            active: None,
            grabbed: false,
            #[cfg(test)]
            forced: None,
        }
    }

    /// One of its players has been picked up. That one is the hand's now; the wonder goes on the
    /// next tick and takes the rest of its players' plans with it.
    ///
    /// Gives the ground it was on, if it had got there, so whoever picked it up from a seat or a
    /// rope and simply let go again finds it back on its feet beside the place the wonder was.
    pub(super) fn grab(&mut self, id: CreatureId, x: f32) -> Option<(f32, SurfaceAttachment)> {
        let active = self.active.as_mut().filter(|active| !active.leaving())?;
        let index = active.players.iter().position(|player| player.id == id)?;
        let player = active.players.remove(index);
        self.grabbed = true;
        let on_ground = player.arrived
            || player
                .legs
                .front()
                .is_some_and(|leg| matches!(leg, Leg::Walk));
        on_ground.then(|| (active.at.y, active.ground.surface(x)))
    }

    /// Whether this companion is one of the players of the wonder out now.
    pub(super) fn owns(&self, id: CreatureId) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| !active.leaving() && active.players.iter().any(|p| p.id == id))
    }

    /// Both players of the same wonder, held together on purpose.
    pub(super) fn together(&self, a: CreatureId, b: CreatureId) -> bool {
        self.owns(a) && self.owns(b)
    }

    pub(super) fn players(&self) -> impl Iterator<Item = CreatureId> + '_ {
        self.active
            .iter()
            .filter(|active| !active.leaving())
            .flat_map(|active| active.players.iter().map(|player| player.id))
    }

    #[cfg(test)]
    pub(super) fn due_now(&mut self, kind: Option<WonderKind>) {
        self.next_in = 0.0;
        self.forced = kind;
    }

    #[cfg(test)]
    pub(super) fn hold_off(&mut self) {
        self.next_in = f32::MAX;
    }

    #[cfg(test)]
    pub(super) fn leaving(&self) -> bool {
        self.active.as_ref().is_some_and(Active::leaving)
    }

    #[cfg(test)]
    pub(super) fn player_ids(&self) -> Vec<CreatureId> {
        self.players().collect()
    }
}

/// The ground a wonder stands on.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ground {
    Floor {
        monitor: MonitorId,
    },
    Ledge {
        monitor: MonitorId,
        window: WindowKey,
        bounds: DesktopRect,
    },
    Village {
        monitor: MonitorId,
    },
}

impl Ground {
    const fn monitor(self) -> MonitorId {
        match self {
            Self::Floor { monitor } | Self::Ledge { monitor, .. } | Self::Village { monitor } => {
                monitor
            }
        }
    }

    const fn window(self) -> Option<WindowKey> {
        match self {
            Self::Ledge { window, .. } => Some(window),
            _ => None,
        }
    }

    fn surface(self, x: f32) -> SurfaceAttachment {
        match self {
            Self::Ledge {
                monitor,
                window,
                bounds,
            } => SurfaceAttachment {
                kind: SurfaceKind::WindowLedge,
                monitor_id: monitor,
                window_key: Some(window),
                relative_x: ((x - bounds.x) / bounds.width).clamp(0.05, 0.95),
            },
            Self::Floor { monitor } | Self::Village { monitor } => SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id: monitor,
                window_key: None,
                relative_x: 0.5,
            },
        }
    }

    /// Whether the creature is already standing on this ground.
    fn holds(self, creature: &Creature) -> bool {
        let surface = &creature.state.surface;
        match self {
            Self::Ledge { window, .. } => surface.window_key == Some(window),
            Self::Floor { monitor } => {
                surface.window_key.is_none()
                    && surface.kind == SurfaceKind::ScreenFloor
                    && surface.monitor_id == monitor
            }
            // Only those already down on the village's own display: a resident still on its
            // way home from another one is not on this ground yet, however close it looks.
            Self::Village { monitor } => {
                surface.window_key.is_none() && surface.monitor_id == monitor
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Role {
    Lead,
    Partner,
}

/// One part of the way over.
enum Leg {
    Journey(WindowJourney),
    Walk,
}

struct Player {
    id: CreatureId,
    role: Role,
    legs: VecDeque<Leg>,
    /// Where it stands to begin, on the wonder's own ground.
    start: Point,
    arrived: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// On the way over.
    Gathering,
    /// The finder of a new kind, marvelling at it.
    Marvel(f32),
    /// Playing on it, this many seconds in.
    Playing(f32),
    /// Going, this many seconds into going. Its players are their own again.
    Leaving(f32),
}

struct Active {
    kind: WonderKind,
    ground: Ground,
    /// The middle of where it stands, on the ground.
    at: Point,
    /// Art pixels to points here.
    unit: f32,
    /// Turned round: every script plays mirrored.
    mirrored: bool,
    players: Vec<Player>,
    phase: Phase,
    /// Seconds since it appeared.
    elapsed: f32,
    /// Seconds spent gathering.
    waited: f32,
    /// How long its script lasts.
    length: f32,
    /// A coin its script tosses: which way the arrow ends up, who wins at cards.
    toss: bool,
    /// Whether this is the colony's first of the kind.
    first: bool,
}

impl Active {
    const fn leaving(&self) -> bool {
        matches!(self.phase, Phase::Leaving(_))
    }

    fn presence(&self) -> f32 {
        match self.phase {
            Phase::Leaving(t) => (1.0 - t / WONDERS.vanish_secs).clamp(0.0, 1.0),
            _ => (self.elapsed / WONDERS.appear_secs).clamp(0.0, 1.0),
        }
    }

    fn side(&self) -> f32 {
        if self.mirrored { -1.0 } else { 1.0 }
    }

    /// An offset in the script's art pixels, as a point on the desktop.
    fn place(&self, dx: f32, dy: f32) -> Point {
        Point {
            x: self.at.x + dx * self.side() * self.unit,
            y: self.at.y + dy * self.unit,
        }
    }

    fn script_time(&self) -> Option<f32> {
        match self.phase {
            Phase::Playing(t) => Some(t),
            _ => None,
        }
    }
}

/// Where a player is and what it is doing at one moment of a wonder's script, in art pixels from
/// the middle of the wonder, as if it were not turned round.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Stance {
    pub dx: f32,
    pub dy: f32,
    pub facing_right: bool,
    pub action: ActionKind,
    pub gesture: Option<Gesture>,
    pub emotion: AttentionEmotion,
    /// Where it looks, in the same art pixels, if anywhere in particular.
    pub look: Option<(f32, f32)>,
}

impl Stance {
    pub(super) const fn at(dx: f32, dy: f32, facing_right: bool) -> Self {
        Self {
            dx,
            dy,
            facing_right,
            action: ActionKind::Idle,
            gesture: None,
            emotion: AttentionEmotion::Enjoying,
            look: None,
        }
    }

    pub(super) const fn pose(mut self, gesture: Gesture) -> Self {
        self.gesture = Some(gesture);
        self
    }

    pub(super) const fn doing(mut self, action: ActionKind) -> Self {
        self.action = action;
        self
    }

    pub(super) const fn feeling(mut self, emotion: AttentionEmotion) -> Self {
        self.emotion = emotion;
        self
    }

    pub(super) const fn looking(mut self, dx: f32, dy: f32) -> Self {
        self.look = Some((dx, dy));
        self
    }
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A crouch to get ready, then a hop from one place to another over a little arc, as a stance at
/// `t` seconds into it; `None` once it has landed. Every hop on or off a wonder is one of these,
/// so getting on always reads as getting on rather than appearing in the seat.
pub(super) fn hop(t: f32, from: (f32, f32), to: (f32, f32), facing_right: bool) -> Option<Stance> {
    const CROUCH: f32 = 0.28;
    const AIR: f32 = 0.42;
    if t < CROUCH {
        return Some(Stance::at(from.0, from.1, facing_right).pose(Gesture::Crouch));
    }
    let p = (t - CROUCH) / AIR;
    if p >= 1.0 {
        return None;
    }
    let lift = (p * std::f32::consts::PI).sin() * (6.0 + (to.1 - from.1).abs() * 0.4);
    Some(
        Stance::at(
            mix(from.0, to.0, p),
            mix(from.1, to.1, ease(p)) - lift,
            facing_right,
        )
        .doing(ActionKind::Landing),
    )
}

/// How long the crouch and the hop together take.
pub(super) const HOP_SECS: f32 = 0.7;

/// How far the moving part of a wonder has moved along, in art pixels: a sled's ride. Zero for
/// everything that stays where it was put.
pub(super) fn carried(kind: WonderKind, t: Option<f32>, length: f32) -> f32 {
    let Some(t) = t else { return 0.0 };
    match kind {
        WonderKind::LeafSled => {
            let ride = t - HOP_SECS;
            let out = (length - 2.0 * HOP_SECS - 0.8) / 2.0;
            if ride < 0.0 {
                0.0
            } else if ride < out {
                64.0 * ease(ride / out)
            } else if ride < out + 0.8 {
                64.0
            } else {
                64.0 * (1.0 - ease((ride - out - 0.8) / out))
            }
        }
        _ => 0.0,
    }
}

/// What a wonder's own moving part is doing at `t` seconds into its script, from -1 to 1.
pub(super) fn motion(kind: WonderKind, t: Option<f32>, length: f32, toss: bool) -> f32 {
    let Some(t) = t else { return 0.0 };
    let wind = |at: f32| (at * std::f32::consts::TAU).sin();
    match kind {
        WonderKind::Seesaw => {
            let on = HOP_SECS + 0.2;
            let off = length - HOP_SECS - 0.4;
            if t < on || t > off {
                0.0
            } else {
                let fade = ((t - on) / 0.6).min((off - t) / 0.6).clamp(0.0, 1.0);
                wind((t - on) / 1.6) * fade
            }
        }
        WonderKind::Hammock => {
            if t < HOP_SECS {
                0.0
            } else {
                wind((t - HOP_SECS) / 3.2) * ((t - HOP_SECS) / 1.5).min(1.0)
            }
        }
        // The arrow spins from 2.2 seconds to 4.2 and settles pointing one way or the other.
        WonderKind::ArrowSign => {
            let end = if toss { 1.0 } else { -1.0 };
            if t < 2.2 {
                1.0
            } else if t < 4.2 {
                let p = (t - 2.2) / 2.0;
                // Several turns, slowing, and then the way it settles.
                let turns = (1.0 - (1.0 - p).powi(2)) * 3.5;
                let swing = (turns * std::f32::consts::PI).cos();
                mix(swing, end, ease((p - 0.7) / 0.3))
            } else {
                end
            }
        }
        WonderKind::Fountain => {
            if (3.0..9.0).contains(&t) {
                (wind((t - 3.0) / 0.8) * 0.5 + 0.5).powi(2)
            } else {
                0.0
            }
        }
        WonderKind::Tightrope => {
            let walk = t - HOP_SECS;
            let middle = walk - 4.0;
            if (0.0..1.4).contains(&middle) {
                wind(middle / 0.45) * (1.0 - middle / 1.4)
            } else if walk > 0.0 && walk < 9.0 {
                wind(walk / 1.1) * 0.2
            } else {
                0.0
            }
        }
        WonderKind::LeafSled => {
            // Sliding: forward on the way out, back on the way home.
            let a = carried(kind, Some(t), length);
            let b = carried(kind, Some(t + 0.05), length);
            ((b - a) * 4.0).clamp(-1.0, 1.0)
        }
        // How many cards are down, in quarters: one more every turn, gathered up at the end.
        WonderKind::StumpTable => {
            if t > HOP_SECS && t < length - HOP_SECS - 1.5 {
                (((t - HOP_SECS) / 1.6).floor() % 4.0 + 1.0) / 4.0
            } else {
                0.0
            }
        }
        WonderKind::Chair | WonderKind::BookStack => 0.0,
    }
}

/// How long each wonder's script lasts.
pub(super) const fn script_length(kind: WonderKind) -> f32 {
    match kind {
        WonderKind::Chair => 13.0,
        WonderKind::LeafSled => 13.0,
        WonderKind::Fountain => 13.0,
        WonderKind::Tightrope => 14.0,
        WonderKind::StumpTable => 15.0,
        WonderKind::ArrowSign => 9.0,
        WonderKind::Hammock => 20.0,
        WonderKind::BookStack => 14.0,
        WonderKind::Seesaw => 14.0,
    }
}

/// Where each player begins, beside the wonder on its ground, in art pixels from its middle.
pub(super) const fn start_offset(kind: WonderKind, role: Role) -> f32 {
    let lead = matches!(role, Role::Lead);
    match kind {
        WonderKind::Chair => -22.0,
        WonderKind::LeafSled => -20.0,
        WonderKind::Fountain => {
            if lead {
                -30.0
            } else {
                30.0
            }
        }
        WonderKind::Tightrope => {
            if lead {
                -56.0
            } else {
                -76.0
            }
        }
        WonderKind::StumpTable => {
            if lead {
                -44.0
            } else {
                44.0
            }
        }
        WonderKind::ArrowSign => {
            if lead {
                -18.0
            } else {
                -42.0
            }
        }
        WonderKind::Hammock => -26.0,
        WonderKind::BookStack => {
            if lead {
                -20.0
            } else {
                22.0
            }
        }
        WonderKind::Seesaw => {
            if lead {
                -50.0
            } else {
                50.0
            }
        }
    }
}

/// Where a player is and what it does `t` seconds into a wonder's script, `length` seconds long.
/// `pair` is whether it has a partner; `toss` is the script's own coin.
#[allow(clippy::too_many_lines)]
pub(super) fn stance(
    kind: WonderKind,
    role: Role,
    pair: bool,
    t: f32,
    length: f32,
    toss: bool,
) -> Stance {
    let start = start_offset(kind, role);
    let lead = role == Role::Lead;
    let off = length - HOP_SECS;
    match kind {
        WonderKind::Chair => {
            let seat = (0.0, -10.0);
            if let Some(stance) = hop(t, (start, 0.0), seat, true) {
                return stance;
            }
            if t >= off {
                return hop(t - off, seat, (22.0, 0.0), true)
                    .unwrap_or_else(|| Stance::at(22.0, 0.0, true));
            }
            let sat = t - HOP_SECS;
            // Settled in, a look round, a lean back with its eyes shut for a moment, and up.
            let base = Stance::at(seat.0, seat.1, true).pose(Gesture::Sit);
            if (5.0..7.5).contains(&sat) {
                base.looking(-40.0, -20.0)
                    .feeling(AttentionEmotion::Curious)
            } else if (8.0..10.0).contains(&sat) {
                base.feeling(AttentionEmotion::Relieved)
            } else {
                base
            }
        }
        // Sat down low in the curl of the leaf, scooting it along by paw and back again.
        WonderKind::LeafSled => {
            let seat = (0.0, -5.0);
            if let Some(stance) = hop(t, (start, 0.0), seat, true) {
                return stance;
            }
            let ride = carried(kind, Some(t), length);
            if t >= off {
                return hop(t - off, seat, (22.0, 0.0), true)
                    .unwrap_or_else(|| Stance::at(22.0, 0.0, true));
            }
            let going_out = carried(kind, Some(t + 0.05), length) >= ride;
            Stance::at(seat.0 + ride, seat.1, going_out).pose(Gesture::Scoot)
        }
        WonderKind::Fountain => {
            let facing_right = lead;
            let at = Stance::at(start, 0.0, facing_right).looking(0.0, -24.0);
            // Partners answer a beat behind.
            let t = if lead { t } else { t - 0.4 };
            if t < 1.2 {
                at.pose(Gesture::Watch).feeling(AttentionEmotion::Curious)
            } else if t < 3.0 {
                at.pose(Gesture::Reach)
            } else if t < 9.0 {
                // Splashing: a bop, and every so often both paws up in it.
                if ((t - 3.0) / 1.6).fract() < 0.7 {
                    at.pose(Gesture::Bop)
                } else {
                    at.pose(Gesture::Cheer)
                }
            } else if t < 10.0 && pair && !lead {
                at.pose(Gesture::Gasp).feeling(AttentionEmotion::Startled)
            } else if t < length - 1.0 {
                at.pose(Gesture::Cheer)
            } else {
                at
            }
        }
        WonderKind::Tightrope => {
            let post = (-44.0, -25.0);
            let far = (44.0, -25.0);
            if !lead {
                // Watching from the end, worried at the wobble, cheering at the far post.
                let walk = t - HOP_SECS;
                let at = Stance::at(start, 0.0, true);
                let lead_x = rope_x(walk);
                let watching = at.looking(lead_x, -40.0);
                return if (4.0..5.4).contains(&walk) {
                    watching
                        .pose(Gesture::Worry)
                        .feeling(AttentionEmotion::Concerned)
                } else if walk >= 9.0 && t < off {
                    watching.pose(Gesture::Cheer)
                } else {
                    watching
                        .pose(Gesture::Watch)
                        .feeling(AttentionEmotion::Curious)
                };
            }
            if let Some(stance) = hop(t, (start, 0.0), post, true) {
                return stance;
            }
            if t >= off {
                return hop(t - off, far, (56.0, 0.0), true)
                    .unwrap_or_else(|| Stance::at(56.0, 0.0, true));
            }
            let walk = t - HOP_SECS;
            if walk >= 9.0 {
                return Stance::at(far.0, far.1, true).pose(Gesture::Cheer);
            }
            let x = rope_x(walk);
            let sag = 3.0 * (1.0 - (x / 44.0).powi(2));
            let wobble = motion(kind, Some(t), length, toss);
            let stance = Stance::at(x + wobble * 1.5, -24.0 + sag, true).pose(Gesture::Balance);
            if (4.0..5.4).contains(&walk) {
                stance.feeling(AttentionEmotion::Concerned)
            } else {
                stance
            }
        }
        WonderKind::StumpTable => {
            // Low stools out beside the table: sat on with the feet near the ground, and far
            // enough out that the table stands clear between the two players.
            let seat = (if lead { -28.0 } else { 28.0 }, -4.0);
            let facing_right = lead;
            if let Some(stance) = hop(t, (start, 0.0), seat, facing_right) {
                return stance;
            }
            if t >= off {
                return hop(t - off, seat, (start, 0.0), !facing_right)
                    .unwrap_or_else(|| Stance::at(start, 0.0, facing_right));
            }
            let sat = t - HOP_SECS;
            let base = Stance::at(seat.0, seat.1, facing_right)
                .pose(Gesture::Sit)
                .looking(0.0, -12.0);
            let end = length - 2.0 * HOP_SECS - 1.5;
            if sat >= end {
                // Whoever the coin chose wins and cheers; the other one gives a cheer too.
                let won = lead == toss || !pair;
                return if won || sat >= end + 0.5 {
                    base.pose(Gesture::Cheer)
                } else {
                    base.feeling(AttentionEmotion::Startled)
                };
            }
            // Turns at the cards: a reach across to lay one down, then a look at the table.
            let turn = (sat / 1.6).floor() as i32;
            let mine = !pair || (turn % 2 == 0) == lead;
            if mine && (sat / 1.6).fract() < 0.55 {
                base.pose(Gesture::Reach)
            } else {
                base.feeling(AttentionEmotion::Curious)
            }
        }
        WonderKind::ArrowSign => {
            let arrow = (0.0, -38.0);
            let way = if toss { 1.0 } else { -1.0 };
            let at = Stance::at(start, 0.0, true);
            if !lead {
                return if t < 4.4 {
                    at.looking(arrow.0, arrow.1)
                        .pose(Gesture::Watch)
                        .feeling(AttentionEmotion::Curious)
                } else if t < length - 1.2 {
                    at.looking(way * 400.0, -20.0)
                        .pose(Gesture::Watch)
                        .feeling(AttentionEmotion::Curious)
                } else {
                    at.pose(Gesture::Cheer)
                };
            }
            if t < 1.5 {
                at.looking(arrow.0, arrow.1)
                    .pose(Gesture::Watch)
                    .feeling(AttentionEmotion::Curious)
            } else if t < 2.2 {
                at.looking(arrow.0, arrow.1).pose(Gesture::Reach)
            } else if t < 4.4 {
                at.looking(arrow.0, arrow.1)
                    .pose(Gesture::Watch)
                    .feeling(AttentionEmotion::Startled)
            } else if t < length - 1.2 {
                Stance::at(start, 0.0, way > 0.0)
                    .looking(way * 400.0, -20.0)
                    .pose(Gesture::Reach)
            } else {
                Stance::at(start, 0.0, way > 0.0).pose(Gesture::Cheer)
            }
        }
        WonderKind::Hammock => {
            let bed = (0.0, -10.0);
            if let Some(stance) = hop(t, (start, 0.0), bed, true) {
                return stance;
            }
            if t >= off {
                return hop(t - off, bed, (26.0, 0.0), true)
                    .unwrap_or_else(|| Stance::at(26.0, 0.0, true).pose(Gesture::Stretch));
            }
            let sway = motion(kind, Some(t), length, toss);
            Stance::at(bed.0 + sway * 3.0, bed.1, true).doing(ActionKind::Sleep)
        }
        WonderKind::BookStack => {
            let step = (-11.0, -16.0);
            let top = (0.0, -21.0);
            if !lead {
                // Peeking out from behind the stack, until it is spotted.
                let at = Stance::at(start, 0.0, false).looking(0.0, -30.0);
                return if t < 2.0 * HOP_SECS {
                    at.pose(Gesture::Watch).feeling(AttentionEmotion::Curious)
                } else if t < 8.0 {
                    if ((t - 2.0 * HOP_SECS) / 2.0).fract() < 0.6 {
                        at.pose(Gesture::Peek)
                    } else {
                        at.pose(Gesture::Crouch)
                    }
                } else if t < length - 2.0 * HOP_SECS {
                    at.pose(Gesture::Cheer)
                } else {
                    at
                };
            }
            if let Some(stance) = hop(t, (start, 0.0), step, true) {
                return stance;
            }
            if let Some(stance) = hop(t - HOP_SECS, step, top, true) {
                return stance;
            }
            let down = length - 2.0 * HOP_SECS;
            if t >= down {
                let t = t - down;
                return hop(t, top, step, false)
                    .or_else(|| hop(t - HOP_SECS, step, (start, 0.0), false))
                    .unwrap_or_else(|| Stance::at(start, 0.0, true));
            }
            let base = Stance::at(top.0, top.1, true).pose(Gesture::Sit);
            if pair && (8.0..9.0).contains(&t) {
                base.looking(30.0, 0.0)
                    .pose(Gesture::Gasp)
                    .feeling(AttentionEmotion::Startled)
            } else if pair && t >= 9.0 {
                base.looking(30.0, 0.0)
            } else {
                // Reading: eyes down at its lap.
                base.looking(4.0, 0.0).feeling(AttentionEmotion::Curious)
            }
        }
        WonderKind::Seesaw => {
            let end = if lead { -34.0 } else { 34.0 };
            let facing_right = lead;
            let tilt = motion(kind, Some(t), length, toss);
            // The left end goes up as the tilt goes up.
            let height = -9.0 - tilt * 7.0 * if lead { 1.0 } else { -1.0 };
            if let Some(stance) = hop(t, (start, 0.0), (end, -9.0), facing_right) {
                return stance;
            }
            if t >= off {
                return hop(t - off, (end, -9.0), (start, 0.0), !facing_right)
                    .unwrap_or_else(|| Stance::at(start, 0.0, facing_right));
            }
            let base = Stance::at(end, height, facing_right).pose(Gesture::Sit);
            // Paws up at the top of each swing.
            if tilt * if lead { 1.0 } else { -1.0 } > 0.8 {
                base.pose(Gesture::Cheer)
            } else {
                base
            }
        }
    }
}

/// Where along the rope the walker is, `walk` seconds after stepping onto it: slowly across with a
/// stop in the middle.
fn rope_x(walk: f32) -> f32 {
    if walk < 0.0 {
        -44.0
    } else if walk < 4.0 {
        mix(-44.0, -4.0, walk / 4.0)
    } else if walk < 5.4 {
        -4.0
    } else if walk < 9.0 {
        mix(-4.0, 44.0, ease((walk - 5.4) / 3.6))
    } else {
        44.0
    }
}

impl World {
    /// Free to be sent off to a wonder: here, awake and on its own feet, and doing nothing else
    /// that owns it.
    fn free_for_a_wonder(&self, creature: &Creature, home: bool) -> bool {
        let state = &creature.state;
        state.arrival_delay_secs <= 0.0
            && !state.indoors
            && state.beat.is_none()
            && state.drives.energy > 0.2
            && state.drives.sleep_pressure < 0.8
            && (if home {
                matches!(
                    state.action,
                    ActionKind::Homebound | ActionKind::Traverse | ActionKind::Idle
                )
            } else {
                matches!(
                    state.action,
                    ActionKind::Idle
                        | ActionKind::Traverse
                        | ActionKind::Perch
                        | ActionKind::InspectScreen
                        | ActionKind::SoloPlay
                )
            })
            && !self.attention.owns(creature.id)
            && !self.window_journeys.contains_key(&creature.id)
            && !self.window_routes.contains_key(&creature.id)
            && !self.tosses.contains_key(&creature.id)
            && !self.bond_plans.contains_key(&creature.id)
            && !self
                .bond_plans
                .values()
                .any(|plan| plan.target == creature.id)
            && !self.tows.involves(creature.id)
            && !self.village_life.contains_key(&creature.id)
            && offers::enjoying(&self.offers, creature.id).is_none()
            && !self
                .interaction
                .as_ref()
                .is_some_and(|interaction| interaction.creature_id == creature.id)
    }

    /// The wonder out now, for the overlay to draw.
    pub fn wonder(&self) -> Option<WonderView> {
        let active = self.wonders.active.as_ref()?;
        let t = active.script_time();
        let ride = carried(active.kind, t, active.length);
        let mut at = active.place(ride, 0.0);
        at.y = active.at.y;
        Some(WonderView {
            kind: active.kind,
            at,
            monitor_id: active.ground.monitor(),
            window_key: active.ground.window(),
            mirrored: active.mirrored,
            presence: active.presence(),
            elapsed: active.elapsed,
            motion: motion(active.kind, t, active.length, active.toss),
            glint: matches!(active.phase, Phase::Marvel(_)),
        })
    }

    /// Sends the wonder out now away at once, letting its players go where they stand. Picking
    /// one of them up, the village coming or going, and the colony being settled all end it.
    pub(super) fn send_wonder_away(&mut self, desktop: Option<&DesktopSnapshot>) {
        let Some(active) = self.wonders.active.as_mut() else {
            return;
        };
        if active.leaving() {
            return;
        }
        let ground = active.ground;
        let ground_y = active.at.y;
        let players: Vec<_> = active.players.drain(..).collect();
        active.phase = Phase::Leaving(0.0);
        for player in players {
            let Some(creature) = creature_mut(&mut self.save.creatures, player.id) else {
                continue;
            };
            let journeying = player
                .legs
                .front()
                .is_some_and(|leg| matches!(leg, Leg::Journey(_)));
            match (journeying, desktop) {
                (true, Some(desktop)) => settle_interrupted_journey(
                    creature,
                    desktop,
                    &self.save.settings.habitat,
                    &mut self.events,
                ),
                _ => {
                    // Back down on the wonder's own ground, wherever along it the player was.
                    if ground.holds(creature) || player.arrived {
                        creature.state.position.y = ground_y;
                        creature.state.surface = ground.surface(creature.state.position.x);
                    }
                }
            }
            release_player(creature, &mut self.events);
        }
    }

    /// Moves the wonder on by `dt`: starts one when one is due, walks its players over, plays its
    /// script, and lets it go.
    pub(super) fn advance_wonders(&mut self, dt: f32, desktop: &DesktopSnapshot, home: bool) {
        let lively = self.save.settings.visible
            && !self.save.settings.paused
            && !self.save.settings.reduce_motion
            && self.save.companion.quiet_until.is_none()
            && self.colony_plan.is_none()
            && self.village_moment.is_none();
        if !lively || std::mem::take(&mut self.wonders.grabbed) {
            self.send_wonder_away(Some(desktop));
        }
        if let Some(active) = self.wonders.active.as_mut() {
            active.elapsed += dt;
            if let Phase::Leaving(t) = &mut active.phase {
                *t += dt;
                if *t >= WONDERS.vanish_secs {
                    self.wonders.active = None;
                }
                return;
            }
            let at_home = matches!(active.ground, Ground::Village { .. });
            if at_home != home || !self.wonder_ground_holds(desktop) {
                self.send_wonder_away(Some(desktop));
                return;
            }
            self.play_wonder(dt, desktop);
            return;
        }
        if !lively || self.interaction.is_some() {
            return;
        }
        self.wonders.next_in -= dt;
        if self.wonders.next_in > 0.0 {
            return;
        }
        self.wonders.next_in = if self.start_wonder(desktop, home) {
            self.wonders.rng.random_range(WONDERS.interval_secs)
        } else {
            self.wonders.rng.random_range(WONDERS.retry_secs)
        };
    }

    /// Whether the ground under the wonder is still there and still where it was.
    fn wonder_ground_holds(&self, desktop: &DesktopSnapshot) -> bool {
        let Some(active) = &self.wonders.active else {
            return false;
        };
        let monitor_here = desktop
            .monitors
            .iter()
            .any(|monitor| monitor.id == active.ground.monitor());
        match active.ground {
            Ground::Ledge { window, bounds, .. } => desktop.windows.iter().any(|candidate| {
                candidate.key == window
                    && candidate.bounds == bounds
                    && candidate.visible
                    && !candidate.minimized
            }),
            Ground::Floor { .. } | Ground::Village { .. } => monitor_here,
        }
    }

    /// Tries to start a wonder. False when nobody was free or nowhere would take one.
    fn start_wonder(&mut self, desktop: &DesktopSnapshot, home: bool) -> bool {
        let free: Vec<CreatureId> = self
            .save
            .creatures
            .iter()
            .filter(|creature| self.free_for_a_wonder(creature, home))
            .map(|creature| creature.id)
            .collect();
        if free.is_empty() {
            return false;
        }
        // Playful and curious companions are likelier to be the ones a wonder turns up for.
        let weights: Vec<f32> = free
            .iter()
            .filter_map(|id| self.save.creatures.iter().find(|c| c.id == *id))
            .map(|c| 0.3 + c.personality.playfulness + c.personality.curiosity * 0.5)
            .collect();
        let total: f32 = weights.iter().sum();
        let mut pick = self.wonders.rng.random_range(0.0..total.max(0.001));
        let mut lead = free[0];
        for (id, weight) in free.iter().zip(&weights) {
            if pick < *weight {
                lead = *id;
                break;
            }
            pick -= weight;
        }
        let partners: Vec<CreatureId> = free.iter().copied().filter(|id| *id != lead).collect();
        let Some(kind) = self.choose_wonder_kind(!partners.is_empty()) else {
            return false;
        };
        let pair = match kind.seats() {
            WonderSeats::One => false,
            WonderSeats::Two => true,
            WonderSeats::OneOrTwo => !partners.is_empty() && self.wonders.rng.random_bool(0.7),
        };
        let Some(lead_creature) = self.save.creatures.iter().find(|c| c.id == lead).cloned() else {
            return false;
        };
        let Some((ground, at, unit)) =
            self.choose_wonder_ground(kind, &lead_creature, desktop, home)
        else {
            return false;
        };
        // Turned so the lead's side faces it, so the lead never walks past it to begin.
        let mirrored = lead_creature.state.position.x > at.x;
        let mut active = Active {
            kind,
            ground,
            at,
            unit,
            mirrored,
            players: Vec::new(),
            phase: Phase::Gathering,
            elapsed: 0.0,
            waited: 0.0,
            length: script_length(kind),
            toss: self.wonders.rng.random_bool(0.5),
            first: !self
                .save
                .companion
                .wonders
                .iter()
                .any(|record| record.kind == kind),
        };
        let mut chosen = vec![(lead, Role::Lead)];
        if pair {
            // The nearest free friend that can get there.
            let mut by_distance: Vec<(f32, CreatureId)> = partners
                .iter()
                .filter_map(|id| self.save.creatures.iter().find(|c| c.id == *id))
                .filter(|c| c.state.surface.monitor_id == ground.monitor())
                .map(|c| (c.state.position.distance(at), c.id))
                .collect();
            by_distance.sort_by(|a, b| a.0.total_cmp(&b.0));
            match by_distance.first() {
                Some((_, id)) => chosen.push((*id, Role::Partner)),
                None if kind.seats() == WonderSeats::Two => return false,
                None => {}
            }
        }
        for (id, role) in chosen {
            let Some(creature) = self.save.creatures.iter().find(|c| c.id == id) else {
                return false;
            };
            let start_dx = start_offset(kind, role);
            let start = active.place(start_dx, 0.0);
            let Some(legs) = wonder_route(creature, ground, start, desktop) else {
                if role == Role::Lead || kind.seats() == WonderSeats::Two {
                    return false;
                }
                continue;
            };
            active.players.push(Player {
                id,
                role,
                legs,
                start,
                arrived: false,
            });
        }
        if active.players.len() < 2 && kind.seats() == WonderSeats::Two {
            return false;
        }
        // Everyone it turned up for drops what it was doing and sets off.
        for player in &active.players {
            self.drop_village_activity(player.id);
            self.home_moments.remove(&player.id);
            self.home_roam.remove(&player.id);
            self.action_choices.remove(&player.id);
            if let Some(creature) = creature_mut(&mut self.save.creatures, player.id) {
                let old = creature.state.action;
                Self::emit(
                    &mut self.events,
                    WorldEvent::ActionCompleted {
                        creature_id: creature.id,
                        action: old,
                    },
                );
                creature.state.beat = None;
                creature.state.flourish = None;
                creature.state.attention = None;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = f32::MAX;
                bubbles::show(&mut self.bubbles, creature.id, BubbleIcon::Surprise);
            }
        }
        Self::emit(&mut self.events, WorldEvent::WonderAppeared { kind });
        self.wonders.active = Some(active);
        true
    }

    /// Which kind turns up: one the colony has not had yet three times in four while there are
    /// any, and never one for two with nobody to share it.
    fn choose_wonder_kind(&mut self, friend_free: bool) -> Option<WonderKind> {
        #[cfg(test)]
        if let Some(kind) = self.wonders.forced {
            return (friend_free || kind.seats() != WonderSeats::Two).then_some(kind);
        }
        let possible: Vec<WonderKind> = WonderKind::ALL
            .into_iter()
            .filter(|kind| friend_free || kind.seats() != WonderSeats::Two)
            .collect();
        let new: Vec<WonderKind> = possible
            .iter()
            .copied()
            .filter(|kind| {
                !self
                    .save
                    .companion
                    .wonders
                    .iter()
                    .any(|record| record.kind == *kind)
            })
            .collect();
        let pool = if !new.is_empty() && self.wonders.rng.random_bool(0.75) {
            new
        } else {
            possible
        };
        (!pool.is_empty()).then(|| pool[self.wonders.rng.random_range(0..pool.len())])
    }

    /// Where a wonder of this kind turns up for this companion: somewhere along the ground it is
    /// on, down on the floor below, or up on a window it could climb to. At home, only along the
    /// village's own ground. Gives the ground, the middle of the wonder on it, and art pixels to
    /// points there.
    fn choose_wonder_ground(
        &mut self,
        kind: WonderKind,
        lead: &Creature,
        desktop: &DesktopSnapshot,
        home: bool,
    ) -> Option<(Ground, Point, f32)> {
        let display_scale = self.save.settings.display_scale;
        let policy = &self.save.settings.habitat;
        let monitor = desktop
            .monitors
            .iter()
            .find(|m| m.id == lead.state.surface.monitor_id)?;
        let frame = spacing::frame_width(display_scale, monitor.scale_factor);
        let unit = frame / CREATURE_ART_WIDTH;
        let room = kind.room() * unit;
        // Runs of ground that would take it, as (ground, low x, high x, y), each with a weight.
        let mut runs: Vec<(Ground, f32, f32, f32, f32)> = Vec::new();
        if home {
            let commons = home_commons(
                &self.save.home,
                colony_cottage_list(&self.save.creatures).as_slice(),
                &desktop.monitors,
                policy,
                display_scale,
            )?;
            runs.push((
                Ground::Village {
                    monitor: commons.monitor_id,
                },
                commons.low_x,
                commons.high_x,
                commons.ground_y,
                1.0,
            ));
        } else {
            let on_ledge = lead.state.surface.window_key;
            for region in accessible_regions(policy, monitor) {
                runs.push((
                    Ground::Floor {
                        monitor: monitor.id,
                    },
                    region.x + 8.0,
                    region.right() - 8.0,
                    region.bottom() - 4.0,
                    if on_ledge.is_none() { 3.0 } else { 2.0 },
                ));
            }
            if self.save.settings.window_ledges {
                let floor_y = monitor.usable_bounds.bottom() - 4.0;
                for window in self.topology.windows() {
                    let bounds = window.bounds;
                    let mine = on_ledge == Some(window.key);
                    let near = (lead.state.position.x - (bounds.x + bounds.width / 2.0)).abs()
                        <= WONDERS.climb_reach + bounds.width / 2.0;
                    let rise = floor_y - bounds.y;
                    let headroom =
                        bounds.y - frame * WONDERS.headroom_frames >= monitor.usable_bounds.y;
                    if !(mine || (near && rise >= 36.0)) || !headroom {
                        continue;
                    }
                    if !monitor.bounds.contains(Point {
                        x: (bounds.x + bounds.width / 2.0),
                        y: bounds.y,
                    }) {
                        continue;
                    }
                    // Only the stretch of its top edge inside the habitat and in plain view.
                    let low = bounds.x + 12.0;
                    let high = bounds.right() - 12.0;
                    if high - low < room {
                        continue;
                    }
                    let open = (0..=4).all(|i| {
                        let x = mix(low, high, i as f32 / 4.0);
                        let point = Point {
                            x,
                            y: bounds.y - 4.0,
                        };
                        habitat_contains(policy, monitor, Point { x, y: bounds.y })
                            && point_exposed(point, Some(window.key), desktop)
                    });
                    if !open {
                        continue;
                    }
                    runs.push((
                        Ground::Ledge {
                            monitor: monitor.id,
                            window: window.key,
                            bounds,
                        },
                        low,
                        high,
                        bounds.y,
                        if mine { 3.0 } else { 1.0 },
                    ));
                }
            }
        }
        runs.retain(|run| run.2 - run.1 >= room);
        let total: f32 = runs.iter().map(|run| run.4).sum();
        if runs.is_empty() || total <= 0.0 {
            return None;
        }
        let mut pick = self.wonders.rng.random_range(0.0..total);
        let mut chosen = runs[0];
        for run in &runs {
            if pick < run.4 {
                chosen = *run;
                break;
            }
            pick -= run.4;
        }
        let (ground, low, high, y, _) = chosen;
        let (low, high) = (low + room / 2.0, high - room / 2.0);
        // A little way along from where the lead is, on whichever side has room.
        let away = self.wonders.rng.random_range(WONDERS.near_frames) * frame;
        let from = lead.state.position.x.clamp(low, high);
        let toward_right = self.wonders.rng.random_bool(0.5);
        let x = if toward_right {
            if from + away <= high {
                from + away
            } else {
                from - away
            }
        } else if from - away >= low {
            from - away
        } else {
            from + away
        }
        .clamp(low, high);
        Some((ground, Point { x, y }, unit))
    }

    /// Walks, hops and climbs every player on toward the wonder, marvels, plays the script, and
    /// lets it go when it is done.
    fn play_wonder(&mut self, dt: f32, desktop: &DesktopSnapshot) {
        let Some(mut active) = self.wonders.active.take() else {
            return;
        };
        let mut give_up = false;
        if active.phase == Phase::Gathering {
            active.waited += dt;
            for player in &mut active.players {
                if player.arrived {
                    continue;
                }
                let Some(creature) = creature_mut(&mut self.save.creatures, player.id) else {
                    give_up = true;
                    continue;
                };
                if !go_toward(creature, player, dt, desktop, &mut self.events) {
                    give_up = true;
                }
            }
            if active.waited > WONDERS.patience_secs {
                // Whoever has not made it is let go; a wonder for two without both is over.
                let late: Vec<CreatureId> = active
                    .players
                    .iter()
                    .filter(|player| !player.arrived)
                    .map(|player| player.id)
                    .collect();
                let lead_late = active
                    .players
                    .iter()
                    .any(|player| player.role == Role::Lead && !player.arrived);
                if lead_late || active.kind.seats() == WonderSeats::Two {
                    give_up = true;
                } else {
                    active.players.retain(|player| player.arrived);
                    for id in late {
                        if let Some(creature) = creature_mut(&mut self.save.creatures, id) {
                            release_player(creature, &mut self.events);
                        }
                    }
                }
            }
            if !give_up && active.players.iter().all(|player| player.arrived) {
                active.phase = if active.first {
                    self.find_wonder(&active);
                    Phase::Marvel(0.0)
                } else {
                    Phase::Playing(0.0)
                };
                self.count_wonder_go(active.kind);
            }
        }
        match &mut active.phase {
            Phase::Marvel(t) => {
                *t += dt;
                if *t >= WONDERS.marvel_secs {
                    active.phase = Phase::Playing(0.0);
                }
            }
            Phase::Playing(t) => {
                *t += dt;
            }
            _ => {}
        }
        if give_up {
            self.wonders.active = Some(active);
            self.send_wonder_away(Some(desktop));
            return;
        }
        // Everyone who has arrived takes up their place in the script, or marvels, or waits.
        let pair = active.players.len() > 1;
        for player in &active.players {
            if !player.arrived {
                continue;
            }
            let Some(creature) = creature_mut(&mut self.save.creatures, player.id) else {
                continue;
            };
            let stance = match active.phase {
                Phase::Playing(t) => stance(
                    active.kind,
                    player.role,
                    pair,
                    t,
                    active.length,
                    active.toss,
                ),
                Phase::Marvel(t) => {
                    let dx = start_offset(active.kind, player.role);
                    let at = Stance::at(dx, 0.0, dx < 0.0).looking(0.0, -16.0);
                    if player.role == Role::Partner {
                        at.pose(Gesture::Watch)
                    } else if t < 0.6 {
                        at.pose(Gesture::Gasp).feeling(AttentionEmotion::Startled)
                    } else {
                        at.pose(Gesture::Cheer)
                    }
                }
                _ => {
                    // Waiting for a friend still on the way, looking at the wonder.
                    let dx = start_offset(active.kind, player.role);
                    Stance::at(dx, 0.0, dx < 0.0)
                        .looking(0.0, -16.0)
                        .pose(Gesture::Watch)
                        .feeling(AttentionEmotion::Curious)
                }
            };
            show_stance(creature, &active, stance);
        }
        let over = matches!(active.phase, Phase::Playing(t) if t >= active.length);
        if over {
            let ids: Vec<CreatureId> = active.players.iter().map(|player| player.id).collect();
            if ids.len() == 2 {
                Self::emit(
                    &mut self.events,
                    WorldEvent::BondInteraction {
                        a: ids[0],
                        b: ids[1],
                        experience: RelationshipExperience::PositivePlay,
                    },
                );
                for id in &ids {
                    bubbles::show(&mut self.bubbles, *id, BubbleIcon::Heart);
                }
            }
            self.wonders.active = Some(active);
            // Each player ends on the wonder's ground, beside it, where its script left it.
            self.send_wonder_away(Some(desktop));
            return;
        }
        self.wonders.active = Some(active);
    }

    /// The first of a kind: written into the notebook, with a sparkle over its finder.
    fn find_wonder(&mut self, active: &Active) {
        let Some(lead) = active
            .players
            .iter()
            .find(|player| player.role == Role::Lead)
        else {
            return;
        };
        let name = self
            .save
            .creatures
            .iter()
            .find(|creature| creature.id == lead.id)
            .map(|creature| creature.name.clone())
            .unwrap_or_default();
        if !self
            .save
            .companion
            .wonders
            .iter()
            .any(|record| record.kind == active.kind)
        {
            self.save.companion.wonders.push(WonderRecord {
                kind: active.kind,
                first_at: self.save.maximum_seen_utc,
                finder: Some(lead.id),
                finder_name: name,
                goes: 0,
            });
        }
        bubbles::show(&mut self.bubbles, lead.id, BubbleIcon::Sparkle);
        Self::emit(
            &mut self.events,
            WorldEvent::WonderFound {
                creature_id: lead.id,
                kind: active.kind,
            },
        );
    }

    fn count_wonder_go(&mut self, kind: WonderKind) {
        if let Some(record) = self
            .save
            .companion
            .wonders
            .iter_mut()
            .find(|record| record.kind == kind)
        {
            record.goes = record.goes.saturating_add(1);
        }
    }
}

/// Puts a player where its stance says, striking its pose.
fn show_stance(creature: &mut Creature, active: &Active, stance: Stance) {
    creature.state.position = active.place(stance.dx, stance.dy);
    creature.state.facing_right = stance.facing_right != active.mirrored;
    creature.state.velocity = Point::default();
    if creature.state.action != stance.action {
        creature.state.action = stance.action;
        creature.state.action_elapsed = 0.0;
    }
    creature.state.action_duration = f32::MAX;
    creature.state.attention = Some(AttentionPose {
        target: stance
            .look
            .map_or_else(|| active.place(0.0, -16.0), |(dx, dy)| active.place(dx, dy)),
        emotion: stance.emotion,
        hanging: 0.0,
        gesture: stance.gesture,
    });
}

/// Lets a player go where it stands, to carry on as itself.
fn release_player(creature: &mut Creature, events: &mut Vec<WorldEvent>) {
    let old = creature.state.action;
    creature.state.attention = None;
    creature.state.velocity = Point::default();
    creature.state.action = ActionKind::Idle;
    creature.state.action_elapsed = 0.0;
    creature.state.action_duration = 2.5;
    World::emit(
        events,
        WorldEvent::ActionCompleted {
            creature_id: creature.id,
            action: old,
        },
    );
    World::emit(
        events,
        WorldEvent::ActionStarted {
            creature_id: creature.id,
            action: ActionKind::Idle,
        },
    );
}

/// The way from where a creature is to `start` on the wonder's ground: a walk if it is already on
/// that ground, a hop down first if it is up on a window and the wonder is not, and a climb up
/// onto the window the wonder is on otherwise, by the same hop and climb any trip up there takes.
fn wonder_route(
    creature: &Creature,
    ground: Ground,
    start: Point,
    desktop: &DesktopSnapshot,
) -> Option<VecDeque<Leg>> {
    let mut legs = VecDeque::new();
    if ground.holds(creature) {
        // Up on its own roof at home: down in a hop first, the way it went up, rather than
        // dropped to the ground between two frames.
        if let Ground::Village { .. } = ground
            && creature.state.position.y < start.y - 1.0
        {
            let landing = Point {
                x: creature.state.position.x,
                y: start.y,
            };
            legs.push_back(Leg::Journey(super::village_life::roof_hop_down(
                creature, landing,
            )));
        }
        legs.push_back(Leg::Walk);
        return Some(legs);
    }
    let mut from = creature.clone();
    if from.state.surface.window_key.is_some() {
        // Down to the floor first, straight below.
        let monitor = desktop
            .monitors
            .iter()
            .find(|m| m.id == from.state.surface.monitor_id)?;
        let floor = Point {
            x: from.state.position.x.clamp(
                monitor.usable_bounds.x + 8.0,
                monitor.usable_bounds.right() - 8.0,
            ),
            y: monitor.usable_bounds.bottom() - 4.0,
        };
        let surface = SurfaceAttachment {
            kind: SurfaceKind::ScreenFloor,
            monitor_id: monitor.id,
            window_key: None,
            relative_x: 0.5,
        };
        legs.push_back(Leg::Journey(WindowJourney::Hop(HopJourney {
            start: from.state.position,
            target: floor,
            surface: surface.clone(),
            elapsed: 0.0,
            duration: (from.state.position.distance(floor) / 180.0).max(0.25),
            lift: 0.0,
        })));
        from.state.position = floor;
        from.state.surface = surface;
    }
    if let Ground::Ledge { .. } = ground {
        if from.state.surface.monitor_id != ground.monitor() {
            return None;
        }
        legs.push_back(Leg::Journey(build_window_journey(
            &from,
            start,
            ground.surface(start.x),
            desktop,
        )));
    } else if from.state.surface.monitor_id != ground.monitor() {
        return None;
    }
    legs.push_back(Leg::Walk);
    Some(legs)
}

/// One step of a player's way over. False if the way has gone: a window it was climbing moved.
fn go_toward(
    creature: &mut Creature,
    player: &mut Player,
    dt: f32,
    desktop: &DesktopSnapshot,
    events: &mut Vec<WorldEvent>,
) -> bool {
    match player.legs.front_mut() {
        Some(Leg::Journey(journey)) => {
            if !journey.valid(desktop) {
                return false;
            }
            let step = journey.advance(dt);
            creature.state.facing_right = step.position.x >= creature.state.position.x
                || (step.position.x - creature.state.position.x).abs() < 0.01
                    && creature.state.facing_right;
            creature.state.position = step.position;
            creature.state.attention = None;
            if creature.state.action != step.action {
                creature.state.action = step.action;
                creature.state.action_elapsed = 0.0;
            }
            if step.complete {
                let surface = journey.surface().clone();
                creature.state.surface = surface.clone();
                World::emit(
                    events,
                    WorldEvent::SurfaceChanged {
                        creature_id: creature.id,
                        kind: surface.kind,
                    },
                );
                player.legs.pop_front();
            }
            true
        }
        Some(Leg::Walk) => {
            let to = player.start.x;
            let gap = to - creature.state.position.x;
            creature.state.attention = None;
            creature.state.position.y = player.start.y;
            if gap.abs() <= WONDERS.arrived {
                creature.state.position.x = to;
                player.legs.pop_front();
                player.arrived = true;
                return true;
            }
            // An eager run when it is a way off; a trot for the last of it.
            let running = gap.abs() > WONDERS.run_from;
            let speed =
                (40.0 + creature.personality.activity * 40.0) * if running { 1.8 } else { 1.0 };
            let action = if running {
                ActionKind::Sprint
            } else {
                ActionKind::Traverse
            };
            if creature.state.action != action {
                creature.state.action = action;
                creature.state.action_elapsed = 0.0;
            }
            let step = (speed * dt).min(gap.abs());
            creature.state.position.x += step * gap.signum();
            creature.state.facing_right = gap > 0.0;
            creature.state.velocity = Point {
                x: speed * gap.signum(),
                y: 0.0,
            };
            true
        }
        None => {
            player.arrived = true;
            true
        }
    }
}

/// One player's place and pose at one moment of a wonder's script, for review sheets: art pixels
/// from the middle of the wonder as if it were not turned round, as the simulation places it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WonderPose {
    pub dx: f32,
    pub dy: f32,
    pub facing_right: bool,
    pub action: ActionKind,
    pub gesture: Option<Gesture>,
    pub emotion: AttentionEmotion,
}

/// How long a wonder's script lasts, in seconds.
pub fn wonder_length(kind: WonderKind) -> f32 {
    script_length(kind)
}

/// Where each player is `t` seconds into a wonder's script: the lead first, then a partner if
/// there are two.
pub fn wonder_poses(kind: WonderKind, players: usize, t: f32, toss: bool) -> Vec<WonderPose> {
    let pair = players > 1;
    [Role::Lead, Role::Partner]
        .into_iter()
        .take(players.clamp(1, 2))
        .map(|role| {
            let s = stance(kind, role, pair, t, script_length(kind), toss);
            WonderPose {
                dx: s.dx,
                dy: s.dy,
                facing_right: s.facing_right,
                action: s.action,
                gesture: s.gesture,
                emotion: s.emotion,
            }
        })
        .collect()
}

/// What a wonder's moving part is doing `t` seconds into its script, and how far a sled has
/// ridden, as the overlay is told it.
pub fn wonder_motion(kind: WonderKind, t: f32, toss: bool) -> (f32, f32) {
    let length = script_length(kind);
    (
        motion(kind, Some(t), length, toss),
        carried(kind, Some(t), length),
    )
}
