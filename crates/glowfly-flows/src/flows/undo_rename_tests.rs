//! Undo and redo of a branch rename through the runner: the plain rename, and a rename with the
//! remote, whose undo renames it back here only.

use super::branch_tests::with_origin;
use super::tests::drive;
use glowfly_core::gitcli::{args, run as git_run};
use glowfly_core::i18n::Lang;
use glowfly_model::msg::{BranchMsg, JobMsg, Msg};
use glowfly_model::state::AppState;
use std::path::Path;

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn rename(remote: bool) -> Msg {
    Msg::Branch(BranchMsg::RenameBranch { old: "feature".into(), new: "feature-2".into(), remote })
}

fn undo_label(st: &AppState) -> String {
    st.session.undo.undo_entry(Lang::En, true).0
}

/// Whether `dir` (a working or bare repository) has branch `name`.
fn has_branch(dir: &Path, name: &str) -> bool {
    git_run(dir, &args(&["rev-parse", "--verify", "--quiet", &format!("refs/heads/{name}")])).0
}

/// Branch `name`'s upstream, as git config says.
fn upstream(dir: &Path, name: &str) -> Option<(String, String)> {
    let repo = git2::Repository::open(dir).unwrap();
    let cfg = repo.config().unwrap().snapshot().unwrap();
    let get = |k: &str| cfg.get_string(&format!("branch.{name}.{k}")).ok();
    Some((get("remote")?, get("merge")?))
}

#[test]
fn a_rename_undoes_and_redoes() {
    let (mut st, work, bare) = with_origin("undo-flow-rename");
    drive(&mut st, rename(false));
    assert!(has_branch(&work, "feature-2") && !has_branch(&work, "feature"));
    assert_eq!(undo_label(&st), "Undo rename of branch feature to feature-2");
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(has_branch(&work, "feature") && !has_branch(&work, "feature-2"), "the old name is back");
    assert_eq!(upstream(&work, "feature"), Some(("origin".into(), "refs/heads/feature".into())), "with its upstream");
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert!(has_branch(&work, "feature-2") && !has_branch(&work, "feature"), "renamed again");
    assert_eq!(upstream(&work, "feature-2"), Some(("origin".into(), "refs/heads/feature".into())));
    cleanup(&[&work, &bare]);
}

#[test]
fn a_rename_with_the_remote_is_undone_here_only() {
    let (mut st, work, bare) = with_origin("undo-flow-rename-remote");
    drive(&mut st, rename(true));
    assert!(has_branch(&bare, "feature-2") && !has_branch(&bare, "feature"), "renamed on origin too");
    let label = undo_label(&st);
    assert_eq!(label, "Undo rename of branch feature to feature-2 (here only, not on origin)");
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(has_branch(&work, "feature") && !has_branch(&work, "feature-2"), "the local name is back");
    assert!(has_branch(&bare, "feature-2") && !has_branch(&bare, "feature"), "the remote keeps the new name");
    assert_eq!(upstream(&work, "feature"), Some(("origin".into(), "refs/heads/feature-2".into())), "still tracking the renamed one");
    cleanup(&[&work, &bare]);
}
