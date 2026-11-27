//! Companions noticing the desktop around them: pointing at a window beside them, peering round
//! the end of a ledge, sitting on a window's top with their legs over the front, nudging a friend
//! off the end of one, and cheering a friend up onto one.
//!
//! Every minute or two out on the desktop, while nothing else is going on — no scene, no colony
//! cooldown after one — one companion who happens to be in the right place does one of these, and
//! never the same kind twice running, so what it does reads as noticing where it is rather than a
//! loop. Each is a beat (`world/beats.rs`): it holds the companion where it stands while the art
//! strikes a pose it already has, and lets go when it is over or when anything bigger comes along.
//! The nudge is the one that moves anybody: a playful companion, fond of a friend standing at the
//! end of a ledge with a short drop under it, puts out a paw, and the friend tips off in the same
//! little arc a tumble takes (`TossState`), landing wherever that lands — a lower ledge, or the
//! floor — and then has its say about it. Nobody is nudged over the edge of the habitat or a drop
//! that is not there to be had.
//!
//! Nothing here is saved, counted or written down; a companion has a while to itself after one;
//! reduced motion, quiet mode, hiding, pausing and a ritual hold all of it back; and every choice
//! comes from a stream of its own, so none of it shifts any other choice the colony makes.

use super::surfaces::drop_below;
use super::*;
use crate::tuning::SURROUNDINGS;

/// What a companion does about its surroundings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Doing {
    PointAt,
    PeekRound,
    LedgeSit,
    Nudge,
}

/// A nudge under way: the push still to land, then the friend in the air, then its say.
#[derive(Clone, Copy, Debug)]
struct NudgeUnderway {
    nudger: CreatureId,
    friend: CreatureId,
    /// Seconds until the paw meets the friend.
    push_in: f32,
    /// Which way the friend goes: +1 to the right.
    direction: f32,
    /// Whether it has been pushed, and how long it has been on its way down since.
    pushed: Option<f32>,
}

/// When the next of these is due, who has had one lately, and a nudge under way.
#[derive(Clone, Debug)]
pub(super) struct Surroundings {
    next_in: f32,
    pub(super) cooldowns: BTreeMap<CreatureId, f32>,
    last: Option<Doing>,
    nudge: Option<NudgeUnderway>,
    /// Visible seconds out on the desktop until a friend may next react to another's climb or
    /// fall: one budget for both, so reacting stays occasional.
    reaction_in: f32,
    /// Who was in the air at the last tick, so a fall is noticed as it starts.
    airborne: BTreeSet<CreatureId>,
    /// A friend who flinched at a fall and looks over once the faller is down.
    watching: Option<FallWatch>,
    /// Seconds until the next look for a pair in the right place for a nudge, and visible seconds
    /// out on the desktop until a nudge may come again.
    nudge_look_in: f32,
    nudge_rest: f32,
    rng: ChaCha12Rng,
    /// How many of each have begun, for the simulated-week counts and the tests.
    #[cfg(test)]
    pub(super) begun: BTreeMap<&'static str, u32>,
}

impl Surroundings {
    pub(super) fn new(streams: &SeedStream) -> Self {
        let mut rng = streams.rng("surroundings", 0);
        Self {
            next_in: rng.random_range(SURROUNDINGS.interval_secs),
            cooldowns: BTreeMap::new(),
            last: None,
            nudge: None,
            reaction_in: rng.random_range(SURROUNDINGS.reaction_gap_secs) * 0.25,
            airborne: BTreeSet::new(),
            watching: None,
            nudge_look_in: NUDGE_LOOK_SECS,
            nudge_rest: 0.0,
            rng,
            #[cfg(test)]
            begun: BTreeMap::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn due_now(&mut self) {
        self.next_in = 0.0;
    }

    #[cfg(test)]
    pub(super) fn react_now(&mut self) {
        self.reaction_in = 0.0;
    }

    #[cfg(test)]
    pub(super) fn nudging(&self) -> Option<(CreatureId, CreatureId)> {
        self.nudge.map(|n| (n.nudger, n.friend))
    }
}

/// A friend watching another come down: who, and how long it has been watching.
#[derive(Clone, Copy, Debug)]
struct FallWatch {
    faller: CreatureId,
    watcher: CreatureId,
    since: f32,
}

/// Somebody who could do something about where it is: what, how much it would want to, where it
/// would look, and the friend it would nudge.
#[derive(Clone, Copy)]
struct Candidate(CreatureId, Doing, f32, Option<Point>, Option<CreatureId>);

/// How often to look for a pair in the right place for a nudge, in seconds.
const NUDGE_LOOK_SECS: f32 = 2.0;
/// How long the nudger's paw takes to meet the friend.
const PUSH_SECS: f32 = 0.55;
/// How long a nudged friend has to come down before it is let go of without having its say.
const LANDING_PATIENCE_SECS: f32 = 7.0;

impl World {
    /// Lets go of everything here: the colony is being settled.
    pub(super) fn clear_surroundings(&mut self) {
        self.surroundings.nudge = None;
        self.surroundings.watching = None;
    }

    /// Lets go of whatever here has to do with one companion: it is going away somewhere — into
    /// a house open in Formiga Home — so its ledge sit or peek, and a nudge or a watch it is part
    /// of, end now rather than wait for the next tick to notice.
    pub(super) fn let_go_of_surroundings(&mut self, id: CreatureId) {
        if self
            .surroundings
            .nudge
            .is_some_and(|n| n.nudger == id || n.friend == id)
        {
            self.surroundings.nudge = None;
        }
        if self
            .surroundings
            .watching
            .is_some_and(|w| w.faller == id || w.watcher == id)
        {
            self.surroundings.watching = None;
        }
        if let Some(creature) = creature_mut(&mut self.save.creatures, id)
            && creature.state.beat.is_some_and(|beat| {
                matches!(
                    beat.kind,
                    BeatKind::PointAt
                        | BeatKind::PeekRound
                        | BeatKind::LedgeSit
                        | BeatKind::Nudge
                        | BeatKind::Relieved
                        | BeatKind::Amused
                        | BeatKind::Delight
                )
            })
        {
            creature.state.beat = None;
        }
    }

    /// Whether anything new about the desktop may begin: out on it, visible, unpaused, motion
    /// not reduced, not quiet, no ritual, and no scene or colony cooldown after one.
    fn surroundings_lively(&self) -> bool {
        self.save.settings.visible
            && !self.save.settings.paused
            && !self.save.settings.reduce_motion
            && self.save.companion.quiet_until.is_none()
            && self.colony_plan.is_none()
            && !self.save.home.is_active()
    }

    /// Moves a nudge under way along, lets go of a ledge sit whose ledge has gone, and now and
    /// then has somebody notice where it is. Called once a tick while the colony is out on the
    /// desktop.
    pub(super) fn advance_surroundings(&mut self, dt: f32, desktop: &DesktopSnapshot) {
        for cooldown in self.surroundings.cooldowns.values_mut() {
            *cooldown -= dt;
        }
        self.surroundings.cooldowns.retain(|_, left| *left > 0.0);
        // A ledge sit or a peek is had on a ledge: anything that takes it off one ends it.
        for creature in &mut self.save.creatures {
            if creature
                .state
                .beat
                .is_some_and(|beat| matches!(beat.kind, BeatKind::LedgeSit | BeatKind::PeekRound))
                && (creature.state.surface.kind != SurfaceKind::WindowLedge
                    || !matches!(creature.state.action, ActionKind::Idle | ActionKind::Perch))
            {
                creature.state.beat = None;
            }
        }
        let lively = self.surroundings_lively();
        self.advance_nudge(dt, desktop, lively);
        if lively {
            self.surroundings.reaction_in = (self.surroundings.reaction_in - dt).max(0.0);
        }
        self.watch_falls(dt, desktop, lively);
        if !lively {
            return;
        }
        self.look_for_a_nudge(dt, desktop);
        self.surroundings.next_in -= dt;
        if self.surroundings.next_in > 0.0 {
            return;
        }
        let started = self.attention.settled()
            && self.surroundings.nudge.is_none()
            && self.notice_surroundings(desktop);
        self.surroundings.next_in = self.surroundings.rng.random_range(if started {
            SURROUNDINGS.interval_secs
        } else {
            SURROUNDINGS.retry_secs
        });
    }

    /// Whether a companion could do one of these now: free for a beat, out on the desktop, on its
    /// own feet, and not still having a while to itself after the last.
    fn free_to_notice(&self, creature: &Creature) -> bool {
        matches!(creature.state.action, ActionKind::Idle | ActionKind::Perch)
            && !self.surroundings.cooldowns.contains_key(&creature.id)
            && !self.house_visit.contains(&creature.id)
            && beats::free_for_a_beat(self, creature)
    }

    /// Looks for somebody in the right place for one of these, and starts it. Every kind anybody
    /// could do is weighed by who would do it; the kind done last time is left out.
    fn notice_surroundings(&mut self, desktop: &DesktopSnapshot) -> bool {
        let mut options: Vec<Candidate> = Vec::new();
        for creature in &self.save.creatures {
            if !self.free_to_notice(creature) {
                continue;
            }
            let p = &creature.personality;
            let traits = creature.traits();
            let has = |any: &[Trait]| traits.iter().any(|t| any.contains(t));
            let curious = p.curiosity
                + if has(&[Trait::Curious, Trait::Nosy, Trait::Observant]) {
                    0.6
                } else {
                    0.0
                };
            if let Some(look) = self.window_beside(creature, desktop) {
                options.push(Candidate(
                    creature.id,
                    Doing::PointAt,
                    0.3 + curious,
                    Some(look),
                    None,
                ));
            }
            if creature.state.surface.kind == SurfaceKind::WindowLedge {
                if let Some(look) = self.ledge_end_view(creature, desktop) {
                    let shy = if has(&[Trait::Shy, Trait::Cautious, Trait::Wary]) {
                        0.5
                    } else {
                        0.0
                    };
                    options.push(Candidate(
                        creature.id,
                        Doing::PeekRound,
                        0.2 + curious * 0.7 + shy,
                        Some(look),
                        None,
                    ));
                }
                // The original legless blob has nothing to hang over the edge: lowered, it only
                // looks sunk into the window, so it does not sit this way.
                let legless = creature.appearance.design.is_none()
                    && creature.appearance.family == BodyFamily::Blob;
                if !legless
                    && drop_below(
                        creature,
                        creature.state.position.x,
                        desktop,
                        &self.save.settings,
                    ) >= SURROUNDINGS.ledge_sit_drop
                {
                    let calm = 1.0 - p.activity
                        + if has(&[Trait::Mellow, Trait::Easygoing, Trait::Sleepy, Trait::Lazy]) {
                            0.6
                        } else {
                            0.0
                        };
                    options.push(Candidate(
                        creature.id,
                        Doing::LedgeSit,
                        0.3 + calm,
                        None,
                        None,
                    ));
                }
                if let Some((friend, direction)) = self.friend_to_nudge(creature, desktop) {
                    let playful = p.playfulness
                        + if has(&[Trait::Mischievous, Trait::Playful, Trait::Silly]) {
                            0.8
                        } else {
                            0.0
                        };
                    if playful >= SURROUNDINGS.nudge_playfulness
                        && self.surroundings.nudge_rest <= 0.0
                    {
                        // A pair in the right place is rare, so when there is one it is likely.
                        options.push(Candidate(
                            creature.id,
                            Doing::Nudge,
                            1.0 + playful * 2.0,
                            Some(Point {
                                x: direction,
                                y: 0.0,
                            }),
                            Some(friend),
                        ));
                    }
                }
            }
        }
        let last = self.surroundings.last;
        options.retain(|option| Some(option.1) != last);
        let total: f32 = options.iter().map(|option| option.2).sum();
        if options.is_empty() || total <= 0.0 {
            return false;
        }
        let mut roll = self.surroundings.rng.random_range(0.0..total);
        let chosen = options
            .iter()
            .find(|option| {
                let hit = roll < option.2;
                roll -= option.2;
                hit
            })
            .copied()
            .unwrap_or(options[0]);
        let Candidate(id, doing, _, look, friend) = chosen;
        self.surroundings.last = Some(doing);
        let cooldown = self
            .surroundings
            .rng
            .random_range(SURROUNDINGS.cooldown_secs);
        self.surroundings.cooldowns.insert(id, cooldown);
        #[cfg(test)]
        {
            *self
                .surroundings
                .begun
                .entry(match doing {
                    Doing::PointAt => "point",
                    Doing::PeekRound => "peek",
                    Doing::LedgeSit => "sit",
                    Doing::Nudge => "nudge",
                })
                .or_default() += 1;
        }
        match doing {
            Doing::PointAt => {
                let look = look.unwrap_or_default();
                self.hold_in_beat(id, BeatKind::PointAt, 3.0, Some(look));
                if self.surroundings.rng.random_bool(0.4) {
                    self.show_bubble(id, BubbleIcon::Question);
                }
            }
            Doing::PeekRound => {
                self.hold_in_beat(id, BeatKind::PeekRound, 2.8, look);
                if self.surroundings.rng.random_bool(0.3) {
                    self.show_bubble(id, BubbleIcon::Watching);
                }
            }
            Doing::LedgeSit => {
                let length = self
                    .surroundings
                    .rng
                    .random_range(SURROUNDINGS.ledge_sit_secs);
                self.hold_in_beat(id, BeatKind::LedgeSit, length, None);
            }
            Doing::Nudge => {
                let (Some(friend), Some(direction)) = (friend, look) else {
                    return false;
                };
                self.start_nudge(id, friend, direction.x, cooldown);
            }
        }
        true
    }

    /// A playful companion puts out a paw to a friend at the end of the ledge.
    fn start_nudge(&mut self, id: CreatureId, friend: CreatureId, direction: f32, cooldown: f32) {
        let look = self
            .save
            .creatures
            .iter()
            .find(|c| c.id == friend)
            .map(|c| c.state.position);
        self.hold_in_beat(id, BeatKind::Nudge, 1.4, look);
        self.surroundings.cooldowns.insert(friend, cooldown);
        self.surroundings.nudge = Some(NudgeUnderway {
            nudger: id,
            friend,
            push_in: PUSH_SECS,
            direction,
            pushed: None,
        });
        self.surroundings.nudge_rest = self
            .surroundings
            .rng
            .random_range(SURROUNDINGS.nudge_rest_secs);
        // It is not going to startle at its own push.
        self.reacted_to_toss.insert((id, friend));
    }

    /// A playful pair in just the right place on a ledge does not last long, so it is looked for
    /// every couple of seconds rather than only when something is due — and once a nudge has
    /// happened, not again for hours.
    fn look_for_a_nudge(&mut self, dt: f32, desktop: &DesktopSnapshot) {
        self.surroundings.nudge_rest = (self.surroundings.nudge_rest - dt).max(0.0);
        self.surroundings.nudge_look_in -= dt;
        if self.surroundings.nudge_look_in > 0.0 {
            return;
        }
        self.surroundings.nudge_look_in = NUDGE_LOOK_SECS;
        if self.surroundings.nudge_rest > 0.0
            || self.surroundings.nudge.is_some()
            || !self.attention.settled()
        {
            return;
        }
        let found = self.save.creatures.iter().find_map(|creature| {
            if creature.state.surface.kind != SurfaceKind::WindowLedge
                || !self.free_to_notice(creature)
            {
                return None;
            }
            let traits = creature.traits();
            let playful = creature.personality.playfulness
                + if traits
                    .iter()
                    .any(|t| matches!(t, Trait::Mischievous | Trait::Playful | Trait::Silly))
                {
                    0.8
                } else {
                    0.0
                };
            if playful < SURROUNDINGS.nudge_playfulness {
                return None;
            }
            self.friend_to_nudge(creature, desktop)
                .map(|(friend, direction)| (creature.id, friend, direction))
        });
        let Some((id, friend, direction)) = found else {
            return;
        };
        if !self.surroundings.rng.random_bool(SURROUNDINGS.nudge_chance) {
            return;
        }
        let cooldown = self
            .surroundings
            .rng
            .random_range(SURROUNDINGS.cooldown_secs);
        self.surroundings.cooldowns.insert(id, cooldown);
        self.surroundings.last = Some(Doing::Nudge);
        #[cfg(test)]
        {
            *self.surroundings.begun.entry("nudge").or_default() += 1;
        }
        self.start_nudge(id, friend, direction, cooldown);
    }

    /// Holds a companion in a beat, turned toward `look` if it names a place.
    fn hold_in_beat(&mut self, id: CreatureId, kind: BeatKind, length: f32, look: Option<Point>) {
        let Some(creature) = creature_mut(&mut self.save.creatures, id) else {
            return;
        };
        if let Some(look) = look
            && (look.x - creature.state.position.x).abs() > 1.0
        {
            creature.state.facing_right = look.x > creature.state.position.x;
        }
        creature.state.beat = Some(Beat {
            look,
            ..Beat::new(kind, length)
        });
    }

    /// A window standing beside a companion at its own height, not the one it is on, with a side
    /// near enough to point at: where on that side to look, about half way up the companion's
    /// own height into the window.
    fn window_beside(&self, creature: &Creature, desktop: &DesktopSnapshot) -> Option<Point> {
        let frame =
            spacing::creature_frame_width(creature, self.save.settings.display_scale, desktop);
        let reach = frame * SURROUNDINGS.near_edge_frames;
        let at = creature.state.position;
        let head = at.y - frame * 0.5;
        desktop
            .windows
            .iter()
            .take(MAX_TOPOLOGY_WINDOWS)
            .filter(|w| w.visible && !w.minimized)
            .filter(|w| Some(w.key) != creature.state.surface.window_key)
            .filter(|w| w.bounds.y < head && w.bounds.bottom() > head)
            .filter(|w| w.bounds.width >= frame && w.bounds.height >= frame)
            .filter_map(|w| {
                let left = w.bounds.x - at.x;
                let right = at.x - w.bounds.right();
                let (gap, edge, inward) = if left > 0.0 {
                    (left, w.bounds.x, 1.0)
                } else if right > 0.0 {
                    (right, w.bounds.right(), -1.0)
                } else {
                    return None;
                };
                (gap >= frame * 0.3 && gap <= reach).then_some((
                    gap,
                    Point {
                        x: edge + inward * frame * 0.6,
                        y: head,
                    },
                ))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, look)| look)
    }

    /// Standing near the end of a window's top with air beside it: a spot down past the end of
    /// the ledge to peer round at.
    fn ledge_end_view(&self, creature: &Creature, desktop: &DesktopSnapshot) -> Option<Point> {
        let key = creature.state.surface.window_key?;
        let window = desktop
            .windows
            .iter()
            .take(MAX_TOPOLOGY_WINDOWS)
            .find(|w| w.key == key && w.visible && !w.minimized)?;
        let frame =
            spacing::creature_frame_width(creature, self.save.settings.display_scale, desktop);
        let x = creature.state.position.x;
        let (edge, outward) = if x - window.bounds.x < window.bounds.right() - x {
            (window.bounds.x, -1.0)
        } else {
            (window.bounds.right(), 1.0)
        };
        if (x - edge).abs() > frame {
            return None;
        }
        let beyond = edge + outward * frame * 0.5;
        (drop_below(creature, beyond, desktop, &self.save.settings) >= 24.0).then_some(Point {
            x: beyond,
            y: creature.state.position.y + frame,
        })
    }

    /// A friend on the same ledge, near enough, nearer its end than the nudger and close to it,
    /// with a fun drop beyond that end onto somewhere it can land inside the habitat: the friend,
    /// and the way it would go.
    fn friend_to_nudge(
        &self,
        creature: &Creature,
        desktop: &DesktopSnapshot,
    ) -> Option<(CreatureId, f32)> {
        self.friend_to_nudge_among(creature, desktop, None)
    }

    /// [`Self::friend_to_nudge`], or, with `only`, whether that one friend is still where a nudge
    /// would be fun and safe: it has been promised to the nudge, so its while to itself is not
    /// asked about.
    fn friend_to_nudge_among(
        &self,
        creature: &Creature,
        desktop: &DesktopSnapshot,
        only: Option<CreatureId>,
    ) -> Option<(CreatureId, f32)> {
        let key = creature.state.surface.window_key?;
        let window = desktop
            .windows
            .iter()
            .take(MAX_TOPOLOGY_WINDOWS)
            .find(|w| w.key == key && w.visible && !w.minimized)?;
        let frame =
            spacing::creature_frame_width(creature, self.save.settings.display_scale, desktop);
        let monitor = desktop
            .monitors
            .iter()
            .find(|m| m.id == creature.state.surface.monitor_id)?;
        let regions = accessible_regions(&self.save.settings.habitat, monitor);
        self.save.creatures.iter().find_map(|friend| {
            // Being nudged is not something the friend does, so its own while to itself after
            // the last thing it did is not asked about: only that it is standing or sat, free.
            let free = only.is_none_or(|id| friend.id == id)
                && matches!(friend.state.action, ActionKind::Idle | ActionKind::Perch)
                && !self.house_visit.contains(&friend.id)
                && beats::free_for_a_beat(self, friend);
            if friend.id == creature.id || friend.state.surface.window_key != Some(key) || !free {
                return None;
            }
            let fond = relationship_between(&self.save.relationships, creature.id, friend.id)
                .is_some_and(|bond| {
                    bond.affinity >= SURROUNDINGS.nudge_affinity && bond.avoidance < 128
                });
            if !fond {
                return None;
            }
            let apart = friend.state.position.x - creature.state.position.x;
            if apart.abs() > frame * SURROUNDINGS.nudge_reach_frames || apart.abs() < frame * 0.3 {
                return None;
            }
            let direction = apart.signum();
            let edge = if direction > 0.0 {
                window.bounds.right()
            } else {
                window.bounds.x
            };
            if (edge - friend.state.position.x).abs() > frame * SURROUNDINGS.nudge_edge_frames {
                return None;
            }
            let landing_x = edge + direction * frame * 0.6;
            let drop = drop_below(friend, landing_x, desktop, &self.save.settings);
            if !SURROUNDINGS.nudge_drop.contains(&drop) {
                return None;
            }
            let landing = Point {
                x: landing_x,
                y: friend.state.position.y + drop - 1.0,
            };
            let inside = regions
                .iter()
                .any(|r| r.contains(friend.state.position) && r.contains(landing));
            inside.then_some((friend.id, direction))
        })
    }

    /// The nudge under way: the paw meets the friend, the friend tips off in a little arc, and
    /// once it is down it has its say, and the nudger laughs. Anything that takes either of them
    /// first lets the nudge go, and a friend already in the air simply lands as any tumble does.
    fn advance_nudge(&mut self, dt: f32, desktop: &DesktopSnapshot, lively: bool) {
        let Some(mut nudge) = self.surroundings.nudge else {
            return;
        };
        let (Some(nudger), Some(friend)) = (
            self.save
                .creatures
                .iter()
                .position(|c| c.id == nudge.nudger),
            self.save
                .creatures
                .iter()
                .position(|c| c.id == nudge.friend),
        ) else {
            self.surroundings.nudge = None;
            return;
        };
        match nudge.pushed {
            None => {
                let still_nudging = self.save.creatures[nudger]
                    .state
                    .beat
                    .is_some_and(|beat| beat.kind == BeatKind::Nudge);
                let friend_state = &self.save.creatures[friend];
                let friend_ready = lively
                    && beats::free_for_a_beat(self, friend_state)
                    && friend_state.state.surface.window_key
                        == self.save.creatures[nudger].state.surface.window_key
                    && self.friend_to_nudge_again(nudger, friend, desktop);
                if !still_nudging || !friend_ready {
                    self.surroundings.nudge = None;
                    return;
                }
                nudge.push_in -= dt;
                if nudge.push_in <= 0.0 {
                    self.tip_off(friend, nudge.direction);
                    nudge.pushed = Some(0.0);
                }
                self.surroundings.nudge = Some(nudge);
            }
            Some(since) => {
                let since = since + dt;
                // Down and getting over the landing: once it has, it stands a moment to have its
                // say before it does anything else, rather than setting straight off again.
                let state = &mut self.save.creatures[friend].state;
                let in_the_air =
                    self.tosses.contains_key(&nudge.friend) || state.action == ActionKind::Tossed;
                if !in_the_air
                    && state.action == ActionKind::Landing
                    && state.action_elapsed + 2.0 * dt >= state.action_duration
                {
                    state.action = ActionKind::Idle;
                    state.action_elapsed = 0.0;
                    state.action_duration = 3.0;
                }
                let friend_state = &self.save.creatures[friend];
                let down = !in_the_air && friend_state.state.action != ActionKind::Landing;
                // Down, and over the landing: it has its say once it is free to.
                let ready = down && beats::free_for_a_beat(self, friend_state);
                if !ready && since <= LANDING_PATIENCE_SECS && lively {
                    nudge.pushed = Some(since);
                    self.surroundings.nudge = Some(nudge);
                    return;
                }
                self.surroundings.nudge = None;
                if down && lively {
                    self.have_a_say_after_a_nudge(friend, nudger);
                }
            }
        }
    }

    /// Whether the friend is still where a nudge would send it somewhere safe.
    fn friend_to_nudge_again(
        &self,
        nudger: usize,
        friend: usize,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let friend_id = self.save.creatures[friend].id;
        self.friend_to_nudge_among(&self.save.creatures[nudger], desktop, Some(friend_id))
            .is_some_and(|(id, _)| id == friend_id)
    }

    /// Tips a friend off the end of a ledge in the little arc a tumble takes, so it lands as any
    /// tumble does: on the first support its path crosses, or the floor.
    fn tip_off(&mut self, friend: usize, direction: f32) {
        let creature = &mut self.save.creatures[friend];
        creature.state.beat = None;
        self.tosses.insert(
            creature.id,
            TossState {
                elapsed: 0.0,
                bounces: 0,
                last_safe_position: creature.state.position,
                last_safe_surface: creature.state.surface.clone(),
            },
        );
        creature.state.action = ActionKind::Tossed;
        creature.state.action_elapsed = 0.0;
        creature.state.facing_right = direction < 0.0;
        creature.state.velocity = Point {
            x: direction * 95.0,
            y: -150.0,
        };
        let id = creature.id;
        self.show_bubble(id, BubbleIcon::Surprise);
    }

    /// Down again after a nudge: the friend has its say as it is, and the nudger laughs.
    fn have_a_say_after_a_nudge(&mut self, friend: usize, nudger: usize) {
        let friend_id = self.save.creatures[friend].id;
        let nudger_id = self.save.creatures[nudger].id;
        let at_friend = self.save.creatures[friend].state.position;
        let at_nudger = self.save.creatures[nudger].state.position;
        let creature = &self.save.creatures[friend];
        let traits = creature.traits();
        let has = |any: &[Trait]| traits.iter().any(|t| any.contains(t));
        let (beat, bubble) = if has(&[Trait::Grumpy, Trait::Irritable, Trait::Stubborn]) {
            (BeatKind::Huff, BubbleIcon::Grumble)
        } else if has(&[Trait::Shy, Trait::EasilyEmbarrassed, Trait::Nervous]) {
            (BeatKind::Peek, BubbleIcon::Blush)
        } else {
            (BeatKind::Delight, BubbleIcon::Music)
        };
        if beats::free_for_a_beat(self, creature) {
            self.hold_in_beat(friend_id, beat, 1.8, Some(at_nudger));
        }
        self.show_bubble(friend_id, bubble);
        let nudger_creature = &self.save.creatures[nudger];
        if beats::free_for_a_beat(self, nudger_creature) {
            self.hold_in_beat(nudger_id, BeatKind::Delight, 1.6, Some(at_friend));
            self.show_bubble(nudger_id, BubbleIcon::Music);
        }
    }

    /// Somebody just got up onto a window: a fond friend on the ground nearby, free for it, may
    /// cheer. Answered from the same cue a show-off's strut is (`antics::Cue::Climbed`).
    pub(super) fn cheer_a_climb(&mut self, climber: CreatureId) {
        if !self.surroundings_lively() {
            return;
        }
        let Some(up) = self.save.creatures.iter().find(|c| c.id == climber) else {
            return;
        };
        let at = up.state.position;
        let monitor = up.state.surface.monitor_id;
        let reach = CREATURE_ART_WIDTH * 2.0 * 5.0;
        let cheerer = self
            .save
            .creatures
            .iter()
            .filter(|c| {
                c.id != climber
                    && c.state.surface.monitor_id == monitor
                    && (c.state.position.x - at.x).abs() <= reach
                    && c.state.position.y > at.y + 20.0
                    && self.free_to_notice(c)
            })
            .filter(|c| {
                relationship_between(&self.save.relationships, climber, c.id)
                    .is_some_and(|bond| bond.affinity >= SURROUNDINGS.nudge_affinity)
            })
            .min_by(|a, b| {
                (a.state.position.x - at.x)
                    .abs()
                    .total_cmp(&(b.state.position.x - at.x).abs())
            })
            .map(|c| c.id);
        let Some(id) = cheerer else {
            return;
        };
        if self.surroundings.reaction_in > 0.0
            || !self
                .surroundings
                .rng
                .random_bool(SURROUNDINGS.cheer_climb_chance)
        {
            return;
        }
        self.surroundings.reaction_in = self
            .surroundings
            .rng
            .random_range(SURROUNDINGS.reaction_gap_secs);
        let cooldown = self
            .surroundings
            .rng
            .random_range(SURROUNDINGS.cooldown_secs);
        self.surroundings.cooldowns.insert(id, cooldown);
        self.hold_in_beat(id, BeatKind::Delight, 1.8, Some(at));
        self.show_bubble(id, BubbleIcon::Heart);
        #[cfg(test)]
        {
            *self.surroundings.begun.entry("cheer").or_default() += 1;
        }
    }
}

impl World {
    /// Somebody nearby starts to fall — tipped off a ledge, thrown, tumbling — and a fond friend
    /// free for it flinches with a gasp as it drops, then, once it is down, looks over at it: glad
    /// it is all right, or, for a playful one, finding it funny. Shares one budget with cheering a
    /// climb, so a colony reacts to one now and then, not to every one.
    fn watch_falls(&mut self, dt: f32, desktop: &DesktopSnapshot, lively: bool) {
        let in_air: BTreeSet<CreatureId> = self
            .save
            .creatures
            .iter()
            .filter(|c| c.state.action == ActionKind::Tossed || self.tosses.contains_key(&c.id))
            .map(|c| c.id)
            .collect();
        let started: Vec<CreatureId> = in_air
            .difference(&self.surroundings.airborne)
            .copied()
            .collect();
        self.surroundings.airborne = in_air;
        if let Some(mut watch) = self.surroundings.watching {
            watch.since += dt;
            self.surroundings.watching = Some(watch);
            let faller = self.save.creatures.iter().find(|c| c.id == watch.faller);
            let down = faller.is_some_and(|c| {
                !self.surroundings.airborne.contains(&c.id) && c.state.action != ActionKind::Landing
            });
            if !lively || faller.is_none() || watch.since > 8.0 {
                self.surroundings.watching = None;
            } else if down {
                self.surroundings.watching = None;
                self.look_over_after_a_fall(watch);
            }
        }
        if !lively || self.surroundings.watching.is_some() || self.surroundings.reaction_in > 0.0 {
            return;
        }
        let nudger = self.surroundings.nudge.map(|n| n.nudger);
        for faller in started {
            let Some(down) = self.save.creatures.iter().find(|c| c.id == faller) else {
                continue;
            };
            let at = down.state.position;
            let monitor = down.state.surface.monitor_id;
            let reach =
                spacing::creature_frame_width(down, self.save.settings.display_scale, desktop)
                    * 6.0;
            let watcher = self
                .save
                .creatures
                .iter()
                .filter(|c| {
                    c.id != faller
                        && nudger != Some(c.id)
                        && c.state.surface.monitor_id == monitor
                        && c.state.position.distance(at) <= reach
                        && self.free_to_notice(c)
                })
                .filter(|c| {
                    relationship_between(&self.save.relationships, faller, c.id)
                        .is_some_and(|bond| bond.affinity >= SURROUNDINGS.nudge_affinity)
                })
                .min_by(|a, b| {
                    a.state
                        .position
                        .distance(at)
                        .total_cmp(&b.state.position.distance(at))
                })
                .map(|c| c.id);
            let Some(watcher) = watcher else {
                continue;
            };
            if !self
                .surroundings
                .rng
                .random_bool(SURROUNDINGS.watch_fall_chance)
            {
                continue;
            }
            self.surroundings.reaction_in = self
                .surroundings
                .rng
                .random_range(SURROUNDINGS.reaction_gap_secs);
            let cooldown = self
                .surroundings
                .rng
                .random_range(SURROUNDINGS.cooldown_secs);
            self.surroundings.cooldowns.insert(watcher, cooldown);
            self.hold_in_beat(watcher, BeatKind::Startle, 0.9, Some(at));
            self.surroundings.watching = Some(FallWatch {
                faller,
                watcher,
                since: 0.0,
            });
            #[cfg(test)]
            {
                *self.surroundings.begun.entry("fall").or_default() += 1;
            }
            return;
        }
    }

    /// The faller is down: its watcher looks over at it, by temperament glad or amused.
    fn look_over_after_a_fall(&mut self, watch: FallWatch) {
        let Some(faller) = self.save.creatures.iter().find(|c| c.id == watch.faller) else {
            return;
        };
        let at = faller.state.position;
        let Some(watcher) = self.save.creatures.iter().find(|c| c.id == watch.watcher) else {
            return;
        };
        // Still in its flinch is as good as free: the look over follows straight on from it.
        let mut probe = watcher.clone();
        if probe
            .state
            .beat
            .is_some_and(|beat| beat.kind == BeatKind::Startle)
        {
            probe.state.beat = None;
        }
        if !beats::free_for_a_beat(self, &probe) {
            return;
        }
        let traits = watcher.traits();
        let has = |any: &[Trait]| traits.iter().any(|t| any.contains(t));
        let amused = has(&[Trait::Mischievous, Trait::Silly, Trait::Playful])
            || watcher.personality.playfulness >= 0.75;
        let (kind, bubble) = if amused {
            (BeatKind::Amused, BubbleIcon::Music)
        } else {
            (BeatKind::Relieved, BubbleIcon::Heart)
        };
        self.hold_in_beat(watch.watcher, kind, 1.8, Some(at));
        self.show_bubble(watch.watcher, bubble);
    }
}
