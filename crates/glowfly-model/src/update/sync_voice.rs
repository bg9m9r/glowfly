//! The success voice of a push or pull in the active tab: one short sentence that says what it
//! did ("Pushed 2 commits to origin/main.", "Published search to origin.", "Pulled 3 commits
//! into main.", "main is already up to date."), worked out from the snapshot taken before the
//! job and the ref lines git printed. Preferences > General > Notifications' Confirm pushes and
//! pulls turns the toast off; the activity log keeps the sentence either way. A commit gets no
//! toast at all: the graph shows it (`shown_in_graph`).

use crate::state::{AppState, RunningJob};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;

/// What a successful push or pull did, in words; None for any other job, a job in another tab,
/// or a failure.
pub(super) fn headline(st: &AppState, job: &RunningJob, ok: bool, output: &str) -> Option<String> {
    let snap =
        st.snap.as_ref().filter(|s| ok && job.workdir.as_deref().is_some_and(|w| glowfly_core::paths::same_spelling(w, &s.workdir)))?;
    let t = st.settings.lang();
    let label = job.label.as_str();
    if label == "Push" || label.starts_with("Push to ") {
        Some(pushed(t, snap, label.strip_prefix("Push to "), output))
    } else if label == "Pull" || label.starts_with("Pull (") {
        snap.head_branch.as_deref().map(|b| pulled(t, snap, b, output))
    } else {
        None
    }
}

/// A successful job labelled `label` shows in the graph, so it needs no toast: a commit.
pub(super) fn shown_in_graph(label: &str) -> bool {
    matches!(label, "Commit" | "Amend")
}

/// The ref lines of git's push or fetch report ("   a1b2c3d..e4f5a6b  main -> main", " * [new
/// branch]      search -> search"): (source, destination, whether it updated an existing ref).
/// The bracketed words may be translated; the arrow and the ids aren't.
fn ref_lines(output: &str) -> impl Iterator<Item = (&str, &str, bool)> {
    output.lines().filter(|l| !l.starts_with("$ ")).filter_map(|l| {
        let (left, right) = l.split_once(" -> ")?;
        let src = left.split_whitespace().last()?;
        let dst = right.split_whitespace().next()?;
        let updated = left.split_whitespace().any(|w| w.contains("..") && w.chars().all(|c| c.is_ascii_hexdigit() || c == '.'));
        Some((src, dst, updated))
    })
}

fn pushed(t: Lang, snap: &Snapshot, remote: Option<&str>, output: &str) -> String {
    let head = snap.head_branch.as_deref().unwrap_or_default();
    let Some((src, _, updated)) = ref_lines(output).next() else {
        // Nothing was sent: the remote branch already had everything.
        return t.trf("sync.up_to_date", &[("branch", &head)]);
    };
    let name = src.strip_prefix("refs/heads/").unwrap_or(src);
    let branch = snap.locals.iter().find(|b| b.name == name);
    let upstream = branch.and_then(crate::state::branches::upstream_label);
    let remote = remote.map(str::to_owned).or_else(|| upstream.as_deref().and_then(|u| u.split_once('/')).map(|(r, _)| r.to_owned()));
    let remote = remote.or_else(|| crate::state::branches::default_remote(snap)).unwrap_or_default();
    if !updated {
        return t.trf("sync.published", &[("branch", &name), ("remote", &remote)]);
    }
    let to = upstream.unwrap_or_else(|| format!("{remote}/{name}"));
    match branch.map_or(0, |b| b.ahead) {
        0 => t.trf("sync.pushed_branch", &[("branch", &name), ("upstream", &to)]),
        n => t.trn("sync.pushed", n, &[("upstream", &to)]),
    }
}

fn pulled(t: Lang, snap: &Snapshot, branch: &str, output: &str) -> String {
    let info = snap.head_info();
    let upstream = info.and_then(|b| b.upstream.as_deref());
    // The fetch moved the upstream past what the snapshot knew: the count is unknown.
    let moved = upstream.is_some_and(|u| ref_lines(output).any(|(_, dst, _)| dst == u));
    match (moved, info.map_or(0, |b| b.behind)) {
        (true, _) => t.trf("sync.pulled_new", &[("branch", &branch)]),
        (false, 0) => t.trf("sync.up_to_date", &[("branch", &branch)]),
        (false, n) => t.trn("sync.pulled", n, &[("branch", &branch)]),
    }
}
