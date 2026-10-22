//! A folder of its own for a test that writes files, the way a session or a visit does.

use std::path::PathBuf;

/// A directory under the system's temporary directory, named for the test and this process, empty
/// when made and removed when dropped.
pub(crate) struct Scratch(pub(crate) PathBuf);

impl Scratch {
    /// `formiga-<area>-<name>-<process>`: `area` says whose tests these are, `name` which test.
    pub(crate) fn new(area: &str, name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("formiga-{area}-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
