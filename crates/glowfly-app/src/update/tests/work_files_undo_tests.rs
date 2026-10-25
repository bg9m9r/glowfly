//! Headless tests for undoing Delete… and Unstage and delete… end to end with the test build's
//! stand-in trash: the file (and its staged entry) comes back, redo deletes it again, nothing
//! at the path is overwritten, the toasts say what Undo can do, a result for another tab goes
//! into that tab's history, and the home Trash offer when a drive's trash can't be used.

use super::tests::{drive, git, keys, scratch_repo, state};
use super::update;
use super::work_files_tests::{entry, loaded, run};
use crate::effect::{DeleteMode, Effect, FileOp, WorkFileOk};
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::TabsMsg;
use crate::msg::WorkFileMsg;
use crate::runner;
use crate::state::files::TRASH;
use crate::state::{AppState, Modal};
use glowfly_core::repo::{Deleted, TrashProblem, TrashRefusal};
use glowfly_core::undo::{Action, DeleteVia, Saved};
use std::time::Instant;

/// How many files in the stand-in bin hold `contents`.
fn in_bin(contents: &str) -> usize {
    let Ok(dir) = std::fs::read_dir(runner::files::test_bin()) else { return 0 };
    dir.flatten().filter(|e| std::fs::read_to_string(e.path()).is_ok_and(|t| t == contents)).count()
}

fn said(st: &AppState) -> Vec<&str> {
    st.toasts.iter().map(|t| t.text.as_str()).chain(st.log.iter().map(|l| l.text.as_str())).collect()
}

fn delete_confirmed(path: &str) -> Msg {
    Msg::WorkFile(WorkFileMsg::DeleteFile { path: path.into(), confirmed: true })
}

/// An undo entry for a delete of `path`, as the runner reports one.
fn undo_entry(path: &str) -> Box<Action> {
    let before = Saved { blob: git2::Oid::zero(), executable: false };
    Box::new(Action::DeleteFile { path: path.into(), before, via: DeleteVia::Trash, staged: None })
}

fn deleted_ok(path: &str, undo: bool, too_large: bool) -> WorkFileOk {
    WorkFileOk::Deleted { how: Deleted::Trashed, undo: undo.then(|| undo_entry(path)), too_large }
}

#[test]
fn a_deleted_file_comes_back_with_undo_and_goes_again_with_redo() {
    let dir = scratch_repo("wfu-e2e");
    let contents = format!("undo me {}\n", dir.display());
    std::fs::write(dir.join("a.txt"), &contents).unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, delete_confirmed("a.txt"));
    assert!(!dir.join("a.txt").exists());
    assert!(matches!(st.undo.undo.last(), Some(Action::DeleteFile { path, via: DeleteVia::Trash, .. }) if path == "a.txt"));
    assert_eq!(st.undo.undo_entry(st.settings.lang(), true).0, "Undo delete of a.txt");
    let bin_before = in_bin(&contents);

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), contents);
    assert!(st.job.is_none() && st.undo.redo.len() == 1, "{:?}", st.undo);
    assert!(said(&st).iter().any(|t| t.contains(&format!("Put back a.txt (its copy in the {TRASH} stays there)"))), "{:?}", said(&st));

    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert!(!dir.join("a.txt").exists());
    assert_eq!(in_bin(&contents), bin_before + 1, "redo moved it into the stand-in bin again");
    assert!(matches!(st.undo.undo.last(), Some(Action::DeleteFile { .. })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_leaves_a_file_that_is_back_alone_and_keeps_the_entry() {
    let dir = scratch_repo("wfu-in-the-way");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, delete_confirmed("a.txt"));
    std::fs::write(dir.join("a.txt"), "someone else's\n").unwrap();
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "someone else's\n");
    assert!(matches!(st.undo.undo.last(), Some(Action::DeleteFile { .. })), "the entry is kept: {:?}", st.undo);
    assert!(said(&st).iter().any(|t| t.contains("a.txt is in your working tree again; move it away")), "{:?}", said(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_and_delete_is_staged_again_by_undo() {
    let dir = scratch_repo("wfu-staged");
    std::fs::write(dir.join("n.txt"), "new\n").unwrap();
    git(&dir, &["add", "n.txt"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::WorkFile(WorkFileMsg::DeleteFile { path: "n.txt".into(), confirmed: false }));
    assert!(matches!(&st.modal, Some(Modal::ConfirmDanger { button, .. }) if button == "Unstage and delete"));
    drive(&mut st, Msg::ConfirmAccepted);
    assert!(!dir.join("n.txt").exists());
    assert!(!st.snap.as_ref().unwrap().staged.iter().any(|f| f.path == "n.txt"));

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(std::fs::read_to_string(dir.join("n.txt")).unwrap(), "new\n");
    assert!(st.snap.as_ref().unwrap().staged.contains(&entry("n.txt", 'A')), "{:?}", st.snap.as_ref().unwrap().staged);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn toasts_say_what_undo_can_do() {
    let (mut st, dir) = loaded("wfu-toasts");
    let delete = |mode| FileOp::Delete { path: "a.txt".into(), mode };
    let toast = |st: &mut AppState, op: FileOp, ok: WorkFileOk| {
        run(st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op, result: Ok(ok) }));
        st.toasts.last().map(|t| t.text.clone()).unwrap_or_default()
    };
    let key = keys("Ctrl+Z");
    assert_eq!(
        toast(&mut st, delete(DeleteMode::Trash), deleted_ok("a.txt", true, false)),
        format!("Moved a.txt to the {TRASH}. Undo ({key}) puts it back.")
    );
    assert!(matches!(st.undo.undo.last(), Some(Action::DeleteFile { .. })), "recorded");
    let permanent = WorkFileOk::Deleted { how: Deleted::Permanently, undo: None, too_large: true };
    assert_eq!(toast(&mut st, delete(DeleteMode::Permanent), permanent), "Deleted a.txt. It's over 64 MB, so no copy was kept for Undo.");
    assert_eq!(
        toast(&mut st, delete(DeleteMode::Trash), deleted_ok("a.txt", false, true)),
        format!("Moved a.txt to the {TRASH}. It's over 64 MB, so Undo can't put it back; restore it from the {TRASH}.")
    );
    let with_undo = WorkFileOk::Deleted { how: Deleted::Permanently, undo: Some(undo_entry("a.txt")), too_large: false };
    assert_eq!(toast(&mut st, delete(DeleteMode::Permanent), with_undo), format!("Deleted a.txt. Undo ({key}) puts it back."));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_delete_for_an_inactive_tab_goes_into_its_history() {
    let (a, b) = (scratch_repo("wfu-tab-a"), scratch_repo("wfu-tab-b"));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));

    let effect = update(&mut st, delete_confirmed("a.txt"), Instant::now())
        .into_iter()
        .find(|e| matches!(e, Effect::WorkFile { .. }))
        .expect("a delete");
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), Instant::now());
    let reply = runner::execute(effect).expect("a result");
    assert!(matches!(reply, Msg::Scoped { .. }), "scoped to tab A's repository");
    update(&mut st, reply, Instant::now());

    assert!(!a.join("a.txt").exists());
    assert!(st.undo.undo.is_empty(), "tab B's history is untouched");
    let parked = st.tabs[0].parked.as_ref().expect("tab A is parked");
    assert!(matches!(parked.undo.undo.last(), Some(Action::DeleteFile { path, .. }) if path == "a.txt"), "{:?}", parked.undo);
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some(format!("Moved a.txt to the {TRASH}. Undo puts it back.").as_str()));
    for d in [&a, &b] {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn offer(st: &AppState) -> &crate::state::files::TrashOffer {
    match &st.modal {
        Some(Modal::TrashOffer(o)) => o,
        _ => panic!("expected the home Trash offer"),
    }
}

#[test]
fn a_drive_without_a_usable_trash_offers_the_home_trash() {
    let (mut st, dir) = loaded("wfu-home-offer");
    let refusal = || TrashRefusal::new(TrashProblem::HomeTrashOnly, "/mnt/usb has no trash folder you can use");
    for op in [
        FileOp::UnstageDelete { path: "a.txt".into(), mode: DeleteMode::Trash },
        FileOp::Delete { path: "a.txt".into(), mode: DeleteMode::Trash },
        FileOp::Discard { path: "a.txt".into(), mode: DeleteMode::Trash },
    ] {
        run(&mut st, Msg::WorkFile(WorkFileMsg::TrashRefused { op: op.clone(), refusal: refusal() }));
        let o = offer(&st);
        assert_eq!((&o.op, o.path.as_str()), (&op, "a.txt"));
        assert_eq!(o.op.clone().home_trash().delete_path(), Some("a.txt"));
        let home = o.op.clone().home_trash();
        assert!(
            matches!(
                home,
                FileOp::UnstageDelete { mode: DeleteMode::HomeTrash, .. }
                    | FileOp::Delete { mode: DeleteMode::HomeTrash, .. }
                    | FileOp::Discard { mode: DeleteMode::HomeTrash, .. }
            ),
            "{home:?}"
        );
        let permanent = o.op.clone().permanent();
        assert!(
            matches!(
                permanent,
                FileOp::UnstageDelete { mode: DeleteMode::PermanentNoTrash, .. }
                    | FileOp::Delete { mode: DeleteMode::PermanentNoTrash, .. }
                    | FileOp::Discard { mode: DeleteMode::PermanentNoTrash, .. }
            ),
            "{permanent:?}"
        );
        if matches!(op, FileOp::Delete { .. }) {
            assert_eq!(
                o.body,
                format!(
                    "a.txt is on a drive whose trash folder Glowfly can't use: /mnt/usb has no trash folder you can use.\n\nMove it to the Trash in your home folder instead? It is copied there, which can take a while for a large file. Or delete it permanently; Undo ({}) can still bring it back while this tab is open.",
                    keys("Ctrl+Z")
                )
            );
        }
        if matches!(op, FileOp::Discard { .. }) {
            assert!(o.body.ends_with("Or delete it permanently, which can't be undone."), "{}", o.body);
        }
    }
    // Esc closes it like Cancel.
    assert!(run(&mut st, Msg::CloseModal).is_empty());
    assert!(st.modal.is_none());

    let op = FileOp::Delete { path: "a.txt".into(), mode: DeleteMode::HomeTrash };
    run(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op, result: Ok(WorkFileOk::deleted(Deleted::Trashed)) }));
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Moved a.txt to the Trash in your home folder."));
    let _ = std::fs::remove_dir_all(&dir);
}
