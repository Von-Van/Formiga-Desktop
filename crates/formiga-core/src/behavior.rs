use crate::{
    ActionChoice, ActionKind, Creature, CreatureId, CreatureRelationship, DesktopSnapshot,
    LearnedTendencies, Point, SurfaceKind, routine_key,
};
use rand::Rng;

#[derive(Clone, Copy, Debug)]
pub struct BehaviorContext {
    pub nearest_creature_distance: Option<f32>,
    pub nearest_creature_position: Option<Point>,
    pub nearest_creature_id: Option<crate::CreatureId>,
    pub bond: Option<BondContext>,
    pub on_window_ledge: bool,
    pub reachable_window_ledge: bool,
    pub window_changed_nearby: bool,
    pub objects: ObjectUtility,
    pub hour_utc: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ObjectUtility {
    pub sleep: f32,
    pub play: f32,
    pub comfort: f32,
    pub social: f32,
    pub curiosity: f32,
}

impl ObjectUtility {
    pub fn add(&mut self, role: crate::ColonyObjectRole, amount: f32) {
        let score = match role {
            crate::ColonyObjectRole::Sleep => &mut self.sleep,
            crate::ColonyObjectRole::Play => &mut self.play,
            crate::ColonyObjectRole::Comfort => &mut self.comfort,
            crate::ColonyObjectRole::Social => &mut self.social,
            crate::ColonyObjectRole::Curiosity => &mut self.curiosity,
        };
        *score = (*score + amount.max(0.0)).min(0.25);
    }

    fn for_action(self, action: ActionKind) -> f32 {
        match action {
            ActionKind::Idle | ActionKind::Eat | ActionKind::Drink => self.comfort,
            ActionKind::Sleep => self.sleep,
            ActionKind::SoloPlay | ActionKind::SocialPlay => self.play,
            ActionKind::Greet | ActionKind::Follow => self.social,
            ActionKind::Traverse
            | ActionKind::Perch
            | ActionKind::InvestigateCursor
            | ActionKind::InspectScreen => self.curiosity,
            _ => 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BondContext {
    pub target_creature: CreatureId,
    pub target_position: Point,
    pub distance: f32,
    pub relationship: CreatureRelationship,
    pub target_action: ActionKind,
    pub target_surface: SurfaceKind,
}

pub fn choose_action<R: Rng + ?Sized>(
    creature: &Creature,
    desktop: &DesktopSnapshot,
    context: BehaviorContext,
    rng: &mut R,
) -> ActionChoice {
    let p = &creature.personality;
    let d = &creature.state.drives;
    let cursor_distance = if desktop.cursor.available {
        creature.state.position.distance(desktop.cursor.position)
    } else {
        f32::INFINITY
    };
    let cursor_speed =
        (desktop.cursor.velocity.x.powi(2) + desktop.cursor.velocity.y.powi(2)).sqrt();
    let night = context.hour_utc >= 22 || context.hour_utc < 6;

    let mut scored = Vec::with_capacity(ActionKind::AUTONOMOUS.len());
    for action in ActionKind::AUTONOMOUS {
        let score = match action {
            ActionKind::Idle => 0.45 + d.comfort * 0.5 - d.boredom * 0.25,
            ActionKind::Traverse => 0.25 + d.boredom * 0.8 + p.activity * 0.5,
            ActionKind::Perch => {
                if context.reachable_window_ledge {
                    0.82 + p.curiosity * 0.8 + d.boredom * 0.5
                } else if context.on_window_ledge {
                    0.45 + d.comfort * 0.7
                } else {
                    -2.0
                }
            }
            ActionKind::Sleep => {
                d.sleep_pressure * 1.2
                    + (1.0 - d.energy)
                    + f32::from(night) * p.sleep_timing * 0.35
                    + sleep_companion_score(context)
            }
            ActionKind::InvestigateCursor => {
                if cursor_distance < 180.0 && creature.state.cursor_cooldown <= 0.0 {
                    p.cursor_interest * p.curiosity * 1.1 + (1.0 - cursor_distance / 180.0)
                } else {
                    -2.0
                }
            }
            ActionKind::AvoidCursor => {
                if cursor_distance < 90.0
                    && cursor_speed > 220.0
                    && creature.state.cursor_cooldown <= 0.0
                {
                    (1.0 - p.boldness) * 1.3 + cursor_speed / 600.0
                } else {
                    -2.0
                }
            }
            ActionKind::ReactToWindow => {
                if context.window_changed_nearby {
                    0.5 + (1.0 - p.window_tolerance) + d.arousal
                } else {
                    -2.0
                }
            }
            ActionKind::RideWindow => {
                if context.on_window_ledge {
                    0.3 + p.boldness * 0.5 + p.window_tolerance * 0.5
                } else {
                    -2.0
                }
            }
            ActionKind::SoloPlay => 0.08 + p.playfulness * d.boredom * 0.95,
            ActionKind::Eat => {
                if d.energy < 0.78 {
                    0.12 + (1.0 - d.energy) * 1.05 + d.comfort * 0.05
                } else {
                    -2.0
                }
            }
            ActionKind::Drink => {
                0.06 + d.boredom * 0.42 + (1.0 - d.comfort) * 0.38 + d.arousal * 0.12
            }
            ActionKind::Sprint => {
                if d.energy > 0.35 {
                    0.18 + p.activity * (0.55 + d.boredom * 0.85) + d.arousal * 0.15
                } else {
                    -2.0
                }
            }
            ActionKind::Greet => social_score(creature, context, action, 160.0, 0.45),
            ActionKind::Follow => social_score(creature, context, action, 420.0, 0.35),
            ActionKind::SocialPlay => {
                social_score(creature, context, action, 120.0, p.playfulness * 0.55)
            }
            ActionKind::Dragged
            | ActionKind::SqueezeWindow
            | ActionKind::Landing
            | ActionKind::Homebound
            | ActionKind::ClimbWindow
            | ActionKind::Dangle
            | ActionKind::InspectScreen
            | ActionKind::PresentDiscovery
            | ActionKind::Tossed
            | ActionKind::PetReaction => -2.0,
        };
        let score = score + context.objects.for_action(action);
        let routine = creature.routines.strength(routine_key(
            creature.state.surface.kind,
            creature.state.surface.relative_x,
            action,
            context.hour_utc,
        ));
        let learned = (learned_modifier(creature, action)
            + routine
                * p.routine_affinity
                * (0.1 + (f32::from(creature.tendencies.routine) + 100.0) / 200.0 * 0.1))
            .clamp(-0.35, 0.35);
        let commitment = if action == creature.state.action {
            0.35
        } else {
            0.0
        };
        scored.push((action, score + learned + commitment));
    }

    let action = softmax_sample(&scored, p.decision_temperature.max(0.08), rng);
    let (target_creature, target_point) = match action {
        ActionKind::Greet | ActionKind::Follow | ActionKind::SocialPlay => context
            .bond
            .map(|bond| (Some(bond.target_creature), Some(bond.target_position)))
            .unwrap_or((
                context.nearest_creature_id,
                context.nearest_creature_position,
            )),
        ActionKind::Sleep => context
            .bond
            .filter(|bond| sleep_companion_eligible(*bond))
            .map(|bond| {
                (
                    Some(bond.target_creature),
                    Some(beside_target(creature.state.position, bond.target_position)),
                )
            })
            .unwrap_or((None, None)),
        ActionKind::InvestigateCursor if desktop.cursor.available => {
            (None, Some(desktop.cursor.position))
        }
        ActionKind::Traverse => (
            None,
            preferred_region_target(creature, desktop).filter(|_| {
                rng.random::<u8>()
                    <= creature
                        .memory
                        .preferred_region
                        .map_or(0, |preferred| preferred.confidence)
            }),
        ),
        _ => (None, None),
    };
    ActionChoice {
        action,
        target_creature,
        target_point,
    }
}

fn beside_target(actor: Point, target: Point) -> Point {
    let side = if actor.x <= target.x { -1.0 } else { 1.0 };
    Point {
        x: target.x + side * 30.0,
        y: target.y,
    }
}

fn sleep_companion_eligible(bond: BondContext) -> bool {
    bond.distance <= 180.0
        && bond.relationship.affinity >= 96
        && bond.relationship.familiarity >= 48
        && bond.relationship.avoidance < 128
        && !matches!(bond.target_action, ActionKind::Dragged | ActionKind::Tossed)
}

fn sleep_companion_score(context: BehaviorContext) -> f32 {
    context
        .bond
        .filter(|bond| sleep_companion_eligible(*bond))
        .map_or(0.0, |bond| {
            relationship_unit(bond.relationship.affinity) * 0.22
                + relationship_unit(bond.relationship.familiarity) * 0.12
                - relationship_unit(bond.relationship.avoidance) * 0.24
        })
}

fn preferred_region_target(creature: &Creature, desktop: &DesktopSnapshot) -> Option<Point> {
    let preferred = creature.memory.preferred_region?;
    let monitor = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.display_key == preferred.display)?;
    let column = f32::from(preferred.cell.min(8) % 3);
    let row = f32::from(preferred.cell.min(8) / 3);
    Some(Point {
        x: monitor.usable_bounds.x + monitor.usable_bounds.width * ((column + 0.5) / 3.0),
        y: monitor.usable_bounds.y + monitor.usable_bounds.height * ((row + 0.5) / 3.0),
    })
}

fn learned_modifier(creature: &Creature, action: ActionKind) -> f32 {
    let tendencies = creature.tendencies;
    match action {
        ActionKind::InvestigateCursor => LearnedTendencies::utility(tendencies.cursor_trust),
        ActionKind::AvoidCursor => -LearnedTendencies::utility(tendencies.cursor_trust),
        ActionKind::Perch => LearnedTendencies::utility(tendencies.climbing),
        ActionKind::RideWindow => (LearnedTendencies::utility(tendencies.climbing)
            + ride_confidence(creature))
        .clamp(-0.35, 0.35),
        ActionKind::ReactToWindow => -ride_confidence(creature),
        ActionKind::Sleep => LearnedTendencies::utility(tendencies.sleep_security),
        ActionKind::Traverse | ActionKind::Sprint => {
            LearnedTendencies::utility(tendencies.exploration)
        }
        ActionKind::SoloPlay => LearnedTendencies::utility(tendencies.play),
        ActionKind::Greet | ActionKind::Follow => {
            LearnedTendencies::utility(tendencies.sociability)
        }
        ActionKind::SocialPlay => (LearnedTendencies::utility(tendencies.sociability)
            + LearnedTendencies::utility(tendencies.play))
        .clamp(-0.35, 0.35),
        _ => 0.0,
    }
}

fn ride_confidence(creature: &Creature) -> f32 {
    let five_minute_blocks = (creature.memory.window_ride_seconds / (5 * 60)).min(100) as i8;
    LearnedTendencies::utility(five_minute_blocks)
}

fn social_score(
    creature: &Creature,
    context: BehaviorContext,
    action: ActionKind,
    max_distance: f32,
    extra: f32,
) -> f32 {
    match context.bond {
        Some(bond)
            if bond.distance < max_distance
                && !matches!(bond.target_action, ActionKind::Dragged | ActionKind::Tossed)
                && !(matches!(action, ActionKind::Greet | ActionKind::SocialPlay)
                    && matches!(
                        bond.target_action,
                        ActionKind::Sleep | ActionKind::Homebound
                    )) =>
        {
            let affinity = relationship_unit(bond.relationship.affinity);
            let familiarity = relationship_unit(bond.relationship.familiarity);
            let playfulness = relationship_unit(bond.relationship.playfulness);
            let avoidance = relationship_unit(bond.relationship.avoidance);
            let playful = if action == ActionKind::SocialPlay {
                playfulness * 0.5
            } else {
                0.0
            };
            creature.personality.sociability * creature.state.drives.social_need
                + affinity * 0.38
                + familiarity * 0.22
                + playful
                - avoidance * 0.7
                + extra
        }
        None => match context.nearest_creature_distance {
            Some(distance) if distance < max_distance => {
                creature.personality.sociability * creature.state.drives.social_need + extra * 0.5
            }
            _ => -2.0,
        },
        _ => -2.0,
    }
}

fn relationship_unit(score: u8) -> f32 {
    f32::from(score) / f32::from(u8::MAX)
}

fn softmax_sample<R: Rng + ?Sized>(
    scored: &[(ActionKind, f32)],
    temperature: f32,
    rng: &mut R,
) -> ActionKind {
    let max = scored
        .iter()
        .map(|(_, score)| *score)
        .fold(f32::NEG_INFINITY, f32::max);
    let weights: Vec<f32> = scored
        .iter()
        .map(|(_, score)| ((*score - max) / temperature).exp())
        .collect();
    let total: f32 = weights.iter().sum();
    let mut selection = rng.random::<f32>() * total;
    for ((action, _), weight) in scored.iter().zip(weights) {
        selection -= weight;
        if selection <= 0.0 {
            return *action;
        }
    }
    scored.last().map(|item| item.0).unwrap_or(ActionKind::Idle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DesktopRect, DisplayKey, MonitorInfo, World};
    use rand::SeedableRng;
    use rand_chacha::ChaCha12Rng;

    fn fixture() -> (Creature, DesktopSnapshot, BehaviorContext) {
        let desktop = DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: DisplayKey([1; 16]),
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1280.0,
                    height: 800.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 1280.0,
                    height: 736.0,
                },
                scale_factor: 1.0,
                primary: true,
            }],
            ..DesktopSnapshot::default()
        };
        let creature = World::new([73; 32], time::OffsetDateTime::UNIX_EPOCH, &desktop)
            .save
            .creatures
            .remove(0);
        let context = BehaviorContext {
            nearest_creature_distance: None,
            nearest_creature_position: None,
            nearest_creature_id: None,
            bond: None,
            on_window_ledge: false,
            reachable_window_ledge: false,
            window_changed_nearby: false,
            objects: ObjectUtility::default(),
            hour_utc: 12,
        };
        (creature, desktop, context)
    }

    fn selection_count(
        creature: &Creature,
        desktop: &DesktopSnapshot,
        context: BehaviorContext,
        action: ActionKind,
    ) -> usize {
        let mut rng = ChaCha12Rng::from_seed([9; 32]);
        (0..64)
            .filter(|_| choose_action(creature, desktop, context, &mut rng).action == action)
            .count()
    }

    fn selection_count_large(
        creature: &Creature,
        desktop: &DesktopSnapshot,
        context: BehaviorContext,
        action: ActionKind,
    ) -> usize {
        let mut rng = ChaCha12Rng::from_seed([19; 32]);
        (0..4_096)
            .filter(|_| choose_action(creature, desktop, context, &mut rng).action == action)
            .count()
    }

    #[test]
    fn passive_actions_become_preferred_in_their_intended_states() {
        let (mut creature, desktop, context) = fixture();
        creature.personality.activity = 0.0;
        creature.personality.playfulness = 0.0;
        creature.personality.sleep_timing = 0.0;
        creature.personality.routine_affinity = 0.0;
        creature.personality.decision_temperature = 0.08;
        creature.state.drives.energy = 0.05;
        creature.state.drives.sleep_pressure = 0.0;
        creature.state.drives.boredom = 0.0;
        let eat_count = selection_count(&creature, &desktop, context, ActionKind::Eat);
        assert!(eat_count > 20, "eat selected {eat_count}/64 times");

        creature.state.drives.energy = 1.0;
        creature.state.drives.boredom = 0.4;
        creature.state.drives.comfort = 0.0;
        creature.state.drives.arousal = 1.0;
        let drink_count = selection_count(&creature, &desktop, context, ActionKind::Drink);
        assert!(drink_count > 20, "drink selected {drink_count}/64 times");

        creature.personality.activity = 1.0;
        creature.state.drives.boredom = 1.0;
        creature.state.drives.comfort = 0.5;
        let sprint_count = selection_count(&creature, &desktop, context, ActionKind::Sprint);
        assert!(sprint_count > 35, "sprint selected {sprint_count}/64 times");
    }

    #[test]
    fn learned_climbing_changes_probability_monotonically_and_reversibly() {
        let (mut creature, desktop, mut context) = fixture();
        context.reachable_window_ledge = true;
        creature.personality.decision_temperature = 0.5;
        creature.tendencies.climbing = -100;
        let low = selection_count_large(&creature, &desktop, context, ActionKind::Perch);
        creature.tendencies.climbing = 0;
        let neutral = selection_count_large(&creature, &desktop, context, ActionKind::Perch);
        creature.tendencies.climbing = 100;
        let high = selection_count_large(&creature, &desktop, context, ActionKind::Perch);
        creature.tendencies.climbing = -100;
        let reversed = selection_count_large(&creature, &desktop, context, ActionKind::Perch);
        assert!(
            low < neutral && neutral < high,
            "{low} < {neutral} < {high}"
        );
        assert_eq!(low, reversed);
    }

    #[test]
    fn every_learned_modifier_is_bounded_and_has_a_contrary_direction() {
        let (mut creature, _, _) = fixture();
        for value in [-100, 100] {
            creature.tendencies = crate::LearnedTendencies {
                cursor_trust: value,
                sociability: value,
                climbing: value,
                sleep_security: value,
                exploration: value,
                play: value,
                home_affinity: value,
                routine: value,
            };
            for action in [
                ActionKind::InvestigateCursor,
                ActionKind::AvoidCursor,
                ActionKind::Perch,
                ActionKind::Sleep,
                ActionKind::Traverse,
                ActionKind::SoloPlay,
                ActionKind::Greet,
                ActionKind::SocialPlay,
            ] {
                assert!(learned_modifier(&creature, action).abs() <= 0.35);
            }
        }
        creature.tendencies.cursor_trust = 100;
        let trusting = learned_modifier(&creature, ActionKind::InvestigateCursor);
        creature.tendencies.cursor_trust = -100;
        let wary = learned_modifier(&creature, ActionKind::InvestigateCursor);
        assert!(trusting > 0.0 && wary < 0.0);

        creature.memory.window_ride_seconds = 5 * 60 - 1;
        let before_five_minutes = ride_confidence(&creature);
        creature.memory.window_ride_seconds = 5 * 60;
        let after_five_minutes = ride_confidence(&creature);
        creature.memory.window_ride_seconds = u32::MAX;
        assert_eq!(before_five_minutes, 0.0);
        assert!(after_five_minutes > before_five_minutes);
        assert_eq!(ride_confidence(&creature), 0.35);
    }

    #[test]
    fn compact_bond_scores_monotonically_influence_social_selection() {
        let (mut creature, desktop, mut context) = fixture();
        creature.personality.decision_temperature = 0.5;
        creature.state.drives.social_need = 1.0;
        let target_position = Point {
            x: creature.state.position.x + 60.0,
            y: creature.state.position.y,
        };
        let relationship = |affinity, familiarity, playfulness, avoidance| CreatureRelationship {
            a: creature.id.min(99),
            b: creature.id.max(99),
            affinity,
            familiarity,
            playfulness,
            avoidance,
        };
        context.nearest_creature_distance = Some(60.0);
        context.nearest_creature_position = Some(target_position);
        context.nearest_creature_id = Some(99);
        context.bond = Some(BondContext {
            target_creature: 99,
            target_position,
            distance: 60.0,
            relationship: relationship(16, 16, 16, 220),
            target_action: ActionKind::Idle,
            target_surface: SurfaceKind::ScreenFloor,
        });
        let avoided = selection_count_large(&creature, &desktop, context, ActionKind::Greet)
            + selection_count_large(&creature, &desktop, context, ActionKind::SocialPlay);
        context.bond.as_mut().unwrap().relationship = relationship(220, 220, 220, 0);
        let bonded = selection_count_large(&creature, &desktop, context, ActionKind::Greet)
            + selection_count_large(&creature, &desktop, context, ActionKind::SocialPlay);
        assert!(bonded > avoided, "bonded {bonded}, avoided {avoided}");

        let mut rng = ChaCha12Rng::from_seed([27; 32]);
        let sleep_choice = (0..10_000)
            .map(|_| choose_action(&creature, &desktop, context, &mut rng))
            .find(|choice| choice.action == ActionKind::Sleep)
            .expect("sleep remains selectable");
        assert_eq!(sleep_choice.target_creature, Some(99));
        assert_ne!(sleep_choice.target_point, Some(target_position));
    }

    #[test]
    fn nearby_object_utility_is_role_specific_and_capped() {
        let mut utility = ObjectUtility::default();
        utility.add(crate::ColonyObjectRole::Play, 0.08);
        assert_eq!(utility.for_action(ActionKind::SoloPlay), 0.08);
        assert_eq!(utility.for_action(ActionKind::Sleep), 0.0);
        for _ in 0..8 {
            utility.add(crate::ColonyObjectRole::Play, 0.08);
        }
        assert_eq!(utility.play, 0.25);
        assert_eq!(utility.for_action(ActionKind::SocialPlay), 0.25);
    }
}
