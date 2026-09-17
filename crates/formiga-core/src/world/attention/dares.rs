//! A conspicuous success invites a companion to try the same gap. The answer may be an attempt,
//! a cautious approach that reconsiders, or a plain refusal — and the creature that just landed
//! becomes the one watching.
use super::*;

/// A dare comes once the landing has been celebrated, while it is still fresh.
const DARE_WINDOW: std::ops::Range<f32> = 1.0..1.8;

impl World {
    /// Offer the gap a companion just cleared to one watcher of that success.
    pub(super) fn try_dare(&mut self, desktop: &DesktopSnapshot) {
        let finished = self.attention.plans.iter().find_map(|(&id, p)| {
            let Role::Journey {
                stage: Stage::Recover(Outcome::Completed),
                target_window: Some(target_window),
                target_bounds: Some(target_bounds),
                rewarding: true,
                ..
            } = p.role
            else {
                return None;
            };
            DARE_WINDOW
                .contains(&p.elapsed)
                .then_some((id, p.origin, target_window, target_bounds))
        });
        let Some((winner, origin, target_window, target_bounds)) = finished else {
            return;
        };
        // The dare is the gap itself: any watcher standing on a ledge that faces the same
        // destination can be asked, from its own edge.
        let candidate = self
            .attention
            .plans
            .iter()
            .filter(|(_, p)| matches!(p.role, Role::Observer { actor } if actor == winner))
            .filter_map(|(&id, _)| {
                let c = self.save.creatures.iter().find(|c| c.id == id)?;
                let source = c.state.surface.window_key?;
                let source_bounds = desktop
                    .windows
                    .iter()
                    .find(|w| w.key == source && w.visible && !w.minimized)?
                    .bounds;
                let unit = creature_unit(c, &self.save.settings, desktop);
                let right = target_bounds.x >= source_bounds.right();
                let gap = if right {
                    target_bounds.x - source_bounds.right()
                } else {
                    source_bounds.x - target_bounds.right()
                };
                if !(0.0..=110.0 * unit).contains(&gap)
                    || (target_bounds.y - source_bounds.y).abs() > 36.0 * unit
                    || c.personality.curiosity <= 0.4
                    || c.state.drives.energy <= 0.4
                {
                    return None;
                }
                let edge = Point {
                    x: if right {
                        source_bounds.right()
                    } else {
                        source_bounds.x
                    },
                    y: source_bounds.y,
                };
                ((c.state.position.x - edge.x).abs() <= 150.0).then_some((
                    id,
                    source,
                    source_bounds,
                    edge,
                    right,
                    c.personality.boldness + c.personality.playfulness * 0.5,
                ))
            })
            .max_by(|a, b| a.5.total_cmp(&b.5));
        let Some((dared, source, source_bounds, edge, right, _)) = candidate else {
            return;
        };
        let landing = self.attention.plans[&winner].target;
        let creature = self.save.creatures.iter().find(|c| c.id == dared).unwrap();
        let unit = creature_unit(creature, &self.save.settings, desktop);
        let approach = motion::safe_goal(
            self,
            creature,
            edge.x - if right { 20.0 * unit } else { -20.0 * unit },
            desktop,
        );
        let plan = Reaction {
            origin,
            role: Role::Dare {
                source,
                source_bounds,
                target_window,
                target_bounds,
                landing,
            },
            target: landing,
            emotion: AttentionEmotion::Curious,
            elapsed: 0.0,
            seconds: 6.0,
            delay: 0.0,
            travel_elapsed: 0.0,
            walk: (!self.save.settings.reduce_motion)
                .then_some(approach)
                .flatten()
                .map(ShortWalk::playing),
            display_walk: None,
            surface: (creature.state.surface.monitor_id, Some(source)),
            action: ActionKind::InspectScreen,
            cue: None,
            ride: None,
        };
        self.begin_attention(dared, plan);
        // The creature that just landed turns around and watches the answer.
        if let Some(previous) = self.attention.plans.get_mut(&winner) {
            previous.role = Role::Observer { actor: dared };
            previous.cue = None;
            previous.elapsed = 0.0;
            previous.delay = 0.0;
            previous.travel_elapsed = 0.0;
            previous.seconds = 6.5;
            previous.walk = None;
        }
        self.extend_audience(dared, 6.0);
        self.attention.colony_cooldown = 15.0;
    }

    /// When the dared creature reaches the edge it answers: attempt, hesitate, or decline.
    pub(super) fn update_dares(&mut self, desktop: &DesktopSnapshot) {
        let ready: Vec<_> = self
            .attention
            .plans
            .iter()
            .filter_map(|(&id, p)| {
                let Role::Dare { .. } = p.role else {
                    return None;
                };
                (p.walk.is_none() && p.elapsed >= 0.6).then_some(id)
            })
            .collect();
        for id in ready {
            let Some(plan) = self.attention.plans.get(&id).copied() else {
                continue;
            };
            let Role::Dare {
                source,
                target_window,
                landing,
                ..
            } = plan.role
            else {
                continue;
            };
            let Some(creature) = self.save.creatures.iter().find(|c| c.id == id) else {
                continue;
            };
            let answer = self
                .gap_candidate(creature, desktop)
                .filter(|(gap, _, _)| gap.hop.surface.window_key == Some(target_window));
            self.attention.plans.remove(&id);
            match answer {
                Some((journey, gaps::GapDecision::Commit, drop)) => {
                    self.begin_gap_attention(id, journey, true, drop, desktop);
                }
                Some((journey, gaps::GapDecision::Hesitate, drop)) => {
                    self.begin_hesitation(id, journey, drop, desktop);
                }
                // A cautious answer: look the gap over from the edge and stay where it is.
                _ => {
                    let bounds = desktop
                        .windows
                        .iter()
                        .find(|w| w.key == source)
                        .map(|w| w.bounds);
                    let Some(bounds) = bounds else { continue };
                    let declined = Reaction {
                        role: Role::Ledge {
                            window: source,
                            bounds,
                            drop: 160.0,
                            resting: false,
                            declined: true,
                            commute: false,
                        },
                        target: landing,
                        emotion: AttentionEmotion::Concerned,
                        elapsed: 0.0,
                        seconds: REACTION_SECONDS,
                        walk: None,
                        ..plan
                    };
                    self.begin_attention(id, declined);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::ledges::tests::edge_scene;
    use super::*;

    fn tick(world: &mut World, desktop: &mut DesktopSnapshot, now: OffsetDateTime, step: u64) {
        desktop.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
        world.tick(
            now + Duration::milliseconds(step as i64 * 50),
            0.05,
            desktop,
        );
    }

    /// A bold jumper clears the gap while a companion of the given nerve watches from the ledge.
    fn dare_scene(nerve: f32) -> (World, DesktopSnapshot, OffsetDateTime) {
        let (mut world, desktop, now) = edge_scene(true, true);
        for (index, c) in world.save.creatures.iter_mut().enumerate() {
            if index == 1 {
                c.state.position = Point { x: 700.0, y: 600.0 };
                c.state.surface.relative_x = (700.0 - 200.0) / 600.0;
                c.personality.boldness = nerve;
                c.personality.curiosity = 0.9;
                c.personality.playfulness = 0.8;
                c.state.drives.energy = 0.85;
            }
        }
        (world, desktop, now)
    }

    #[test]
    fn a_watched_success_invites_a_companion_and_the_winner_becomes_the_audience() {
        let (mut world, mut desktop, now) = dare_scene(1.0);
        let (winner, dared) = (world.save.creatures[0].id, world.save.creatures[1].id);
        let mut invited = false;
        let mut watched_back = false;
        let mut answered = false;
        for step in 3..320 {
            tick(&mut world, &mut desktop, now, step);
            if matches!(
                world.attention.plans.get(&dared).map(|p| p.role),
                Some(Role::Dare { .. })
            ) {
                invited = true;
                // The creature that just landed is now watching the one it dared: it turns its
                // head away from its own victory and toward the companion at the edge.
                let head = head_point(&world.save.creatures[1], &world.save.settings, &desktop);
                watched_back |= matches!(
                    world.attention.plans.get(&winner).map(|p| p.role),
                    Some(Role::Observer { actor }) if actor == dared
                ) && world.save.creatures[0]
                    .state
                    .attention
                    .is_some_and(|pose| (pose.target.x - head.x).abs() < 4.0);
            }
            answered |= invited
                && (world.window_journeys.contains_key(&dared)
                    || matches!(
                        world.attention.plans.get(&dared).map(|p| p.role),
                        Some(Role::Hesitate { .. } | Role::Ledge { declined: true, .. })
                    ));
        }
        assert!(
            invited && watched_back && answered,
            "{invited} {watched_back} {answered}"
        );
        assert_eq!(world.save.creatures[1].state.surface.window_key, Some(702));
        assert!(world.attention.plans.is_empty());
    }

    #[test]
    fn a_timid_companion_answers_with_caution_instead_of_a_jump() {
        let (mut world, mut desktop, now) = dare_scene(0.0);
        let dared = world.save.creatures[1].id;
        let mut invited = false;
        let mut declined = false;
        for step in 3..320 {
            tick(&mut world, &mut desktop, now, step);
            invited |= matches!(
                world.attention.plans.get(&dared).map(|p| p.role),
                Some(Role::Dare { .. })
            );
            declined |= matches!(
                world.attention.plans.get(&dared).map(|p| p.role),
                Some(Role::Ledge { declined: true, .. } | Role::Hesitate { .. })
            );
            assert!(
                !world.window_journeys.contains_key(&dared),
                "a timid creature never takes the dare"
            );
        }
        assert!(invited && declined, "{invited} {declined}");
        assert_eq!(world.save.creatures[1].state.surface.window_key, Some(701));
    }
}
