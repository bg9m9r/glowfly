//! Headless tests for what Glowfly records for undo when it renames a branch (also with its
//! remote), renames or removes a remote, and creates a worktree with a new branch; the Fetch
//! button after a removed remote comes back, and the refusal while a worktree to remove is open
//! in a tab. The undos themselves run through the runner in glowfly-flows.

use super::tests::{git, scratch_repo, state};
use crate::effect::{Effect, JobEffect, UndoResult};
use crate::msg::{BranchMsg, JobMsg, Msg, TabsMsg};
use crate::remote::RemoteMsg;
use crate::state::remotes::RemoteForm;
use crate::state::worktrees::WorktreeForm;
use crate::state::{AppState, Modal, Tab};
use crate::worktree::{NewWorktree, WorktreeMsg};
use glowfly_core::repo::{LoadOptions, load};
use glowfly_core::undo::{Action, Created, RemoteConfig, UndoKind};
use glowfly_core::worktree_ops::WorktreeBase;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn send(st: &mut AppState, m: Msg) -> Vec<Effect> {
    super::update(st, m, Instant::now())
}

/// A state whose only tab shows `dir`, loaded.
fn opened(dir: &Path) -> AppState {
    let mut st = state();
    st.tabs[0] = Tab::unloaded(dir.to_path_buf());
    st.snap = Some(load(dir, &LoadOptions::all(10)).unwrap());
    st
}

/// `scratch_repo` with a bare `origin` next to it that `main` is pushed to with its upstream.
fn with_origin(name: &str) -> (PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    let bare = dir.with_file_name(format!("{}-origin.git", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&bare);
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare", "-b", "main"]);
    git(&dir, &["remote", "add", "origin", &bare.display().to_string()]);
    git(&dir, &["push", "-q", "-u", "origin", "main"]);
    (dir, bare)
}

/// The undo record of the git job `fx` starts.
fn record(fx: &[Effect]) -> Option<UndoKind> {
    fx.iter()
        .find_map(|e| match e {
            Effect::Job(JobEffect::RunGit { record, .. }) => Some(record.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no git job in {fx:?}"))
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn a_branch_rename_records_it_also_with_its_remote() {
    let (dir, bare) = with_origin("undo-rec-rename");
    git(&dir, &["branch", "feature"]);
    git(&dir, &["push", "-q", "-u", "origin", "feature"]);
    let mut st = opened(&dir);
    let rename = |remote| Msg::Branch(BranchMsg::RenameBranch { old: "feature".into(), new: "feature-2".into(), remote });
    let fx = send(&mut st, rename(false));
    assert_eq!(record(&fx), Some(UndoKind::RenameBranch { old: "feature".into(), new: "feature-2".into(), remote: None }));

    st.job = None;
    let fx = send(&mut st, rename(true));
    let with_remote = UndoKind::RenameBranch { old: "feature".into(), new: "feature-2".into(), remote: Some("origin".into()) };
    assert_eq!(record(&fx), Some(with_remote));
    // A later push failed: the local rename is still recorded.
    let action = Action::RenameBranch { old: "feature".into(), new: "feature-2".into(), remote: Some("origin".into()) };
    let done =
        JobMsg::JobDone { ok: false, output: "error: failed to push".into(), undo: Some(UndoResult::Recorded(Some(action.clone()))) };
    send(&mut st, Msg::Job(done));
    assert_eq!(st.session.undo.undo.last(), Some(&action));
    cleanup(&[&dir, &bare]);
}

#[test]
fn remote_remove_and_rename_are_recorded_and_a_url_edit_is_not() {
    let (dir, bare) = with_origin("undo-rec-remote");
    let mut st = opened(&dir);
    let fx = send(&mut st, Msg::Remote(RemoteMsg::Remove("origin".into())));
    assert_eq!(record(&fx), Some(UndoKind::RemoveRemote("origin".into())));

    let info = st.snap.as_ref().unwrap().remote_info.iter().find(|r| r.name == "origin").cloned().unwrap();
    let submit = |st: &mut AppState, name: &str, url: &str| {
        st.job = None;
        let mut form = RemoteForm::edit(&info);
        form.name = name.into();
        form.url = url.into();
        st.modal = Some(Modal::Remote(form));
        send(st, Msg::Remote(RemoteMsg::Submit))
    };
    let url = info.fetch_url().to_owned();
    let fx = submit(&mut st, "upstream", &url);
    assert_eq!(record(&fx), Some(UndoKind::RenameRemote { old: "origin".into(), new: "upstream".into() }));
    let fx = submit(&mut st, " upstream ", "https://example.invalid/new.git");
    assert_eq!(record(&fx), Some(UndoKind::RenameRemote { old: "origin".into(), new: "upstream".into() }), "the name is trimmed");
    let fx = submit(&mut st, "origin", "https://example.invalid/new.git");
    assert_eq!(record(&fx), None, "a URL-only edit isn't recorded");
    cleanup(&[&dir, &bare]);
}

fn removed() -> Action {
    let config =
        RemoteConfig { entries: vec![("url".into(), "https://example.invalid/a.git".into())], branches: vec![], push_default: false };
    Action::RemoveRemote { name: "origin".into(), config }
}

/// Undoes the removed remote on top of `st`'s history and reports it done.
fn undo_done(st: &mut AppState, park: bool) {
    st.session.undo.record(removed());
    let fx = send(st, Msg::Job(JobMsg::Undo));
    assert!(fx.iter().any(|e| matches!(e, Effect::Job(JobEffect::UndoRedo { .. }))), "{fx:?}");
    if park {
        send(st, Msg::Tabs(TabsMsg::NewTab));
    }
    let undo = UndoResult::Done { redo: false, before: removed(), after: Box::new(removed()), stash: None };
    send(st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: Some(undo) }));
}

#[test]
fn a_removed_remote_put_back_offers_to_fetch_it() {
    let dir = scratch_repo("undo-rec-fetch");
    let mut st = opened(&dir);
    undo_done(&mut st, false);
    let t = st.toasts.last().expect("a toast");
    assert_eq!(t.text, "Remote origin is back. Fetch it to bring back its remote-tracking branches.");
    let action = t.action.as_ref().expect("the Fetch button");
    assert_eq!(action.label, "Fetch origin");
    assert!(matches!(&*action.msg, Msg::Remote(RemoteMsg::Fetch(n)) if n == "origin"));
    assert!(action.workdir.as_deref().is_some_and(|w| glowfly_core::paths::same_spelling(w, &dir)));

    // Finished while another tab is active: no button.
    let mut parked = opened(&dir);
    undo_done(&mut parked, true);
    assert!(parked.toasts.iter().all(|t| t.action.as_ref().is_none_or(|a| a.label != "Fetch origin")));
    cleanup(&[&dir]);
}

fn wt_folder(dir: &Path) -> PathBuf {
    let folder = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// Sends the folder check's result for a Create worktree dialog waiting on `req`.
fn checked(st: &mut AppState, req: NewWorktree) -> Vec<Effect> {
    st.job = None;
    let snap = st.snap.as_ref().unwrap();
    let mut form = WorktreeForm::new(snap, req.base.clone());
    form.pending = Some(req.clone());
    st.modal = Some(Modal::NewWorktree(form));
    send(st, Msg::Worktree(WorktreeMsg::FolderChecked { request: req, result: Ok(()) }))
}

#[test]
fn a_worktree_with_a_new_branch_is_recorded() {
    let (dir, bare) = with_origin("undo-rec-wt");
    git(&dir, &["branch", "side"]);
    git(&dir, &["push", "-q", "origin", "side"]);
    git(&dir, &["branch", "-D", "side"]);
    let mut st = opened(&dir);
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    let folder = wt_folder(&dir);
    let req = |base, branch: Option<&str>| NewWorktree {
        repo: dir.clone(),
        base,
        folder: folder.clone(),
        branch: branch.map(Into::into),
        open: false,
    };
    let fx = checked(&mut st, req(WorktreeBase::Commit(head), Some("feature")));
    assert_eq!(record(&fx), Some(UndoKind::AddWorktree { path: folder.clone(), branch: "feature".into() }));
    let remote = WorktreeBase::Remote { remote: "origin".into(), branch: "side".into() };
    let fx = checked(&mut st, req(remote, Some("side")));
    assert_eq!(record(&fx), Some(UndoKind::AddWorktree { path: folder.clone(), branch: "side".into() }), "a remote base makes a branch");
    let fx = checked(&mut st, req(WorktreeBase::Commit(head), None));
    assert_eq!(record(&fx), None, "a detached worktree makes no branch");
    cleanup(&[&dir, &bare]);
}

#[test]
fn undoing_a_worktree_open_in_a_tab_is_refused() {
    let dir = scratch_repo("undo-rec-wt-open");
    let folder = wt_folder(&dir);
    git(&dir, &["worktree", "add", "-q", "-b", "wt", &folder.display().to_string(), "HEAD"]);
    let mut st = opened(&dir);
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    let listed = st.snap.as_ref().unwrap().worktrees.iter().find(|w| !w.is_main()).map(|w| w.path.clone()).expect("listed");
    let action = Action::AddWorktree { path: listed.clone(), created: Created { name: "wt".into(), oid: head, upstream: None } };
    st.session.undo.record(action);
    st.tabs.push(Tab::unloaded(listed));
    let fx = send(&mut st, Msg::Job(JobMsg::Undo));
    assert!(fx.is_empty() && st.job.is_none(), "{fx:?}");
    let name = folder.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(st.log.last().map(|l| l.text.clone()), Some(format!("{name} is open in a tab. Close that tab, then undo.")));

    st.tabs.pop();
    let fx = send(&mut st, Msg::Job(JobMsg::Undo));
    assert!(fx.iter().any(|e| matches!(e, Effect::Job(JobEffect::UndoRedo { .. }))), "starts once no tab shows it: {fx:?}");
    git(&dir, &["worktree", "remove", "--force", &folder.display().to_string()]);
    cleanup(&[&folder, &dir]);
}
