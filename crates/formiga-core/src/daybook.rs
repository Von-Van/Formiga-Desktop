//! The day book: a few counts for each of the last week's local days, so the notebook can say how
//! today compares — more time at home than yesterday, two companions who sought each other out
//! twice, a first sit up on a roof this week.
//!
//! Like the tallies, it keeps counts of things that happened and nothing else: no places, no times
//! within a day, nothing read off the desktop. It remembers the first day it counted, and nothing
//! is ever said about a day before that: a colony upgraded today has no "yesterday" to compare
//! with until tomorrow.
//!
//! [`day_notes`] reads the book, and the garden as it grows, and says what today has that is worth
//! a line. It reads the save and nothing else, and below its thresholds there is nothing to say.

use crate::tuning::TODAY_NOTES;
use crate::{
    CreatureId, GardenKind, GardenStage, MAX_COLONY_CREATURES, MAX_RELATIONSHIPS, SaveFile,
    SharedMomentKind, canonical_creature_pair,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// How many local days the book keeps: today and the week before it.
pub const DAYS_KEPT: usize = 8;

/// The last week's days, counted.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayBook {
    /// The first local day anything was counted, as a Julian day number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<i32>,
    /// The days kept, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub days: Vec<DayRecord>,
}

/// One local day's counts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayRecord {
    /// The local day, as a Julian day number.
    pub day: i32,
    /// Seconds the houses were out, while Formiga was running.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub home_seconds: u32,
    /// Who sat up on the roof of their house, each once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roof: Vec<CreatureId>,
    /// What pairs did together, one record per pair that did anything.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pairs: Vec<DayPair>,
}

/// What one pair did together on one day.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayPair {
    pub a: CreatureId,
    pub b: CreatureId,
    /// How often each went looking for the other, as `[a sought b, b sought a]`.
    #[serde(default)]
    pub sought: [u8; 2],
    #[serde(default)]
    pub rests: u8,
    #[serde(default)]
    pub plays: u8,
    #[serde(default)]
    pub gifts: u8,
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

impl DayBook {
    pub fn is_empty(&self) -> bool {
        self.since.is_none() && self.days.is_empty()
    }

    /// The record for `day`, if it was counted.
    pub fn on(&self, day: i32) -> Option<&DayRecord> {
        self.days.iter().find(|record| record.day == day)
    }

    /// The record for `day`, begun if need be. Only the newest week is kept, so a day older than
    /// all of it — a clock set far back — is not counted at all.
    pub fn day_mut(&mut self, day: i32) -> Option<&mut DayRecord> {
        if self.days.len() >= DAYS_KEPT && self.days.first().is_some_and(|first| day < first.day) {
            return None;
        }
        self.since = Some(self.since.map_or(day, |since| since.min(day)));
        let index = match self.days.binary_search_by_key(&day, |record| record.day) {
            Ok(index) => index,
            Err(index) => {
                self.days.insert(
                    index,
                    DayRecord {
                        day,
                        ..DayRecord::default()
                    },
                );
                index
            }
        };
        if self.days.len() > DAYS_KEPT {
            self.days.remove(0);
            return self.days.get_mut(index - 1);
        }
        self.days.get_mut(index)
    }

    /// Count something two companions did together on `day`. `a_began` says which of them went
    /// looking for the other, where one did.
    pub fn count_pair(
        &mut self,
        day: i32,
        a: CreatureId,
        b: CreatureId,
        a_began: Option<bool>,
        kind: SharedMomentKind,
        sought_out: bool,
    ) {
        let Some((first, second)) = canonical_creature_pair(a, b) else {
            return;
        };
        let Some(record) = self.day_mut(day) else {
            return;
        };
        let index = match record
            .pairs
            .iter()
            .position(|pair| pair.a == first && pair.b == second)
        {
            Some(index) => index,
            None if record.pairs.len() < MAX_RELATIONSHIPS => {
                let index = record
                    .pairs
                    .partition_point(|pair| (pair.a, pair.b) < (first, second));
                record.pairs.insert(
                    index,
                    DayPair {
                        a: first,
                        b: second,
                        ..DayPair::default()
                    },
                );
                index
            }
            None => return,
        };
        let pair = &mut record.pairs[index];
        let field = match kind {
            SharedMomentKind::Rest => Some(&mut pair.rests),
            SharedMomentKind::Play => Some(&mut pair.plays),
            SharedMomentKind::Gift => Some(&mut pair.gifts),
            _ => None,
        };
        if let Some(field) = field {
            *field = field.saturating_add(1);
        }
        if sought_out && let Some(a_began) = a_began {
            // `a_began` is about the pair as it was named; the record keeps it in canonical order.
            let side = usize::from(a_began != (a == first));
            pair.sought[side] = pair.sought[side].saturating_add(1);
        }
    }

    /// Count a sit up on a roof on `day`.
    pub fn count_roof(&mut self, day: i32, creature: CreatureId) {
        let Some(record) = self.day_mut(day) else {
            return;
        };
        if !record.roof.contains(&creature) && record.roof.len() < MAX_COLONY_CREATURES {
            record.roof.push(creature);
        }
    }

    /// Count seconds the houses were out on `day`.
    pub fn count_home(&mut self, day: i32, seconds: u32) {
        if let Some(record) = self.day_mut(day) {
            record.home_seconds = record.home_seconds.saturating_add(seconds).min(86_400);
        }
    }

    /// Put back what an earlier copy of the book counted for `returning` companions, where
    /// everyone it names is `present`: a companion brought back by undo comes back with its days.
    pub fn restore(&mut self, earlier: &DayBook, returning: &[CreatureId], present: &[CreatureId]) {
        for record in &earlier.days {
            let roof: Vec<CreatureId> = record
                .roof
                .iter()
                .copied()
                .filter(|id| returning.contains(id))
                .collect();
            let pairs: Vec<DayPair> = record
                .pairs
                .iter()
                .copied()
                .filter(|pair| returning.contains(&pair.a) || returning.contains(&pair.b))
                .filter(|pair| present.contains(&pair.a) && present.contains(&pair.b))
                .collect();
            if roof.is_empty() && pairs.is_empty() {
                continue;
            }
            let Some(now) = self.day_mut(record.day) else {
                continue;
            };
            for id in roof {
                if !now.roof.contains(&id) {
                    now.roof.push(id);
                }
            }
            for pair in pairs {
                if !now.pairs.iter().any(|p| p.a == pair.a && p.b == pair.b) {
                    now.pairs.push(pair);
                }
            }
            now.pairs.sort_by_key(|pair| (pair.a, pair.b));
        }
    }

    /// Whether the book was counting for the whole of the week before `day`.
    pub fn kept_week_before(&self, day: i32) -> bool {
        self.since.is_some_and(|since| since <= day - 6)
    }

    /// Whether the book was counting on `day`.
    pub fn counted(&self, day: i32) -> bool {
        self.since.is_some_and(|since| since <= day)
    }

    /// Bring the book inside its limits, and let go of anyone who no longer lives here.
    pub fn normalize(&mut self, residents: &[CreatureId]) {
        self.days.sort_by_key(|record| record.day);
        self.days.dedup_by_key(|record| record.day);
        let excess = self.days.len().saturating_sub(DAYS_KEPT);
        self.days.drain(..excess);
        if let (Some(since), Some(first)) = (self.since, self.days.first()) {
            self.since = Some(since.min(first.day));
        } else if self.since.is_none() {
            self.since = self.days.first().map(|record| record.day);
        }
        for record in &mut self.days {
            record.home_seconds = record.home_seconds.min(86_400);
            let mut seen = Vec::new();
            record.roof.retain(|id| {
                let keep = residents.contains(id) && !seen.contains(id);
                seen.push(*id);
                keep
            });
            let mut pairs = Vec::new();
            record.pairs.retain(|pair| {
                let keep = canonical_creature_pair(pair.a, pair.b) == Some((pair.a, pair.b))
                    && residents.contains(&pair.a)
                    && residents.contains(&pair.b)
                    && !pairs.contains(&(pair.a, pair.b));
                pairs.push((pair.a, pair.b));
                keep
            });
            record.pairs.sort_by_key(|pair| (pair.a, pair.b));
            record.pairs.truncate(MAX_RELATIONSHIPS);
        }
    }

    /// What the book holds that a validated colony never does, each said in a line.
    pub fn violations(&self, residents: &[CreatureId]) -> Vec<String> {
        let mut found = Vec::new();
        if self.days.len() > DAYS_KEPT {
            found.push(format!("{} days in the day book", self.days.len()));
        }
        if !self.days.windows(2).all(|pair| pair[0].day < pair[1].day) {
            found.push("the day book's days are not in order".to_owned());
        }
        if let (Some(since), Some(first)) = (self.since, self.days.first())
            && since > first.day
        {
            found.push("the day book counted a day before it began".to_owned());
        }
        for record in &self.days {
            if record.roof.iter().any(|id| !residents.contains(id))
                || record
                    .pairs
                    .iter()
                    .any(|pair| !residents.contains(&pair.a) || !residents.contains(&pair.b))
            {
                found.push(format!(
                    "day {} counts somebody who is not here",
                    record.day
                ));
            }
            if record.pairs.len() > MAX_RELATIONSHIPS || record.home_seconds > 86_400 {
                found.push(format!(
                    "day {} counts more than a day can hold",
                    record.day
                ));
            }
        }
        found
    }
}

/// Something today has that is worth a line on the Today page, with the counts it rests on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DayNote {
    /// Each went looking for the other today.
    SoughtEachOther {
        a: CreatureId,
        b: CreatureId,
        /// How often each went looking for the other, as `[a sought b, b sought a]`.
        sought: [u8; 2],
    },
    /// One kept going looking for the other today.
    WentLookingFor {
        seeker: CreatureId,
        sought: CreatureId,
        times: u8,
    },
    /// Up on its roof today, and not once in the six days before.
    FirstRoofSitThisWeek { creature: CreatureId },
    /// A garden patch came round to a stage today.
    GardenStage {
        kind: GardenKind,
        stage: GardenStage,
    },
    /// The houses were out longer today than on the whole of yesterday.
    MoreTimeAtHome { today: u32, yesterday: u32 },
    /// More play between companions today than on the whole of yesterday.
    MorePlay { today: u16, yesterday: u16 },
}

impl DayNote {
    /// The companions the note is about.
    pub fn subjects(&self) -> Vec<CreatureId> {
        match *self {
            Self::SoughtEachOther { a, b, .. } => vec![a, b],
            Self::WentLookingFor { seeker, sought, .. } => vec![seeker, sought],
            Self::FirstRoofSitThisWeek { creature } => vec![creature],
            Self::GardenStage { .. } | Self::MoreTimeAtHome { .. } | Self::MorePlay { .. } => {
                Vec::new()
            }
        }
    }
}

/// What today has worth a line, most personal first, at most `TODAY_NOTES.most` of them. `local`
/// is now in the reader's own time zone, which decides where today begins.
pub fn day_notes(save: &SaveFile, local: OffsetDateTime) -> Vec<DayNote> {
    let book = &save.day_book;
    let day = local.date().to_julian_day();
    let today = book.on(day);
    let yesterday = book.counted(day - 1).then(|| book.on(day - 1));
    let mut notes = Vec::new();

    // The pair who sought each other out the most today, or one who kept looking for another.
    if let Some(record) = today {
        let mutual = record
            .pairs
            .iter()
            .filter(|pair| pair.sought[0] > 0 && pair.sought[1] > 0)
            .map(|pair| (u16::from(pair.sought[0]) + u16::from(pair.sought[1]), pair))
            .filter(|(times, _)| *times >= TODAY_NOTES.sought_each_other)
            .max_by_key(|(times, pair)| (*times, std::cmp::Reverse((pair.a, pair.b))));
        if let Some((_, pair)) = mutual {
            notes.push(DayNote::SoughtEachOther {
                a: pair.a,
                b: pair.b,
                sought: pair.sought,
            });
        } else if let Some((times, seeker, sought)) = record
            .pairs
            .iter()
            .flat_map(|pair| {
                [
                    (pair.sought[0], pair.a, pair.b),
                    (pair.sought[1], pair.b, pair.a),
                ]
            })
            .filter(|(times, ..)| *times >= TODAY_NOTES.went_looking)
            .max_by_key(|(times, seeker, sought)| (*times, std::cmp::Reverse((*seeker, *sought))))
        {
            notes.push(DayNote::WentLookingFor {
                seeker,
                sought,
                times,
            });
        }
    }

    // A first sit up on a roof this week, only where the whole week was counted.
    if let Some(record) = today
        && book.kept_week_before(day)
        && let Some(creature) = record.roof.iter().copied().find(|creature| {
            !(day - 6..day).any(|earlier| {
                book.on(earlier)
                    .is_some_and(|earlier| earlier.roof.contains(creature))
            })
        })
    {
        notes.push(DayNote::FirstRoofSitThisWeek { creature });
    }

    // The garden patch that most recently came round to its third or fourth stage today.
    let midnight = local.replace_time(time::Time::MIDNIGHT);
    let utc = local.to_offset(time::UtcOffset::UTC);
    if let Some((kind, stage, _)) = save
        .home
        .gardens
        .iter()
        .filter_map(|patch| {
            let stage = patch.stage(utc);
            let began = stage_began(patch, utc);
            (stage >= GardenStage::Grown && began >= midnight).then_some((patch.kind, stage, began))
        })
        .max_by_key(|(kind, _, began)| (*began, kind.index()))
    {
        notes.push(DayNote::GardenStage { kind, stage });
    }

    // Longer at home than all of yesterday, once it is long enough to mention.
    if let (Some(record), Some(before)) = (today, yesterday) {
        let before = before.map_or(0, |record| record.home_seconds);
        if record.home_seconds >= TODAY_NOTES.home_seconds && record.home_seconds > before {
            notes.push(DayNote::MoreTimeAtHome {
                today: record.home_seconds,
                yesterday: before,
            });
        }
    }

    // More play than all of yesterday.
    if let (Some(record), Some(before)) = (today, yesterday) {
        let plays = |record: &DayRecord| -> u16 {
            record.pairs.iter().map(|pair| u16::from(pair.plays)).sum()
        };
        let (today, yesterday) = (plays(record), before.map_or(0, plays));
        if today >= TODAY_NOTES.plays && today > yesterday {
            notes.push(DayNote::MorePlay { today, yesterday });
        }
    }

    notes.retain(|note| {
        note.subjects()
            .iter()
            .all(|id| save.creatures.iter().any(|creature| creature.id == *id))
    });
    notes.truncate(TODAY_NOTES.most);
    notes
}

/// When a patch came round to the stage it is at `now`.
fn stage_began(patch: &crate::GardenPatch, now: OffsetDateTime) -> OffsetDateTime {
    let step = patch.kind.stage_hours();
    match patch.planted_at_utc {
        Some(planted) => {
            let hours = (now - planted).whole_hours().max(0);
            planted + time::Duration::hours(hours - hours.rem_euclid(step))
        }
        None => {
            // An old patch keeps a clock of its own: see `GardenPatch::stage`.
            let offset = i64::from(patch.kind.index()) * 7;
            let hours = now.unix_timestamp() / 3600 + offset;
            let began = hours - hours.rem_euclid(step) - offset;
            OffsetDateTime::from_unix_timestamp(began * 3600).unwrap_or(now)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn a_day_is_kept_in_order_and_a_week_is_all_that_is_kept() {
        let mut book = DayBook::default();
        for day in [10, 12, 11, 20, 13, 14, 15, 16, 17, 18, 19] {
            book.count_home(day, 60);
        }
        assert_eq!(book.days.len(), DAYS_KEPT);
        assert!(book.days.windows(2).all(|pair| pair[0].day < pair[1].day));
        assert_eq!(book.days.first().unwrap().day, 13);
        assert_eq!(book.since, Some(10));
        // A clock set back past the week kept counts nothing, and unsettles nothing.
        let before = book.clone();
        book.count_roof(9, 1);
        assert_eq!(book.days, before.days);
        assert!(book.violations(&[1]).is_empty());
    }

    #[test]
    fn who_sought_whom_is_kept_with_the_right_one_of_the_pair() {
        let mut book = DayBook::default();
        // Named the other way round from the pair's own order: 9 went looking for 4.
        book.count_pair(5, 9, 4, Some(true), SharedMomentKind::Greeting, true);
        book.count_pair(5, 4, 9, Some(true), SharedMomentKind::Greeting, true);
        book.count_pair(5, 4, 9, Some(true), SharedMomentKind::Play, false);
        let pair = book.on(5).unwrap().pairs[0];
        assert_eq!((pair.a, pair.b), (4, 9));
        assert_eq!(pair.sought, [1, 1]);
        assert_eq!(pair.plays, 1);
    }

    /// A colony of two, so there is a pair to count.
    fn colony() -> SaveFile {
        let desktop = crate::DesktopSnapshot::default();
        let now = datetime!(2026-10-01 9:00 UTC);
        let mut world = crate::World::new([31; 32], now, &desktop);
        world
            .add_designed_adult([32; 32], None, now, &desktop)
            .unwrap();
        world.save
    }

    #[test]
    fn nothing_is_compared_with_a_day_that_was_not_counted() {
        let mut save = colony();
        let local = datetime!(2026-10-02 18:00 UTC);
        let day = local.date().to_julian_day();
        save.day_book.count_home(day, 3 * 3600);
        for _ in 0..5 {
            let (a, b) = (save.creatures[0].id, save.creatures[1].id);
            save.day_book
                .count_pair(day, a, b, Some(true), SharedMomentKind::Play, false);
        }
        save.day_book.count_roof(day, save.creatures[0].id);
        save.home.gardens.clear();
        // Counting began today: no yesterday to beat, no week to be first in.
        assert_eq!(day_notes(&save, local), Vec::new());
        // With yesterday counted, today's afternoon at home and its games are worth a line.
        save.day_book.count_home(day - 1, 600);
        let notes = day_notes(&save, local);
        assert!(notes.contains(&DayNote::MoreTimeAtHome {
            today: 3 * 3600,
            yesterday: 600
        }));
        assert!(notes.contains(&DayNote::MorePlay {
            today: 5,
            yesterday: 0
        }));
        assert!(
            !notes
                .iter()
                .any(|note| matches!(note, DayNote::FirstRoofSitThisWeek { .. })),
            "a week that was not all counted has no first in it"
        );
        save.day_book.count_home(day - 6, 60);
        assert!(
            day_notes(&save, local).contains(&DayNote::FirstRoofSitThisWeek {
                creature: save.creatures[0].id
            })
        );
        save.day_book.count_roof(day - 3, save.creatures[0].id);
        assert!(
            !day_notes(&save, local)
                .iter()
                .any(|note| matches!(note, DayNote::FirstRoofSitThisWeek { .. }))
        );
    }

    #[test]
    fn two_who_sought_each_other_out_are_noted_before_one_who_only_followed() {
        let mut save = colony();
        let local = datetime!(2026-10-02 18:00 UTC);
        let day = local.date().to_julian_day();
        let (a, b) = (save.creatures[0].id, save.creatures[1].id);
        save.day_book
            .count_pair(day, a, b, Some(true), SharedMomentKind::Greeting, true);
        save.day_book
            .count_pair(day, a, b, Some(true), SharedMomentKind::Greeting, true);
        save.home.gardens.clear();
        assert_eq!(
            day_notes(&save, local),
            vec![DayNote::WentLookingFor {
                seeker: a,
                sought: b,
                times: 2
            }]
        );
        save.day_book
            .count_pair(day, b, a, Some(true), SharedMomentKind::Greeting, true);
        let (first, second) = canonical_creature_pair(a, b).unwrap();
        assert_eq!(
            day_notes(&save, local),
            vec![DayNote::SoughtEachOther {
                a: first,
                b: second,
                sought: if first == a { [2, 1] } else { [1, 2] },
            }]
        );
    }

    #[test]
    fn a_garden_that_came_into_its_third_stage_today_is_noted_once() {
        let mut save = colony();
        let local = datetime!(2026-10-02 18:00 UTC);
        save.home.gardens = vec![crate::GardenPatch {
            kind: GardenKind::Herbs,
            along: 0.5,
            // Herbs take five hours a stage: planted at 08:00, grown from 18:00.
            planted_at_utc: Some(datetime!(2026-10-02 8:00 UTC)),
        }];
        assert_eq!(
            day_notes(&save, local),
            vec![DayNote::GardenStage {
                kind: GardenKind::Herbs,
                stage: GardenStage::Grown
            }]
        );
        // Planted the day before and grown by midnight: nothing came round today.
        save.home.gardens[0].planted_at_utc = Some(datetime!(2026-10-01 8:00 UTC));
        let notes = day_notes(&save, datetime!(2026-10-02 1:00 UTC));
        assert!(notes.is_empty(), "{notes:?}");
    }
}
