//! Talk: the bubbles companions say things to each other in, and the ones they answer the person
//! at the desk with, as pictures.
//!
//! A bubble is a style and a topic. The style is how it is said — chatting, excited, whispered,
//! thought, sung, tender, grumbled, shouted — and shows in the colour of the bubble's paper; the
//! topic is what it is about — a ball, a stick, the food, a game, the moon, a heart, a question —
//! and shows as one picture inside. Runtime only: nothing here is saved.

use crate::BubbleIcon;

/// How something is said, which is the shape and colour of its bubble.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TalkStyle {
    /// A round cream bubble: hello, and the ordinary back and forth.
    Chat,
    /// A yellow starburst: an invitation to play, a cheer, a big catch.
    Excited,
    /// Small, pale and dotted round: a secret, or talking beside a sleeper.
    Whisper,
    /// A cloud with a trail of dots: wanting, wondering, dreaming.
    Thought,
    /// A blue bubble with a wavy edge: humming, singing, dancing.
    Song,
    /// A pink bubble trimmed with hearts: sharing, comforting, goodnight.
    Tender,
    /// A dark, jagged little cloud: a squabble, a tease, being woken.
    Grumble,
    /// A bold bubble with lines flying off it: calling out, a warning, a dare.
    Shout,
}

impl TalkStyle {
    pub const ALL: [Self; 8] = [
        Self::Chat,
        Self::Excited,
        Self::Whisper,
        Self::Thought,
        Self::Song,
        Self::Tender,
        Self::Grumble,
        Self::Shout,
    ];
}

/// What something is about, as a small picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TalkTopic {
    Heart,
    Wave,
    Food,
    Ball,
    Stick,
    Umbrella,
    Kite,
    Fish,
    Note,
    Question,
    Exclaim,
    Sparkle,
    Moon,
    Sleep,
    Home,
    Storm,
    CrackedHeart,
    Spiral,
    Blush,
    Eye,
    Dots,
    No,
    Footprints,
    Flag,
    Sun,
    Flower,
    Blocks,
}

impl TalkTopic {
    pub const ALL: [Self; 27] = [
        Self::Heart,
        Self::Wave,
        Self::Food,
        Self::Ball,
        Self::Stick,
        Self::Umbrella,
        Self::Kite,
        Self::Fish,
        Self::Note,
        Self::Question,
        Self::Exclaim,
        Self::Sparkle,
        Self::Moon,
        Self::Sleep,
        Self::Home,
        Self::Storm,
        Self::CrackedHeart,
        Self::Spiral,
        Self::Blush,
        Self::Eye,
        Self::Dots,
        Self::No,
        Self::Footprints,
        Self::Flag,
        Self::Sun,
        Self::Flower,
        Self::Blocks,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    /// The picture a game of this shape is talked about with.
    pub fn of_shape(shape: crate::play::PlayShape) -> Self {
        use crate::play::PlayShape::*;
        match shape {
            Round => Self::Ball,
            Stick => Self::Stick,
            Umbrella => Self::Umbrella,
            Flying => Self::Kite,
            Spinning => Self::Spiral,
            Noisy => Self::Note,
            Soft => Self::Heart,
            Stacking => Self::Blocks,
            Floating => Self::Fish,
        }
    }
}

/// What is said: how, and about what.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Saying {
    pub style: TalkStyle,
    pub topic: TalkTopic,
}

impl Saying {
    pub const fn one(style: TalkStyle, topic: TalkTopic) -> Self {
        Self { style, topic }
    }

    /// How an answer to the person at the desk is said: the icon it has always shown, in the
    /// bubble that fits it.
    pub const fn answering(icon: BubbleIcon) -> Self {
        use TalkStyle::*;
        use TalkTopic as T;
        let (style, topic) = match icon {
            BubbleIcon::Heart => (Tender, T::Heart),
            BubbleIcon::Snack => (Chat, T::Food),
            BubbleIcon::Toy => (Excited, T::Ball),
            BubbleIcon::Home => (Chat, T::Home),
            BubbleIcon::Sleepy => (Thought, T::Sleep),
            BubbleIcon::Surprise => (Shout, T::Exclaim),
            BubbleIcon::Question => (Thought, T::Question),
            BubbleIcon::Ellipsis => (Thought, T::Dots),
            BubbleIcon::Decline => (Chat, T::No),
            BubbleIcon::Music => (Song, T::Note),
            BubbleIcon::Dizzy => (Thought, T::Spiral),
            BubbleIcon::Hello => (Chat, T::Wave),
            BubbleIcon::Sparkle => (Excited, T::Sparkle),
            BubbleIcon::Stay => (Tender, T::Home),
            BubbleIcon::Grumble => (Grumble, T::Storm),
            BubbleIcon::Jealous => (Grumble, T::CrackedHeart),
            BubbleIcon::Swoon => (Tender, T::Sparkle),
            BubbleIcon::Blush => (Whisper, T::Blush),
            BubbleIcon::Watching => (Thought, T::Eye),
        };
        Self::one(style, topic)
    }
}

/// How long a companion's bubble to another is up, from its first pixel to its last.
pub const TALK_SECS: f32 = 2.2;
/// Each of the two growing steps on the way in, and the two shrinking ones on the way out.
const TALK_STEP_SECS: f32 = 0.12;

/// Something one companion is saying to another, up over its head just now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TalkBubble {
    pub creature_id: crate::CreatureId,
    pub saying: Saying,
    /// Seconds since it appeared; below zero, it is still waiting to be said, as an answer does.
    pub age: f32,
    /// Whom an answer answers. It is said only if they are still there to hear it.
    pub(crate) answering: Option<crate::CreatureId>,
    /// Seconds an answer has waited past its time, for room beside a companion answering the
    /// person at the desk.
    pub(crate) held: f32,
}

impl TalkBubble {
    /// How much of it is drawn, as a thought bubble grows in and shrinks out. `None` while it
    /// waits.
    pub fn growth(&self, reduce_motion: bool) -> Option<crate::BubbleGrowth> {
        if self.age < 0.0 || self.age >= TALK_SECS {
            return None;
        }
        if reduce_motion {
            return Some(crate::BubbleGrowth::Full);
        }
        let edge = self.age.min(TALK_SECS - self.age);
        Some(if edge < TALK_STEP_SECS {
            crate::BubbleGrowth::Small
        } else if edge < 2.0 * TALK_STEP_SECS {
            crate::BubbleGrowth::Medium
        } else {
            crate::BubbleGrowth::Full
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_answer_has_a_saying_and_they_are_told_apart() {
        let mut seen = std::collections::BTreeSet::new();
        for icon in BubbleIcon::ALL {
            let saying = Saying::answering(icon);
            seen.insert(format!("{saying:?}"));
        }
        assert_eq!(seen.len(), BubbleIcon::ALL.len());
    }

    #[test]
    fn a_bubble_waits_grows_and_goes() {
        let mut bubble = TalkBubble {
            creature_id: 1,
            saying: Saying::one(TalkStyle::Chat, TalkTopic::Wave),
            age: -0.5,
            answering: None,
            held: 0.0,
        };
        assert_eq!(bubble.growth(false), None);
        bubble.age = 0.05;
        assert_eq!(bubble.growth(false), Some(crate::BubbleGrowth::Small));
        bubble.age = 1.0;
        assert_eq!(bubble.growth(false), Some(crate::BubbleGrowth::Full));
        bubble.age = TALK_SECS;
        assert_eq!(bubble.growth(false), None);
    }
}
