//! Working inside a submodule from its parent's SUBMODULES row: Record this checkout (staging
//! the gitlink), and Fetch, Pull and Discard changes inside, which run as jobs in the
//! submodule's own folder. Why each is unavailable, the jobs' labels, Pull's refusal on a
//! detached HEAD or a branch without an upstream, Discard's confirmation and its success text.

use glowfly_core::i18n::Lang;
use glowfly_core::repo::{InsideHead, Snapshot, SubmoduleInfo};
use glowfly_core::undo::DISCARD_STASH_MESSAGE;
use std::path::PathBuf;

/// What a job inside a submodule does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsideKind {
    Fetch,
    Pull,
    Discard,
}

/// A job started inside a submodule, remembered so its end refreshes the parent (and Discard's
/// toast offers to open the submodule).
#[derive(Clone, Debug, PartialEq)]
pub struct InsideJob {
    /// The parent's working tree, the tab it started from.
    pub parent: PathBuf,
    /// The submodule's path in the parent (repo-relative, with '/').
    pub path: String,
    pub kind: InsideKind,
    /// The job's label (`label`).
    pub label: String,
}

/// Why Record this checkout is unavailable: not checked out, already what the index records,
/// or in a merge conflict (an unmerged gitlink is resolved in the conflict view).
pub fn record_blocked(t: Lang, sm: &SubmoduleInfo, snap: &Snapshot) -> Option<&'static str> {
    if !sm.is_initialized() {
        return Some(t.tr("sidebar.submodules.blocked.record_not_checked_out"));
    }
    if snap.unstaged.iter().any(|f| f.status == '!' && f.path == sm.path) {
        return Some(t.tr("sidebar.submodules.blocked.conflict"));
    }
    (!sm.is_out_of_sync()).then(|| t.tr("sidebar.submodules.blocked.in_sync"))
}

/// Why Fetch inside is unavailable: there is no repository there yet.
pub fn fetch_blocked(t: Lang, sm: &SubmoduleInfo) -> Option<&'static str> {
    (!sm.is_initialized()).then(|| t.tr("sidebar.submodules.blocked.not_checked_out"))
}

/// Why Pull inside is unavailable (its branch is checked when it starts: `pull_refused`).
pub fn pull_blocked(t: Lang, sm: &SubmoduleInfo) -> Option<&'static str> {
    fetch_blocked(t, sm)
}

/// Why Discard changes inside… is unavailable: nothing uncommitted or untracked inside.
pub fn discard_blocked(t: Lang, sm: &SubmoduleInfo) -> Option<&'static str> {
    (!sm.is_dirty()).then(|| t.tr("sidebar.submodules.blocked.nothing_inside"))
}

/// Why `kind` is unavailable for `sm`.
pub fn blocked(t: Lang, kind: InsideKind, sm: &SubmoduleInfo) -> Option<&'static str> {
    match kind {
        InsideKind::Fetch => fetch_blocked(t, sm),
        InsideKind::Pull => pull_blocked(t, sm),
        InsideKind::Discard => discard_blocked(t, sm),
    }
}

/// The log line when `kind` (None: Record this checkout) is refused for `path`.
pub fn refused(t: Lang, kind: Option<InsideKind>, path: &str, why: &str) -> String {
    let args: [(&str, &dyn std::fmt::Display); 2] = [("path", &path), ("why", &why)];
    match kind {
        None => t.trf("sidebar.submodules.inside.refused.record", &args),
        Some(InsideKind::Fetch) => t.trf("sidebar.submodules.inside.refused.fetch", &args),
        Some(InsideKind::Pull) => t.trf("sidebar.submodules.inside.refused.pull", &args),
        Some(InsideKind::Discard) => t.trf("sidebar.submodules.inside.refused.discard", &args),
    }
}

/// The job's label, in English: it names the job to the code, and `job_spec::labels` shows it
/// in the UI language.
pub fn label(kind: InsideKind, path: &str) -> String {
    match kind {
        InsideKind::Fetch => format!("Fetch in submodule {path}"),
        InsideKind::Pull => format!("Pull in submodule {path}"),
        InsideKind::Discard => format!("Discard changes inside {path}"),
    }
}

/// The git command a job of `kind` runs in the submodule's folder. Discard is Discard all's own
/// stash command, so its undo and its stash read the same.
pub fn cmds(kind: InsideKind) -> Vec<Vec<String>> {
    match kind {
        InsideKind::Fetch => vec![glowfly_core::gitcli::args(&["fetch", "--all", "--prune"])],
        InsideKind::Pull => vec![glowfly_core::gitcli::args(&["pull", "--ff"])],
        InsideKind::Discard => vec![glowfly_core::undo::discard_command()],
    }
}

/// Discard changes inside…'s confirmation: title, body and button.
pub fn discard_confirm(t: Lang, path: &str) -> (String, String, &'static str) {
    let title = t.trf("sidebar.submodules.inside.discard.title", &[("path", &path)]);
    let body = t.trf("sidebar.submodules.inside.discard.body", &[("path", &path), ("stash", &DISCARD_STASH_MESSAGE)]);
    (title, body, t.tr("sidebar.submodules.inside.discard.button"))
}

/// Why Pull inside doesn't start for the submodule at `path` whose HEAD is `head`: git pull
/// needs a branch with an upstream.
pub fn pull_refused(t: Lang, path: &str, head: &InsideHead) -> Option<String> {
    match (&head.branch, &head.upstream) {
        (None, _) => Some(t.trf("sidebar.submodules.inside.pull_detached", &[("path", &path)])),
        (Some(b), None) => Some(t.trf("sidebar.submodules.inside.pull_no_upstream", &[("path", &path), ("branch", b)])),
        (Some(_), Some(_)) => None,
    }
}

/// The toast for a job of `kind` that ended: Discard's success names its stash; anything else
/// (a failure included, which the failure toast explains) keeps the usual headline.
pub fn headline(t: Lang, kind: InsideKind, path: &str, ok: bool) -> Option<String> {
    (ok && kind == InsideKind::Discard)
        .then(|| t.trf("sidebar.submodules.inside.discarded", &[("path", &path), ("stash", &DISCARD_STASH_MESSAGE)]))
}
