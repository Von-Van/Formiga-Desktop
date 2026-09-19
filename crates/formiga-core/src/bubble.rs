/// What a creature's thought bubble shows. Runtime-only: never serialized.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BubbleIcon {
    /// Petted; affection.
    Heart,
    /// Accepted a snack.
    Snack,
    /// Accepted a toy.
    Toy,
    /// Heading home.
    Home,
    /// Too tired; declines because it is sleepy.
    Sleepy,
    /// Picked up or startled.
    Surprise,
    /// Unsure or curious.
    Question,
    /// Thinking it over.
    Ellipsis,
    /// A gentle "no thanks".
    Decline,
    /// Happy; playing.
    Music,
    /// After a toss.
    Dizzy,
    /// A visitor's greeting.
    Hello,
    /// Delighted.
    Sparkle,
    /// A visitor agreed to stay.
    Stay,
}

impl BubbleIcon {
    pub const ALL: [Self; 14] = [
        Self::Heart,
        Self::Snack,
        Self::Toy,
        Self::Home,
        Self::Sleepy,
        Self::Surprise,
        Self::Question,
        Self::Ellipsis,
        Self::Decline,
        Self::Music,
        Self::Dizzy,
        Self::Hello,
        Self::Sparkle,
        Self::Stay,
    ];
}
