//! The watcher's view of the files the repository's config includes (`glowfly_core::config_includes`):
//! classification takes a change to one as a change to the config, and each one's folder outside
//! the folders already watched gets a watch of its own (not recursive: only the file matters).
//! Read again when the watcher starts and after every burst that reloads everything, so an
//! include added, removed or switched by a branch change is followed.

use crate::effect::WatchTarget;
use glowfly_core::watch::WatchPaths;
use notify::{RecursiveMode, Watcher};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The folders watched for included files, beyond the work tree and git dir.
#[derive(Default)]
pub(super) struct IncludeWatches {
    dirs: BTreeSet<PathBuf>,
}

impl IncludeWatches {
    /// Reads the includes again, tells `paths` about them and watches their folders in `watcher`
    /// (dropping the watches no file needs any more). A folder that can't be watched is skipped:
    /// the safety poll on Windows and F5 still find the change.
    pub(super) fn refresh(&mut self, watcher: &mut impl Watcher, target: &WatchTarget, common: Option<&Path>, paths: &mut WatchPaths) {
        let files = glowfly_core::config_includes::of_repo(&target.git_dir, common).files;
        paths.set_config_files(&files);
        let covered = covered_roots(target, common);
        let wanted: BTreeSet<PathBuf> = files
            .iter()
            .filter_map(|f| f.parent())
            .filter(|d| d.is_dir() && !covered.iter().any(|(root, deep)| if *deep { d.starts_with(root) } else { d == root }))
            .map(Path::to_path_buf)
            .collect();
        for gone in self.dirs.difference(&wanted) {
            let _ = watcher.unwatch(gone);
        }
        let mut now = BTreeSet::new();
        for dir in wanted {
            if self.dirs.contains(&dir) {
                now.insert(dir);
                continue;
            }
            match watcher.watch(&dir, RecursiveMode::NonRecursive) {
                Ok(()) => {
                    now.insert(dir);
                }
                Err(e) => tracing::debug!(dir = %dir.display(), "could not watch an included config file's folder: {e}"),
            }
        }
        self.dirs = now;
    }
}

/// The folders the watcher already covers, in each spelling, and whether everything below each
/// is covered too: the work tree and git dir are watched recursively, the common dir itself
/// only, its refs recursively.
fn covered_roots(target: &WatchTarget, common: Option<&Path>) -> Vec<(PathBuf, bool)> {
    let spellings = |p: &Path| {
        let mut all = vec![p.to_path_buf()];
        if let Ok(real) = std::fs::canonicalize(p) {
            all.push(glowfly_core::paths::plain(&real));
        }
        all
    };
    let mut out: Vec<(PathBuf, bool)> = Vec::new();
    for deep in [&target.workdir, &target.git_dir] {
        out.extend(spellings(deep).into_iter().map(|p| (p, true)));
    }
    if let Some(c) = common {
        out.extend(spellings(c).into_iter().map(|p| (p, false)));
        out.extend(spellings(&c.join("refs")).into_iter().map(|p| (p, true)));
    }
    out
}
