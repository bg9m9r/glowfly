//! Which file changes in a repository matter to the app: path classification for a file
//! watcher, plus a cache of ignored folders read through libgit2 (paths the index holds are
//! never ignored). No watching happens here.
//! Submodules' repositories (in `<git dir>/modules` or a nested `.git` folder) count only when
//! their HEAD or refs move, which changes the parent's status. The config (`config`, a
//! worktree's own `config.worktree`, and the files they include, wherever they are) reloads
//! everything: remotes, upstreams and the settings that decide whether git may run there all
//! come from it.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

/// What a changed path calls for. Ordered, so a burst of changes merges with `max()`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum Change {
    /// Nothing to reload.
    #[default]
    None,
    /// Only some work-tree paths changed: re-read the unstaged rows under them.
    Paths,
    /// The index changed (and maybe some work-tree paths): re-read the rows of the paths whose
    /// index entries changed, with the named paths.
    Index,
    /// An ignore or attributes file (any `.gitattributes`, or `info/attributes` in the git dir or
    /// a linked worktree's common dir) changed, or events were lost: re-read the whole status,
    /// which also re-reads the root `.gitattributes`' LFS patterns.
    Status,
    /// HEAD, refs or a merge/rebase state changed: reload everything.
    Full,
}

/// What happened to a path, as far as classification cares.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Touch {
    /// Its contents or metadata changed. On Windows a folder reports this whenever an entry
    /// inside it is written, so for a folder it says nothing about the folder itself.
    Modified,
    /// It was created, removed or renamed, or the platform didn't say what happened.
    Other,
}

/// The most distinct work-tree paths one burst re-reads on their own; past it the whole status is
/// read again.
pub const MAX_WATCH_PATHS: usize = 512;

/// The roots a watcher reports paths under.
pub struct WatchPaths {
    workdirs: Vec<PathBuf>,
    git_dirs: Vec<PathBuf>,
    /// A linked worktree's common git dir (the main repository's `.git`), whose refs, packed
    /// refs, HEAD and config it shares; empty when it is the git dir itself.
    common_dirs: Vec<PathBuf>,
    /// The files the repository's config includes (`config_includes`), in each spelling a
    /// watcher may report.
    config_files: Vec<PathBuf>,
}

/// The common git dir of a linked worktree's git dir (`<repo>/.git/worktrees/<name>`): the
/// folder its `commondir` file names (relative to `git_dir` unless absolute). None for any other
/// git dir, which is its own common dir.
pub fn common_dir(git_dir: &Path) -> Option<PathBuf> {
    // A regular file of a few bytes: a nested repository's content names this one.
    let text = crate::config_includes::read_file_bounded(&git_dir.join("commondir"), 4096).and_then(|b| String::from_utf8(b).ok());
    match text {
        Some(text) if !text.trim().is_empty() => {
            let named = Path::new(text.trim_end_matches(['\n', '\r']));
            let joined = if named.is_absolute() { named.to_path_buf() } else { git_dir.join(named) };
            Some(crate::paths::resolve(&joined))
        }
        _ => None,
    }
}

/// `path`, plus its canonical form when that differs (some platforms report real paths, others
/// the path that was watched).
fn with_canonical(path: &Path) -> Vec<PathBuf> {
    let mut all = vec![path.to_path_buf()];
    if let Ok(c) = std::fs::canonicalize(path)
        && c != path
    {
        all.push(c);
    }
    all
}

impl WatchPaths {
    pub fn new(workdir: &Path, git_dir: &Path) -> Self {
        Self { workdirs: with_canonical(workdir), git_dirs: with_canonical(git_dir), common_dirs: Vec::new(), config_files: Vec::new() }
    }

    /// Classifies a change to any of `files`, the files the config includes, as a change to the
    /// config itself, replacing the files given before.
    pub fn set_config_files(&mut self, files: &[PathBuf]) {
        self.config_files.clear();
        for f in files {
            self.config_files.extend(with_canonical(f));
            // A file that doesn't exist yet has no canonical form, but its folder may.
            if let (Some(dir), Some(name)) = (f.parent(), f.file_name())
                && let Ok(real) = std::fs::canonicalize(dir)
            {
                self.config_files.push(crate::paths::plain(&real).join(name));
            }
        }
        self.config_files.sort();
        self.config_files.dedup();
    }

    /// Also classifies paths in the common git dir `common` (unless it is the git dir).
    pub fn with_common(mut self, common: &Path) -> Self {
        let all = with_canonical(common);
        if !all.iter().any(|c| self.git_dirs.contains(c)) {
            self.common_dirs = all;
        }
        self
    }

    /// Whether `path` is one of the files git reads the repository's config from: the git dir's
    /// `config` or `config.worktree`, a linked worktree's common dir's `config`, or a file they
    /// include. A change to one may change the identity commits get.
    pub fn is_config(&self, path: &Path) -> bool {
        let named = |rel: &Path, names: &[&str]| {
            let mut parts = rel.components();
            matches!((parts.next(), parts.next()), (Some(Component::Normal(n)), None) if n.to_str().is_some_and(|n| names.contains(&n)))
        };
        self.config_files.iter().any(|f| f == path)
            || self.git_dirs.iter().filter_map(|g| path.strip_prefix(g).ok()).any(|rel| named(rel, &["config", "config.worktree"]))
            || self.common_dirs.iter().filter_map(|c| path.strip_prefix(c).ok()).any(|rel| named(rel, &["config"]))
    }

    /// What `touch` on `path` calls for. `ignored` answers whether a work-tree path (relative,
    /// '/'-separated) is ignored by `.gitignore` or `info/exclude` and not tracked
    /// (`Ignores::is_ignored`).
    pub fn classify(&self, path: &Path, touch: Touch, ignored: &mut impl FnMut(&str) -> bool) -> Change {
        self.classify_path(path, touch, ignored).0
    }

    /// `classify`, plus the work-tree path ('/'-separated, relative) when only it changed
    /// (`Change::Paths`). The git dir's index gives `Change::Index` and no path. An ignore,
    /// attributes or `.gitmodules` file, or a name that isn't UTF-8, calls for the whole status,
    /// as does HEAD or a ref moving in a nested `.git` folder (a submodule that keeps its own
    /// repository); the rest of such a folder is ignored.
    pub fn classify_path(&self, path: &Path, touch: Touch, ignored: &mut impl FnMut(&str) -> bool) -> (Change, Option<String>) {
        // An included config file may sit anywhere, the work tree and the git dir too.
        if self.config_files.iter().any(|f| f == path) {
            return (Change::Full, None);
        }
        // The git dir usually sits inside the workdir, so it is checked first.
        if let Some(rel) = self.git_dirs.iter().find_map(|g| path.strip_prefix(g).ok()) {
            return (git_change(rel, touch), None);
        }
        if let Some(rel) = self.common_dirs.iter().find_map(|c| path.strip_prefix(c).ok()) {
            return (common_change(rel), None);
        }
        let Some(rel) = self.workdirs.iter().find_map(|w| path.strip_prefix(w).ok()) else {
            return (Change::None, None);
        };
        let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        // The git folder of a nested repository or a submodule that keeps its own: a commit,
        // checkout or ref change there changes what the parent's status says about it.
        if let Some(i) = parts.iter().position(|p| p == ".git") {
            let inner: Vec<&str> = parts[i + 1..].iter().map(String::as_str).collect();
            return match inner.as_slice() {
                [.., last] if last.ends_with(".lock") => (Change::None, None),
                ["HEAD"] | ["packed-refs"] | ["refs", ..] => (Change::Status, None),
                _ => (Change::None, None),
            };
        }
        // The root itself, and ignored paths.
        if parts.is_empty() || ignored(&parts.join("/")) {
            return (Change::None, None);
        }
        let whole = rel.components().any(|c| c.as_os_str().to_str().is_none())
            || matches!(parts.last().map(String::as_str), Some(".gitignore" | ".gitattributes"))
            || parts == [".gitmodules"];
        if whole { (Change::Status, None) } else { (Change::Paths, Some(parts.join("/"))) }
    }
}

/// What `touch` on `rel`, a path relative to the git dir, calls for.
pub fn git_change(rel: &Path, touch: Touch) -> Change {
    let parts: Vec<&str> = rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect();
    let Some(last) = parts.last() else { return Change::None };
    if last.ends_with(".lock") {
        return Change::None;
    }
    if matches!(parts[0], "refs" | "rebase-merge" | "rebase-apply" | "sequencer") {
        return Change::Full;
    }
    match parts.as_slice() {
        // A linked worktree added, removed, moved, locked or switched to another HEAD. The
        // folders count only when they come or go: every index write in a linked worktree
        // modifies `worktrees/<name>` on Windows.
        ["worktrees"] | ["worktrees", _] if touch != Touch::Modified => Change::Full,
        ["worktrees", _, "HEAD" | "locked" | "gitdir"] => Change::Full,
        ["HEAD" | "packed-refs" | "shallow" | "MERGE_HEAD" | "CHERRY_PICK_HEAD" | "REVERT_HEAD" | "REBASE_HEAD" | "BISECT_LOG"] => {
            Change::Full
        }
        ["index"] => Change::Index,
        ["info", "exclude" | "attributes"] => Change::Status,
        // Remotes, upstreams and command-running settings. Another worktree's
        // `worktrees/<name>/config.worktree` falls through to None.
        ["config" | "config.worktree"] => Change::Full,
        ["modules", rest @ ..] => module_change(rest),
        _ => Change::None,
    }
}

/// What a change to `parts`, a path under `<git dir>/modules` (where git keeps submodules'
/// repositories, nested ones under `<name>/modules`), calls for: a moved HEAD or ref changes the
/// parent's status. Their index is left out on purpose: reading the parent's status runs
/// `git status` inside, which rewrites it, and that would reload again and again.
fn module_change(parts: &[&str]) -> Change {
    let after_logs = |i: usize| i > 0 && parts[i - 1] == "logs";
    let head = parts.len() >= 2 && parts[parts.len() - 1] == "HEAD" && !after_logs(parts.len() - 1);
    let refs = parts.iter().enumerate().any(|(i, p)| *p == "refs" && !after_logs(i) && i + 1 < parts.len());
    if head || refs || parts.last() == Some(&"packed-refs") { Change::Status } else { Change::None }
}

/// What a change to `rel`, a path in a linked worktree's common git dir, calls for: the refs
/// it shares, its config and the main worktree's HEAD (which branch is checked out there) reload
/// everything; `info/attributes` re-reads the status; the rest belongs to the main worktree (its index, its rebase, its
/// `config.worktree`) or to no one.
pub fn common_change(rel: &Path) -> Change {
    let parts: Vec<&str> = rel.components().filter_map(|c| c.as_os_str().to_str()).collect();
    match parts.as_slice() {
        [.., last] if last.ends_with(".lock") => Change::None,
        ["refs", ..] | ["packed-refs"] | ["HEAD"] | ["config"] => Change::Full,
        // Every worktree reads the common dir's attributes.
        ["info", "attributes"] => Change::Status,
        _ => Change::None,
    }
}

/// Most ignored folders remembered before the cache starts over.
const MAX_IGNORED_DIRS: usize = 4096;

/// Answers whether work-tree paths are ignored, remembering ignored folders so a build writing
/// thousands of files under `target/` costs a set lookup each. A path the index holds is never
/// ignored, whatever the patterns say (git's rule: tracked files aren't affected), so an edit to
/// a force-added file, or one committed before a pattern covered it, still counts. The cache
/// says only which folders match a pattern; the index is asked after it, for ignored paths only.
pub struct Ignores {
    repo: Option<git2::Repository>,
    dirs: HashSet<String>,
    /// The repository's index, loaded the first time an ignored path is seen.
    index: Option<git2::Index>,
    /// The index must be (re-)read before it is asked: not loaded yet, or changed on disk since.
    index_stale: bool,
}

impl Ignores {
    /// When `workdir` is not a repository, nothing is ignored.
    pub fn open(workdir: &Path) -> Self {
        Self { repo: git2::Repository::open(workdir).ok(), dirs: HashSet::new(), index: None, index_stale: true }
    }

    /// Whether `rel` (relative to the workdir, '/'-separated) is ignored and not tracked. Errors
    /// count as not ignored; an index that can't be read counts as holding nothing. A folder
    /// counts as tracked when some index path is below it, but only for `Touch::Other` (it came,
    /// went or was renamed): Windows' modification of a folder whenever an entry inside is
    /// written says nothing about the files below it.
    pub fn is_ignored(&mut self, rel: &str, touch: Touch) -> bool {
        if !(prefixes(rel).any(|p| self.dirs.contains(p)) || self.dirs.contains(rel)) {
            let Some(repo) = &self.repo else { return false };
            if !repo.is_path_ignored(rel).unwrap_or(false) {
                return false;
            }
            // Remember the shallowest ignored folder, so later paths below it skip libgit2.
            if let Some(dir) = prefixes(rel).find(|p| repo.is_path_ignored(format!("{p}/")).unwrap_or(false)) {
                if self.dirs.len() >= MAX_IGNORED_DIRS {
                    self.dirs.clear();
                }
                self.dirs.insert(dir.to_owned());
            }
        }
        !self.tracked(rel, touch)
    }

    /// Whether the index holds `rel` at any stage or, for `Touch::Other`, some path below it.
    fn tracked(&mut self, rel: &str, touch: Touch) -> bool {
        if self.index_stale {
            self.index_stale = false;
            match self.index.as_mut() {
                // Re-read only when the file changed on disk.
                Some(index) => {
                    let _ = index.read(false);
                }
                None => self.index = self.repo.as_ref().and_then(|r| r.index().ok()),
            }
        }
        let Some(index) = &self.index else { return false };
        (0..=3).any(|stage| index.get_path(Path::new(rel), stage).is_some())
            || (touch == Touch::Other && index.find_prefix(format!("{rel}/")).is_ok())
    }

    /// The index changed on disk: read it again before it is next asked.
    pub fn index_changed(&mut self) {
        self.index_stale = true;
    }

    /// Forgets the cached folders (after a `.gitignore` or `info/exclude` changed). The index
    /// stays.
    pub fn clear(&mut self) {
        self.dirs.clear();
    }
}

/// The parent folders of `rel`, shallowest first: "a", "a/b" for "a/b/c".
pub(crate) fn prefixes(rel: &str) -> impl Iterator<Item = &str> {
    rel.match_indices('/').map(|(i, _)| &rel[..i])
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
