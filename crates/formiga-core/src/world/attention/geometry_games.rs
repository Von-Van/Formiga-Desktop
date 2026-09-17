//! Games about the shape of the desktop itself: a race across the windows, a round of
//! the-floor-is-lava along the ledges, and hide and seek behind whatever happens to stand in the
//! way. These consult window rectangles, exactly as every other behavior does, and never what any
//! window contains. Returning `false` winds the scene down.
use super::play::{PlayRole, Session};
use super::*;
use crate::topology::{RoutePreferences, TopologyRouteHop};

/// Creature-scale distance at which a seeker has found someone.
const FOUND: f32 = 34.0;
/// Seekers count for this long, which is also the hider's head start.
const COUNT_SECONDS: f32 = 2.6;
/// Standing this near the end of a ledge is standing at the edge of the lava.
const BRINK: f32 = 34.0;
/// A ledge needs this much air under it before the floor is worth staying off.
const LAVA_DROP: f32 = 90.0;
/// The widest gap any of these games asks a creature to cross on foot.
const REACH: f32 = 110.0;
/// How far along its own ledge one creature can keep track of another.
const SIGHT: f32 = 150.0;

impl World {
    /// Give a member the feeling its own game calls for. Expressive games keep it; the shared
    /// wind-down still overrides everyone at the end of a scene.
    fn feel(&mut self, id: CreatureId, emotion: AttentionEmotion) {
        if let Some(plan) = self.attention.plans.get_mut(&id) {
            plan.emotion = emotion;
        }
    }

    /// The route this creature would take from where it stands, by its own nerve, toward `hint`.
    /// An empty route means it cannot get anywhere it could actually walk and hop to.
    fn race_route(
        &self,
        c: &Creature,
        hint: Option<Point>,
        desktop: &DesktopSnapshot,
    ) -> Vec<TopologyRouteHop> {
        let Some(start) = c.state.surface.window_key else {
            return Vec::new();
        };
        let (max_rise, max_drop) = traversal_ability(c);
        let route = self.topology.plan_route(
            start,
            RoutePreferences {
                climbing: c.tendencies.climbing,
                exploration: c.tendencies.exploration,
                cursor_trust: c.tendencies.cursor_trust,
                target_hint: hint,
                max_rise,
                max_drop,
            },
        );
        // A racer may only run a route made of hops it could cross on foot, inside its habitat.
        let unit = creature_unit(c, &self.save.settings, desktop);
        let runnable = !route.is_empty()
            && route.iter().all(|hop| {
                let gap = (hop.to_bounds.x - hop.from_bounds.right())
                    .max(hop.from_bounds.x - hop.to_bounds.right());
                (0.0..=REACH * unit).contains(&gap)
                    && (hop.to_bounds.y - hop.from_bounds.y).abs() <= 36.0 * unit
                    && desktop.monitors.iter().any(|m| {
                        m.id == hop.monitor_id
                            && habitat_contains(&self.save.settings.habitat, m, hop.target)
                    })
            });
        if runnable { route } else { Vec::new() }
    }

    /// One finish line both racers agree on: the first one's own best route names it, and the
    /// other has to be able to get there too, by whatever route its own ability allows.
    pub(super) fn race_destination(
        &self,
        a: &Creature,
        b: &Creature,
        min_hops: usize,
        desktop: &DesktopSnapshot,
    ) -> Option<WindowKey> {
        // Route planning is a breadth-first search, and this is asked of every eligible pair while
        // an encounter builds. A race of `min_hops` needs that many ledges beyond the one they are
        // standing on, so count them first and plan only when a race could exist at all.
        let reachable = self
            .topology
            .windows()
            .iter()
            .filter(|w| {
                Some(w.key) != a.state.surface.window_key
                    && w.monitor_id == a.state.surface.monitor_id
            })
            .count();
        if reachable < min_hops.max(1) {
            return None;
        }
        // Aim at the window furthest along the desktop from where they stand; whatever route the
        // planner returns toward it is what this creature would actually run. A race is never
        // just the gap next door — that is a jumping contest, and it is offered first.
        let far = self
            .topology
            .windows()
            .iter()
            .filter(|w| Some(w.key) != a.state.surface.window_key)
            .max_by(|x, y| {
                let reach = |w: &&crate::topology::TopologyWindow| {
                    (w.bounds.x + w.bounds.width * 0.5 - a.state.position.x).abs()
                };
                reach(x).total_cmp(&reach(y))
            })
            .map(|w| Point {
                x: w.bounds.x + w.bounds.width * 0.5,
                y: w.bounds.y,
            })?;
        let first = self.race_route(a, Some(far), desktop);
        if first.len() < min_hops {
            return None;
        }
        let last = first.last()?;
        let (destination, hint) = (last.to_window, last.target);
        if Some(destination) == b.state.surface.window_key {
            return None;
        }
        let second = self.race_route(b, Some(hint), desktop);
        (second.last()?.to_window == destination).then_some(destination)
    }

    /// Everyone runs for the same window, each by the route its own nerve will take. The first
    /// one over the line wins; a finish line that disappears is replaced or the race is off.
    pub(super) fn advance_race(&mut self, s: &mut Session, desktop: &DesktopSnapshot) -> bool {
        self.rejoin_players(s, desktop);
        let ids: Vec<_> = s.members.iter().flatten().copied().collect();
        let standing = s.goal.filter(|key| {
            desktop
                .windows
                .iter()
                .take(MAX_TOPOLOGY_WINDOWS)
                .any(|w| w.key == *key && w.visible && !w.minimized)
        });
        if standing.is_none() {
            // No finish line yet, or it has gone: agree on another one, or call the race off.
            s.goal = ids.first().zip(ids.get(1)).and_then(|(&a, &b)| {
                let a = self.save.creatures.iter().find(|c| c.id == a)?;
                let b = self.save.creatures.iter().find(|c| c.id == b)?;
                self.race_destination(a, b, 1, desktop)
            });
        }
        let Some(finish) = s.goal.and_then(|key| {
            desktop
                .windows
                .iter()
                .take(MAX_TOPOLOGY_WINDOWS)
                .find(|w| w.key == key && w.visible && !w.minimized)
                .map(|w| w.bounds)
        }) else {
            return false;
        };
        let destination = s.goal.unwrap();
        let line = Point {
            x: finish.x + finish.width * 0.5,
            y: finish.y,
        };
        let mut winner = None;
        let mut running = false;
        for id in ids.iter().copied() {
            let Some(c) = self.save.creatures.iter().find(|c| c.id == id) else {
                continue;
            };
            if c.state.surface.window_key == Some(destination) {
                winner = winner.or(Some(id));
                continue;
            }
            // Mid-hop: that jump is its own scene for a moment, with its own stages.
            if self
                .attention
                .plans
                .get(&id)
                .is_none_or(|p| !matches!(p.role, Role::Play { .. }))
            {
                running = true;
                continue;
            }
            let unit = creature_unit(c, &self.save.settings, desktop);
            let Some(hop) = self.race_route(c, Some(line), desktop).first().copied() else {
                // No way on from here: this racer is out of it, and watches the rest finish.
                self.steer_play(id, None, ActionKind::InspectScreen);
                self.look_at(id, line);
                continue;
            };
            let right = hop.to_bounds.x >= hop.from_bounds.right();
            let direction = if right { 1.0 } else { -1.0 };
            let edge = Point {
                x: if right {
                    hop.from_bounds.right()
                } else {
                    hop.from_bounds.x
                },
                y: hop.from_bounds.y,
            };
            match self
                .gap_candidate(c, desktop)
                .filter(|(journey, _, _)| journey.hop.surface.window_key == Some(hop.to_window))
            {
                Some((journey, decision, drop)) => {
                    running = true;
                    self.attention.plans.remove(&id);
                    match decision {
                        gaps::GapDecision::Commit => {
                            self.begin_gap_attention(id, journey, true, drop, desktop)
                        }
                        gaps::GapDecision::Hesitate => {
                            self.begin_hesitation(id, journey, drop, desktop)
                        }
                        gaps::GapDecision::Refuse => {
                            self.begin_gap_attention(id, journey, false, drop, desktop)
                        }
                    }
                }
                // Not at the edge yet: run for it.
                None => {
                    let goal = self.step_toward(id, edge.x - direction * 16.0 * unit, desktop);
                    running |= goal.is_some();
                    self.steer_play(id, goal, ActionKind::Traverse);
                    self.look_at(id, edge);
                }
            }
        }
        let Some(champion) = winner else {
            return running || s.elapsed < 4.0;
        };
        // Over the line: a moment of delight, and the rest of the field cheers it in.
        let focus = self.play_position(champion).unwrap_or(line);
        for id in ids {
            let gesture = if id == champion {
                ActionKind::Greet
            } else {
                ActionKind::InspectScreen
            };
            self.steer_play(id, None, gesture);
            self.look_at(id, focus);
        }
        false
    }

    /// How far the floor is below this creature's own ledge.
    pub(super) fn lava_drop(&self, c: &Creature, desktop: &DesktopSnapshot) -> f32 {
        super::super::surfaces::drop_below(c, c.state.position.x, desktop, &self.save.settings)
    }

    /// Whether this creature is standing close enough to the end of its ledge to be uneasy.
    pub(super) fn at_the_brink(&self, c: &Creature, desktop: &DesktopSnapshot) -> bool {
        let Some(bounds) = desktop
            .windows
            .iter()
            .take(MAX_TOPOLOGY_WINDOWS)
            .find(|w| Some(w.key) == c.state.surface.window_key && w.visible && !w.minimized)
            .map(|w| w.bounds)
        else {
            return false;
        };
        let unit = creature_unit(c, &self.save.settings, desktop);
        (c.state.position.x - bounds.x).min(bounds.right() - c.state.position.x) <= BRINK * unit
    }

    /// The floor is lava: keep to the ledges, cross what can be crossed, and be visibly unhappy
    /// about the edge. Nothing here prevents a fall — touching the floor simply ends the round.
    pub(super) fn advance_lava(&mut self, s: &mut Session, desktop: &DesktopSnapshot) -> bool {
        self.rejoin_players(s, desktop);
        let ids: Vec<_> = s.members.iter().flatten().copied().collect();
        let mut grounded = None;
        let mut aloft = 0;
        for (index, id) in ids.iter().copied().enumerate() {
            let Some(c) = self.save.creatures.iter().find(|c| c.id == id) else {
                continue;
            };
            let Some(bounds) = c.state.surface.window_key.and_then(|key| {
                desktop
                    .windows
                    .iter()
                    .take(MAX_TOPOLOGY_WINDOWS)
                    .find(|w| w.key == key && w.visible && !w.minimized)
                    .map(|w| w.bounds)
            }) else {
                // Down in the lava. Safety always permitted the landing; the round is what ends.
                grounded = grounded.or(Some(id));
                continue;
            };
            aloft += 1;
            // Mid-hop: the jump publishes its own stages, and watchers react to the save.
            if self
                .attention
                .plans
                .get(&id)
                .is_none_or(|p| !matches!(p.role, Role::Play { .. }))
            {
                continue;
            }
            let unit = creature_unit(c, &self.save.settings, desktop);
            let position = c.state.position;
            let from_edge = (position.x - bounds.x).min(bounds.right() - position.x);
            if from_edge <= BRINK * unit {
                // At the end of the ledge: cross to the next one, or back away from the brink.
                if let Some((journey, decision, drop)) = self.gap_candidate(c, desktop) {
                    self.attention.plans.remove(&id);
                    match decision {
                        gaps::GapDecision::Commit => {
                            self.begin_gap_attention(id, journey, true, drop, desktop)
                        }
                        _ => self.begin_hesitation(id, journey, drop, desktop),
                    }
                    continue;
                }
                let inward = if position.x - bounds.x < bounds.right() - position.x {
                    1.0
                } else {
                    -1.0
                };
                let goal = self.step_toward(id, position.x + inward * BRINK * 2.0 * unit, desktop);
                self.steer_play(
                    id,
                    goal,
                    if goal.is_some() {
                        ActionKind::Traverse
                    } else {
                        ActionKind::Perch
                    },
                );
                self.look_at(
                    id,
                    Point {
                        x: position.x - inward * BRINK * unit,
                        y: bounds.y + 70.0 * unit,
                    },
                );
                self.feel(id, AttentionEmotion::Concerned);
                continue;
            }
            // Out on the safe middle of the ledge: keep moving, and keep checking what is below.
            let heading = if (s.seed ^ index as u64).is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            let goal = [heading, -heading]
                .into_iter()
                .find_map(|direction| self.travel_goal(id, position, direction, unit, desktop));
            self.steer_play(
                id,
                goal,
                if goal.is_some() {
                    ActionKind::Traverse
                } else {
                    ActionKind::Perch
                },
            );
            self.look_at(
                id,
                Point {
                    x: goal.map_or(position.x, |g| g.x),
                    y: bounds.y + 70.0 * unit,
                },
            );
            self.feel(id, AttentionEmotion::Enjoying);
        }
        let Some(fallen) = grounded else {
            return aloft > 0;
        };
        let Some(point) = self.play_position(fallen) else {
            return false;
        };
        for id in ids {
            self.steer_play(id, None, ActionKind::InspectScreen);
            self.look_at(id, point);
            self.feel(
                id,
                if id == fallen {
                    AttentionEmotion::Averting
                } else {
                    AttentionEmotion::Relieved
                },
            );
        }
        false
    }

    /// Whether a seeker can actually see someone: the same ledge, close enough, and turned that
    /// way. A creature is never hidden by standing where the person at the desk cannot see it —
    /// this world keeps every creature in plain view — so hiding here means getting out of a
    /// companion's line of sight, around the far corner of a surface.
    fn in_sight(seeker: &Creature, hidden: &Creature, unit: f32) -> bool {
        seeker.state.surface.monitor_id == hidden.state.surface.monitor_id
            && seeker.state.surface.window_key == hidden.state.surface.window_key
            && (seeker.state.position.x - hidden.state.position.x).abs() <= SIGHT * unit
            && seeker.state.facing_right == (hidden.state.position.x > seeker.state.position.x)
    }

    /// A corner of this creature's own surface, as far from `from` as the surface allows. `None`
    /// means there is nowhere on it worth going: the ledge is too short to get away along.
    pub(super) fn hiding_spot(
        &self,
        c: &Creature,
        from: Point,
        desktop: &DesktopSnapshot,
    ) -> Option<Point> {
        let bounds = desktop
            .windows
            .iter()
            .take(MAX_TOPOLOGY_WINDOWS)
            .find(|w| Some(w.key) == c.state.surface.window_key && w.visible && !w.minimized)
            .map(|w| w.bounds)?;
        let unit = creature_unit(c, &self.save.settings, desktop);
        let margin = BRINK * unit;
        if bounds.width <= margin * 2.0 {
            return None;
        }
        let head = head_point(c, &self.save.settings, desktop);
        let support = c.state.surface.window_key;
        (0u8..=8)
            .map(f32::from)
            .map(|step| bounds.x + margin + (bounds.width - margin * 2.0) * (step / 8.0))
            .filter(|x| (x - from.x).abs() > SIGHT * unit)
            .filter(|x| point_exposed(Point { x: *x, y: head.y }, support, desktop))
            .map(|x| Point {
                x,
                y: c.state.position.y,
            })
            .max_by(|a, b| (a.x - from.x).abs().total_cmp(&(b.x - from.x).abs()))
    }

    /// Hide and seek. The seekers count while the hider makes for a corner, and the search that
    /// follows uses only where the hider was last actually in view, never where it is now.
    pub(super) fn advance_hide_and_seek(
        &mut self,
        s: &mut Session,
        desktop: &DesktopSnapshot,
    ) -> bool {
        let Some((_, hider)) = s.with_role(PlayRole::Lead).next() else {
            return false;
        };
        let seekers: Vec<_> = s.with_role(PlayRole::Follow).map(|(_, id)| id).collect();
        let Some(&first) = seekers.first() else {
            return false;
        };
        let Some(creature) = self.save.creatures.iter().find(|c| c.id == hider) else {
            return false;
        };
        let Some(looking) = self.play_position(first) else {
            return false;
        };
        let hidden = creature.state.position;
        let unit = creature_unit(creature, &self.save.settings, desktop);
        let counting = s.elapsed < COUNT_SECONDS;
        // The only thing a seeker knows is where the hider was while it could still see it, and
        // a counting seeker is facing the other way.
        let seen_now = !counting
            && seekers.iter().any(|id| {
                self.save
                    .creatures
                    .iter()
                    .find(|c| c.id == *id)
                    .is_some_and(|seeker| Self::in_sight(seeker, creature, unit))
            });
        if s.trail[1].is_none() || seen_now {
            s.trail[1] = Some(hidden);
        }
        // Close enough to have found them, once the counting is over.
        if !counting
            && seekers
                .iter()
                .filter_map(|id| self.play_position(*id))
                .any(|point| point.distance(hidden) <= FOUND * unit)
        {
            for id in seekers.iter().copied() {
                self.steer_play(id, None, ActionKind::Greet);
                self.feel(id, AttentionEmotion::Enjoying);
                self.look_at(id, hidden);
            }
            self.steer_play(hider, None, ActionKind::Greet);
            self.feel(hider, AttentionEmotion::Enjoying);
            self.look_at(hider, looking);
            return false;
        }
        // The hider keeps making for its chosen corner, in ordinary short steps.
        let spot = match s.trail[0] {
            Some(spot) => Some(spot),
            None => {
                s.trail[0] = self.hiding_spot(creature, looking, desktop);
                s.trail[0]
            }
        };
        let Some(spot) = spot else {
            return false;
        };
        let arrived = (hidden.x - spot.x).abs() <= 20.0 * unit;
        let goal = (!arrived)
            .then(|| self.step_toward(hider, spot.x, desktop))
            .flatten();
        self.steer_play(
            hider,
            goal,
            if goal.is_some() {
                ActionKind::Traverse
            } else {
                ActionKind::Perch
            },
        );
        self.look_at(hider, if arrived { looking } else { spot });
        self.feel(
            hider,
            if arrived {
                AttentionEmotion::Enjoying
            } else {
                AttentionEmotion::Curious
            },
        );
        for id in seekers {
            let Some(position) = self.play_position(id) else {
                continue;
            };
            if counting {
                // Turned away and counting. Nothing about the hider is consulted here at all.
                self.steer_play(id, None, ActionKind::InspectScreen);
                self.look_at(
                    id,
                    Point {
                        x: position.x + (position.x - hidden.x).signum() * SIGHT * unit,
                        y: position.y,
                    },
                );
                self.feel(id, AttentionEmotion::Curious);
                continue;
            }
            // Search: make for the last place the hider was actually in view, then look past it.
            let Some(seen) = s.trail[1] else { continue };
            let reached = (seen.x - position.x).abs() <= 28.0 * unit;
            let sweep = if reached {
                seen.x + (seen.x - position.x).signum() * REACH * unit
            } else {
                seen.x
            };
            let goal = self
                .step_toward(id, sweep, desktop)
                .or_else(|| self.step_toward(id, seen.x, desktop));
            self.steer_play(
                id,
                goal,
                if goal.is_some() {
                    ActionKind::Traverse
                } else {
                    ActionKind::InspectScreen
                },
            );
            self.look_at(id, goal.unwrap_or(seen));
            self.feel(id, AttentionEmotion::Curious);
        }
        true
    }

    /// Whether this pair is somewhere worth playing the floor-is-lava: a ledge with real air
    /// under it, and one of them already standing at the edge of it.
    pub(super) fn lava_ready(&self, a: &Creature, b: &Creature, desktop: &DesktopSnapshot) -> bool {
        let unit = creature_unit(a, &self.save.settings, desktop);
        a.state.surface.window_key.is_some()
            && self.lava_drop(a, desktop) > LAVA_DROP * unit
            && (self.at_the_brink(a, desktop) || self.at_the_brink(b, desktop))
    }
}

#[cfg(test)]
mod tests {
    use super::super::play::Kind;
    use super::super::play::tests::{Audience, everyone_settled, scene, tick};
    use super::*;

    /// Add a window beside the colony's ledge, the shape these games need.
    fn add_window(d: &mut DesktopSnapshot, key: WindowKey, bounds: DesktopRect, z_order: u32) {
        let mut window = d.windows[0].clone();
        window.key = key;
        window.bounds = bounds;
        window.z_order = z_order;
        d.windows.push(window);
    }

    /// The pair one of these games is about. Any `watching` companions are seated on the floor
    /// below: near enough to see the ledge, far enough down that they can never pair off with
    /// anyone on it, and spread too widely to start a game of their own.
    fn quiet_pair(w: &mut World, watching: usize) {
        for c in w.save.creatures.iter_mut() {
            c.personality.playfulness = 1.0;
            c.personality.sociability = 0.9;
            c.personality.boldness = 1.0;
            c.personality.window_tolerance = 1.0;
            c.state.drives.energy = 0.9;
            // Keep the settling-beside-a-companion scene from claiming the pair first.
            c.state.drives.social_need = 0.0;
        }
        w.save.creatures.truncate(2 + watching);
        for (index, c) in w.save.creatures.iter_mut().enumerate().skip(2) {
            floor_seat(c, index);
        }
    }

    /// Sit one companion on the floor under the ledge, where it can watch and nothing more.
    fn floor_seat(c: &mut Creature, index: usize) {
        c.state.surface = SurfaceAttachment {
            kind: SurfaceKind::ScreenFloor,
            monitor_id: 1,
            window_key: None,
            relative_x: 0.5,
        };
        c.state.position = Point {
            x: 380.0 + (index - 2) as f32 * 210.0,
            y: 846.0,
        };
        c.state.action = ActionKind::Idle;
        c.state.action_elapsed = 0.0;
        c.state.action_duration = 100.0;
        c.state.facing_right = true;
    }

    /// A chain of three ledges, twenty points apart, with the pair back on the first one once the
    /// new windows have stopped being news.
    fn race_scene(watching: usize) -> (World, DesktopSnapshot, OffsetDateTime) {
        let (mut w, mut d, now) = scene(false);
        quiet_pair(&mut w, watching);
        add_window(
            &mut d,
            702,
            DesktopRect {
                x: 820.0,
                y: 600.0,
                width: 220.0,
                height: 200.0,
            },
            0,
        );
        add_window(
            &mut d,
            703,
            DesktopRect {
                x: 1060.0,
                y: 600.0,
                width: 220.0,
                height: 200.0,
            },
            0,
        );
        for step in 1..80 {
            tick(&mut w, &mut d, now, step);
        }
        w.clear_attention();
        w.window_journeys.clear();
        w.bond_plans.clear();
        w.attention.play.cooldown = 0.0;
        w.attention.colony_cooldown = 0.0;
        for (index, c) in w.save.creatures.iter_mut().enumerate() {
            c.state.drives = Drives::default();
            c.state.drives.energy = 0.9;
            c.state.drives.social_need = 0.0;
            c.state.action_elapsed = 0.0;
            c.state.action_duration = 100.0;
            c.state.facing_right = true;
            // The audience takes its seat again: the new windows are old news by now.
            if index >= 2 {
                floor_seat(c, index);
                continue;
            }
            c.state.surface = SurfaceAttachment {
                kind: SurfaceKind::WindowLedge,
                monitor_id: 1,
                window_key: Some(701),
                relative_x: (440.0 + index as f32 * 60.0) / 600.0,
            };
            c.state.position = Point {
                x: 640.0 + index as f32 * 60.0,
                y: 600.0,
            };
            c.state.action = ActionKind::Perch;
        }
        (w, d, now)
    }

    #[test]
    fn two_racers_run_their_own_routes_to_one_finish_line_and_someone_wins() {
        let (mut w, mut d, now) = race_scene(2);
        assert_eq!(
            w.race_destination(&w.save.creatures[0], &w.save.creatures[1], 2, &d),
            Some(703),
            "both of them can reach the far ledge"
        );
        let racers: Vec<CreatureId> = w.save.creatures.iter().take(2).map(|c| c.id).collect();
        let mut audience = Audience::default();
        let mut raced = false;
        let mut destination = None;
        let mut finished = false;
        for step in 80..700 {
            tick(&mut w, &mut d, now, step);
            if let Some(s) = w.attention.play.session.filter(|s| s.kind == Kind::Race) {
                raced = true;
                audience.note(&w);
                destination = destination.or(s.goal);
                if let (Some(goal), Some(first)) = (s.goal, destination) {
                    assert_eq!(goal, first, "the finish line does not move under them");
                }
            } else if raced {
                finished = true;
                assert!(everyone_settled(&w), "{audience:?}");
                break;
            }
        }
        assert!(raced && finished, "{raced} {finished}");
        assert_eq!(destination, Some(703));
        assert!(
            w.save
                .creatures
                .iter()
                .take(2)
                .any(|c| c.state.surface.window_key != Some(701)),
            "at least one racer left the starting ledge"
        );
        assert!(w.attention.plans.is_empty());
        // The race is run to an audience on the floor below, and their eyes run along with it.
        assert!(
            audience.strayed.is_empty()
                && audience.conscripted.is_empty()
                && audience.blank.is_empty(),
            "{audience:?}"
        );
        // A racer taking a gap is its own scene for that moment; nobody watching leaves the one
        // origin the race itself was published under.
        assert!(
            audience.split.iter().all(|id| racers.contains(id)),
            "{audience:?}"
        );
        let watchers = audience.watchers();
        assert_eq!(watchers.len(), 2, "{audience:?}");
        for id in watchers {
            let looked = audience.looked_at(id);
            assert!(
                looked.len() >= 6 && looked.last() > looked.first(),
                "a watcher's gaze runs up the ledge with them: {looked:?}"
            );
            // A race lasts sixteen seconds and a watcher's interest in one lasts nine, so the
            // audience is released while the runners are still on the first ledge: it is curious
            // about the running and never sees the running end. When a race learns to hold its
            // audience to the finish line, this is the feeling that should change.
            assert_eq!(
                audience.felt_by(id),
                [AttentionEmotion::Curious],
                "{audience:?}"
            );
        }
    }

    #[test]
    fn a_race_whose_finish_line_disappears_is_replanned_or_called_off() {
        let (mut w, mut d, now) = race_scene(0);
        let mut closed = false;
        for step in 80..700 {
            tick(&mut w, &mut d, now, step);
            let Some(s) = w.attention.play.session.filter(|s| s.kind == Kind::Race) else {
                continue;
            };
            if !closed && s.goal == Some(703) {
                // The finish line closes mid-race.
                d.windows.retain(|window| window.key != 703);
                closed = true;
                continue;
            }
            if closed {
                assert_ne!(s.goal, Some(703), "a closed window is not a finish line");
            }
        }
        assert!(closed, "the race reached a finish line before it closed");
        assert!(w.attention.play.session.is_none());
        assert!(
            !w.attention
                .plans
                .values()
                .any(|p| matches!(p.role, Role::Play { .. }))
        );
    }

    #[test]
    fn the_floor_is_lava_keeps_to_the_ledge_and_shows_it_near_the_edge() {
        let (mut w, mut d, now) = scene(false);
        // Two on the ledge and two companions on the floor below, watching from the lava.
        quiet_pair(&mut w, 2);
        // Both of them out at the right-hand end of the ledge, with a long drop below.
        for (index, c) in w.save.creatures.iter_mut().enumerate().take(2) {
            c.state.position.x = 780.0 - index as f32 * 60.0;
            c.state.surface.relative_x = (c.state.position.x - 200.0) / 600.0;
            c.state.facing_right = index == 0;
        }
        assert!(w.lava_drop(&w.save.creatures[0], &d) > LAVA_DROP);
        let mut played = false;
        let mut reluctant = false;
        let mut backed_away = false;
        let mut audience = Audience::default();
        for step in 1..400 {
            tick(&mut w, &mut d, now, step);
            audience.note(&w);
            if w.attention
                .play
                .session
                .is_some_and(|s| s.kind == Kind::Lava)
            {
                played = true;
                reluctant |= w.save.creatures.iter().any(|c| {
                    c.state
                        .attention
                        .is_some_and(|p| p.emotion == AttentionEmotion::Concerned)
                });
                backed_away |= w
                    .save
                    .creatures
                    .iter()
                    .take(2)
                    .all(|c| !w.at_the_brink(c, &d) && c.state.surface.window_key == Some(701));
            }
        }
        assert!(played && reluctant, "{played} {reluctant}");
        assert!(backed_away, "the pair moves in off the edge");
        // The companions below are an audience, not players: they watch, and they are never
        // drawn into a round of a game about staying off the floor they are standing on.
        assert!(audience.coherent(), "{audience:?}");
        for id in audience.watchers() {
            assert!(
                !audience.felt_by(id).is_empty(),
                "a watcher of the round says something about it: {audience:?}"
            );
        }
        assert!(
            w.save
                .creatures
                .iter()
                .take(2)
                .all(|c| c.state.surface.window_key == Some(701)),
            "no one is pushed off the ledge by the game"
        );
        assert!(w.attention.play.session.is_none());
    }

    #[test]
    fn touching_the_floor_ends_the_round_and_the_others_react() {
        let (mut w, mut d, now) = scene(false);
        quiet_pair(&mut w, 0);
        for (index, c) in w.save.creatures.iter_mut().enumerate() {
            c.state.position.x = 780.0 - index as f32 * 60.0;
            c.state.surface.relative_x = (c.state.position.x - 200.0) / 600.0;
        }
        let mut started = false;
        let mut reacted = false;
        for step in 1..400 {
            tick(&mut w, &mut d, now, step);
            let playing = w
                .attention
                .play
                .session
                .is_some_and(|s| s.kind == Kind::Lava);
            if playing && !started {
                started = true;
                // One of them ends up on the floor: the round is over, not the creature.
                let c = &mut w.save.creatures[1];
                c.state.surface = SurfaceAttachment {
                    kind: SurfaceKind::ScreenFloor,
                    monitor_id: 1,
                    window_key: None,
                    relative_x: 0.5,
                };
                c.state.position = Point { x: 760.0, y: 846.0 };
                continue;
            }
            if started {
                reacted |= w.save.creatures.iter().any(|c| {
                    c.state.attention.is_some_and(|p| {
                        matches!(
                            p.emotion,
                            AttentionEmotion::Relieved | AttentionEmotion::Averting
                        )
                    })
                });
            }
        }
        assert!(started && reacted, "{started} {reacted}");
        assert!(w.attention.play.session.is_none());
        assert!(w.save.creatures[1].state.drives.energy > 0.0);
    }

    /// Two companions on one long ledge, far enough apart to lose sight of one another, with any
    /// `watching` companions on the floor below.
    fn hiding_scene(watching: usize) -> (World, DesktopSnapshot, OffsetDateTime) {
        let (mut w, d, now) = scene(false);
        quiet_pair(&mut w, watching);
        for (index, c) in w.save.creatures.iter_mut().enumerate() {
            c.personality.curiosity = 0.9;
            if index >= 2 {
                continue;
            }
            c.state.position.x = 300.0 + index as f32 * 160.0;
            c.state.surface.relative_x = (c.state.position.x - 200.0) / 600.0;
            c.state.facing_right = true;
        }
        (w, d, now)
    }

    #[test]
    fn a_hider_makes_for_a_corner_and_a_seeker_searches_where_it_last_had_them_in_view() {
        let (mut w, mut d, now) = hiding_scene(2);
        let spot = w.hiding_spot(&w.save.creatures[0], w.save.creatures[1].state.position, &d);
        assert!(spot.is_some(), "the ledge is long enough to get away along");
        let (hider, seeker) = (w.save.creatures[0].id, w.save.creatures[1].id);
        let mut audience = Audience::default();
        let mut played = false;
        let mut counted = false;
        let mut searched = false;
        let mut moved_off = false;
        let mut memories = BTreeSet::new();
        let mut was_in_sight = false;
        for step in 1..600 {
            tick(&mut w, &mut d, now, step);
            let Some(s) = w
                .attention
                .play
                .session
                .filter(|s| s.kind == Kind::HideAndSeek)
            else {
                continue;
            };
            played = true;
            audience.note(&w);
            let hider_at = w.save.creatures.iter().find(|c| c.id == hider).unwrap();
            let seeker_at = w.save.creatures.iter().find(|c| c.id == seeker).unwrap();
            let unit = creature_unit(hider_at, &w.save.settings, &d);
            let in_sight = World::in_sight(seeker_at, hider_at, unit);
            if let Some(seen) = s.trail[1] {
                let moved = memories.insert(seen.x as i32);
                // A remembered position only ever changes while the hider is actually in view.
                assert!(
                    !moved || in_sight || was_in_sight || memories.len() == 1,
                    "the memory is a position, not a live feed: {seen:?}"
                );
            }
            was_in_sight = in_sight;
            if s.elapsed < COUNT_SECONDS {
                counted = true;
                assert!(
                    w.attention
                        .plans
                        .get(&seeker)
                        .is_none_or(|p| p.walk.is_none()),
                    "a counting seeker stays put"
                );
                assert!(memories.len() <= 1, "counting learns nothing new");
            } else {
                searched |= w
                    .attention
                    .plans
                    .get(&seeker)
                    .is_some_and(|p| p.walk.is_some());
                moved_off |= (hider_at.state.position.x - 300.0).abs() > 60.0;
            }
        }
        assert!(played && counted && searched && moved_off);
        assert!(
            memories.len() > 1,
            "the seeker's memory does refresh while it can see them: {memories:?}"
        );
        assert!(w.attention.play.session.is_none());
        assert!(w.attention.plans.is_empty());
        // The game is played to an audience on the floor, and the audience never gives the hider
        // away. Each watcher is pointed at whoever opened the scene for the one tick before the
        // game names the seeker, which is well inside the moment it takes to look up at all, so
        // what anyone can actually see of the audience is two companions following the search.
        assert!(audience.coherent(), "{audience:?}");
        let watchers = audience.watchers();
        assert_eq!(watchers.len(), 2, "{audience:?}");
        for id in watchers {
            assert_eq!(audience.followed[&id], vec![hider, seeker], "{audience:?}");
            let looked = audience.looked_at(id);
            assert!(looked.len() >= 4, "the gaze searches too: {looked:?}");
            let felt = audience.felt_by(id);
            assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
            assert!(felt.len() >= 2, "and the finding lands on them: {felt:?}");
        }
        assert!(everyone_settled(&w), "{audience:?}");
    }

    #[test]
    fn hide_and_seek_ends_whether_or_not_anyone_is_found() {
        let (mut w, mut d, now) = hiding_scene(0);
        let mut longest = 0.0f32;
        let mut sessions = 0;
        let mut playing = false;
        for step in 1..900 {
            tick(&mut w, &mut d, now, step);
            match w
                .attention
                .play
                .session
                .filter(|s| s.kind == Kind::HideAndSeek)
            {
                Some(s) => {
                    longest = longest.max(s.elapsed);
                    if !playing {
                        sessions += 1;
                        playing = true;
                    }
                }
                None => playing = false,
            }
        }
        assert!(sessions >= 1);
        assert!(longest <= 20.2, "a round always ends: {longest}");
        assert!(w.attention.play.session.is_none());
    }

    /// Space is claimed honestly: nobody walks to a spot another creature has taken or reserved,
    /// and nobody aims a landing at one either. There is no collision solver behind this — the
    /// claims are made when a goal is chosen and simply refused when the spot is spoken for.
    #[test]
    fn players_never_claim_a_spot_another_creature_has_taken_or_reserved() {
        let (mut w, mut d, now) = race_scene(0);
        let mut checked = 0;
        for step in 80..700 {
            tick(&mut w, &mut d, now, step);
            if w.attention.play.session.is_none() {
                continue;
            }
            checked += 1;
            let claims: Vec<(CreatureId, Point)> = w
                .save
                .creatures
                .iter()
                .filter(|c| c.state.arrival_delay_secs <= 0.0)
                .map(|c| {
                    let claim = w
                        .attention
                        .plans
                        .get(&c.id)
                        .and_then(|p| p.walk)
                        .map_or(c.state.position, |walk| walk.destination);
                    (c.id, claim)
                })
                .collect();
            for (index, (id, claim)) in claims.iter().enumerate() {
                for (other, elsewhere) in &claims[index + 1..] {
                    if (claim.y - elsewhere.y).abs() >= 8.0 {
                        continue;
                    }
                    assert!(
                        (claim.x - elsewhere.x).abs() >= 12.0,
                        "{id} and {other} both claim {claim:?}"
                    );
                }
                // A hop aims clear of everyone who is not the one taking it.
                for (jumper, journey) in &w.window_journeys {
                    if jumper == id {
                        continue;
                    }
                    let landing = motion::landing_point(journey);
                    assert!(
                        landing.distance(*claim) >= 12.0,
                        "a landing is aimed at {claim:?}, which {id} has claimed"
                    );
                }
            }
        }
        assert!(checked > 40, "the scene ran long enough to mean something");
    }

    #[test]
    fn reduced_motion_starts_none_of_the_geometry_games() {
        for kind in 0..3 {
            let (mut w, mut d, now) = if kind == 2 {
                hiding_scene(0)
            } else if kind == 0 {
                race_scene(0)
            } else {
                let (mut w, mut d, now) = scene(false);
                quiet_pair(&mut w, 0);
                if false {
                    add_window(
                        &mut d,
                        702,
                        DesktopRect {
                            x: 820.0,
                            y: 600.0,
                            width: 220.0,
                            height: 200.0,
                        },
                        0,
                    );
                    add_window(
                        &mut d,
                        703,
                        DesktopRect {
                            x: 1060.0,
                            y: 600.0,
                            width: 220.0,
                            height: 200.0,
                        },
                        0,
                    );
                }
                for (index, c) in w.save.creatures.iter_mut().enumerate() {
                    c.state.position.x = 700.0 + index as f32 * 60.0;
                    c.state.surface.relative_x = (c.state.position.x - 200.0) / 600.0;
                    c.state.facing_right = true;
                }
                (w, d, now)
            };
            w.save.settings.reduce_motion = true;
            for step in 1..200 {
                tick(&mut w, &mut d, now, step);
                assert!(
                    !w.attention.play.session.is_some_and(|s| matches!(
                        s.kind,
                        Kind::Race | Kind::Lava | Kind::HideAndSeek
                    )),
                    "kind {kind}"
                );
            }
        }
    }
}
