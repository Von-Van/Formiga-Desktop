//! A friend towing a sleeper out of the way on a little rope.
//!
//! When two companions are drawn through one another and the one that has to move is asleep, it
//! used to slide over in its sleep with nothing to explain it. Now a friend nearby, awake and doing
//! nothing much, comes over, takes up a rope, walks the sleeper clear at an unhurried pull, lets
//! go, and goes back to what it was doing. With nobody free to, the sleeper wriggles over in its
//! sleep instead. A tow is runtime only and never saved, and anything that takes hold of either of
//! the two ends it where they stand.

use super::*;

/// The most tows at once: one friend for every sleeper in a colony that is half asleep.
const MAX_TOWS: usize = MAX_COLONY_CREATURES / 2;

/// How far behind its friend a towed sleeper rides, in creature widths: close enough to be pulled,
/// far enough apart for the rope between them to read.
const ROPE_FRAMES: f32 = 0.9;

/// The furthest a friend will come to tow a sleeper, in creature widths.
const TOW_REACH_FRAMES: f32 = 6.0;

/// An unhurried pull, in points per second: slower than any companion walks.
pub(super) const TOW_SPEED: f32 = 16.0;

/// How long a friend may take to reach the sleeper before it gives up and the sleeper wriggles
/// over by itself.
const APPROACH_SECONDS: f32 = 6.0;

/// How long a friend stands with the rope let go before going back to what it was doing.
const LET_GO_SECONDS: f32 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TowPhase {
    /// Walking over to take up the rope.
    Approach,
    /// Pulling.
    Pull,
    /// The rope dropped, a moment before carrying on.
    LetGo,
}

#[derive(Clone, Copy, Debug)]
struct Tow {
    tower: CreatureId,
    sleeper: CreatureId,
    /// Where the sleeper is being taken.
    target_x: f32,
    /// Which way it is pulled: 1 to the right, -1 to the left.
    direction: f32,
    /// How far behind the friend the sleeper rides, in points.
    rope: f32,
    phase: TowPhase,
    elapsed: f32,
}

/// Every tow under way. Bounded and runtime only, like the overlap bookkeeping it serves.
#[derive(Default)]
pub(super) struct TowTable {
    slots: [Option<Tow>; MAX_TOWS],
}

impl TowTable {
    /// Whether this creature is towing somebody, at any stage of it: the tow walks it, and
    /// nothing else should.
    pub(super) fn towing(&self, creature: CreatureId) -> bool {
        self.slots.iter().flatten().any(|tow| tow.tower == creature)
    }

    /// Whether this creature is in a tow at either end.
    pub(super) fn involves(&self, creature: CreatureId) -> bool {
        self.slots
            .iter()
            .flatten()
            .any(|tow| tow.tower == creature || tow.sleeper == creature)
    }

    pub(super) fn clear(&mut self) {
        self.slots = [None; MAX_TOWS];
    }
}

impl World {
    /// Ask a friend to tow this sleeper over to `target_x`. False when nobody nearby is free to,
    /// or the rope would take either of them off the surface they are on, so the caller can let
    /// the sleeper wriggle over by itself instead.
    pub(super) fn start_tow(
        &mut self,
        sleeper_id: CreatureId,
        target_x: f32,
        desktop: &DesktopSnapshot,
    ) -> bool {
        if self.save.settings.reduce_motion {
            return false;
        }
        let Some(slot) = self.tows.slots.iter().position(Option::is_none) else {
            return false;
        };
        let Some(sleeper) = self.save.creatures.iter().find(|c| c.id == sleeper_id) else {
            return false;
        };
        let direction = if target_x >= sleeper.state.position.x {
            1.0
        } else {
            -1.0
        };
        let frame =
            spacing::creature_frame_width(sleeper, self.save.settings.display_scale, desktop);
        let rope = frame * ROPE_FRAMES;
        let (low, high) = surface_span(sleeper, desktop);
        // The friend stands a rope's length ahead of the sleeper, and finishes a rope's length
        // past where the sleeper is going; both have to be on the same ground.
        let grip = sleeper.state.position.x + direction * rope;
        let finish = target_x + direction * rope;
        if !(low..=high).contains(&grip) || !(low..=high).contains(&finish) {
            return false;
        }
        let affinity = |id: CreatureId| {
            self.save
                .relationships
                .iter()
                .find(|bond| bond.contains(id) && bond.contains(sleeper_id))
                .map_or(0, |bond| bond.affinity)
        };
        let tower = self
            .save
            .creatures
            .iter()
            .filter(|candidate| {
                candidate.id != sleeper_id
                    && candidate.state.arrival_delay_secs <= 0.0
                    && matches!(
                        candidate.state.action,
                        ActionKind::Idle | ActionKind::Traverse | ActionKind::Perch
                    )
                    && candidate.state.surface.monitor_id == sleeper.state.surface.monitor_id
                    && candidate.state.surface.window_key == sleeper.state.surface.window_key
                    && (candidate.state.position.x - sleeper.state.position.x).abs()
                        <= frame * TOW_REACH_FRAMES
                    && self.free_to_tow(candidate.id)
            })
            .max_by(|a, b| {
                let distance = |c: &Creature| (c.state.position.x - sleeper.state.position.x).abs();
                affinity(a.id)
                    .cmp(&affinity(b.id))
                    .then(distance(b).total_cmp(&distance(a)))
                    .then(b.id.cmp(&a.id))
            })
            .map(|creature| creature.id);
        let Some(tower) = tower else {
            return false;
        };
        self.action_choices.remove(&tower);
        self.bond_plans.remove(&tower);
        if let Some(creature) = self.save.creatures.iter_mut().find(|c| c.id == tower) {
            creature.state.action = ActionKind::Traverse;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = APPROACH_SECONDS * 2.0;
            creature.state.flourish = None;
        }
        self.tows.slots[slot] = Some(Tow {
            tower,
            sleeper: sleeper_id,
            target_x,
            direction,
            rope,
            phase: TowPhase::Approach,
            elapsed: 0.0,
        });
        true
    }

    /// Awake, on its own feet, and not wanted by anything else.
    fn free_to_tow(&self, id: CreatureId) -> bool {
        !self.tows.involves(id)
            && !self
                .interaction
                .as_ref()
                .is_some_and(|session| session.creature_id == id)
            && !self.tosses.contains_key(&id)
            && !self.window_journeys.contains_key(&id)
            && !self.attention.owns(id)
            && !self.bond_plans.contains_key(&id)
            && !self.bond_plans.values().any(|plan| plan.target == id)
            && !self.colony_plan.as_ref().is_some_and(|plan| {
                plan.participants
                    .iter()
                    .any(|participant| participant.creature_id == id)
            })
    }

    /// Walk every tow on by one tick: over to the sleeper, then the pull, then letting go.
    pub(super) fn advance_tows(&mut self, dt: f32, desktop: &DesktopSnapshot) {
        for index in 0..MAX_TOWS {
            let Some(mut tow) = self.tows.slots[index] else {
                continue;
            };
            tow.elapsed += dt;
            if !self.tow_still_holds(&tow) {
                self.end_tow(index, true, desktop);
                continue;
            }
            let (sleeper_x, sleeper_surface) = {
                let sleeper = self
                    .save
                    .creatures
                    .iter()
                    .find(|c| c.id == tow.sleeper)
                    .expect("a tow that still holds has its sleeper");
                (sleeper.state.position.x, sleeper.state.surface.clone())
            };
            let Some(tower) = self.save.creatures.iter_mut().find(|c| c.id == tow.tower) else {
                self.end_tow(index, true, desktop);
                continue;
            };
            if tower.state.surface.monitor_id != sleeper_surface.monitor_id
                || tower.state.surface.window_key != sleeper_surface.window_key
            {
                self.end_tow(index, true, desktop);
                continue;
            }
            let walk = 24.0 + tower.personality.activity * 34.0;
            match tow.phase {
                TowPhase::Approach => {
                    let grip = sleeper_x + tow.direction * tow.rope;
                    let dx = grip - tower.state.position.x;
                    let step = walk * dt;
                    if dx.abs() <= step {
                        tower.state.position.x = grip;
                        tower.state.velocity = Point::default();
                        tower.state.facing_right = tow.direction > 0.0;
                        tow.phase = TowPhase::Pull;
                        tow.elapsed = 0.0;
                    } else if tow.elapsed > APPROACH_SECONDS {
                        self.end_tow(index, true, desktop);
                        continue;
                    } else {
                        tower.state.position.x += dx.signum() * step;
                        tower.state.velocity = Point {
                            x: dx.signum() * walk,
                            y: 0.0,
                        };
                        tower.state.facing_right = dx > 0.0;
                    }
                }
                TowPhase::Pull => {
                    let remaining = tow.direction * (tow.target_x - sleeper_x);
                    let step = (TOW_SPEED * dt).min(remaining.max(0.0));
                    tower.state.position.x += tow.direction * step;
                    tower.state.velocity = Point {
                        x: tow.direction * TOW_SPEED,
                        y: 0.0,
                    };
                    tower.state.facing_right = tow.direction > 0.0;
                    // The pull is slower than a walk, and so are its steps.
                    tower.state.action_elapsed -= dt * (1.0 - TOW_SPEED / walk).max(0.0);
                    let towed_to = tower.state.position.x - tow.direction * tow.rope;
                    let arrived = remaining - step <= 0.25;
                    if let Some(sleeper) =
                        self.save.creatures.iter_mut().find(|c| c.id == tow.sleeper)
                    {
                        sleeper.state.position.x = if arrived { tow.target_x } else { towed_to };
                        sleeper.state.nudge =
                            (!arrived).then_some(SleepNudge::Towed { by: tow.tower });
                    }
                    if arrived {
                        if let Some(tower) =
                            self.save.creatures.iter_mut().find(|c| c.id == tow.tower)
                        {
                            tower.state.velocity = Point::default();
                            tower.state.action = ActionKind::Idle;
                            tower.state.action_elapsed = 0.0;
                            tower.state.action_duration = LET_GO_SECONDS + 1.0;
                        }
                        tow.phase = TowPhase::LetGo;
                        tow.elapsed = 0.0;
                    }
                }
                TowPhase::LetGo => {
                    tower.state.velocity = Point::default();
                    if tow.elapsed >= LET_GO_SECONDS {
                        self.tows.slots[index] = None;
                        continue;
                    }
                }
            }
            if let Some(creature) = self.save.creatures.iter_mut().find(|c| c.id == tow.tower) {
                constrain_to_surface(creature, desktop, &self.save.settings.habitat);
            }
            self.tows.slots[index] = Some(tow);
        }
    }

    /// Both ends are still there, still where they were, and nothing else has taken either.
    fn tow_still_holds(&self, tow: &Tow) -> bool {
        let sleeper_asleep = self
            .save
            .creatures
            .iter()
            .any(|c| c.id == tow.sleeper && c.state.action == ActionKind::Sleep);
        let taken = |id: CreatureId| {
            self.interaction
                .as_ref()
                .is_some_and(|session| session.creature_id == id)
                || self.tosses.contains_key(&id)
                || self.window_journeys.contains_key(&id)
                || self.attention.owns(id)
        };
        let tower_there = self.save.creatures.iter().any(|c| c.id == tow.tower);
        (sleeper_asleep || tow.phase == TowPhase::LetGo)
            && tower_there
            && !taken(tow.sleeper)
            && !taken(tow.tower)
            && !self.save.settings.reduce_motion
    }

    /// End one tow. Cut short, the friend stops where it is and the sleeper, if it is still
    /// asleep and still short of where it was going, wriggles the rest of the way itself.
    fn end_tow(&mut self, index: usize, cut_short: bool, desktop: &DesktopSnapshot) {
        let Some(tow) = self.tows.slots[index].take() else {
            return;
        };
        if let Some(tower) = self.save.creatures.iter_mut().find(|c| c.id == tow.tower)
            && tower.state.action == ActionKind::Traverse
            && self
                .interaction
                .as_ref()
                .is_none_or(|s| s.creature_id != tow.tower)
        {
            tower.state.velocity = Point::default();
            tower.state.action = ActionKind::Idle;
            tower.state.action_elapsed = 0.0;
            tower.state.action_duration = 1.0;
        }
        let Some(sleeper) = self.save.creatures.iter_mut().find(|c| c.id == tow.sleeper) else {
            return;
        };
        sleeper.state.nudge = None;
        let short = tow.direction * (tow.target_x - sleeper.state.position.x) > 0.5;
        if cut_short && short && sleeper.state.action == ActionKind::Sleep {
            let _ = desktop;
            self.wriggle_over(tow.sleeper, tow.target_x);
        }
    }

    /// Let go of every tow at once, where everybody stands, for the moments that settle the whole
    /// colony.
    pub(super) fn drop_all_tows(&mut self) {
        let sleepers: Vec<CreatureId> = self
            .tows
            .slots
            .iter()
            .flatten()
            .map(|t| t.sleeper)
            .collect();
        let towers: Vec<CreatureId> = self.tows.slots.iter().flatten().map(|t| t.tower).collect();
        self.tows.clear();
        for creature in &mut self.save.creatures {
            if sleepers.contains(&creature.id) {
                creature.state.nudge = None;
            }
            if towers.contains(&creature.id) && creature.state.action == ActionKind::Traverse {
                creature.state.velocity = Point::default();
                creature.state.action = ActionKind::Idle;
                creature.state.action_elapsed = 0.0;
                creature.state.action_duration = 1.0;
            }
        }
    }

    /// Forget any tow this creature was part of, when it leaves the colony.
    pub(super) fn forget_tows_of(&mut self, id: CreatureId) {
        for index in 0..MAX_TOWS {
            if self.tows.slots[index].is_some_and(|tow| tow.tower == id || tow.sleeper == id) {
                let tow = self.tows.slots[index].take().expect("just checked");
                let other = if tow.tower == id {
                    tow.sleeper
                } else {
                    tow.tower
                };
                if let Some(creature) = self.save.creatures.iter_mut().find(|c| c.id == other) {
                    creature.state.nudge = None;
                }
            }
        }
    }
}

/// The run of ground a creature is standing on, less half a frame at each end: the window's ledge
/// for a creature on one, and the display's usable width for one on the floor.
fn surface_span(creature: &Creature, desktop: &DesktopSnapshot) -> (f32, f32) {
    let ledge = creature.state.surface.window_key.and_then(|key| {
        desktop
            .windows
            .iter()
            .find(|window| window.key == key)
            .map(|window| (window.bounds.x, window.bounds.x + window.bounds.width))
    });
    let floor = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.id == creature.state.surface.monitor_id)
        .map(|monitor| {
            (
                monitor.usable_bounds.x,
                monitor.usable_bounds.x + monitor.usable_bounds.width,
            )
        });
    let (low, high) = ledge.or(floor).unwrap_or((f32::MIN, f32::MAX));
    let margin = CREATURE_ART_WIDTH / 2.0;
    (low + margin, high - margin)
}
