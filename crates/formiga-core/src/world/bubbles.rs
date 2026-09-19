use super::*;

/// How long a bubble is up from its first pixel to its last.
pub(super) const BUBBLE_SECS: f32 = 2.4;
/// Each of the two growing steps on the way in, and the two shrinking ones on the way out.
const BUBBLE_STEP_SECS: f32 = 0.12;
/// A bubble asked for again while it is still up is held open rather than popped afresh, so
/// petting a creature several times reads as one steady answer instead of a flicker.
const BUBBLE_HOLD_SECS: f32 = BUBBLE_SECS - 2.0 * BUBBLE_STEP_SECS - 1.2;
/// Enough for a visitor's hello and everyone's answer; an interaction only ever needs one.
pub(super) const MAX_BUBBLES: usize = 5;

/// How much of a bubble is drawn. It grows in and shrinks out in two small steps, which is what
/// keeps it from startling anyone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BubbleGrowth {
    Small,
    Medium,
    Full,
}

/// An icon over one creature's head, answering something the person at the desk just did.
/// Runtime-only: it is never saved, journaled, or counted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThoughtBubble {
    pub creature_id: CreatureId,
    pub icon: BubbleIcon,
    pub age: f32,
}

impl ThoughtBubble {
    pub fn growth(&self, reduce_motion: bool) -> BubbleGrowth {
        if reduce_motion {
            return BubbleGrowth::Full;
        }
        let remaining = BUBBLE_SECS - self.age;
        match self.age.min(remaining) {
            edge if edge < BUBBLE_STEP_SECS => BubbleGrowth::Small,
            edge if edge < 2.0 * BUBBLE_STEP_SECS => BubbleGrowth::Medium,
            _ => BubbleGrowth::Full,
        }
    }
}

/// The form other systems use while `World` is partly borrowed, such as inside the creature loop.
pub(super) fn show(bubbles: &mut Vec<ThoughtBubble>, creature_id: CreatureId, icon: BubbleIcon) {
    if let Some(existing) = bubbles
        .iter_mut()
        .find(|bubble| bubble.creature_id == creature_id)
    {
        if existing.icon == icon {
            existing.age = existing.age.min(BUBBLE_HOLD_SECS);
        } else {
            // A new answer replaces the old one without shrinking first: the bubble is already
            // there, so only its icon changes.
            existing.icon = icon;
            existing.age = 2.0 * BUBBLE_STEP_SECS;
        }
        return;
    }
    if bubbles.len() >= MAX_BUBBLES {
        bubbles.remove(0);
    }
    bubbles.push(ThoughtBubble {
        creature_id,
        icon,
        age: 0.0,
    });
}

impl World {
    /// Bubbles to draw this frame, oldest first.
    pub fn thought_bubbles(&self) -> &[ThoughtBubble] {
        &self.bubbles
    }

    pub(super) fn show_bubble(&mut self, creature_id: CreatureId, icon: BubbleIcon) {
        show(&mut self.bubbles, creature_id, icon);
    }

    pub(super) fn tick_bubbles(&mut self, dt: f32) {
        if self.bubbles.is_empty() {
            return;
        }
        if !self.save.settings.visible {
            self.bubbles.clear();
            return;
        }
        for bubble in &mut self.bubbles {
            bubble.age += dt;
        }
        let guest = self.save.visitors.on_stage().map(|creature| creature.id);
        let creatures = &self.save.creatures;
        self.bubbles.retain(|bubble| {
            bubble.age < BUBBLE_SECS
                && (creatures
                    .iter()
                    .any(|creature| creature.id == bubble.creature_id)
                    || guest == Some(bubble.creature_id))
        });
    }
}
