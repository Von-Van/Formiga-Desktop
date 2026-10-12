//! Echo Fox Mode: every companion drawn as the Echo Fox for as long as the owner keeps it on.
//!
//! The mode changes how a companion is drawn and nothing else. Its name, traits, memories and
//! look stay exactly as they were underneath, the flag that asks for the fox is never saved, and
//! a relaunch or switching the mode off brings everybody back as themselves. A companion who
//! arrives, is reshaped or is put back by an undo while the mode is on is a fox by the next tick.

use super::*;

impl World {
    /// Switches Echo Fox Mode on or off for the whole colony, at once.
    pub fn set_echo_fox(&mut self, on: bool) {
        self.echo_fox = on;
        self.keep_echo_fox();
    }

    pub fn echo_fox(&self) -> bool {
        self.echo_fox
    }

    /// Draws everybody here as the mode says, whoever has joined or changed since it was set.
    pub(super) fn keep_echo_fox(&mut self) {
        for creature in &mut self.save.creatures {
            creature.appearance.echo_fox = self.echo_fox;
        }
    }
}
