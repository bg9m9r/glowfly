//! The rules the toolbar's buttons and their tooltips share: why each git button is disabled
//! (the same sentence its tooltip shows), the head branch's ahead/behind line, when the
//! repository last fetched, and the Push, Pull, Stash and Pop previews built from the loaded
//! snapshot. Push names, counts and previews against where a plain `git push` goes
//! (`branches::plain_push`), which config may make another remote than the upstream's. Pure.

use super::AppState;
use super::branches::plain_push;
use super::tip::{PREVIEW_ROWS, TipPreview, TipRow, commit_rows};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{BranchInfo, Divergence, Snapshot};
use glowfly_core::settings::Settings;

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
// Its helpers serve the other crates' tests too (the test-support feature); without the tests
// some of them go unused.
#[cfg_attr(not(test), allow(dead_code, unused_imports))]
#[path = "toolbar_tips_tests.rs"]
pub mod tests;

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

/// Why Push can't run: a job, a detached HEAD, or git refusing a plain push of the head
/// branch (`plain_push::refused`). Without a remote it opens Publish instead.
pub fn push_blocked(st: &AppState) -> Option<String> {
    job_running(st).or_else(|| detached(st, |t| t.tr("toolbar.tip.blocked.detached_push"))).or_else(|| {
        let snap = st.snap.as_ref()?;
        head(snap).and_then(|b| plain_push::refused(st.settings.lang(), b))
    })
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

/// Why Pop can't run: a job, no stash, or conflicted files (git can't apply a stash then).
pub fn pop_blocked(st: &AppState) -> Option<String> {
    job_running(st).or_else(|| {
        let snap = st.snap.as_ref()?;
        let t = st.settings.lang();
        if snap.stashes.is_empty() {
            return Some(t.tr("toolbar.tip.blocked.no_stashes").to_owned());
        }
        super::stash_confirm::apply_blocked(snap, t).map(str::to_owned)
    })
}

/// Why the PR button can't run: a job, or a detached HEAD (in the service's own noun).
pub fn pr_blocked(st: &AppState) -> Option<String> {
    let merge_request = st.session.forge.as_ref().is_some_and(|f| f.pr_noun() == "merge request");
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

/// Where Push sends the head branch, as its menu's first item and its tooltip's title say:
/// "Push main to origin/main", or where config sends a plain `git push` instead ("Push main to
/// fork/main"), "Push main to fork" when git refuses that push, or without an upstream
/// "Publish search to origin (creates origin/search)" on the remote Push picks. None on a
/// detached HEAD or without a remote.
pub fn push_target(t: Lang, snap: &Snapshot) -> Option<String> {
    let (template, names) = push_target_parts(t, snap)?;
    let args: Vec<(&str, &dyn std::fmt::Display)> = names.iter().map(|(slot, name)| (*slot, name as &dyn std::fmt::Display)).collect();
    Some(glowfly_core::i18n::fill(template, &args))
}

/// `push_target` as its template and the names that fill it, for a menu that cuts long names.
pub fn push_target_parts(t: Lang, snap: &Snapshot) -> Option<(&'static str, [(&'static str, String); 2])> {
    let b = head(snap)?;
    if let Some(p) = plain_push::resolved(b).filter(|p| p.dest.is_err()) {
        return Some((t.tr("toolbar.tip.push_refused_title"), [("branch", b.name.clone()), ("remote", p.remote.clone())]));
    }
    if let Some((dest, _)) = push_ahead(snap) {
        return Some((t.tr("toolbar.tip.push_title"), [("branch", b.name.clone()), ("upstream", dest)]));
    }
    let remote = super::branches::default_remote(snap)?;
    Some((t.tr("toolbar.tip.push_publish_title"), [("branch", b.name.clone()), ("remote", remote)]))
}

/// Where a plain `git push` of the head branch (which has an upstream) sends it, as a label
/// ("origin/main", "fork/main"), and how many commits that lacks, counted over the whole
/// history at load (None when that ref isn't known: the push creates it). None on a detached
/// HEAD, without an upstream, or when git refuses the push.
pub fn push_ahead(snap: &Snapshot) -> Option<(String, Option<usize>)> {
    let b = head(snap)?;
    match plain_push::resolved(b) {
        Some(p) => {
            let d = p.dest.as_ref().ok()?;
            let known = d.tip.is_some() || plain_push::to_upstream(b, d);
            Some((p.label(d), known.then(|| plain_push::counts(b, d).0)))
        }
        None => Some((b.upstream.clone()?, Some(b.ahead))),
    }
}

/// The Push tooltip's note on where a plain push goes: a configured push remote that is
/// missing, or a matching or mirror push (`plain_push::note`).
pub fn push_note(t: Lang, snap: &Snapshot) -> Option<String> {
    head(snap).and_then(|b| plain_push::note(t, b))
}

/// Pull's title: "Pull origin/main into main (rebase)", or "(merge)", as a plain `git pull` of
/// the head branch would; None on a detached HEAD or without an upstream.
pub fn pull_target(t: Lang, snap: &Snapshot) -> Option<String> {
    let b = head(snap)?;
    let up = b.upstream.as_ref()?;
    let args: &[(&str, &dyn std::fmt::Display)] = &[("upstream", up), ("branch", &b.name)];
    Some(if snap.pull_rebase { t.trf("toolbar.tip.pull_title_rebase", args) } else { t.trf("toolbar.tip.pull_title_merge", args) })
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

/// What Push sends: the head's commits where a plain push goes lacks (its upstream, unless
/// config sends it elsewhere). None when git refuses the push or nothing is known there.
pub fn push_preview(t: Lang, snap: &Snapshot) -> Option<TipPreview> {
    let b = head(snap)?;
    b.upstream.as_ref()?;
    let upstream = || (super::branches::upstream_label(b), snap.upstream_oid(b), (b.ahead, b.behind));
    let (label, theirs, (ahead, behind)) = match plain_push::resolved(b) {
        Some(p) => match p.dest.as_ref().ok()? {
            d if plain_push::to_upstream(b, d) => upstream(),
            d => (Some(p.label(d)), Some(d.tip?), (d.ahead, d.behind)),
        },
        None => upstream(),
    };
    let label = label?;
    let heading = t.trn("toolbar.tip.push_sends", ahead, &[]);
    let mut p = side_preview(t, snap, b, (theirs, ahead), heading, |d| &d.ahead)?;
    if behind > 0 {
        let behind = t.trf("toolbar.tip.push_behind_note", &[("upstream", &label), ("branch", &b.name)]);
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
    side_preview(t, snap, b, (snap.upstream_oid(b), b.behind), heading, |d| &d.behind)
}

/// The preview of one side of `b` and `theirs` (its upstream's tip, or where Push goes): `count`
/// (counted over the whole history at load) in the heading, the rows the loaded graph has, and
/// a note when it lacks some.
fn side_preview(
    t: Lang,
    snap: &Snapshot,
    b: &BranchInfo,
    (theirs, count): (Option<git2::Oid>, usize),
    heading: String,
    side: fn(&Divergence) -> &Vec<git2::Oid>,
) -> Option<TipPreview> {
    if count == 0 {
        return None;
    }
    let found = b.oid.zip(theirs).and_then(|(ours, theirs)| snap.divergence(ours, theirs, PREVIEW_ROWS));
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
