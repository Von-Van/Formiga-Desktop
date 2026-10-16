//! The colony's Battle record, as the colony keeps it and as Battle's contract reads it.
//!
//! The colony keeps the record in `formiga_core`, in exactly the shape the contract's
//! `BattleRecord` writes, so it crosses between the two through that shape and nothing else.
//! Whatever does not cross whole is not taken.

use formiga_battle_contract::{BattleRecord, BattleResult};
use formiga_core::World;
use time::OffsetDateTime;

/// What keeping a result kept: the matches applied for the first time, the keepsakes new to the
/// colony, and whether the Handler card or trios changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Kept {
    pub matches: usize,
    pub keepsakes: usize,
    pub registry: bool,
}

/// Keep what a session's matches earned: each match is applied to the colony's record by the
/// contract's own rule, once, by its id, and only a match applied for the first time adds its
/// lines to the journal and its keepsakes to the colony's. The Handler card and trios the session
/// ended with are kept first, by the contract's own rule, so a match played as a trio named in
/// the same session counts for it, and a lineup a trio played with and then changed is kept as
/// one of its earlier lineups. Everything is weighed before anything changes, so the colony
/// keeps all of it or none, and keeping the same result again changes nothing.
pub fn keep_result(
    world: &mut World,
    result: &BattleResult,
    now: OffsetDateTime,
) -> Result<Kept, String> {
    let before = to_contract(&world.save.battle.record)?;
    let mut applied = before.clone();
    if let Some(registry) = &result.registry {
        applied.register_session(registry, &result.matches, now);
    }
    let registry = applied != before;
    let new: Vec<_> = result
        .matches
        .iter()
        .filter(|played| applied.apply(played))
        .collect();
    if new.is_empty() && !registry {
        return Ok(Kept::default());
    }
    world.save.battle.record = from_contract(&applied)?;
    let mut kept = Kept {
        matches: new.len(),
        keepsakes: 0,
        registry,
    };
    for played in new {
        kept.keepsakes +=
            world.keep_battle_match(&played.rewards.journal, &played.rewards.keepsakes, now);
    }
    Ok(kept)
}

/// The record as the contract reads it, to send to Battle and to apply a result to.
pub fn to_contract(record: &formiga_core::BattleRecord) -> Result<BattleRecord, String> {
    serde_json::to_value(record)
        .and_then(serde_json::from_value)
        .map_err(|error| format!("the colony's Battle record does not read as Battle's: {error}"))
}

/// The record as the colony keeps it, once a result has been applied to it.
pub fn from_contract(record: &BattleRecord) -> Result<formiga_core::BattleRecord, String> {
    let kept: formiga_core::BattleRecord = serde_json::to_value(record)
        .and_then(serde_json::from_value)
        .map_err(|error| format!("Battle's record does not read as the colony's: {error}"))?;
    if kept.holds() {
        Ok(kept)
    } else {
        Err("Battle's record is outside what the colony keeps".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_battle_contract::{BattleSnapshot, MatchResult, Mode, Rewards, sample};
    use formiga_core::{BattleKeepsake, JournalMoment};
    use time::macros::datetime;

    fn colony() -> World {
        let now = datetime!(2026-10-15 12:00 UTC);
        World::new([7; 32], now, &formiga_core::DesktopSnapshot::default())
    }

    fn cup_won() -> MatchResult {
        MatchResult {
            id: "abcdefabcdefabcdefabcdefabcdef01".to_owned(),
            mode: Mode::Ai,
            rewards: Rewards {
                cup_win: Some("cup.backyard".to_owned()),
                unlocks: vec!["cup.woodland".to_owned()],
                journal: vec!["Mochi, Biscuit and Fig won the Backyard Cup.".to_owned()],
                keepsakes: vec![
                    "battle.trophy.backyard_cup".to_owned(),
                    "battle.medal.backyard_cup".to_owned(),
                ],
                ..Rewards::default()
            },
        }
    }

    #[test]
    fn every_cups_trophy_and_medal_is_kept() {
        let mut world = colony();
        let mut result = sample::result();
        result.matches.clear();
        for (index, cup) in ["backyard", "woodland", "millpond", "cove"]
            .iter()
            .enumerate()
        {
            let mut won = cup_won();
            won.id = format!("abcdefabcdefabcdefabcdefabcdef1{index}");
            won.rewards.cup_win = Some(format!("cup.{cup}"));
            won.rewards.unlocks.clear();
            won.rewards.keepsakes = vec![
                format!("battle.trophy.{cup}_cup"),
                format!("battle.medal.{cup}_cup"),
            ];
            result.matches.push(won);
        }
        let now = datetime!(2026-10-15 13:00 UTC);
        let kept = keep_result(&mut world, &result, now).unwrap();
        assert_eq!(kept.keepsakes, BattleKeepsake::ALL.len());
        let battle = &world.save.battle;
        assert!(BattleKeepsake::ALL.iter().all(|k| battle.has_keepsake(*k)));
        assert!(battle.holds());
    }

    #[test]
    fn a_result_is_kept_once_however_often_it_is_read() {
        let mut world = colony();
        let mut result = sample::result();
        result.matches.push(cup_won());
        let now = datetime!(2026-10-15 13:00 UTC);
        let kept = keep_result(&mut world, &result, now).unwrap();
        assert_eq!(
            kept,
            Kept {
                matches: 3,
                keepsakes: 2,
                registry: true,
            }
        );
        let battle = world.save.battle.clone();
        assert_eq!(battle.record.circuit_points, 15);
        assert_eq!(battle.record.applied.len(), 3);
        assert_eq!(battle.record.history.len(), 1);
        assert!(battle.record.unlocked.contains(&"cup.woodland".to_owned()));
        assert!(battle.has_keepsake(BattleKeepsake::BackyardCupTrophy));
        assert!(battle.has_keepsake(BattleKeepsake::BackyardCupMedal));
        let lines = |world: &World| {
            world
                .save
                .companion
                .journal
                .iter()
                .filter(|entry| matches!(entry.moment, JournalMoment::Battle(_)))
                .count()
        };
        assert_eq!(lines(&world), 2);
        let journal = world.save.companion.journal.clone();

        let later = now + time::Duration::days(2);
        assert_eq!(
            keep_result(&mut world, &result, later).unwrap(),
            Kept::default()
        );
        assert_eq!(
            world.save.battle, battle,
            "applying it twice changes nothing"
        );
        assert_eq!(world.save.companion.journal, journal);
    }

    #[test]
    fn a_keepsake_desktop_does_not_know_is_left_out() {
        let mut world = colony();
        let mut played = cup_won();
        played.rewards.keepsakes = vec!["battle.trophy.someday_cup".to_owned()];
        let mut result = sample::result();
        result.matches = vec![played];
        let kept = keep_result(&mut world, &result, datetime!(2026-10-15 13:00 UTC)).unwrap();
        assert_eq!(
            kept,
            Kept {
                matches: 1,
                keepsakes: 0,
                registry: true,
            }
        );
        assert!(world.save.battle.keepsakes.is_empty());
    }

    #[test]
    fn a_session_that_only_named_a_trio_keeps_it_and_its_matches_count_for_it() {
        let mut world = colony();
        let mut result = sample::result();
        let matches = std::mem::take(&mut result.matches);
        let now = datetime!(2026-10-16 13:00 UTC);
        let kept = keep_result(&mut world, &result, now).unwrap();
        assert_eq!(
            kept,
            Kept {
                registry: true,
                ..Kept::default()
            }
        );
        let record = &world.save.battle.record;
        assert_eq!(record.handler.as_ref().map(|h| h.name.as_str()), Some("Jo"));
        assert_eq!(record.trios.len(), 2);
        assert!(record.trios.iter().all(|t| t.formed_at_utc == now));
        assert_eq!(
            keep_result(&mut world, &result, now).unwrap(),
            Kept::default(),
            "the same names again change nothing"
        );

        result.matches = matches;
        let kept = keep_result(&mut world, &result, now).unwrap();
        assert_eq!(kept.matches, 2);
        let thistle = &world.save.battle.record.trios[0];
        assert_eq!((thistle.friends.played, thistle.visitors.won), (1, 1));
        assert!(world.save.battle.holds());
    }

    #[test]
    fn a_lineup_changed_after_it_played_stays_with_the_colony_as_an_earlier_one() {
        let mut world = colony();
        let mut result = sample::result();
        let registry = result.registry.as_mut().unwrap();
        let played = registry.trios[0].members.clone();
        // The Thistle Three played both matches, then one of them gave their place to another.
        let now_three = formiga_battle_contract::lineup_of(&[played[0], played[1], 999]);
        registry.trios[0].members = now_three.clone();
        let now = datetime!(2026-10-15 13:00 UTC);
        let kept = keep_result(&mut world, &result, now).unwrap();
        assert_eq!(kept.matches, 2);
        let thistle = &world.save.battle.record.trios[0];
        assert_eq!(thistle.members, now_three);
        assert_eq!(thistle.lineups.len(), 1);
        assert_eq!(thistle.lineups[0].members, played);
        assert_eq!(thistle.lineups[0].until_utc, now);
        assert!(world.save.battle.holds());
        let battle = world.save.battle.clone();
        keep_result(&mut world, &result, now + time::Duration::days(1)).unwrap();
        assert_eq!(
            world.save.battle, battle,
            "keeping it again changes nothing"
        );
    }

    #[test]
    fn the_record_crosses_both_ways_unchanged() {
        let record = sample::record();
        let kept = from_contract(&record).unwrap();
        assert_eq!(
            serde_json::to_value(&kept).unwrap(),
            serde_json::to_value(&record).unwrap()
        );
        assert_eq!(to_contract(&kept).unwrap(), record);
        assert!(to_contract(&formiga_core::BattleRecord::default()).is_ok());
    }

    #[test]
    fn every_record_battle_has_written_is_one_the_colony_keeps() {
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../formiga-battle-contract/tests/fixtures");
        let bytes = std::fs::read(fixtures.join("snapshot-v2.json")).unwrap();
        let record = BattleSnapshot::decode(&bytes).unwrap().record.unwrap();
        assert_eq!(
            to_contract(&from_contract(&record).unwrap()).unwrap(),
            record
        );
    }

    #[test]
    fn a_line_the_contract_lets_cross_is_one_the_colony_keeps() {
        for line in [
            "Mochi, Biscuit and Fig won a Backyard Cup match.",
            "Zoë and ミケ held the gate",
            "A long line that goes on and on with words, written well within the bounds of the rule",
        ] {
            assert_eq!(
                formiga_core::is_battle_line(line),
                formiga_expansion_rulebook::is_sanitized(line, 120),
                "{line:?}"
            );
        }
        for line in [
            " x",
            "x ",
            "a  b",
            "a\tb",
            "a\u{202E}b",
            "a\u{2066}b",
            "",
            &"y".repeat(121),
        ] {
            assert_eq!(
                formiga_core::is_battle_line(line),
                formiga_expansion_rulebook::is_sanitized(line, 120),
                "{line:?}"
            );
        }
    }
}
