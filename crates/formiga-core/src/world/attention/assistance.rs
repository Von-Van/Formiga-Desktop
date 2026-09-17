//! A watcher may become a helper; the transfer releases its old viewing reservation.
use super::*;

impl World {
    pub(super) fn try_assistance(&mut self, desktop: &DesktopSnapshot) {
        for (&id, journey) in &mut self.window_journeys {
            let WindowJourney::Gap(gap) = journey else {
                continue;
            };
            if let Some(helper) = gap.helper {
                let engaged =
                    self.attention.plans.get(&helper).is_some_and(
                        |p| matches!(p.role, Role::Helper { actor, .. } if actor == id),
                    );
                gap.helped = engaged
                    && self
                        .save
                        .creatures
                        .iter()
                        .find(|c| c.id == helper)
                        .is_some_and(|c| {
                            let scale = desktop
                                .monitors
                                .iter()
                                .find(|m| m.id == c.state.surface.monitor_id)
                                .map_or(1.0, |m| {
                                    (f32::from(self.save.settings.display_scale) / m.scale_factor)
                                        .clamp(0.5, 2.0)
                                });
                            self.attention
                                .plans
                                .get(&helper)
                                .and_then(|p| p.walk)
                                .is_none()
                                && c.state.surface.window_key == gap.hop.surface.window_key
                                && c.state.position.distance(gap.hop.target) <= 36.0 * scale
                        });
                if !engaged {
                    gap.helper = None;
                }
            }
        }
        let slip = self.window_journeys.iter().find_map(|(&id, j)| match j {
            WindowJourney::Gap(g)
                if g.helped
                    && g.assistance_slip
                    && g.elapsed - g.preparation - g.hop.duration >= 1.1 =>
            {
                g.helper.map(|helper| (id, helper))
            }
            _ => None,
        });
        if let Some((actor, helper)) = slip {
            if self.start_shared_tumble(actor, helper, desktop) {
                return;
            }
            if let Some(WindowJourney::Gap(g)) = self.window_journeys.get_mut(&actor) {
                g.assistance_slip = false;
            }
        }
        let actor = self.window_journeys.iter().find_map(|(&id, j)| match j {
            WindowJourney::Gap(g)
                if g.catch
                    && !g.assistance_checked
                    && g.elapsed >= g.preparation + g.hop.duration =>
            {
                Some(id)
            }
            _ => None,
        });
        let Some(actor) = actor else {
            return;
        };
        let Some(WindowJourney::Gap(gap)) = self.window_journeys.get_mut(&actor) else {
            return;
        };
        gap.assistance_checked = true;
        let target = gap.hop.target;
        let window = gap.hop.surface.window_key.unwrap();
        let bounds = gap.target_bounds;
        let monitor = gap.hop.surface.monitor_id;
        let candidate = self
            .save
            .creatures
            .iter()
            .filter(|c| {
                c.id != actor
                    && c.state.surface.window_key == Some(window)
                    && c.state.drives.energy > 0.4
                    && c.personality.sociability > 0.5
                    && c.state.position.distance(target) < 120.0
                    && self.attention.plans.get(&c.id).is_some_and(
                        |p| matches!(p.role, Role::Observer { actor: watched } if watched == actor),
                    )
            })
            .filter(|c| {
                !self.save.relationships.iter().any(|b| {
                    ((b.a == actor && b.b == c.id) || (b.b == actor && b.a == c.id))
                        && b.avoidance > 128
                })
            })
            .filter_map(|c| {
                let scale = desktop
                    .monitors
                    .iter()
                    .find(|m| m.id == monitor)
                    .map_or(1.0, |m| {
                        (f32::from(self.save.settings.display_scale) / m.scale_factor)
                            .clamp(0.5, 2.0)
                    });
                let side = if c.state.position.x >= target.x {
                    1.0
                } else {
                    -1.0
                };
                let destination =
                    motion::safe_goal(self, c, target.x + side * 34.0 * scale, desktop)?;
                Some((c.state.position.distance(destination), c.id, destination))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, helper, destination)) = candidate else {
            return;
        };
        let plan = self.attention.plans.get_mut(&helper).unwrap();
        plan.role = Role::Helper {
            actor,
            window,
            bounds,
        };
        plan.walk = Some(ShortWalk::watching(destination, bounds));
        plan.delay = 0.0;
        plan.travel_elapsed = 0.0;
        plan.elapsed = 0.45;
        plan.seconds = 4.0;
        plan.emotion = AttentionEmotion::Concerned;
        if let Some(WindowJourney::Gap(gap)) = self.window_journeys.get_mut(&actor) {
            gap.helper = Some(helper);
            gap.assistance_slip = self.ambient_rng.random_ratio(1, 12);
        }
    }

    fn start_shared_tumble(
        &mut self,
        actor: CreatureId,
        helper: CreatureId,
        desktop: &DesktopSnapshot,
    ) -> bool {
        if self.save.settings.reduce_motion {
            return false;
        }
        let Some(origin) = self.attention.plans.get(&actor).map(|p| p.origin) else {
            return false;
        };
        let participants = [actor, helper];
        let Some(actor_point) = self
            .save
            .creatures
            .iter()
            .find(|c| c.id == actor)
            .map(|c| c.state.position)
        else {
            return false;
        };
        let Some(helper_point) = self
            .save
            .creatures
            .iter()
            .find(|c| c.id == helper)
            .map(|c| c.state.position)
        else {
            return false;
        };
        let side = if actor_point.x < helper_point.x {
            -1.0
        } else {
            1.0
        };
        let mut launches = Vec::with_capacity(2);
        for (index, id) in participants.into_iter().enumerate() {
            let c = self.save.creatures.iter().find(|c| c.id == id).unwrap();
            let Some(monitor) = desktop
                .monitors
                .iter()
                .find(|m| m.id == c.state.surface.monitor_id)
            else {
                return false;
            };
            let scale = (f32::from(self.save.settings.display_scale) / monitor.scale_factor)
                .clamp(0.5, 2.0);
            let direction = if index == 0 { side } else { -side };
            let mut start = c.state.position;
            start.y += (c.state.attention.map_or(0.0, |p| p.hanging) * 36.0 * scale).max(2.0);
            let ray = Point {
                x: start.x + direction * 25.0 * scale,
                y: start.y,
            };
            let Some((landing, surface)) =
                find_drop_support(ray, desktop, &self.save.settings.habitat, true)
            else {
                return false;
            };
            if landing.y - start.y < 30.0
                || !accessible_regions(&self.save.settings.habitat, monitor)
                    .iter()
                    .any(|r| r.contains(start) && r.contains(landing))
            {
                return false;
            }
            launches.push((id, start, landing, surface, direction * 65.0 * scale));
        }
        if launches[0].2.distance(launches[1].2) < 40.0 {
            return false;
        }
        if let Some(WindowJourney::Gap(gap)) = self.window_journeys.remove(&actor) {
            self.attention
                .record_setback(actor, gap.hop.surface.window_key.map(|t| (gap.source, t)));
        }
        self.attention.record_setback(helper, None);
        for (id, start, landing, surface, vx) in launches {
            let c = creature_mut(&mut self.save.creatures, id).unwrap();
            c.state.position = start;
            c.state.action = ActionKind::Tossed;
            c.state.action_elapsed = 0.0;
            c.state.velocity = Point { x: vx, y: 70.0 };
            c.state.attention = None;
            self.tosses.insert(
                id,
                TossState {
                    elapsed: 0.0,
                    bounces: 0,
                    last_safe_position: landing,
                    last_safe_surface: surface,
                },
            );
            let reaction = Reaction {
                origin,
                role: Role::Tumble { stage: Stage::Act },
                target: if id == actor {
                    helper_point
                } else {
                    actor_point
                },
                emotion: AttentionEmotion::Startled,
                elapsed: 0.0,
                seconds: 4.5,
                delay: 0.0,
                travel_elapsed: 0.0,
                walk: None,
                display_walk: None,
                surface: (c.state.surface.monitor_id, c.state.surface.window_key),
                action: ActionKind::Tossed,
                cue: None,
                ride: None,
            };
            self.begin_attention(id, reaction);
        }
        for plan in self.attention.plans.values_mut() {
            if matches!(plan.role,Role::Observer { actor: watched } if watched == actor) {
                plan.walk = None;
                plan.elapsed = 0.0;
                plan.seconds = 5.0;
                plan.travel_elapsed = 0.0;
                plan.emotion = AttentionEmotion::Concerned;
            }
        }
        true
    }

    pub(super) fn update_tumble_attention(&mut self) {
        for (&id, plan) in &mut self.attention.plans {
            let Role::Tumble { stage } = &mut plan.role else {
                continue;
            };
            if !self.tosses.contains_key(&id)
                && !matches!(stage, Stage::Recover(_))
                && let Some(c) = self
                    .save
                    .creatures
                    .iter()
                    .find(|c| c.id == id && c.state.action == ActionKind::Landing)
            {
                *stage = Stage::Recover(Outcome::Slipped);
                plan.elapsed = 0.0;
                plan.seconds = 1.6;
                plan.surface = (c.state.surface.monitor_id, c.state.surface.window_key);
            }
        }
    }

    pub(super) fn cancel_tumble_attention(&mut self, desktop: &DesktopSnapshot) {
        let ids: Vec<_> = self
            .attention
            .plans
            .iter()
            .filter_map(|(&id, p)| matches!(p.role, Role::Tumble { .. }).then_some(id))
            .collect();
        for id in ids {
            if let Some(toss) = self.tosses.remove(&id)
                && let Some(c) = creature_mut(&mut self.save.creatures, id)
            {
                settle_toss(
                    c,
                    &toss,
                    desktop,
                    &self.save.settings.habitat,
                    self.save.settings.window_ledges,
                );
            }
            self.cancel_creature_attention(id);
        }
    }
}
