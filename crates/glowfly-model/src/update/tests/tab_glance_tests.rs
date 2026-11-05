//! Headless tests for what a tab shows at a glance (`state/tabs/glance.rs`): a parked tab keeps
//! the counts of the loads and whole status reads that arrive for it (`Inactive::Counts`), a
//! read of some paths leaves them, a job's failure marks the tab it ran in until it is dismissed
//! (`LogsMsg::DismissFailure`) or a later run succeeds, and the running job marks its tab.

use super::tests::{said, scratch_repo, state};
use super::update;
use crate::effect::{LoadKind, StatusRead};
use crate::logs::LogsMsg;
use crate::msg::{JobMsg, LoadMsg, Msg, RepoMsg, TabsMsg};
use crate::state::tabs::glance::{Glance, Mark};
use crate::state::{AppState, RunningJob, Tab};
use git2::RepositoryState;
use glowfly_core::repo::{FileEntry, LoadOptions, PathsStatus, load};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn send(st: &mut AppState, msg: Msg) {
    update(st, msg, Instant::now());
}

/// `msg` for the repository at `dir`, as the runner scopes its results.
fn scoped(dir: &Path, msg: Msg) -> Msg {
    Msg::Scoped { workdir: dir.to_path_buf(), msg: Box::new(msg) }
}

/// A state whose first tab shows `dir` (one modified file), parked behind a new empty tab.
fn parked(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    st.tabs[0] = Tab::unloaded(dir.clone());
    st.snap = Some(load(&dir, &LoadOptions::all(10)).unwrap());
    send(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert_eq!(st.active_tab, 1);
    (st, dir)
}

fn status_loaded(result: StatusRead) -> Msg {
    Msg::Load(LoadMsg::StatusLoaded {
        generation: 0,
        result: Ok(result),
        submodules: None,
        open_stamp: None,
        slow: None,
        lfs_patterns: None,
        index: None,
    })
}

/// A running fetch in `dir`.
pub(super) fn job(dir: &Path) -> RunningJob {
    RunningJob {
        label: "Fetch all".into(),
        workdir: Some(dir.to_path_buf()),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_continue: false,
        then_pr: None,
        then_link: false,
        keeps_undo: true,
        fetches: true,
    }
}

fn done(st: &mut AppState, ok: bool) {
    send(st, Msg::Job(JobMsg::JobDone { ok, output: if ok { String::new() } else { "fatal: unable to access".into() }, undo: None }));
}

#[test]
fn a_parked_tab_shows_its_snapshot_counts() {
    let (st, dir) = parked("glance-parked");
    let g = st.tab_glance(0);
    assert_eq!(g.counts.as_ref().map(|c| c.changed), Some(1), "a.txt is modified");
    assert_eq!(g.mark(), Some(Mark::Changed));
    assert_eq!(st.tab_glance(1).mark(), None, "the new tab has nothing to show");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_load_for_a_parked_tab_keeps_its_counts() {
    let (mut st, dir) = parked("glance-load");
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    let snap = load(&dir, &LoadOptions::all(10)).unwrap();
    send(&mut st, scoped(&dir, Msg::Repo(RepoMsg::RepoLoaded { generation: 0, kind: LoadKind::Refresh, result: Ok(Box::new(snap)) })));
    assert_eq!(st.active_tab, 1, "still parked");
    assert_eq!(st.tab_glance(0).counts.map(|c| c.changed), Some(2));
    // A failed load leaves them.
    send(&mut st, scoped(&dir, Msg::Repo(RepoMsg::RepoLoaded { generation: 0, kind: LoadKind::Refresh, result: Err(said("gone")) })));
    assert_eq!(st.tab_glance(0).counts.map(|c| c.changed), Some(2));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_whole_status_read_for_a_parked_tab_moves_its_changes_and_keeps_its_branch_counts() {
    let (mut st, dir) = parked("glance-status");
    let behind = Glance { behind: 2, upstream: Some("origin/main".into()), ..Glance::default() };
    st.tabs[0].parked.as_mut().unwrap().glance = Some(behind);
    let rows = vec![FileEntry::new("a.txt", '!'), FileEntry::new("b.txt", 'M')];
    send(&mut st, scoped(&dir, status_loaded(StatusRead::Whole(rows, Vec::new(), RepositoryState::Merge))));
    let c = st.tab_glance(0).counts.unwrap();
    assert_eq!((c.changed, c.conflicted, c.op), (2, 1, Some(RepositoryState::Merge)));
    assert_eq!((c.behind, c.upstream.as_deref()), (2, Some("origin/main")), "a status read leaves the branch counts");
    assert_eq!(st.tab_glance(0).mark(), Some(Mark::InProgress));

    // A read of some paths can't say how many files changed: the counts stay.
    send(
        &mut st,
        scoped(
            &dir,
            status_loaded(StatusRead::Paths(PathsStatus {
                covered: vec!["a.txt".into()],
                unstaged: Vec::new(),
                state: RepositoryState::Clean,
                submodule_touched: false,
            })),
        ),
    );
    assert_eq!(st.tab_glance(0).counts.unwrap().changed, 2);

    // An unchanged repository's status read moves them too.
    let unchanged = LoadMsg::RepoUnchanged {
        generation: 0,
        result: Ok((Vec::new(), Vec::new(), RepositoryState::Clean)),
        submodules: None,
        open_stamp: None,
        slow: None,
        lfs_patterns: None,
        index: None,
    };
    send(&mut st, scoped(&dir, Msg::Load(unchanged)));
    let g = st.tab_glance(0);
    assert_eq!(g.counts.as_ref().map(|c| (c.changed, c.op)), Some((0, None)));
    assert_eq!(g.mark(), Some(Mark::Behind(2)), "clean, and behind its upstream");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_parked_tab_shows_counts_from_its_snapshot_again_once_shown() {
    let (mut st, dir) = parked("glance-shown");
    st.tabs[0].parked.as_mut().unwrap().glance = Some(Glance { changed: 9, ..Glance::default() });
    send(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert_eq!(st.active_tab, 0);
    assert_eq!(st.tab_glance(0).counts.map(|c| c.changed), Some(1), "the active tab's snapshot");
    send(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)));
    assert_eq!(st.tab_glance(0).counts.map(|c| c.changed), Some(1), "parked again, with no read since");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_job_marks_its_tab_while_it_runs_and_its_failure_after() {
    let (mut st, dir) = parked("glance-job");
    st.job = Some(job(&dir));
    assert_eq!(st.tab_glance(0).mark(), Some(Mark::Running), "it runs in the parked tab's repository");
    assert_eq!(st.tab_glance(1).mark(), None);
    done(&mut st, false);
    let g = st.tab_glance(0);
    assert_eq!(g.failed.as_ref().map(|f| f.label.as_str()), Some("Fetch all"));
    assert_eq!(g.mark(), Some(Mark::Failed), "the parked tab keeps its job's outcome");
    assert!(st.tab_glance(1).failed.is_none(), "only the tab it ran in");

    // A later success in that repository clears it.
    st.job = Some(job(&dir));
    done(&mut st, true);
    assert!(st.tab_glance(0).failed.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dismissed_failure_is_no_longer_shown() {
    let (mut st, dir) = parked("glance-dismiss");
    send(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    st.job = Some(job(&dir));
    done(&mut st, false);
    let failure = st.tab_glance(0).failed.expect("the active tab's failure");
    send(&mut st, Msg::Logs(LogsMsg::DismissFailure(failure.entry)));
    assert!(st.tab_glance(0).failed.is_none());
    assert_eq!(st.tab_glance(0).mark(), Some(Mark::Changed), "the next state shows");
    // Parked, it stays dismissed.
    send(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)));
    assert!(st.tab_glance(0).failed.is_none());
    // A dismissed failure is for inactive tabs a UI intent only: nothing is kept.
    assert!(LogsMsg::DismissFailure(failure.entry).on_inactive().is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
