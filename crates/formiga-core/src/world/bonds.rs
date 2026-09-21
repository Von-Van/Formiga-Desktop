use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BondPlan {
    pub(super) target: CreatureId,
    pub(super) final_action: ActionKind,
    pub(super) experience: RelationshipExperience,
    pub(super) approaching: bool,
}

pub(super) fn normalize_relationships(save: &mut SaveFile) {
    let creature_ids: BTreeSet<_> = save.creatures.iter().map(|creature| creature.id).collect();
    let mut canonical = BTreeMap::new();
    for mut relationship in save.relationships.drain(..) {
        let Some((a, b)) = canonical_creature_pair(relationship.a, relationship.b) else {
            continue;
        };
        if !creature_ids.contains(&a) || !creature_ids.contains(&b) {
            continue;
        }
        relationship.a = a;
        relationship.b = b;
        canonical
            .entry((a, b))
            .and_modify(|existing: &mut CreatureRelationship| {
                existing.affinity = existing.affinity.max(relationship.affinity);
                existing.familiarity = existing.familiarity.max(relationship.familiarity);
                existing.playfulness = existing.playfulness.max(relationship.playfulness);
                existing.avoidance = existing.avoidance.max(relationship.avoidance);
            })
            .or_insert(relationship);
    }
    let ids: Vec<_> = creature_ids.into_iter().collect();
    for (index, a) in ids.iter().copied().enumerate() {
        for b in ids.iter().copied().skip(index + 1) {
            canonical
                .entry((a, b))
                .or_insert_with(|| CreatureRelationship::new(a, b).expect("distinct pair"));
        }
    }
    save.relationships = canonical.into_values().take(MAX_RELATIONSHIPS).collect();
}

pub(super) fn relationship_mut_or_insert(
    relationships: &mut Vec<CreatureRelationship>,
    a: CreatureId,
    b: CreatureId,
) -> Option<&mut CreatureRelationship> {
    let pair = canonical_creature_pair(a, b)?;
    if let Some(index) = relationships
        .iter()
        .position(|relationship| relationship.a == pair.0 && relationship.b == pair.1)
    {
        return relationships.get_mut(index);
    }
    if relationships.len() >= MAX_RELATIONSHIPS {
        return None;
    }
    relationships.push(CreatureRelationship::new(pair.0, pair.1)?);
    relationships.last_mut()
}

pub(super) fn calm_for_proximity(action: ActionKind) -> bool {
    !matches!(
        action,
        ActionKind::Dragged
            | ActionKind::Tossed
            | ActionKind::AvoidCursor
            | ActionKind::ReactToWindow
            | ActionKind::Sprint
            | ActionKind::ClimbWindow
            | ActionKind::Dangle
    )
}

pub(super) fn add_arrival_relationships(
    relationships: &mut Vec<CreatureRelationship>,
    creatures: &[Creature],
    arriving: CreatureId,
    parent: Option<CreatureId>,
) {
    for creature in creatures {
        if relationships.len() >= MAX_RELATIONSHIPS {
            break;
        }
        let Some(mut relationship) = CreatureRelationship::new(creature.id, arriving) else {
            continue;
        };
        if parent == Some(creature.id) {
            relationship.affinity = 217;
            relationship.familiarity = 48;
            relationship.playfulness = 64;
        }
        relationships.push(relationship);
    }
    relationships.sort_by_key(|relationship| (relationship.a, relationship.b));
    relationships.dedup_by_key(|relationship| (relationship.a, relationship.b));
}

pub(super) fn preferred_bond_context(
    creature: &Creature,
    creatures: &[Creature],
    relationships: &[CreatureRelationship],
) -> Option<BondContext> {
    relationships
        .iter()
        .copied()
        .filter_map(|relationship| {
            let target_id = relationship.other(creature.id)?;
            let target = creatures
                .iter()
                .find(|target| target.id == target_id && target.state.arrival_delay_secs <= 0.0)?;
            let distance = creature.state.position.distance(target.state.position);
            let same_monitor = creature.state.surface.monitor_id == target.state.surface.monitor_id;
            let score = relationship.closeness()
                + i16::from(relationship.playfulness) / 2
                + if same_monitor { 32 } else { -128 };
            Some((
                score,
                std::cmp::Reverse(target.id),
                target,
                relationship,
                distance,
            ))
        })
        .max_by_key(|(score, target_id, ..)| (*score, *target_id))
        .map(|(_, _, target, relationship, distance)| BondContext {
            target_creature: target.id,
            target_position: target.state.position,
            distance,
            relationship,
            target_action: target.state.action,
            target_surface: target.state.surface.kind,
        })
}

pub(super) fn relationship_experience_for_action(
    action: ActionKind,
) -> Option<RelationshipExperience> {
    match action {
        ActionKind::Follow => Some(RelationshipExperience::Followed),
        ActionKind::Greet => Some(RelationshipExperience::Greeting),
        ActionKind::Sleep => Some(RelationshipExperience::SharedRest),
        ActionKind::SocialPlay => Some(RelationshipExperience::PositivePlay),
        _ => None,
    }
}

/// Whether a walk is worth making to reach a bond's mark, measured in how wide a creature draws
/// so the answer is the same at every display scale.
/// A walk stops within a stride or so of the mark it was given rather than exactly on it, so
/// every mark carries that much on top of the distance it is really asking for.
pub(super) const BOND_SETTLE: f32 = 5.0;

pub(super) fn bond_approach_required(
    actor: Point,
    target: Point,
    final_action: ActionKind,
    frame_width: f32,
) -> bool {
    let threshold = match final_action {
        ActionKind::Sleep => 0.81,
        ActionKind::Greet | ActionKind::SocialPlay => 1.0,
        ActionKind::PresentDiscovery => 1.34,
        _ => return false,
    };
    actor.distance(target) > threshold * frame_width
}

pub(super) fn bond_target_point(
    actor: &Creature,
    creatures: &[Creature],
    target_id: CreatureId,
    action: ActionKind,
    frame_width: f32,
) -> Option<Point> {
    let target = creatures.iter().find(|target| {
        target.id == target_id
            && target.state.arrival_delay_secs <= 0.0
            && target.state.surface.monitor_id == actor.state.surface.monitor_id
            && (action == ActionKind::ReactToWindow
                || target.state.surface.kind == actor.state.surface.kind)
    })?;
    let allowed = match action {
        ActionKind::ReactToWindow => !matches!(
            target.state.action,
            ActionKind::Dragged | ActionKind::Homebound
        ),
        ActionKind::InspectScreen => matches!(
            target.state.action,
            ActionKind::ClimbWindow | ActionKind::Perch
        ),
        ActionKind::Sleep => !matches!(
            target.state.action,
            ActionKind::Dragged | ActionKind::Tossed | ActionKind::Homebound
        ),
        ActionKind::Greet | ActionKind::SocialPlay | ActionKind::PresentDiscovery => !matches!(
            target.state.action,
            ActionKind::Sleep | ActionKind::Dragged | ActionKind::Tossed | ActionKind::Homebound
        ),
        ActionKind::Follow => !matches!(
            target.state.action,
            ActionKind::Sleep | ActionKind::Dragged | ActionKind::Tossed | ActionKind::Homebound
        ),
        _ => true,
    };
    if !allowed {
        return None;
    }
    // A mark is a place to stand, so it is measured in how wide a creature draws rather than in
    // flat points that meant something different at each display scale. Shoulder to shoulder is
    // the closest any of them comes: paws, motifs, and expressions still read as one interaction,
    // and neither face ends up behind the other body for as long as the interaction lasts.
    let staging_offset = match action {
        ActionKind::Greet | ActionKind::SocialPlay | ActionKind::Sleep => spacing::FACE_CLEAR_RATIO,
        ActionKind::PresentDiscovery => spacing::FACE_CLEAR_RATIO * 1.1,
        // A follower trails a little further back than a companion standing alongside.
        ActionKind::Follow => spacing::FACE_CLEAR_RATIO * 1.25,
        _ => return Some(target.state.position),
    } * frame_width
        + BOND_SETTLE;
    let side = if actor.state.position.x < target.state.position.x {
        -1.0
    } else if actor.state.position.x > target.state.position.x {
        1.0
    } else if actor.id < target.id {
        -1.0
    } else {
        1.0
    };
    // A mark measured against one companion keeps the actor clear of that companion and of nobody
    // else. On a floor with room to spare those are the same thing; on a crowded one they are not,
    // and a mark that lands on a third creature leaves two companions drawn through one another
    // while each of them is standing exactly where its own errand sent it — so neither can be
    // asked to move, and the pair stays that way for as long as the errands last.
    //
    // So on open ground the mark slides outward past whoever is already standing on it: always
    // further from the companion, and always on the side the actor is on, because sending it round
    // to the other side would turn the actor about the moment it crossed over and it would spend
    // the errand walking back and forth. A ledge is not open ground — there is nowhere to slide
    // to, and overlap handling has its own answer for a perch with no room left — so a mark on one
    // is left where it falls.
    let mut x = target.state.position.x + side * staging_offset;
    if actor.state.surface.kind == SurfaceKind::ScreenFloor {
        let clear = spacing::FACE_CLEAR_RATIO * frame_width;
        for _ in 0..creatures.len() {
            let blocker = creatures.iter().find(|other| {
                other.id != actor.id
                    && other.id != target.id
                    && other.state.arrival_delay_secs <= 0.0
                    && other.state.surface.monitor_id == actor.state.surface.monitor_id
                    && other.state.surface.kind == SurfaceKind::ScreenFloor
                    && (other.state.position.x - x).abs() < clear
            });
            let Some(blocker) = blocker else { break };
            x = blocker.state.position.x + side * (clear + BOND_SETTLE);
        }
    }
    Some(Point {
        x,
        y: target.state.position.y,
    })
}
