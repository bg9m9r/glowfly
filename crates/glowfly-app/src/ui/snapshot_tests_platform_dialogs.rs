//! Questions that come from what the operating system would do: running a working-tree file
//! that opens as a program, retrying a move to the trash that another program blocked, a git
//! too old for some features, and a repository another user owns.
//! Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::effect::{DeleteMode, FileOp};
use crate::msg::{FilesMsg, Msg, WorkFileMsg};
use crate::state::Modal;
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::{TrashProblem, TrashRefusal};
use glowfly_core::settings::ThemeChoice;

#[test]
fn confirm_run_program() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "confirm-run-program");
    let workdir = st.workdir().unwrap();
    let found = FilesMsg::OpenNeedsConfirm { workdir, path: "tools/run.bat".into(), kind: "a script" };
    crate::update::update(&mut st, Msg::Files(found), std::time::Instant::now());
    assert!(matches!(&st.modal, Some(Modal::Confirm { .. })));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("confirm_run_program");
    h.get_by_label("Run").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ConfirmAccepted)), "the button accepts");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn trash_in_use_retry() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "trash-in-use");
    let refusal = TrashRefusal::new(TrashProblem::InUse, "in use");
    let op = FileOp::Delete { path: "notes/scratch.txt".into(), mode: DeleteMode::Trash };
    crate::update::update(&mut st, Msg::WorkFile(WorkFileMsg::TrashRefused { op, refusal }), std::time::Instant::now());
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("trash_in_use_retry");
    assert!(h.query_by_label("Delete permanently").is_none(), "no permanent delete for a file in use");
    h.get_by_label("Retry").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ConfirmAccepted)), "Retry accepts");
    drop(h);
    remove_fixture(&dir);
}

/// The trash on the file's drive can't be used (Linux): the home Trash or a permanent delete.
#[test]
fn trash_home_offer() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "trash-home-offer");
    let refusal = TrashRefusal::new(TrashProblem::HomeTrashOnly, "/mnt/usb has no trash folder you can use");
    let op = FileOp::Delete { path: "notes/scratch.txt".into(), mode: DeleteMode::Trash };
    crate::update::update(&mut st, Msg::WorkFile(WorkFileMsg::TrashRefused { op, refusal }), std::time::Instant::now());
    assert!(matches!(&st.modal, Some(Modal::TrashOffer(_))));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("trash_home_offer");
    let sent = |msgs: &[Msg], want: DeleteMode| {
        msgs.iter().any(|m| matches!(m, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(FileOp::Delete { mode, .. })) if *mode == want))
    };
    h.get_by_label("Move to home Trash").click();
    h.run();
    assert!(sent(&h.state().1, DeleteMode::HomeTrash), "moves it to the home Trash");
    assert!(!sent(&h.state().1, DeleteMode::PermanentNoTrash));
    h.get_by_label("Delete permanently").click();
    h.run();
    assert!(sent(&h.state().1, DeleteMode::PermanentNoTrash), "or deletes it for good");
    drop(h);
    remove_fixture(&dir);
}

/// The banner while git is older than every feature needs; Choose git… opens Preferences > Git
/// and Dismiss closes it.
#[test]
fn git_too_old_banner() {
    use crate::git_env::GitEnvMsg;
    use glowfly_core::git_version::GitInfo;
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "git-too-old");
    let info = GitInfo { path: Some("C:/Program Files/Git/cmd/git.exe".into()), version: Some((2, 34, 1)), chosen: false };
    crate::update::update(&mut st, Msg::GitEnv(GitEnvMsg::Detected { generation: 0, info }), std::time::Instant::now());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("git_too_old_banner");
    h.get_by_label("Choose git…").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::GitEnv(GitEnvMsg::ShowPage))), "opens Preferences > Git");
    h.get_by_label("Dismiss").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::GitEnv(GitEnvMsg::DismissBanner))));
    drop(h);
    remove_fixture(&dir);
}

/// What opening a repository another user owns asks.
#[test]
fn repository_owned_by_another_user() {
    use crate::effect::LoadKind;
    use crate::git_env::GitEnvMsg;
    use crate::msg::RepoMsg;
    let mut st = crate::state::AppState::new(super::snapshot_tests::settings(ThemeChoice::Light), Vec::new(), std::time::Instant::now());
    let root = std::path::PathBuf::from(if cfg!(windows) { r"\\nas\team\app" } else { "/mnt/nas/team/app" });
    crate::update::update(&mut st, Msg::Repo(RepoMsg::Open(root.clone())), std::time::Instant::now());
    let generation = st.load_gen;
    crate::update::update(&mut st, Msg::GitEnv(GitEnvMsg::NotOwned { generation, kind: LoadKind::Open, root }), std::time::Instant::now());
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("repository_owned_by_another_user");
    h.get_by_label("Trust and open").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::ConfirmAccepted)));
}
