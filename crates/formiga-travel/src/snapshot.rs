//! The colony as it leaves for Hill: who is travelling, how each one looks and carries itself,
//! how they get on, and how the owner likes things shown. Nothing about where they were on the
//! desktop, what was open on it, or how Desktop runs them.

use crate::appearance::{DesignRecipe, TravelAppearance, mirror};
use crate::document::{Document, TravelError, header_ok, unhex};
use crate::ids::{SessionId, TravelerId};
use crate::limits::*;
use crate::receipt::is_reward_id;
use crate::text::is_sanitized;
use crate::{SNAPSHOT_FORMAT, TRAVEL_FORMAT_VERSION};
use formiga_core as core;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use time::OffsetDateTime;

mirror!(
    TemperamentKind = core::TemperamentKind {
        Sweetheart,
        Troublemaker,
        Grump,
        Explorer,
        Wallflower,
        Showoff,
        Scholar,
        Oddball,
        Lazybones,
        Guardian,
    }
);
mirror!(
    Tension = core::Tension {
        BraveButNervous,
        AffectionateButIndependent,
        PlayfulButShy,
        GentleButStubborn,
        SeriousButCurious,
        ConfidentButEasilyEmbarrassed,
        LazyButCompetitive,
        SuspiciousButLoyal,
        DramaticButCowardly,
        GrumpyButAffectionate,
    }
);
mirror!(Celebration = core::Celebration { Hop, Dance, Twirl });
mirror!(
    Habit = core::Habit {
        LooksFoodOver,
        StretchesBeforeNaps,
        CirclesBeforeNaps,
        WavesHello,
        PlayBows,
    }
);
mirror!(
    AccessoryKind = core::AccessoryKind {
        LeafHat,
        FlowerCrown,
        AcornCap,
        FeatherCap,
        MushroomCap,
        RibbonBow,
        PartyHat,
        Sprout,
        StarClip,
        MoonClip,
        CloudEarmuffs,
        Nightcap,
        KnittedScarf,
        Bandana,
        BellCollar,
        ShellNecklace,
        PetalRuff,
        FriendshipCord,
        TinySatchel,
        SnailPack,
    }
);

/// One of the traits a profile shows, by an identifier that stays the same however Desktop words
/// it: `brave`, `nosy`, `night_owl`. Content should match on these, never on the words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trait {
    Outgoing,
    Chatty,
    Clingy,
    Loner,
    Energetic,
    Excitable,
    Restless,
    Mellow,
    Sleepy,
    Lazy,
    Brave,
    Confident,
    Reckless,
    Cautious,
    Shy,
    Nervous,
    Cowardly,
    Playful,
    Mischievous,
    Silly,
    Serious,
    Stoic,
    Grumpy,
    Feisty,
    Stubborn,
    Bossy,
    Irritable,
    Competitive,
    Gentle,
    Sweet,
    Easygoing,
    Impulsive,
    Impatient,
    Distractible,
    Spontaneous,
    Patient,
    Steady,
    Particular,
    Picky,
    Suspicious,
    Wary,
    Watchful,
    Trusting,
    Friendly,
    Affectionate,
    Loyal,
    Jealous,
    Protective,
    Nurturing,
    Independent,
    Aloof,
    Curious,
    Nosy,
    Obsessive,
    Observant,
    Unbothered,
    Dramatic,
    Vain,
    AttentionSeeking,
    FoodMotivated,
    Messy,
    Eccentric,
    Unpredictable,
    NightOwl,
    EarlyRiser,
    EasilyEmbarrassed,
    SecretlyAffectionate,
    DeeplyLoyal,
    /// A trait a newer Desktop has that this build does not know. It matches nothing.
    #[serde(other)]
    Unknown,
}

impl Trait {
    /// Every trait a profile can show, in the order Desktop's own list keeps them: every
    /// identifier content may name. [`Trait::Unknown`] is not among them.
    pub const ALL: [Self; 68] = [
        Self::Outgoing,
        Self::Chatty,
        Self::Clingy,
        Self::Loner,
        Self::Energetic,
        Self::Excitable,
        Self::Restless,
        Self::Mellow,
        Self::Sleepy,
        Self::Lazy,
        Self::Brave,
        Self::Confident,
        Self::Reckless,
        Self::Cautious,
        Self::Shy,
        Self::Nervous,
        Self::Cowardly,
        Self::Playful,
        Self::Mischievous,
        Self::Silly,
        Self::Serious,
        Self::Stoic,
        Self::Grumpy,
        Self::Feisty,
        Self::Stubborn,
        Self::Bossy,
        Self::Irritable,
        Self::Competitive,
        Self::Gentle,
        Self::Sweet,
        Self::Easygoing,
        Self::Impulsive,
        Self::Impatient,
        Self::Distractible,
        Self::Spontaneous,
        Self::Patient,
        Self::Steady,
        Self::Particular,
        Self::Picky,
        Self::Suspicious,
        Self::Wary,
        Self::Watchful,
        Self::Trusting,
        Self::Friendly,
        Self::Affectionate,
        Self::Loyal,
        Self::Jealous,
        Self::Protective,
        Self::Nurturing,
        Self::Independent,
        Self::Aloof,
        Self::Curious,
        Self::Nosy,
        Self::Obsessive,
        Self::Observant,
        Self::Unbothered,
        Self::Dramatic,
        Self::Vain,
        Self::AttentionSeeking,
        Self::FoodMotivated,
        Self::Messy,
        Self::Eccentric,
        Self::Unpredictable,
        Self::NightOwl,
        Self::EarlyRiser,
        Self::EasilyEmbarrassed,
        Self::SecretlyAffectionate,
        Self::DeeplyLoyal,
    ];
}

impl From<core::Trait> for Trait {
    fn from(value: core::Trait) -> Self {
        match value {
            core::Trait::Outgoing => Self::Outgoing,
            core::Trait::Chatty => Self::Chatty,
            core::Trait::Clingy => Self::Clingy,
            core::Trait::Loner => Self::Loner,
            core::Trait::Energetic => Self::Energetic,
            core::Trait::Excitable => Self::Excitable,
            core::Trait::Restless => Self::Restless,
            core::Trait::Mellow => Self::Mellow,
            core::Trait::Sleepy => Self::Sleepy,
            core::Trait::Lazy => Self::Lazy,
            core::Trait::Brave => Self::Brave,
            core::Trait::Confident => Self::Confident,
            core::Trait::Reckless => Self::Reckless,
            core::Trait::Cautious => Self::Cautious,
            core::Trait::Shy => Self::Shy,
            core::Trait::Nervous => Self::Nervous,
            core::Trait::Cowardly => Self::Cowardly,
            core::Trait::Playful => Self::Playful,
            core::Trait::Mischievous => Self::Mischievous,
            core::Trait::Silly => Self::Silly,
            core::Trait::Serious => Self::Serious,
            core::Trait::Stoic => Self::Stoic,
            core::Trait::Grumpy => Self::Grumpy,
            core::Trait::Feisty => Self::Feisty,
            core::Trait::Stubborn => Self::Stubborn,
            core::Trait::Bossy => Self::Bossy,
            core::Trait::Irritable => Self::Irritable,
            core::Trait::Competitive => Self::Competitive,
            core::Trait::Gentle => Self::Gentle,
            core::Trait::Sweet => Self::Sweet,
            core::Trait::Easygoing => Self::Easygoing,
            core::Trait::Impulsive => Self::Impulsive,
            core::Trait::Impatient => Self::Impatient,
            core::Trait::Distractible => Self::Distractible,
            core::Trait::Spontaneous => Self::Spontaneous,
            core::Trait::Patient => Self::Patient,
            core::Trait::Steady => Self::Steady,
            core::Trait::Particular => Self::Particular,
            core::Trait::Picky => Self::Picky,
            core::Trait::Suspicious => Self::Suspicious,
            core::Trait::Wary => Self::Wary,
            core::Trait::Watchful => Self::Watchful,
            core::Trait::Trusting => Self::Trusting,
            core::Trait::Friendly => Self::Friendly,
            core::Trait::Affectionate => Self::Affectionate,
            core::Trait::Loyal => Self::Loyal,
            core::Trait::Jealous => Self::Jealous,
            core::Trait::Protective => Self::Protective,
            core::Trait::Nurturing => Self::Nurturing,
            core::Trait::Independent => Self::Independent,
            core::Trait::Aloof => Self::Aloof,
            core::Trait::Curious => Self::Curious,
            core::Trait::Nosy => Self::Nosy,
            core::Trait::Obsessive => Self::Obsessive,
            core::Trait::Observant => Self::Observant,
            core::Trait::Unbothered => Self::Unbothered,
            core::Trait::Dramatic => Self::Dramatic,
            core::Trait::Vain => Self::Vain,
            core::Trait::AttentionSeeking => Self::AttentionSeeking,
            core::Trait::FoodMotivated => Self::FoodMotivated,
            core::Trait::Messy => Self::Messy,
            core::Trait::Eccentric => Self::Eccentric,
            core::Trait::Unpredictable => Self::Unpredictable,
            core::Trait::NightOwl => Self::NightOwl,
            core::Trait::EarlyRiser => Self::EarlyRiser,
            core::Trait::EasilyEmbarrassed => Self::EasilyEmbarrassed,
            core::Trait::SecretlyAffectionate => Self::SecretlyAffectionate,
            core::Trait::DeeplyLoyal => Self::DeeplyLoyal,
        }
    }
}

/// What Desktop will do with a receipt. Hill should only ask for what is listed; anything else in
/// a receipt is set aside unread.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// A [`crate::ReturnEffect::Visit`]: Desktop counts the trip and writes one line about it in
    /// the colony's journal, in its own words.
    VisitRecord,
    /// [`crate::ReturnEffect::Souvenir`]s: Desktop keeps each souvenir that
    /// [`TravelSnapshot::accepts_souvenirs`] lists once, to be looked at in its journal, and
    /// nothing else comes of it.
    Souvenirs,
    /// Anything a newer Desktop offers that this build does not know.
    #[serde(other)]
    Unknown,
}

mirror!(
    Theme = core::ThemeChoice {
        System,
        Light,
        Dark
    }
);

/// How the owner likes things shown, as far as both apps share it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Presentation {
    pub reduce_motion: bool,
    /// The notebook's light or dark paper, or whatever the system uses.
    pub theme: Theme,
    /// The notebook's text size, in percent: 100 to 150.
    pub text_scale_percent: u8,
}

impl Default for Presentation {
    fn default() -> Self {
        Self {
            reduce_motion: false,
            theme: Theme::System,
            text_scale_percent: 100,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TravelRole {
    Adult,
    /// A little one, and the adult it belongs to, who always travels with it.
    Mini {
        parent_id: TravelerId,
    },
}

/// Where a companion sits on each of its nine temperament scales, each from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TravelAxes {
    pub social: f32,
    pub energy: f32,
    pub boldness: f32,
    pub playfulness: f32,
    pub curiosity: f32,
    pub feistiness: f32,
    pub impulsiveness: f32,
    pub suspicion: f32,
    pub affection: f32,
}

impl TravelAxes {
    fn values(&self) -> [f32; 9] {
        [
            self.social,
            self.energy,
            self.boldness,
            self.playfulness,
            self.curiosity,
            self.feistiness,
            self.impulsiveness,
            self.suspicion,
            self.affection,
        ]
    }
}

impl From<core::Axes> for TravelAxes {
    fn from(axes: core::Axes) -> Self {
        Self {
            social: axes.social,
            energy: axes.energy,
            boldness: axes.boldness,
            playfulness: axes.playfulness,
            curiosity: axes.curiosity,
            feistiness: axes.feistiness,
            impulsiveness: axes.impulsiveness,
            suspicion: axes.suspicion,
            affection: axes.affection,
        }
    }
}

impl From<TravelAxes> for core::Axes {
    fn from(axes: TravelAxes) -> Self {
        Self {
            social: axes.social,
            energy: axes.energy,
            boldness: axes.boldness,
            playfulness: axes.playfulness,
            curiosity: axes.curiosity,
            feistiness: axes.feistiness,
            impulsiveness: axes.impulsiveness,
            suspicion: axes.suspicion,
            affection: axes.affection,
        }
    }
}

/// Who a companion is, as its profile in Desktop's notebook shows it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TravelCharacter {
    pub temperament: TemperamentKind,
    pub axes: TravelAxes,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tension: Option<Tension>,
    /// The three traits its profile shows, in Desktop's words: "Brave", "Nosy", "Night owl". For
    /// showing only: the words may be changed or translated.
    pub traits: Vec<String>,
    /// The same traits as identifiers, in the same order: what content should match on. Since
    /// travel version 2; a snapshot from an older Desktop has none, and only the words.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trait_ids: Vec<Trait>,
    /// The line its profile leads with: "A suspicious guardian".
    pub phrase: String,
}

/// The pace it moves at, each from 0 to 1: what gives its walk and its play their own tempo.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TravelMotion {
    pub activity: f32,
    pub playfulness: f32,
    pub boldness: f32,
    /// How it celebrates when something goes its way. Optional: Desktop works it out from the
    /// companion's own seed, which never travels, so Hill should use this rather than work it out
    /// again from a stand-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub celebration: Option<Celebration>,
}

/// What it is wearing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AccessoryItem {
    /// Something made from a find.
    Worn { item: AccessoryKind },
    /// A find itself, by catalogue variant, pinned on the chest.
    Pin { trinket: u8 },
}

/// The five colours an accessory is drawn in: resolved by Desktop from the colony it belongs to,
/// so Hill draws it the same without knowing anything about that colony.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessoryColors {
    pub outline: [u8; 3],
    pub deep: [u8; 3],
    pub body: [u8; 3],
    pub light: [u8; 3],
    pub accent: [u8; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TravelAccessory {
    pub item: AccessoryItem,
    pub colors: AccessoryColors,
}

/// One companion on the train. Its identity is its id, its name and its recipe; the seed it was
/// first drawn from stays home, since a founder's is the colony's own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Traveler {
    pub id: TravelerId,
    pub name: String,
    pub role: TravelRole,
    #[serde(with = "time::serde::rfc3339")]
    pub born_at_utc: OffsetDateTime,
    pub appearance: TravelAppearance,
    /// How tall it stands against the average companion, in percent: the same on every display
    /// and at every scale.
    pub stature_percent: u8,
    /// Its share of an adult's size, in percent: 100 for an adult, less for a little one.
    pub scale_percent: u8,
    pub character: TravelCharacter,
    pub motion: TravelMotion,
    /// The little habits it has picked up, in the order it picked them up.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub habits: Vec<Habit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessory: Option<TravelAccessory>,
}

/// How strongly something is so between two companions. Bands rather than Desktop's own scores,
/// so retuning how bonds grow on Desktop never changes what Hill reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Band {
    None,
    Low,
    Medium,
    High,
}

impl Band {
    /// The band a Desktop bond score falls in. `High` begins where Desktop calls two companions
    /// close friends.
    pub const fn of(score: u8) -> Self {
        match score {
            0..16 => Self::None,
            16..64 => Self::Low,
            64..112 => Self::Medium,
            _ => Self::High,
        }
    }
}

/// How one pair of travelers get on. The pair is written lower id first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TravelRelationship {
    pub a: TravelerId,
    pub b: TravelerId,
    pub affinity: Band,
    pub familiarity: Band,
    pub playfulness: Band,
    pub avoidance: Band,
}

/// The colony as it leaves for Hill.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TravelSnapshot {
    pub format: String,
    pub version: u32,
    pub min_reader_version: u32,
    pub session_id: SessionId,
    /// The same colony on every trip, as 16 lowercase hex digits: a one-way digest of what makes
    /// it that colony, so Hill can keep its own record of each colony it has hosted without
    /// learning anything it could rebuild the colony from.
    pub colony_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at_utc: OffsetDateTime,
    pub desktop_version: String,
    pub capabilities: Vec<Capability>,
    /// The souvenirs Desktop keeps, by Formiga Hill's own identifiers. The list is the same for
    /// every colony and says nothing about which the colony already has. Hill brings home only
    /// these, and only while [`Capability::Souvenirs`] is offered. Empty before version 3.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepts_souvenirs: Vec<String>,
    pub travelers: Vec<Traveler>,
    #[serde(default)]
    pub relationships: Vec<TravelRelationship>,
    #[serde(default)]
    pub presentation: Presentation,
}

impl TravelSnapshot {
    /// An empty snapshot with this build's header, for the projection to fill.
    pub(crate) fn new(
        session_id: SessionId,
        min_reader_version: u32,
        colony_id: String,
        created_at_utc: OffsetDateTime,
        desktop_version: String,
    ) -> Self {
        Self {
            format: SNAPSHOT_FORMAT.to_owned(),
            version: TRAVEL_FORMAT_VERSION,
            min_reader_version,
            session_id,
            colony_id,
            created_at_utc,
            desktop_version,
            capabilities: vec![Capability::VisitRecord, Capability::Souvenirs],
            accepts_souvenirs: core::Souvenir::ALL
                .map(|souvenir| souvenir.id().to_owned())
                .to_vec(),
            travelers: Vec::new(),
            relationships: Vec::new(),
            presentation: Presentation::default(),
        }
    }

    pub fn traveler(&self, id: TravelerId) -> Option<&Traveler> {
        self.travelers.iter().find(|traveler| traveler.id == id)
    }

    pub fn offers(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// Whether Desktop keeps the souvenir Formiga Hill calls `id`, if the colony brings it home.
    pub fn accepts_souvenir(&self, id: &str) -> bool {
        self.offers(Capability::Souvenirs) && self.accepts_souvenirs.iter().any(|kept| kept == id)
    }
}

impl Document for TravelSnapshot {
    const FORMAT: &'static str = SNAPSHOT_FORMAT;
    const MAX_BYTES: u64 = MAX_SNAPSHOT_BYTES;

    fn validate(&self) -> Result<(), TravelError> {
        let invalid = TravelError::invalid;
        if !header_ok(
            &self.format,
            self.version,
            self.min_reader_version,
            SNAPSHOT_FORMAT,
        ) {
            return Err(invalid(
                "the snapshot's header is not one this build writes",
            ));
        }
        if !is_sanitized(&self.desktop_version, MAX_VERSION_CHARS) {
            return Err(invalid("the Desktop version is not plain text"));
        }
        if unhex::<8>(&self.colony_id).is_none() {
            return Err(invalid("a colony id is 16 lowercase hex digits"));
        }
        if !(100..=150).contains(&self.presentation.text_scale_percent) {
            return Err(invalid("a text size out of range"));
        }
        if self.capabilities.len() > MAX_CAPABILITIES {
            return Err(invalid("too many capabilities"));
        }
        let souvenirs: BTreeSet<_> = self.accepts_souvenirs.iter().collect();
        if self.accepts_souvenirs.len() > MAX_SOUVENIRS
            || souvenirs.len() != self.accepts_souvenirs.len()
            || !self.accepts_souvenirs.iter().all(|id| is_reward_id(id))
        {
            return Err(invalid(
                "the souvenirs Desktop keeps are not a short list of different identifiers",
            ));
        }
        if self.travelers.is_empty() || self.travelers.len() > MAX_TRAVELERS {
            return Err(invalid("a snapshot carries one to twelve travelers"));
        }
        let ids: BTreeSet<_> = self.travelers.iter().map(|t| t.id).collect();
        if ids.len() != self.travelers.len() {
            return Err(invalid("two travelers share an id"));
        }
        for traveler in &self.travelers {
            traveler.validate()?;
            if let TravelRole::Mini { parent_id } = traveler.role {
                let parent_is_adult = self
                    .traveler(parent_id)
                    .is_some_and(|parent| parent.role == TravelRole::Adult);
                if !parent_is_adult {
                    return Err(invalid("a little one travels without its adult"));
                }
            }
        }
        if self.relationships.len() > MAX_RELATIONSHIPS {
            return Err(invalid("too many relationships"));
        }
        let mut pairs = BTreeSet::new();
        for pair in &self.relationships {
            if pair.a >= pair.b || !ids.contains(&pair.a) || !ids.contains(&pair.b) {
                return Err(invalid("a relationship names someone not travelling"));
            }
            if !pairs.insert((pair.a, pair.b)) {
                return Err(invalid("a pair is listed twice"));
            }
        }
        Ok(())
    }
}

impl Traveler {
    fn validate(&self) -> Result<(), TravelError> {
        let invalid = TravelError::invalid;
        if !is_sanitized(&self.name, MAX_NAME_CHARS) {
            return Err(invalid("a traveler's name is not plain text"));
        }
        if self.role == (TravelRole::Mini { parent_id: self.id }) {
            return Err(invalid("a little one is its own adult"));
        }
        self.appearance.validate()?;
        if !(50..=200).contains(&self.stature_percent) || !(1..=100).contains(&self.scale_percent) {
            return Err(invalid("a traveler's size is out of range"));
        }
        let unit = |value: f32| value.is_finite() && (0.0..=1.0).contains(&value);
        if !self.character.axes.values().into_iter().all(unit)
            || ![
                self.motion.activity,
                self.motion.playfulness,
                self.motion.boldness,
            ]
            .into_iter()
            .all(unit)
        {
            return Err(invalid("a traveler's character is out of range"));
        }
        if self.character.traits.len() > MAX_TRAITS
            || self.character.trait_ids.len() > MAX_TRAITS
            || !self
                .character
                .traits
                .iter()
                .all(|label| is_sanitized(label, MAX_TRAIT_CHARS))
            || !is_sanitized(&self.character.phrase, MAX_PHRASE_CHARS)
        {
            return Err(invalid("a traveler's profile is not plain text"));
        }
        let habits: BTreeSet<_> = self.habits.iter().map(|habit| *habit as u8).collect();
        if self.habits.len() > MAX_HABITS || habits.len() != self.habits.len() {
            return Err(invalid(
                "a traveler's habits are not a short list of different ones",
            ));
        }
        if let Some(TravelAccessory {
            item: AccessoryItem::Pin { trinket },
            ..
        }) = self.accessory
            && trinket >= core::TRINKET_VARIANTS
        {
            return Err(invalid("a pinned find this build does not have"));
        }
        Ok(())
    }

    /// The recipe its share code would carry.
    pub fn recipe(&self) -> Option<&DesignRecipe> {
        self.appearance.design.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_begin_where_desktop_calls_a_pair_close() {
        assert_eq!(Band::of(0), Band::None);
        assert_eq!(Band::of(15), Band::None);
        assert_eq!(Band::of(16), Band::Low);
        assert_eq!(Band::of(63), Band::Low);
        assert_eq!(Band::of(64), Band::Medium);
        assert_eq!(Band::of(111), Band::Medium);
        assert_eq!(Band::of(112), Band::High);
        assert_eq!(Band::of(255), Band::High);
    }

    #[test]
    fn every_trait_desktop_knows_has_an_identifier_and_no_two_share_one() {
        let ids: Vec<_> = core::Trait::ALL.map(Trait::from).to_vec();
        assert_eq!(ids, Trait::ALL.to_vec(), "the list follows Desktop's own");
        let written: std::collections::BTreeSet<String> = Trait::ALL
            .iter()
            .map(|id| {
                serde_json::to_value(id)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert_eq!(written.len(), Trait::ALL.len());
        assert!(written.contains("night_owl") && written.contains("brave"));
        assert!(!written.contains("unknown"));
    }

    #[test]
    fn capabilities_from_a_newer_desktop_read_as_unknown() {
        let read: Vec<Capability> =
            serde_json::from_str(r#"["visit_record", "souvenirs", "photos", "postcards"]"#)
                .unwrap();
        assert_eq!(
            read,
            vec![
                Capability::VisitRecord,
                Capability::Souvenirs,
                Capability::Unknown,
                Capability::Unknown
            ]
        );
    }
}
