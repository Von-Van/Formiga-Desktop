//! What a change made in the notebook will do, said the way it will look on the desktop.
//!
//! Formiga's behaviour is subtle on purpose, and subtlety hides cause and effect: a leaning
//! chosen on a companion's page, a routine set for the evenings, a cushion put down at home all
//! change what the colony does, but quietly, and later. So whenever a change in the notebook
//! changes what the companions will do, the footer says so once, in the notebook's own words —
//! "Moss will now wander farther from home", "Relax begins at 22:00 on weekdays" — and nothing
//! more: no numbers, no popups, nothing that stays.
//!
//! Everything here is a pure function of what changed, so every sentence is tested.

use formiga_core::{
    GardenKind, HangoutKind, HomeCorner, RoamingLeaning, RoutineSchedule, Settings,
};
use time::OffsetDateTime;

/// What applying preferences changes about how the colony behaves, said the way it will look on
/// the desktop: "Applied · they'll keep off window ledges · they'll hold still". `None` when
/// nothing the companions do changes.
pub(crate) fn preferences_applied(before: &Settings, after: &Settings) -> Option<String> {
    let mut said = Vec::new();
    let mut say = |changed: bool, on: &'static str, off: &'static str, value: bool| {
        if changed {
            said.push(if value { on } else { off });
        }
    };
    say(
        before.visible != after.visible,
        "the colony is back on your desktop",
        "the colony is hidden; they carry on out of sight",
        after.visible,
    );
    say(
        before.paused != after.paused,
        "everyone holds still until you resume",
        "everyone is moving again",
        after.paused,
    );
    say(
        before.window_ledges != after.window_ledges,
        "they may climb onto window ledges again",
        "they'll keep off window ledges and come down",
        after.window_ledges,
    );
    say(
        before.cursor_reactions != after.cursor_reactions,
        "they'll notice your cursor again",
        "they'll ignore your cursor",
        after.cursor_reactions,
    );
    say(
        before.direct_manipulation != after.direct_manipulation,
        "you can pet and carry them again",
        "clicks pass straight through them",
        after.direct_manipulation,
    );
    say(
        before.reduce_motion != after.reduce_motion,
        "motion is reduced: fewer bounces and no page turns",
        "full motion is back",
        after.reduce_motion,
    );
    say(
        before.fullscreen_app_occlusion != after.fullscreen_app_occlusion,
        "they'll hide behind full-screen apps",
        "they'll stay visible over full-screen apps",
        after.fullscreen_app_occlusion,
    );
    if before.display_scale != after.display_scale {
        said.push(match after.display_scale {
            2 => "they're drawn small",
            4 => "they're drawn large",
            _ => "they're drawn at medium size",
        });
    }
    if before.habitat != after.habitat {
        said.push("anyone outside the new habitat walks back inside");
    }
    if before.application_occlusion_rules != after.application_occlusion_rules {
        said.push("the chosen apps' windows now cover them");
    }
    (!said.is_empty()).then(|| format!("Applied · {}", said.join(" · ")))
}

/// A companion given a new roaming leaning.
pub(crate) fn leaning(name: &str, before: RoamingLeaning, after: RoamingLeaning) -> String {
    use RoamingLeaning::*;
    match (before, after) {
        (Homebody, Anywhere) => format!("{name} will now wander farther from home"),
        (Homebody, Climber) => {
            format!("{name} will now wander farther from home, up onto window ledges")
        }
        (Homebody, FloorDweller) => {
            format!("{name} will now wander farther from home, keeping to the floor")
        }
        (_, Homebody) => format!("{name} will now keep close to the village and the floor"),
        (_, Climber) => format!("{name} will now seek out window ledges and stay up longer"),
        (_, FloorDweller) => {
            format!("{name} will now keep to the floor and come down from ledges sooner")
        }
        (_, Anywhere) => format!("{name} will go wherever it likes again"),
    }
}

/// The name of one of the two saved routines.
fn routine_name(preset: u8) -> &'static str {
    if preset == 0 { "Work" } else { "Relax" }
}

/// When something happens, from `local`: "at 22:00 today", "at 09:00 tomorrow", "at 09:00 on
/// Monday".
fn when(at: OffsetDateTime, local: OffsetDateTime) -> String {
    let day = if at.date() == local.date() {
        "today".to_owned()
    } else if Some(at.date()) == local.date().next_day() {
        "tomorrow".to_owned()
    } else {
        format!("on {}", at.weekday())
    };
    format!("at {:02}:{:02} {day}", at.hour(), at.minute())
}

/// A weekly routine saved by hand. `local` is now, in the reader's own time.
pub(crate) fn routine_saved(schedule: &RoutineSchedule, local: OffsetDateTime) -> String {
    if !schedule.enabled {
        return "The weekly routine is off · the colony keeps the preferences it has now"
            .to_owned();
    }
    match schedule.next_after(local) {
        Some((at, preset)) => format!(
            "{} begins {} · until then, the preferences stay as they are",
            routine_name(preset),
            when(at, local)
        ),
        None => "The weekly routine has no changes in it yet".to_owned(),
    }
}

/// Going back to the weekly routine after a change by hand.
pub(crate) fn routine_resumed(schedule: &RoutineSchedule, local: OffsetDateTime) -> String {
    let now = schedule
        .intended(local)
        .map_or("the routine".to_owned(), |preset| {
            format!("{} for now", routine_name(preset))
        });
    match schedule.next_after(local) {
        Some((at, preset)) => format!(
            "Back on the weekly routine · {now}, then {} {}",
            routine_name(preset),
            when(at, local)
        ),
        None => format!("Back on the weekly routine · {now}"),
    }
}

/// One of the two routines saved from the current preferences.
pub(crate) fn routine_kept(index: usize) -> String {
    format!(
        "Saved as your {} routine · a weekly routine can now move to it",
        routine_name(u8::from(index != 0))
    )
}

/// Everyone sent home for a quiet while, or let out again.
pub(crate) fn quiet(minutes: u16, until: Option<OffsetDateTime>) -> String {
    match (minutes, until) {
        (0, _) => "Quiet moment over · everyone is back to their usual adventures".to_owned(),
        (_, Some(until)) => format!(
            "Everyone is heading home to settle until {:02}:{:02}",
            until.hour(),
            until.minute()
        ),
        (minutes, None) => format!("Everyone is heading home to settle for {minutes} minutes"),
    }
}

/// A hangout spot put down, moved along, or picked up.
pub(crate) fn hangout(
    kind: HangoutKind,
    before: Option<f32>,
    after: Option<f32>,
) -> Option<String> {
    let name = kind.label().to_lowercase();
    match (before, after) {
        (None, Some(_)) => Some(format!(
            "The {name} is out · residents will go to it now and then when the houses are out"
        )),
        (Some(_), None) => Some(format!(
            "The {name} is put away · residents will find somewhere else for it"
        )),
        _ => None,
    }
}

/// A garden patch planted or dug up.
pub(crate) fn garden(kind: GardenKind, planted: bool) -> String {
    let name = kind.label().to_lowercase();
    if planted {
        format!(
            "The {name} is planted · it grows a stage every {} hours, and residents will tend it",
            kind.stage_hours()
        )
    } else {
        format!("The {name} is dug up · nobody will tend it now")
    }
}

/// The village moved to another corner, or to another display.
pub(crate) fn home_moved(corner: Option<HomeCorner>, display: Option<&str>) -> String {
    match (corner, display) {
        (Some(HomeCorner::BottomLeft), _) => {
            "The village now stands in the bottom-left corner".to_owned()
        }
        (Some(HomeCorner::BottomRight), _) => {
            "The village now stands in the bottom-right corner".to_owned()
        }
        (None, Some(display)) => format!("The village now stands on {display}"),
        (None, None) => "The village has moved".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::ScheduledTransition;
    use time::macros::datetime;

    #[test]
    fn a_new_leaning_says_what_the_companion_will_do_now() {
        use RoamingLeaning::*;
        assert_eq!(
            leaning("Moss", Homebody, Anywhere),
            "Moss will now wander farther from home"
        );
        assert_eq!(
            leaning("Moss", Anywhere, Homebody),
            "Moss will now keep close to the village and the floor"
        );
        for before in RoamingLeaning::ALL {
            for after in RoamingLeaning::ALL {
                let said = leaning("Moss", before, after);
                assert!(said.starts_with("Moss will "), "{said}");
                assert!(!said.contains("  ") && !said.ends_with('.'), "{said}");
            }
        }
    }

    fn evenings() -> RoutineSchedule {
        RoutineSchedule {
            enabled: true,
            transitions: vec![
                ScheduledTransition {
                    days: 0b001_1111,
                    minute: 9 * 60,
                    preset: 0,
                },
                ScheduledTransition {
                    days: 0b001_1111,
                    minute: 22 * 60,
                    preset: 1,
                },
            ],
            ..RoutineSchedule::default()
        }
    }

    #[test]
    fn a_routine_says_what_begins_next_and_when() {
        // A Friday afternoon: Relax at ten tonight, then nothing until Monday morning.
        let friday = datetime!(2026-10-02 15:30 UTC);
        assert_eq!(
            routine_saved(&evenings(), friday),
            "Relax begins at 22:00 today · until then, the preferences stay as they are"
        );
        assert_eq!(
            routine_saved(&evenings(), datetime!(2026-10-02 23:00 UTC)),
            "Work begins at 09:00 on Monday · until then, the preferences stay as they are"
        );
        assert_eq!(
            routine_saved(&evenings(), datetime!(2026-10-01 23:00 UTC)),
            "Work begins at 09:00 tomorrow · until then, the preferences stay as they are"
        );
        assert_eq!(
            routine_resumed(&evenings(), friday),
            "Back on the weekly routine · Work for now, then Relax at 22:00 today"
        );
        let mut off = evenings();
        off.enabled = false;
        assert!(routine_saved(&off, friday).starts_with("The weekly routine is off"));
    }

    #[test]
    fn a_quiet_while_says_until_when() {
        assert_eq!(
            quiet(30, Some(datetime!(2026-10-02 22:05 UTC))),
            "Everyone is heading home to settle until 22:05"
        );
        assert!(quiet(0, None).starts_with("Quiet moment over"));
    }

    #[test]
    fn only_putting_out_and_putting_away_change_what_residents_do() {
        assert!(hangout(HangoutKind::Cushion, None, Some(0.3)).is_some());
        assert!(hangout(HangoutKind::Cushion, Some(0.3), None).is_some());
        assert_eq!(hangout(HangoutKind::Cushion, Some(0.3), Some(0.6)), None);
        assert!(garden(GardenKind::Herbs, true).contains("every 5 hours"));
    }
}
