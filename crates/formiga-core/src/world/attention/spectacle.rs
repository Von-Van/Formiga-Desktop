//! One-way visible intentions. Observers consume cues; they never publish another origin.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Completed,
    Declined,
    Slipped,
    Settled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stage {
    Notice,
    Prepare,
    Act,
    Catch,
    Recover(Outcome),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Cue {
    pub stage: Stage,
    pub risky: bool,
    /// Seconds the actor has spent in `stage`; a sudden stage can draw a brief gasp.
    pub since: f32,
    pub sudden: bool,
}

/// Spectators gasp only during the first moment of a catch or an unplanned fall.
const GASP_SECONDS: f32 = 0.4;

pub(super) fn cue(plan: &Reaction) -> Option<Cue> {
    if let Role::Play { stage, hopping, .. } = plan.role {
        return Some(Cue {
            stage,
            // A game is ordinarily safe to watch, but a companion in the air over a gap is the
            // same sight as any other leap, and a timid watcher is entitled to look away.
            risky: hopping,
            since: plan.elapsed,
            sudden: false,
        });
    }
    if let Role::Journey {
        stage,
        rewarding,
        escape,
        since,
        ..
    } = plan.role
    {
        return Some(Cue {
            stage,
            risky: rewarding || escape,
            since: plan.elapsed - since,
            sudden: escape,
        });
    }
    if let Role::Tumble { stage } = plan.role {
        return Some(Cue {
            stage,
            risky: true,
            since: plan.elapsed,
            sudden: true,
        });
    }
    if let Role::Hesitate {
        phase,
        phase_elapsed,
        attempt,
        ..
    } = plan.role
    {
        return Some(Cue {
            stage: match phase {
                HesitatePhase::Look if attempt == 1 && phase_elapsed < 0.45 => Stage::Notice,
                HesitatePhase::Look | HesitatePhase::BackUp | HesitatePhase::Approach => {
                    Stage::Prepare
                }
                HesitatePhase::Reconsider => Stage::Act,
                HesitatePhase::Retreat => Stage::Recover(Outcome::Declined),
            },
            risky: true,
            since: phase_elapsed,
            sudden: false,
        });
    }
    // Reaching down for someone who is hanging is as visible an intention as the jump was, and
    // reads through the same stages: going to them, holding on, and the moment it is over.
    if let Role::Helper { .. } = plan.role {
        return Some(Cue {
            stage: if plan.walk.is_some() {
                Stage::Prepare
            } else if plan.elapsed < plan.seconds - 0.9 {
                Stage::Catch
            } else {
                Stage::Recover(Outcome::Completed)
            },
            risky: true,
            since: plan.elapsed,
            sudden: false,
        });
    }
    if let Role::Dare { .. } = plan.role {
        return Some(Cue {
            stage: if plan.elapsed < 0.45 {
                Stage::Notice
            } else {
                Stage::Prepare
            },
            risky: true,
            since: plan.elapsed,
            sudden: false,
        });
    }
    // Window reactions, cursor interest, and display exploration publish the same stages, so a
    // watcher gasps at a startle and settles with it rather than reading the desktop itself.
    if matches!(
        plan.role,
        Role::Actor { .. } | Role::Cursor { .. } | Role::Display { .. }
    ) {
        let elapsed = plan.elapsed - plan.delay - plan.travel_elapsed;
        return Some(Cue {
            stage: if elapsed < 0.45 {
                Stage::Notice
            } else if elapsed < 2.15 {
                Stage::Act
            } else {
                Stage::Recover(Outcome::Settled)
            },
            risky: false,
            since: elapsed.max(0.0) % 2.15,
            sudden: plan.emotion == AttentionEmotion::Startled,
        });
    }
    let Role::Ledge {
        drop,
        resting,
        declined,
        commute,
        ..
    } = plan.role
    else {
        return None;
    };
    let elapsed = plan.elapsed - plan.delay - plan.travel_elapsed;
    Some(Cue {
        risky: !resting && drop > 140.0,
        stage: if elapsed < 0.45 {
            Stage::Notice
        } else if plan.walk.is_some() && !commute {
            Stage::Prepare
        } else if elapsed < 2.15 {
            Stage::Act
        } else {
            Stage::Recover(if declined {
                Outcome::Declined
            } else {
                Outcome::Completed
            })
        },
        since: elapsed,
        sudden: false,
    })
}

/// Every watcher gets its own beginning, even one that looks up in the middle of the action.
const NOTICE_SECONDS: f32 = 0.35;

/// What a watcher shows: its pose, how it feels, and where it is looking.
pub(super) struct Watching<'a> {
    pub reduced: bool,
    pub walking: bool,
    /// Seconds this watcher has been part of the scene, so it gets its own beginning.
    pub watching_for: f32,
    pub action: &'a mut ActionKind,
    pub emotion: &'a mut Option<AttentionEmotion>,
    pub target: &'a mut Point,
    /// The body pose the response calls for. Presentation still decides whether it can show.
    pub gesture: &'a mut Option<Gesture>,
}

pub(super) fn present_observer(creature: &Creature, cue: Cue, watching: &mut Watching) {
    let Watching {
        reduced,
        walking,
        watching_for,
        action,
        emotion,
        target,
        gesture,
    } = watching;
    let (reduced, walking, watching_for) = (*reduced, *walking, *watching_for);
    **gesture = None;
    if watching_for < NOTICE_SECONDS {
        **emotion = Some(AttentionEmotion::Curious);
        return;
    }
    let gasp = !reduced && cue.since < GASP_SECONDS;
    // Fretting belongs to a companion taking a real risk, not to a window that merely jumped.
    let fret = cue.risky.then_some(Gesture::Worry);
    let (feeling, pose) = match cue.stage {
        Stage::Notice | Stage::Prepare => (AttentionEmotion::Curious, None),
        Stage::Catch if gasp => (AttentionEmotion::Startled, Some(Gesture::Gasp)),
        // A sudden event draws a brief gasp, which settles into concern for the companion.
        Stage::Act if cue.sudden && gasp => (AttentionEmotion::Startled, Some(Gesture::Gasp)),
        Stage::Act if cue.sudden => (AttentionEmotion::Concerned, fret),
        Stage::Act | Stage::Catch
            if cue.risky && creature.personality.boldness < 0.3 && !reduced =>
        {
            // A timid watcher covers its eyes and turns its gaze away, still planted where it is.
            target.x = creature.state.position.x - (target.x - creature.state.position.x);
            (AttentionEmotion::Averting, Some(Gesture::Cover))
        }
        Stage::Act if cue.risky => (AttentionEmotion::Concerned, fret),
        Stage::Catch => (AttentionEmotion::Concerned, fret),
        Stage::Act => (AttentionEmotion::Curious, None),
        Stage::Recover(Outcome::Completed) if creature.personality.playfulness > 0.65 => {
            if !reduced && !walking {
                **action = ActionKind::Greet;
                (AttentionEmotion::Enjoying, Some(Gesture::Cheer))
            } else {
                (AttentionEmotion::Enjoying, None)
            }
        }
        Stage::Recover(_) => (AttentionEmotion::Relieved, None),
    };
    **emotion = Some(feeling);
    **gesture = pose;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(role: Role) -> Reaction {
        Reaction {
            origin: Origin::Cursor(1),
            role,
            target: Point { x: 0.0, y: 0.0 },
            emotion: AttentionEmotion::Curious,
            elapsed: 0.0,
            seconds: 4.0,
            delay: 0.0,
            travel_elapsed: 0.0,
            walk: None,
            display_walk: None,
            surface: (1, Some(701)),
            action: ActionKind::InspectScreen,
            cue: None,
            ride: None,
        }
    }

    fn bounds() -> DesktopRect {
        DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        }
    }

    fn every_role() -> Vec<(&'static str, Role)> {
        vec![
            (
                "actor",
                Role::Actor {
                    window: 701,
                    vanished: false,
                    companion: None,
                },
            ),
            (
                "cursor",
                Role::Cursor {
                    investigate: true,
                    anchor: Point { x: 0.0, y: 0.0 },
                    racing: false,
                },
            ),
            ("display", Role::Display { discovery: false }),
            (
                "ledge",
                Role::Ledge {
                    window: 701,
                    bounds: bounds(),
                    drop: 200.0,
                    resting: false,
                    declined: false,
                    commute: false,
                },
            ),
            (
                "journey",
                Role::Journey {
                    target_window: Some(702),
                    target_bounds: Some(bounds()),
                    stage: Stage::Act,
                    hanging: 0.0,
                    rewarding: true,
                    escape: false,
                    since: 0.0,
                    caught: false,
                },
            ),
            (
                "hesitate",
                Role::Hesitate {
                    source: 701,
                    source_bounds: bounds(),
                    target_window: 702,
                    target_bounds: bounds(),
                    edge: Point { x: 0.0, y: 0.0 },
                    runup: Point { x: 0.0, y: 0.0 },
                    landing: Point { x: 0.0, y: 0.0 },
                    drop: 200.0,
                    attempt: 1,
                    phase: HesitatePhase::Look,
                    phase_elapsed: 0.0,
                    retry: false,
                    commit: true,
                },
            ),
            (
                "helper",
                Role::Helper {
                    actor: 7,
                    window: 701,
                    bounds: bounds(),
                },
            ),
            ("tumble", Role::Tumble { stage: Stage::Act }),
            (
                "play",
                Role::Play {
                    stage: Stage::Act,
                    gesture: ActionKind::Greet,
                    bounds: Some(bounds()),
                    hopping: false,
                    pose: None,
                },
            ),
            (
                "dare",
                Role::Dare {
                    source: 701,
                    source_bounds: bounds(),
                    target_window: 702,
                    target_bounds: bounds(),
                    landing: Point { x: 0.0, y: 0.0 },
                },
            ),
        ]
    }

    /// Every role a creature can take publishes the same stages, so a watcher reads a jump,
    /// a rescue, a game, and a look at the desktop through one vocabulary rather than ten. Roles
    /// whose own machinery owns the stage report exactly that stage; the rest derive it from how
    /// far through the plan they are, and all of them move.
    #[test]
    fn every_role_that_acts_publishes_the_shared_stages_and_only_observers_stay_silent() {
        let staged = [
            Stage::Notice,
            Stage::Prepare,
            Stage::Act,
            Stage::Catch,
            Stage::Recover(Outcome::Completed),
            Stage::Recover(Outcome::Declined),
            Stage::Recover(Outcome::Slipped),
            Stage::Recover(Outcome::Settled),
        ];
        for (name, role) in every_role() {
            let mut reaction = plan(role);
            // A role that carries its own stage must report that stage, unchanged.
            let owns_stage = matches!(
                reaction.role,
                Role::Journey { .. } | Role::Tumble { .. } | Role::Play { .. }
            );
            if owns_stage {
                for expected in staged {
                    match &mut reaction.role {
                        Role::Journey { stage, .. }
                        | Role::Tumble { stage }
                        | Role::Play { stage, .. } => *stage = expected,
                        _ => unreachable!(),
                    }
                    let published = cue(&reaction).unwrap_or_else(|| panic!("{name}"));
                    assert_eq!(published.stage, expected, "{name} rewrote its own stage");
                }
                continue;
            }
            // A helper is given its approach, the way `try_assistance` gives it one, and arrives
            // partway through: walk over, hold on, let go.
            if matches!(reaction.role, Role::Helper { .. }) {
                reaction.walk = Some(ShortWalk::watching(Point { x: 0.0, y: 0.0 }, bounds()));
            }
            let mut seen = Vec::new();
            for step in 0..=40 {
                reaction.elapsed = step as f32 * reaction.seconds / 40.0;
                if step >= 10 {
                    reaction.walk = None;
                }
                if let Role::Hesitate {
                    phase,
                    phase_elapsed,
                    ..
                } = &mut reaction.role
                {
                    *phase_elapsed = reaction.elapsed;
                    *phase = match step {
                        0..=8 => HesitatePhase::Look,
                        9..=16 => HesitatePhase::BackUp,
                        17..=24 => HesitatePhase::Approach,
                        25..=32 => HesitatePhase::Reconsider,
                        _ => HesitatePhase::Retreat,
                    };
                }
                let Some(cue) = cue(&reaction) else {
                    panic!("{name} publishes nothing");
                };
                assert!(cue.since >= 0.0, "{name} reports negative stage time");
                if seen.last() != Some(&cue.stage) {
                    seen.push(cue.stage);
                }
            }
            assert!(seen.len() >= 2, "{name} never leaves one stage: {seen:?}");
            assert!(
                matches!(
                    seen.first(),
                    Some(Stage::Notice | Stage::Prepare | Stage::Act)
                ),
                "{name} starts at {:?}",
                seen.first()
            );
            // A dare owns only the walk to the edge: the answer itself is published by whichever
            // role takes over, so Prepare is the whole of its own vocabulary's end.
            let handover = matches!(reaction.role, Role::Dare { .. });
            assert!(
                matches!(
                    seen.last(),
                    Some(Stage::Recover(_) | Stage::Act | Stage::Catch)
                ) || (handover && seen.last() == Some(&Stage::Prepare)),
                "{name} ends at {:?}",
                seen.last()
            );
        }
        // Observers consume the vocabulary; they never publish another origin into it.
        assert!(cue(&plan(Role::Observer { actor: 7 })).is_none());
    }

    /// A watcher's response is its own: the same cue reads differently to a timid creature and a
    /// playful one, and nobody reacts before they have had a moment to notice.
    #[test]
    fn one_cue_produces_each_watchers_own_response() {
        let mut creature = World::preview_adult(
            [5; 32],
            OffsetDateTime::UNIX_EPOCH,
            &DesktopSnapshot::default(),
        );
        let cue = Cue {
            stage: Stage::Act,
            risky: true,
            since: 1.0,
            sudden: false,
        };
        let mut responses = Vec::new();
        for (boldness, playfulness, watching_for) in
            [(0.1, 0.5, 1.0), (0.9, 0.5, 1.0), (0.9, 0.5, 0.1)]
        {
            creature.personality.boldness = boldness;
            creature.personality.playfulness = playfulness;
            let (mut action, mut emotion, mut target, mut gesture) = (
                ActionKind::InspectScreen,
                None,
                Point { x: 40.0, y: 0.0 },
                None,
            );
            present_observer(
                &creature,
                cue,
                &mut Watching {
                    reduced: false,
                    walking: false,
                    watching_for,
                    action: &mut action,
                    emotion: &mut emotion,
                    target: &mut target,
                    gesture: &mut gesture,
                },
            );
            responses.push((emotion.unwrap(), target.x));
        }
        // The timid one looks away, which also turns its gaze; the bold one watches.
        assert_eq!(responses[0].0, AttentionEmotion::Averting);
        assert_ne!(responses[0].1, responses[1].1);
        assert_eq!(responses[1].0, AttentionEmotion::Concerned);
        // A watcher that has only just looked up begins at the beginning, whatever the cue says.
        assert_eq!(responses[2].0, AttentionEmotion::Curious);
    }
}
