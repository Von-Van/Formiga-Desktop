//! How each participant in an attention scene is shown on a tick: the action it plays, where
//! it looks, how it feels, and the pose struck over that, and how it is let go when the scene
//! ends.
use super::*;

/// How long a creature that caught the far edge teeters there once it has pulled itself up.
pub(super) const CATCH_WOBBLE_SECONDS: f32 = 0.5;
/// How close a stationary investigator must be to the cursor to reach out for it.
pub(super) const CURSOR_REACH: f32 = 120.0;

/// Present one plan for this tick: the action, where the creature looks and how it feels, and any
/// body pose struck over that action. `held` is false when something after presentation still
/// moves the creature this tick — its own journey stepping on, or a toss — so a pose chosen now
/// could not be trusted to match what is drawn.
pub(super) fn present(
    creature: &mut Creature,
    plan: &mut Reaction,
    reduced_motion: bool,
    held: bool,
) {
    present_role(creature, plan, reduced_motion);
    // Every role proposes its own pose. This is the one place that decides whether the body is
    // actually free to show it, so no role can put a pose over travel by mistake.
    if !body_free(creature, plan, reduced_motion, held)
        && let Some(pose) = &mut creature.state.attention
    {
        pose.gesture = None;
    }
}

/// Whether a gesture may stand in for the action's own clip. Only a planted presentation gives
/// its body over to a pose: never with reduced motion, never while walking, hopping, carried by a
/// journey or a toss, or hanging by the hands, and never over an action whose clip is itself the
/// point, such as a sprint, a squeeze, a meal, a ride, a nap, or a toy being shown off.
///
/// What is actually moving the creature decides this, rather than its velocity: an approach that
/// has just handed over to a journey leaves the last stride on the books for a while, and a
/// creature standing perfectly still at the edge of a gap is not travelling anywhere.
pub(super) fn body_free(
    creature: &Creature,
    plan: &Reaction,
    reduced_motion: bool,
    held: bool,
) -> bool {
    let planted = matches!(
        creature.state.action,
        ActionKind::Idle
            | ActionKind::Perch
            | ActionKind::InspectScreen
            | ActionKind::Greet
            | ActionKind::SocialPlay
            | ActionKind::SoloPlay
            | ActionKind::ReactToWindow
            | ActionKind::InvestigateCursor
    );
    planted
        && held
        && !reduced_motion
        && plan.walk.is_none()
        && plan.display_walk.is_none()
        && !matches!(plan.role, Role::Play { hopping: true, .. })
        && creature
            .state
            .attention
            .is_some_and(|pose| pose.hanging <= 0.0)
}

/// Whether this tick's step of a creature's own journey leaves it exactly where, and as, it was
/// just presented. The journey moves the creature after presentation, so a pose chosen during a
/// pause must not survive into the step that sets it going again.
pub(super) fn journey_holds_still(journey: &WindowJourney, creature: &Creature, dt: f32) -> bool {
    let step = journey.clone().advance(dt);
    !step.complete
        && step.action == creature.state.action
        && step.position == creature.state.position
}

/// A helper hauls on its companion while it hangs, and cheers only once the companion is up.
pub(super) fn helper_pose(plan: &Reaction) -> Option<Gesture> {
    let rescued = plan
        .cue
        .is_some_and(|actor| actor.stage == Stage::Recover(Outcome::Completed));
    match spectacle::cue(plan)?.stage {
        Stage::Catch | Stage::Recover(_) if rescued => Some(Gesture::Cheer),
        Stage::Catch => Some(Gesture::Heave),
        _ => None,
    }
}

pub(super) fn present_role(creature: &mut Creature, plan: &mut Reaction, reduced_motion: bool) {
    if let Role::Play {
        gesture,
        hopping,
        pose,
        ..
    } = plan.role
    {
        if hopping {
            // A leap in progress owns its own contact, action, and timing.
            plan.action = creature.state.action;
            creature.state.attention = Some(AttentionPose {
                target: plan.target,
                emotion: plan.emotion,
                hanging: 0.0,
                gesture: None,
            });
            return;
        }
        let walking = plan.walk.is_some() && !reduced_motion;
        let action = if reduced_motion {
            ActionKind::InspectScreen
        } else {
            gesture
        };
        if creature.state.action != action {
            creature.state.action_elapsed = 0.0;
        }
        creature.state.action = action;
        creature.state.action_duration = f32::MAX;
        // A travelling player is steered by its own walk, which sets facing and velocity.
        if !walking {
            creature.state.velocity = Point::default();
            creature.state.facing_right = plan.target.x >= creature.state.position.x;
        }
        creature.state.attention = Some(AttentionPose {
            target: plan.target,
            emotion: plan.emotion,
            hanging: 0.0,
            gesture: pose,
        });
        plan.action = action;
        return;
    }
    if let Role::Tumble { stage } = plan.role {
        if matches!(stage, Stage::Recover(_)) {
            creature.state.action = if creature.state.surface.window_key.is_some() {
                ActionKind::Perch
            } else {
                ActionKind::Idle
            };
            creature.state.velocity = Point::default();
            creature.state.action_duration = f32::MAX;
        }
        plan.action = creature.state.action;
        let recovering = matches!(stage, Stage::Recover(_));
        creature.state.attention = Some(AttentionPose {
            target: plan.target,
            emotion: if recovering {
                AttentionEmotion::Relieved
            } else {
                AttentionEmotion::Startled
            },
            hanging: 0.0,
            // A tumble is tossed or landing until it recovers, and both keep the body, so the
            // fall itself has no pose to strike.
            gesture: None,
        });
        return;
    }
    if let Role::Dare { landing, .. } = plan.role {
        let walking = plan.walk.is_some() && !reduced_motion;
        let action = if walking {
            ActionKind::Traverse
        } else {
            ActionKind::InspectScreen
        };
        if creature.state.action != action {
            creature.state.action_elapsed = 0.0;
        }
        creature.state.action = action;
        creature.state.action_duration = f32::MAX;
        if !walking {
            creature.state.velocity = Point::default();
            creature.state.facing_right = landing.x > creature.state.position.x;
        }
        plan.action = action;
        creature.state.attention = Some(AttentionPose {
            target: landing,
            emotion: plan.emotion,
            hanging: 0.0,
            // Facing the gap it was dared to try: a bold creature squares up to it, and a timid
            // one frets.
            gesture: Some(if creature.personality.boldness >= 0.5 {
                Gesture::Crouch
            } else {
                Gesture::Worry
            }),
        });
        return;
    }
    if let Role::Hesitate {
        phase,
        phase_elapsed,
        landing,
        edge,
        drop,
        ..
    } = plan.role
    {
        let walking = plan.walk.is_some() && !reduced_motion;
        let below = Point {
            x: edge.x + if landing.x >= edge.x { 20.0 } else { -20.0 },
            y: edge.y + drop.clamp(12.0, 180.0),
        };
        let (action, target, emotion, gesture) = match phase {
            HesitatePhase::Look if phase_elapsed < 0.45 => (
                ActionKind::InspectScreen,
                landing,
                AttentionEmotion::Curious,
                None,
            ),
            HesitatePhase::Look => (
                ActionKind::InspectScreen,
                below,
                AttentionEmotion::Concerned,
                Some(Gesture::Worry),
            ),
            HesitatePhase::BackUp | HesitatePhase::Approach if walking => (
                ActionKind::Traverse,
                landing,
                AttentionEmotion::Concerned,
                None,
            ),
            HesitatePhase::BackUp | HesitatePhase::Approach => (
                ActionKind::InspectScreen,
                landing,
                AttentionEmotion::Concerned,
                Some(Gesture::Worry),
            ),
            // Leaning over the edge: alternate between the landing and the drop below it.
            HesitatePhase::Reconsider => (
                ActionKind::InspectScreen,
                if ((phase_elapsed / 0.5) as u32).is_multiple_of(2) {
                    below
                } else {
                    landing
                },
                AttentionEmotion::Concerned,
                Some(Gesture::Balance),
            ),
            HesitatePhase::Retreat if walking => (
                ActionKind::Traverse,
                landing,
                AttentionEmotion::Relieved,
                None,
            ),
            HesitatePhase::Retreat => {
                (ActionKind::Perch, landing, AttentionEmotion::Relieved, None)
            }
        };
        if creature.state.action != action {
            creature.state.action = action;
            creature.state.action_elapsed = 0.0;
        }
        plan.action = action;
        creature.state.action_duration = f32::MAX;
        if !walking {
            creature.state.velocity = Point::default();
            if (landing.x - creature.state.position.x).abs() > 8.0 {
                creature.state.facing_right = landing.x > creature.state.position.x;
            }
        }
        creature.state.attention = Some(AttentionPose {
            target,
            emotion,
            hanging: 0.0,
            gesture,
        });
        return;
    }
    if let Role::Journey {
        stage,
        hanging,
        rewarding,
        escape,
        since,
        caught,
        ..
    } = plan.role
    {
        if matches!(stage, Stage::Recover(_)) {
            creature.state.action = if !reduced_motion && rewarding {
                ActionKind::Greet
            } else if creature.state.surface.window_key.is_some() {
                ActionKind::Perch
            } else {
                ActionKind::Idle
            };
            creature.state.velocity = Point::default();
        }
        plan.action = creature.state.action;
        let in_stage = plan.elapsed - since;
        let gesture = match stage {
            // Pulled back up after catching the far edge: a wobble at the brink, then delight.
            Stage::Recover(Outcome::Completed) if caught && in_stage < CATCH_WOBBLE_SECONDS => {
                Some(Gesture::Balance)
            }
            Stage::Recover(Outcome::Completed) if rewarding => Some(Gesture::Cheer),
            Stage::Recover(_) => None,
            // Squaring up to a real leap. A gap's run-up is travel, so the still moment at the
            // edge before it sets off is the leap's only wind-up a pose can show over.
            Stage::Notice if rewarding && in_stage >= 0.15 => Some(Gesture::Crouch),
            // The pause before a climb, or before slipping through a squeeze.
            Stage::Prepare => Some(Gesture::Crouch),
            _ => None,
        };
        creature.state.attention = Some(AttentionPose {
            target: plan.target,
            emotion: match stage {
                Stage::Recover(Outcome::Completed) => AttentionEmotion::Enjoying,
                Stage::Recover(_) => AttentionEmotion::Relieved,
                _ if escape => AttentionEmotion::Startled,
                Stage::Act => AttentionEmotion::Concerned,
                Stage::Catch => AttentionEmotion::Startled,
                _ => AttentionEmotion::Curious,
            },
            hanging,
            gesture,
        });
        return;
    }
    let elapsed = plan.elapsed - plan.delay - plan.travel_elapsed;
    let walking = (plan.walk.is_some() || plan.display_walk.is_some())
        && plan.elapsed - plan.delay >= 0.45
        && !reduced_motion;
    let mut emotion = if elapsed < 0.0 {
        None
    } else if elapsed < 0.45 && !walking {
        Some(AttentionEmotion::Curious)
    } else if elapsed < 2.15 {
        Some(plan.emotion)
    } else {
        Some(match plan.emotion {
            AttentionEmotion::Startled | AttentionEmotion::Concerned => AttentionEmotion::Relieved,
            _ => AttentionEmotion::Curious,
        })
    };
    let rider = matches!(plan.role, Role::Actor { window, .. } if Some(window) == creature.state.surface.window_key);
    let mut action = if walking {
        if matches!(plan.role, Role::Cursor { .. }) && plan.emotion == AttentionEmotion::Concerned {
            ActionKind::AvoidCursor
        } else if matches!(
            plan.role,
            Role::Cursor {
                investigate: true,
                ..
            } | Role::Cursor { racing: true, .. }
        ) {
            ActionKind::InvestigateCursor
        } else if plan.emotion == AttentionEmotion::Startled {
            ActionKind::ReactToWindow
        } else {
            ActionKind::Traverse
        }
    } else if rider && !reduced_motion {
        if elapsed >= 2.15 {
            ActionKind::Perch
        } else if plan.emotion == AttentionEmotion::Startled {
            ActionKind::ReactToWindow
        } else {
            ActionKind::RideWindow
        }
    } else if matches!(plan.role, Role::Display { discovery: true }) && elapsed >= 2.15 {
        ActionKind::Perch
    } else {
        ActionKind::InspectScreen
    };
    if matches!(plan.role, Role::Ledge { .. }) && !walking && elapsed >= 2.15 {
        action = ActionKind::Perch;
    }
    if matches!(plan.role, Role::Helper { .. }) && !walking && !reduced_motion {
        action = ActionKind::SocialPlay;
    }
    let mut target = plan.target;
    if matches!(plan.role, Role::Ledge { declined: true, .. })
        && ((0.85..1.15).contains(&elapsed) || (1.65..1.95).contains(&elapsed))
    {
        action = ActionKind::Perch;
        target = Point {
            x: creature.state.position.x,
            y: creature.state.position.y + 40.0,
        };
    }
    let mut watched = None;
    if let Some(cue) = plan.cue
        && elapsed >= 0.0
    {
        spectacle::present_observer(
            creature,
            cue,
            &mut spectacle::Watching {
                reduced: reduced_motion,
                walking,
                watching_for: elapsed,
                action: &mut action,
                emotion: &mut emotion,
                target: &mut target,
                gesture: &mut watched,
            },
        );
    }
    let hanging = if matches!(plan.role, Role::Ledge { commute: true, .. }) && !reduced_motion {
        if elapsed < 0.45 {
            0.0
        } else if elapsed < 1.45 {
            action = ActionKind::Dangle;
            ((elapsed - 0.45) / 0.45).clamp(0.0, 1.0)
        } else if elapsed < 2.15 {
            action = ActionKind::ClimbWindow;
            (1.0 - (elapsed - 1.45) / 0.7).clamp(0.0, 1.0)
        } else {
            0.0
        }
    } else if rider && let Some(ride) = plan.ride {
        rides::present_ride(
            creature,
            plan,
            ride,
            reduced_motion,
            walking,
            &mut action,
            &mut target,
        )
    } else {
        0.0
    };
    if action != creature.state.action {
        creature.state.action = action;
        creature.state.action_elapsed = 0.0;
    }
    plan.action = action;
    creature.state.action_duration = f32::MAX;
    if !walking {
        creature.state.velocity = Point::default();
    }
    if matches!(plan.role, Role::Actor { vanished: true, .. }) && (0.45..2.15).contains(&elapsed) {
        target.x += if (((elapsed - 0.45) / 0.55) as u32).is_multiple_of(2) {
            -48.0
        } else {
            48.0
        };
    }
    let gesture = match plan.role {
        Role::Observer { .. } => watched,
        Role::Helper { .. } => helper_pose(plan),
        // The support is simply gone: a gasp, then a look around for where it went. Nothing is
        // there to watch, so no watching pose belongs here.
        Role::Actor { vanished: true, .. } => (elapsed < 0.45).then_some(Gesture::Gasp),
        // A window arriving, growing, moving, or being shoved about with the rest of the desktop.
        // A startled creature flinches where it stands; once the notice beat is over, anyone still
        // planted beside or on the window settles into watching it, which is the whole point of
        // the scene. A rider's own pose still belongs to the ride while the ride owns its body,
        // and presentation drops this one if it does.
        Role::Actor { .. } => {
            if emotion == Some(AttentionEmotion::Startled) {
                (!rider).then_some(Gesture::Gasp)
            } else {
                (elapsed >= 0.45).then_some(Gesture::Watch)
            }
        }
        // Close enough to the cursor to reach for it, once it has had a look.
        Role::Cursor {
            investigate: true, ..
        } => (plan.emotion == AttentionEmotion::Curious
            && elapsed >= 0.45
            && (plan.target.x - creature.state.position.x).abs() > 8.0
            && plan.target.distance(creature.state.position) <= CURSOR_REACH)
            .then_some(Gesture::Reach),
        // Looking over a long way down. A commute's hang and climb keep the body, and settling
        // back onto a familiar spot is not an inspection of anything.
        Role::Ledge {
            commute: false,
            resting: false,
            ..
        } => {
            if emotion == Some(AttentionEmotion::Concerned) {
                Some(Gesture::Worry)
            } else {
                // Planted at the edge with the drop in view: the same held look a creature gives
                // a window from beside it, given to the one it is standing on.
                (elapsed >= 0.45).then_some(Gesture::Watch)
            }
        }
        _ => None,
    };
    creature.state.attention = emotion.map(|emotion| AttentionPose {
        target,
        emotion,
        hanging,
        gesture,
    });
    if !walking && elapsed >= 0.0 && (plan.target.x - creature.state.position.x).abs() > 8.0 {
        creature.state.facing_right = plan.target.x > creature.state.position.x;
    }
}

pub(super) fn actor_emotion(creature: &Creature, signal: GeometrySignal) -> AttentionEmotion {
    let tolerance =
        creature.personality.window_tolerance * 0.65 + creature.personality.boldness * 0.35;
    if creature.state.surface.window_key == Some(signal.window) && tolerance >= 0.65 {
        AttentionEmotion::Enjoying
    } else if matches!(
        signal.kind,
        GeometryChange::Expanded | GeometryChange::Moved
    ) && signal.strength > 0.5
        && tolerance < 0.65
    {
        AttentionEmotion::Startled
    } else {
        AttentionEmotion::Curious
    }
}

pub(super) fn release(creature: &mut Creature, plan: Reaction) {
    creature.state.attention = None;
    if matches!(
        plan.role,
        Role::Journey {
            stage: Stage::Notice | Stage::Prepare | Stage::Act | Stage::Catch,
            ..
        }
    ) {
        return;
    }
    if matches!(plan.role, Role::Tumble { stage: Stage::Act })
        && creature.state.action == ActionKind::Tossed
    {
        return;
    }
    // A leap already in the air lands on its own terms.
    if matches!(plan.role, Role::Play { hopping: true, .. }) {
        return;
    }
    if creature.state.action == plan.action
        && (
            creature.state.surface.monitor_id,
            creature.state.surface.window_key,
        ) == plan.surface
    {
        creature.state.action = if creature.state.surface.kind == SurfaceKind::WindowLedge {
            ActionKind::Perch
        } else {
            ActionKind::Idle
        };
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 2.5;
        creature.state.velocity = Point::default();
    }
}
