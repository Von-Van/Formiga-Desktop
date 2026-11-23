//! What a companion's page writes about it, in the researcher's loose first person, from what the
//! colony recorded and from nothing else: the latest thing seen, dated, first; then what has been
//! noticed about it; who it gets on with, as faces with a word under each; its firsts; and its
//! little ways. Every sentence is a template filled from a record, so each is tested.

use super::*;
use formiga_core::{Observation, PairMemory, SharedMomentKind};

/// A friend on a companion's page: who, and the one word for how they are together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Friend {
    pub(crate) id: CreatureId,
    pub(crate) word: &'static str,
}

/// Everything the page writes about one companion, in the order it is written.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Story {
    /// The newest thing seen of it, dated: a moment shared with a friend or a moment the journal
    /// wrote down about it.
    pub(crate) lately: Option<String>,
    /// What the notebook has noticed about it, at most two sentences' worth.
    pub(crate) noticed: Vec<String>,
    /// Who it gets on with, closest first, at most four.
    pub(crate) friends: Vec<Friend>,
    /// When it joined, first grew close to someone, and first found something; empty for a
    /// companion that joined today and has done neither yet.
    pub(crate) firsts: String,
    /// How it celebrates and the habits it has picked up, as sentences.
    pub(crate) ways: String,
}

impl Story {
    /// Whether there is anything to say yet beyond who it is.
    pub(crate) fn is_sparse(&self) -> bool {
        self.lately.is_none() && self.noticed.is_empty() && self.friends.is_empty()
    }
}

fn name_of(save: &SaveFile, id: CreatureId) -> Option<&str> {
    save.creatures
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.name.as_str())
}

/// A find by name, as it reads in a sentence: "a key", "an acorn", "matching charms".
pub(crate) fn find_name(variant: u8) -> Option<String> {
    let name = formiga_core::trinket_info(variant)?.name.to_lowercase();
    let plural = name.ends_with('s') && !name.ends_with("ss");
    Some(if plural {
        name
    } else if name.starts_with(['a', 'e', 'i', 'o', 'u']) {
        format!("an {name}")
    } else {
        format!("a {name}")
    })
}

/// The find a journal's "found a little treasure" was, where the scrapbook kept it as a first
/// find by the same companion within a minute of the moment.
pub(crate) fn find_of(save: &SaveFile, entry: &JournalEntry) -> Option<String> {
    if entry.moment != JournalMoment::Discovery {
        return None;
    }
    let finder = entry.creature?;
    save.companion
        .scrapbook
        .iter()
        .filter(|record| record.finder == Some(finder))
        .find(|record| (record.first_at - entry.at).abs() <= time::Duration::minutes(1))
        .and_then(|record| find_name(record.variant))
}

/// When something happened, as a sentence ends with it: "this morning", "yesterday", "on
/// Tuesday", "on 14 Sep".
pub(crate) fn when(at: OffsetDateTime, now: OffsetDateTime, offset: time::UtcOffset) -> String {
    let (local, today) = (at.to_offset(offset), now.to_offset(offset).date());
    match (today - local.date()).whole_days() {
        days if days <= 0 => match local.hour() {
            0..=11 => "this morning".into(),
            12..=17 => "this afternoon".into(),
            _ => "this evening".into(),
        },
        1 => "yesterday".into(),
        days if days < 7 => format!("on {}", local.weekday()),
        _ => format!("on {}", dates::on(at, offset)),
    }
}

/// The last warm thing two shared, said from one side: "Napped beside Tofu this morning, the
/// ninth time I've caught them at it."
fn shared_line(
    friend: &str,
    memory: PairMemory,
    tally: &formiga_core::RelationshipTally,
    now: OffsetDateTime,
    offset: time::UtcOffset,
) -> String {
    let when = when(memory.at, now, offset);
    let (said, count) = match memory.kind {
        SharedMomentKind::Rest => (format!("Napped beside {friend}"), tally.shared_rests),
        SharedMomentKind::Play => (format!("Played with {friend}"), tally.plays),
        SharedMomentKind::Gift => (format!("Brought {friend} a find"), tally.gifts),
        SharedMomentKind::Greeting => (format!("Said hello to {friend}"), tally.greetings),
        SharedMomentKind::Calm => (format!("Spent a quiet while with {friend}"), 0),
        SharedMomentKind::Squabble => (format!("Had a squabble with {friend}"), tally.squabbles),
    };
    if count >= 2 {
        format!(
            "{said} {when}, the {} time I've seen it.",
            ordinal(u32::from(count))
        )
    } else {
        format!("{said} {when}.")
    }
}

/// A moment the journal wrote down about a companion, said of it: "Picked up a little habit
/// yesterday: waves hello." `None` for a moment that is not about it alone.
fn moment_line(
    save: &SaveFile,
    entry: &JournalEntry,
    now: OffsetDateTime,
    offset: time::UtcOffset,
) -> Option<String> {
    let when = when(entry.at, now, offset);
    Some(match &entry.moment {
        JournalMoment::Friendship(friend) => {
            format!("Grew close to {} {when}.", name_of(save, *friend)?)
        }
        JournalMoment::Discovery => match find_of(save, entry) {
            Some(find) => format!("Found {find} {when}, a first for the colony."),
            None => format!("Found a little treasure {when}."),
        },
        JournalMoment::Habit(habit) => format!(
            "Picked up a little habit {when}: {}.",
            habit.label().to_lowercase()
        ),
        JournalMoment::Preference(descriptor) => format!(
            "Started to be known {when} for this: {}.",
            descriptor.label().to_lowercase()
        ),
        JournalMoment::Wonder(kind) => format!(
            "Found a wonder {when}: the {}.",
            kind.label().to_lowercase()
        ),
        JournalMoment::NewLook => format!("Came back from Formiga Farm with a new look {when}."),
        JournalMoment::HouseVisit => format!("You spent a while inside its house {when}."),
        _ => return None,
    })
}

/// The newest thing seen of a companion, dated.
fn lately(
    save: &SaveFile,
    creature: CreatureId,
    now: OffsetDateTime,
    offset: time::UtcOffset,
) -> Option<String> {
    let shared = save
        .tallies
        .iter()
        .filter(|pair| pair.a == creature || pair.b == creature)
        .filter_map(|pair| {
            let other = if pair.a == creature { pair.b } else { pair.a };
            let memory = pair.tally.memory?;
            let friend = name_of(save, other)?;
            Some((
                memory.at,
                shared_line(friend, memory, &pair.tally, now, offset),
            ))
        })
        .max_by_key(|(at, _)| *at);
    let written = save
        .companion
        .journal
        .iter()
        .rev()
        .filter(|entry| {
            entry.creature == Some(creature) || entry.moment == JournalMoment::Friendship(creature)
        })
        .find_map(|entry| {
            let mut entry = entry.clone();
            // A friendship is written from the side of whoever grew close first; on the other's
            // page it is the same moment, said of the other.
            if let (JournalMoment::Friendship(friend), Some(other)) =
                (&entry.moment, entry.creature)
                && *friend == creature
            {
                entry.moment = JournalMoment::Friendship(other);
                entry.creature = Some(creature);
            }
            Some((entry.at, moment_line(save, &entry, now, offset)?))
        });
    [shared, written]
        .into_iter()
        .flatten()
        .max_by_key(|(at, _)| *at)
        .filter(|(at, _)| now - *at <= time::Duration::days(14))
        .map(|(_, line)| line)
}

/// What has been noticed about a companion, as the Today page notices it: the first observation
/// with its evidence, the second as its claim alone. Where nothing says where it keeps to, where
/// it has been seen to like best.
fn noticed(
    save: &SaveFile,
    monitors: &[MonitorInfo],
    creature: &Creature,
    learned: &[&str],
) -> Vec<String> {
    let observations = formiga_core::observations_of(save, creature.id);
    let mut noticed = Vec::new();
    for (index, observation) in observations.iter().take(2).enumerate() {
        let (claim, evidence) = today::observation_text(save, monitors, observation);
        noticed.push(if index == 0 {
            format!("{claim} {evidence}")
        } else {
            claim
        });
    }
    let keeps_to_a_place = observations
        .iter()
        .any(|observation| matches!(observation, Observation::KeepsToAPlace { .. }));
    if noticed.len() < 2
        && !keeps_to_a_place
        && let Some(preferred) = creature.memory.preferred_region
    {
        noticed.push(format!(
            "Seems happiest at the {} of {}.",
            crate::settings::region_label(preferred.cell),
            crate::settings::display_label(preferred.display, monitors)
        ));
    }
    if noticed.len() < 2 && !learned.is_empty() {
        noticed.push(format!(
            "Becoming known for this: {}.",
            learned
                .iter()
                .map(|label| label.to_lowercase())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    noticed
}

/// The one word for how two get along: "cozy", "playmates", "squabbles".
pub(crate) fn friend_word(
    a: &Creature,
    b: &Creature,
    relationship: &formiga_core::CreatureRelationship,
) -> &'static str {
    use formiga_core::TemperamentKind as Kind;
    if relationship.avoidance >= 160 {
        return "keep apart";
    }
    if relationship.playfulness >= 72 {
        return "playmates";
    }
    let (a, b) = (a.temperament(), b.temperament());
    let feisty = |t: &formiga_core::Temperament| t.axes.feistiness >= 0.65;
    if feisty(&a) && feisty(&b) {
        "squabbles"
    } else if a.kind == Kind::Troublemaker && b.kind == Kind::Troublemaker {
        "mischief"
    } else if (a.kind == Kind::Grump || b.kind == Kind::Grump) && feisty(&a) != feisty(&b) {
        "odd couple"
    } else if relationship.affinity >= 112 {
        "cozy"
    } else if relationship.affinity >= 48 {
        "warming up"
    } else {
        "polite"
    }
}

/// Who a companion has spent time with, closest first, those keeping apart last, at most four.
fn friends(save: &SaveFile, creature: &Creature) -> Vec<Friend> {
    let mut known: Vec<(i16, Friend)> = save
        .relationships
        .iter()
        .filter(|relationship| relationship.familiarity > 0 || relationship.affinity > 0)
        .filter_map(|relationship| {
            let other = relationship.other(creature.id)?;
            let other = save.creatures.iter().find(|c| c.id == other)?;
            let word = friend_word(creature, other, relationship);
            let rank = if word == "keep apart" {
                i16::MIN
            } else {
                relationship.closeness()
            };
            Some((rank, Friend { id: other.id, word }))
        })
        .collect();
    known.sort_by_key(|(rank, friend)| (std::cmp::Reverse(*rank), friend.id));
    known
        .into_iter()
        .take(4)
        .map(|(_, friend)| friend)
        .collect()
}

/// When it joined, first grew close to someone, and first found something: "Joined 5 Aug · first
/// friend Tofu, 9 Sep · first find a key, 12 Sep".
fn firsts(
    save: &SaveFile,
    creature: &Creature,
    now: OffsetDateTime,
    offset: time::UtcOffset,
) -> String {
    let mut firsts = vec![format!(
        "Joined {}",
        dates::on(creature.born_at_utc, offset)
    )];
    let first_friend = save
        .companion
        .journal
        .iter()
        .filter_map(|entry| match entry.moment {
            JournalMoment::Friendship(friend) if entry.creature == Some(creature.id) => {
                Some((entry.at, friend))
            }
            JournalMoment::Friendship(friend) if friend == creature.id => {
                Some((entry.at, entry.creature?))
            }
            _ => None,
        })
        .min_by_key(|(at, _)| *at)
        .and_then(|(at, friend)| Some((at, name_of(save, friend)?)));
    if let Some((at, friend)) = first_friend {
        firsts.push(format!("first friend {friend}, {}", dates::on(at, offset)));
    }
    if let Some((at, find)) = save
        .companion
        .scrapbook
        .iter()
        .filter(|record| record.finder == Some(creature.id))
        .min_by_key(|record| record.first_at)
        .and_then(|record| Some((record.first_at, find_name(record.variant)?)))
    {
        firsts.push(format!("first find {find}, {}", dates::on(at, offset)));
    }
    // A companion that joined today says so beside its name already.
    if firsts.len() == 1
        && creature.born_at_utc.to_offset(offset).date() >= now.to_offset(offset).date()
    {
        return String::new();
    }
    firsts.join(" · ")
}

/// How it celebrates, then each habit it has picked up, as sentences.
pub(crate) fn ways(creature: &Creature) -> String {
    std::iter::once(formiga_core::Celebration::for_creature(creature).label())
        .chain(creature.memory.habits.iter().map(|habit| habit.label()))
        .map(|way| format!("{way}."))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Everything the page writes about `creature`, with `learned` the ways it has come to be known
/// for, by their labels.
pub(crate) fn story(
    save: &SaveFile,
    monitors: &[MonitorInfo],
    creature: &Creature,
    learned: &[&str],
    now: OffsetDateTime,
    offset: time::UtcOffset,
) -> Story {
    Story {
        lately: lately(save, creature.id, now, offset),
        noticed: noticed(save, monitors, creature, learned),
        friends: friends(save, creature),
        firsts: firsts(save, creature, now, offset),
        ways: ways(creature),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn a_moment_is_dated_the_way_a_note_would_date_it() {
        let now = datetime!(2026-09-14 18:00 UTC);
        let utc = time::UtcOffset::UTC;
        assert_eq!(
            when(datetime!(2026-09-14 08:00 UTC), now, utc),
            "this morning"
        );
        assert_eq!(
            when(datetime!(2026-09-14 13:00 UTC), now, utc),
            "this afternoon"
        );
        assert_eq!(when(datetime!(2026-09-13 23:00 UTC), now, utc), "yesterday");
        assert_eq!(
            when(datetime!(2026-09-10 23:00 UTC), now, utc),
            "on Thursday"
        );
        assert_eq!(when(datetime!(2026-08-30 23:00 UTC), now, utc), "on 30 Aug");
    }

    #[test]
    fn a_find_is_named_as_a_sentence_names_it() {
        let name = |wanted: &str| {
            (0..=u8::MAX)
                .find(|variant| {
                    formiga_core::trinket_info(*variant).is_some_and(|info| info.name == wanted)
                })
                .and_then(find_name)
        };
        assert_eq!(name("Key").as_deref(), Some("a key"));
        assert_eq!(name("Acorn").as_deref(), Some("an acorn"));
        assert_eq!(name("Matching charms").as_deref(), Some("matching charms"));
    }

    #[test]
    fn a_shared_moment_says_how_often_it_has_been_seen() {
        let now = datetime!(2026-09-14 18:00 UTC);
        let tally = formiga_core::RelationshipTally {
            shared_rests: 9,
            ..Default::default()
        };
        let memory = PairMemory {
            at: datetime!(2026-09-14 09:00 UTC),
            kind: SharedMomentKind::Rest,
        };
        assert_eq!(
            shared_line("Tofu", memory, &tally, now, time::UtcOffset::UTC),
            "Napped beside Tofu this morning, the ninth time I've seen it."
        );
        let memory = PairMemory {
            kind: SharedMomentKind::Play,
            ..memory
        };
        assert_eq!(
            shared_line("Tofu", memory, &tally, now, time::UtcOffset::UTC),
            "Played with Tofu this morning."
        );
    }
}
