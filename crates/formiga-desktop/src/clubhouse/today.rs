//! The notebook's first page: the colony today, at a glance. What is new since it was last read,
//! who is doing what right now, how the week has gone, what the notebook has noticed, and the
//! last few things companions shared with one another. Everything on it is read from what the
//! colony has recorded; nothing here is a task, a score or a reminder.
use super::shell::Shell;
use super::*;
use crate::settings::SettingsTab;
use formiga_core::{DayNote, GardenKind, GardenStage, Observation, SharedMomentKind};

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

/// What an observation says, and the evidence it rests on, both in the researcher's own voice: a
/// claim jotted down, then the counts that back it.
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
            format!("{} loves high places!", name(*creature)),
            format!(
                "I've clocked {} up on window ledges, more than anyone else, and {} climbs to \
                 get up there!",
                duration_text(*ledge_seconds),
                climbs
            ),
        ),
        Observation::WindowRider {
            creature,
            ride_seconds,
        } => (
            format!("{} loves riding along on moving windows!", name(*creature)),
            format!(
                "{} riding windows so far, more than anyone else.",
                duration_text(*ride_seconds)
            ),
        ),
        Observation::Finder {
            creature,
            firsts,
            scrapbook,
        } => (
            format!("{} has a nose for finds!", name(*creature)),
            format!(
                "First to find {firsts} of the {scrapbook} things in the collection. Nobody's \
                 found more."
            ),
        ),
        Observation::SoundSleeper {
            creature,
            longest_seconds,
        } => (
            format!("{} sleeps like a log.", name(*creature)),
            format!(
                "Slept {} at a stretch without stirring once!",
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
            "Wherever it's set down, it heads back there more often than it wanders off.".into(),
        ),
        Observation::Playful { creature, sessions } => (
            format!("{} is the most playful of the lot!", name(*creature)),
            format!("{sessions} games played, more than anyone else."),
        ),
        Observation::SeekEachOther {
            a,
            b,
            a_sought,
            b_sought,
        } => (
            format!("{} and {} keep finding each other.", name(*a), name(*b)),
            format!(
                "{} went looking for {} {}, and {} went looking for {} {}.",
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
                "{} is always off looking for {}.",
                name(*seeker),
                name(*sought)
            ),
            format!(
                "{} all on its own, and {} the other way round.",
                capitalised(&times(u32::from(*count))),
                if *returned == 0 {
                    "never".to_owned()
                } else {
                    times(u32::from(*returned))
                }
            ),
        ),
        Observation::NapTogether { a, b, naps } => (
            format!("{} and {} love a nap side by side.", name(*a), name(*b)),
            format!(
                "Caught them napping together {} now!",
                times(u32::from(*naps))
            ),
        ),
        Observation::PlayTogether { a, b, games } => (
            format!("{} and {} play so well together!", name(*a), name(*b)),
            format!("{games} games together so far."),
        ),
        Observation::ShareFinds { a, b, gifts } => (
            format!(
                "{} and {} bring each other their finds.",
                name(*a),
                name(*b)
            ),
            format!(
                "I've seen a find carried over to the other {}.",
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
                "{squabbles} squabbles, against {games} games together. They always make it up \
                 in their own time."
            ),
        ),
        Observation::RegularVisitor { name, visits } => (
            format!("{name} is a regular!"),
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

/// A number as a word, capitalised, up to twelve; past that, the figure.
fn count_word(count: usize) -> String {
    const WORDS: [&str; 13] = [
        "None", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten",
        "Eleven", "Twelve",
    ];
    WORDS
        .get(count)
        .map_or_else(|| count.to_string(), |word| (*word).to_owned())
}

/// The day of the colony it is, from 1.
pub(crate) fn colony_day(save: &SaveFile, now: OffsetDateTime) -> i64 {
    (now - save.created_at_utc).whole_days().max(0) + 1
}

/// The line under the page's name: which day of the colony this is, and how many are about.
pub(crate) fn today_observation(save: &SaveFile, now: OffsetDateTime) -> String {
    let day = colony_day(save, now);
    let count = save.creatures.len();
    let awake = save
        .creatures
        .iter()
        .filter(|c| c.state.action != ActionKind::Sleep)
        .count();
    let who = match (count, awake) {
        (0, _) => "Nobody here yet.".to_owned(),
        (1, 1) => "Just the one, and they're up and about.".to_owned(),
        (1, _) => "Just the one, and they're napping.".to_owned(),
        (n, a) if a == n => format!("{} of them, all up and about.", count_word(n)),
        (n, 0) => format!("{} of them, all napping.", count_word(n)),
        (n, 1) => format!("{} of them, and one is up and about.", count_word(n)),
        (n, a) => format!(
            "{} of them, and {} are up and about.",
            count_word(n),
            count_word(a).to_lowercase()
        ),
    };
    format!("Day {day}! {who}")
}

/// How long the Today page keeps its comparisons before reading them again: long enough that a
/// line does not come and go as the counts under it tick over.
const NOTES_FRESH_FOR: std::time::Duration = std::time::Duration::from_secs(30);

/// The Today page's own state. Never saved.
#[derive(Default)]
pub(crate) struct TodayState {
    /// How today compares with the rest of the week, and when that was read.
    notes: Vec<DayNote>,
    noted_at: Option<std::time::Instant>,
}

/// The garden a patch is, as a sentence's subject: what it is called, and whether it is several.
fn garden_subject(kind: GardenKind) -> (String, bool) {
    let plural = matches!(kind, GardenKind::Sunflowers | GardenKind::Cactus);
    (format!("the {}", kind.label().to_lowercase()), plural)
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// What a comparison says, and the counts it rests on, both in the researcher's own voice.
pub(crate) fn day_note_text(save: &SaveFile, note: &DayNote) -> (String, String) {
    match *note {
        DayNote::SoughtEachOther {
            a,
            b,
            sought: [by_a, by_b],
        } => {
            let count = u16::from(by_a) + u16::from(by_b);
            (
                format!(
                    "{} and {} went looking for each other {} today!",
                    name_of(save, a),
                    name_of(save, b),
                    times(u32::from(count))
                ),
                format!(
                    "{} {}, {} {}.",
                    name_of(save, a),
                    times(u32::from(by_a)),
                    name_of(save, b),
                    times(u32::from(by_b))
                ),
            )
        }
        DayNote::WentLookingFor {
            seeker,
            sought,
            times: count,
        } => (
            format!(
                "{} went looking for {} {} today!",
                name_of(save, seeker),
                name_of(save, sought),
                times(u32::from(count))
            ),
            "Followed them, greeted them coming home, or brought them a find.".to_owned(),
        ),
        DayNote::FirstRoofSitThisWeek { creature } => (
            format!("{} made it up on the roof!", name_of(save, creature)),
            "First time all week. Not once in the six days before.".to_owned(),
        ),
        DayNote::GardenStage { kind, stage } => {
            let (subject, plural) = garden_subject(kind);
            let (has, its) = if plural {
                ("have", "their")
            } else {
                ("has", "its")
            };
            let said = match stage {
                GardenStage::Bounty => format!(
                    "{} {has} reached {its} fourth stage, as full as it gets!",
                    capitalised(&subject)
                ),
                _ => format!("{} {has} reached {its} third stage!", capitalised(&subject)),
            };
            (
                said,
                format!(
                    "It came round to it today, at {} hours a stage.",
                    kind.stage_hours()
                ),
            )
        }
        DayNote::MoreTimeAtHome { today, yesterday } => (
            "More time at home than yesterday!".to_owned(),
            if yesterday == 0 {
                format!(
                    "{} with the houses out today, and none at all yesterday.",
                    duration_text(today)
                )
            } else {
                format!(
                    "{} with the houses out today, up from {} yesterday.",
                    duration_text(today),
                    duration_text(yesterday)
                )
            },
        ),
        DayNote::MorePlay { today, yesterday } => (
            "Way more play than yesterday!".to_owned(),
            format!("{today} games today, up from {yesterday} yesterday."),
        ),
    }
}

/// A section's heading on its line, with a few printed words at the far end of the line: how many
/// there are, or what the section rests on. The heading says what the section is when pointed at.
fn section(ui: &mut Ui, title: &str, aside: Option<&str>, help: &str) {
    let scale = journal::text_scale(ui);
    journal::ruled_row(ui, |ui| {
        journal::kicker(ui, title).on_hover_text(help);
        if let Some(aside) = aside {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                journal::write(
                    ui,
                    journal::kicker_job(aside, journal::kicker_size(scale), muted()),
                );
            });
        }
    });
}

/// A line written in the pencil hand's bold.
fn bold(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(formiga_pencil::hand_bold(size))
}

/// Words that turn to another page when clicked, written on the next line: a pencilled link.
fn page_link(ui: &mut Ui, text: &str) -> bool {
    let response = journal::write_with(
        ui,
        RichText::new(text).color(forest()),
        None,
        egui::Sense::click(),
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

/// "1 new friendship", "2 discoveries".
fn counted(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// What kind of moment something was, as the highlight's printed line names it.
fn moment_kind(moment: &JournalMoment) -> &'static str {
    match moment {
        JournalMoment::Arrival => "new arrival",
        JournalMoment::Friendship(_) => "new friendship",
        JournalMoment::Visit(_) => "visitor",
        JournalMoment::Revisit(..) => "visitor, back again",
        JournalMoment::Discovery => "discovery",
        JournalMoment::Preference(_) => "new preference",
        JournalMoment::Habit(_) => "new habit",
        JournalMoment::Wonder(_) => "wonder",
        JournalMoment::Ritual(_) => "shared moment",
        JournalMoment::Trip => "trip away",
        JournalMoment::HouseVisit => "house visit",
        JournalMoment::NewLook => "new look",
        JournalMoment::Battle(_) => "match",
        JournalMoment::Object(_) | JournalMoment::Decoration(_) | JournalMoment::Unlocked(_) => {
            "new keepsake"
        }
    }
}

/// Everyone a moment is about who still lives here: whoever it is about, and a new friend.
fn moment_faces<'a>(save: &'a SaveFile, entry: &JournalEntry) -> Vec<&'a Creature> {
    let mut ids = Vec::new();
    ids.extend(entry.creature);
    if let JournalMoment::Friendship(friend) = entry.moment {
        ids.push(friend);
    }
    ids.iter()
        .filter_map(|id| save.creatures.iter().find(|c| c.id == *id))
        .collect()
}

/// The moment today's highlight is about: a landmark, the newest unread one first, then the
/// newest; failing that, the newest moment worth a small sign. The page only ever pastes in
/// something the journal wrote down.
fn highlight<'a>(today: &Today<'a>, reading: Option<&Reading>) -> Option<&'a JournalEntry> {
    let unread = |entry: &&&JournalEntry| reading.is_some_and(|r| r.is_new(entry));
    let landmarks = || today.moments.iter().filter(|e| e.moment.is_landmark());
    landmarks()
        .find(unread)
        .or_else(|| landmarks().next())
        .or_else(|| today.moments.iter().find(|e| e.moment.is_noteworthy()))
        .copied()
}

/// "16:40", in the reader's own time.
fn clock(at: OffsetDateTime, offset: time::UtcOffset) -> String {
    let local = at.to_offset(offset);
    format!("{:02}:{:02}", local.hour(), local.minute())
}

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
        let local = now.to_offset(offset);
        let scale = journal::text_scale(ui);
        if self
            .noted_at
            .is_none_or(|at| at.elapsed() >= NOTES_FRESH_FOR)
        {
            self.notes = formiga_core::day_notes(save, local);
            self.noted_at = Some(std::time::Instant::now());
        }
        let top = ui.cursor().top();
        journal::page_heading(
            ui,
            SettingsTab::Today,
            "Today",
            &today_observation(save, now),
        );
        // The date, stamped in the corner the way a field note is dated.
        let corner = egui::pos2(ui.max_rect().right() - 70.0 * scale, top + 52.0 * scale);
        let date = format!(
            "{} {} {} {}",
            &local.weekday().to_string()[..3],
            local.day(),
            &local.month().to_string()[..3],
            local.year()
        );
        paper::stamp(
            ui.painter(),
            corner,
            &[
                (&date, 15.0 * scale),
                (&format!("Day {}", colony_day(save, now)), 24.0 * scale),
            ],
            stamp(),
            None,
            (-4f32).to_radians(),
        );

        let today = today(save, local.date(), offset);
        self.right_now(ui, shell, save, now, &today, tab);
        self.highlight_and_tally(ui, shell, save, now, offset, &today, reading, tab);
        self.log(ui, shell, save, now, offset, &today);
        self.noticed(ui, save, monitors, tab);
        self.compared(ui, save, local);
        self.week(ui, shell, save, now, offset);
        ui.add_space(8.0);
        journal::write(
            ui,
            RichText::new(
                "Everything here is stuff I actually saw them do on this computer. If Formiga \
                 wasn't running, I don't make anything up.",
            )
            .color(muted()),
        );
        // Right now stays right now while the page is open, at a page's frame every two seconds.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(2));
    }

    /// Right now: the colony standing on a strip of ground, each with its name, what it is
    /// doing, and the bubble of whatever it did today. A colony that has only just moved in gets
    /// a note pasted over it instead of empty boxes further down.
    fn right_now(
        &mut self,
        ui: &mut Ui,
        shell: &mut Shell,
        save: &SaveFile,
        now: OffsetDateTime,
        today: &Today<'_>,
        tab: &mut SettingsTab,
    ) {
        let scale = journal::text_scale(ui);
        let pitch = journal::rule_pitch(scale);
        let awake = save
            .creatures
            .iter()
            .filter(|c| c.state.action != ActionKind::Sleep)
            .count();
        let napping = save.creatures.len() - awake;
        let aside = match (awake, napping) {
            (0, 0) => None,
            (_, 0) => Some(format!("{awake} awake")),
            (0, _) => Some(format!("{napping} napping")),
            _ => Some(format!("{awake} awake · {napping} napping")),
        };
        section(
            ui,
            "Right now",
            aside.as_deref(),
            "What each companion is doing this minute.",
        );
        journal::to_next_rule(ui);
        let (plate, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 8.0 * pitch),
            egui::Sense::hover(),
        );
        let painter = ui.painter().clone();
        let mut fills = journal::Fills::default();
        journal::stepped_box(&mut fills, plate, card_fill(), line());
        let ground = egui::Rect::from_min_max(
            egui::pos2(plate.left(), plate.bottom() - 46.0 * scale),
            plate.right_bottom(),
        );
        fills.rect(ground, ground_strip());
        fills.rect(
            egui::Rect::from_min_size(ground.left_top(), egui::vec2(ground.width(), 2.0)),
            ground_edge(),
        );
        fills.paint(&painter);

        // What is going on with the colony as a whole, jotted in the plate's corner.
        let mut status = Vec::new();
        if save.settings.paused {
            status.push("Everything's paused, so they're all holding still.".to_owned());
        } else if !save.settings.visible {
            status.push("Hidden from the desktop. They carry on out of sight.".to_owned());
        }
        if let Some(until) = save.companion.quiet_until
            && until > now
        {
            status.push(format!(
                "Settled at home until {}.",
                clock(until, local_offset())
            ));
        } else if save.home.is_active() {
            status.push("The houses are out, and everyone's at home.".to_owned());
        }
        if let Some(guest) = save.visitors.on_stage() {
            status.push(format!("{} is visiting!", guest.name));
        }
        if !status.is_empty() {
            let galley = painter.layout(
                status.join(" "),
                egui::FontId::proportional(15.0 * scale),
                muted(),
                plate.width() * 0.6,
            );
            painter.galley(
                egui::pos2(plate.right() - 16.0 - galley.size().x, plate.top() + 8.0),
                galley,
                muted(),
            );
        }

        if save.creatures.is_empty() {
            let galley = painter.layout(
                "Nobody lives here just now. The studio can find someone new!".to_owned(),
                egui::FontId::proportional(16.0 * scale),
                ink(),
                plate.width() - 32.0,
            );
            painter.galley(plate.center() - galley.size() / 2.0, galley, ink());
            return;
        }

        // Everyone stands on the ground, as many to a row as fit, centred when they are few.
        let count = save.creatures.len() as f32;
        let gap = 10.0;
        let column = (104.0 * scale).min((plate.width() - 32.0 - gap * (count - 1.0)) / count);
        let portrait = (76.0 * scale.min(1.2)).min(column - 8.0).max(24.0);
        let row = column * count + gap * (count - 1.0);
        let left = if count <= 3.0 {
            plate.center().x - row / 2.0
        } else {
            plate.left() + (plate.width() - row) / 2.0
        };
        let name_size = 16.0 * scale;
        let doing_size = 15.0 * scale;
        for (index, creature) in save.creatures.iter().enumerate() {
            let x = left + index as f32 * (column + gap);
            let middle = x + column / 2.0;
            let doing = if creature.state.surface.window_key.is_some() {
                "Up on a window".to_owned()
            } else {
                capitalised(&crate::settings::activity_label(creature.state.action))
            };
            let doing_galley = painter.layout(
                doing.clone(),
                egui::FontId::proportional(doing_size),
                muted(),
                column,
            );
            let name_galley = painter.layout_no_wrap(
                creature.name.clone(),
                formiga_pencil::hand_bold(name_size),
                ink(),
            );
            let doing_top = plate.bottom() - 8.0 - doing_galley.size().y;
            let name_top = doing_top - name_galley.size().y + 4.0;
            let face = egui::Rect::from_min_size(
                egui::pos2(middle - portrait / 2.0, name_top - 4.0 - portrait),
                egui::vec2(portrait, portrait),
            );
            let texture = shell.portrait_texture(ui.ctx(), creature);
            paper::face(&painter, face, &texture, false);
            painter.galley(
                egui::pos2(middle - name_galley.size().x / 2.0, name_top),
                name_galley,
                ink(),
            );
            painter.galley(
                egui::pos2(middle - doing_galley.size().x / 2.0, doing_top),
                doing_galley,
                muted(),
            );
            // The bubble of the last thing it did today, over its head.
            let last = today.moments.iter().find(|entry| {
                entry.creature == Some(creature.id)
                    || entry.moment == JournalMoment::Friendship(creature.id)
            });
            if let Some(entry) = last {
                let size = 32.0 * scale.min(1.2);
                let icon = paper::moment_icon(&entry.moment);
                shell.bubble(
                    &painter,
                    egui::pos2(middle - size * 17.0 / 32.0, face.top() - size),
                    size,
                    icon,
                );
            }
            let spot = egui::Rect::from_min_max(
                egui::pos2(x, face.top()),
                egui::pos2(x + column, plate.bottom()),
            );
            let hint = match last {
                Some(entry) => format!(
                    "{}: {}. Today: {}.",
                    creature.name,
                    doing.to_lowercase(),
                    moment_text(save, entry)
                ),
                None => format!("{}: {}.", creature.name, doing.to_lowercase()),
            };
            ui.interact(
                spot,
                ui.id().with(("right-now", creature.id)),
                egui::Sense::hover(),
            )
            .on_hover_text(hint);
        }

        if just_begun(save, now) {
            let width = (plate.width() * 0.52)
                .max(260.0 * scale)
                .min(plate.width() - 32.0);
            let note = egui::Rect::from_min_size(
                egui::pos2(plate.right() - 20.0 - width, plate.top() + 14.0),
                egui::vec2(width, plate.height() - 28.0),
            );
            let mut sticky = ui.new_child(egui::UiBuilder::new().max_rect(note));
            journal::sticky_note(&mut sticky, |ui| {
                ui.label(bold("Settling in!", 18.0 * scale));
                ui.label(
                    "They've only just moved in, so there's not much to write yet. That's totally \
                     fine! I'll fill this page in as they go.",
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
    }

    /// The day's one highlight, pasted in with tape, beside a card counting what kinds of thing
    /// happened today and what was found. Side by side in a wide window, one above the other in a
    /// narrow one.
    #[allow(clippy::too_many_arguments)]
    fn highlight_and_tally(
        &mut self,
        ui: &mut Ui,
        shell: &mut Shell,
        save: &SaveFile,
        now: OffsetDateTime,
        offset: time::UtcOffset,
        today: &Today<'_>,
        reading: Option<&Reading>,
        tab: &mut SettingsTab,
    ) {
        let scale = journal::text_scale(ui);
        let pitch = journal::rule_pitch(scale);
        let gap = pitch;
        ui.add_space(pitch);
        journal::to_next_rule(ui);
        let width = ui.available_width();
        let side_by_side = width >= 560.0 * scale;
        let column = if side_by_side {
            ((width - gap) / 2.0).floor()
        } else {
            width
        };
        let tall = 9.0 * pitch;
        let mut highlight_card = |ui: &mut Ui, shell: &mut Shell| {
            paper::pasted(ui, column, tall, true, |ui| {
                self.highlight(ui, shell, save, now, offset, today, reading, tab);
            });
        };
        let tally_card = |ui: &mut Ui, shell: &mut Shell| {
            paper::pasted(ui, column, tall, false, |ui| {
                tally(ui, shell, save, today);
            });
        };
        if side_by_side {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                highlight_card(ui, shell);
                tally_card(ui, shell);
            });
        } else {
            highlight_card(ui, shell);
            ui.add_space(gap / 2.0);
            tally_card(ui, shell);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn highlight(
        &mut self,
        ui: &mut Ui,
        shell: &mut Shell,
        save: &SaveFile,
        now: OffsetDateTime,
        offset: time::UtcOffset,
        today: &Today<'_>,
        reading: Option<&Reading>,
        tab: &mut SettingsTab,
    ) {
        let scale = journal::text_scale(ui);
        let kicker_size = journal::kicker_size(scale);
        let chosen = highlight(today, reading);
        let new = chosen.is_some_and(|entry| reading.is_some_and(|r| r.is_new(entry)));
        let head = ui.label(journal::kicker_job(
            "Today's highlight",
            kicker_size,
            forest(),
        ));
        if new {
            let at = egui::pos2(ui.max_rect().right() - 24.0 * scale, head.rect.center().y);
            paper::stamp(
                ui.painter(),
                at,
                &[("New", kicker_size)],
                forest(),
                Some(mint()),
                3f32.to_radians(),
            );
        }
        let Some(entry) = chosen else {
            ui.add_space(12.0);
            ui.label(
                "Nothing big today yet. I'll paste the day's best moment in here as soon as it \
                 happens!",
            );
            return;
        };
        ui.add_space(14.0 * scale);
        ui.horizontal_top(|ui| {
            // The faces it is about, overlapping a little, with its bubble over the first.
            let faces = moment_faces(save, entry);
            let size = 64.0 * scale.min(1.2);
            let overlap = size * 0.22;
            let across = if faces.is_empty() {
                size
            } else {
                faces.len() as f32 * size - (faces.len() as f32 - 1.0) * overlap
            };
            let bubble = 32.0 * scale.min(1.2);
            let (room, _) = ui.allocate_exact_size(
                egui::vec2(across, size + bubble * 0.7),
                egui::Sense::hover(),
            );
            let painter = ui.painter().clone();
            let top = room.top() + bubble * 0.7;
            for (index, creature) in faces.iter().enumerate() {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(room.left() + index as f32 * (size - overlap), top),
                    egui::vec2(size, size),
                );
                let texture = shell.portrait_texture(ui.ctx(), creature);
                paper::face(&painter, rect, &texture, true);
            }
            shell.bubble(
                &painter,
                egui::pos2(room.left() + size * 0.5 - bubble * 0.5, room.top()),
                bubble,
                paper::moment_icon(&entry.moment),
            );
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.label(bold(moment_text(save, entry), 22.0 * scale.min(1.2)));
                ui.label(journal::kicker_job(
                    &format!(
                        "{} · {} · {}",
                        clock(entry.at, offset),
                        ago_text(entry.at, now, offset),
                        moment_kind(&entry.moment)
                    ),
                    kicker_size,
                    muted(),
                ));
                // What the day book counted about the same companions, if it counted anything.
                let about = moment_faces(save, entry)
                    .iter()
                    .map(|c| c.id)
                    .collect::<Vec<_>>();
                if let Some(note) = self.notes.iter().find(|note| {
                    let subjects = note.subjects();
                    !subjects.is_empty() && subjects.iter().all(|id| about.contains(id))
                }) {
                    let (_, evidence) = day_note_text(save, note);
                    ui.label(RichText::new(evidence).color(muted()));
                }
                ui.add_space(4.0);
                if ui.button("Read all about it ›").clicked() {
                    *tab = SettingsTab::Journal;
                }
            });
        });
    }

    /// Today's log: every moment of the day on the page's own lines, newest first, each with its
    /// time, its picture and the faces it is about; landmarks in bold. Then the last warm thing
    /// each pair shared lately.
    fn log(
        &mut self,
        ui: &mut Ui,
        shell: &mut Shell,
        save: &SaveFile,
        now: OffsetDateTime,
        offset: time::UtcOffset,
        today: &Today<'_>,
    ) {
        let scale = journal::text_scale(ui);
        let pitch = journal::rule_pitch(scale);
        ui.add_space(pitch);
        let count = today.moments.len();
        let aside = (count > 0).then(|| counted(count, "moment", "moments"));
        section(
            ui,
            "Today's log",
            aside.as_deref(),
            "Everything the journal wrote down today, newest first.",
        );
        if count == 0 {
            journal::write(
                ui,
                "Nothing written down yet today. I'll jot things here as they happen!",
            );
        }
        for entry in today.moments.iter().take(8) {
            journal::ruled_row(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                journal::write(ui, RichText::new(clock(entry.at, offset)).color(muted()));
                let (spot, _) =
                    ui.allocate_exact_size(egui::vec2(17.0, pitch), egui::Sense::hover());
                shell.bubble(
                    ui.painter(),
                    egui::pos2(spot.left(), spot.bottom() - 19.0),
                    16.0,
                    paper::moment_icon(&entry.moment),
                );
                let faces = moment_faces(save, entry);
                let face = 18.0;
                let room = ui.available_width() - faces.len() as f32 * (face + 4.0) - 8.0;
                let text = moment_text(save, entry);
                let text = if entry.moment.is_landmark() {
                    bold(text, 16.0 * scale)
                } else {
                    RichText::new(text)
                };
                journal::write_with(ui, text, Some(room), egui::Sense::hover());
                for creature in faces {
                    let (spot, _) =
                        ui.allocate_exact_size(egui::vec2(face, pitch), egui::Sense::hover());
                    let texture = shell.portrait_texture(ui.ctx(), creature);
                    paper::face(
                        ui.painter(),
                        egui::Rect::from_min_size(
                            egui::pos2(spot.left(), spot.bottom() - face - 2.0),
                            egui::vec2(face, face),
                        ),
                        &texture,
                        true,
                    );
                }
            });
        }
        if count > 8 {
            journal::write(
                ui,
                RichText::new(format!(
                    "…and {} more from today in the journal.",
                    count - 8
                ))
                .color(muted()),
            );
        }
        if today.rolled_out {
            journal::write(
                ui,
                RichText::new(format!(
                    "The journal only keeps its last {MAX_JOURNAL_ENTRIES} moments, so a few \
                     from earlier today have already rolled out."
                ))
                .color(muted()),
            );
        }

        let memories = recent_memories(save, now);
        if !memories.is_empty() {
            ui.add_space(pitch / 2.0);
            section(
                ui,
                "Lately, between friends",
                None,
                "The last warm moment each pair was seen sharing, from the past fortnight.",
            );
            for (a, b, memory) in memories.iter().take(4) {
                journal::ruled_row(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let (spot, _) =
                        ui.allocate_exact_size(egui::vec2(17.0, pitch), egui::Sense::hover());
                    shell.bubble(
                        ui.painter(),
                        egui::pos2(spot.left(), spot.bottom() - 19.0),
                        16.0,
                        paper::shared_icon(memory.kind),
                    );
                    journal::write(
                        ui,
                        format!(
                            "{} and {} {}",
                            name_of(save, *a),
                            name_of(save, *b),
                            shared_text(memory.kind)
                        ),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        journal::write(
                            ui,
                            journal::kicker_job(
                                &ago_text(memory.at, now, offset),
                                journal::kicker_size(scale),
                                muted(),
                            ),
                        );
                    });
                });
            }
        }
    }

    /// What the notebook has noticed in the colony's own records, each claim with the counts it
    /// rests on.
    fn noticed(
        &mut self,
        ui: &mut Ui,
        save: &SaveFile,
        monitors: &[MonitorInfo],
        tab: &mut SettingsTab,
    ) {
        let scale = journal::text_scale(ui);
        ui.add_space(journal::rule_pitch(scale));
        let observations = formiga_core::observe(save);
        section(
            ui,
            "Things I've noticed",
            Some("and how I know"),
            "Patterns in the colony's own records. Each comes with the counts it rests on, and \
             nothing is said without them.",
        );
        if observations.is_empty() {
            journal::write(
                ui,
                "Nothing I can vouch for yet. Once their own records show a pattern, a favourite \
                 perch or a pair who keep finding each other, I'll write it here. No guessing!",
            );
            return;
        }
        for observation in observations.iter().take(4) {
            let (said, evidence) = observation_text(save, monitors, observation);
            journal::write(ui, bold(said, 16.0 * scale));
            journal::write(ui, RichText::new(evidence).color(muted()));
        }
        if observations.len() > 4
            && page_link(
                ui,
                &format!(
                    "{} more on the companions' own pages ›",
                    observations.len() - 4
                ),
            )
        {
            *tab = SettingsTab::Colony;
        }
    }

    /// How today compares with the rest of the week: a few lines, each with the counts it rests
    /// on, and only for days the day book was counting.
    fn compared(&mut self, ui: &mut Ui, save: &SaveFile, local: OffsetDateTime) {
        let scale = journal::text_scale(ui);
        ui.add_space(journal::rule_pitch(scale));
        section(
            ui,
            "Vs. the rest of the week",
            None,
            "How today compares with the rest of the week, from the few counts the colony keeps \
             for each day. Nothing is compared with a day before it began keeping them.",
        );
        if self.notes.is_empty() {
            let day = local.date().to_julian_day();
            journal::write(
                ui,
                if save.day_book.counted(day - 1) {
                    "Nothing stands out yet today. I'll note it when today does something the \
                     rest of the week didn't."
                } else {
                    "I only started counting days today, so there's no yesterday to compare with \
                     until tomorrow!"
                },
            );
            return;
        }
        for note in &self.notes {
            let (said, evidence) = day_note_text(save, note);
            journal::write(ui, bold(said, 16.0 * scale));
            journal::write(ui, RichText::new(evidence).color(muted()));
        }
    }

    /// The last seven days, a cell each: a dot for every moment and the picture of the day's
    /// landmark. Today is edged in forest; days before the colony began are faint.
    fn week(
        &mut self,
        ui: &mut Ui,
        shell: &mut Shell,
        save: &SaveFile,
        now: OffsetDateTime,
        offset: time::UtcOffset,
    ) {
        let scale = journal::text_scale(ui);
        let pitch = journal::rule_pitch(scale);
        ui.add_space(pitch);
        let week = this_week(save, now, offset);
        let friendships = week
            .iter()
            .filter(|e| matches!(e.moment, JournalMoment::Friendship(_)))
            .count();
        let finds = save
            .companion
            .scrapbook
            .iter()
            .filter(|r| now - r.first_at <= time::Duration::days(7))
            .count();
        let mut aside = vec![counted(week.len(), "moment", "moments")];
        if friendships > 0 {
            aside.push(counted(friendships, "new friendship", "new friendships"));
        }
        if finds > 0 {
            aside.push(counted(finds, "first find", "first finds"));
        }
        section(
            ui,
            "This week",
            Some(&aside.join(" · ")),
            "The last seven days of the journal, a dot for every moment.",
        );
        ui.add_space(4.0);
        journal::to_next_rule(ui);
        let (row, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 3.0 * pitch),
            egui::Sense::hover(),
        );
        let gap = 8.0;
        let cell_width = (row.width() - gap * 6.0) / 7.0;
        let painter = ui.painter().clone();
        let today = now.to_offset(offset).date();
        let began = save.created_at_utc.to_offset(offset).date();
        for index in 0..7 {
            let date = today - time::Duration::days(6 - index);
            let cell = egui::Rect::from_min_size(
                egui::pos2(
                    row.left() + index as f32 * (cell_width + gap),
                    row.top() + 2.0,
                ),
                egui::vec2(cell_width, row.height() - 6.0),
            );
            let faint = if date < began { 0.35 } else { 1.0 };
            let mut fills = journal::Fills::default();
            journal::stepped_box(
                &mut fills,
                cell,
                card_fill().gamma_multiply(faint),
                if date == today {
                    forest()
                } else {
                    line().gamma_multiply(0.55 * faint)
                },
            );
            if date == today {
                journal::stepped_outline(&mut fills, cell.expand(2.0), 2.0, forest());
            }
            let moments: Vec<_> = week
                .iter()
                .filter(|e| e.at.to_offset(offset).date() == date)
                .collect();
            for (dot, _) in moments.iter().enumerate().take(12) {
                let (col, line_no) = (dot % 6, dot / 6);
                fills.rect(
                    egui::Rect::from_min_size(
                        egui::pos2(
                            cell.left() + 6.0 + col as f32 * 7.0,
                            cell.bottom() - 10.0 - line_no as f32 * 7.0,
                        ),
                        egui::vec2(4.0, 4.0),
                    ),
                    forest().gamma_multiply(faint),
                );
            }
            fills.paint(&painter);
            let day_name = painter.layout_job(journal::kicker_job(
                &date.weekday().to_string()[..3],
                15.0 * scale,
                muted().gamma_multiply(faint),
            ));
            painter.galley(cell.left_top() + egui::vec2(6.0, 2.0), day_name, muted());
            let number = painter.layout_no_wrap(
                date.day().to_string(),
                formiga_pencil::hand_bold(18.0 * scale),
                ink().gamma_multiply(faint),
            );
            painter.galley(
                egui::pos2(cell.right() - 6.0 - number.size().x, cell.top() + 2.0),
                number,
                ink(),
            );
            if let Some(landmark) = moments.iter().find(|e| e.moment.is_landmark()) {
                shell.bubble(
                    &painter,
                    egui::pos2(cell.right() - 23.0, cell.bottom() - 22.0),
                    16.0,
                    paper::moment_icon(&landmark.moment),
                );
            } else if moments.is_empty() && date >= began {
                let quiet = painter.layout_no_wrap(
                    "quiet".to_owned(),
                    egui::FontId::proportional(15.0 * scale),
                    muted(),
                );
                painter.galley(
                    egui::pos2(cell.left() + 6.0, cell.bottom() - 4.0 - quiet.size().y),
                    quiet,
                    muted(),
                );
            }
        }
    }
}

/// Today's tally: a picture, a count and a word for each kind of thing that happened, and the
/// keepsakes first found today, each in a frame with who found it.
fn tally(ui: &mut Ui, shell: &mut Shell, save: &SaveFile, today: &Today<'_>) {
    let scale = journal::text_scale(ui);
    let kicker_size = journal::kicker_size(scale);
    ui.label(journal::kicker_job("Today's tally", kicker_size, forest()));
    let counts = today_tally(&today.moments);
    if counts.is_empty() && today.found.is_empty() {
        ui.label("Nothing to count yet today. Give them time!");
        return;
    }
    let half = ((ui.available_width() - 10.0) / 2.0).floor();
    egui::Grid::new("today-tally")
        .num_columns(2)
        .min_col_width(half)
        .spacing(egui::vec2(10.0, 6.0))
        .show(ui, |ui| {
            for (index, (icon, n, word)) in counts.iter().enumerate() {
                ui.horizontal(|ui| {
                    let (spot, _) =
                        ui.allocate_exact_size(egui::vec2(34.0, 32.0), egui::Sense::hover());
                    shell.bubble(ui.painter(), spot.min, 32.0, *icon);
                    ui.label(bold(n.to_string(), 18.0 * scale));
                    ui.label(*word);
                });
                if index % 2 == 1 {
                    ui.end_row();
                }
            }
        });
    if today.found.is_empty() {
        return;
    }
    ui.add_space(6.0);
    let (rule, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 4.0), egui::Sense::hover());
    let mut fills = journal::Fills::default();
    journal::dashes(
        &mut fills,
        rule.left_center(),
        rule.right_center(),
        6.0,
        4.0,
        2.0,
        gold(),
    );
    fills.paint(ui.painter());
    ui.label(journal::kicker_job("Found today!", kicker_size, forest()));
    let atlas = shell.trinket_atlas(ui, save);
    for variant in today.found.iter().take(3) {
        ui.horizontal(|ui| {
            let (frame, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
            let mut fills = journal::Fills::default();
            journal::stepped_box(&mut fills, frame, faint(), line().gamma_multiply(0.55));
            fills.paint(ui.painter());
            ui.painter().image(
                atlas.id(),
                frame.shrink(4.0),
                Shell::trinket_uv(*variant),
                Color32::WHITE,
            );
            let finder = save
                .companion
                .scrapbook
                .iter()
                .find(|record| record.variant == *variant)
                .map(|record| record.finder_name.as_str())
                .filter(|name| !name.is_empty());
            ui.label(match finder {
                Some(name) => format!("{name}'s first find!"),
                None => "A first find!".to_owned(),
            });
        });
    }
    if today.found.len() > 3 {
        ui.label(RichText::new(format!("…and {} more.", today.found.len() - 3)).color(muted()));
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

/// How many of each kind of moment a day held, in a fixed order: the picture it is jotted with,
/// the count, and the word for it.
fn today_tally(moments: &[&JournalEntry]) -> Vec<(formiga_core::BubbleIcon, usize, &'static str)> {
    let count = |test: &dyn Fn(&JournalMoment) -> bool| {
        moments.iter().filter(|entry| test(&entry.moment)).count()
    };
    let mut tally = Vec::new();
    for (test, one, many) in [
        (
            &(|m: &JournalMoment| matches!(m, JournalMoment::Arrival))
                as &dyn Fn(&JournalMoment) -> bool,
            "arrival",
            "arrivals",
        ),
        (
            &|m: &JournalMoment| matches!(m, JournalMoment::Friendship(_)),
            "new friendship",
            "new friendships",
        ),
        (
            &|m: &JournalMoment| matches!(m, JournalMoment::Discovery),
            "discovery",
            "discoveries",
        ),
        (
            &|m: &JournalMoment| matches!(m, JournalMoment::Ritual(_)),
            "shared moment",
            "shared moments",
        ),
        (
            &|m: &JournalMoment| matches!(m, JournalMoment::Visit(_) | JournalMoment::Revisit(..)),
            "visitor",
            "visitors",
        ),
        (
            &|m: &JournalMoment| matches!(m, JournalMoment::Preference(_)),
            "new preference",
            "new preferences",
        ),
        (
            &|m: &JournalMoment| {
                matches!(
                    m,
                    JournalMoment::Object(_)
                        | JournalMoment::Decoration(_)
                        | JournalMoment::Unlocked(_)
                )
            },
            "new keepsake",
            "new keepsakes",
        ),
        (
            &|m: &JournalMoment| matches!(m, JournalMoment::Habit(_)),
            "new habit",
            "new habits",
        ),
        (
            &|m: &JournalMoment| matches!(m, JournalMoment::Wonder(_)),
            "new wonder",
            "new wonders",
        ),
        (
            &|m: &JournalMoment| matches!(m, JournalMoment::Trip),
            "trip away",
            "trips away",
        ),
    ] {
        let n = count(test);
        if n > 0 {
            let icon = moments
                .iter()
                .find(|entry| test(&entry.moment))
                .map_or(formiga_core::BubbleIcon::Sparkle, |entry| {
                    paper::moment_icon(&entry.moment)
                });
            tally.push((icon, n, if n == 1 { one } else { many }));
        }
    }
    tally
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
