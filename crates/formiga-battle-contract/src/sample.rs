//! The sample colony a trip to Hill rehearses with, as Battle's roster: one of everything the
//! format carries, made through Desktop's own `World`, the same every time. Its companions have
//! lived a little, so their leanings are not all where they rest.

use crate::BattleSnapshot;
use formiga_core::{LearnedTendencies, SaveFile};
use formiga_expansion_rulebook::SessionId;

pub fn colony() -> SaveFile {
    let mut save = formiga_travel::sample::colony(3);
    for (n, creature) in save.creatures.iter_mut().enumerate() {
        let t = &mut creature.tendencies;
        let k = n as f32;
        LearnedTendencies::adjust(&mut t.exploration, 18.0 * k - 30.0);
        LearnedTendencies::adjust(&mut t.sociability, 25.0 - 11.0 * k);
        LearnedTendencies::adjust(&mut t.play, 9.0 * k);
        LearnedTendencies::adjust(&mut t.routine, -12.0 * k + 20.0);
    }
    save
}

/// A neighbouring colony, made the same way from another seed: who an AI match is played
/// against, so the other team are real companions with looks and temperaments of their own.
pub fn visitors(seed: u8) -> BattleSnapshot {
    let seed = if seed == 3 { 4 } else { seed };
    crate::project_roster(
        &formiga_travel::sample::colony(seed),
        SessionId::parse(formiga_travel::sample::SESSION).expect("the sample session"),
        formiga_travel::sample::MADE,
        "0.67.6",
    )
    .expect("a sample colony projects")
}

pub fn snapshot() -> BattleSnapshot {
    crate::project_roster(
        &colony(),
        SessionId::parse(formiga_travel::sample::SESSION).expect("the sample session"),
        formiga_travel::sample::MADE,
        "0.67.6",
    )
    .expect("the sample colony projects")
}

/// A colony that has played a little: a few Open Circuit points, a cup begun, a history line and
/// a notebook page.
pub fn record() -> crate::BattleRecord {
    let save = colony();
    let ids: Vec<u64> = save.creatures.iter().map(|c| c.id).take(3).collect();
    let summary = crate::MatchSummary {
        finished_at_utc: formiga_travel::sample::MADE,
        mode: crate::Mode::Friend,
        won: Some(true),
        ended_by: "capture".to_owned(),
        turns: 8,
        team: ids.clone(),
        opponent: Some("Sam".to_owned()),
    };
    let mut showing = crate::Showing {
        steps: 14,
        ..crate::Showing::default()
    };
    showing.signs.insert("took_the_edge".to_owned(), 4);
    showing.friends.insert(ids[1], 3);
    crate::BattleRecord {
        circuit_points: 15,
        unlocked: Vec::new(),
        cups: vec![crate::CupWins {
            cup: "cup.backyard".to_owned(),
            wins: 1,
        }],
        history: vec![summary],
        notebook: vec![crate::NotebookPage {
            creature_id: ids[0],
            matches: 1,
            wins: 1,
            recent: vec![showing],
            moments: vec!["Held firm against two at once".to_owned()],
        }],
        applied: vec!["00000000000000000000000000000001".to_owned()],
    }
}

/// The sample roster with its record, as a colony that has played sends it.
pub fn snapshot_with_record() -> BattleSnapshot {
    snapshot().with_record(record())
}

/// A session's answer: one friend match and one match against the AI.
pub fn result() -> crate::BattleResult {
    let snapshot = snapshot_with_record();
    let bytes = snapshot.encode().expect("the sample encodes");
    let mut result = crate::BattleResult::new(
        snapshot.session_id.clone(),
        formiga_expansion_rulebook::sha256_hex(&bytes),
        "0.1.0",
    );
    let ids: Vec<u64> = snapshot
        .contenders
        .iter()
        .map(|c| c.traveler.id.0)
        .take(3)
        .collect();
    let friend = record().history[0].clone();
    let mut showing = crate::Showing {
        steps: 12,
        ..crate::Showing::default()
    };
    showing.signs.insert("hung_back".to_owned(), 2);
    result.matches.push(crate::MatchResult {
        id: "0123456789abcdef0123456789abcdef".to_owned(),
        mode: crate::Mode::Friend,
        rewards: crate::Rewards {
            circuit_points: 15,
            participation: ids.clone(),
            notebook: vec![crate::NotebookAddition {
                creature_id: ids[0],
                won: true,
                showing,
                moments: vec!["Was holding the beacon when it lit".to_owned()],
            }],
            history: Some(friend),
            ..crate::Rewards::default()
        },
    });
    result.matches.push(crate::MatchResult {
        id: "fedcba9876543210fedcba9876543210".to_owned(),
        mode: crate::Mode::Ai,
        rewards: crate::Rewards {
            cup_win: Some("cup.backyard".to_owned()),
            journal: vec!["Mochi, Biscuit and Fig won a Backyard Cup match.".to_owned()],
            ..crate::Rewards::default()
        },
    });
    result
}
