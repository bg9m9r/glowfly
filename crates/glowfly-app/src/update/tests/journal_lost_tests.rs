//! Headless tests for the journal's Lost commits tab: loading it only while it shows, stale
//! and closed results, refreshes, the selected commit's history, Restore, Create branch here,
//! the stash actions and parking it with its tab.

use super::journal_tests::{open_loaded, opened};
use super::tests::{drive, finish, git};
use super::update;
use crate::effect::{Effect, EffectMeta};
use crate::journal::{JournalEffect, JournalMsg, LostMsg};
use crate::msg::{JobMsg, Msg, RepoMsg, TabsMsg};
use crate::state::journal_lost::JournalTab;
use crate::state::{AppState, Modal};
use git2::Oid;
use glowfly_core::journal::{LostCommit, LostCommits, LostKind, Op, Preview};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn run(st: &mut AppState, m: JournalMsg) -> Vec<Effect> {
    update(st, Msg::Journal(m), Instant::now())
}

fn lost(st: &mut AppState, m: LostMsg) -> Vec<Effect> {
    run(st, JournalMsg::Lost(m))
}

fn lost_loads(fx: &[Effect]) -> Vec<u64> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Journal(JournalEffect::Lost { generation, .. }) => Some(*generation),
            _ => None,
        })
        .collect()
}

fn timeline_loads(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::Journal(JournalEffect::Load { .. }))).count()
}

fn rev(dir: &Path, r: &str) -> Oid {
    let out = glowfly_core::gitcli::read(dir, &glowfly_core::gitcli::args(&["rev-parse", r]));
    Oid::from_str(out.stdout.trim()).unwrap()
}

/// The journal open on the fixture with `second` reset away and a dropped stash on `first`;
/// returns the lost commit and the stash.
fn fixture(name: &str) -> (AppState, PathBuf, Oid, Oid) {
    let (mut st, dir) = opened(name);
    let second = rev(&dir, "HEAD");
    git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
    std::fs::write(dir.join("a.txt"), "one\nstashed\n").unwrap();
    git(&dir, &["stash", "push", "-q", "-m", "keep"]);
    let stash = rev(&dir, "refs/stash");
    git(&dir, &["stash", "drop", "-q"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    open_loaded(&mut st, &dir);
    (st, dir, second, stash)
}

/// Shows the Lost tab and lands what `git fsck` finds.
fn show_found(st: &mut AppState, dir: &Path) {
    let fx = run(st, JournalMsg::Tab(JournalTab::Lost));
    let [g] = lost_loads(&fx)[..] else { panic!("one lost load") };
    let result = glowfly_core::journal::lost::find(dir).map(Box::new).map_err(|e| e.to_string());
    lost(st, LostMsg::Loaded { generation: g, result });
}

fn hand_built(commits: Vec<LostCommit>) -> Result<Box<LostCommits>, String> {
    Ok(Box::new(LostCommits { commits, ..Default::default() }))
}

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn entry(n: u8, kind: LostKind) -> LostCommit {
    LostCommit { oid: oid(n), kind, summary: "s".into(), message: "s".into(), author: "T".into(), time: 1 }
}

fn running() -> crate::state::RunningJob {
    crate::state::RunningJob {
        label: "x".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    }
}

#[test]
fn the_lost_tab_loads_once_scoped_to_the_repository() {
    let (mut st, dir) = opened("journal-lost-once");
    open_loaded(&mut st, &dir);
    assert_eq!(st.session.journal.view.as_ref().unwrap().tab, JournalTab::States, "States first");
    let fx = run(&mut st, JournalMsg::Tab(JournalTab::Lost));
    let [Effect::Journal(e @ JournalEffect::Lost { generation, .. })] = &fx[..] else { panic!("one lost load: {fx:?}") };
    assert!(super::tests::same_place(e.workdir(), &dir), "scoped to the repository");
    assert_eq!(e.name(), "journal-lost");
    lost(&mut st, LostMsg::Loaded { generation: *generation, result: hand_built(Vec::new()) });
    assert!(run(&mut st, JournalMsg::Tab(JournalTab::States)).is_empty());
    assert!(lost_loads(&run(&mut st, JournalMsg::Tab(JournalTab::Lost))).is_empty(), "a fresh list is not read again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_or_closed_lost_results_are_dropped() {
    let (mut st, dir) = opened("journal-lost-stale");
    open_loaded(&mut st, &dir);
    let first = lost_loads(&run(&mut st, JournalMsg::Tab(JournalTab::Lost)))[0];
    lost(&mut st, LostMsg::Loaded { generation: first, result: hand_built(Vec::new()) });
    let second = lost_loads(&run(&mut st, JournalMsg::Refresh))[0];
    assert!(second > first);
    lost(&mut st, LostMsg::Loaded { generation: first, result: Err("old".into()) });
    let v = st.session.journal.view.as_ref().unwrap().lost.as_ref().unwrap();
    assert!(v.loading && matches!(v.result, Some(Ok(_))), "an older result is dropped");
    lost(&mut st, LostMsg::Loaded { generation: second, result: hand_built(vec![entry(1, LostKind::Commit)]) });
    let v = st.session.journal.view.as_ref().unwrap().lost.as_ref().unwrap();
    assert!(!v.loading && v.commits().len() == 1);
    run(&mut st, JournalMsg::Close);
    lost(&mut st, LostMsg::Loaded { generation: second, result: hand_built(Vec::new()) });
    assert!(st.session.journal.view.is_none(), "a closed view stays closed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refresh_reloads_lost_only_while_it_shows() {
    let (mut st, dir) = opened("journal-lost-refresh");
    open_loaded(&mut st, &dir);
    let g = lost_loads(&run(&mut st, JournalMsg::Tab(JournalTab::Lost)))[0];
    lost(&mut st, LostMsg::Loaded { generation: g, result: hand_built(Vec::new()) });
    let fx = run(&mut st, JournalMsg::Refresh);
    assert_eq!((timeline_loads(&fx), lost_loads(&fx).len()), (1, 1), "both while the Lost tab shows");
    let g = lost_loads(&fx)[0];
    lost(&mut st, LostMsg::Loaded { generation: g, result: hand_built(Vec::new()) });
    run(&mut st, JournalMsg::Tab(JournalTab::States));
    let fx = run(&mut st, JournalMsg::Refresh);
    assert_eq!((timeline_loads(&fx), lost_loads(&fx).len()), (1, 0), "only the timeline on the States tab");
    assert!(st.session.journal.view.as_ref().unwrap().lost.as_ref().unwrap().stale);
    assert_eq!(lost_loads(&run(&mut st, JournalMsg::Tab(JournalTab::Lost))).len(), 1, "a stale list is read again");
    let _ = std::fs::remove_dir_all(&dir);
}

fn preview_result(st: &AppState) -> bool {
    st.session.journal.view.as_ref().unwrap().lost.as_ref().unwrap().preview.as_ref().unwrap().result.is_some()
}

#[test]
fn selecting_previews_from_the_stash_base() {
    let (mut st, dir) = opened("journal-lost-select");
    open_loaded(&mut st, &dir);
    let g = lost_loads(&run(&mut st, JournalMsg::Tab(JournalTab::Lost)))[0];
    let stash = entry(3, LostKind::Stash { base: oid(2), branch: None, untracked: false });
    let list = vec![stash, entry(1, LostKind::Commit)];
    lost(&mut st, LostMsg::Loaded { generation: g, result: hand_built(list.clone()) });
    assert!(lost(&mut st, LostMsg::Select(oid(7))).is_empty(), "not listed");
    let fx = lost(&mut st, LostMsg::Select(oid(3)));
    let [Effect::Journal(JournalEffect::LostPreview { oid: o, from, generation, .. })] = &fx[..] else { panic!("one preview") };
    assert_eq!((*o, *from), (oid(3), oid(2)), "a stash's history starts at its base");
    let g = *generation;
    let p = Preview { history: Vec::new(), leaving: None, arriving: None };
    lost(&mut st, LostMsg::PreviewLoaded { generation: g - 1, oid: oid(3), result: Ok(p.clone()) });
    assert!(!preview_result(&st), "a stale preview is dropped");
    lost(&mut st, LostMsg::PreviewLoaded { generation: g, oid: oid(1), result: Ok(p.clone()) });
    assert!(!preview_result(&st), "so is another commit's");
    lost(&mut st, LostMsg::PreviewLoaded { generation: g, oid: oid(3), result: Ok(p) });
    assert!(preview_result(&st));

    let g = lost_loads(&run(&mut st, JournalMsg::Refresh))[0];
    let fx = lost(&mut st, LostMsg::Loaded { generation: g, result: hand_built(list) });
    assert!(matches!(&fx[..], [Effect::Journal(JournalEffect::LostPreview { oid: o, .. })] if *o == oid(3)), "kept and read again");
    assert_eq!(st.session.journal.view.as_ref().unwrap().lost.as_ref().unwrap().selected, Some(oid(3)));
    let g = lost_loads(&run(&mut st, JournalMsg::Refresh))[0];
    lost(&mut st, LostMsg::Loaded { generation: g, result: hand_built(vec![entry(1, LostKind::Commit)]) });
    let v = st.session.journal.view.as_ref().unwrap().lost.as_ref().unwrap();
    assert!(v.selected.is_none() && v.preview.is_none(), "cleared once it is no longer lost");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_confirms_a_journaled_detached_checkout() {
    let (mut st, dir, second, _) = fixture("journal-lost-restore");
    show_found(&mut st, &dir);
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    lost(&mut st, LostMsg::Restore(second));
    let Some(Modal::Confirm { title, then, .. }) = &st.modal else { panic!("a confirmation") };
    assert_eq!(title, "Check out this state?");
    let Msg::Job(JobMsg::Journaled { cmd, op, expect, .. }) = &**then else { panic!("a journaled job") };
    assert_eq!(cmd, &glowfly_core::journal::restore::checkout_detached(second));
    assert!(matches!(op, Op::Restore { branch: None, .. }), "{op:?}");
    assert_eq!(expect, &Some(("HEAD".into(), head)));
    st.modal = None;
    st.job = Some(running());
    lost(&mut st, LostMsg::Restore(second));
    assert!(st.modal.is_none(), "refused while a job runs");
    assert!(st.log.last().is_some_and(|l| l.text == "Another operation is still running."));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_branch_opens_the_dialog_at_the_commit() {
    let (mut st, dir, second, _) = fixture("journal-lost-branch");
    show_found(&mut st, &dir);
    lost(&mut st, LostMsg::CreateBranch(second));
    let Some(Modal::NewBranch { start, name, .. }) = &st.modal else { panic!("the New branch dialog") };
    assert_eq!((start.clone(), name.as_str()), (Some(second.to_string()), ""));
    let _ = std::fs::remove_dir_all(&dir);
}

fn git_cmds(fx: &[Effect]) -> Vec<Vec<String>> {
    match fx.iter().find(|e| matches!(e, Effect::RunGit { .. })) {
        Some(Effect::RunGit { cmds, .. }) => cmds.clone(),
        _ => panic!("a git job: {fx:?}"),
    }
}

#[test]
fn apply_and_store_run_on_the_stash() {
    let (mut st, dir, second, stash) = fixture("journal-lost-apply");
    show_found(&mut st, &dir);
    let listed = st.session.journal.view.as_ref().unwrap().lost.as_ref().unwrap().commit(stash).cloned();
    assert_eq!(listed.expect("the stash is lost").summary, "On main: keep");
    // A stash has no branch or Restore, and a commit has no stash actions.
    for m in [LostMsg::Restore(stash), LostMsg::CreateBranch(stash), LostMsg::Apply(second), LostMsg::Store(second)] {
        assert!(lost(&mut st, m).is_empty() && st.modal.is_none());
    }
    let id = stash.to_string();
    let fx = lost(&mut st, LostMsg::Store(stash));
    assert_eq!(git_cmds(&fx), vec![glowfly_core::gitcli::args(&["stash", "store", "-m", "On main: keep", &id])]);
    st.job = None;
    let fx = lost(&mut st, LostMsg::Apply(stash));
    assert_eq!(git_cmds(&fx), vec![glowfly_core::gitcli::args(&["stash", "apply", &id])], "starts at once on a clean tree");
    finish(&mut st, fx);
    let text = std::fs::read_to_string(dir.join("a.txt")).unwrap().replace("\r\n", "\n");
    assert_eq!(text, "one\nstashed\n", "the stash's change is back");
    let fx = lost(&mut st, LostMsg::Apply(stash));
    finish(&mut st, fx);
    assert!(matches!(&st.modal, Some(Modal::Confirm { body, .. }) if body.contains("a.txt")), "asks first on a dirty tree");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apply_stash_is_refused_while_a_job_runs_or_an_operation_is_in_progress() {
    let (mut st, dir, _, stash) = fixture("journal-lost-apply-refused");
    show_found(&mut st, &dir);
    st.job = Some(running());
    assert!(lost(&mut st, LostMsg::Apply(stash)).is_empty() && st.modal.is_none(), "refused while a job runs");
    assert!(st.log.last().is_some_and(|l| l.text == "Another operation is still running."));
    st.job = None;
    st.snap.as_mut().unwrap().state = git2::RepositoryState::Merge;
    assert!(lost(&mut st, LostMsg::Apply(stash)).is_empty() && st.modal.is_none(), "refused mid-merge");
    assert!(st.log.last().is_some_and(|l| l.text == "Finish or abort the operation in progress first."));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn parking_the_tab_asks_again() {
    let (mut st, dir) = opened("journal-lost-park");
    open_loaded(&mut st, &dir);
    let before = lost_loads(&run(&mut st, JournalMsg::Tab(JournalTab::Lost)))[0];
    drive(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert!(st.session.journal.view.is_none());
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    let v = st.session.journal.view.as_ref().expect("back with its tab");
    assert_eq!(v.tab, JournalTab::Lost, "the tab is parked with the view");
    let again = lost_loads(&fx);
    assert_eq!(again.len(), 1, "the lost load in flight is asked for again");
    assert!(again[0] > before && v.lost.as_ref().unwrap().generation == again[0]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refresh_during_a_load_finds_them_again() {
    let (mut st, dir) = opened("journal-lost-refresh-mid-load");
    open_loaded(&mut st, &dir);
    let g = lost_loads(&run(&mut st, JournalMsg::Tab(JournalTab::Lost)))[0];
    assert!(lost_loads(&run(&mut st, JournalMsg::Refresh)).is_empty(), "one fsck at a time");
    let fx = lost(&mut st, LostMsg::Loaded { generation: g, result: hand_built(Vec::new()) });
    let [again] = lost_loads(&fx)[..] else { panic!("the out-of-date result is found again: {fx:?}") };
    assert!(again > g);
    let fx = lost(&mut st, LostMsg::Loaded { generation: again, result: hand_built(Vec::new()) });
    assert!(lost_loads(&fx).is_empty(), "a result with no refresh during it is current");
    let v = st.session.journal.view.as_ref().unwrap().lost.as_ref().unwrap();
    assert!(!v.loading && !v.stale);

    // Hidden by the time it lands: kept stale until the tab shows.
    let g = lost_loads(&run(&mut st, JournalMsg::Refresh))[0];
    run(&mut st, JournalMsg::Refresh);
    run(&mut st, JournalMsg::Tab(JournalTab::States));
    assert!(lost_loads(&lost(&mut st, LostMsg::Loaded { generation: g, result: hand_built(Vec::new()) })).is_empty());
    assert!(st.session.journal.view.as_ref().unwrap().lost.as_ref().unwrap().stale);
    assert_eq!(lost_loads(&run(&mut st, JournalMsg::Tab(JournalTab::Lost))).len(), 1, "read again when it shows");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn parking_on_the_states_tab_waits_for_the_lost_tab() {
    let (mut st, dir) = opened("journal-lost-park-states");
    open_loaded(&mut st, &dir);
    lost_loads(&run(&mut st, JournalMsg::Tab(JournalTab::Lost)));
    run(&mut st, JournalMsg::Tab(JournalTab::States));
    drive(&mut st, Msg::Tabs(TabsMsg::NewTab));
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert!(lost_loads(&fx).is_empty(), "no fsck while the States tab shows");
    let v = st.session.journal.view.as_ref().unwrap().lost.as_ref().unwrap();
    assert!(!v.loading && v.stale);
    assert_eq!(lost_loads(&run(&mut st, JournalMsg::Tab(JournalTab::Lost))).len(), 1, "found when it shows");
    let _ = std::fs::remove_dir_all(&dir);
}
