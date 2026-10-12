//! What the world tells the desktop happened, and what the desktop asks of the world.

use super::{
    ActionKind, ColonyObjectKind, CreatureId, DisplayKey, Point, ProfileDescriptor,
    RelationshipExperience, RitualKind, SurfaceKind, VillageItem, VillageMoment,
};

/// What the person at the desk is holding out. Runtime-only: an offer is a moment, not a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OfferKind {
    Snack,
    Toy,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorldEvent {
    CreatureSpawned {
        creature_id: CreatureId,
    },
    /// A wonder turned up for somebody to play on.
    WonderAppeared {
        kind: crate::WonderKind,
    },
    /// The colony's first wonder of a kind, and who it turned up for.
    WonderFound {
        creature_id: CreatureId,
        kind: crate::WonderKind,
    },
    ActionStarted {
        creature_id: CreatureId,
        action: ActionKind,
    },
    ActionCompleted {
        creature_id: CreatureId,
        action: ActionKind,
    },
    SurfaceChanged {
        creature_id: CreatureId,
        kind: SurfaceKind,
    },
    CursorReaction {
        creature_id: CreatureId,
        action: ActionKind,
    },
    WindowReaction {
        creature_id: CreatureId,
        action: ActionKind,
    },
    BondInteraction {
        a: CreatureId,
        b: CreatureId,
        experience: RelationshipExperience,
    },
    CreatureSlept {
        creature_id: CreatureId,
    },
    CreatureWoke {
        creature_id: CreatureId,
    },
    CreatureRested {
        creature_id: CreatureId,
        uninterrupted_seconds: u32,
    },
    SleepInterrupted {
        creature_id: CreatureId,
        elapsed_seconds: u32,
    },
    CreaturePetted {
        creature_id: CreatureId,
    },
    CreaturePlaced {
        creature_id: CreatureId,
        display: DisplayKey,
        region: u8,
    },
    ObservationElapsed {
        creature_id: CreatureId,
        display: DisplayKey,
        region: u8,
        on_ledge: bool,
        riding_window: bool,
        nearby_creature: Option<CreatureId>,
        active_seconds: u8,
    },
    ProfileChanged {
        creature_id: CreatureId,
        new_descriptor: Option<ProfileDescriptor>,
        show_milestone: bool,
    },
    DragStarted {
        creature_id: CreatureId,
    },
    DragEnded {
        creature_id: CreatureId,
        outcome: DragReleaseKind,
    },
    TossLanded {
        creature_id: CreatureId,
        surface: SurfaceKind,
        bounced: bool,
    },
    /// Something was held out to one creature and it gave its answer. Being asked kindly is a
    /// good thing to have happen whichever way the answer went; the meal or the game that may
    /// follow is counted by its own completed action, not by this.
    OfferAnswered {
        creature_id: CreatureId,
        kind: OfferKind,
        accepted: bool,
    },
    HomeAppeared,
    HomeDisappeared {
        interrupted: bool,
    },
    RitualStarted {
        kind: RitualKind,
    },
    RitualCompleted {
        kind: RitualKind,
    },
    RitualInterrupted {
        kind: RitualKind,
    },
    ColonyObjectAdded {
        object_id: u64,
        kind: ColonyObjectKind,
    },
    /// Something new for the village to choose from: a decoration, a hangout spot, a garden or
    /// an ornament.
    VillageUnlocked {
        item: VillageItem,
    },
    /// A companion picked up a little habit of its own.
    HabitLearned {
        creature_id: CreatureId,
        habit: crate::Habit,
    },
}

impl WorldEvent {
    /// How soon this event needs the colony written to disk. Arrivals, the houses coming and going,
    /// rituals, new belongings and a newly picked-up habit are kept at once; a creature starting an
    /// action or stepping onto another surface is everyday movement, gathered into the next
    /// routine checkpoint.
    pub fn save_urgency(&self) -> crate::SaveUrgency {
        match self {
            Self::CreatureSpawned { .. }
            | Self::HomeAppeared
            | Self::HomeDisappeared { .. }
            | Self::RitualStarted { .. }
            | Self::RitualInterrupted { .. }
            | Self::ColonyObjectAdded { .. }
            | Self::VillageUnlocked { .. }
            | Self::HabitLearned { .. }
            | Self::WonderFound { .. } => crate::SaveUrgency::Prompt,
            Self::ActionStarted { .. } | Self::SurfaceChanged { .. } => crate::SaveUrgency::Routine,
            _ => crate::SaveUrgency::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorldCommand {
    BeginInteraction {
        creature_id: CreatureId,
        cursor: Point,
    },
    UpdateInteraction {
        cursor: Point,
        velocity: Point,
    },
    EndInteraction {
        cursor: Point,
        velocity: Point,
    },
    CancelInteraction,
    GatherCreatures,
    /// Hold out a snack. The creature decides for itself whether it wants one.
    OfferSnack {
        creature_id: CreatureId,
    },
    /// Hold out a toy. The creature decides for itself whether it feels like playing.
    OfferToy {
        creature_id: CreatureId,
    },
    /// Call the whole colony home now, exactly as the ordinary home visit would.
    SendHome,
    /// Ask the village to share a moment while the houses are out. The companion the menu was
    /// opened on is the one asking; everyone who is home decides for themselves.
    InviteVillageMoment {
        creature_id: CreatureId,
        moment: VillageMoment,
    },
    /// Bring the moment under way to an end.
    StopVillageMoment,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragReleaseKind {
    Placed(SurfaceKind),
    Tossed { velocity: Point },
}
