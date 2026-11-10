//! Headless tests for working inside a submodule from its row, on injected rows: Record this
//! checkout stages the gitlink (or says why not), Fetch, Pull and Discard changes inside start
//! jobs in the submodule's folder (Pull after reading its branch, Discard after asking), the
//! parent refreshes when they end, failures caused by changes inside offer to open the
//! submodule, and the diff's gitlink card follows the diff on screen.

use super::tests::state;
use super::update;
use crate::effect::{Effect, FilesEffect, JobEffect, LoadEffect, LocalOp, SubmoduleEffect};
use crate::msg::{JobMsg, Msg, RepoMsg};
use crate::state::submodules::inside::InsideKind;
use crate::state::{AppState, Modal, gitlink_card};
use crate::submodule::SubmoduleMsg;
use git2::Oid;
use glowfly_core::repo::{
    CommitRun, DiffLine, DiffTarget, FileDiff, FileEntry, GitlinkMove, Hunk, InsideHead, MoveKind, SubmoduleInfo, SubmoduleState,
};
use glowfly_core::settings::SavedTrust;
use glowfly_core::undo::{DISCARD_STASH_MESSAGE, UndoKind};
use std::path::PathBuf;
use std::time::Instant;

const PATH: &str = "libs/lib";

fn oid(c: char) -> Oid {
    Oid::from_str(&c.to_string().repeat(40)).unwrap()
}

fn info(state: SubmoduleState) -> SubmoduleInfo {
    let checked_out = match state {
        SubmoduleState::Uninitialized | SubmoduleState::Missing => None,
        SubmoduleState::OutOfSync | SubmoduleState::DirtyOutOfSync => Some(oid('b')),
        _ => Some(oid('a')),
    };
    SubmoduleInfo {
        name: PATH.into(),
        path: PATH.into(),
        url: Some("../lib".into()),
        clone_url: None,
        branch: None,
        added: false,
        recorded: oid('a'),
        checked_out,
        state,
        left_behind: Vec::new(),
        outside_sparse: false,
    }
}

/// A repository open with one submodule row in `state`, the submodule's folder trusted.
/// Returns (state, the submodule's folder).
fn with_sub(name: &str, state_: SubmoduleState) -> (AppState, PathBuf) {
    let mut st = state();
    let mut snap = crate::state::op_fixture::snapshot(&format!("inside-{name}"));
    snap.submodules = vec![info(state_)];
    let folder = snap.workdir.join("libs").join("lib");
    st.snap = Some(snap);
    st.settings.set_trust(&folder, Some(SavedTrust::trusted(None)));
    (st, folder)
}

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn sub(m: SubmoduleMsg) -> Msg {
    Msg::Submodule(m)
}

/// Whether `msg` opens the submodule.
fn opens(msg: &Msg) -> bool {
    matches!(msg, Msg::Submodule(SubmoduleMsg::Open(p)) if p == PATH)
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

/// The git job among `fx`: its folder, commands and undo kind.
fn job(fx: &[Effect]) -> Option<(PathBuf, Vec<Vec<String>>, Option<UndoKind>)> {
    fx.iter().find_map(|e| match e {
        Effect::Job(JobEffect::RunGit { cwd, cmds, record, .. }) => Some((cwd.clone(), cmds.clone(), record.clone())),
        _ => None,
    })
}

fn strings(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

#[test]
fn record_stages_an_out_of_sync_gitlink() {
    let (mut st, _) = with_sub("record", SubmoduleState::OutOfSync);
    let fx = run(&mut st, sub(SubmoduleMsg::Record(PATH.into())));
    assert!(fx.iter().any(|e| matches!(e, Effect::Files(FilesEffect::LocalOp { op: LocalOp::StageFile(p), .. }) if p == PATH)));
}

#[test]
fn record_refuses_what_it_cannot_record() {
    for (state_, why) in [
        (SubmoduleState::InSync, "already what the index records"),
        (SubmoduleState::Uninitialized, "Not checked out"),
        (SubmoduleState::Dirty, "already what the index records"),
    ] {
        let (mut st, _) = with_sub("record-refused", state_);
        assert!(run(&mut st, sub(SubmoduleMsg::Record(PATH.into()))).is_empty(), "{state_:?}");
        assert!(last_log(&st).contains(why), "{state_:?}: {}", last_log(&st));
    }
    let (mut st, _) = with_sub("record-conflict", SubmoduleState::OutOfSync);
    st.snap.as_mut().unwrap().unstaged = vec![FileEntry::new(PATH, '!')];
    assert!(run(&mut st, sub(SubmoduleMsg::Record(PATH.into()))).is_empty());
    assert!(last_log(&st).contains("merge conflict"), "{}", last_log(&st));
}

#[test]
fn fetch_runs_in_the_submodule_folder() {
    let (mut st, folder) = with_sub("fetch", SubmoduleState::InSync);
    let fx = run(&mut st, sub(SubmoduleMsg::Fetch(PATH.into())));
    let (cwd, cmds, record) = job(&fx).expect("a job");
    assert_eq!(cwd, folder);
    assert_eq!(cmds, vec![strings(&["fetch", "--all", "--prune"])]);
    assert_eq!(record, None);
    let inside = st.submodules.inside.as_ref().expect("remembered");
    assert_eq!((inside.kind, inside.path.as_str(), inside.label.as_str()), (InsideKind::Fetch, PATH, "Fetch in submodule libs/lib"));

    let (mut st, _) = with_sub("fetch-uninit", SubmoduleState::Uninitialized);
    assert!(job(&run(&mut st, sub(SubmoduleMsg::Fetch(PATH.into())))).is_none());
    assert!(last_log(&st).contains("Not checked out yet"));
}

#[test]
fn pull_reads_the_branch_first() {
    let (mut st, folder) = with_sub("pull", SubmoduleState::InSync);
    let parent = st.workdir().unwrap();
    let fx = run(&mut st, sub(SubmoduleMsg::Pull(PATH.into())));
    assert!(matches!(&fx[..], [Effect::Submodule(SubmoduleEffect::ReadHead { workdir, path })] if *workdir == parent && path == PATH));
    assert!(SubmoduleMsg::HeadRead { path: PATH.into(), result: Ok(None) }.on_inactive().is_none());

    // Detached: no job, a toast that opens the submodule.
    let head =
        |branch: Option<&str>, upstream: Option<&str>| InsideHead { branch: branch.map(Into::into), upstream: upstream.map(Into::into) };
    let fx = run(&mut st, sub(SubmoduleMsg::HeadRead { path: PATH.into(), result: Ok(Some(head(None, None))) }));
    assert!(job(&fx).is_none());
    let toast = st.toasts.last().expect("a toast");
    assert!(!toast.ok && toast.text.starts_with("Pull needs a branch: libs/lib is on no branch (detached HEAD)."), "{}", toast.text);
    let action = toast.action.as_ref().expect("Open submodule");
    assert_eq!(action.label, "Open submodule");
    assert!(opens(&action.msg));
    assert_eq!(action.workdir.as_ref(), Some(&parent));

    let fx = run(&mut st, sub(SubmoduleMsg::HeadRead { path: PATH.into(), result: Ok(Some(head(Some("work"), None))) }));
    assert!(job(&fx).is_none());
    assert!(st.toasts.last().unwrap().text.contains("is on work, which has no upstream branch"));

    let fx = run(&mut st, sub(SubmoduleMsg::HeadRead { path: PATH.into(), result: Ok(Some(head(Some("main"), Some("origin/main")))) }));
    let (cwd, cmds, _) = job(&fx).expect("the pull");
    assert_eq!((cwd, cmds), (folder, vec![strings(&["pull", "--ff"])]));
}

#[test]
fn discard_inside_asks_then_stashes_in_the_submodule() {
    let (mut st, folder) = with_sub("discard", SubmoduleState::Dirty);
    let fx = run(&mut st, sub(SubmoduleMsg::DiscardInside { path: PATH.into(), confirmed: false }));
    assert!(fx.is_empty());
    let Some(Modal::ConfirmDanger { title, body, button, then, .. }) = st.modal.take() else { panic!("a danger confirmation") };
    assert_eq!((title.as_str(), button.as_str()), ("Discard changes inside libs/lib", "Discard changes inside"));
    assert!(body.contains(&format!("a stash named \"{DISCARD_STASH_MESSAGE}\" in the submodule")), "{body}");
    assert!(matches!(&*then, Msg::Submodule(SubmoduleMsg::DiscardInside { path, confirmed: true }) if path == PATH));

    let fx = run(&mut st, *then);
    let (cwd, cmds, record) = job(&fx).expect("the stash");
    assert_eq!(cwd, folder);
    assert_eq!(cmds, vec![strings(&["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE])]);
    assert_eq!(record, Some(UndoKind::DiscardAll));

    let (mut st, _) = with_sub("discard-clean", SubmoduleState::InSync);
    assert!(run(&mut st, sub(SubmoduleMsg::DiscardInside { path: PATH.into(), confirmed: false })).is_empty());
    assert!(st.modal.is_none());
    assert!(last_log(&st).contains("Nothing to discard"));
}

fn done(st: &mut AppState, ok: bool, output: &str) -> Vec<Effect> {
    run(st, Msg::Job(JobMsg::JobDone { ok, output: output.into(), undo: None }))
}

fn refreshes(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Load(LoadEffect::Repo { .. })))
}

#[test]
fn a_finished_discard_refreshes_the_parent_and_offers_to_open_the_submodule() {
    let (mut st, _) = with_sub("discard-done", SubmoduleState::Dirty);
    run(&mut st, sub(SubmoduleMsg::DiscardInside { path: PATH.into(), confirmed: true }));
    let fx = done(&mut st, true, "");
    assert!(refreshes(&fx), "the parent is still the active tab");
    let toast = st.toasts.last().expect("a toast");
    assert_eq!(toast.text, format!("Discarded the changes inside libs/lib; they are kept in its stash \"{DISCARD_STASH_MESSAGE}\"."));
    assert!(toast.action.as_ref().is_some_and(|a| opens(&a.msg)));
    assert!(st.submodules.inside.is_none());

    // The parent's tab is no longer active: nothing refreshes.
    let (mut st, _) = with_sub("discard-away", SubmoduleState::Dirty);
    run(&mut st, sub(SubmoduleMsg::DiscardInside { path: PATH.into(), confirmed: true }));
    st.snap = Some(crate::state::op_fixture::snapshot("inside-discard-away-other"));
    let fx = done(&mut st, true, "");
    assert!(!refreshes(&fx));
    assert!(st.submodules.inside.is_none());
}

#[test]
fn a_failure_over_changes_inside_offers_to_open_the_submodule() {
    let (mut st, _) = with_sub("failure", SubmoduleState::Dirty);
    run(&mut st, Msg::git("Checkout", &["checkout", "moved"]));
    assert!(st.job.is_some());
    done(&mut st, false, "error: Submodule 'libs/lib' could not be updated.\nerror: Cannot update submodule:\n\tlibs/lib\n");
    let toast = st.toasts.last().expect("a toast");
    assert!(toast.text.starts_with("Stopped because of changes inside the submodule libs/lib."), "{}", toast.text);
    assert_eq!(toast.action.as_ref().map(|a| a.label.as_str()), Some("Open submodule"));
    // The toast's button runs the fix kept with the run.
    assert!(st.logs.fixes.last().is_some_and(|(_, f)| opens(&f.msg)));

    let (mut st, _) = with_sub("failure-other", SubmoduleState::Dirty);
    run(&mut st, Msg::git("Checkout", &["checkout", "moved"]));
    done(&mut st, false, "error: Submodule 'vendor/x' could not be updated.\n");
    let toast = st.toasts.last().expect("a toast");
    assert!(toast.text.starts_with("Stopped because of changes inside the submodule vendor/x."), "{}", toast.text);
    let label = toast.action.as_ref().map(|a| a.label.as_str());
    assert!(label != Some("Open submodule"), "no submodule there to open: {label:?}");
}

fn gitlink_diff() -> FileDiff {
    let line = |origin: char, text: &str| DiffLine { origin, old: Some(1), new: Some(1), text: text.into() };
    let lines = vec![line('-', &format!("Subproject commit {}", oid('a'))), line('+', &format!("Subproject commit {}", oid('b')))];
    FileDiff {
        binary: false,
        hunks: vec![Hunk { header: "@@ -1 +1 @@".into(), lines }],
        highlight: None,
        intraline: Default::default(),
        whole: None,
    }
}

fn moved() -> GitlinkMove {
    GitlinkMove {
        path: PATH.into(),
        old: Some(oid('a')),
        new: Some(oid('b')),
        kind: MoveKind::Forward,
        ahead: CommitRun::default(),
        behind: CommitRun::default(),
        dirty: false,
        type_change: false,
    }
}

fn open_with(st: &mut AppState, target: &DiffTarget, gitlink: Option<GitlinkMove>) {
    run(st, Msg::Repo(RepoMsg::OpenDiff(target.clone())));
    let generation = st.session.diff.generation;
    let result = Ok(gitlink_diff());
    let gitlink = gitlink.map(Box::new);
    run(st, Msg::Repo(RepoMsg::DiffLoaded { target: target.clone(), reload: false, generation, result, lfs: None, raw: None, gitlink }));
}

#[test]
fn the_card_follows_the_diff_on_screen() {
    let (mut st, _) = with_sub("card", SubmoduleState::OutOfSync);
    let unstaged = DiffTarget::Unstaged(PATH.into());
    open_with(&mut st, &unstaged, Some(moved()));
    assert_eq!(gitlink_card::shown(&st), Some(&moved()));
    assert_eq!(gitlink_card::record_offered(&st).as_deref(), Some(PATH));
    assert_eq!(gitlink_card::open_offered(&st).as_deref(), Some(PATH));

    // Staged: nothing to record from there.
    let staged = DiffTarget::Staged(PATH.into());
    open_with(&mut st, &staged, Some(moved()));
    assert!(gitlink_card::shown(&st).is_some());
    assert_eq!(gitlink_card::record_offered(&st), None);

    open_with(&mut st, &DiffTarget::Unstaged("a.txt".into()), None);
    assert_eq!(gitlink_card::shown(&st), None, "another file's diff");

    // In sync: the checkout is what the index records already.
    let (mut st, _) = with_sub("card-in-sync", SubmoduleState::InSync);
    open_with(&mut st, &unstaged, Some(moved()));
    assert_eq!(gitlink_card::record_offered(&st), None);
    assert_eq!(gitlink_card::open_offered(&st).as_deref(), Some(PATH));
}
