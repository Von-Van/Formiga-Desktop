//! A companion app's visits on Desktop's side, kept the same way for every app.
//!
//! Each app has a directory of its own in the data directory, beside the colony and never inside
//! it (`travel/` for Formiga Hill), holding:
//!
//! - its marker, while a visit is open: which session it is, and what the app's side needs to
//!   finish the visit cleanly after Desktop has itself been restarted. It holds nothing else
//!   about the colony.
//! - `<session>/`, the session directory the app is given: what Desktop wrote for it, and whatever
//!   it writes back.
//! - any file of Desktop's own that outlasts every visit, as [`Folder::kept`] names it.
//!
//! Once the app has gone, a visit is closed by deleting the marker and the session directory. A
//! visit Desktop ends while the app may still be running is closed by deleting only the marker: its
//! session directory stays, holding the recall, for the app to find. Anything else in the directory
//! (a called-home session, or one left by a visit that could not be closed) is swept away when
//! Desktop starts and before each new visit, so at most one such directory is ever kept.

use formiga_expansion_rulebook::{SessionId, read_bounded, write_atomically};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

const MAX_MARKER_BYTES: u64 = 4 * 1024;

/// Where an app's visits are kept, in the data directory.
pub struct Folder {
    /// Its directory.
    pub directory: &'static str,
    /// The marker in that directory that says a visit is open.
    pub marker_file: &'static str,
    /// The format the marker is written as, and must be to be read.
    pub marker_format: &'static str,
    /// Desktop's own files in that directory that outlast every visit, which a sweep leaves.
    pub kept: &'static [&'static str],
}

/// A marker as it is written: its format and session, then what the app's side remembers, `M`.
#[derive(Serialize, Deserialize)]
struct Marker<M> {
    format: String,
    session_id: SessionId,
    #[serde(flatten)]
    visit: M,
}

/// One app's directory of visits, whose marker remembers `M` beside the session.
pub struct VisitFiles<M> {
    root: PathBuf,
    folder: &'static Folder,
    marker: PhantomData<fn() -> M>,
}

impl<M: Serialize + DeserializeOwned> VisitFiles<M> {
    pub fn new(data_dir: &Path, folder: &'static Folder) -> Self {
        Self {
            root: data_dir.join(folder.directory),
            folder,
            marker: PhantomData,
        }
    }

    /// The app's directory, where Desktop may keep files of its own beside the visits.
    pub fn directory(&self) -> &Path {
        &self.root
    }

    fn marker_path(&self) -> PathBuf {
        self.root.join(self.folder.marker_file)
    }

    /// A session's directory. Safe to build from the identifier, since a [`SessionId`] is never
    /// anything but 32 hex digits.
    pub fn session_dir(&self, session: &SessionId) -> PathBuf {
        self.root.join(session.as_str())
    }

    /// Write a visit: a new session directory for `session`, filled by `write`, then the marker
    /// that says a visit is open, remembering what `write` returns for it. Either everything is
    /// written or nothing is left behind.
    pub fn open<T, E: From<io::Error>>(
        &self,
        session: &SessionId,
        write: impl FnOnce(&Path) -> Result<(T, M), E>,
    ) -> Result<T, E> {
        fs::create_dir_all(&self.root)?;
        self.sweep();
        let dir = self.session_dir(session);
        // A fresh directory every time: an existing one is never reused or trusted.
        fs::create_dir(&dir)?;
        let written = write(&dir).and_then(|(opened, visit)| {
            let marker = Marker {
                format: self.folder.marker_format.to_owned(),
                session_id: session.clone(),
                visit,
            };
            let mut bytes = serde_json::to_vec_pretty(&marker).map_err(io::Error::from)?;
            bytes.push(b'\n');
            write_atomically(&self.marker_path(), &bytes)?;
            Ok(opened)
        });
        if written.is_err() {
            let _ = fs::remove_dir_all(&dir);
        }
        written
    }

    /// The visit left open, if Desktop stopped before closing one: its session and what the
    /// marker remembers of it. A marker that cannot be read is no visit at all.
    pub fn open_visit(&self) -> Option<(SessionId, M)> {
        let bytes = read_bounded(&self.marker_path(), MAX_MARKER_BYTES).ok()?;
        let marker: Marker<M> = serde_json::from_slice(&bytes).ok()?;
        (marker.format == self.folder.marker_format).then_some((marker.session_id, marker.visit))
    }

    /// Forget the open visit, leaving its session directory where it is.
    pub fn forget(&self) {
        if let Err(error) = fs::remove_file(self.marker_path())
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(%error, directory = self.folder.directory, "could not close a visit's marker");
        }
    }

    /// Close a visit once its app has gone, or was never started: the marker goes first, so a
    /// visit is never half-closed and then reopened, and then everything that was written for it
    /// in `dir`.
    pub fn close(&self, dir: &Path) {
        self.forget();
        if let Err(error) = fs::remove_dir_all(dir)
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(%error, directory = self.folder.directory, "could not clear a visit's files");
        }
    }

    /// Remove everything in the directory that is neither the open visit nor a file Desktop keeps
    /// there. Directories are removed without following links.
    pub fn sweep(&self) {
        let open = self.open_visit().map(|(session, _)| session);
        let Ok(entries) = fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let keep = name == self.folder.marker_file
                || self.folder.kept.iter().any(|kept| name == *kept)
                || open
                    .as_ref()
                    .is_some_and(|session| name == session.as_str());
            if keep {
                continue;
            }
            let path = entry.path();
            let removed = match entry.file_type() {
                Ok(kind) if kind.is_dir() => fs::remove_dir_all(&path),
                _ => fs::remove_file(&path),
            };
            if let Err(error) = removed {
                tracing::warn!(%error, directory = self.folder.directory, "could not sweep an old visit's file");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static FOLDER: Folder = Folder {
        directory: "sample",
        marker_file: "visit.json",
        marker_format: "formiga.desktop.sample-visit",
        kept: &["kept.json"],
    };

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Remembered {
        note: String,
    }

    /// A directory of its own under the system's temporary directory, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "formiga-expansion-files-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn open(files: &VisitFiles<Remembered>, note: &str) -> (SessionId, PathBuf) {
        let session = SessionId::generate().unwrap();
        let dir = files
            .open(&session, |dir| {
                fs::write(dir.join("snapshot.json"), b"{}")?;
                Ok::<_, io::Error>((
                    dir.to_owned(),
                    Remembered {
                        note: note.to_owned(),
                    },
                ))
            })
            .unwrap();
        (session, dir)
    }

    #[test]
    fn a_visit_is_written_whole_and_found_again_after_a_restart() {
        let scratch = Scratch::new("open");
        let files = VisitFiles::<Remembered>::new(&scratch.0, &FOLDER);
        assert_eq!(files.open_visit(), None);
        let (session, dir) = open(&files, "first");
        assert_eq!(dir, scratch.0.join("sample").join(session.as_str()));
        assert!(dir.join("snapshot.json").is_file());
        let marker = fs::read_to_string(scratch.0.join("sample").join("visit.json")).unwrap();
        assert_eq!(
            marker,
            format!(
                "{{\n  \"format\": \"formiga.desktop.sample-visit\",\n  \"session_id\": \
                 \"{session}\",\n  \"note\": \"first\"\n}}\n"
            ),
            "the format and session first, then what the app's side remembers"
        );
        let found = VisitFiles::<Remembered>::new(&scratch.0, &FOLDER).open_visit();
        assert_eq!(
            found,
            Some((
                session.clone(),
                Remembered {
                    note: "first".to_owned()
                }
            ))
        );
        // A session directory is never reused.
        assert!(
            files
                .open(&session, |_| Ok::<_, io::Error>((
                    (),
                    Remembered {
                        note: "again".to_owned()
                    }
                )))
                .is_err()
        );
    }

    #[test]
    fn a_visit_that_cannot_be_written_leaves_nothing_behind() {
        let scratch = Scratch::new("failed");
        let files = VisitFiles::<Remembered>::new(&scratch.0, &FOLDER);
        let session = SessionId::generate().unwrap();
        let failed = files.open(&session, |dir| {
            fs::write(dir.join("snapshot.json"), b"{}")?;
            Err::<((), Remembered), _>(io::Error::other("no room"))
        });
        assert!(failed.is_err());
        assert!(!files.session_dir(&session).exists());
        assert_eq!(files.open_visit(), None);
    }

    #[test]
    fn a_marker_that_does_not_check_out_is_no_visit() {
        let scratch = Scratch::new("markers");
        let files = VisitFiles::<Remembered>::new(&scratch.0, &FOLDER);
        let (session, _) = open(&files, "first");
        let path = scratch.0.join("sample").join("visit.json");
        let good = fs::read_to_string(&path).unwrap();
        for (what, bytes) in [
            (
                "another format",
                good.replace("sample-visit", "trip").into_bytes(),
            ),
            (
                "a bad session",
                good.replace(session.as_str(), "../../colony").into_bytes(),
            ),
            ("half-written", good.as_bytes()[..good.len() / 2].to_vec()),
            ("oversized", vec![b' '; 8 * 1024]),
        ] {
            fs::write(&path, bytes).unwrap();
            assert_eq!(files.open_visit(), None, "{what} was taken as a visit");
        }
    }

    #[test]
    fn closing_a_visit_leaves_nothing_and_sweeping_leaves_only_the_open_one_and_what_is_kept() {
        let scratch = Scratch::new("sweep");
        let root = scratch.0.join("sample");
        let files = VisitFiles::<Remembered>::new(&scratch.0, &FOLDER);
        let (_, old) = open(&files, "old");
        files.close(&old);
        assert_eq!(files.open_visit(), None);
        assert!(!old.exists());

        let (open_session, _) = open(&files, "open");
        fs::write(root.join("kept.json"), b"kept").unwrap();
        let stale = root.join("0123456789abcdef0123456789abcdef");
        fs::create_dir_all(&stale).unwrap();
        fs::write(stale.join("receipt.json"), b"{}").unwrap();
        fs::write(root.join("visit.json.tmp"), b"half").unwrap();
        // A link left in the directory is removed, and what it points at is not touched.
        let outside = scratch.0.join("colony.json");
        fs::write(&outside, b"the colony").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&scratch.0, root.join("link")).unwrap();
        files.sweep();
        let mut left: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        let mut expected = vec![
            open_session.to_string(),
            "kept.json".to_owned(),
            "visit.json".to_owned(),
        ];
        expected.sort();
        assert_eq!(left, expected);
        assert_eq!(fs::read(&outside).unwrap(), b"the colony");
    }

    #[test]
    fn forgetting_a_visit_leaves_its_directory_until_the_next_sweep() {
        let scratch = Scratch::new("forget");
        let files = VisitFiles::<Remembered>::new(&scratch.0, &FOLDER);
        let (_, called_home) = open(&files, "called home");
        files.forget();
        assert_eq!(files.open_visit(), None, "the visit is over for Desktop");
        assert!(called_home.exists(), "the app can still find its recall");
        let (_, next) = open(&files, "next");
        assert!(!called_home.exists());
        assert!(next.exists());
    }
}
