//! The notebook's first page: the colony today, at a glance. What is new since it was last read,
//! who is doing what right now, how the week has gone, what the notebook has noticed, and the
//! last few things companions shared with one another. Everything on it is read from what the
//! colony has recorded; nothing here is a task, a score or a reminder.
use super::shell::Shell;
use super::*;
use crate::settings::SettingsTab;
use formiga_core::{Observation, SharedMomentKind};

/// What was unread when the reader turned to the Today or Journal page: kept while they stay on
/// those pages, so the news they came to read does not vanish the moment it is marked as read.
/// Never saved.
#[derive(Clone, Debug, Default)]
pub(crate) struct Reading {
    /// The noteworthy moments that were new, newest first.
    pub(crate) unread: Vec<JournalEntry>,
}

impl Reading {
    /// Whether a journal moment was new when the page was opened.
    pub(crate) fn is_new(&self, entry: &JournalEntry) -> bool {
        self.unread.iter().any(|unread| unread == entry)
    }
}

/// A span of seconds the way a field note writes it: "3 h 12 min", "45 min".
pub(crate) fn duration_text(seconds: u32) -> String {
    let minutes = seconds / 60;
    let (hours, minutes) = (minutes / 60, minutes % 60);
    match (hours, minutes) {
        (0, minutes) => format!("{} min", minutes.max(1)),
        (hours, 0) => format!("{hours} h"),
        (hours, minutes) => format!("{hours} h {minutes} min"),
    }
}

/// "a moment ago", "3 hours ago", "yesterday", "4 days ago", or the date.
pub(crate) fn ago_text(at: OffsetDateTime, now: OffsetDateTime, offset: time::UtcOffset) -> String {
    let elapsed = now - at;
    if elapsed < time::Duration::minutes(2) {
        return "a moment ago".into();
    }
    if elapsed < time::Duration::hours(1) {
        return format!("{} minutes ago", elapsed.whole_minutes());
    }
    let (local, today) = (at.to_offset(offset).date(), now.to_offset(offset).date());
    match (today - local).whole_days() {
        days if days <= 0 => match elapsed.whole_hours() {
            1 => "an hour ago".into(),
            hours => format!("{hours} hours ago"),
        },
        1 => "yesterday".into(),
        days if days < 14 => format!("{days} days ago"),
        _ => local.to_string(),
    }
}

fn name_of(save: &SaveFile, id: CreatureId) -> &str {
    save.creatures
        .iter()
        .find(|c| c.id == id)
        .map_or("a past companion", |c| c.name.as_str())
}

fn times(count: u32) -> String {
    match count {
        1 => "once".into(),
        2 => "twice".into(),
        n => format!("{n} times"),
    }
}

/// What an observation says, and the evidence it rests on, both in the notebook's voice.
pub(crate) fn observation_text(
    save: &SaveFile,
    monitors: &[MonitorInfo],
    observation: &Observation,
) -> (String, String) {
    let name = |id| name_of(save, id).to_owned();
    match observation {
        Observation::HighPlaces {
            creature,
            ledge_seconds,
            climbs,
        } => (
            format!("{} prefers high places.", name(*creature)),
            format!(
                "{} up on window ledges in all, more than anyone else here, and {} climbs to \
                 get there.",
                duration_text(*ledge_seconds),
                climbs
            ),
        ),
        Observation::WindowRider {
            creature,
            ride_seconds,
        } => (
            format!("{} likes to ride along on moving windows.", name(*creature)),
            format!(
                "{} riding windows in all, more than anyone else here.",
                duration_text(*ride_seconds)
            ),
        ),
        Observation::Finder {
            creature,
            firsts,
            scrapbook,
        } => (
            format!("{} has a nose for finds.", name(*creature)),
            format!(
                "First to find {firsts} of the {scrapbook} things in the collection, more than \
                 anyone else here."
            ),
        ),
        Observation::SoundSleeper {
            creature,
            longest_seconds,
        } => (
            format!("{} sleeps soundly.", name(*creature)),
            format!(
                "Has slept {} at a stretch without stirring.",
                duration_text(*longest_seconds)
            ),
        ),
        Observation::KeepsToAPlace {
            creature,
            display,
            cell,
            ..
        } => (
            format!(
                "{} keeps to the {} of {}.",
                name(*creature),
                crate::settings::region_label(*cell),
                crate::settings::display_label(*display, monitors)
            ),
            "Comes back there more often than it wanders off, whenever it is set down or \
             settles on its own."
                .into(),
        ),
        Observation::Playful { creature, sessions } => (
            format!("{} is the colony's most playful.", name(*creature)),
            format!("{sessions} games played, more than anyone else here."),
        ),
        Observation::SeekEachOther {
            a,
            b,
            a_sought,
            b_sought,
        } => (
            format!("{} and {} keep seeking each other out.", name(*a), name(*b)),
            format!(
                "{} went looking for {} {}; {} went looking for {} {}.",
                name(*a),
                name(*b),
                times(u32::from(*a_sought)),
                name(*b),
                name(*a),
                times(u32::from(*b_sought))
            ),
        ),
        Observation::FollowsAround {
            seeker,
            sought,
            times: count,
            returned,
        } => (
            format!(
                "{} often goes looking for {}.",
                name(*seeker),
                name(*sought)
            ),
            format!(
                "{} of its own accord, and {} the other way round.",
                times(u32::from(*count)),
                if *returned == 0 {
                    "never".to_owned()
                } else {
                    times(u32::from(*returned))
                }
            ),
        ),
        Observation::NapTogether { a, b, naps } => (
            format!("{} and {} like to nap side by side.", name(*a), name(*b)),
            format!("Seen napping together {}.", times(u32::from(*naps))),
        ),
        Observation::PlayTogether { a, b, games } => (
            format!("{} and {} play well together.", name(*a), name(*b)),
            format!("{games} games together."),
        ),
        Observation::ShareFinds { a, b, gifts } => (
            format!(
                "{} and {} bring each other their finds.",
                name(*a),
                name(*b)
            ),
            format!(
                "A find carried over to the other {}.",
                times(u32::from(*gifts))
            ),
        ),
        Observation::Squabble {
            a,
            b,
            squabbles,
            games,
        } => (
            format!("{} and {} have their squabbles.", name(*a), name(*b)),
            format!(
                "{squabbles} squabbles, against {games} games together. They make it up in \
                 their own time."
            ),
        ),
        Observation::RegularVisitor { name, visits } => (
            format!("{name} is a regular visitor."),
            format!("{visits} visits in the guest book."),
        ),
    }
}

/// The last warm thing a pair shared, in words.
pub(crate) fn shared_text(kind: SharedMomentKind) -> &'static str {
    match kind {
        SharedMomentKind::Rest => "napped side by side",
        SharedMomentKind::Play => "played together",
        SharedMomentKind::Gift => "shared a find",
        SharedMomentKind::Greeting => "said hello",
        SharedMomentKind::Calm => "spent a calm while together",
        SharedMomentKind::Squabble => "had a squabble",
    }
}

/// Relationship memories worth surfacing: the last warm moment each pair shared, newest first,
/// from the last fortnight. Each one names something that was seen to happen.
pub(crate) fn recent_memories(
    save: &SaveFile,
    now: OffsetDateTime,
) -> Vec<(CreatureId, CreatureId, formiga_core::PairMemory)> {
    let here = |id: CreatureId| save.creatures.iter().any(|c| c.id == id);
    let mut memories: Vec<_> = save
        .tallies
        .iter()
        .filter(|r| here(r.a) && here(r.b))
        .filter_map(|r| r.tally.memory.map(|memory| (r.a, r.b, memory)))
        .filter(|(_, _, memory)| now - memory.at <= time::Duration::days(14))
        .collect();
    memories.sort_by_key(|(a, b, memory)| (std::cmp::Reverse(memory.at), *a, *b));
    memories
}

/// The moments of the last seven local days, newest first.
fn this_week(save: &SaveFile, now: OffsetDateTime, offset: time::UtcOffset) -> Vec<&JournalEntry> {
    let start = now.to_offset(offset).date() - time::Duration::days(6);
    save.companion
        .journal
        .iter()
        .rev()
        .filter(|entry| entry.at.to_offset(offset).date() >= start)
        .collect()
}

/// A colony that has only just begun: under a day old, with nothing written but arrivals.
pub(crate) fn just_begun(save: &SaveFile, now: OffsetDateTime) -> bool {
    now - save.created_at_utc < time::Duration::days(1)
        && save
            .companion
            .journal
            .iter()
            .all(|entry| matches!(entry.moment, JournalMoment::Arrival))
}

/// The line under the page's name: which day of the colony this is, and how many are about.
pub(crate) fn today_observation(save: &SaveFile, now: OffsetDateTime) -> String {
    let day = (now - save.created_at_utc).whole_days().max(0) + 1;
    let count = save.creatures.len();
    let awake = save
        .creatures
        .iter()
        .filter(|c| c.state.action != ActionKind::Sleep)
        .count();
    let who = match (count, awake) {
        (0, _) => "Nobody here yet.".to_owned(),
        (1, 1) => "One specimen, awake.".to_owned(),
        (1, _) => "One specimen, napping.".to_owned(),
        (n, a) if a == n => format!("{n} specimens, all awake."),
        (n, 0) => format!("{n} specimens, all napping."),
        (n, a) => format!("{n} specimens, {a} awake."),
    };
    format!("Day {day} of the colony. {who}")
}

/// A small heading inside the page, with a line under it about what the section is.
fn section(ui: &mut Ui, title: &str, help: &str) {
    ui.add_space(14.0);
    journal::kicker(ui, title).on_hover_text(help);
    ui.add_space(4.0);
}

/// The Today page's own state. Never saved.
#[derive(Default)]
pub(crate) struct TodayState {}

impl TodayState {
    /// The Today page. `reading` is what was new when the reader turned to it.
    pub(crate) fn show(
        &mut self,
        ui: &mut Ui,
        shell: &mut Shell,
        save: &SaveFile,
        monitors: &[MonitorInfo],
        tab: &mut SettingsTab,
        reading: Option<&Reading>,
    ) {
        let now = OffsetDateTime::now_utc();
        let offset = local_offset();
        journal::page_heading(
            ui,
            SettingsTab::Today,
            "Today",
            &today_observation(save, now),
        );
        if just_begun(save, now) {
            wide_card(ui, |ui| {
                ui.strong("Settling in");
                ui.label(
                    "Your colony has only just moved in, so there is not much to write down yet \
                     — and that is how it should be. They wander, nap and explore on their own; \
                     this page fills itself in as they do.",
                );
                ui.add_space(4.0);
                ui.small("Click a companion to pet it, drag it to carry it, and right-click it for its menu.");
                ui.small(
                    "The Formiga icon in the menu bar or taskbar hides, pauses and gathers them.",
                );
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Take the tour").clicked() {
                        shell.take_the_tour();
                    }
                    if ui.button("Meet your companions").clicked() {
                        *tab = SettingsTab::Colony;
                    }
                });
            });
        }

        // New since last time: what the reader came to the page for, kept on show while they
        // read it even though it now counts as read.
        let unread = reading.map_or(&[][..], |r| &r.unread[..]);
        if !unread.is_empty() {
            section(
                ui,
                "New since you last looked",
                "Arrivals, friendships, new habits and ways, and visitors written in the journal \
                 since this notebook was last open on Today or the Journal.",
            );
            wide_card(ui, |ui| {
                for entry in unread.iter().take(5) {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(moment_text(save, entry));
                        ui.small(ago_text(entry.at, now, offset));
                    });
                }
                if unread.len() > 5 {
                    ui.small(format!("…and {} more in the journal.", unread.len() - 5));
                }
                if ui.button("Read the journal").clicked() {
                    *tab = SettingsTab::Journal;
                }
            });
        }

        ui.add_space(10.0);
        self.today_card(ui, shell, save, offset);
        section(ui, "Right now", "What each companion is doing this minute.");
        wide_card(ui, |ui| {
            if save.creatures.is_empty() {
                ui.label("Nobody lives here just now. The studio can find someone new.");
                return;
            }
            if save.settings.paused {
                ui.label("Everything is paused, so everyone is holding still.");
            } else if !save.settings.visible {
                ui.label("The colony is hidden from the desktop. They carry on out of sight.");
            }
            if let Some(until) = save.companion.quiet_until
                && until > now
            {
                let local = until.to_offset(offset);
                ui.label(format!(
                    "Settled at home for a quiet while, until {:02}:{:02}.",
                    local.hour(),
                    local.minute()
                ));
            } else if save.home.is_active() {
                ui.label("The houses are out, and the colony is gathered at home.");
            }
            if let Some(guest) = save.visitors.on_stage() {
                ui.label(format!("{} is visiting the houses.", guest.name));
            }
            for creature in &save.creatures {
                ui.horizontal(|ui| {
                    shell.portrait(ui, creature, 32.0);
                    ui.vertical(|ui| {
                        ui.strong(&creature.name);
                        let doing = crate::settings::activity_label(creature.state.action);
                        let high = creature.state.surface.window_key.is_some();
                        ui.small(if high {
                            format!("{doing}, up on a window")
                        } else {
                            doing
                        });
                    });
                });
            }
        });

        let week = this_week(save, now, offset);
        section(
            ui,
            "This week",
            "The last seven days of the journal, counted by kind.",
        );
        wide_card(ui, |ui| {
            let tally = today_tally(&week);
            let found = save
                .companion
                .scrapbook
                .iter()
                .filter(|r| now - r.first_at <= time::Duration::days(7))
                .count();
            if tally.is_empty() && found == 0 {
                ui.label(
                    "A quiet week so far. Nothing written down in the last seven days; the \
                     colony only writes when something happens.",
                );
                return;
            }
            ui.horizontal_wrapped(|ui| {
                for count in tally {
                    egui::Frame::new()
                        .fill(mint())
                        .inner_margin(4)
                        .show(ui, |ui| {
                            ui.small(count);
                        });
                }
                if found > 0 {
                    egui::Frame::new()
                        .fill(mint())
                        .inner_margin(4)
                        .show(ui, |ui| {
                            ui.small(if found == 1 {
                                "1 first find".to_owned()
                            } else {
                                format!("{found} first finds")
                            });
                        });
                }
            });
            // The week's landmarks, if there were any: who arrived and who grew close.
            for entry in week
                .iter()
                .filter(|entry| entry.moment.is_landmark())
                .take(3)
            {
                ui.horizontal_wrapped(|ui| {
                    ui.label(moment_text(save, entry));
                    ui.small(ago_text(entry.at, now, offset));
                });
            }
        });

        let observations = formiga_core::observe(save);
        section(
            ui,
            "Observations",
            "Patterns the notebook has noticed in the colony's own records. Each comes with the \
             counts it rests on, and nothing is said without them.",
        );
        wide_card(ui, |ui| {
            if observations.is_empty() {
                ui.label(
                    "Nothing observed yet. Observations appear once the colony's own records \
                     show a pattern — a favourite perch, a pair who keep finding each other. \
                     Nothing is guessed.",
                );
                return;
            }
            for observation in observations.iter().take(8) {
                let (said, evidence) = observation_text(save, monitors, observation);
                ui.label(said);
                ui.small(RichText::new(evidence).color(muted()));
                ui.add_space(4.0);
            }
            if observations.len() > 8 {
                ui.small(format!(
                    "{} more on the companions' own pages.",
                    observations.len() - 8
                ));
            }
        });

        let memories = recent_memories(save, now);
        if !memories.is_empty() {
            section(
                ui,
                "Lately, between companions",
                "The last warm moment each pair was seen sharing, from the past fortnight.",
            );
            wide_card(ui, |ui| {
                for (a, b, memory) in memories.iter().take(4) {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "{} and {} {}",
                            name_of(save, *a),
                            name_of(save, *b),
                            shared_text(memory.kind)
                        ));
                        ui.small(ago_text(memory.at, now, offset));
                    });
                }
            });
        }
        ui.add_space(10.0);
        ui.small(
            "Everything on this page comes from what the colony has recorded on this computer. \
             Time Formiga was not running is never filled in.",
        );
        // Right now stays right now while the page is open, at a page's frame every two seconds.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(2));
    }
}

/// What today's recap can honestly say: the journal's moments from today, newest first; the
/// treasures the scrapbook says were first found today; and whether earlier moments of today may
/// already have rolled out of a full journal. Nothing is said about time the app was not running,
/// because nothing was written down then.
pub(crate) struct Today<'a> {
    pub(crate) moments: Vec<&'a JournalEntry>,
    pub(crate) found: Vec<u8>,
    pub(crate) rolled_out: bool,
}

pub(crate) fn today<'a>(
    save: &'a SaveFile,
    date: time::Date,
    offset: time::UtcOffset,
) -> Today<'a> {
    let on_date = |at: OffsetDateTime| at.to_offset(offset).date() == date;
    let journal = &save.companion.journal;
    Today {
        moments: journal
            .iter()
            .rev()
            .filter(|entry| on_date(entry.at))
            .collect(),
        found: save
            .companion
            .scrapbook
            .iter()
            .filter(|record| on_date(record.first_at))
            .map(|record| record.variant)
            .collect(),
        // A full journal whose oldest everyday moment is itself from today has dropped whatever
        // came before it, and some of that may have been today's too. Milestones kept past their
        // turn say nothing either way.
        rolled_out: save
            .companion
            .rolled_out_since(date.midnight().assume_offset(offset)),
    }
}

/// Short counts for the kinds of moment a day held, in a fixed order.
fn today_tally(moments: &[&JournalEntry]) -> Vec<String> {
    let count = |test: &dyn Fn(&JournalMoment) -> bool| {
        moments.iter().filter(|entry| test(&entry.moment)).count()
    };
    let plural = |n: usize, one: &str, many: &str| {
        if n == 1 {
            format!("1 {one}")
        } else {
            format!("{n} {many}")
        }
    };
    let mut tally = Vec::new();
    for (n, one, many) in [
        (
            count(&|m| matches!(m, JournalMoment::Arrival)),
            "arrival",
            "arrivals",
        ),
        (
            count(&|m| matches!(m, JournalMoment::Discovery)),
            "discovery",
            "discoveries",
        ),
        (
            count(&|m| matches!(m, JournalMoment::Friendship(_))),
            "new friendship",
            "new friendships",
        ),
        (
            count(&|m| matches!(m, JournalMoment::Preference(_))),
            "new preference",
            "new preferences",
        ),
        (
            count(&|m| matches!(m, JournalMoment::Ritual(_))),
            "shared moment",
            "shared moments",
        ),
        (
            count(&|m| {
                matches!(
                    m,
                    JournalMoment::Object(_)
                        | JournalMoment::Decoration(_)
                        | JournalMoment::Unlocked(_)
                )
            }),
            "new keepsake",
            "new keepsakes",
        ),
        (
            count(&|m| matches!(m, JournalMoment::Visit(_))),
            "visitor",
            "visitors",
        ),
        (
            count(&|m| matches!(m, JournalMoment::Habit(_))),
            "new habit",
            "new habits",
        ),
        (
            count(&|m| matches!(m, JournalMoment::Wonder(_))),
            "new wonder",
            "new wonders",
        ),
    ] {
        if n > 0 {
            tally.push(plural(n, one, many));
        }
    }
    tally
}

impl TodayState {
    /// Today at a glance, on the Today page: who the day was about, what kinds of thing
    /// happened, what was found, and the latest few moments, all read from what was recorded.
    pub(crate) fn today_card(
        &mut self,
        ui: &mut Ui,
        shell: &mut Shell,
        save: &SaveFile,
        offset: time::UtcOffset,
    ) {
        let date = OffsetDateTime::now_utc().to_offset(offset).date();
        let today = today(save, date, offset);
        wide_card(ui, |ui| {
            journal::kicker(ui, "Today in your colony");
            if today.moments.is_empty() && today.found.is_empty() {
                ui.label(
                    "Nothing has been written down yet today. Moments appear here as they happen.",
                );
                return;
            }
            // Everyone the day was about, in colony order.
            let featured: Vec<&Creature> = save
                .creatures
                .iter()
                .filter(|creature| {
                    today.moments.iter().any(|entry| {
                        entry.creature == Some(creature.id)
                            || entry.moment == JournalMoment::Friendship(creature.id)
                    })
                })
                .collect();
            if !featured.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    for creature in featured {
                        shell.portrait(ui, creature, 40.0);
                    }
                });
            }
            ui.horizontal_wrapped(|ui| {
                for count in today_tally(&today.moments) {
                    egui::Frame::new()
                        .fill(mint())
                        .inner_margin(4)
                        .show(ui, |ui| {
                            ui.small(count);
                        });
                }
            });
            if !today.found.is_empty() {
                let atlas = shell.trinket_atlas(ui, save);
                ui.horizontal_wrapped(|ui| {
                    ui.small("Found today:");
                    for variant in &today.found {
                        ui.add(Shell::trinket_image(&atlas, *variant, 28.0));
                    }
                });
            }
            for entry in today.moments.iter().take(4) {
                let local = entry.at.to_offset(offset);
                ui.label(format!(
                    "{:02}:{:02} · {}",
                    local.hour(),
                    local.minute(),
                    moment_text(save, entry)
                ));
            }
            if today.moments.len() > 4 {
                ui.small(format!(
                    "…and {} more from today below.",
                    today.moments.len() - 4
                ));
            }
            if today.rolled_out {
                ui.small(format!(
                    "The journal keeps its last {MAX_JOURNAL_ENTRIES} moments, so some from \
                     earlier today have already rolled out."
                ));
            }
        });
        ui.add_space(10.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn durations_and_ages_read_the_way_a_note_would() {
        assert_eq!(duration_text(30), "1 min");
        assert_eq!(duration_text(45 * 60), "45 min");
        assert_eq!(duration_text(3 * 3600), "3 h");
        assert_eq!(duration_text(3 * 3600 + 12 * 60 + 5), "3 h 12 min");
        let now = datetime!(2026-10-01 18:00 UTC);
        let utc = time::UtcOffset::UTC;
        assert_eq!(ago_text(now, now, utc), "a moment ago");
        assert_eq!(
            ago_text(now - time::Duration::minutes(20), now, utc),
            "20 minutes ago"
        );
        assert_eq!(
            ago_text(now - time::Duration::hours(3), now, utc),
            "3 hours ago"
        );
        assert_eq!(
            ago_text(now - time::Duration::hours(20), now, utc),
            "yesterday"
        );
        assert_eq!(
            ago_text(now - time::Duration::days(4), now, utc),
            "4 days ago"
        );
        assert_eq!(
            ago_text(now - time::Duration::days(40), now, utc),
            "2026-08-22"
        );
        // A clock wound back reads as a moment ago, never as the future.
        assert_eq!(
            ago_text(now + time::Duration::hours(1), now, utc),
            "a moment ago"
        );
    }
}
