use super::super::play::Kind;
use super::super::play::tests::{Audience, everyone_settled, scene, tick};
use super::*;

/// Creature 0 walks after creature 1, the ordinary encounter these games grow from.
fn following_scene(playful: bool) -> (World, DesktopSnapshot, OffsetDateTime) {
    let (mut w, d, now) = scene(false);
    let (follower, target) = (w.save.creatures[0].id, w.save.creatures[1].id);
    for c in w.save.creatures.iter_mut() {
        if !playful {
            c.personality.playfulness = 0.2;
        }
    }
    w.bond_plans.insert(
        follower,
        BondPlan {
            target,
            final_action: ActionKind::SocialPlay,
            experience: RelationshipExperience::PositivePlay,
            approaching: true,
        },
    );
    let c = &mut w.save.creatures[0];
    c.state.action = ActionKind::Follow;
    c.state.action_duration = 100.0;
    c.state.action_elapsed = 0.0;
    (w, d, now)
}

fn session_kind(w: &World) -> Option<Kind> {
    w.attention.play.session.map(|s| s.kind)
}

/// A solo flourish beside companions with an appetite for joining in.
pub(in super::super) fn dance_scene() -> (World, DesktopSnapshot, OffsetDateTime) {
    let (mut w, d, now) = scene(false);
    for (index, c) in w.save.creatures.iter_mut().enumerate() {
        c.state.action = if index == 0 {
            ActionKind::SoloPlay
        } else {
            ActionKind::Perch
        };
        c.state.action_duration = 100.0;
    }
    (w, d, now)
}

/// One creature playing with a toy, beside a companion that would like it.
pub(in super::super) fn keep_away_scene() -> (World, DesktopSnapshot, OffsetDateTime) {
    let (mut w, d, now) = scene(false);
    for (index, c) in w.save.creatures.iter_mut().enumerate() {
        c.state.action = if index == 0 {
            ActionKind::SocialPlay
        } else {
            ActionKind::Perch
        };
        c.state.action_duration = 100.0;
        c.personality.playfulness = 0.9;
        c.personality.curiosity = 0.2;
    }
    (w, d, now)
}

/// Two companions lined up along a ledge, one behind the other, with the appetite to vault.
pub(in super::super) fn leapfrog_scene() -> (World, DesktopSnapshot, OffsetDateTime) {
    let (mut w, d, now) = scene(false);
    for (index, c) in w.save.creatures.iter_mut().enumerate() {
        c.state.action = ActionKind::Perch;
        c.state.action_duration = 100.0;
        c.state.action_elapsed = 0.0;
        c.personality.playfulness = if index == 3 { 0.2 } else { 0.9 };
        c.personality.curiosity = 0.2;
        c.state.facing_right = true;
    }
    (w, d, now)
}

/// Two playful companions and one toy between them.
fn tug_scene() -> (World, DesktopSnapshot, OffsetDateTime) {
    let (mut w, d, now) = scene(false);
    for (index, c) in w.save.creatures.iter_mut().enumerate() {
        c.state.action = if index == 0 {
            ActionKind::SoloPlay
        } else {
            ActionKind::Perch
        };
        c.state.action_duration = 100.0;
        // Exactly one playful companion, so this is a tussle and not a dance.
        c.personality.playfulness = if index < 2 { 0.9 } else { 0.1 };
        c.personality.curiosity = 0.2;
        c.state.facing_right = index == 0;
    }
    (w, d, now)
}

fn window_bounds(d: &DesktopSnapshot) -> (f32, f32) {
    let w = d.windows.iter().find(|w| w.key == 701).unwrap();
    (w.bounds.x, w.bounds.right())
}

/// Every formation a game holds is placed with `held_gap`, so one test settles all of them:
/// standing that far apart, and settling anywhere within the slack of it, leaves both faces
/// clear at every display scale. Contact a game only passes through keeps `CONTACT`, which is
/// deliberately closer, because it is over in a moment.
#[test]
fn a_held_formation_is_face_clear_at_every_display_scale() {
    let (mut w, mut d, _) = scene(false);
    let id = w.save.creatures[0].id;
    for monitor_scale in [1.0_f32, 2.0] {
        d.monitors[0].scale_factor = monitor_scale;
        for display_scale in [2_u8, 3, 4] {
            w.save.settings.display_scale = display_scale;
            let clear = super::super::spacing::frame_width(display_scale, monitor_scale)
                * super::super::spacing::FACE_CLEAR_RATIO;
            let (gap, slack) = (w.held_gap(id, &d), w.held_slack(id, &d));
            assert!(
                gap - slack >= clear,
                "at {display_scale}x on a {monitor_scale}x display a held formation settles \
                 {} apart, inside the {clear} a creature draws",
                gap - slack
            );
            assert!(
                gap - slack * 2.0 >= clear * 0.999,
                "two companions each settling {slack} short of adjacent slots close the gap"
            );
            assert!(
                CONTACT * w.play_unit(id, &d) < clear,
                "momentary contact is meant to be closer than a held formation"
            );
        }
    }
}

#[test]
fn lively_pursuit_becomes_a_chase_that_swaps_roles_and_ends_on_its_own() {
    let (mut w, mut d, now) = following_scene(true);
    let (left, right) = window_bounds(&d);
    // The last companion is far too unplayful to be drawn into a chase, so it watches.
    let watcher = w.save.creatures[3].id;
    let mut audience = Audience::default();
    let mut started = false;
    let mut ran = false;
    let mut swapped = false;
    let mut finished = false;
    let energy_before = w.save.creatures[0].state.drives.energy;
    for step in 1..300 {
        tick(&mut w, &mut d, now, step);
        audience.note(&w);
        if let Some(s) = w.attention.play.session {
            assert_eq!(s.kind, Kind::Chase);
            started = true;
            swapped |= s.swaps > 0;
            ran |= w
                .save
                .creatures
                .iter()
                .any(|c| c.state.action == ActionKind::Sprint);
            for c in w.save.creatures.iter().take(2) {
                assert!(c.state.position.x > left && c.state.position.x < right);
                assert_eq!(c.state.surface.window_key, Some(701));
            }
        } else if started {
            finished = true;
            assert!(everyone_settled(&w), "the audience goes with the chase");
            break;
        }
    }
    assert!(
        started && ran && swapped && finished,
        "{started} {ran} {swapped} {finished}"
    );
    // Somebody is watching the running, not sitting through it: the gaze stays on a runner
    // the length of the ledge, and the face changes when the running stops.
    assert!(audience.coherent(), "{audience:?}");
    let looked = audience.looked_at(watcher);
    assert!(looked.len() >= 6, "a watcher follows the chase: {looked:?}");
    let felt = audience.felt_by(watcher);
    assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
    assert!(felt.len() >= 2, "and feels the end of it: {felt:?}");
    assert!(w.save.creatures[0].state.drives.energy < energy_before);
    assert!(w.attention.plans.is_empty());
    assert!(w.bond_plans.is_empty(), "the follow plan became the game");
    assert!(!w.drain_events().any(|e| matches!(
        e,
        WorldEvent::BondInteraction { .. } | WorldEvent::ActionCompleted { .. }
    )));
}

#[test]
fn a_calm_follow_becomes_a_spaced_procession_that_releases_a_lost_follower() {
    let (mut w, mut d, now) = following_scene(false);
    let watcher = w.save.creatures[3].id;
    let mut audience = Audience::default();
    let mut started = false;
    let mut spaced = false;
    for step in 1..200 {
        tick(&mut w, &mut d, now, step);
        audience.note(&w);
        let Some(s) = w.attention.play.session else {
            if started {
                break;
            }
            continue;
        };
        assert_eq!(s.kind, Kind::Parade);
        started = true;
        let (lead, follower) = (
            w.save.creatures[1].state.position,
            w.save.creatures[0].state.position,
        );
        if s.elapsed > 2.0 {
            let gap = (lead.x - follower.x).abs();
            spaced |= (20.0..140.0).contains(&gap);
        }
        // Separate the follower: the procession lets it go instead of dragging it along.
        // The ledge attachment has to move with it, or the next scan pulls it back.
        if s.elapsed > 3.0 {
            let far = if lead.x > 500.0 { 0.05 } else { 0.95 };
            let c = &mut w.save.creatures[0];
            c.state.surface.relative_x = far;
            c.state.position.x = 200.0 + 600.0 * far;
            tick(&mut w, &mut d, now, step + 1);
            assert!(w.attention.play.session.is_none());
            assert!(!w.attention.owns(w.save.creatures[0].id));
            // A procession that falls apart takes its audience with it: the watchers are
            // let go in the same moment, rather than left staring at an empty ledge.
            assert!(everyone_settled(&w), "{audience:?}");
            break;
        }
    }
    assert!(started && spaced);
    // Watching a procession is watching the front of it walk away. This one is cut short on
    // purpose, so the audience's whole part in it is the looking.
    assert!(audience.coherent(), "{audience:?}");
    let looked = audience.looked_at(watcher);
    assert!(looked.len() >= 4, "the gaze travels with it: {looked:?}");
    assert_eq!(
        audience.felt_by(watcher).first(),
        Some(&AttentionEmotion::Curious)
    );
}

#[test]
fn a_solo_flourish_gathers_a_dance_circle_with_staggered_responses() {
    let (mut w, mut d, now) = scene(false);
    for (i, c) in w.save.creatures.iter_mut().enumerate() {
        c.state.action = if i == 0 {
            ActionKind::SoloPlay
        } else {
            ActionKind::Perch
        };
        c.state.action_duration = 100.0;
    }
    // The second companion is playful enough to be pulled into the circle; the last is not.
    let (joiner, watcher) = (w.save.creatures[2].id, w.save.creatures[3].id);
    let mut audience = Audience::default();
    let mut members = BTreeSet::new();
    let mut gestured = BTreeSet::new();
    let mut approached = false;
    let mut started = false;
    for step in 1..260 {
        tick(&mut w, &mut d, now, step);
        audience.note(&w);
        let Some(s) = w.attention.play.session else {
            if started {
                assert!(everyone_settled(&w), "{audience:?}");
                break;
            }
            continue;
        };
        assert_eq!(s.kind, Kind::Dance);
        started = true;
        for (&id, p) in &w.attention.plans {
            if let Role::Play { gesture, .. } = p.role {
                members.insert(id);
                if gesture == ActionKind::Greet {
                    gestured.insert(id);
                }
                approached |= gesture == ActionKind::Traverse;
            }
        }
    }
    assert!(started && approached);
    assert!(members.len() >= 3, "the circle gathers companions");
    assert!(!gestured.is_empty());
    assert!(w.attention.plans.is_empty());
    // The circle grows out of its own audience, and openly: the companion it invites in
    // takes a dancer's part, while the one with no appetite for it watches the whole thing
    // through, feeling one way about the dancing and another about its ending.
    assert!(audience.coherent(), "{audience:?}");
    assert!(audience.joined.contains(&joiner), "{audience:?}");
    assert_eq!(audience.watchers(), vec![watcher], "{audience:?}");
    let felt = audience.felt_by(watcher);
    assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
    assert!(
        felt.len() >= 2,
        "the dance ends on the watcher too: {felt:?}"
    );
}

#[test]
fn resting_company_forms_a_pile_without_commandeering_the_anchor() {
    let (mut w, mut d, now) = scene(false);
    for (i, c) in w.save.creatures.iter_mut().enumerate() {
        c.state.action = ActionKind::Perch;
        c.state.action_duration = 100.0;
        c.state.action_elapsed = 8.0;
        c.state.drives.social_need = 0.9;
        c.personality.sociability = 0.9;
        // Calm company: nobody is staring, vaulting, or dancing.
        c.state.facing_right = true;
        c.personality.curiosity = if i == 0 { 0.8 } else { 0.2 };
        c.personality.playfulness = 0.2;
    }
    let anchor_start = w.save.creatures[1].state.position;
    let mut audience = Audience::default();
    let mut started = false;
    let mut settled_close = false;
    for step in 1..240 {
        tick(&mut w, &mut d, now, step);
        audience.note(&w);
        let Some(s) = w.attention.play.session else {
            if started {
                assert!(everyone_settled(&w), "{audience:?}");
                break;
            }
            continue;
        };
        assert_eq!(s.kind, Kind::Pile);
        started = true;
        // The anchor keeps resting exactly where it was, from the first full tick on.
        assert_eq!(w.save.creatures[1].state.position, anchor_start);
        if s.elapsed > 0.2 {
            assert_eq!(w.save.creatures[1].state.action, ActionKind::Perch);
        }
        // Shoulder to shoulder: near enough to be one pile, far enough that the anchor's
        // face is not behind whoever settled beside it.
        let gap = (w.save.creatures[0].state.position.x - anchor_start.x).abs();
        let clear = 48.0 * super::super::super::spacing::FACE_CLEAR_RATIO;
        settled_close |= s.elapsed > 2.0 && (clear..clear * 2.0).contains(&gap);
    }
    assert!(started && settled_close);
    assert!(w.attention.plans.is_empty());
    // Even settling down has an audience. The two companions who do not join keep their eyes
    // on the creature the pile is forming around — it never moves, so neither does the gaze —
    // and they are calm about the whole thing by the time it is over.
    assert!(audience.coherent(), "{audience:?}");
    let watchers = audience.watchers();
    assert_eq!(watchers.len(), 2, "{audience:?}");
    for id in watchers {
        let looked = audience.looked_at(id);
        assert!(
            looked
                .iter()
                .all(|x| (x - anchor_start.x as i32).abs() <= 12),
            "a watcher's gaze rests on the anchor: {looked:?}"
        );
        let felt = audience.felt_by(id);
        assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
        assert!(felt.len() >= 2, "and settles with the pile: {felt:?}");
    }
}

#[test]
fn playful_companions_take_turns_vaulting_over_one_another() {
    let (mut w, mut d, now) = scene(false);
    for (i, c) in w.save.creatures.iter_mut().enumerate() {
        c.state.action = ActionKind::Perch;
        c.state.action_duration = 100.0;
        c.state.action_elapsed = 0.0;
        // The last companion has no appetite for vaulting, so it never joins the rotation.
        c.personality.playfulness = if i == 3 { 0.2 } else { 0.9 };
        c.personality.curiosity = 0.2; // Not a staring contest.
        c.state.facing_right = true;
    }
    let (first, second) = (w.save.creatures[0].id, w.save.creatures[1].id);
    let watcher = w.save.creatures[3].id;
    let start = w.save.creatures[0].state.position.x;
    let mut audience = Audience::default();
    let mut started = false;
    let mut airborne = BTreeSet::new();
    let mut progressed = false;
    let mut vaulted = false;
    for step in 1..300 {
        tick(&mut w, &mut d, now, step);
        audience.note(&w);
        let Some(s) = w.attention.play.session else {
            if started {
                assert!(everyone_settled(&w), "{audience:?}");
                break;
            }
            continue;
        };
        assert_eq!(s.kind, Kind::Leapfrog);
        started = true;
        assert!(s.swaps <= 4, "turns are bounded");
        for id in [first, second] {
            if w.window_journeys.contains_key(&id) {
                airborne.insert(id);
                let c = w.save.creatures.iter().find(|c| c.id == id).unwrap();
                assert_eq!(c.state.action, ActionKind::Landing);
            }
        }
        progressed |= w.save.creatures[0].state.position.x > start + 40.0;
        for c in w.save.creatures.iter().take(2) {
            assert_eq!(c.state.surface.window_key, Some(701));
            // Only a leap in progress leaves the ledge, and only upward.
            if w.window_journeys.contains_key(&c.id) {
                assert!(c.state.position.y <= 600.0);
                vaulted |= c.state.position.y < 597.0;
            } else {
                assert_eq!(c.state.position.y, 600.0);
            }
        }
    }
    assert!(
        started && progressed && vaulted,
        "{started} {progressed} {vaulted}"
    );
    assert_eq!(airborne.len(), 2, "both take a turn");
    assert!(w.window_journeys.is_empty());
    assert!(w.attention.plans.is_empty());
    for c in w.save.creatures.iter().take(2) {
        assert!(matches!(
            c.state.action,
            ActionKind::Perch | ActionKind::Idle | ActionKind::Traverse
        ));
    }
    // The companion sitting this one out watches the vaulting travel down the ledge, and is
    // never quietly counted as one of the vaulters.
    assert!(audience.coherent(), "{audience:?}");
    assert!(audience.watchers().contains(&watcher), "{audience:?}");
    let looked = audience.looked_at(watcher);
    assert!(looked.len() >= 4, "the gaze follows the turns: {looked:?}");
    let felt = audience.felt_by(watcher);
    assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
    assert!(felt.len() >= 2, "and changes at the end: {felt:?}");
}

#[test]
fn a_toy_changes_hands_and_disappears_with_the_scene() {
    let (mut w, mut d, now) = scene(false);
    for (i, c) in w.save.creatures.iter_mut().enumerate() {
        c.state.action = if i == 0 {
            ActionKind::SocialPlay
        } else {
            ActionKind::Perch
        };
        c.state.action_duration = 100.0;
        c.personality.playfulness = 0.9;
        c.personality.curiosity = 0.2;
        // Everyone is carrying the memory of a keepsake that only turns up after dark. A toy
        // is a deliberate choice, so none of it may end up in anybody's hands here.
        c.state.activity_variant = 9;
    }
    let first = w.save.creatures[0].id;
    let mut audience = Audience::default();
    let mut holders = BTreeSet::new();
    let mut shown = BTreeSet::new();
    let mut chased = false;
    let mut started = false;
    for step in 1..300 {
        tick(&mut w, &mut d, now, step);
        audience.note(&w);
        let Some(s) = w.attention.play.session else {
            if started {
                assert!(everyone_settled(&w), "{audience:?}");
                break;
            }
            continue;
        };
        assert_eq!(s.kind, Kind::KeepAway);
        started = true;
        let prop = s.prop.expect("keep-away always has its toy");
        assert!(prop.handoffs <= 3, "hand-offs are bounded");
        holders.insert(prop.holder);
        for c in &w.save.creatures {
            if c.state.action == ActionKind::PresentDiscovery {
                // Only whoever is holding it shows the toy, and the toy is an everyday one
                // taken from this scene's own seed rather than whatever they last found.
                assert_eq!(c.id, prop.holder);
                assert_eq!(c.state.activity_variant, (s.seed % 8) as u8);
                shown.insert(c.id);
            }
            chased |= c.id != prop.holder && c.state.action == ActionKind::Sprint;
        }
    }
    assert!(started && chased);
    assert!(holders.len() >= 2, "the toy changes hands");
    assert!(shown.contains(&first));
    // The toy is runtime-only and goes with the scene.
    assert!(w.attention.play.session.is_none());
    assert!(
        w.save
            .creatures
            .iter()
            .all(|c| c.state.action != ActionKind::PresentDiscovery)
    );
    assert!(!w.drain_events().any(|e| matches!(
        e,
        WorldEvent::ActionCompleted {
            action: ActionKind::PresentDiscovery,
            ..
        }
    )));
    assert!(
        w.save
            .creatures
            .iter()
            .all(|c| c.memory.discoveries_found == 0),
        "a plaything is never a discovery"
    );
    assert!(
        w.save.companion.scrapbook.is_empty()
            && !w
                .save
                .companion
                .journal
                .iter()
                .any(|entry| entry.moment == JournalMoment::Discovery),
        "a game leaves nothing in the scrapbook or the journal"
    );
    // The tussle is watched. Both companions who are not in it track it up and down the
    // ledge, and both are playful enough to cheer it out at the end rather than just being
    // glad it is over.
    assert!(audience.coherent(), "{audience:?}");
    let watchers = audience.watchers();
    assert_eq!(watchers.len(), 2, "{audience:?}");
    for id in watchers {
        let looked = audience.looked_at(id);
        assert!(looked.len() >= 6, "a watcher tracks the toy: {looked:?}");
        let felt = audience.felt_by(id);
        assert!(felt.contains(&AttentionEmotion::Enjoying), "{felt:?}");
        assert!(audience.cheered.contains(&id), "{audience:?}");
        // The toy is what the scene is about, so the gaze goes with it: every time it
        // changes hands the audience is handed the new holder.
        assert!(
            audience.followed[&id].len() > 2,
            "the audience follows the toy: {audience:?}"
        );
    }
}

#[test]
fn a_tug_of_war_pulls_both_ways_and_ends_with_one_holder() {
    let (mut w, mut d, now) = scene(false);
    for (i, c) in w.save.creatures.iter_mut().enumerate() {
        c.state.action = if i == 0 {
            ActionKind::SoloPlay
        } else {
            ActionKind::Perch
        };
        c.state.action_duration = 100.0;
        // Exactly one playful companion, so this is a tussle and not a dance.
        c.personality.playfulness = if i < 2 { 0.9 } else { 0.1 };
        c.personality.curiosity = 0.2;
        c.state.facing_right = i == 0;
        // A keepsake found beside a close friend, still in mind. The toy they haul on is not
        // it, and holding a toy up is not finding one.
        c.state.activity_variant = 15;
    }
    let (first, second) = (w.save.creatures[0].id, w.save.creatures[1].id);
    let mut audience = Audience::default();
    let mut started = false;
    let mut closed_in = false;
    let mut pulled_both_ways = BTreeSet::new();
    let mut settled = false;
    for step in 1..300 {
        tick(&mut w, &mut d, now, step);
        audience.note(&w);
        let Some(s) = w.attention.play.session else {
            if started {
                assert!(everyone_settled(&w), "{audience:?}");
                break;
            }
            continue;
        };
        assert_eq!(s.kind, Kind::Tug);
        started = true;
        let (a, b) = (
            w.save.creatures[0].state.position.x,
            w.save.creatures[1].state.position.x,
        );
        closed_in |= (a - b).abs() < 60.0;
        for c in w.save.creatures.iter().take(2) {
            if c.state.velocity.x.abs() > 0.5 {
                pulled_both_ways.insert(c.state.velocity.x > 0.0);
            }
        }
        let showing: Vec<_> = w
            .save
            .creatures
            .iter()
            .filter(|c| c.state.action == ActionKind::PresentDiscovery)
            .collect();
        for c in &showing {
            // The toy is this scene's own, and always an everyday one.
            assert_eq!(c.state.activity_variant, (s.seed % 8) as u8);
        }
        settled |= s.elapsed >= s.ends_at - 2.2 && showing.len() == 1;
    }
    assert!(started && closed_in, "{started} {closed_in}");
    assert_eq!(pulled_both_ways.len(), 2, "the toy moves both ways");
    assert!(settled, "one of them ends up with it");
    assert!(w.attention.plans.is_empty());
    assert!(
        w.save.companion.scrapbook.is_empty()
            && w.save
                .creatures
                .iter()
                .all(|c| c.memory.discoveries_found == 0),
        "hauling on a toy is not finding one"
    );
    let _ = (first, second);
    // The two companions with no appetite for a tussle still watch one: their eyes stay on a
    // puller, and when it is decided they are relieved rather than delighted, which is not
    // how the pair who wanted the toy take it.
    assert!(audience.coherent(), "{audience:?}");
    let watchers = audience.watchers();
    assert_eq!(watchers.len(), 2, "{audience:?}");
    for id in watchers {
        let felt = audience.felt_by(id);
        assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
        assert!(felt.contains(&AttentionEmotion::Relieved), "{felt:?}");
        assert!(!felt.contains(&AttentionEmotion::Enjoying), "{felt:?}");
        assert!(!audience.cheered.contains(&id), "{audience:?}");
    }
}

#[test]
fn tag_passes_the_role_on_contact_and_cannot_bounce_straight_back() {
    let (mut w, mut d, now) = scene(false);
    for (i, c) in w.save.creatures.iter_mut().enumerate() {
        c.state.action = if i == 0 {
            ActionKind::Sprint
        } else {
            ActionKind::Perch
        };
        c.state.action_duration = 100.0;
        // Two watchers of different temper: one who would love a game, one who would not.
        c.personality.playfulness = if i == 3 { 0.2 } else { 0.9 };
        c.personality.curiosity = 0.2;
    }
    // Whoever ends up being chased is decided by where the sprinter has got to, so the lively
    // watcher is whichever of the playful companions is left on the sidelines.
    let quiet = w.save.creatures[3].id;
    let mut audience = Audience::default();
    let mut started = false;
    let mut its = Vec::new();
    let mut swap_times = Vec::new();
    for step in 1..300 {
        tick(&mut w, &mut d, now, step);
        audience.note(&w);
        let Some(s) = w.attention.play.session else {
            if started {
                assert!(everyone_settled(&w), "{audience:?}");
                break;
            }
            continue;
        };
        assert_eq!(s.kind, Kind::Tag);
        started = true;
        assert!(s.swaps <= 3, "the role passes a bounded number of times");
        let it = s
            .with_role(PlayRole::Lead)
            .next()
            .map(|(_, id)| id)
            .expect("somebody is it");
        if its.last() != Some(&it) {
            its.push(it);
            swap_times.push(s.elapsed);
        }
    }
    assert!(started);
    assert!(its.len() >= 2, "the role is passed on: {its:?}");
    for pair in swap_times.windows(2) {
        assert!(
            pair[1] - pair[0] >= TAG_IMMUNITY,
            "no instant tag back: {swap_times:?}"
        );
    }
    assert!(w.attention.plans.is_empty());
    // Nobody on the sidelines is quietly made "it". Both watch the running, and the same
    // ending reads differently to each of them: the lively one cheers the last tag out loud,
    // the quiet one is only glad it is over.
    assert!(audience.coherent(), "{audience:?}");
    let watchers = audience.watchers();
    assert_eq!(watchers.len(), 2, "{audience:?}");
    let lively = watchers
        .into_iter()
        .find(|id| *id != quiet)
        .expect("one of the two watchers has an appetite for games");
    let looked = audience.looked_at(lively);
    assert!(looked.len() >= 6, "the gaze follows a runner: {looked:?}");
    assert!(
        audience
            .felt_by(lively)
            .contains(&AttentionEmotion::Enjoying)
            && audience.cheered.contains(&lively),
        "{audience:?}"
    );
    let felt = audience.felt_by(quiet);
    assert!(felt.contains(&AttentionEmotion::Relieved), "{felt:?}");
    assert!(!felt.contains(&AttentionEmotion::Enjoying), "{felt:?}");
    assert!(!audience.cheered.contains(&quiet), "{audience:?}");
}

#[test]
fn a_jump_contest_gives_each_competitor_a_turn_with_the_others_watching() {
    let (mut world, mut desktop, now) = super::super::ledges::tests::edge_scene(true, true);
    world.clear_attention();
    world.window_journeys.clear();
    for (index, c) in world.save.creatures.iter_mut().enumerate() {
        c.state.action = ActionKind::Perch;
        c.state.action_duration = 100.0;
        c.personality.playfulness = 0.9;
        c.personality.curiosity = 0.5;
        c.state.drives.energy = 0.85;
        // Two competitors beside the same gap, the rest watching from further along.
        // Two competitors beside the gap, and two companions settled close enough behind
        // them to watch rather than wander off after each other.
        c.state.position.x = [770.0, 690.0, 620.0, 560.0][index];
        c.state.surface.relative_x = (c.state.position.x - 200.0) / 600.0;
        c.state.facing_right = true;
        if index >= 2 {
            c.personality.sociability = 0.2;
            c.state.drives.social_need = 0.0;
        }
    }
    world.attention.play.cooldown = 0.0;
    world.surface_memory.inspect_in = 100.0;
    let mut started = false;
    let mut performers = BTreeSet::new();
    let mut watched = false;
    for step in 3..500 {
        tick(&mut world, &mut desktop, now, step);
        let Some(s) = world.attention.play.session else {
            if started {
                break;
            }
            continue;
        };
        assert_eq!(s.kind, Kind::JumpContest);
        started = true;
        assert!(s.swaps <= 2, "turns are bounded");
        for id in s.members.iter().flatten() {
            if world.window_journeys.contains_key(id) {
                performers.insert(*id);
                // Whoever is not in the air is watching the one who is. That is the
                // competitor waiting its turn here: the two companions further along the
                // ledge drift into errands of their own within a second or two of the
                // contest starting, and a creature busy elsewhere is not recruited. Each
                // turn is an ordinary gap attempt, so it gathers whatever audience is free
                // — which is covered where that is what the scene is about, in
                // `bold_gap_has_anticipation_airtime_and_an_outcome_audience`.
                watched |= world.save.creatures.iter().any(|c| {
                    c.id != *id
                        && c.state.attention.is_some()
                        && c.state.action == ActionKind::InspectScreen
                });
            }
        }
    }
    assert!(started && watched, "{started} {watched}");
    assert_eq!(performers.len(), 2, "both competitors take a turn");
    // A turn still in the air when the contest ends is allowed to land before everything is
    // expected to be put away; it is the landing that clears it, not the scene ending.
    for step in 500..900 {
        tick(&mut world, &mut desktop, now, step);
        if world.window_journeys.is_empty() {
            break;
        }
    }
    assert!(world.attention.plans.is_empty());
    assert!(world.window_journeys.is_empty());
}

/// One creature asleep, the rest awake beside it.
fn sleeper_scene(playful: bool) -> (World, DesktopSnapshot, OffsetDateTime) {
    let (mut w, d, now) = scene(false);
    for (i, c) in w.save.creatures.iter_mut().enumerate() {
        c.personality.playfulness = if playful { 0.9 } else { 0.2 };
        c.personality.sociability = 0.9;
        c.personality.curiosity = 0.2;
        c.state.action_duration = 100.0;
        if i == 1 {
            c.state.action = ActionKind::Sleep;
            c.state.drives.sleep_pressure = if playful { 0.2 } else { 0.65 };
        } else {
            c.state.action = ActionKind::Perch;
        }
    }
    (w, d, now)
}

#[test]
fn company_creeps_around_a_sleeping_companion_without_disturbing_it() {
    let (mut w, mut d, now) = sleeper_scene(false);
    let sleeper = w.save.creatures[1].id;
    let asleep_at = w.save.creatures[1].state.position;
    let mut started = false;
    let mut crept = false;
    let mut froze = false;
    for step in 1..300 {
        tick(&mut w, &mut d, now, step);
        let Some(s) = w.attention.play.session else {
            if started {
                break;
            }
            continue;
        };
        assert_eq!(s.kind, Kind::Hush);
        started = true;
        assert_eq!(s.focus, Some(sleeper));
        assert!(
            !s.members.contains(&Some(sleeper)),
            "the sleeper is left alone"
        );
        // The sleeper keeps sleeping, exactly where it was.
        assert_eq!(w.save.creatures[1].state.action, ActionKind::Sleep);
        assert_eq!(w.save.creatures[1].state.position, asleep_at);
        crept |= w.save.creatures[0].state.action == ActionKind::Traverse;
        froze |= w.save.creatures[0].state.action == ActionKind::Perch
            && w.save.creatures[0].state.attention.is_some();
    }
    assert!(started && crept && froze, "{started} {crept} {froze}");
    assert!(!w.drain_events().any(|e| matches!(
        e,
        WorldEvent::SleepInterrupted { .. } | WorldEvent::CreatureWoke { .. }
    )));
}

#[test]
fn a_playful_creature_can_pester_a_rested_sleeper_awake_but_not_an_exhausted_one() {
    for playful in [false, true] {
        let (mut w, mut d, now) = sleeper_scene(playful);
        let sleeper = w.save.creatures[1].id;
        let mut audience = Audience::default();
        let mut woke = false;
        let mut played = false;
        let mut attempts = 0;
        for step in 1..320 {
            tick(&mut w, &mut d, now, step);
            audience.note(&w);
            if let Some(s) = w.attention.play.session {
                played = true;
                attempts = attempts.max(s.swaps);
                assert!(s.swaps <= 4, "attempts are capped");
                // The companion this is all about is asleep, so it is neither a player nor
                // part of the audience: nothing of it is posed while it sleeps.
                let resting = w.save.creatures.iter().find(|c| c.id == sleeper).unwrap();
                if resting.state.action == ActionKind::Sleep {
                    assert!(resting.state.attention.is_none());
                    assert!(!s.members.contains(&Some(sleeper)));
                }
            } else if played {
                played = false;
                assert!(everyone_settled(&w), "{audience:?}");
            }
            woke |= w
                .save
                .creatures
                .iter()
                .any(|c| c.id == sleeper && c.state.action != ActionKind::Sleep);
        }
        assert_eq!(woke, playful, "only a rested sleeper is roused: {playful}");
        // Creeping around a sleeper, or pestering one awake, is a performance with an
        // audience: the two companions who keep out of it watch the one making the attempt,
        // and feel differently about it once it is over.
        assert!(audience.coherent(), "{audience:?}");
        let watchers = audience.watchers();
        assert_eq!(watchers.len(), 2, "{audience:?}");
        for id in watchers {
            assert!(!audience.looked_at(id).is_empty());
            let felt = audience.felt_by(id);
            assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
            assert!(felt.len() >= 2, "the attempt ends on them too: {felt:?}");
        }
        if playful {
            assert!(attempts >= 1);
            let events: Vec<_> = w.drain_events().collect();
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, WorldEvent::SleepInterrupted { .. }))
            );
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, WorldEvent::CreatureWoke { .. }))
            );
        }
    }
}

#[test]
fn followers_walk_the_leaders_own_route_and_hesitate_where_they_cannot() {
    let (mut w, mut d, now) = following_scene(true);
    // Playful but not energetic: they copy the route instead of racing.
    for c in w.save.creatures.iter_mut() {
        c.state.drives.energy = 0.45;
    }
    let (follower, leader) = (w.save.creatures[0].id, w.save.creatures[1].id);
    let watcher = w.save.creatures[3].id;
    let mut audience = Audience::default();
    let mut started = false;
    let mut trailed = false;
    let mut followed = false;
    for step in 1..300 {
        tick(&mut w, &mut d, now, step);
        audience.note(&w);
        let Some(s) = w.attention.play.session else {
            if started {
                assert!(everyone_settled(&w), "{audience:?}");
                break;
            }
            continue;
        };
        assert_eq!(s.kind, Kind::FollowLeader);
        started = true;
        trailed |= s.trail.iter().flatten().count() >= 2;
        let (lead, follower) = (
            w.save.creatures[1].state.position.x,
            w.save.creatures[0].state.position.x,
        );
        // The follower walks the way the leader went, staying behind it.
        followed |= w.save.creatures[0].state.action == ActionKind::Traverse
            && (lead - follower).abs() < 200.0;
        // A route is only ever positions; nothing about it is written down.
        let saved = serde_json::to_string(&w.save).unwrap();
        assert!(!saved.contains("trail"));
    }
    assert!(
        started && trailed && followed,
        "{started} {trailed} {followed}"
    );
    // Watching this one means watching the front of it: after the opening tick the audience
    // is given the leader, and keeps its eyes on them the whole way along the ledge.
    assert!(audience.coherent(), "{audience:?}");
    assert_eq!(
        audience.followed[&watcher],
        vec![follower, leader],
        "{audience:?}"
    );
    let looked = audience.looked_at(watcher);
    assert!(looked.len() >= 4, "the gaze walks the route: {looked:?}");
    let felt = audience.felt_by(watcher);
    assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
    assert!(felt.len() >= 2, "and the route ends on them: {felt:?}");
}

#[test]
fn the_best_spot_on_a_ledge_changes_hands_by_stepping_aside() {
    let (mut w, mut d, now) = scene(false);
    // Window 701 spans 200..800, so its middle is 500.
    for (i, c) in w.save.creatures.iter_mut().enumerate() {
        c.state.action = ActionKind::Perch;
        c.state.action_duration = 100.0;
        c.personality.playfulness = 0.8;
        c.personality.boldness = 0.8;
        c.personality.curiosity = 0.2;
        c.state.position.x = [640.0, 500.0, 340.0, 280.0][i];
        c.state.surface.relative_x = (c.state.position.x - 200.0) / 600.0;
    }
    let holder = w.save.creatures[1].id;
    let mut audience = Audience::default();
    let mut started = false;
    let mut kings = Vec::new();
    let mut stepped_aside = false;
    for step in 1..320 {
        tick(&mut w, &mut d, now, step);
        audience.note(&w);
        let Some(s) = w.attention.play.session else {
            if started {
                assert!(everyone_settled(&w), "{audience:?}");
                break;
            }
            continue;
        };
        assert_eq!(s.kind, Kind::Hill);
        started = true;
        assert!(
            s.swaps <= 2,
            "the spot changes hands a bounded number of times"
        );
        let king = s.with_role(PlayRole::Lead).next().map(|(_, id)| id);
        if kings.last() != king.as_ref() {
            kings.extend(king);
        }
        // Yielding is walking aside, never shoving: nobody is pushed off the ledge.
        for c in w.save.creatures.iter().take(2) {
            assert_eq!(c.state.surface.window_key, Some(701));
            assert_eq!(c.state.position.y, 600.0);
        }
        stepped_aside |= kings.len() > 1
            && w.save
                .creatures
                .iter()
                .take(2)
                .any(|c| c.state.action == ActionKind::Traverse && Some(c.id) != king);
    }
    assert!(started, "a contest begins");
    assert_eq!(
        kings.first().copied(),
        Some(holder),
        "the occupant starts on it"
    );
    assert!(kings.len() >= 2, "the spot is taken: {kings:?}");
    assert!(stepped_aside, "the previous occupant walks aside");
    // The contest is played to the two companions further down the ledge. Their eyes stay on
    // the companion standing in the middle of it — the one with something to lose, who barely
    // moves while holding the spot — and they are pleased enough with the business of it
    // changing hands to say so at the end.
    assert!(audience.coherent(), "{audience:?}");
    let watchers = audience.watchers();
    assert_eq!(watchers.len(), 2, "{audience:?}");
    for id in watchers {
        let looked = audience.looked_at(id);
        // The spot is in the middle of the ledge, so the gaze stays about there — but it
        // goes with whoever is standing on it, including the moment they step aside.
        // A challenger comes up a companion's width from the spot, so the gaze reaches
        // that far out and no further.
        assert!(
            looked.iter().all(|x| (x - 500).abs() <= 90),
            "the gaze stays about the middle of the ledge: {looked:?}"
        );
        assert!(
            looked.iter().any(|x| (x - 500).abs() > 12),
            "and moves when the spot changes hands: {looked:?}"
        );
        let felt = audience.felt_by(id);
        assert_eq!(felt.first(), Some(&AttentionEmotion::Curious));
        assert!(felt.contains(&AttentionEmotion::Enjoying), "{felt:?}");
        assert!(audience.cheered.contains(&id), "{audience:?}");
    }
}

#[test]
fn reduced_motion_never_starts_a_travelling_game() {
    for reduced in [false, true] {
        let (mut w, mut d, now) = following_scene(true);
        w.save.settings.reduce_motion = reduced;
        let mut travelled = false;
        for step in 1..120 {
            tick(&mut w, &mut d, now, step);
            travelled |= session_kind(&w).is_some_and(Kind::travels);
        }
        assert_eq!(travelled, !reduced);
    }
}

/// A tug of war is a contest of shoulders. Once the pair has the toy between them — which is
/// where their settling-in walk leaves them, whether or not the last hand's width is a stride
/// either will take — whoever is digging in this turn hauls on it, while the one being pulled
/// about is travelling and keeps its own body, as does whoever is holding the toy up.
#[test]
fn a_tug_of_war_heaves_on_the_toy_it_is_holding_between_them() {
    let (mut w, mut d, now) = tug_scene();
    let mut poses = super::super::tests::Poses::default();
    let mut heaved_holding = 0;
    for step in 1..300 {
        poses.tick(&mut w, |w| tick(w, &mut d, now, step));
        let Some(s) = w.attention.play.session.filter(|s| s.kind == Kind::Tug) else {
            continue;
        };
        let holder = s.prop.expect("a tug always has its toy").holder;
        for c in w.save.creatures.iter().take(2) {
            if c.state.attention.and_then(|pose| pose.gesture) == Some(Gesture::Heave) {
                assert_eq!(c.state.action, ActionKind::SocialPlay);
                heaved_holding += usize::from(c.id != holder);
                assert_ne!(c.id, holder, "the toy itself keeps the holder's body");
            }
        }
    }
    assert!(
        heaved_holding >= 3,
        "the pair closed in but never hauled on the toy: {:?}",
        poses.struck
    );
}

/// Keep-away is about who does not have the toy: they chase it while there is room to run,
/// and when the tussle winds down they are left standing there reaching after it.
#[test]
fn keep_away_leaves_the_empty_handed_reaching_after_the_toy() {
    let (mut w, mut d, now) = keep_away_scene();
    let mut poses = super::super::tests::Poses::default();
    let mut reached = 0;
    for step in 1..300 {
        poses.tick(&mut w, |w| tick(w, &mut d, now, step));
        let Some(s) = w
            .attention
            .play
            .session
            .filter(|s| s.kind == Kind::KeepAway)
        else {
            continue;
        };
        let holder = s.prop.expect("keep-away always has its toy").holder;
        for c in &w.save.creatures {
            if c.state.attention.and_then(|pose| pose.gesture) == Some(Gesture::Reach) {
                assert_ne!(c.id, holder, "whoever has the toy is busy showing it off");
                assert!(s.members.contains(&Some(c.id)));
                reached += 1;
            }
        }
    }
    assert!(
        reached >= 5,
        "nobody reached for the toy: {:?}",
        poses.struck
    );
}

/// A dance circle keeps its own beat: each dancer bops on the beat it joined in on, and walks
/// into place without one. The beat carries on through the wind-down, where the circle stays
/// where it is standing rather than travelling.
#[test]
fn a_dance_circle_bops_on_its_own_beat() {
    let (mut w, mut d, now) = dance_scene();
    let mut poses = super::super::tests::Poses::default();
    let mut danced_on_the_beat = 0;
    for step in 1..260 {
        poses.tick(&mut w, |w| tick(w, &mut d, now, step));
        for c in &w.save.creatures {
            if c.state.attention.and_then(|pose| pose.gesture) == Some(Gesture::Bop) {
                assert!(
                    matches!(c.state.action, ActionKind::Greet | ActionKind::Perch),
                    "bopping over {:?}",
                    c.state.action
                );
                danced_on_the_beat += usize::from(c.state.action == ActionKind::Greet);
            }
        }
    }
    assert!(
        danced_on_the_beat >= 10,
        "the circle never danced: {poses:?}"
    );
    let dancers: Vec<_> = poses
        .struck
        .iter()
        .filter(|(_, struck)| struck.contains(&Gesture::Bop))
        .collect();
    assert!(dancers.len() >= 2, "the circle never danced: {poses:?}");
    for (id, struck) in dancers {
        assert!(
            struck.iter().all(|pose| *pose == Gesture::Bop),
            "a dancer did something else: {id} {struck:?}"
        );
        assert!(poses.ticks(*id, Gesture::Bop) >= 10, "{poses:?}");
    }
}
