//! Bounded, local keepsakes and user preferences. No polling or desktop-content history.
use crate::*;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const MAX_JOURNAL_ENTRIES: usize = 64;
/// Pinned moments sit alongside the rolling journal without extending it.
pub const MAX_PINNED_ENTRIES: usize = 8;
/// The trinket variants the artwork can draw; the identifier is the variant itself. Forty-eight
/// turn up on any ordinary day and the rest only under some circumstance; `crate::trinkets` is
/// the table.
pub const TRINKET_VARIANTS: u8 = 160;
/// A week of routine changes is plenty; more would be a calendar, not a habit.
pub const MAX_SCHEDULED_TRANSITIONS: usize = 14;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum JournalMoment {
    Arrival,
    Discovery,
    Friendship(CreatureId),
    Preference(ProfileDescriptor),
    Ritual(RitualKind),
    Object(ColonyObjectKind),
    Decoration(ShelterDecorationKind),
    /// Someone came by the houses. A visitor is never a colony member, so the moment carries the
    /// name it went by rather than an identifier the colony could not look up later.
    Visit(String),
    /// A companion picked up a little habit of its own.
    Habit(Habit),
    /// Something new arrived for the village to choose from.
    Unlocked(VillageItem),
    /// A visitor the guest book already knew came by again, and which visit of theirs this was,
    /// counting the ones the book still holds.
    Revisit(String, u16),
}

/// How many moments the journal holds on to past their turn to roll out because they are the
/// colony's landmarks: arrivals, friendships and first visits. Inside the journal's own cap, not
/// on top of it.
pub const MAX_LANDMARK_ENTRIES: usize = 24;

impl JournalMoment {
    /// A landmark in the colony's history: who arrived, who grew close, and who first came by.
    /// When the journal is full, everyday moments roll out before these do.
    pub const fn is_landmark(&self) -> bool {
        matches!(self, Self::Arrival | Self::Friendship(_) | Self::Visit(_))
    }

    /// Worth a small sign that something new was written: everything but the everyday finds,
    /// shared moments and village news, which happen most days.
    pub const fn is_noteworthy(&self) -> bool {
        matches!(
            self,
            Self::Arrival
                | Self::Friendship(_)
                | Self::Preference(_)
                | Self::Visit(_)
                | Self::Revisit(..)
                | Self::Habit(_)
        )
    }

    /// The kind of moment, for the journal's filters.
    pub const fn kind(&self) -> MomentKind {
        match self {
            Self::Arrival => MomentKind::Arrivals,
            Self::Friendship(_) => MomentKind::Friendships,
            Self::Discovery => MomentKind::Finds,
            Self::Preference(_) | Self::Habit(_) => MomentKind::Ways,
            Self::Ritual(_) => MomentKind::Together,
            Self::Object(_) | Self::Decoration(_) | Self::Unlocked(_) => MomentKind::Village,
            Self::Visit(_) | Self::Revisit(..) => MomentKind::Visitors,
        }
    }
}

/// What the journal can be filtered to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MomentKind {
    Arrivals,
    Friendships,
    Finds,
    Ways,
    Together,
    Village,
    Visitors,
}

impl MomentKind {
    pub const ALL: [Self; 7] = [
        Self::Arrivals,
        Self::Friendships,
        Self::Ways,
        Self::Together,
        Self::Finds,
        Self::Visitors,
        Self::Village,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Arrivals => "Arrivals",
            Self::Friendships => "Friendships",
            Self::Finds => "Finds",
            Self::Ways => "Ways & habits",
            Self::Together => "Together",
            Self::Village => "The village",
            Self::Visitors => "Visitors",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JournalEntry {
    #[serde(with = "time::serde::rfc3339")]
    pub at: OffsetDateTime,
    pub creature: Option<CreatureId>,
    pub moment: JournalMoment,
}

/// A journal moment the reader has kept. It names an existing entry rather than copying its words,
/// so a pin can never say something the journal did not.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PinnedMoment {
    #[serde(with = "time::serde::rfc3339")]
    pub at: OffsetDateTime,
    pub creature: Option<CreatureId>,
    pub moment: JournalMoment,
}

impl PinnedMoment {
    pub fn names(&self, entry: &JournalEntry) -> bool {
        self.at == entry.at && self.creature == entry.creature && self.moment == entry.moment
    }

    pub fn of(entry: &JournalEntry) -> Self {
        Self {
            at: entry.at,
            creature: entry.creature,
            moment: entry.moment.clone(),
        }
    }
}

/// The first time one trinket variant was found, and who found it. Nothing here is invented from
/// an aggregate count: a colony that predates the scrapbook simply starts it empty.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScrapbookRecord {
    /// A catalogue variant, stable for the life of the colony: the identifier the artwork is
    /// drawn from and `trinket_info` describes.
    pub variant: u8,
    #[serde(with = "time::serde::rfc3339")]
    pub first_at: OffsetDateTime,
    pub finder: Option<CreatureId>,
    /// Kept so a departed finder still has a name, without keeping a copy of the creature.
    pub finder_name: String,
}

/// How the interface should look. These never change what a creature does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearancePreferences {
    pub theme: ThemeChoice,
    /// Percent of the base text size: 100 to 150, in steps of ten.
    pub text_scale: u8,
    /// A subtle cached outline behind sprites, for bright or busy wallpaper.
    pub sprite_outline: bool,
}

impl Default for AppearancePreferences {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::default(),
            text_scale: 100,
            sprite_outline: false,
        }
    }
}

impl AppearancePreferences {
    pub fn normalize(&mut self) {
        self.text_scale = (self.text_scale / 10 * 10).clamp(100, 150);
    }
}

/// One weekday-and-time change between the two saved presets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledTransition {
    /// Which days it applies to; bit 0 is Monday.
    pub days: u8,
    /// Local minutes past midnight, 0..1440.
    pub minute: u16,
    /// Which of the two saved presets to move to.
    pub preset: u8,
}

/// Opt-in weekday transitions between the two saved presets. Nothing here polls: the schedule is
/// evaluated inside the existing world tick, and adds no timer, thread, or wakeup of its own.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoutineSchedule {
    pub enabled: bool,
    pub transitions: Vec<ScheduledTransition>,
    /// A manual choice, honored until the next scheduled transition or an explicit resume.
    pub overridden: bool,
    /// The preset the schedule last put in place, so waking does not replay a missed week.
    pub applied: Option<u8>,
}

impl RoutineSchedule {
    /// Which days a transition covers, as the local weekday bit the schedule stores.
    fn covers(transition: &ScheduledTransition, weekday: time::Weekday) -> bool {
        transition.days & (1 << weekday.number_days_from_monday()) != 0
    }

    /// The preset this schedule intends at `local`: the most recent transition at or before it,
    /// looking back one week. Only the current intended state matters, so a machine that was
    /// asleep through three transitions wakes into the right one rather than replaying them.
    pub fn intended(&self, local: OffsetDateTime) -> Option<u8> {
        let minutes = i64::from(local.hour()) * 60 + i64::from(local.minute());
        (0..8).find_map(|back| {
            let date = local
                .date()
                .checked_sub(time::Duration::days(back))
                .unwrap_or(local.date());
            self.transitions
                .iter()
                .enumerate()
                .filter(|(_, t)| Self::covers(t, date.weekday()))
                .filter(|(_, t)| back > 0 || i64::from(t.minute) <= minutes)
                // Two transitions at the same minute are settled by their order in the list, so
                // the answer never depends on when the question is asked.
                .max_by_key(|(index, t)| (t.minute, *index))
                .map(|(_, t)| t.preset)
        })
    }

    /// The next transition after `local`, for showing what happens next.
    pub fn next_after(&self, local: OffsetDateTime) -> Option<(OffsetDateTime, u8)> {
        let minutes = i64::from(local.hour()) * 60 + i64::from(local.minute());
        (0..8).find_map(|ahead| {
            let date = local.date().checked_add(time::Duration::days(ahead))?;
            self.transitions
                .iter()
                .enumerate()
                .filter(|(_, t)| Self::covers(t, date.weekday()))
                .filter(|(_, t)| ahead > 0 || i64::from(t.minute) > minutes)
                .min_by_key(|(index, t)| (t.minute, *index))
                .and_then(|(_, t)| {
                    Some((
                        date.with_hms((t.minute / 60) as u8, (t.minute % 60) as u8, 0)
                            .ok()?
                            .assume_offset(local.offset()),
                        t.preset,
                    ))
                })
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BehaviorPreset {
    pub habitat: HabitatPolicy,
    pub window_ledges: bool,
    pub cursor_reactions: bool,
    pub reduce_motion: bool,
}
impl BehaviorPreset {
    pub fn capture(settings: &Settings) -> Self {
        Self {
            habitat: settings.habitat.clone(),
            window_ledges: settings.window_ledges,
            cursor_reactions: settings.cursor_reactions,
            reduce_motion: settings.reduce_motion,
        }
    }
    pub fn apply(&self, settings: &mut Settings) {
        settings.habitat = self.habitat.clone();
        settings.window_ledges = self.window_ledges;
        settings.cursor_reactions = self.cursor_reactions;
        settings.reduce_motion = self.reduce_motion;
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompanionState {
    pub journal: Vec<JournalEntry>,
    pub onboarding_complete: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    pub quiet_until: Option<OffsetDateTime>,
    pub modes: [Option<BehaviorPreset>; 2],
    /// Journal moments kept on purpose, at most eight, never shown twice.
    pub pins: Vec<PinnedMoment>,
    /// One record per trinket variant: the first find of each.
    pub scrapbook: Vec<ScrapbookRecord>,
    pub appearance: AppearancePreferences,
    pub schedule: RoutineSchedule,
    /// Everything written in the journal up to this moment has been read. A noteworthy moment
    /// after it is what the notebook's tabs and the tray icon quietly mark.
    #[serde(
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub journal_seen_until: Option<OffsetDateTime>,
}
impl Default for CompanionState {
    fn default() -> Self {
        // Existing colonies never see an unsolicited introduction after migration.
        Self {
            journal: Vec::new(),
            onboarding_complete: true,
            quiet_until: None,
            modes: [None, None],
            pins: Vec::new(),
            scrapbook: Vec::new(),
            appearance: AppearancePreferences::default(),
            schedule: RoutineSchedule::default(),
            journal_seen_until: None,
        }
    }
}
impl CompanionState {
    pub fn record(&mut self, event: &WorldEvent, at: OffsetDateTime) {
        let (creature, moment) = match *event {
            WorldEvent::CreatureSpawned { creature_id } => {
                (Some(creature_id), JournalMoment::Arrival)
            }
            WorldEvent::ActionCompleted {
                creature_id,
                action: ActionKind::PresentDiscovery,
            } => (Some(creature_id), JournalMoment::Discovery),
            WorldEvent::ProfileChanged {
                creature_id,
                new_descriptor: Some(descriptor),
                ..
            } => (Some(creature_id), JournalMoment::Preference(descriptor)),
            WorldEvent::RitualCompleted { kind } => (None, JournalMoment::Ritual(kind)),
            WorldEvent::ColonyObjectAdded { kind, .. } => (None, JournalMoment::Object(kind)),
            WorldEvent::VillageUnlocked { item } => (None, JournalMoment::Unlocked(item)),
            WorldEvent::HabitLearned { creature_id, habit } => {
                (Some(creature_id), JournalMoment::Habit(habit))
            }
            _ => return,
        };
        self.remember(creature, moment, at);
    }
    pub(crate) fn remember(
        &mut self,
        creature: Option<CreatureId>,
        moment: JournalMoment,
        at: OffsetDateTime,
    ) {
        // Repeated discoveries/preferences remain meaningful, not a play-by-play feed.
        if self.journal.iter().rev().any(|entry| {
            entry.creature == creature
                && entry.moment == moment
                && at - entry.at < time::Duration::hours(6)
        }) {
            return;
        }
        self.journal.push(JournalEntry {
            at,
            creature,
            moment,
        });
        self.trim_journal();
    }

    /// Bring the journal back within its cap. Everyday moments roll out oldest first; the
    /// colony's landmarks are kept past their turn, up to [`MAX_LANDMARK_ENTRIES`] of them, and
    /// only the oldest of those goes once there are more.
    fn trim_journal(&mut self) {
        while self.journal.len() > MAX_JOURNAL_ENTRIES {
            let landmarks = self
                .journal
                .iter()
                .filter(|entry| entry.moment.is_landmark())
                .count();
            let oldest = if landmarks > MAX_LANDMARK_ENTRIES {
                self.journal
                    .iter()
                    .position(|entry| entry.moment.is_landmark())
            } else {
                self.journal
                    .iter()
                    .position(|entry| !entry.moment.is_landmark())
            };
            self.journal.remove(oldest.unwrap_or(0));
        }
    }

    /// Whether an everyday moment has rolled out of the journal already: it is full, and the
    /// oldest everyday moment it still holds is no older than `since`.
    pub fn rolled_out_since(&self, since: OffsetDateTime) -> bool {
        self.journal.len() >= MAX_JOURNAL_ENTRIES
            && self
                .journal
                .iter()
                .find(|entry| !entry.moment.is_landmark())
                .is_some_and(|entry| entry.at >= since)
    }

    /// The noteworthy moments written since the journal was last read, newest first.
    pub fn unread(&self) -> impl Iterator<Item = &JournalEntry> {
        self.journal
            .iter()
            .rev()
            .take_while(|entry| self.journal_seen_until.is_none_or(|seen| entry.at > seen))
            .filter(|entry| entry.moment.is_noteworthy())
    }

    /// Everything written so far has been read.
    pub fn mark_read(&mut self) -> bool {
        let newest = self.journal.iter().map(|entry| entry.at).max();
        if newest.is_some() && newest > self.journal_seen_until {
            self.journal_seen_until = newest;
            return true;
        }
        false
    }
    /// Keep a moment. A pin points at a journal entry; keeping one never invents a new moment.
    pub fn pin(&mut self, entry: &JournalEntry) -> bool {
        if self.pins.iter().any(|pin| pin.names(entry)) || self.pins.len() >= MAX_PINNED_ENTRIES {
            return false;
        }
        self.pins.push(PinnedMoment::of(entry));
        true
    }

    pub fn unpin(&mut self, entry: &JournalEntry) {
        self.pins.retain(|pin| !pin.names(entry));
    }

    pub fn pinned(&self, entry: &JournalEntry) -> bool {
        self.pins.iter().any(|pin| pin.names(entry))
    }

    /// The first find of one trinket variant. Later finds of the same variant change nothing.
    pub fn remember_discovery(
        &mut self,
        variant: u8,
        finder: CreatureId,
        finder_name: String,
        at: OffsetDateTime,
    ) {
        let variant = variant % TRINKET_VARIANTS;
        if self.scrapbook.iter().any(|r| r.variant == variant) {
            return;
        }
        self.scrapbook.push(ScrapbookRecord {
            variant,
            first_at: at,
            finder: Some(finder),
            finder_name,
        });
        self.scrapbook.sort_by_key(|record| record.variant);
    }

    pub fn normalize(&mut self) {
        self.trim_journal();
        self.pins.truncate(MAX_PINNED_ENTRIES);
        self.scrapbook.retain(|r| r.variant < TRINKET_VARIANTS);
        self.scrapbook.sort_by_key(|record| record.variant);
        self.scrapbook.dedup_by_key(|record| record.variant);
        self.scrapbook.truncate(usize::from(TRINKET_VARIANTS));
        self.appearance.normalize();
        self.schedule
            .transitions
            .retain(|t| t.minute < 1440 && t.preset < 2 && t.days != 0);
        self.schedule
            .transitions
            .truncate(MAX_SCHEDULED_TRANSITIONS);
        for mode in self.modes.iter_mut().flatten() {
            mode.habitat.zones.truncate(MAX_HABITAT_ZONES);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::{Duration, macros::datetime};
    fn work_and_relax() -> RoutineSchedule {
        RoutineSchedule {
            enabled: true,
            // Weekdays: work at 09:00, relax at 18:00. Bit 0 is Monday.
            transitions: vec![
                ScheduledTransition {
                    days: 0b0011111,
                    minute: 9 * 60,
                    preset: 0,
                },
                ScheduledTransition {
                    days: 0b0011111,
                    minute: 18 * 60,
                    preset: 1,
                },
            ],
            ..RoutineSchedule::default()
        }
    }

    fn entry(minutes: i64, moment: JournalMoment) -> (OffsetDateTime, JournalMoment) {
        (
            datetime!(2026-09-01 0:00 UTC) + Duration::minutes(minutes),
            moment,
        )
    }

    #[test]
    fn a_full_journal_keeps_its_landmarks_past_their_turn() {
        let mut state = CompanionState::default();
        let (at, moment) = entry(0, JournalMoment::Arrival);
        state.remember(Some(1), moment, at);
        let (at, moment) = entry(1, JournalMoment::Friendship(2));
        state.remember(Some(1), moment, at);
        // Far more everyday moments than the journal holds, each its own so none is throttled.
        for index in 0..200 {
            let (at, moment) = entry(10 + index * 400, JournalMoment::Discovery);
            state.remember(Some(1 + (index as u64 % 3)), moment, at);
        }
        assert_eq!(state.journal.len(), MAX_JOURNAL_ENTRIES);
        assert_eq!(state.journal[0].moment, JournalMoment::Arrival);
        assert_eq!(state.journal[1].moment, JournalMoment::Friendship(2));
        assert!(
            state
                .journal
                .windows(2)
                .all(|pair| pair[0].at <= pair[1].at)
        );
        // Landmarks themselves are capped inside the journal, oldest first.
        for index in 0..40 {
            let (at, moment) = entry(100_000 + index, JournalMoment::Visit(format!("V{index}")));
            state.remember(None, moment, at);
        }
        let landmarks = state
            .journal
            .iter()
            .filter(|e| e.moment.is_landmark())
            .count();
        assert_eq!(landmarks, MAX_LANDMARK_ENTRIES);
        assert_eq!(state.journal.len(), MAX_JOURNAL_ENTRIES);
        assert!(
            !state
                .journal
                .iter()
                .any(|e| e.moment == JournalMoment::Arrival)
        );
    }

    #[test]
    fn only_noteworthy_moments_since_the_last_read_are_unread() {
        let mut state = CompanionState::default();
        let (at, moment) = entry(0, JournalMoment::Arrival);
        state.remember(Some(1), moment, at);
        assert_eq!(state.unread().count(), 1);
        assert!(state.mark_read());
        assert!(!state.mark_read());
        assert_eq!(state.unread().count(), 0);
        let (at, moment) = entry(10, JournalMoment::Discovery);
        state.remember(Some(1), moment, at);
        assert_eq!(state.unread().count(), 0, "a find is everyday news");
        let (at, moment) = entry(20, JournalMoment::Revisit("Wren".into(), 2));
        state.remember(None, moment, at);
        assert_eq!(state.unread().count(), 1);
    }

    #[test]
    fn a_tally_counts_who_sought_whom_and_remembers_only_warm_moments() {
        let mut tally = RelationshipTally::default();
        let at = datetime!(2026-09-01 0:00 UTC);
        tally.count(Some(true), RelationshipExperience::Followed, at);
        tally.count(Some(true), RelationshipExperience::HomecomingGreeting, at);
        tally.count(Some(false), RelationshipExperience::BroughtDiscovery, at);
        // A ritual greeting was nobody's choice.
        tally.count(Some(false), RelationshipExperience::Greeting, at);
        assert_eq!(tally.sought, [2, 1]);
        assert_eq!(tally.greetings, 2);
        assert_eq!(tally.gifts, 1);
        let later = at + Duration::hours(1);
        tally.count(None, RelationshipExperience::Squabble, later);
        tally.count(None, RelationshipExperience::CalmProximity, later);
        assert_eq!(tally.memory.map(|m| m.at), Some(at));
        assert_eq!(tally.squabbles, 1);
        assert_eq!(tally.calm_spells, 2);
        for _ in 0..70_000 {
            tally.count(None, RelationshipExperience::SharedRest, later);
        }
        assert_eq!(tally.shared_rests, u16::MAX);
    }

    #[test]
    fn a_schedule_names_the_current_routine_and_the_next_change() {
        let schedule = work_and_relax();
        // Wednesday.
        let midmorning = datetime!(2026-09-16 10:30 UTC);
        assert_eq!(schedule.intended(midmorning), Some(0));
        assert_eq!(
            schedule.next_after(midmorning),
            Some((datetime!(2026-09-16 18:00 UTC), 1))
        );
        // Overnight: before the first change of the day, the previous evening still holds.
        let small_hours = datetime!(2026-09-17 3:00 UTC);
        assert_eq!(schedule.intended(small_hours), Some(1));
        assert_eq!(
            schedule.next_after(small_hours),
            Some((datetime!(2026-09-17 9:00 UTC), 0))
        );
        // The weekend keeps Friday evening's routine right through to Monday morning.
        let sunday = datetime!(2026-09-20 12:00 UTC);
        assert_eq!(schedule.intended(sunday), Some(1));
        assert_eq!(
            schedule.next_after(sunday),
            Some((datetime!(2026-09-21 9:00 UTC), 0))
        );
        // An empty schedule has nothing to say, and never guesses.
        assert_eq!(RoutineSchedule::default().intended(midmorning), None);
        assert_eq!(RoutineSchedule::default().next_after(midmorning), None);
    }

    #[test]
    fn duplicate_times_timezone_changes_and_missed_transitions_stay_predictable() {
        let mut schedule = work_and_relax();
        // Two changes at the same minute: list order decides, the same way every time.
        schedule.transitions.push(ScheduledTransition {
            days: 0b1111111,
            minute: 9 * 60,
            preset: 1,
        });
        let wednesday = datetime!(2026-09-16 9:30 UTC);
        assert_eq!(schedule.intended(wednesday), Some(1));
        assert_eq!(schedule.intended(wednesday), Some(1));
        // The same instant read in another timezone answers for that local time, not for UTC.
        let schedule = work_and_relax();
        let before_work = datetime!(2026-09-16 8:30 UTC);
        assert_eq!(schedule.intended(before_work), Some(1));
        let same_instant_further_east =
            before_work.to_offset(time::UtcOffset::from_hms(3, 0, 0).unwrap());
        assert_eq!(schedule.intended(same_instant_further_east), Some(0));
        // A week asleep still resolves to one answer: today's, not a replay of every change.
        assert_eq!(schedule.intended(datetime!(2026-09-23 10:00 UTC)), Some(0));
        // An hour that a daylight-saving jump skips simply never becomes the most recent one
        // until the clock is past it, and then it applies exactly once.
        let skipped = RoutineSchedule {
            enabled: true,
            transitions: vec![ScheduledTransition {
                days: 0b1111111,
                minute: 2 * 60 + 30,
                preset: 1,
            }],
            ..RoutineSchedule::default()
        };
        assert_eq!(skipped.intended(datetime!(2026-03-29 1:30 UTC)), Some(1));
        assert_eq!(skipped.intended(datetime!(2026-03-29 3:30 UTC)), Some(1));
    }

    #[test]
    fn journal_is_bounded_throttled_and_omits_desktop_observations() {
        let now = datetime!(2026-09-14 12:00 UTC);
        let mut state = CompanionState::default();
        let discovery = WorldEvent::ActionCompleted {
            creature_id: 7,
            action: ActionKind::PresentDiscovery,
        };
        state.record(&discovery, now);
        state.record(&discovery, now + Duration::minutes(3));
        state.record(
            &WorldEvent::CreaturePlaced {
                creature_id: 7,
                display: DisplayKey([44; 16]),
                region: 8,
            },
            now,
        );
        assert_eq!(state.journal.len(), 1);
        for day in 1..100 {
            state.record(&discovery, now + Duration::days(day));
        }
        assert_eq!(state.journal.len(), MAX_JOURNAL_ENTRIES);
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.len() < 12 * 1024);
        assert!(!json.contains("display"));
        assert!(!json.contains("region"));
        assert_eq!(state, serde_json::from_str(&json).unwrap());
    }
    /// A desktop that will not settle: a ledge to stand on, a window sliding about beside it, and
    /// a cursor sweeping past. Everything the lifecycle checks has to survive this.
    fn restless_desktop(step: u64) -> DesktopSnapshot {
        let drift = (step % 8) as f32 * 30.0;
        DesktopSnapshot {
            monitors: vec![MonitorInfo {
                id: 1,
                display_key: DisplayKey([1; 16]),
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1440.0,
                    height: 900.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 24.0,
                    width: 1440.0,
                    height: 826.0,
                },
                scale_factor: 2.0,
                primary: true,
            }],
            windows: vec![
                DesktopWindow {
                    key: 701,
                    bounds: DesktopRect {
                        x: 260.0,
                        y: 600.0,
                        width: 560.0,
                        height: 230.0,
                    },
                    z_order: 0,
                    visible: true,
                    minimized: false,
                    application: None,
                    application_name: None,
                },
                DesktopWindow {
                    key: 702,
                    bounds: DesktopRect {
                        x: 900.0 - drift,
                        y: 380.0,
                        width: 420.0,
                        height: 190.0,
                    },
                    z_order: 1,
                    visible: true,
                    minimized: false,
                    application: None,
                    application_name: None,
                },
            ],
            cursor: CursorSnapshot {
                position: Point {
                    x: 1300.0 - drift,
                    y: 700.0,
                },
                velocity: Point { x: -600.0, y: 0.0 },
                available: true,
            },
            window_sample: Some(WindowSample {
                monotonic_millis: step * 50,
                reliable: true,
            }),
            cursor_sample_millis: Some(step * 50),
            ..Default::default()
        }
    }

    /// A colony of exactly `count` companions, out of the house and living on that desktop.
    fn colony_of(count: usize) -> (World, OffsetDateTime) {
        let created = datetime!(2026-01-05 0:00 UTC);
        let age = match count {
            1 => Duration::ZERO,
            2 => Duration::hours(2),
            _ => Duration::days(40),
        };
        let now = created + age;
        let desktop = restless_desktop(0);
        // Tuned on the companions the original generator made for this seed.
        let mut world = World::new_original([64; 32], created, &desktop);
        world.tick(now, 0.05, &desktop);
        world.save.home.active_since_utc = None;
        world.save.home.last_disappeared_utc = Some(now);
        world.save.ritual.next_at_utc = now + Duration::days(1);
        for creature in &mut world.save.creatures {
            creature.state.arrival_delay_secs = 0.0;
            creature.state.drives = Drives::default();
            creature.personality.curiosity = 1.0;
            creature.personality.sociability = 1.0;
            creature.personality.playfulness = 1.0;
            creature.personality.cursor_interest = 1.0;
        }
        assert_eq!(world.save.creatures.len(), count);
        (world, now)
    }

    fn only_monitor() -> MonitorInfo {
        restless_desktop(0).monitors.remove(0)
    }

    fn live(world: &mut World, now: OffsetDateTime, steps: std::ops::Range<u64>) {
        for step in steps {
            world.tick(
                now + Duration::milliseconds(step as i64 * 50),
                0.05,
                &restless_desktop(step),
            );
            world.drain_events().for_each(drop);
        }
    }

    /// True of a colony that is still whole and standing somewhere real, whatever it has just been
    /// through. Every lifecycle check below demands this as well as its own rule.
    fn colony_is_whole(save: &SaveFile, expected: usize) -> bool {
        save.creatures.len() == expected
            && save
                .creatures
                .iter()
                .any(|creature| creature.role.is_adult())
            && save.creatures.iter().all(|creature| {
                creature.state.position.x.is_finite()
                    && creature.state.position.y.is_finite()
                    && creature.state.surface.monitor_id == 1
            })
    }

    /// One row of the lifecycle matrix: a preference, what it keeps true from the moment it is
    /// chosen, and how long a colony is given to walk to wherever the choice leaves it. The first
    /// colony passed to the rule is how things stood the instant the preference was made.
    type PreferenceRule = (
        &'static str,
        fn(&mut Settings),
        fn(&SaveFile, &SaveFile) -> bool,
        u64,
    );

    const PREFERENCE_MATRIX: [PreferenceRule; 5] = [
        (
            "cursor reactions off",
            |settings| settings.cursor_reactions = false,
            |_, now| {
                now.creatures.iter().all(|creature| {
                    !matches!(
                        creature.state.action,
                        ActionKind::InvestigateCursor | ActionKind::AvoidCursor
                    )
                })
            },
            0,
        ),
        // Reduced motion is an alternative, not a stillness: the calm version of a reaction keeps
        // both feet on the surface rather than leaving anyone hanging by a hand.
        (
            "reduced motion",
            |settings| settings.reduce_motion = true,
            |_, now| {
                now.creatures.iter().all(|creature| {
                    creature.state.action != ActionKind::Dangle
                        && creature
                            .state
                            .attention
                            .is_none_or(|pose| pose.hanging == 0.0)
                })
            },
            0,
        ),
        ("paused", |settings| settings.paused = true, undisturbed, 0),
        // Hiding does not stop the colony living; it stops it watching. Out of sight, nobody is
        // gathering an audience or following the cursor.
        (
            "hidden",
            |settings| settings.visible = false,
            |_, now| {
                now.creatures.iter().all(|creature| {
                    creature.state.attention.is_none()
                        && !matches!(
                            creature.state.action,
                            ActionKind::InvestigateCursor | ActionKind::AvoidCursor
                        )
                })
            },
            0,
        ),
        (
            "bottom corners only",
            |settings| settings.habitat.preset = HabitatPreset::BottomCorners,
            |_, now| {
                let monitor = only_monitor();
                now.creatures.iter().all(|creature| {
                    habitat_contains(&now.settings.habitat, &monitor, creature.state.position)
                })
            },
            400,
        ),
    ];

    /// A paused colony does not move, does not change what it is doing, and does not quietly
    /// accumulate anything while the desktop carries on without it.
    fn undisturbed(before: &SaveFile, now: &SaveFile) -> bool {
        before.companion.journal == now.companion.journal
            && before
                .creatures
                .iter()
                .zip(&now.creatures)
                .all(|(before, now)| {
                    before.state.position == now.state.position
                        && before.state.action == now.state.action
                        && before.state.surface == now.state.surface
                })
    }

    #[test]
    fn every_preference_in_the_matrix_holds_for_one_two_and_four_companions() {
        for count in [1, 2, 4] {
            for (name, choose, holds, grace) in PREFERENCE_MATRIX {
                let (mut world, now) = colony_of(count);
                // Ordinary life first, so the preference has something to interrupt.
                live(&mut world, now, 1..60);
                choose(&mut world.save.settings);
                let chosen = world.save.clone();
                // Coming down off a ledge or back inside a habitat is a walk, not a jump.
                live(&mut world, now, 60..60 + grace);
                let mut living = false;
                for step in 60 + grace..860 {
                    let previous = world.save.creatures.clone();
                    live(&mut world, now, step..step + 1);
                    living |= world
                        .save
                        .creatures
                        .iter()
                        .zip(&previous)
                        .any(|(now, before)| {
                            now.state.action != before.state.action
                                || now.state.position != before.state.position
                                || now.state.action_elapsed != before.state.action_elapsed
                        });
                    assert!(
                        colony_is_whole(&world.save, count),
                        "{name} with {count}: the colony came apart at step {step}"
                    );
                    assert!(
                        world.save.creatures.iter().all(|creature| !matches!(
                            creature.state.action,
                            ActionKind::Dragged | ActionKind::Tossed
                        )),
                        "{name} with {count}: nobody touched them at step {step}"
                    );
                    assert!(
                        holds(&chosen, &world.save),
                        "{name} with {count}: broken at step {step}"
                    );
                }
                // Only a paused colony stands completely still. Every other row has to have been
                // getting on with its life while the rule above was being checked, or the rule was
                // being asked of nobody.
                assert_eq!(
                    living,
                    name != "paused",
                    "{name} with {count}: the colony was not living"
                );
            }
        }
    }

    /// One thing that happens to a colony, what has to be true once the colony has settled after
    /// it, and how many companions are left.
    type LifecycleEvent = (
        &'static str,
        fn(&mut World, OffsetDateTime, &DesktopSnapshot),
        fn(&World) -> bool,
        usize,
    );

    const LIFECYCLE_EVENTS: [LifecycleEvent; 6] = [
        (
            "carried over a window and put down, with and without ledges",
            |world, _, desktop| {
                let creature_id = world.save.creatures[0].id;
                // Let go directly above a window twice. Whether the ledge counts as somewhere to
                // put a companion down is the preference's to decide.
                for (ledges, expected) in [
                    (false, SurfaceKind::ScreenFloor),
                    (true, SurfaceKind::WindowLedge),
                ] {
                    world.save.settings.window_ledges = ledges;
                    let cursor = world.save.creatures[0].state.position;
                    let over_window = Point { x: 540.0, y: 560.0 };
                    world.handle_command(
                        WorldCommand::BeginInteraction {
                            creature_id,
                            cursor,
                        },
                        desktop,
                    );
                    world.handle_command(
                        WorldCommand::UpdateInteraction {
                            cursor: over_window,
                            velocity: Point::default(),
                        },
                        desktop,
                    );
                    world.handle_command(
                        WorldCommand::EndInteraction {
                            cursor: over_window,
                            velocity: Point::default(),
                        },
                        desktop,
                    );
                    assert_eq!(
                        world.save.creatures[0].state.surface.kind, expected,
                        "put down over a window with ledges {ledges}"
                    );
                }
            },
            |world| {
                habitat_contains(
                    &world.save.settings.habitat,
                    &only_monitor(),
                    world.save.creatures[0].state.position,
                )
            },
            4,
        ),
        (
            "thrown, caught in the air, and let go of",
            |world, _, desktop| {
                let creature_id = world.save.creatures[0].id;
                let safe = world.save.creatures[0].state.position;
                world.handle_command(
                    WorldCommand::BeginInteraction {
                        creature_id,
                        cursor: safe,
                    },
                    desktop,
                );
                world.handle_command(
                    WorldCommand::UpdateInteraction {
                        cursor: Point { x: 700.0, y: 200.0 },
                        velocity: Point {
                            x: 900.0,
                            y: -400.0,
                        },
                    },
                    desktop,
                );
                world.handle_command(
                    WorldCommand::EndInteraction {
                        cursor: Point { x: 700.0, y: 200.0 },
                        velocity: Point {
                            x: 900.0,
                            y: -400.0,
                        },
                    },
                    desktop,
                );
                assert_eq!(world.save.creatures[0].state.action, ActionKind::Tossed);
                // Caught again while still in the air, then let go of without a new landing.
                world.handle_command(
                    WorldCommand::BeginInteraction {
                        creature_id,
                        cursor: world.save.creatures[0].state.position,
                    },
                    desktop,
                );
                world.handle_command(WorldCommand::CancelInteraction, desktop);
                assert_eq!(world.save.creatures[0].state.position, safe);
            },
            |world| {
                !world.is_interacting()
                    && !matches!(
                        world.save.creatures[0].state.action,
                        ActionKind::Dragged | ActionKind::Tossed
                    )
            },
            4,
        ),
        (
            "a companion is taken away",
            |world, _, _| {
                let departing = world.save.creatures[3].id;
                world.remove_colony_creature(departing).unwrap();
                assert!(
                    world
                        .save
                        .relationships
                        .iter()
                        .all(|bond| !bond.contains(departing))
                );
            },
            // Whoever is left still belongs to somebody who is also still here.
            |world| {
                world.save.creatures.iter().all(|creature| {
                    creature.role.parent_id().is_none_or(|parent| {
                        world.save.creatures.iter().any(|other| other.id == parent)
                    })
                })
            },
            3,
        ),
        (
            "gathered back into the habitat",
            |world, _, desktop| {
                world.handle_command(WorldCommand::GatherCreatures, desktop);
            },
            |world| {
                let monitor = only_monitor();
                world.save.creatures.iter().all(|creature| {
                    habitat_contains(
                        &world.save.settings.habitat,
                        &monitor,
                        creature.state.position,
                    )
                })
            },
            4,
        ),
        (
            "the house appears and everybody goes home",
            |world, now, _| {
                world.save.home.active_since_utc = Some(now);
                world.save.home.last_disappeared_utc = None;
            },
            |world| {
                // Everyone has come home: at its own place or door, strolling the commons, or
                // partway through one of the small things residents do while the home is out.
                world.save.home.is_active()
                    && world
                        .save
                        .creatures
                        .iter()
                        .all(|creature| world.come_home(creature.id))
            },
            4,
        ),
        (
            "quiet time",
            |world, now, _| {
                world.set_quiet_mode(30, now);
                assert!(world.save.companion.quiet_until.is_some());
            },
            |world| {
                world.save.home.is_active()
                    && world.save.creatures.iter().all(|creature| {
                        !matches!(
                            creature.state.action,
                            ActionKind::Sprint | ActionKind::SocialPlay
                        )
                    })
            },
            4,
        ),
    ];

    #[test]
    fn every_turn_a_colonys_life_can_take_leaves_it_whole_and_standing_somewhere() {
        for (name, happen, holds, remaining) in LIFECYCLE_EVENTS {
            let (mut world, now) = colony_of(4);
            live(&mut world, now, 1..120);
            happen(
                &mut world,
                now + Duration::milliseconds(120 * 50),
                &restless_desktop(120),
            );
            live(&mut world, now, 120..1_200);
            assert!(
                colony_is_whole(&world.save, remaining),
                "{name}: the colony did not come through it whole"
            );
            assert!(holds(&world), "{name}: the colony did not settle");
            assert!(
                world.save.creatures.iter().all(|creature| !matches!(
                    creature.state.action,
                    ActionKind::Dragged | ActionKind::Tossed
                )),
                "{name}: somebody was left in the air"
            );
        }
    }

    #[test]
    fn a_long_run_of_events_leaves_the_journal_and_every_keepsake_at_its_cap() {
        let now = datetime!(2026-03-01 8:00 UTC);
        let mut state = CompanionState::default();
        // A colony's worth of milestones, arriving faster than any colony could produce them.
        for hour in 0..4_000i64 {
            let at = now + Duration::hours(hour);
            for creature_id in 1..5u64 {
                state.record(
                    &WorldEvent::ActionCompleted {
                        creature_id,
                        action: ActionKind::PresentDiscovery,
                    },
                    at,
                );
                state.record(&WorldEvent::CreatureSpawned { creature_id }, at);
                state.record(
                    &WorldEvent::RitualCompleted {
                        kind: RitualKind::Picnic,
                    },
                    at,
                );
            }
            // Every entry is offered for keeping, and every trinket is found again and again.
            let recent: Vec<_> = state.journal.iter().rev().take(4).cloned().collect();
            for entry in recent {
                state.pin(&entry);
            }
            for variant in 0..TRINKET_VARIANTS {
                state.remember_discovery(variant, 1, format!("Finder {hour}"), at);
            }
            assert!(state.journal.len() <= MAX_JOURNAL_ENTRIES, "hour {hour}");
            assert!(state.pins.len() <= MAX_PINNED_ENTRIES, "hour {hour}");
            assert!(state.scrapbook.len() <= usize::from(TRINKET_VARIANTS));
        }
        assert_eq!(state.journal.len(), MAX_JOURNAL_ENTRIES);
        assert_eq!(state.pins.len(), MAX_PINNED_ENTRIES);
        assert_eq!(state.scrapbook.len(), usize::from(TRINKET_VARIANTS));
        // The first finder is still the one credited, four thousand hours later.
        assert!(
            state
                .scrapbook
                .iter()
                .all(|record| record.finder_name == "Finder 0")
        );
        // Nothing grew: the file is the same size it was after the first few days. A full
        // scrapbook of 160 finds is most of it.
        let bytes = serde_json::to_string(&state).unwrap().len();
        eprintln!("journal and keepsakes after 4,000 hours: {bytes} bytes");
        assert!(bytes < 32 * 1024, "keepsakes grew to {bytes} bytes");
        state.normalize();
        assert_eq!(state.journal.len(), MAX_JOURNAL_ENTRIES);
        assert_eq!(state.pins.len(), MAX_PINNED_ENTRIES);
    }

    #[test]
    fn saved_routines_change_only_behavior_preferences() {
        let mut settings = Settings {
            launch_at_login: true,
            visible: false,
            display_scale: 4,
            ..Default::default()
        };
        let mut work = settings.clone();
        work.cursor_reactions = false;
        work.habitat.preset = HabitatPreset::BottomCorners;
        BehaviorPreset::capture(&work).apply(&mut settings);
        assert!(!settings.cursor_reactions);
        assert_eq!(settings.habitat.preset, HabitatPreset::BottomCorners);
        assert!(settings.launch_at_login);
        assert!(!settings.visible);
        assert_eq!(settings.display_scale, 4);
    }
}
