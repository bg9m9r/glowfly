//! The repositories a journey works on, all under one folder in the system's temporary folder:
//! `remote/` for bare remotes, `work/` for the clones the app opens, `config/` for the app's
//! own files. Git runs with fixed identities and dates; the checks read the disk through
//! libgit2, never the screen.

use git2::{ConfigLevel, Oid, Repository, RepositoryState, StatusOptions};
use glowfly_core::gitcli;
use std::path::{Path, PathBuf};

const DATE: &str = "2026-01-10T10:00:00+00:00";

pub(super) struct Fixture {
    pub root: PathBuf,
}

impl Fixture {
    /// A fresh folder for the journey `name` (an earlier run's is removed first).
    pub fn new(name: &str) -> Self {
        let root = glowfly_core::paths::temp_dir().join(format!("trellis-journey-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for sub in ["remote", "work", "config"] {
            std::fs::create_dir_all(root.join(sub)).unwrap();
        }
        Self { root }
    }

    pub fn remote(&self) -> PathBuf {
        self.root.join("remote")
    }

    pub fn work(&self) -> PathBuf {
        self.root.join("work")
    }

    /// The app's own folder (settings.json, recent.txt, tabs.txt).
    pub fn config(&self) -> PathBuf {
        self.root.join("config")
    }

    /// An empty bare repository `remote/<name>.git` whose default branch is main.
    pub fn bare(&self, name: &str) -> PathBuf {
        let bare = self.remote().join(format!("{name}.git"));
        git(&self.remote(), &["init", "-q", "--bare", "-b", "main", &format!("{name}.git")]);
        bare
    }

    /// A bare remote `name` holding one commit of `files` on main, and the repository it was
    /// pushed from (`seed-<name>`, outside `work/`).
    pub fn seeded_remote(&self, name: &str, files: &[(&str, &str)]) -> (PathBuf, PathBuf) {
        let bare = self.bare(name);
        let seed = self.root.join(format!("seed-{name}"));
        self.init(&seed, files);
        git(&seed, &["remote", "add", "origin", &shown(&bare)]);
        git(&seed, &["push", "-q", "-u", "origin", "main"]);
        (bare, seed)
    }

    /// A new repository at `dir` with `files` committed on main as "Initial commit".
    pub fn init(&self, dir: &Path, files: &[(&str, &str)]) {
        std::fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        configure(dir);
        commit_files(dir, files, "Initial commit");
    }

    /// Clones `bare` into `dest` with git (another user's clone, or one the journey opens).
    pub fn clone_cli(&self, bare: &Path, dest: &Path) {
        git(&self.root, &["clone", "-q", &shown(bare), &shown(dest)]);
        configure_clone(dest);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// `path` as git and the user see it on this system.
pub(super) fn shown(path: &Path) -> String {
    glowfly_core::native_path::shown(path)
}

/// Runs git in `dir` with a fixed author and committer, and asserts it succeeded.
pub(super) fn git(dir: &Path, args: &[&str]) {
    let env = [
        ("GIT_AUTHOR_NAME", "Journey Author"),
        ("GIT_AUTHOR_EMAIL", "author@example.com"),
        ("GIT_AUTHOR_DATE", DATE),
        ("GIT_COMMITTER_NAME", "Journey Author"),
        ("GIT_COMMITTER_EMAIL", "author@example.com"),
        ("GIT_COMMITTER_DATE", DATE),
    ];
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let (ok, out) = gitcli::run_with_input(dir, &args, "", &env);
    assert!(ok, "git {} in {} failed:\n{out}", args.join(" "), dir.display());
}

/// The repository settings every journey repository gets, so the user's global config (signing,
/// line endings, pull strategy, background maintenance) can't change what a journey sees.
pub(super) fn configure(dir: &Path) {
    let repo = Repository::open(dir).unwrap();
    let mut cfg = repo.config().unwrap().open_level(ConfigLevel::Local).unwrap();
    cfg.set_str("user.name", "Journey User").unwrap();
    cfg.set_str("user.email", "user@example.com").unwrap();
    for (key, value) in [("commit.gpgsign", false), ("maintenance.auto", false), ("core.autocrlf", false), ("pull.rebase", false)] {
        cfg.set_bool(key, value).unwrap();
    }
}

/// `configure` for a clone the app made, then its files written again under those settings: a
/// global core.autocrlf may have checked them out with CRLF, which would show as changes now.
pub(super) fn configure_clone(dir: &Path) {
    configure(dir);
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().is_some_and(|n| n == ".git") {
            continue;
        }
        if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) }.unwrap();
    }
    git(dir, &["reset", "-q", "--hard"]);
}

/// Writes `files` in `dir`, stages and commits them as `message`.
pub(super) fn commit_files(dir: &Path, files: &[(&str, &str)], message: &str) {
    for (name, text) in files {
        write(dir, name, text);
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
}

pub(super) fn write(dir: &Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).unwrap();
}

fn open(dir: &Path) -> Repository {
    Repository::open(dir).unwrap_or_else(|e| panic!("{} is not a repository: {e}", dir.display()))
}

pub(super) fn head_branch(dir: &Path) -> Option<String> {
    let repo = open(dir);
    let head = repo.head().ok()?;
    head.is_branch().then(|| head.shorthand().unwrap_or_default().to_owned())
}

pub(super) fn head_oid(dir: &Path) -> Oid {
    open(dir).head().unwrap().target().unwrap()
}

pub(super) fn branch_oid(dir: &Path, name: &str) -> Option<Oid> {
    open(dir).find_reference(&format!("refs/heads/{name}")).ok()?.target()
}

pub(super) fn parents(dir: &Path, oid: Oid) -> Vec<Oid> {
    open(dir).find_commit(oid).unwrap().parent_ids().collect()
}

pub(super) fn summary(dir: &Path, oid: Oid) -> String {
    open(dir).find_commit(oid).unwrap().summary().unwrap_or_default().to_owned()
}

pub(super) fn state(dir: &Path) -> RepositoryState {
    open(dir).state()
}

pub(super) fn conflicted_paths(dir: &Path) -> Vec<String> {
    let index = open(dir).index().unwrap();
    let conflicts = index.conflicts().unwrap();
    conflicts
        .filter_map(|c| {
            let c = c.ok()?;
            let entry = c.our.or(c.their).or(c.ancestor)?;
            Some(String::from_utf8_lossy(&entry.path).into_owned())
        })
        .collect()
}

fn blob_text(repo: &Repository, oid: Oid) -> String {
    String::from_utf8_lossy(repo.find_blob(oid).unwrap().content()).replace('\r', "")
}

/// `path` as staged (stage 0 of the index), if it is there.
pub(super) fn index_text(dir: &Path, path: &str) -> Option<String> {
    let repo = open(dir);
    let index = repo.index().unwrap();
    let entry = index.get_path(Path::new(path), 0)?;
    Some(blob_text(&repo, entry.id))
}

/// The working file `path`, without carriage returns (a global core.autocrlf affects the clone
/// the app makes).
pub(super) fn work_text(dir: &Path, path: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(path)).ok().map(|t| t.replace('\r', ""))
}

/// `path` in commit `oid`'s tree, if it is there.
pub(super) fn tree_text(dir: &Path, oid: Oid, path: &str) -> Option<String> {
    let repo = open(dir);
    let tree = repo.find_commit(oid).unwrap().tree().unwrap();
    let entry = tree.get_path(Path::new(path)).ok()?;
    Some(blob_text(&repo, entry.id()))
}

/// No staged, unstaged or untracked changes.
pub(super) fn is_clean(dir: &Path) -> bool {
    let repo = open(dir);
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).include_ignored(false);
    repo.statuses(Some(&mut opts)).unwrap().is_empty()
}

pub(super) fn stash_count(dir: &Path) -> usize {
    let repo = open(dir);
    repo.reflog("refs/stash").map(|r| r.len()).unwrap_or(0)
}

/// Where `branch` points in the bare repository `bare`.
pub(super) fn remote_oid(bare: &Path, branch: &str) -> Option<Oid> {
    branch_oid(bare, branch)
}
