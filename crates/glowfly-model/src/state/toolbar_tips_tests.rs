//! The toolbar's shared rules: blocked reasons, the ahead/behind line, the fetch age and the
//! Push, Pull, Stash and Pop previews, on a one-commit fixture whose snapshot the tests edit
//! (made-up commits added to its graph, an upstream for main).

use super::*;
use crate::state::op_fixture;
use crate::state::tip::TipRow;
use crate::state::{AppState, RunningJob};
use git2::Oid;
use glowfly_core::repo::{CommitInfo, StashInfo, Tracking};
use std::time::Instant;

/// The one-commit fixture with its branch called main.
pub fn state(name: &str) -> AppState {
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    let mut snap = op_fixture::snapshot(name);
    snap.head_branch = Some("main".into());
    snap.locals.iter_mut().filter(|b| b.is_head).for_each(|b| b.name = "main".into());
    st.snap = Some(snap);
    st
}

fn fake(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn commit(oid: Oid, parent: Oid, summary: &str) -> CommitInfo {
    CommitInfo {
        oid,
        summary: summary.into(),
        body: Default::default(),
        author: glowfly_core::repo::Author::new("T", "t@example.com"),
        time: 0,
        parents: [parent].into_iter().collect(),
    }
}

/// Puts `ahead` commits on main and `behind` on origin/main over the fixture's commit, and makes
/// origin/main main's upstream with those counts. `loaded` false leaves origin/main's commits
/// out of the graph. Each call starts again from the fixture's own commit ("one").
pub fn diverge(snap: &mut Snapshot, ahead: u8, behind: u8, loaded: bool) {
    let mut kept = snap.commits.to_vec();
    kept.retain(|c| c.summary == "one");
    let base = kept[0].oid;
    let mut rows = Vec::new();
    let mut tip = base;
    for n in 1..=ahead {
        rows.push(commit(fake(n), tip, &format!("Ahead {n}")));
        tip = fake(n);
    }
    let main = tip;
    tip = base;
    for n in 1..=behind {
        let oid = fake(100 + n);
        if loaded {
            rows.push(commit(oid, tip, &format!("Behind {n}")));
        }
        tip = oid;
    }
    let upstream = tip;
    rows.reverse();
    rows.append(&mut kept);
    snap.commits = rows.into();
    snap.row_of = snap.commits.iter().enumerate().map(|(i, c)| (c.oid, i)).collect();
    snap.head_oid = Some(main);
    snap.remotes = vec![("origin".into(), vec![("main".into(), upstream)])];
    let head = snap.locals.iter_mut().find(|b| b.is_head).unwrap();
    head.oid = Some(main);
    head.upstream = Some("origin/main".into());
    head.tracking = Some(Tracking { remote: "origin".into(), merge: "main".into() });
    head.ahead = ahead.into();
    head.behind = behind.into();
}

pub fn running(st: &mut AppState) {
    st.job = Some(RunningJob {
        label: "Fetch all".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: true,
    });
}

fn detach(st: &mut AppState) {
    st.snap.as_mut().unwrap().head_branch = None;
}

#[test]
fn sync_line_reads_the_head_branch() {
    let mut st = state("tips-sync");
    let en = Lang::En;
    assert_eq!(sync_line(en, st.snap.as_ref().unwrap()).as_deref(), Some("main isn't on a remote yet"));
    let snap = st.snap.as_mut().unwrap();
    diverge(snap, 2, 0, true);
    assert_eq!(sync_line(en, snap).as_deref(), Some("main is 2 ahead, 0 behind origin/main"));
    diverge(snap, 0, 0, true);
    assert_eq!(sync_line(en, snap).as_deref(), Some("main is up to date with origin/main"));
    detach(&mut st);
    assert_eq!(sync_line(en, st.snap.as_ref().unwrap()), None);
}

#[test]
fn blocked_rules_give_their_reasons() {
    let mut st = state("tips-blocked");
    // One commit, no remote, no stash, a clean tree.
    assert_eq!(pull_blocked(&st).as_deref(), Some("This repository has no remote to pull from."));
    assert_eq!(fetch_blocked(&st).as_deref(), Some("This repository has no remote to fetch from."));
    assert_eq!(push_blocked(&st), None, "Push opens Publish");
    assert_eq!(stash_blocked(&st).as_deref(), Some("No changes to stash"));
    assert_eq!(pop_blocked(&st).as_deref(), Some("No stashes to pop"));
    assert_eq!(branch_blocked(&st), None);

    // A remote, but main tracks nothing on it.
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), Vec::new())];
    assert_eq!(pull_blocked(&st).as_deref(), Some("main has no upstream branch: push it first, or set one from its menu."));
    assert_eq!(fetch_blocked(&st), None);
    diverge(st.snap.as_mut().unwrap(), 1, 0, true);
    assert_eq!(pull_blocked(&st), None);

    // A tracking setting whose ref is gone counts as none.
    st.snap.as_mut().unwrap().locals[0].upstream = None;
    assert!(pull_blocked(&st).is_some_and(|w| w.contains("no upstream")));

    detach(&mut st);
    assert_eq!(push_blocked(&st).as_deref(), Some("Push isn't available on a detached HEAD: check out a branch first."));
    assert_eq!(pull_blocked(&st).as_deref(), Some("Pull isn't available on a detached HEAD: check out a branch first."));
    assert_eq!(pull_variant_blocked(&st, "Pull (rebase)").as_deref(), Some("Pull (rebase) isn't available on a detached HEAD."));
    assert_eq!(pr_blocked(&st).as_deref(), Some("Check out a branch to create a pull request"));
    assert_eq!(fetch_blocked(&st), None, "Fetch all doesn't need a branch");

    let snap = st.snap.as_mut().unwrap();
    snap.stashes = vec![StashInfo { index: 0, message: "On main: wip".into(), oid: fake(9) }];
    snap.unstaged = op_fixture::rows(&[("a.txt", 'M')]);
    assert_eq!((stash_blocked(&st), pop_blocked(&st)), (None, None));
    running(&mut st);
    let job = Lang::En.tr("commit.blocked.job_running");
    for why in [
        pull_blocked(&st),
        pull_variant_blocked(&st, "Pull (rebase)"),
        fetch_blocked(&st),
        push_blocked(&st),
        branch_blocked(&st),
        stash_blocked(&st),
        pop_blocked(&st),
        pr_blocked(&st),
    ] {
        assert_eq!(why.as_deref(), Some(job));
    }
}

#[test]
fn busy_blocked_only_waits_for_a_job() {
    let mut st = state("tips-busy");
    assert_eq!(busy_blocked(&st), None);
    detach(&mut st);
    assert_eq!(busy_blocked(&st), None, "no branch needed");
    running(&mut st);
    assert_eq!(busy_blocked(&st).as_deref(), Some(Lang::En.tr("commit.blocked.job_running")));
}

#[test]
fn push_preview_lists_what_it_sends() {
    let mut st = state("tips-push");
    let snap = st.snap.as_mut().unwrap();
    diverge(snap, 2, 0, true);
    let p = push_preview(Lang::En, snap).unwrap();
    assert_eq!(p.heading, "Sends 2 commits");
    assert_eq!(p.rows.len(), 2);
    assert!(matches!(&p.rows[0], TipRow::Commit { sha, summary } if sha.len() == 7 && summary == "Ahead 2"), "{:?}", p.rows);
    assert_eq!((p.more, p.note.as_deref()), (0, None));

    diverge(snap, 7, 0, true);
    let p = push_preview(Lang::En, snap).unwrap();
    assert_eq!((p.rows.len(), p.more), (5, 2));

    diverge(snap, 2, 1, false);
    let p = push_preview(Lang::En, snap).unwrap();
    assert_eq!(p.heading, "Sends 2 commits");
    assert!(p.rows.is_empty());
    let note = p.note.unwrap();
    assert!(note.contains("These commits aren't in the loaded history."), "{note}");
    assert!(note.contains("origin/main has commits main doesn't; pull first"), "{note}");

    diverge(snap, 0, 3, true);
    assert_eq!(push_preview(Lang::En, snap), None, "nothing to send");
    let p = pull_preview(Lang::En, snap).unwrap();
    assert_eq!((p.heading.as_str(), p.rows.len()), ("Brings in 3 commits", 3));
}

#[test]
fn a_short_walk_says_some_commits_are_missing() {
    let mut st = state("tips-short");
    let snap = st.snap.as_mut().unwrap();
    diverge(snap, 2, 0, true);
    // BranchInfo counts the whole history: more than the graph holds.
    snap.locals[0].ahead = 4;
    let p = push_preview(Lang::En, snap).unwrap();
    assert_eq!((p.heading.as_str(), p.rows.len(), p.more), ("Sends 4 commits", 2, 2));
    assert_eq!(p.note.as_deref(), Some("Some of these commits aren't in the loaded history."));
}

#[test]
fn fetched_line_buckets() {
    let s = Settings::default();
    let now = 1_769_947_200;
    let line = |ago: Option<i64>| fetched_line(Lang::En, &s, ago.map(|a| now - a), now, 0);
    assert_eq!(line(Some(30)), "Last fetched just now");
    assert_eq!(line(Some(120)), "Last fetched 2 minutes ago");
    assert_eq!(line(Some(60)), "Last fetched 1 minute ago");
    assert_eq!(line(Some(3 * 3600)), "Last fetched 3 hours ago");
    assert_eq!(line(Some(2 * 86400)), "Last fetched 2 days ago");
    let old = now - 40 * 86400;
    assert_eq!(line(Some(40 * 86400)), format!("Last fetched {}", s.format_date_word(old, 0)));
    assert_eq!(line(None), "Not fetched yet");
    let german = fetched_line(Lang::De, &s, Some(now - 120), now, 0);
    assert_eq!(german, Lang::De.trn("toolbar.tip.fetched_minutes", 2, &[]));
    assert_ne!(german, "Last fetched 2 minutes ago");
}

#[test]
fn stash_and_pop_previews() {
    let mut st = state("tips-stash");
    let snap = st.snap.as_mut().unwrap();
    assert_eq!(stash_preview(Lang::En, snap), None);
    snap.staged = op_fixture::rows(&[("b.txt", 'A'), ("a.txt", 'M')]);
    snap.unstaged = op_fixture::rows(&[("a.txt", 'M'), ("g.txt", 'D'), ("c.txt", '?'), ("e.txt", 'M'), ("f.txt", 'M'), ("d.txt", 'M')]);
    let p = stash_preview(Lang::En, snap).unwrap();
    let paths: Vec<&str> =
        p.rows.iter().filter_map(|r| if let TipRow::File { path, .. } = r { Some(path.as_str()) } else { None }).collect();
    assert_eq!(paths, ["a.txt", "b.txt", "c.txt", "d.txt", "e.txt"], "merged, once each, by path");
    assert_eq!((p.heading.as_str(), p.more), ("Stashes", 2));

    assert_eq!(pop_preview(Lang::En, snap), None);
    snap.stashes = vec![StashInfo { index: 0, message: "On main: wip".into(), oid: fake(9) }];
    let p = pop_preview(Lang::En, snap).unwrap();
    assert_eq!((p.heading.as_str(), &p.rows[..]), ("Applies", &[TipRow::Line("On main: wip".into())][..]));
    assert_eq!(p.note.as_deref(), Some("You have local changes: Pop asks before applying."));
}
