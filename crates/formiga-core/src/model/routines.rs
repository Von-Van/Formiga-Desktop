//! The routines a companion falls into: where and when it tends to do what.

use super::{ActionKind, SurfaceKind};
use serde::{Deserialize, Serialize};

pub const MAX_ROUTINES: usize = 12;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineSlot {
    pub key: u16,
    pub strength: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineTable {
    pub slots: [RoutineSlot; MAX_ROUTINES],
    pub len: u8,
}

impl Default for RoutineTable {
    fn default() -> Self {
        Self {
            slots: [RoutineSlot::default(); MAX_ROUTINES],
            len: 0,
        }
    }
}

impl RoutineTable {
    pub fn strength(&self, key: u16) -> f32 {
        self.slots[..usize::from(self.len.min(MAX_ROUTINES as u8))]
            .iter()
            .find(|slot| slot.key == key)
            .map_or(0.0, |slot| f32::from(slot.strength) / 255.0)
    }

    pub fn reinforce(&mut self, key: u16) {
        let len = usize::from(self.len.min(MAX_ROUTINES as u8));
        for slot in &mut self.slots[..len] {
            slot.strength = slot.strength.saturating_sub(1);
        }
        if let Some(slot) = self.slots[..len].iter_mut().find(|slot| slot.key == key) {
            slot.strength = slot.strength.saturating_add(5);
            return;
        }
        if len < MAX_ROUTINES {
            self.slots[len] = RoutineSlot { key, strength: 5 };
            self.len += 1;
            return;
        }
        let weakest = self.slots[..len]
            .iter()
            .enumerate()
            .min_by_key(|(_, slot)| (slot.strength, slot.key))
            .map(|(index, _)| index)
            .unwrap_or_default();
        if self.slots[weakest].strength <= 5 {
            self.slots[weakest] = RoutineSlot { key, strength: 5 };
        }
    }

    pub fn from_ranked(mut entries: Vec<(u16, f32)>) -> Self {
        entries.sort_by(|(key_a, value_a), (key_b, value_b)| {
            value_b.total_cmp(value_a).then_with(|| key_a.cmp(key_b))
        });
        let mut table = Self::default();
        for (index, (key, strength)) in entries.into_iter().take(MAX_ROUTINES).enumerate() {
            table.slots[index] = RoutineSlot {
                key,
                strength: (strength.clamp(0.0, 1.0) * 255.0).round() as u8,
            };
            table.len += 1;
        }
        table
    }
}

pub fn routine_key(surface: SurfaceKind, relative_x: f32, action: ActionKind, hour_utc: u8) -> u16 {
    let time_bucket = u16::from((hour_utc / 6).min(3));
    let region = u16::from((relative_x.clamp(0.0, 0.999) * 3.0) as u8);
    let surface = match surface {
        SurfaceKind::ScreenFloor => 0,
        SurfaceKind::WindowLedge => 1,
    };
    time_bucket | (region << 2) | (surface << 4) | (u16::from(action.routine_code()) << 5)
}
