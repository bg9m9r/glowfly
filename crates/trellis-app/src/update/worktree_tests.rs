//! Headless tests for the WORKTREES section and the Create worktree dialog: opening a worktree
//! in a tab and refusing what can't be opened; creating one from a branch or a commit (folder
//! check, then `git worktree add` as a job, then the new tab); the refusals the form and the disk
//! check give; Browse…; and the section's text rules.

use super::tests::same_place;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::PrefsMsg;
use crate::msg::RepoMsg;
use crate::msg::TabsMsg;
use crate::msg::{FolderPurpose, Msg};
use crate::runner;
use crate::state::worktrees::{WorktreeForm, checked_out_in, folder_warning, right_text, section_shown};
use crate::state::{AppState, Modal, RunningJob};
use crate::worktree::{NewWorktree, WorktreeMsg};
use git2::Oid;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::worktree_ops::WorktreeBase;

/// A repository with a second branch `side`, opened in `st`.
fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["branch", "side"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

/// `<parent>/<repo>-worktrees`, where the default folders go; emptied first.
fn wt_root(dir: &Path) -> PathBuf {
    let root = dir.with_file_name(format!("{}-worktrees", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&root);
    root
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(wt_root(dir));
    let _ = std::fs::remove_dir_all(dir);
}

fn wt(m: WorktreeMsg) -> Msg {
    Msg::Worktree(m)
}

fn form(st: &mut AppState) -> &mut WorktreeForm {
    match st.modal.as_mut() {
        Some(Modal::NewWorktree(f)) => f,
        _ => panic!("the Create worktree dialog is not open"),
    }
}

fn last_log(st: &AppState) -> (bool, String) {
    st.log.last().map(|l| (l.ok, l.text.clone())).expect("a log entry")
}

fn head(st: &AppState) -> Oid {
    st.snap.as_ref().unwrap().head_oid.unwrap()
}

/// Runs `fx` the way `drive` does, returning the git command lines it ran.
fn run(st: &mut AppState, fx: Vec<Effect>) -> Vec<Vec<String>> {
    let mut ran = Vec::new();
    for e in fx {
        match &e {
            Effect::RunGit { cmds, .. } => ran.extend(cmds.clone()),
            Effect::LoadRepo { .. } | Effect::LoadStatus { .. } | Effect::CheckWorktreeFolder(_) => {}
            _ => continue,
        }
        if let Some(m) = runner::execute(e) {
            let more = update(st, m, Instant::now());
            ran.extend(run(st, more));
        }
    }
    ran
}

/// Submit, check the folder, start the job: returns the effects after the folder check.
fn submit(st: &mut AppState) -> Vec<Effect> {
    let fx = update(st, wt(WorktreeMsg::Submit), Instant::now());
    let [Effect::CheckWorktreeFolder(_)] = &fx[..] else { panic!("{fx:?}") };
    let checked = runner::execute(fx.into_iter().next().unwrap()).unwrap();
    update(st, checked, Instant::now())
}

fn args_of(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

#[test]
fn open_linked_current_and_missing() {
    let (mut st, dir) = opened("wt-open");
    let root = wt_root(&dir);
    let (linked, gone) = (root.join("linked"), root.join("gone"));
    git(&dir, &["worktree", "add", "-q", "-b", "linked", &linked.display().to_string()]);
    git(&dir, &["worktree", "add", "-q", "-b", "gone", &gone.display().to_string()]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(st.snap.as_ref().unwrap().worktrees.len(), 3);

    drive(&mut st, wt(WorktreeMsg::Open(linked.clone())));
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1));
    assert!(same_place(&st.snap.as_ref().unwrap().workdir, &linked));

    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert!(update(&mut st, wt(WorktreeMsg::Open(dir.clone())), Instant::now()).is_empty());
    let (ok, text) = last_log(&st);
    assert!(!ok && text.ends_with("This worktree is open in this tab."), "{text}");
    assert_eq!(st.tabs.len(), 2);

    std::fs::remove_dir_all(&gone).unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(update(&mut st, wt(WorktreeMsg::Open(gone.clone())), Instant::now()).is_empty());
    let (ok, text) = last_log(&st);
    assert!(!ok && text.ends_with("Its folder is missing."), "{text}");

    assert!(update(&mut st, wt(WorktreeMsg::Open(root.join("nope"))), Instant::now()).is_empty());
    assert!(last_log(&st).1.ends_with("is no longer a worktree of this repository."));
    assert_eq!(st.tabs.len(), 2);
    cleanup(&dir);
}

#[test]
fn dialog_defaults() {
    let (mut st, dir) = opened("wt-dialog");
    let root = wt_root(&dir);
    drive(&mut st, wt(WorktreeMsg::CreateDialog(WorktreeBase::Branch("side".into()))));
    let f = form(&mut st).clone();
    assert!(f.open && f.branch.is_empty() && !f.folder_edited && !f.busy());
    assert_eq!(PathBuf::from(&f.folder), root.join("side"));
    assert!(Path::new(&f.folder).starts_with(dir.parent().unwrap()));
    cleanup(&dir);
}

#[test]
fn create_from_branch_and_open() {
    let (mut st, dir) = opened("wt-create-branch");
    let folder = wt_root(&dir).join("side");
    drive(&mut st, wt(WorktreeMsg::CreateDialog(WorktreeBase::Branch("side".into()))));
    let fx = submit(&mut st);
    assert!(st.modal.is_none(), "closed when the job starts");
    let ran = run(&mut st, fx);
    assert_eq!(ran, [args_of(&["worktree", "add", &folder.display().to_string(), "side"])]);
    assert!(folder.is_dir());
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1));
    let snap = st.snap.as_ref().unwrap();
    assert!(same_place(&snap.workdir, &folder));
    assert_eq!(snap.head_branch.as_deref(), Some("side"));

    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    let snap = st.snap.as_ref().unwrap();
    assert!(same_place(&snap.workdir, &dir));
    assert_eq!(snap.worktrees.len(), 2, "{:?}", snap.worktrees);
    assert!(section_shown(snap));
    cleanup(&dir);
}

#[test]
fn checked_out_branch_needs_a_new_one() {
    let (mut st, dir) = opened("wt-create-main");
    let folder = wt_root(&dir).join("topic");
    drive(&mut st, wt(WorktreeMsg::CreateDialog(WorktreeBase::Branch("main".into()))));
    assert!(update(&mut st, wt(WorktreeMsg::Submit), Instant::now()).is_empty());
    assert_eq!(last_log(&st), (false, "main is checked out in this worktree. Enter a name for a new branch.".to_owned()));
    assert!(st.modal.is_some());

    let f = form(&mut st);
    f.branch = "topic".into();
    f.folder = folder.display().to_string();
    f.open = false;
    let fx = submit(&mut st);
    let ran = run(&mut st, fx);
    assert_eq!(ran, [args_of(&["worktree", "add", "-b", "topic", &folder.display().to_string(), "main"])]);
    assert_eq!(st.tabs.len(), 1);
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.locals.iter().any(|b| b.name == "topic"));
    assert_eq!(checked_out_in(snap, "topic").map(|w| w.folder_name()).as_deref(), Some("topic"));
    cleanup(&dir);
}

#[test]
fn create_detached_from_commit() {
    let (mut st, dir) = opened("wt-create-commit");
    let sha = head(&st);
    let short = sha.to_string()[..7].to_owned();
    let folder = wt_root(&dir).join(&short);
    drive(&mut st, wt(WorktreeMsg::CreateDialog(WorktreeBase::Commit(sha))));
    assert_eq!(PathBuf::from(&form(&mut st).folder), folder);
    form(&mut st).open = false;
    let fx = submit(&mut st);
    let ran = run(&mut st, fx);
    assert_eq!(ran, [args_of(&["worktree", "add", "--detach", &folder.display().to_string(), &sha.to_string()])]);
    assert_eq!(st.tabs.len(), 1);
    let snap = st.snap.as_ref().unwrap();
    let linked = snap.worktrees.iter().find(|w| !w.is_main()).expect("a linked worktree");
    assert_eq!((linked.branch.as_deref(), linked.head), (None, Some(sha)));
    assert_eq!(right_text(linked), format!("detached {short}"));
    cleanup(&dir);
}

#[test]
fn folder_not_empty_stays_open() {
    let (mut st, dir) = opened("wt-not-empty");
    let folder = wt_root(&dir).join("side");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("x.txt"), "x").unwrap();
    drive(&mut st, wt(WorktreeMsg::CreateDialog(WorktreeBase::Branch("side".into()))));
    let fx = submit(&mut st);
    assert!(fx.is_empty(), "{fx:?}");
    assert!(st.job.is_none());
    let f = form(&mut st).clone();
    let text = folder.display().to_string();
    let message = trellis_core::worktree_ops::FolderProblem::NotEmpty.message(&text);
    assert_eq!(f.error, Some((text, message.clone())));
    assert_eq!(f.current_error(), Some(message.as_str()));
    assert!(!f.busy());
    form(&mut st).folder.push('2');
    assert_eq!(form(&mut st).current_error(), None, "hidden once the field changes");
    cleanup(&dir);
}

#[test]
fn bad_names_and_registered_folder() {
    let (mut st, dir) = opened("wt-names");
    drive(&mut st, wt(WorktreeMsg::CreateDialog(WorktreeBase::Branch("side".into()))));
    for (name, want) in [("bad name", "Not a valid branch name."), ("main", "A branch named main already exists.")] {
        form(&mut st).branch = name.into();
        assert!(update(&mut st, wt(WorktreeMsg::Submit), Instant::now()).is_empty());
        assert_eq!(last_log(&st), (false, want.to_owned()));
    }
    form(&mut st).branch.clear();
    form(&mut st).folder = dir.display().to_string();
    assert!(update(&mut st, wt(WorktreeMsg::Submit), Instant::now()).is_empty());
    let (ok, text) = last_log(&st);
    assert!(!ok && text.starts_with("A worktree is already registered at "), "{text}");
    form(&mut st).folder = "relative/dir".into();
    assert!(update(&mut st, wt(WorktreeMsg::Submit), Instant::now()).is_empty());
    assert_eq!(last_log(&st).1, "Enter the full path of the folder.");
    assert!(st.modal.is_some());
    cleanup(&dir);
}

#[test]
fn busy_and_stale_tab() {
    let (mut st, dir) = opened("wt-busy");
    drive(&mut st, wt(WorktreeMsg::CreateDialog(WorktreeBase::Branch("side".into()))));
    st.job = Some(RunningJob {
        label: "Fetch".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        keeps_undo: true,
    });
    assert!(update(&mut st, wt(WorktreeMsg::Submit), Instant::now()).is_empty());
    assert_eq!(last_log(&st), (false, "Another operation is still running.".to_owned()));
    assert!(st.modal.is_some() && !form(&mut st).busy());
    st.job = None;

    // The dialog is waiting for a request whose tab is no longer the active one.
    let request = NewWorktree {
        repo: trellis_core::paths::temp_dir().join("trellis-elsewhere"),
        base: WorktreeBase::Branch("side".into()),
        folder: wt_root(&dir).join("side"),
        branch: None,
        open: true,
    };
    form(&mut st).pending = Some(request.clone());
    let fx = update(&mut st, wt(WorktreeMsg::FolderChecked { request, result: Ok(()) }), Instant::now());
    assert!(fx.is_empty());
    assert_eq!(last_log(&st), (false, "The worktree was not created: its repository's tab is no longer active.".to_owned()));
    assert!(st.modal.is_none());
    cleanup(&dir);
}

/// The disk check's result arrives after Cancel, or after the dialog was opened again for
/// another request: nothing is created and the open dialog is left alone.
#[test]
fn folder_check_after_cancel_is_dropped() {
    let (mut st, dir) = opened("wt-cancel");
    let checked = |st: &mut AppState| {
        let fx = update(st, wt(WorktreeMsg::Submit), Instant::now());
        let [Effect::CheckWorktreeFolder(req)] = &fx[..] else { panic!("{fx:?}") };
        let req = req.clone();
        let Some(Msg::Worktree(WorktreeMsg::FolderChecked { result, .. })) = runner::execute(fx.into_iter().next().unwrap()) else {
            panic!("no folder check result")
        };
        (req, result)
    };
    let result_for = |request: &NewWorktree, result: &Result<(), String>| {
        wt(WorktreeMsg::FolderChecked { request: request.clone(), result: result.clone() })
    };
    let root = wt_root(&dir);
    drive(&mut st, wt(WorktreeMsg::CreateDialog(WorktreeBase::Branch("main".into()))));
    form(&mut st).branch = "cancelled".into();
    form(&mut st).folder = root.join("cancelled").display().to_string();
    let (first, first_checked) = checked(&mut st);
    assert!(form(&mut st).busy());
    drive(&mut st, Msg::CloseModal);
    let logged = st.log.len();
    assert!(update(&mut st, result_for(&first, &first_checked), Instant::now()).is_empty(), "no job after Cancel");
    assert!(st.modal.is_none() && st.job.is_none());
    assert_eq!(st.log.len(), logged, "nothing to report: the user cancelled");

    // A newer dialog waiting for its own request ignores the old result.
    drive(&mut st, wt(WorktreeMsg::CreateDialog(WorktreeBase::Branch("main".into()))));
    form(&mut st).branch = "newer".into();
    form(&mut st).folder = root.join("newer").display().to_string();
    let (second, second_checked) = checked(&mut st);
    assert_ne!(first, second);
    assert!(update(&mut st, result_for(&first, &first_checked), Instant::now()).is_empty());
    assert!(st.job.is_none());
    assert_eq!(form(&mut st).pending.as_ref(), Some(&second), "still waiting for its own check");

    // Its own result creates the worktree and closes it.
    let fx = update(&mut st, result_for(&second, &second_checked), Instant::now());
    let ran = run(&mut st, fx);
    assert_eq!(ran, [args_of(&["worktree", "add", "-b", "newer", &second.folder.display().to_string(), "main"])]);
    assert!(st.modal.is_none());
    let names: Vec<_> = st.snap.as_ref().unwrap().worktrees.iter().filter_map(|w| w.branch.clone()).collect();
    assert!(names.contains(&"newer".to_owned()) && !names.contains(&"cancelled".to_owned()), "{names:?}");
    assert!(!first.folder.exists(), "the cancelled worktree was not created");
    let repo = git2::Repository::open(&dir).unwrap();
    assert!(repo.find_branch("cancelled", git2::BranchType::Local).is_err(), "nor its branch");
    cleanup(&dir);
}

#[test]
fn browse_picks_the_parent() {
    let (mut st, dir) = opened("wt-browse");
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::PickFolder(FolderPurpose::WorktreeParent)), Instant::now());
    match &fx[..] {
        [Effect::PickFolder { title, purpose: FolderPurpose::WorktreeParent }] => assert_eq!(title, "Create the worktree in…"),
        other => panic!("{other:?}"),
    }
    drive(&mut st, wt(WorktreeMsg::CreateDialog(WorktreeBase::Branch("side".into()))));
    form(&mut st).branch = "feature/x".into();
    let parent = trellis_core::paths::temp_dir().join("picked");
    drive(&mut st, Msg::Prefs(PrefsMsg::FolderPicked { purpose: FolderPurpose::WorktreeParent, path: Some(parent.clone()) }));
    let f = form(&mut st);
    assert_eq!(PathBuf::from(&f.folder), parent.join("feature-x"));
    assert!(f.folder_edited);
    cleanup(&dir);
}

#[test]
fn section_text_rules() {
    let (mut st, dir) = opened("wt-text");
    let snap = st.snap.as_mut().unwrap();
    assert_eq!(snap.worktrees.len(), 1);
    assert!(!section_shown(snap), "only the main worktree");
    assert_eq!(checked_out_in(snap, "main").map(|w| w.is_main()), Some(true));
    assert!(checked_out_in(snap, "side").is_none());

    let main = snap.worktrees[0].clone();
    let mut linked = main.clone();
    linked.name = Some("x".into());
    linked.current = false;
    linked.path = main.path.with_file_name("x");
    linked.branch = None;
    linked.head = Some(Oid::from_str("abc1234def5678901234567890123456789abcde").unwrap());
    assert_eq!(right_text(&linked), "detached abc1234");
    linked.locked = Some(String::new());
    linked.missing = true;
    assert_eq!(right_text(&linked), "detached abc1234 · locked · missing");
    assert_eq!(right_text(&main), "main");
    snap.worktrees.push(linked);
    assert!(section_shown(snap));

    let inside = main.path.join("sub").display().to_string();
    let warning = folder_warning(snap, &inside).expect("a warning inside the main worktree");
    assert!(warning.starts_with(&format!("This folder is inside the worktree at {}.", main.path.display())), "{warning}");
    let sibling = format!("{}-wt", main.path.display());
    assert_eq!(folder_warning(snap, &sibling), None, "a sibling with a shared prefix");
    cleanup(&dir);
}
