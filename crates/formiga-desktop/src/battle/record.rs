//! The colony's Battle record, as the colony keeps it and as Battle's contract reads it.
//!
//! The colony keeps the record in `formiga_core`, in exactly the shape the contract's
//! `BattleRecord` writes, so it crosses between the two through that shape and nothing else.
//! Whatever does not cross whole is not taken.

use formiga_battle_contract::BattleRecord;

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
    use formiga_battle_contract::{BattleSnapshot, sample};

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
