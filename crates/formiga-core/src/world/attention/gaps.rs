//! Short gap attempts reuse window journeys and the same one-origin audience as curiosity.
use super::super::surfaces::drop_below;
use super::*;

/// Caution is a valid outcome: a clear margin commits, a borderline one hesitates first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GapDecision {
    Commit,
    Hesitate,
    Refuse,
}

/// Confidence below risk by less than this prepares and reconsiders instead of refusing outright.
const HESITATION_MARGIN: f32 = 0.15;

pub(in super::super) fn gap_step_safe(
    journey: &WindowJourney,
    creature: &Creature,
    point: Point,
    desktop: &DesktopSnapshot,
    settings: &Settings,
    neighbors: &[Creature],
) -> bool {
    let WindowJourney::Gap(gap) = journey else {
        return true;
    };
    let scale = desktop
        .monitors
        .iter()
        .find(|m| m.id == creature.state.surface.monitor_id)
        .map_or(1.0, |m| {
            (f32::from(settings.display_scale) / m.scale_factor).clamp(0.5, 2.0)
        });
    point_exposed(
        Point {
            x: point.x,
            y: point.y + (-24.0 + gap_hanging(gap) * 36.0) * scale,
        },
        if gap.catch && gap.elapsed >= gap.preparation + gap.hop.duration {
            gap.hop.surface.window_key
        } else {
            None
        },
        desktop,
    ) && !neighbors.iter().any(|c| {
        c.id != creature.id
            && c.state.arrival_delay_secs <= 0.0
            && ((Some(c.id) != gap.helper
                && c.state.position.distance(gap.hop.target) < 40.0 * scale)
                || c.state.position.distance(point) < 28.0 * scale)
    })
}

impl World {
    pub(super) fn promote_window_journey(&mut self, desktop: &DesktopSnapshot) {
        if self.attention.colony_cooldown > 0.0 {
            return;
        }
        let candidate = self
            .window_journeys
            .iter()
            .filter_map(|(&id, j)| {
                if self.attention.cooldowns.contains_key(&id) {
                    return None;
                }
                let c = self.save.creatures.iter().find(|c| c.id == id)?;
                if !point_exposed(
                    head_point(c, &self.save.settings, desktop),
                    c.state.surface.window_key,
                    desktop,
                ) {
                    return None;
                }
                let (elapsed, seconds, rewarding) = match j {
                    WindowJourney::Gap(_) => return None,
                    WindowJourney::Climb(g) => (
                        g.elapsed,
                        g.approach_duration + g.climb_duration + g.mantle_duration,
                        (g.start.y - g.target.y).abs() > 150.0,
                    ),
                    WindowJourney::Squeeze(g) => (g.elapsed, g.duration, false),
                    WindowJourney::Hop(g) => {
                        (g.elapsed, g.duration, g.start.distance(g.target) > 120.0)
                    }
                };
                if elapsed > 0.25 {
                    return None;
                }
                let target_window = j.surface().window_key?;
                let target_bounds = desktop
                    .windows
                    .iter()
                    .find(|w| w.key == target_window)?
                    .bounds;
                Some((
                    id,
                    j.surface().monitor_id,
                    motion::landing_point(j),
                    target_window,
                    target_bounds,
                    seconds,
                    rewarding,
                ))
            })
            .next();
        let Some((id, monitor, target, window, bounds, seconds, rewarding)) = candidate else {
            return;
        };
        self.surface_memory.next_origin = self.surface_memory.next_origin.wrapping_add(1);
        let origin = Origin::Surface(self.surface_memory.next_origin);
        let c = self.save.creatures.iter().find(|c| c.id == id).unwrap();
        let reaction = Reaction {
            origin,
            role: Role::Journey {
                target_window: Some(window),
                target_bounds: Some(bounds),
                stage: Stage::Notice,
                hanging: 0.0,
                rewarding,
                escape: false,
                since: 0.0,
            },
            target,
            emotion: AttentionEmotion::Curious,
            elapsed: 0.0,
            seconds: seconds.min(8.0) + 1.8,
            delay: 0.0,
            travel_elapsed: 0.0,
            walk: None,
            display_walk: None,
            surface: (monitor, c.state.surface.window_key),
            action: c.state.action,
            cue: None,
            ride: None,
        };
        self.begin_attention(id, reaction);
        self.recruit_attention_observers(id, origin, desktop);
        for p in self.attention.plans.values_mut() {
            if matches!(p.role,Role::Observer { actor } if actor == id) {
                p.seconds = reaction.seconds + 0.5;
            }
        }
        self.attention.colony_cooldown = 12.0;
    }

    pub(super) fn try_gap_attention(&mut self, desktop: &DesktopSnapshot) -> bool {
        if self.save.settings.reduce_motion {
            return false;
        }
        let candidate = self
            .save
            .creatures
            .iter()
            .filter(|c| {
                self.attention_eligible(c, desktop)
                    && matches!(c.state.action, ActionKind::Idle | ActionKind::Perch)
                    && c.personality.curiosity > 0.45
                    && c.state.drives.energy > 0.35
            })
            .filter_map(|c| self.gap_candidate(c, desktop).map(|gap| (c.id, gap)))
            .next();
        let Some((id, (journey, decision, drop))) = candidate else {
            return false;
        };
        match decision {
            GapDecision::Hesitate => self.begin_hesitation(id, journey, drop, desktop),
            _ => self.begin_gap_attention(
                id,
                journey,
                decision == GapDecision::Commit,
                drop,
                desktop,
            ),
        }
        true
    }

    pub(super) fn begin_gap_attention(
        &mut self,
        id: CreatureId,
        journey: GapJourney,
        accepted: bool,
        drop: f32,
        desktop: &DesktopSnapshot,
    ) {
        let creature = self.save.creatures.iter().find(|c| c.id == id).unwrap();
        let source = creature.state.surface.window_key.unwrap();
        let surface = (creature.state.surface.monitor_id, Some(source));
        self.surface_memory.next_origin = self.surface_memory.next_origin.wrapping_add(1);
        let origin = Origin::Surface(self.surface_memory.next_origin);
        let reaction = Reaction {
            ride: None,
            origin,
            role: if accepted {
                Role::Journey {
                    target_window: journey.hop.surface.window_key,
                    target_bounds: Some(journey.target_bounds),
                    stage: Stage::Notice,
                    hanging: 0.0,
                    rewarding: !journey.bridge,
                    escape: false,
                    since: 0.0,
                }
            } else {
                Role::Ledge {
                    window: source,
                    bounds: journey.source_bounds,
                    drop,
                    resting: false,
                    declined: true,
                    commute: false,
                }
            },
            target: journey.hop.target,
            emotion: if accepted {
                AttentionEmotion::Curious
            } else {
                AttentionEmotion::Concerned
            },
            elapsed: 0.0,
            seconds: if accepted {
                journey.preparation
                    + journey.hop.duration
                    + 2.0
                    + if journey.catch { 2.4 } else { 0.0 }
            } else {
                REACTION_SECONDS
            },
            delay: 0.0,
            travel_elapsed: 0.0,
            walk: None,
            display_walk: None,
            surface,
            action: ActionKind::InspectScreen,
            cue: None,
        };
        if accepted {
            let creature = creature_mut(&mut self.save.creatures, id).unwrap();
            creature.state.action = ActionKind::InspectScreen;
            creature.state.action_elapsed = 0.0;
            creature.state.action_duration = f32::MAX;
            self.window_journeys.insert(id, WindowJourney::Gap(journey));
        }
        self.begin_attention(id, reaction);
        self.recruit_attention_observers(id, origin, desktop);
        for plan in self.attention.plans.values_mut() {
            if matches!(plan.role, Role::Observer { actor } if actor == id) {
                plan.seconds = reaction.seconds + 0.5;
            }
        }
        self.attention.colony_cooldown = 15.0;
    }

    /// The edge of this creature's own surface that faces a gap worth trying, if there is one.
    /// Used to walk a competitor into position before it can judge the gap itself.
    pub(super) fn gap_edge_for(
        &self,
        c: &Creature,
        desktop: &DesktopSnapshot,
    ) -> Option<(Point, f32)> {
        let source = desktop
            .windows
            .iter()
            .take(MAX_TOPOLOGY_WINDOWS)
            .find(|w| Some(w.key) == c.state.surface.window_key && w.visible && !w.minimized)?;
        let unit = creature_unit(c, &self.save.settings, desktop);
        desktop
            .windows
            .iter()
            .take(MAX_TOPOLOGY_WINDOWS)
            .filter(|w| {
                w.key != source.key
                    && w.visible
                    && !w.minimized
                    && (w.bounds.y - source.bounds.y).abs() <= 36.0 * unit
            })
            .find_map(|target| {
                let right = target.bounds.x >= source.bounds.right();
                let gap = if right {
                    target.bounds.x - source.bounds.right()
                } else {
                    source.bounds.x - target.bounds.right()
                };
                (0.0..=110.0 * unit).contains(&gap).then_some((
                    Point {
                        x: if right {
                            source.bounds.right()
                        } else {
                            source.bounds.x
                        },
                        y: source.bounds.y,
                    },
                    if right { 1.0 } else { -1.0 },
                ))
            })
    }

    pub(super) fn gap_candidate(
        &self,
        c: &Creature,
        desktop: &DesktopSnapshot,
    ) -> Option<(GapJourney, GapDecision, f32)> {
        let setback = self.attention.setback(c.id);
        let source = desktop
            .windows
            .iter()
            .take(MAX_TOPOLOGY_WINDOWS)
            .find(|w| Some(w.key) == c.state.surface.window_key && w.visible && !w.minimized)?;
        let monitor = desktop
            .monitors
            .iter()
            .find(|m| m.id == c.state.surface.monitor_id)?;
        let unit =
            (f32::from(self.save.settings.display_scale) / monitor.scale_factor).clamp(0.5, 2.0);
        let start = c.state.position;
        desktop
            .windows
            .iter()
            .take(MAX_TOPOLOGY_WINDOWS)
            .filter(|w| {
                w.key != source.key
                    && w.visible
                    && !w.minimized
                    && (w.bounds.y - start.y).abs() <= 36.0 * unit
            })
            .filter_map(|target_window| {
                let right = target_window.bounds.x >= source.bounds.right();
                let (gap, edge) = if right {
                    (
                        target_window.bounds.x - source.bounds.right(),
                        source.bounds.right(),
                    )
                } else {
                    (
                        source.bounds.x - target_window.bounds.right(),
                        source.bounds.x,
                    )
                };
                if !(0.0..=110.0 * unit).contains(&gap) || (start.x - edge).abs() > 36.0 * unit {
                    return None;
                }
                // No immediate retry of a gap that recently ended in a slip, fall, or retreat.
                if setback.is_some_and(|s| s.route == Some((source.key, target_window.key))) {
                    return None;
                }
                let bridge =
                    gap <= 24.0 * unit && (target_window.bounds.y - start.y).abs() <= 12.0 * unit;
                let direction = if right { 1.0 } else { -1.0 };
                let margin = (target_window.bounds.width * 0.05).max(16.0 * unit);
                if target_window.bounds.width < margin * 2.0 + 24.0 {
                    return None;
                }
                // Land clear of whoever is already there, a little further in if need be.
                let spacing = 40.0 * unit;
                let occupied = |point: Point| {
                    self.save.creatures.iter().any(|other| {
                        other.id != c.id
                            && other.state.arrival_delay_secs <= 0.0
                            && other.state.position.distance(point) < spacing
                    }) || self
                        .window_journeys
                        .values()
                        .any(|j| motion::landing_point(j).distance(point) < spacing)
                        || self.attention.plans.values().any(|p| {
                            p.walk
                                .is_some_and(|w| w.destination.distance(point) < spacing)
                        })
                };
                let target = [1.0, 2.6, 4.2]
                    .into_iter()
                    .map(|step| Point {
                        x: if right {
                            target_window.bounds.x + margin * step
                        } else {
                            target_window.bounds.right() - margin * step
                        },
                        y: target_window.bounds.y,
                    })
                    .filter(|point| {
                        point.x > target_window.bounds.x + 12.0
                            && point.x < target_window.bounds.right() - 12.0
                    })
                    .find(|point| !occupied(*point))?;
                let runup = if bridge {
                    start
                } else {
                    motion::safe_goal(
                        self,
                        c,
                        start.x - direction * (12.0 + c.personality.boldness * 16.0) * unit,
                        desktop,
                    )?
                };
                let hop = HopJourney {
                    start,
                    target,
                    surface: SurfaceAttachment {
                        kind: SurfaceKind::WindowLedge,
                        monitor_id: monitor.id,
                        window_key: Some(target_window.key),
                        relative_x: (target.x - target_window.bounds.x)
                            / target_window.bounds.width,
                    },
                    elapsed: 0.0,
                    duration: if bridge {
                        (start.distance(target) / motion::speed(c)).clamp(0.5, 2.5)
                    } else {
                        (start.distance(target) / (170.0 * unit)).clamp(0.5, 1.2)
                    },
                };
                let regions = accessible_regions(&self.save.settings.habitat, monitor);
                // A fixed sample count, once at selection. Each actual step is validated again.
                if !regions.iter().any(|r| {
                    r.contains(runup)
                        && (0..=16).all(|i| {
                            r.contains(if bridge {
                                lerp_point(hop.start, hop.target, i as f32 / 16.0)
                            } else {
                                gap_position(&hop, i as f32 / 16.0)
                            })
                        })
                }) {
                    return None;
                }
                if (0..=16).any(|i| {
                    let mut head = if bridge {
                        lerp_point(hop.start, hop.target, i as f32 / 16.0)
                    } else {
                        gap_position(&hop, i as f32 / 16.0)
                    };
                    head.y -= 24.0 * unit;
                    !point_exposed(head, None, desktop)
                }) {
                    return None;
                }
                let drop = drop_below(c, edge + direction * 14.0, desktop, &self.save.settings);
                // Temperament, energy, and learned climbing against width, height, sleepiness,
                // and recent setbacks. The scores are bounded; identities never enter them.
                let confidence = c.personality.boldness * 0.65
                    + c.state.drives.energy * 0.25
                    + LearnedTendencies::utility(c.tendencies.climbing) * 0.2
                    - (c.state.drives.sleep_pressure - 0.4).max(0.0) * 0.25;
                let risk = 0.35
                    + gap / (160.0 * unit) * 0.35
                    + (drop / (600.0 * unit)).min(1.0) * 0.18
                    + setback.map_or(0.0, |s| f32::from(s.count) * 0.05);
                let margin = confidence - risk;
                let decision = if bridge || margin >= 0.0 {
                    GapDecision::Commit
                } else if margin >= -HESITATION_MARGIN && !self.save.settings.reduce_motion {
                    GapDecision::Hesitate
                } else {
                    GapDecision::Refuse
                };
                Some((
                    GapJourney {
                        source: source.key,
                        source_bounds: source.bounds,
                        target_bounds: target_window.bounds,
                        hop,
                        runup,
                        preparation: if bridge {
                            0.45
                        } else {
                            1.5 + (1.0 - c.personality.boldness) * 0.6
                                + (drop / (600.0 * unit)).min(1.0) * 0.4
                        },
                        elapsed: 0.0,
                        catch: !bridge && margin < 0.12,
                        helped: false,
                        helper: None,
                        assistance_checked: false,
                        assistance_slip: false,
                        bridge,
                    },
                    decision,
                    drop,
                ))
            })
            .min_by(|a, b| {
                a.0.hop
                    .start
                    .distance(a.0.hop.target)
                    .total_cmp(&b.0.hop.start.distance(b.0.hop.target))
            })
    }

    pub(super) fn update_gap_attention(&mut self, desktop: &DesktopSnapshot) {
        for (&id, plan) in &mut self.attention.plans {
            let Role::Journey {
                stage,
                target_window,
                target_bounds,
                hanging,
                rewarding,
                since,
                ..
            } = &mut plan.role
            else {
                continue;
            };
            let before = *stage;
            if let Some(WindowJourney::Gap(journey)) = self.window_journeys.get(&id) {
                *hanging = gap_hanging(journey);
                *stage = if journey.catch
                    && journey.elapsed >= journey.preparation + journey.hop.duration
                {
                    Stage::Catch
                } else if journey.elapsed < journey.preparation.min(0.6) {
                    Stage::Notice
                } else if journey.elapsed < journey.preparation {
                    Stage::Prepare
                } else {
                    Stage::Act
                };
            } else if let Some(journey) = self.window_journeys.get(&id) {
                // A completed staircase step can hand the gaze to the next actual destination.
                // Repairs of a moved target still fail the captured-bounds check below.
                if journey.surface().window_key != *target_window
                    && self.save.creatures.iter().any(|c| {
                        c.id == id
                            && c.state.surface.window_key == *target_window
                            && c.state.position.distance(plan.target) < 1.0
                    })
                    && target_unchanged(*target_window, *target_bounds, desktop)
                    && let Some(next) = desktop
                        .windows
                        .iter()
                        .find(|w| Some(w.key) == journey.surface().window_key)
                {
                    *target_window = Some(next.key);
                    *target_bounds = Some(next.bounds);
                    plan.target = motion::landing_point(journey);
                }
                *hanging = 0.0;
                *stage = match journey {
                    WindowJourney::Climb(g) if g.elapsed < g.approach_duration => Stage::Prepare,
                    WindowJourney::Squeeze(g) if g.elapsed < 0.2 => Stage::Prepare,
                    _ => Stage::Act,
                };
            } else if !matches!(stage, Stage::Recover(_))
                && let Some(creature) = self.save.creatures.iter().find(|c| c.id == id)
                && creature.state.surface.window_key == *target_window
                && creature.state.position.distance(plan.target) < 1.0
            {
                *stage = Stage::Recover(if *rewarding {
                    Outcome::Completed
                } else {
                    Outcome::Settled
                });
                *hanging = 0.0;
                plan.elapsed = 0.0;
                plan.seconds = 1.8;
                plan.surface = (
                    creature.state.surface.monitor_id,
                    creature.state.surface.window_key,
                );
            }
            if *stage != before {
                *since = plan.elapsed;
            }
        }
        // Reduced motion toggled during an attempt is a safe interruption, never a victory.
        if self.save.settings.reduce_motion {
            self.cancel_gap_journeys(desktop);
        }
    }

    pub(in super::super) fn cancel_gap_journeys(&mut self, desktop: &DesktopSnapshot) {
        self.cancel_tumble_attention(desktop);
        let ids: Vec<_> = self
            .window_journeys
            .iter()
            .filter_map(|(&id, j)| matches!(j, WindowJourney::Gap(_)).then_some(id))
            .collect();
        for id in ids {
            self.window_journeys.remove(&id);
            self.cancel_creature_attention(id);
            if let Some(creature) = creature_mut(&mut self.save.creatures, id) {
                settle_interrupted_journey(
                    creature,
                    desktop,
                    &self.save.settings.habitat,
                    &mut self.events,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::ledges::tests::edge_scene;
    use super::super::tests::scene;
    use super::*;

    fn tick(world: &mut World, desktop: &mut DesktopSnapshot, now: OffsetDateTime, step: u64) {
        desktop.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
        world.tick(
            now + Duration::milliseconds(step as i64 * 50),
            0.05,
            desktop,
        );
    }

    /// A perch on window 701 facing the fixture's 70-point gap, with the same nerve every time.
    /// `ground_below` puts a ledge under the edge so the only difference is how far down it is.
    fn judged_gap(ground_below: bool) -> (GapJourney, GapDecision, f32) {
        let (mut world, mut desktop, _) = edge_scene(true, true);
        world.clear_attention();
        world.window_journeys.clear();
        let creature = &mut world.save.creatures[0];
        creature.state.action = ActionKind::Perch;
        creature.state.position = Point { x: 770.0, y: 600.0 };
        creature.personality.boldness = 0.55;
        creature.state.drives.energy = 0.8;
        if ground_below {
            let mut below = desktop.windows[0].clone();
            below.key = 703;
            below.z_order = 5;
            below.bounds = DesktopRect {
                x: 780.0,
                y: 700.0,
                width: 300.0,
                height: 200.0,
            };
            desktop.windows.push(below);
        }
        world
            .gap_candidate(&world.save.creatures[0], &desktop)
            .expect("the gap itself is unchanged")
    }

    #[test]
    fn a_long_drop_below_the_same_gap_turns_a_commitment_into_a_reconsideration() {
        let (over_the_ground, near_decision, near_drop) = judged_gap(true);
        let (over_a_long_fall, high_decision, high_drop) = judged_gap(false);
        assert_eq!(near_decision, GapDecision::Commit);
        assert_eq!(high_decision, GapDecision::Hesitate);
        assert!(high_drop > near_drop, "{high_drop} {near_drop}");
        // The same creature also spends longer on the edge before the longer fall.
        assert!(over_a_long_fall.preparation > over_the_ground.preparation);
        assert_eq!(over_the_ground.hop.target, over_a_long_fall.hop.target);
    }

    #[test]
    fn a_perch_with_no_room_to_back_up_leaves_the_same_gap_alone() {
        let (mut world, mut desktop, _) = edge_scene(true, true);
        world.clear_attention();
        world.window_journeys.clear();
        let creature = &mut world.save.creatures[0];
        creature.state.action = ActionKind::Perch;
        creature.state.position = Point { x: 770.0, y: 600.0 };
        // The same edge and the same gap, on a platform too narrow for a run-up.
        desktop.windows[0].bounds.x = 750.0;
        desktop.windows[0].bounds.width = 50.0;
        world.save.creatures[0].state.surface.relative_x = 0.4;
        assert!(
            world
                .gap_candidate(&world.save.creatures[0], &desktop)
                .is_none(),
            "backing up would step off the far edge"
        );
        desktop.windows[0].bounds.x = 200.0;
        desktop.windows[0].bounds.width = 600.0;
        world.save.creatures[0].state.surface.relative_x = 0.95;
        let (journey, decision, _) = world
            .gap_candidate(&world.save.creatures[0], &desktop)
            .expect("room to run up");
        assert_eq!(decision, GapDecision::Commit);
        assert!(journey.runup.x < journey.hop.start.x);
        assert!(journey.runup.x > desktop.windows[0].bounds.x + 12.0);
    }

    /// Ordinary traversal is worth watching too. Slipping through a narrow gap between two
    /// windows carries the same notice, effort, and settling for an audience as a jump does —
    /// and a crossing anybody could make is never celebrated.
    #[test]
    fn a_squeeze_between_two_windows_is_published_to_the_watching_colony() {
        let (mut world, mut desktop, now) = scene();
        world.clear_attention();
        let mut neighbor = desktop.windows[0].clone();
        neighbor.key = 702;
        neighbor.bounds.x = 820.0;
        neighbor.bounds.width = 300.0;
        desktop.windows.push(neighbor);
        let id = world.save.creatures[0].id;
        let creature = &mut world.save.creatures[0];
        creature.state.action = ActionKind::Perch;
        creature.state.position = Point { x: 780.0, y: 600.0 };
        creature.state.surface.relative_x = (780.0 - 200.0) / 600.0;
        let (start, target) = (creature.state.position, Point { x: 840.0, y: 600.0 });
        world.window_journeys.insert(
            id,
            WindowJourney::Squeeze(SqueezeJourney {
                from_window: 701,
                from_bounds: desktop.windows[0].bounds,
                target_window: 702,
                target_bounds: desktop.windows[1].bounds,
                start,
                target,
                surface: SurfaceAttachment {
                    kind: SurfaceKind::WindowLedge,
                    monitor_id: 1,
                    window_key: Some(702),
                    relative_x: (target.x - 820.0) / 300.0,
                },
                elapsed: 0.0,
                duration: 1.2,
            }),
        );
        world.promote_window_journey(&desktop);
        let mut stages = Vec::new();
        let mut watched = false;
        for step in 1..90 {
            tick(&mut world, &mut desktop, now, step);
            if let Some(Role::Journey { stage, .. }) =
                world.attention.plans.get(&id).map(|p| p.role)
                && stages.last() != Some(&stage)
            {
                stages.push(stage);
            }
            let head = head_point(&world.save.creatures[0], &world.save.settings, &desktop);
            for other in world.save.creatures.iter().skip(1) {
                let watching = world
                    .attention
                    .plans
                    .get(&other.id)
                    .is_some_and(|p| matches!(p.role, Role::Observer { actor } if actor == id));
                let Some(pose) = other.state.attention.filter(|_| watching) else {
                    continue;
                };
                watched |= (pose.target.x - head.x).abs() < 6.0;
                assert_ne!(other.state.action, ActionKind::Greet);
            }
        }
        assert!(watched);
        assert!(
            stages.contains(&Stage::Prepare) && stages.contains(&Stage::Act),
            "{stages:?}"
        );
        assert_eq!(stages.last(), Some(&Stage::Recover(Outcome::Settled)));
        assert_eq!(world.save.creatures[0].state.surface.window_key, Some(702));
    }

    #[test]
    fn a_gap_that_is_too_wide_or_hidden_behind_a_window_is_never_offered() {
        let (mut world, mut desktop, _) = edge_scene(true, true);
        world.clear_attention();
        world.window_journeys.clear();
        let creature = &mut world.save.creatures[0];
        creature.state.action = ActionKind::Perch;
        creature.state.position = Point { x: 770.0, y: 600.0 };
        let candidate = |world: &World, desktop: &DesktopSnapshot| {
            world
                .gap_candidate(&world.save.creatures[0], desktop)
                .is_some()
        };
        assert!(candidate(&world, &desktop));
        // Beyond one leap: the far ledge is simply not on offer.
        desktop.windows[1].bounds.x = 930.0;
        assert!(!candidate(&world, &desktop));
        desktop.windows[1].bounds.x = 870.0;
        assert!(candidate(&world, &desktop));
        // A window across the flight path hides where the creature would be in mid-air.
        let mut cover = desktop.windows[0].clone();
        cover.key = 704;
        cover.z_order = 0;
        cover.bounds = DesktopRect {
            x: 780.0,
            y: 500.0,
            width: 120.0,
            height: 100.0,
        };
        desktop.windows.push(cover);
        assert!(!candidate(&world, &desktop));
    }
}
