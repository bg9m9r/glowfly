//! Partial clones (`git clone --filter=blob:none`): file contents the object store doesn't have
//! yet, which the remote sends on request. git fetches them lazily; libgit2 can't, so the reads
//! Trellis makes with libgit2 (diffs, the File view, blame's file) check first ([`require`]) and
//! report what is missing as [`Error::MissingContent`]. The app then downloads exactly those
//! objects ([`fetch_cmds`], the same fetch git's own lazy fetch runs) and reads again.
//!
//! The promisor remote is `extensions.partialClone`, else the first remote with
//! `remote.<name>.promisor` true. libgit2 refuses to open a repository that names an extension
//! it doesn't know, so [`allow_extension`] registers `partialclone` once at startup.

use crate::error::{Error, Result};
use crate::gitcli;
use crate::repo::DiffTarget;
use git2::{DiffDelta, Oid, Repository};
use std::path::Path;

/// How many objects one download command asks for (about 8 KB of arguments).
pub const FETCH_BATCH: usize = 200;

/// File contents a read needs that the partial clone hasn't downloaded.
#[derive(Clone, Debug, PartialEq)]
pub struct MissingContent {
    pub oids: Vec<Oid>,
    /// The promisor remote they come from.
    pub remote: String,
    /// The file being read.
    pub path: String,
}

/// Lets libgit2 open repositories with `extensions.partialclone` set (written by older git).
pub fn allow_extension() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // SAFETY: libgit2 keeps the list in a process-global; this runs once, before any
        // repository opens (main calls it first, and every test helper that makes or opens a
        // repository calls it before doing so).
        let _ = unsafe { git2::opts::set_extensions(&["partialclone"]) };
    });
}

/// The promisor remote of the repository at `workdir`: `extensions.partialClone`, else the
/// first remote whose `promisor` setting is true (read as git reads booleans). None when it
/// isn't a partial clone.
pub fn promisor_remote(workdir: &Path) -> Option<String> {
    let ok_name = |n: &str| !n.is_empty() && !n.starts_with('-');
    let named = gitcli::read(workdir, &gitcli::args(&["config", "--get", "extensions.partialClone"]));
    let name = named.stdout.trim();
    if named.ok && ok_name(name) {
        return Some(name.to_owned());
    }
    let listed = gitcli::read(workdir, &gitcli::args(&["config", "--type=bool", "--get-regexp", r"^remote\..*\.promisor$"]));
    listed.stdout.lines().find_map(|line| {
        let (key, value) = line.rsplit_once(' ')?;
        let name = key.strip_prefix("remote.")?.strip_suffix(".promisor")?;
        (value.trim() == "true" && ok_name(name)).then(|| name.to_owned())
    })
}

/// Whether the repository's config names a promisor remote or the partial clone extension (a
/// cheap check through libgit2, before asking git with [`promisor_remote`]).
pub fn looks_partial(repo: &Repository) -> bool {
    let Ok(config) = repo.config() else { return false };
    let Ok(entries) = config.entries(Some(r"^(extensions\.partialclone|remote\..*\.promisor)$")) else { return false };
    let mut found = false;
    let _ = entries.for_each(|_| found = true);
    found
}

/// Fails with [`Error::MissingContent`] when some of `oids` (zero ids skipped) aren't in the
/// object store of a partial clone. Otherwise (everything there, or not a partial clone) Ok,
/// and the read goes on to fail as it would have.
pub(crate) fn require(repo: &Repository, workdir: &Path, path: &str, oids: impl IntoIterator<Item = Oid>) -> Result<()> {
    let odb = repo.odb()?;
    let mut missing: Vec<Oid> = Vec::new();
    for id in oids {
        if !id.is_zero() && !missing.contains(&id) && !odb.exists(id) {
            missing.push(id);
        }
    }
    if missing.is_empty() || !looks_partial(repo) {
        return Ok(());
    }
    let Some(remote) = promisor_remote(workdir) else { return Ok(()) };
    Err(Error::MissingContent(MissingContent { oids: missing, remote, path: path.to_owned() }))
}

/// The blobs of `d` that `target`'s diff reads from the object store: the tree and index
/// sides, never the work tree's, never a submodule's commit, and never a side that is absent.
pub(crate) fn delta_blobs(target: &DiffTarget, d: &DiffDelta) -> Vec<Oid> {
    let (old, new) = match target {
        DiffTarget::Commit(..) | DiffTarget::Between(..) | DiffTarget::Staged(_) => (true, true),
        DiffTarget::ToWorkdir(..) | DiffTarget::Unstaged(_) => (true, false),
        DiffTarget::FromWorkdir(..) => (false, true),
    };
    let blob = |f: git2::DiffFile<'_>| (u32::from(f.mode()) != u32::from(git2::FileMode::Commit) && !f.id().is_zero()).then(|| f.id());
    let mut out = Vec::new();
    out.extend(old.then(|| blob(d.old_file())).flatten());
    out.extend(new.then(|| blob(d.new_file())).flatten());
    out
}

/// Every version of `path` in the history of `at`, old and new side of each change, as
/// `git log --raw` lists them from tree diffs (no rename detection, which would read blobs,
/// and no lazy fetching). Empty when git can't say.
pub(crate) fn history_blobs(workdir: &Path, at: Oid, path: &str) -> Vec<Oid> {
    let a = [
        "--literal-pathspecs",
        "log",
        "--format=",
        "--raw",
        "--no-abbrev",
        "--no-renames",
        "--no-textconv",
        "--no-ext-diff",
        "-z",
        &at.to_string(),
        "--",
        path,
    ];
    let env = [gitcli::NO_PROMPTS, &[("GIT_NO_LAZY_FETCH", "1")]].concat();
    let (out, stdout) = gitcli::output_bytes(workdir, &gitcli::args(&a), &env, Some(gitcli::READ_TIMEOUT));
    if !out.ok {
        return Vec::new();
    }
    parse_raw(&stdout)
}

/// The old and new ids of each `:<mode> <mode> <old> <new> <status>` record of `-z` raw output.
fn parse_raw(bytes: &[u8]) -> Vec<Oid> {
    let mut out: Vec<Oid> = Vec::new();
    for token in bytes.split(|b| *b == 0) {
        let text = String::from_utf8_lossy(token);
        let Some(record) = text.trim_start_matches(['\n', '\r']).strip_prefix(':') else { continue };
        let fields: Vec<&str> = record.split_whitespace().collect();
        for f in fields.iter().skip(2).take(2) {
            if let Ok(id) = Oid::from_str(f)
                && !id.is_zero()
                && !out.contains(&id)
            {
                out.push(id);
            }
        }
    }
    out
}

/// The commands that download `oids` from `remote`, [`FETCH_BATCH`] at a time: no tags, no
/// FETCH_HEAD, no submodules, and the noop negotiator, so the remote sends every object asked
/// for even when a commit we have reaches it.
pub fn fetch_cmds(remote: &str, oids: &[Oid]) -> Vec<Vec<String>> {
    oids.chunks(FETCH_BATCH)
        .map(|batch| {
            let mut cmd = gitcli::args(&[
                "-c",
                "fetch.negotiationAlgorithm=noop",
                "fetch",
                remote,
                "--no-tags",
                "--no-write-fetch-head",
                "--recurse-submodules=no",
                "--filter=blob:none",
            ]);
            cmd.extend(batch.iter().map(Oid::to_string));
            cmd
        })
        .collect()
}

#[cfg(test)]
mod tests;
