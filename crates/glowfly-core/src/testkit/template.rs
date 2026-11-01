//! Fixtures built once per test binary and copied for each test: a fixture that takes dozens of
//! git commands to make costs one build, and every test still gets a folder of its own to change.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A fixture folder `build` makes once per test binary (on first use), copied fresh for each
/// test that asks. `build` must give the same result every time, as a fixture built per test
/// would: fixed dates and identities, nothing that names the folder it was built in (a
/// repository's absolute paths would point the copies back at the template).
pub struct Template {
    name: &'static str,
    build: fn(&Path),
    dir: OnceLock<PathBuf>,
}

impl Template {
    /// The template `name` (unique among the binary's templates), made by `build` in an empty
    /// folder.
    pub const fn new(name: &'static str, build: fn(&Path)) -> Self {
        Template { name, build, dir: OnceLock::new() }
    }

    /// A fresh copy of the template's contents at `dest`, which is removed first. File times
    /// differ from the template's, so a repository's index needs `update-index -q --refresh`
    /// before anything trusts its stat data.
    pub fn copy_to(&self, dest: &Path) {
        let from = self.dir.get_or_init(|| {
            let dir = crate::paths::temp_dir().join(format!("glowfly-template-{}-{}", self.name, std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            (self.build)(&dir);
            dir
        });
        let _ = std::fs::remove_dir_all(dest);
        std::fs::create_dir_all(dest).unwrap();
        copy_tree(from, dest);
    }
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let dest = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
            copy_tree(&entry.path(), &dest);
        } else {
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
}
