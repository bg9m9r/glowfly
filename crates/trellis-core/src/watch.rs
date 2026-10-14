//! Which file changes in a repository matter to the app: path classification for a file
//! watcher, plus a cache of ignored folders read through libgit2. No watching happens here.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

/// What a changed path calls for. Ordered, so a burst of changes merges with `max()`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum Change {
    /// Nothing to reload.
    #[default]
    None,
    /// The working tree or index changed: re-read the status.
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

/// The roots a watcher reports paths under.
pub struct WatchPaths {
    workdirs: Vec<PathBuf>,
    git_dirs: Vec<PathBuf>,
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
        Self { workdirs: with_canonical(workdir), git_dirs: with_canonical(git_dir) }
    }

    /// What `touch` on `path` calls for. `ignored` answers whether a work-tree path (relative,
    /// '/'-separated) is ignored by `.gitignore` or `info/exclude`.
    pub fn classify(&self, path: &Path, touch: Touch, ignored: &mut impl FnMut(&str) -> bool) -> Change {
        // The git dir usually sits inside the workdir, so it is checked first.
        if let Some(rel) = self.git_dirs.iter().find_map(|g| path.strip_prefix(g).ok()) {
            return git_change(rel, touch);
        }
        let Some(rel) = self.workdirs.iter().find_map(|w| path.strip_prefix(w).ok()) else {
            return Change::None;
        };
        let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        // The root itself, and anything inside a nested repository or submodule.
        if parts.is_empty() || parts.iter().any(|p| p == ".git") {
            return Change::None;
        }
        if ignored(&parts.join("/")) { Change::None } else { Change::Status }
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
        ["HEAD" | "packed-refs" | "MERGE_HEAD" | "CHERRY_PICK_HEAD" | "REVERT_HEAD" | "REBASE_HEAD" | "BISECT_LOG"] => Change::Full,
        ["index"] | ["info", "exclude"] => Change::Status,
        _ => Change::None,
    }
}

/// Most ignored folders remembered before the cache starts over.
const MAX_IGNORED_DIRS: usize = 4096;

/// Answers whether work-tree paths are ignored, remembering ignored folders so a build writing
/// thousands of files under `target/` costs a set lookup each.
pub struct Ignores {
    repo: Option<git2::Repository>,
    dirs: HashSet<String>,
}

impl Ignores {
    /// When `workdir` is not a repository, nothing is ignored.
    pub fn open(workdir: &Path) -> Self {
        Self { repo: git2::Repository::open(workdir).ok(), dirs: HashSet::new() }
    }

    /// Whether `rel` (relative to the workdir, '/'-separated) is ignored. Errors count as not.
    pub fn is_ignored(&mut self, rel: &str) -> bool {
        if prefixes(rel).any(|p| self.dirs.contains(p)) || self.dirs.contains(rel) {
            return true;
        }
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
        true
    }

    /// Forgets the cached folders (after a `.gitignore` or `info/exclude` changed).
    pub fn clear(&mut self) {
        self.dirs.clear();
    }
}

/// The parent folders of `rel`, shallowest first: "a", "a/b" for "a/b/c".
fn prefixes(rel: &str) -> impl Iterator<Item = &str> {
    rel.match_indices('/').map(|(i, _)| &rel[..i])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{git, scratch_repo, write};

    #[test]
    fn git_internals_are_classified() {
        let c = |p: &str| git_change(Path::new(p), Touch::Modified);
        assert_eq!(c("index"), Change::Status);
        assert_eq!(c("info/exclude"), Change::Status);
        for full in ["worktrees/wt/HEAD", "worktrees/wt/locked", "worktrees/wt/gitdir"] {
            assert_eq!(c(full), Change::Full, "{full}");
        }
        for none in ["worktrees/wt/index", "worktrees/wt/logs/HEAD", "worktrees/wt/ORIG_HEAD"] {
            assert_eq!(c(none), Change::None, "{none}");
        }
        for full in [
            "HEAD",
            "packed-refs",
            "refs/heads/main",
            "refs/remotes/origin/x",
            "refs/stash",
            "MERGE_HEAD",
            "CHERRY_PICK_HEAD",
            "REBASE_HEAD",
            "rebase-merge/done",
            "rebase-apply/next",
            "sequencer/todo",
        ] {
            assert_eq!(c(full), Change::Full, "{full}");
        }
        for none in [
            "index.lock",
            "refs/heads/main.lock",
            "HEAD.lock",
            "objects/ab/cdef",
            "logs/HEAD",
            "FETCH_HEAD",
            "ORIG_HEAD",
            "COMMIT_EDITMSG",
            "TRELLIS_COMMIT_EDITMSG",
            "config",
            "hooks/pre-commit",
            "",
        ] {
            assert_eq!(c(none), Change::None, "{none}");
        }
    }

    #[test]
    fn linked_worktree_folders_count_only_when_they_come_or_go() {
        let c = |p: &str, touch| git_change(Path::new(p), touch);
        // Windows reports these folders modified on every index write in a linked worktree.
        for p in ["worktrees", "worktrees/wt"] {
            assert_eq!(c(p, Touch::Modified), Change::None, "{p}");
            assert_eq!(c(p, Touch::Other), Change::Full, "{p}");
        }
        // The files inside are judged the same way whatever happened to them.
        for touch in [Touch::Modified, Touch::Other] {
            assert_eq!(c("worktrees/wt/gitdir", touch), Change::Full);
            assert_eq!(c("worktrees/wt/index", touch), Change::None);
            assert_eq!(c("worktrees/wt/index.lock", touch), Change::None);
            assert_eq!(c("refs/heads/main", touch), Change::Full);
            assert_eq!(c("index", touch), Change::Status);
        }
    }

    #[test]
    fn worktree_paths_are_classified() {
        let wd = std::env::temp_dir().join("trellis-watch-classify-missing").join("wd");
        let paths = WatchPaths::new(&wd, &wd.join(".git"));
        let mut seen = Vec::new();
        let mut ignored = |rel: &str| {
            seen.push(rel.to_owned());
            rel.starts_with("target/")
        };
        assert_eq!(paths.classify(&wd.join("src").join("a.rs"), Touch::Modified, &mut ignored), Change::Status);
        assert_eq!(paths.classify(&wd, Touch::Modified, &mut ignored), Change::None);
        assert_eq!(paths.classify(&wd.join("vendor/sub/.git/HEAD"), Touch::Modified, &mut ignored), Change::None);
        assert_eq!(paths.classify(&wd.join(".git").join("index"), Touch::Modified, &mut ignored), Change::Status, "the git rule wins");
        assert_eq!(paths.classify(&wd.join("target").join("debug").join("x.o"), Touch::Modified, &mut ignored), Change::None);
        let outside = std::env::temp_dir().join("trellis-watch-elsewhere").join("a.rs");
        assert_eq!(paths.classify(&outside, Touch::Modified, &mut ignored), Change::None);
        assert_eq!(seen, ["src/a.rs", "target/debug/x.o"], "relative and '/'-separated");

        // A linked worktree: the git dir lives elsewhere and `.git` is a file in the workdir.
        let gd = std::env::temp_dir().join("trellis-watch-classify-missing").join("main").join(".git").join("worktrees").join("wd");
        let linked = WatchPaths::new(&wd, &gd);
        assert_eq!(linked.classify(&gd.join("HEAD"), Touch::Modified, &mut |_| false), Change::Full);
        assert_eq!(linked.classify(&wd.join(".git"), Touch::Modified, &mut |_| false), Change::None);

        assert_eq!(Change::Status.max(Change::Full), Change::Full);
        assert_eq!(Change::None.max(Change::Status), Change::Status);
    }

    #[test]
    fn ignores_follow_gitignore() {
        let dir = scratch_repo("watch-ignores");
        write(&dir, ".gitignore", "target/\n*.log\n");
        std::fs::create_dir_all(dir.join("target/debug")).unwrap();
        write(&dir, "target/debug/x.o", "obj");
        git(&dir, &["add", ".gitignore"]);
        let mut ig = Ignores::open(&dir);
        assert!(ig.is_ignored("target/debug/x.o"));
        assert!(ig.dirs.contains("target"), "{:?}", ig.dirs);
        assert_eq!(ig.dirs.len(), 1);
        // Answered from the cache: even a repository-less lookup would say no.
        let repo = ig.repo.take();
        assert!(ig.is_ignored("target/release/y.o"));
        ig.repo = repo;
        assert!(ig.is_ignored("a.log"));
        assert!(!ig.is_ignored("src/main.rs"));
        assert!(!ig.is_ignored(".gitignore"));
        ig.clear();
        assert!(ig.dirs.is_empty());

        let plain = std::env::temp_dir().join(format!("trellis-watch-plain-{}", std::process::id()));
        std::fs::create_dir_all(&plain).unwrap();
        let mut none = Ignores::open(&plain);
        assert!(!none.is_ignored("target/debug/x.o"));
        assert!(!none.is_ignored("a.log"));
        let _ = std::fs::remove_dir_all(&plain);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
