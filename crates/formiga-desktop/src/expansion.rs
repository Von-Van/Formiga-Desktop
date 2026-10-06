//! The slot a companion app plugs into.
//!
//! Formiga's companion apps are separate programs that Desktop visits the same way: it looks for
//! the app when it starts and then at most every ten minutes, writes a fresh session directory
//! with a marker beside it ([`files`]), starts the app on that directory, waits for it on a
//! thread that sleeps until it exits, reads back what it left, and says in the app's own words why
//! it said no. An app fills in an [`Expansion`] with its names, its folder and those words, and
//! everything here is done for it as for any other. What a visit carries, what Desktop keeps from
//! it, and how it looks on the desktop stay the app's own.
//!
//! Formiga Hill ([`crate::hill::HILL`]) and Formiga Home ([`crate::house::HOME`]) are visited
//! through it.

pub mod files;

use crate::app::UserEvent;
use crate::platform::companion_app::{self, AppInstall, AppNames};
use formiga_core::CreatureId;
use formiga_expansion_rulebook::AckRefusal;
use std::path::Path;
use std::process::Child;
use std::time::{Duration, Instant};
use winit::event_loop::EventLoopProxy;

/// How often Desktop looks again for an app having been installed or removed.
const RECHECK: Duration = Duration::from_secs(10 * 60);

/// What a companion app fills in to be visited.
pub struct Expansion {
    /// Its name, as the owner reads it.
    pub name: &'static str,
    /// The names it is found by, as its contract's `discovery` module gives them.
    pub app: AppNames,
    /// The flag it is started with, ahead of the session directory.
    pub launch_argument: &'static str,
    /// For development only: with [`AppNames::path_override_env`] naming a stand-in, a visit
    /// begins this many seconds after Desktop starts, once, as if the owner had asked for it.
    /// Ignored without the override, so it can never start a visit in an installed app.
    pub start_after_env: &'static str,
    /// Where its visits' files are kept.
    pub folder: files::Folder,
    /// How it is spoken of.
    pub words: Words,
}

/// The words an app is spoken of in, each said after its name.
pub struct Words {
    /// What its contract's version is called: `travel` in "reads travel version 2".
    pub contract: &'static str,
    /// What updating it would let the colony do: "take the colony there".
    pub update_to: &'static str,
    /// What it said when it was busy: "is already hosting a colony".
    pub busy: &'static str,
    /// What it said when it could not read what it was given.
    pub unreadable: &'static str,
    /// What it said when it turned a visit away for any other reason.
    pub declined: &'static str,
    /// What the owner is told when another visit would need someone this app has: the whole
    /// sentence, said on its own.
    pub occupied: &'static str,
}

/// Who a companion app has while a visit to it is open, as far as any other visit is concerned.
/// A creature is away in one app at a time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Holding {
    Nobody,
    /// The whole colony, as on a trip to Formiga Hill.
    Everyone,
    /// These creatures, as a household gone indoors in Formiga Home.
    These(Vec<CreatureId>),
}

impl Holding {
    /// Whether a visit that needs `wants`, or everyone when it is `None`, would need someone held
    /// here. A visit that needs everyone waits for every other visit to end.
    pub fn holds_any(&self, wants: Option<&[CreatureId]>) -> bool {
        match (self, wants) {
            (Self::Nobody, _) => false,
            (Self::Everyone, _) | (Self::These(_), None) => true,
            (Self::These(held), Some(wants)) => held.iter().any(|id| wants.contains(id)),
        }
    }
}

/// Why a visit to `asking` cannot have `wants` (everyone, when `None`) just now, in the words of the
/// first other app in `holdings` that has any of them; `None` when nobody it needs is away.
pub fn busy_elsewhere(
    asking: &Expansion,
    wants: Option<&[CreatureId]>,
    holdings: &[(&'static Expansion, Holding)],
) -> Option<&'static str> {
    holdings
        .iter()
        .find(|(app, holding)| !std::ptr::eq(*app, asking) && holding.holds_any(wants))
        .map(|(app, _)| app.words.occupied)
}

/// An app's own reason for turning a visit away, as its acknowledgement gave it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub reason: AckRefusal,
    /// The app's own version, to say which copy refused; empty if it did not say.
    pub version: String,
}

impl Expansion {
    /// The app, if it is installed: the development override first, then wherever the platform
    /// says it is.
    pub fn find(&self) -> Option<AppInstall> {
        companion_app::find(&self.app)
    }

    /// Start the app for the visit whose session directory is `session`, with a thread that sleeps
    /// until it exits and then sends `exited` through the event loop.
    pub fn start(
        &self,
        install: &AppInstall,
        session: &Path,
        exited: UserEvent,
        proxy: EventLoopProxy<UserEvent>,
    ) -> std::io::Result<()> {
        let child = companion_app::launch(&install.path, self.launch_argument, session)?;
        self.watch(child, exited, proxy)
    }

    /// Wait for the app to exit on a thread of its own, which sleeps until then, and say so
    /// through the event loop.
    fn watch(
        &self,
        mut child: Child,
        exited: UserEvent,
        proxy: EventLoopProxy<UserEvent>,
    ) -> std::io::Result<()> {
        let name = self.name;
        std::thread::Builder::new()
            .name(format!("{}-watch", name.to_lowercase().replace(' ', "-")))
            .spawn(move || {
                if let Err(error) = child.wait() {
                    tracing::warn!(%error, "lost track of {name}");
                }
                let _ = proxy.send_event(exited);
            })?;
        Ok(())
    }

    /// Why `install` cannot take a visit written as version `needs` of the app's contract, said so
    /// that it can be acted on. `None` when it can, or when it does not say which versions it
    /// reads; the app then answers for itself once it has read the snapshot.
    pub fn incompatibility(&self, install: &AppInstall, needs: u32) -> Option<String> {
        let reads = install.reads?;
        (reads < needs).then(|| {
            let which = install.version.as_deref().map_or_else(
                || format!("This copy of {}", self.name),
                |version| format!("{} {version}", self.name),
            );
            format!(
                "{which} reads {} version {reads}, and this colony needs version {needs}. Update \
                 {} to {}.",
                self.words.contract, self.name, self.words.update_to
            )
        })
    }

    /// The app's refusal, said so the owner can do something about it.
    pub fn refusal_text(&self, refusal: &Refusal) -> String {
        let name = self.name;
        let who = if refusal.version.is_empty() {
            name.to_owned()
        } else {
            format!("{name} {}", refusal.version)
        };
        let words = &self.words;
        match refusal.reason {
            AckRefusal::UnsupportedVersion { reads } => format!(
                "{who} reads {} version {reads}, which is too old for this colony. Update {name} \
                 to {}.",
                words.contract, words.update_to
            ),
            AckRefusal::Busy => format!("{who} {}.", words.busy),
            AckRefusal::Invalid => format!("{who} {}.", words.unreadable),
            AckRefusal::Other => format!("{who} {}.", words.declined),
        }
    }
}

/// An app as Desktop keeps track of it: whether it is installed, when Desktop last looked, and
/// when a development run starts a visit by itself.
pub struct Slot {
    pub expansion: &'static Expansion,
    /// The app, while it is installed.
    pub install: Option<AppInstall>,
    looked: Option<Instant>,
    /// When a development run starts a visit by itself; see [`Expansion::start_after_env`].
    start_at: Option<Instant>,
}

impl Slot {
    pub fn new(expansion: &'static Expansion) -> Self {
        let start_at = std::env::var_os(expansion.app.path_override_env)
            .and(std::env::var(expansion.start_after_env).ok())
            .and_then(|seconds| seconds.parse::<f32>().ok())
            .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
            .map(|seconds| Instant::now() + Duration::from_secs_f32(seconds));
        Self {
            expansion,
            install: None,
            looked: None,
            start_at,
        }
    }

    /// Look for the app now, or only if it has been ten minutes since the last look. Returns
    /// whether it looked.
    pub fn look(&mut self, now: bool) -> bool {
        if !now && self.looked.is_some_and(|looked| looked.elapsed() < RECHECK) {
            return false;
        }
        let found = self.expansion.find();
        if found != self.install {
            let name = self.expansion.name;
            match &found {
                Some(install) => tracing::info!(
                    version = install.version.as_deref().unwrap_or("unknown"),
                    "{name} is installed"
                ),
                None => tracing::info!("{name} is not installed"),
            }
        }
        self.install = found;
        self.looked = Some(Instant::now());
        true
    }

    /// Whether a development run's visit is due, which it is once; never while `held`.
    pub fn start_due(&mut self, held: bool) -> bool {
        let due = !held && self.start_at.is_some_and(|at| Instant::now() >= at);
        if due {
            self.start_at = None;
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const SAMPLE_FOLDER: files::Folder = files::Folder {
        directory: "sample",
        marker_file: "visit.json",
        marker_format: "formiga.desktop.sample-visit",
        kept: &[],
    };

    static SAMPLE: Expansion = Expansion {
        name: "Formiga Sample",
        app: AppNames {
            path_override_env: "FORMIGA_SAMPLE_PATH_NEVER_SET",
            macos_bundle_id: "com.formiga.sample",
            macos_reads_key: "FormigaSampleVersion",
            windows_registry_key: "Software\\Formiga\\Sample",
            windows_path_value: "Path",
            windows_version_value: "Version",
            windows_reads_value: "SampleVersion",
        },
        launch_argument: "--formiga-sample",
        start_after_env: "FORMIGA_SAMPLE_VISIT_AFTER",
        folder: SAMPLE_FOLDER,
        words: Words {
            contract: "sample",
            update_to: "open the sample",
            busy: "is already open",
            unreadable: "could not read the sample",
            declined: "could not open the sample this time",
            occupied: "The sample is open just now.",
        },
    };

    static OTHER: Expansion = Expansion {
        name: "Formiga Other",
        folder: SAMPLE_FOLDER,
        words: Words {
            occupied: "Someone is at the other app just now.",
            ..SAMPLE.words
        },
        ..SAMPLE
    };

    fn install(version: Option<&str>, reads: Option<u32>) -> AppInstall {
        AppInstall {
            path: PathBuf::from("Formiga Sample.app"),
            version: version.map(str::to_owned),
            reads,
        }
    }

    #[test]
    fn an_older_app_is_told_exactly_what_it_lacks() {
        let older = install(Some("0.1.0"), Some(1));
        assert_eq!(SAMPLE.incompatibility(&older, 1), None);
        assert_eq!(
            SAMPLE.incompatibility(&older, 2).as_deref(),
            Some(
                "Formiga Sample 0.1.0 reads sample version 1, and this colony needs version 2. \
                 Update Formiga Sample to open the sample."
            )
        );
        assert_eq!(
            SAMPLE
                .incompatibility(&install(None, Some(1)), 2)
                .as_deref(),
            Some(
                "This copy of Formiga Sample reads sample version 1, and this colony needs \
                 version 2. Update Formiga Sample to open the sample."
            )
        );
        assert_eq!(
            SAMPLE.incompatibility(&install(Some("0.1.0"), None), 9),
            None,
            "an app that does not say answers for itself"
        );
    }

    #[test]
    fn every_refusal_is_said_in_the_apps_own_words() {
        let said = |reason, version: &str| {
            SAMPLE.refusal_text(&Refusal {
                reason,
                version: version.to_owned(),
            })
        };
        assert_eq!(
            said(AckRefusal::UnsupportedVersion { reads: 0 }, "0.1.0"),
            "Formiga Sample 0.1.0 reads sample version 0, which is too old for this colony. \
             Update Formiga Sample to open the sample."
        );
        assert_eq!(
            said(AckRefusal::Busy, ""),
            "Formiga Sample is already open."
        );
        assert_eq!(
            said(AckRefusal::Invalid, "0.2.0"),
            "Formiga Sample 0.2.0 could not read the sample."
        );
        assert_eq!(
            said(AckRefusal::Other, ""),
            "Formiga Sample could not open the sample this time."
        );
    }

    #[test]
    fn a_development_visit_starts_only_with_a_stand_in_and_only_once() {
        let mut slot = Slot::new(&SAMPLE);
        assert!(
            !slot.start_due(false),
            "without the override nothing starts by itself"
        );
        slot.start_at = Some(Instant::now());
        assert!(!slot.start_due(true), "not while held");
        assert!(slot.start_due(false));
        assert!(!slot.start_due(false), "once");
    }

    #[test]
    fn a_creature_is_away_in_one_app_at_a_time() {
        let (a, b, c): (CreatureId, CreatureId, CreatureId) = (1, 2, 3);
        let household = [(&OTHER, Holding::These(vec![a, b]))];
        assert_eq!(
            busy_elsewhere(&SAMPLE, Some(&[b, c]), &household),
            Some("Someone is at the other app just now.")
        );
        assert_eq!(busy_elsewhere(&SAMPLE, Some(&[c]), &household), None);
        assert_eq!(
            busy_elsewhere(&SAMPLE, None, &household),
            Some("Someone is at the other app just now."),
            "a visit that needs everyone waits"
        );
        let colony = [(&OTHER, Holding::Everyone)];
        assert!(busy_elsewhere(&SAMPLE, Some(&[c]), &colony).is_some());
        let nobody = [(&OTHER, Holding::Nobody)];
        assert_eq!(busy_elsewhere(&SAMPLE, None, &nobody), None);
        assert_eq!(
            busy_elsewhere(&SAMPLE, None, &[(&SAMPLE, Holding::Everyone)]),
            None,
            "an app's own visit is its own business"
        );
    }
}
