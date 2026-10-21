//! The rules the toolbar's buttons and their tooltips share: why each git button is disabled
//! (the same sentence its tooltip shows), the head branch's ahead/behind line, when the
//! repository last fetched, and the Push, Pull, Stash and Pop previews built from the loaded
//! snapshot. Pure.

use super::AppState;
use super::tip::{PREVIEW_ROWS, TipPreview, TipRow, commit_rows};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{BranchInfo, Divergence, Snapshot};
use glowfly_core::settings::Settings;

#[cfg(test)]
#[path = "toolbar_tips_tests.rs"]
pub(crate) mod tests;

fn job_running(st: &AppState) -> Option<String> {
    st.job.is_some().then(|| st.settings.lang().tr("commit.blocked.job_running").to_owned())
}

/// Why an action that only needs no job running can't start (Force push, Publish).
pub fn busy_blocked(st: &AppState) -> Option<String> {
    job_running(st)
}

/// The checked-out branch's row, or None on a detached HEAD.
fn head(snap: &Snapshot) -> Option<&BranchInfo> {
    snap.head_branch.as_ref()?;
    snap.head_info()
}

/// Why Pull (the button) can't run: a job, a detached HEAD, no remote, or no upstream (a
/// tracking setting whose ref is gone counts as none).
pub fn pull_blocked(st: &AppState) -> Option<String> {
    pull_rule(st, |t| t.tr("toolbar.tip.blocked.detached_pull").to_owned())
}

/// Why the pull menu's `item` ("Pull (rebase)") can't run, naming it on a detached HEAD.
pub fn pull_variant_blocked(st: &AppState, item: &str) -> Option<String> {
    pull_rule(st, |t| t.trf("toolbar.tip.blocked.detached_item", &[("action", &item)]))
}

fn pull_rule(st: &AppState, detached: impl FnOnce(Lang) -> String) -> Option<String> {
    let t = st.settings.lang();
    if let Some(why) = job_running(st) {
        return Some(why);
    }
    let snap = st.snap.as_ref()?;
    let Some(b) = head(snap) else {
        return Some(detached(t));
    };
    if snap.remotes.is_empty() {
        return Some(t.tr("toolbar.tip.blocked.no_remote_pull").to_owned());
    }
    b.upstream.is_none().then(|| t.trf("toolbar.tip.blocked.no_upstream", &[("branch", &b.name)]))
}

/// Why Fetch all can't run: only a job, or no remote at all.
pub fn fetch_blocked(st: &AppState) -> Option<String> {
    job_running(st).or_else(|| {
        let snap = st.snap.as_ref()?;
        snap.remotes.is_empty().then(|| st.settings.lang().tr("toolbar.tip.blocked.no_remote_fetch").to_owned())
    })
}

/// Why Push can't run: a job or a detached HEAD. Without a remote it opens Publish instead.
pub fn push_blocked(st: &AppState) -> Option<String> {
    job_running(st).or_else(|| detached(st, |t| t.tr("toolbar.tip.blocked.detached_push")))
}

pub fn branch_blocked(st: &AppState) -> Option<String> {
    job_running(st)
}

/// Why Stash can't run: a job, or nothing to stash.
pub fn stash_blocked(st: &AppState) -> Option<String> {
    job_running(st).or_else(|| {
        let snap = st.snap.as_ref()?;
        (!snap.is_dirty()).then(|| st.settings.lang().tr("toolbar.tip.blocked.no_changes").to_owned())
    })
}

/// Why Pop can't run: a job, or no stash.
pub fn pop_blocked(st: &AppState) -> Option<String> {
    job_running(st).or_else(|| {
        let snap = st.snap.as_ref()?;
        snap.stashes.is_empty().then(|| st.settings.lang().tr("toolbar.tip.blocked.no_stashes").to_owned())
    })
}

/// Why the PR button can't run: a job, or a detached HEAD (in the service's own noun).
pub fn pr_blocked(st: &AppState) -> Option<String> {
    let merge_request = st.forge.as_ref().is_some_and(|f| f.pr_noun() == "merge request");
    job_running(st).or_else(|| {
        detached(st, |t| {
            if merge_request {
                t.tr("toolbar.tip.blocked.detached_merge_request")
            } else {
                t.tr("toolbar.tip.blocked.detached_pull_request")
            }
        })
    })
}

/// `text`'s sentence on a detached HEAD.
fn detached(st: &AppState, text: impl FnOnce(Lang) -> &'static str) -> Option<String> {
    let snap = st.snap.as_ref()?;
    head(snap).is_none().then(|| text(st.settings.lang()).to_owned())
}

/// The head branch against its upstream: "main is 2 ahead, 3 behind origin/main", the
/// up-to-date form, or "main isn't on a remote yet"; None on a detached HEAD.
pub fn sync_line(t: Lang, snap: &Snapshot) -> Option<String> {
    let b = head(snap)?;
    let Some(up) = &b.upstream else {
        return Some(t.trf("toolbar.tip.no_upstream", &[("branch", &b.name)]));
    };
    if b.ahead == 0 && b.behind == 0 {
        return Some(t.trf("toolbar.tip.up_to_date", &[("branch", &b.name), ("upstream", up)]));
    }
    let args: &[(&str, &dyn std::fmt::Display)] = &[("branch", &b.name), ("ahead", &b.ahead), ("behind", &b.behind), ("upstream", up)];
    Some(t.trf("toolbar.tip.ahead_behind", args))
}

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

/// "Last fetched 2 minutes ago" (just now, minutes, hours, days up to 30), then the Long date
/// format; "Not fetched yet" without a time.
pub fn fetched_line(t: Lang, settings: &Settings, fetched_at: Option<i64>, now: i64, utc_offset: i32) -> String {
    let Some(at) = fetched_at else {
        return t.tr("toolbar.tip.never_fetched").to_owned();
    };
    let age = now - at;
    let n = |unit: i64| (age / unit) as usize;
    if age < MINUTE {
        t.tr("toolbar.tip.fetched_just_now").to_owned()
    } else if age < HOUR {
        t.trn("toolbar.tip.fetched_minutes", n(MINUTE), &[])
    } else if age < DAY {
        t.trn("toolbar.tip.fetched_hours", n(HOUR), &[])
    } else if age <= 30 * DAY {
        t.trn("toolbar.tip.fetched_days", n(DAY), &[])
    } else {
        t.trf("toolbar.tip.fetched_on", &[("date", &settings.format_date_word(at, utc_offset))])
    }
}

/// What Push sends: the head's commits its upstream lacks.
pub fn push_preview(t: Lang, snap: &Snapshot) -> Option<TipPreview> {
    let b = head(snap)?;
    let up = b.upstream.as_deref()?;
    let heading = t.trn("toolbar.tip.push_sends", b.ahead, &[]);
    let mut p = side_preview(t, snap, b, b.ahead, heading, |d| &d.ahead)?;
    if b.behind > 0 {
        let behind = t.trf("toolbar.tip.push_behind_note", &[("upstream", &up), ("branch", &b.name)]);
        p.note = Some(match p.note.take() {
            Some(n) => format!("{behind}\n{n}"),
            None => behind,
        });
    }
    Some(p)
}

/// What Pull brings in: the upstream's commits the head lacks.
pub fn pull_preview(t: Lang, snap: &Snapshot) -> Option<TipPreview> {
    let b = head(snap)?;
    b.upstream.as_ref()?;
    let heading = t.trn("toolbar.tip.pull_brings", b.behind, &[]);
    side_preview(t, snap, b, b.behind, heading, |d| &d.behind)
}

/// The preview of one side of `b` and its upstream: `count` (BranchInfo's, which counts the
/// whole history) in the heading, the rows the loaded graph has, and a note when it lacks some.
fn side_preview(
    t: Lang,
    snap: &Snapshot,
    b: &BranchInfo,
    count: usize,
    heading: String,
    side: fn(&Divergence) -> &Vec<git2::Oid>,
) -> Option<TipPreview> {
    if count == 0 {
        return None;
    }
    let found = b.oid.zip(snap.upstream_oid(b)).and_then(|(ours, theirs)| snap.divergence(ours, theirs, PREVIEW_ROWS));
    let Some(d) = found else {
        return Some(TipPreview::new(heading, Vec::new()).note(Some(t.tr("toolbar.tip.not_loaded_all").to_owned())));
    };
    let rows = commit_rows(snap, side(&d));
    let short = !d.complete || rows.len() < count.min(PREVIEW_ROWS);
    let mut p = TipPreview::new(heading, rows).note(short.then(|| t.tr("toolbar.tip.not_loaded").to_owned()));
    p.more = count.saturating_sub(p.rows.len());
    Some(p)
}

/// The working tree's changed files, staged and unstaged merged (a path in both once, by its
/// staged entry, as `ChangeCounts::of` counts them), sorted by path.
pub fn changed_files(snap: &Snapshot) -> Vec<(char, &str)> {
    let mut files: Vec<(char, &str)> = Vec::new();
    for f in snap.staged.iter().chain(&snap.unstaged) {
        if !files.iter().any(|(_, p)| *p == f.path) {
            files.push((f.status, &f.path));
        }
    }
    files.sort_by(|a, b| a.1.cmp(b.1));
    files
}

/// What Stash saves: up to five changed files and how many more.
pub fn stash_preview(t: Lang, snap: &Snapshot) -> Option<TipPreview> {
    let files = changed_files(snap);
    if files.is_empty() {
        return None;
    }
    let rows = files.iter().take(PREVIEW_ROWS).map(|(s, p)| TipRow::File { status: *s, path: (*p).to_owned() }).collect();
    let mut p = TipPreview::new(t.tr("toolbar.tip.stash_heading"), rows);
    p.more = files.len().saturating_sub(PREVIEW_ROWS);
    Some(p)
}

/// What Pop applies: the newest stash's message, with a note that a dirty tree asks first.
pub fn pop_preview(t: Lang, snap: &Snapshot) -> Option<TipPreview> {
    let top = snap.stashes.first()?;
    let note = snap.is_dirty().then(|| t.tr("toolbar.tip.pop_dirty_note").to_owned());
    Some(TipPreview::new(t.tr("toolbar.tip.pop_heading"), vec![TipRow::Line(top.message.clone())]).note(note))
}
