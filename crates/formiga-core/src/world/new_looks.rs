//! A companion given a new look in Formiga Farm.
//!
//! The look itself is changed by the Farm contract's own `apply_design`, which touches nothing but
//! how the companion looks. All the world keeps of it is a line in the journal, in Desktop's own
//! words: nothing Farm wrote is copied in.

use super::*;

impl World {
    /// `id` was given a new look in Formiga Farm: written in the journal once, however many looks
    /// are tried on it within a few hours. Returns whether it was written.
    pub fn note_new_look(&mut self, id: CreatureId, now: OffsetDateTime) -> bool {
        if !self.save.creatures.iter().any(|creature| creature.id == id) {
            return false;
        }
        let before = self.save.companion.journal.last().cloned();
        self.save
            .companion
            .remember(Some(id), crate::JournalMoment::NewLook, now);
        self.save.companion.journal.last() != before.as_ref()
    }
}
