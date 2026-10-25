//! When a fixture command has to go to the git CLI after all: anything in the repository, its
//! config or the environment that would make git behave differently from the in-process path.

use git2::{Index, Repository, RepositoryState};
use std::path::Path;

/// Environment variables the in-process path honours (the identity and dates of a commit).
pub(super) const IDENTITY_ENV: &[&str] =
    &["GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_AUTHOR_DATE", "GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL", "GIT_COMMITTER_DATE"];

/// Process-wide variables that change where or how git works.
const PROCESS_ENV: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_COMMON_DIR",
    "GIT_CONFIG",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_NOSYSTEM",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
];

/// Whether `env` (or the process) sets anything beyond what the in-process path honours.
pub(super) fn env_needs_cli(env: &[(&str, &str)]) -> bool {
    env.iter().any(|(k, _)| !IDENTITY_ENV.contains(k)) || PROCESS_ENV.iter().any(|k| std::env::var_os(k).is_some())
}

/// Opens `dir` as a repository whose working folder (or, bare, whose git folder) is `dir`
/// itself, as a git command run there would use it. None for a subfolder or not a repository.
pub(super) fn open(dir: &Path) -> Option<Repository> {
    let repo = Repository::open(dir).ok()?;
    let at = repo.workdir().unwrap_or(repo.path());
    crate::paths::same_folder(at, dir).then_some(repo)
}

/// Whether the repository needs the git CLI for any command: hooks, sparse checkout, an index
/// or object format libgit2 may not keep, attributes asking for filters, submodules, or an
/// operation in progress.
pub(super) fn repo_needs_cli(repo: &Repository) -> bool {
    if repo.state() != RepositoryState::Clean || hooks(repo) {
        return true;
    }
    let Ok(config) = repo.config().and_then(|mut c| c.snapshot()) else { return true };
    let set = |key: &str| config.get_bool(key).unwrap_or(false);
    let has = |key: &str| config.get_entry(key).is_ok();
    if set("core.sparsecheckout") || set("core.splitindex") || has("core.fsmonitor") || has("core.hookspath") {
        return true;
    }
    if has("core.attributesfile") || has("commit.cleanup") || has("i18n.commitencoding") || has("core.worktree") {
        return true;
    }
    if has("author.name") || has("author.email") || has("committer.name") || has("committer.email") {
        return true;
    }
    // Repository extensions (object format, worktree config, partial clone) and promisor remotes.
    let local = config.entries(None).ok().map(|mut it| {
        let mut odd = false;
        while let Some(Ok(e)) = it.next() {
            let name = e.name().unwrap_or("").to_ascii_lowercase();
            odd |= name.starts_with("extensions.") || (name.starts_with("remote.") && name.ends_with(".promisor"));
        }
        odd
    });
    if local != Some(false) {
        return true;
    }
    let Some(work) = repo.workdir() else { return false };
    work.join(".gitmodules").exists() || attrs_need_cli(&std::fs::read_to_string(repo.path().join("info/attributes")).unwrap_or_default())
}

/// Whether a repository hook is installed (anything in `hooks/` but git's `.sample` files).
pub(super) fn hooks(repo: &Repository) -> bool {
    let Ok(entries) = std::fs::read_dir(repo.commondir().join("hooks")) else { return false };
    entries.flatten().any(|e| !e.file_name().to_string_lossy().ends_with(".sample"))
}

/// Whether attribute text asks for something only git does: a filter (Git LFS among them) or
/// a working-tree encoding.
pub(super) fn attrs_need_cli(text: &str) -> bool {
    text.contains("filter") || text.contains("working-tree-encoding")
}

/// Whether `index` holds a submodule or an attributes file asking for git's filters.
pub(super) fn index_needs_cli(repo: &Repository, index: &Index) -> bool {
    index.iter().any(|e| {
        let path = String::from_utf8_lossy(&e.path);
        if e.mode == 0o160000 {
            return true;
        }
        let attrs = path == ".gitattributes" || path.ends_with("/.gitattributes");
        attrs && repo.find_blob(e.id).map_or(true, |b| attrs_need_cli(&String::from_utf8_lossy(b.content())))
    })
}
