//! The save format's tests, one file per topic. The colony builders more than one topic starts
//! from live here: `example_save`, the plain colony file most of them vary one field of, and
//! `colony_in_the_middle_of_everything`, a busy colony caught with everything under way at once.
use super::*;
use crate::{ArrivalState, Settings};
use time::macros::datetime;

mod bounds;
mod frozen;
mod privacy;
mod recovery;
mod saving;
mod versions;

/// How long the busy colony is given to have everything happening at once. The scene is
/// deterministic, so it always reaches that moment at the same tick.
const BUSY_STEPS: u64 = 4_000;

const LEDGE_WINDOW: crate::WindowKey = 0x00C0_FFEE;

const DRIFTING_WINDOW: crate::WindowKey = 0x00BA_DBED;

/// A colony forty days old with a shared ledge, a companion crossing the floor, a window
/// sliding about, and a cursor sweeping past. It is driven until all of that is true at once,
/// and returns the tick it stopped on so the desktop it saw can be reconstructed.
pub(super) fn colony_in_the_middle_of_everything() -> (crate::World, u64) {
    let created = datetime!(2026-01-01 0:00 UTC);
    let now = created + time::Duration::days(40);
    let mut desktop = observed_desktop(0);
    let mut world = crate::World::new([92; 32], created, &desktop);
    world.tick(now, 0.05, &desktop);
    // A colony that has played Formiga Battle, against a friend and against the visitors.
    let player = world.save.creatures[0].id;
    world.save.battle.record = crate::BattleRecord {
        circuit_points: 15,
        unlocked: vec!["cup.woodland".to_owned()],
        cups: vec![crate::BattleCupWins {
            cup: "cup.backyard".to_owned(),
            wins: 3,
        }],
        history: vec![crate::BattleMatchSummary {
            finished_at_utc: now,
            mode: crate::BattleMode::Friend,
            won: Some(true),
            ended_by: "capture".to_owned(),
            turns: 8,
            team: vec![player],
            opponent: Some("Sam".to_owned()),
            trio: Some("7a1e5c3b9d2f4068".to_owned()),
            opponent_trio: Some("The Puddle Jumpers".to_owned()),
            opponent_pennant: Some(crate::BattlePennant {
                shape: "swallowtail".to_owned(),
                field: "sky".to_owned(),
                symbol: "wave".to_owned(),
                trim: "plain".to_owned(),
            }),
        }],
        notebook: vec![crate::BattleNotebookPage {
            creature_id: player,
            matches: 1,
            wins: 1,
            recent: vec![crate::BattleShowing {
                steps: 12,
                ..Default::default()
            }],
            moments: vec!["Held firm against two at once".to_owned()],
        }],
        applied: vec!["0123456789abcdef0123456789abcdef".to_owned()],
        handler: Some(crate::BattleHandlerCard {
            name: "Jo".to_owned(),
            club: Some("Hedge End Club".to_owned()),
            pennant: crate::BattlePennant {
                shape: "pennon".to_owned(),
                field: "forest".to_owned(),
                symbol: "acorn".to_owned(),
                trim: "stitched".to_owned(),
            },
        }),
        trios: vec![crate::BattleTrio {
            id: "7a1e5c3b9d2f4068".to_owned(),
            name: "The Thistle Three".to_owned(),
            members: vec![player, player + 1, player + 2],
            pennant: None,
            retired: false,
            formed_at_utc: now,
            members_since_utc: now,
            friends: crate::BattleTally {
                played: 1,
                won: 1,
                drawn: 0,
            },
            visitors: crate::BattleTally::default(),
            cups: Vec::new(),
            lineups: vec![crate::BattleLineup {
                members: vec![player, player + 1, player + 3],
                until_utc: now,
            }],
        }],
        handler_key: Some("5f0c2a7e9b1d4c3a8e6f0b2d4a9c7e15".to_owned()),
    };
    world.keep_battle_match(
        &["They won the Backyard Cup.".to_owned()],
        &["battle.trophy.backyard_cup".to_owned()],
        now,
    );
    // Out of the house, so the colony is living on the desktop rather than walking home.
    world.save.home.active_since_utc = None;
    world.save.home.last_disappeared_utc = Some(now);
    world.save.ritual.next_at_utc = now + time::Duration::days(1);
    // A companion its owner would rather keep on the floor, reshaped in Formiga Farm.
    world.save.creatures[0].leaning = crate::RoamingLeaning::FloorDweller;
    let mut sculpt = crate::forms::Sculpt::starter(crate::forms::Plan::Floater);
    sculpt.markings.push(crate::forms::Marking::new(
        crate::forms::MarkingKind::Spots,
        [240, 236, 228],
    ));
    let reshaped = &mut world.save.creatures[0];
    reshaped.appearance = crate::forms::Design {
        form: crate::forms::Form::Sculpted { sculpt },
        face: reshaped.appearance.face,
    }
    .genome(&reshaped.appearance);
    reshaped.origin.design = reshaped.appearance.design;
    world.save.settings.habitat = crate::HabitatPolicy {
        preset: crate::HabitatPreset::Custom,
        zones: vec![crate::HabitatZone {
            id: 1,
            display: crate::DisplayKey([1; 16]),
            normalized_bounds: crate::DesktopRect {
                x: 0.0,
                y: 0.05,
                width: 1.0,
                height: 0.95,
            },
            kind: crate::HabitatZoneKind::Allowed,
            enabled: true,
        }],
    };
    for (index, creature) in world.save.creatures.iter_mut().enumerate() {
        creature.state.arrival_delay_secs = 0.0;
        creature.state.action = crate::ActionKind::Idle;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 100.0;
        creature.state.drives = crate::Drives::default();
        creature.state.facing_right = index % 2 == 0;
        creature.personality.curiosity = 1.0;
        creature.personality.sociability = 1.0;
        creature.personality.playfulness = 1.0;
        creature.personality.cursor_interest = 1.0;
        creature.personality.boldness = if index == 1 { 1.0 } else { 0.4 };
        creature.personality.window_tolerance = creature.personality.boldness;
        if index < 2 {
            creature.state.surface = crate::SurfaceAttachment {
                kind: crate::SurfaceKind::WindowLedge,
                monitor_id: 1,
                window_key: Some(LEDGE_WINDOW),
                relative_x: (150.0 + index as f32 * 80.0) / 517.3125,
            };
            creature.state.position = crate::Point {
                x: 300.3125 + 150.0 + index as f32 * 80.0,
                y: 600.3125,
            };
        } else {
            creature.state.surface = crate::SurfaceAttachment {
                kind: crate::SurfaceKind::ScreenFloor,
                monitor_id: 1,
                window_key: None,
                relative_x: 0.5,
            };
            creature.state.position = crate::Point {
                x: 980.0 + (index - 2) as f32 * 90.0,
                y: 846.0,
            };
        }
    }
    world.tick(now, 0.05, &desktop);
    world.drain_events().for_each(drop);
    // Two observers fed the same scans, so the test can say what the colony is looking at
    // without reaching into it.
    let mut geometry = crate::attention::GeometryObserver::default();
    let mut cursor = crate::cursor::CursorObserver::default();
    geometry.update(&desktop, 0.05, true);
    cursor.update(&desktop, 0.05, true);
    for step in 1..=BUSY_STEPS {
        desktop = observed_desktop(step);
        world.tick(
            now + time::Duration::milliseconds(step as i64 * 50),
            0.05,
            &desktop,
        );
        world.drain_events().for_each(drop);
        geometry.update(&desktop, 0.05, true);
        cursor.update(&desktop, 0.05, true);
        let together = world.save.creatures.iter().any(|creature| {
            matches!(
                creature.state.action,
                crate::ActionKind::Greet
                    | crate::ActionKind::Follow
                    | crate::ActionKind::SocialPlay
                    | crate::ActionKind::SoloPlay
                    | crate::ActionKind::Sprint
            )
        });
        let travelling = world.save.creatures.iter().any(|creature| {
            matches!(
                creature.state.action,
                crate::ActionKind::Traverse
                    | crate::ActionKind::ClimbWindow
                    | crate::ActionKind::RideWindow
                    | crate::ActionKind::Dangle
            )
        });
        let watching = world
            .save
            .creatures
            .iter()
            .any(|creature| creature.state.attention.is_some());
        if together
            && travelling
            && watching
            && geometry.signals().next().is_some()
            && cursor.cue().is_some()
        {
            // A companion with two little habits of its own, one of them in the journal,
            // and another in the middle of doing one.
            world.save.creatures[0].memory.habits =
                vec![crate::Habit::StretchesBeforeNaps, crate::Habit::WavesHello];
            let first = world.save.creatures[0].id;
            world.save.companion.remember(
                Some(first),
                crate::JournalMoment::Habit(crate::Habit::WavesHello),
                now,
            );
            // Two spots put down on the village ground.
            world
                .save
                .home
                .set_hangout(crate::HangoutKind::Blanket, Some(0.3125));
            world
                .save
                .home
                .set_hangout(crate::HangoutKind::Lookout, Some(0.8125));
            // A village arranged by hand: the cottage order written down when there were more
            // cottages to order, painted in a named palette, with a herb box planted.
            world.save.home.cottage_order =
                crate::house_owners(&world.save.creatures, &[]).as_slice()[1..].to_vec();
            world.save.home.palette = Some(crate::VillagePalette::Harbour);
            world
                .save
                .home
                .set_garden(crate::GardenKind::Herbs, Some(0.5625), now);
            // An ornament set out, a decoration hung, a keepsake chosen for the trees, and a
            // companion wearing a pin.
            world
                .save
                .home
                .set_ornament(crate::OrnamentKind::BirdBath, Some(0.6875));
            world
                .save
                .companion
                .remember_discovery(2, first, "Finder".into(), now);
            let mut hooks = [None; crate::TREE_HOOKS];
            hooks[3] = Some(2);
            world.save.home.set_tree_keepsakes(Some(hooks));
            world.save.creatures[0].accessory = Some(crate::Accessory::Pin(2));
            // And the colony house built as a pillow fort.
            let founder = world.save.creatures[0].id;
            world
                .save
                .home
                .set_house_style(founder, Some(crate::ShelterStyle::PillowFort));
            // And set right at the back of the ground.
            world
                .save
                .home
                .set_house_depth(founder, Some(crate::MAX_HOUSE_DEPTH));
            world.save.home.set_decoration(
                founder,
                crate::DecorationSlot::Eaves,
                Some(crate::ShelterDecorationKind::Banner),
            );
            // Another being towed out of the way in its sleep.
            world.save.creatures[0].state.nudge = Some(crate::SleepNudge::Towed {
                by: world.save.creatures[1].id,
            });
            let last = world.save.creatures.last_mut().expect("a colony");
            last.state.flourish = Some(crate::Flourish {
                habit: crate::Habit::LooksFoodOver,
                action: last.state.action,
                started_at: Some(0.25),
            });
            return (world, step);
        }
    }
    panic!("the colony never had a scene, a journey, a cursor, and a cue all at once");
}

pub(super) fn observed_desktop(step: u64) -> crate::DesktopSnapshot {
    // Odd fractions, so nothing that reached the file could have come from anywhere else.
    let drift = (step % 8) as f32 * 30.0;
    crate::DesktopSnapshot {
        monitors: vec![crate::MonitorInfo {
            id: 1,
            display_key: crate::DisplayKey([1; 16]),
            bounds: crate::DesktopRect {
                x: 0.0,
                y: 0.0,
                width: 1440.0,
                height: 900.0,
            },
            usable_bounds: crate::DesktopRect {
                x: 0.0,
                y: 24.0,
                width: 1440.0,
                height: 826.0,
            },
            scale_factor: 2.0,
            primary: true,
        }],
        windows: vec![
            crate::DesktopWindow {
                key: LEDGE_WINDOW,
                bounds: crate::DesktopRect {
                    x: 300.3125,
                    y: 600.3125,
                    width: 517.3125,
                    height: 233.3125,
                },
                z_order: 0,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            },
            crate::DesktopWindow {
                key: DRIFTING_WINDOW,
                bounds: crate::DesktopRect {
                    x: 902.3125 - drift,
                    y: 380.3125,
                    width: 421.3125,
                    height: 186.3125,
                },
                z_order: 1,
                visible: true,
                minimized: false,
                application: None,
                application_name: None,
            },
        ],
        cursor: crate::CursorSnapshot {
            position: crate::Point {
                x: 1301.3125 - drift,
                y: 96.8125,
            },
            velocity: crate::Point {
                x: -321.8125,
                y: 123.8125,
            },
            available: true,
        },
        window_sample: Some(crate::WindowSample {
            monotonic_millis: step * 50,
            reliable: true,
        }),
        cursor_sample_millis: Some(step * 50),
        ..Default::default()
    }
}

pub(super) fn example_save() -> SaveFile {
    let mut home = crate::ColonyHome::default();
    home.unlocks.next_at_utc = datetime!(2026-01-05 0:00 UTC);
    SaveFile {
        companion: crate::CompanionState::default(),
        save_version: crate::SAVE_VERSION,
        colony_seed: [1; 32],
        created_at_utc: datetime!(2026-01-01 0:00 UTC),
        maximum_seen_utc: datetime!(2026-01-01 0:00 UTC),
        arrival_state: ArrivalState::default(),
        home,
        settings: Settings::default(),
        creatures: Vec::new(),
        relationships: Vec::new(),
        tallies: Vec::new(),
        ritual: crate::RitualState {
            next_at_utc: datetime!(2026-01-02 0:00 UTC),
            ..crate::RitualState::default()
        },
        objects: crate::ColonyObjectState {
            next_at_utc: datetime!(2026-01-04 0:00 UTC),
            ..crate::ColonyObjectState::default()
        },
        visitors: crate::VisitorState::default(),
        finds_today: crate::FindsToday::default(),
        day_book: crate::DayBook::default(),
        trips: crate::TripLog::default(),
        battle: crate::BattleLog::default(),
    }
}

/// A shared code for somebody who has never lived here.
pub(super) fn a_friends_code() -> crate::SharedCreatureSeed {
    crate::SharedCreatureSeed {
        source_colony_seed: [70; 32],
        source_generation: 1,
        design: Some(crate::CreatureDesign::generated([70; 32], 1, None)),
    }
}

/// A form sculpted in Formiga Farm on one of the newer body plans.
pub(super) fn sculpted(
    creature: &crate::Creature,
    plan: crate::forms::Plan,
) -> crate::AppearanceGenome {
    crate::forms::Design {
        form: crate::forms::Form::Sculpted {
            sculpt: crate::forms::Sculpt::starter(plan),
        },
        face: creature.appearance.face,
    }
    .genome(&creature.appearance)
}
