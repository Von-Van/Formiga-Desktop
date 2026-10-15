//! What a match in Formiga Battle leaves in the colony: its lines in the journal, and the
//! keepsakes it earned. Battle's record itself is kept as Battle's contract applies it; this is
//! only what is the colony's own.

use super::*;

impl World {
    /// Keep what one newly played match earned: each of Battle's lines for it in the journal, and
    /// each keepsake Desktop knows, once. A line that is not one line of plain text, and a keepsake
    /// Desktop does not know, are left out, and the same line again within a few hours is written
    /// once, as any moment is. Returns how many keepsakes were new.
    pub fn keep_battle_match(
        &mut self,
        journal: &[String],
        keepsakes: &[String],
        now: OffsetDateTime,
    ) -> usize {
        for line in journal.iter().filter(|line| crate::is_battle_line(line)) {
            self.save
                .companion
                .remember(None, crate::JournalMoment::Battle(line.clone()), now);
        }
        let mut new = 0;
        for keepsake in keepsakes
            .iter()
            .filter_map(|id| crate::BattleKeepsake::from_id(id))
        {
            if !self.save.battle.has_keepsake(keepsake) {
                self.save
                    .battle
                    .keepsakes
                    .push(crate::BattleKeepsakeRecord {
                        keepsake,
                        earned_at_utc: now,
                    });
                new += 1;
            }
        }
        new
    }
}
