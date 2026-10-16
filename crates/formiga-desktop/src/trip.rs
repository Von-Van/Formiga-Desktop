//! The colony leaving the desktop by train for a companion app that takes all of it, and coming
//! home the same way: a trip to Formiga Hill, or a match in Formiga Battle. Each app keeps its own
//! files and its own reasons; the train, and when the app is started, are the same for both.

use crate::hill::scene::TrainScene;
use crate::platform::companion_app::AppInstall;

/// Something to tell the owner once the colony is home.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HomecomingNote {
    pub text: String,
    /// It answers something they just did, so it is said in a dialog rather than only noted.
    pub asked_for: bool,
}

/// Where a trip is, with `O` the app's own open session. Runtime only: across a restart only the
/// app's marker survives.
#[derive(Default)]
pub enum TripState<O> {
    #[default]
    Idle,
    /// The train is in, and the colony is getting on. `app` is the app to start as the train
    /// pulls away, until it has been.
    Departing {
        open: O,
        scene: TrainScene,
        app: Option<AppInstall>,
    },
    /// The colony is in the app.
    Away { open: O },
    /// The colony is home again and getting off the train. The session is already closed.
    Returning {
        scene: TrainScene,
        note: Option<HomecomingNote>,
    },
}

/// What a tick of the train asks of the app.
#[derive(Debug)]
pub enum Step<O> {
    Nothing,
    /// Everyone is aboard and the train is pulling away: start the app now, so its own arrival
    /// follows on. If it cannot be, the colony comes home again.
    Start {
        app: AppInstall,
        open: O,
    },
    /// The train has left the desktop: the colony is away.
    Gone,
    /// Everyone is off the train: the colony is back to living on the desktop.
    Home(Option<HomecomingNote>),
}

impl<O: Clone> TripState<O> {
    /// The trip under way for `open`: the train pulls in if there is anywhere to watch it, and
    /// otherwise the colony is away at once and the app is the caller's to start.
    pub fn leave(
        open: O,
        scene: Option<TrainScene>,
        app: AppInstall,
    ) -> (Self, Option<AppInstall>) {
        match scene {
            Some(scene) => (
                Self::Departing {
                    open,
                    scene,
                    app: Some(app),
                },
                None,
            ),
            None => (Self::Away { open }, Some(app)),
        }
    }

    /// Whether the colony is not simply living on the desktop: the world is held still and the
    /// overlay shows only the trip.
    pub fn holds_world(&self) -> bool {
        !matches!(self, Self::Idle)
    }

    /// The session still open, while there is one.
    pub fn open(&self) -> Option<&O> {
        match self {
            Self::Departing { open, .. } | Self::Away { open } => Some(open),
            _ => None,
        }
    }

    /// Whether the app has not been started yet: the colony is still boarding.
    pub fn boarding(&self) -> bool {
        matches!(self, Self::Departing { app: Some(_), .. })
    }

    pub fn scene(&self) -> Option<&TrainScene> {
        match self {
            Self::Departing { scene, .. } | Self::Returning { scene, .. } => Some(scene),
            _ => None,
        }
    }

    /// The colony on its way home, by `scene` when anyone can see it; with no scene, it is home
    /// already, and the note is the caller's to say.
    pub fn come_home(
        &mut self,
        scene: Option<TrainScene>,
        note: Option<HomecomingNote>,
    ) -> Option<Option<HomecomingNote>> {
        match scene {
            Some(scene) => {
                *self = Self::Returning { scene, note };
                None
            }
            None => {
                *self = Self::Idle;
                Some(note)
            }
        }
    }

    /// One tick of the train, in place of the world's own: it moves on, and a scene that has
    /// played out moves the trip on.
    pub fn advance(&mut self, seconds: f32) -> Step<O> {
        match self {
            Self::Departing { scene, .. } | Self::Returning { scene, .. } => scene.advance(seconds),
            _ => return Step::Nothing,
        }
        if let Self::Departing {
            open,
            scene,
            app: app @ Some(_),
        } = self
            && scene.pulling_away()
        {
            let app = app.take().expect("matched Some");
            return Step::Start {
                app,
                open: open.clone(),
            };
        }
        if !self.scene().is_some_and(TrainScene::finished) {
            return Step::Nothing;
        }
        match std::mem::take(self) {
            Self::Departing { open, .. } => {
                *self = Self::Away { open };
                Step::Gone
            }
            Self::Returning { note, .. } => Step::Home(note),
            other => {
                *self = other;
                Step::Nothing
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formiga_core::{DesktopRect, DesktopSnapshot, DisplayKey, MonitorInfo, SaveFile, World};
    use std::path::PathBuf;
    use time::macros::datetime;

    fn colony() -> (SaveFile, Vec<MonitorInfo>) {
        let bounds = DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        };
        let monitors = vec![MonitorInfo {
            id: 1,
            display_key: DisplayKey([1; 16]),
            bounds,
            usable_bounds: bounds,
            scale_factor: 2.0,
            primary: true,
        }];
        let desktop = DesktopSnapshot {
            monitors: monitors.clone(),
            ..DesktopSnapshot::default()
        };
        let now = datetime!(2026-10-16 9:00 UTC);
        let mut world = World::new([8; 32], now, &desktop);
        world
            .add_designed_adult([9; 32], None, now, &desktop)
            .unwrap();
        (world.save, monitors)
    }

    fn app() -> AppInstall {
        AppInstall {
            path: PathBuf::from("Formiga Battle.app"),
            version: Some("0.67.9".to_owned()),
            reads: Some(3),
        }
    }

    /// Ticks of the overlay's quickest pace until the trip asks for something.
    fn run(trip: &mut TripState<u8>) -> (Step<u8>, f32) {
        let mut seconds = 0.0;
        loop {
            seconds += 0.05;
            match trip.advance(0.05) {
                Step::Nothing if seconds < 120.0 => {}
                step => return (step, seconds),
            }
        }
    }

    /// The app is started once, as everyone is aboard and the train pulls away, and not before:
    /// it is the train that takes the colony, and the app opens with them already riding.
    #[test]
    fn the_app_starts_as_the_train_pulls_away_and_the_colony_is_then_away() {
        let (save, monitors) = colony();
        let (mut trip, now) = TripState::leave(7, TrainScene::departure(&save, &monitors), app());
        assert!(
            now.is_none(),
            "with a train to watch, nothing is started yet"
        );
        assert!(trip.boarding() && trip.holds_world());

        let (step, boarded) = run(&mut trip);
        assert!(
            matches!(&step, Step::Start { app: started, open: 7 } if *started == app()),
            "{step:?}"
        );
        assert!(boarded > 1.0, "everyone walked to the train first");
        assert!(!trip.boarding() && trip.open() == Some(&7));

        let (step, _) = run(&mut trip);
        assert!(matches!(step, Step::Gone), "{step:?}");
        assert!(matches!(trip, TripState::Away { open: 7 }));
        assert!(trip.scene().is_none() && trip.holds_world());
        assert!(matches!(trip.advance(1.0), Step::Nothing));
    }

    /// With nothing on the desktop to watch, the colony is away at once and the app is started
    /// straight off.
    #[test]
    fn with_no_train_to_watch_the_app_starts_at_once() {
        let (mut save, monitors) = colony();
        save.settings.visible = false;
        let (trip, now) = TripState::leave(7, TrainScene::departure(&save, &monitors), app());
        assert_eq!(now, Some(app()));
        assert!(matches!(trip, TripState::Away { open: 7 }));
    }

    /// The app gone, the train brings everyone home, says what there is to say once they are, and
    /// leaves the colony to live on the desktop again.
    #[test]
    fn the_train_brings_everyone_home_and_then_the_note_is_said() {
        let (save, monitors) = colony();
        let mut trip = TripState::Away { open: 7 };
        let note = HomecomingNote {
            text: "Formiga Battle could not open a match this time.".to_owned(),
            asked_for: true,
        };
        let home = trip.come_home(TrainScene::arrival(&save, &monitors), Some(note.clone()));
        assert!(home.is_none(), "the note waits for the train");
        assert!(trip.holds_world() && trip.open().is_none());
        let (step, _) = run(&mut trip);
        assert!(
            matches!(&step, Step::Home(Some(said)) if *said == note),
            "{step:?}"
        );
        assert!(!trip.holds_world());

        let mut trip = TripState::Away { open: 7 };
        assert_eq!(trip.come_home(None, None), Some(None));
        assert!(!trip.holds_world());
    }
}
