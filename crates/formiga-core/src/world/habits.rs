use super::*;

/// A companion starting one of the everyday things a habit can belong to. If it already has a
/// habit for moments like this one it usually does it. If it has room for another and `learning`
/// allows, this may be the moment it picks one up, and then it does it straight away.
///
/// Reduced motion keeps every habit the creature has and still lets it pick up new ones; it only
/// stops the flourish being shown, the way it stops every other body pose.
pub(super) fn cue_habit(
    creature: &mut Creature,
    action: ActionKind,
    rng: &mut ChaCha12Rng,
    learning: bool,
    reduce_motion: bool,
    events: &mut Vec<WorldEvent>,
) {
    creature.state.flourish = None;
    let Some(cue) = HabitCue::for_action(action) else {
        return;
    };
    let habit = if learning
        && let Some(habit) = habit_to_learn(creature, cue)
        && rng.random_bool(learning_chance(creature, habit))
    {
        creature.memory.habits.push(habit);
        creature.memory.profile_revision = creature.memory.profile_revision.saturating_add(1);
        events.push(WorldEvent::HabitLearned {
            creature_id: creature.id,
            habit,
        });
        habit
    } else {
        match habit_for(creature, cue) {
            Some(habit) if rng.random_bool(HABIT_PERFORMANCE_CHANCE) => habit,
            _ => return,
        }
    };
    if !reduce_motion {
        creature.state.flourish = Some(Flourish {
            habit,
            action,
            started_at: None,
        });
    }
}

/// Starts a waiting flourish once the companion is where it is going to do the thing, and ends
/// one that is over, that waited too long, or whose action gave way to another.
///
/// A meal or a game alone is had where the creature is, so its habit starts at once. A nap, a
/// greeting or a visit to the pointer may begin with a walk to a pillow, a friend or the pointer,
/// and its habit waits until the creature has got there and stopped.
pub(super) fn advance_flourish(creature: &mut Creature) {
    let state = &mut creature.state;
    let Some(mut flourish) = state.flourish else {
        return;
    };
    let walks_first = matches!(
        flourish.action,
        ActionKind::Sleep
            | ActionKind::Greet
            | ActionKind::SocialPlay
            | ActionKind::InvestigateCursor
    );
    let still = state.velocity.x.abs() < 1.0 && state.velocity.y.abs() < 1.0;
    let over = match flourish.started_at {
        _ if state.action != flourish.action => true,
        None if still || !walks_first => {
            flourish.started_at = Some(state.action_elapsed);
            state.velocity = Point::default();
            false
        }
        None => state.action_elapsed > FLOURISH_WAIT_SECS,
        Some(started) => {
            let into = state.action_elapsed - started;
            !(0.0..flourish.habit.seconds()).contains(&into)
        }
    };
    state.flourish = (!over).then_some(flourish);
}

/// Whether a flourish is holding the companion where it stands.
pub(super) fn flourishing(creature: &Creature) -> bool {
    creature
        .state
        .flourish
        .is_some_and(|flourish| flourish.progress(&creature.state).is_some())
}
