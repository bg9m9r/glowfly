//! Headless tests for deleting the Git Flow production or development branch: the destructive
//! confirmation in place of the plain one, running or cancelling it, the remote variant, and
//! the Remove worktree dialog's warning.

use super::gitflow_tests::{cleanup, flow, git_job, initialized, job_label, open, run};
use super::tests::{drive, finish, git};
use crate::gitflow::GitFlowMsg;
use crate::msg::{Msg, RepoMsg};
use crate::state::gitflow_base::{BaseDelete, confirm_delete, confirm_remote_delete};
use crate::state::worktree_manage::RemoveForm;
use crate::state::{AppState, Modal};
use trellis_core::gitcli::{args, run as git_run};

const DEVELOP: &str = "develop is the Git Flow development branch. Features and releases start from it and every Finish \
                       merges into it. Until it exists again or Preferences > Git Flow names another branch, those steps \
                       can't run.";
const MAIN: &str = "main is the Git Flow production branch. Releases and hotfixes are merged into it and tagged there, and \
                    hotfixes start from it. Until it exists again or Preferences > Git Flow names another branch, those \
                    steps can't run.";

fn delete(st: &AppState, branch: &str) -> Msg {
    let then = Msg::git("Delete branch", &["branch", "-d", branch]);
    confirm_delete(st.snap.as_ref().unwrap(), branch, "Delete branch", format!("Delete local branch {branch}?"), "Delete", then)
}

fn base(m: Msg) -> Box<BaseDelete> {
    match m {
        Msg::GitFlow(GitFlowMsg::DeleteBase(b)) => b,
        _ => panic!("the destructive confirmation"),
    }
}

#[test]
fn confirm_delete_for_bases() {
    let (mut st, dir) = initialized("base-confirm");
    let b = base(delete(&st, "main"));
    assert_eq!(
        (b.title.as_str(), b.question.as_str(), b.button.as_str()),
        ("Delete production branch", "Delete local branch main?", "Delete anyway")
    );
    assert_eq!(b.warning, MAIN);
    let b = base(delete(&st, "develop"));
    assert_eq!((b.title.as_str(), b.warning.as_str()), ("Delete development branch", DEVELOP));
    git(&dir, &["branch", "feature/x"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(matches!(delete(&st, "feature/x"), Msg::Confirm { .. }));
    assert!(matches!(delete(&st, "Develop"), Msg::Confirm { .. }), "exact names only");

    let snap = st.snap.as_ref().unwrap();
    let then = Msg::git("Delete remote branch", &["push", "origin", "--delete", "develop"]);
    let b = base(confirm_remote_delete(
        snap,
        "origin",
        "develop",
        "Delete remote branch",
        "Delete branch develop on origin?".into(),
        "Delete",
        then,
    ));
    assert_eq!(b.warning, format!("{DEVELOP} origin holds the copy everyone else uses."));
    cleanup(&dir);

    let (st, dir) = open("base-off");
    assert!(matches!(delete(&st, "main"), Msg::Confirm { .. }), "Git Flow isn't set up");
    cleanup(&dir);
}

#[test]
fn delete_base_runs_or_cancels() {
    let (mut st, dir) = initialized("base-run");
    let m = delete(&st, "develop");
    run(&mut st, m);
    assert!(matches!(&st.modal, Some(Modal::GitFlowDeleteBase(_))));
    run(&mut st, Msg::CloseModal);
    assert!(st.modal.is_none() && st.job.is_none());

    let m = delete(&st, "develop");
    run(&mut st, m);
    let fx = flow(&mut st, GitFlowMsg::DeleteBaseConfirmed);
    assert!(st.modal.is_none());
    assert_eq!(git_job(&fx), Some(vec![args(&["branch", "-d", "develop"])]));
    assert_eq!(job_label(&st).as_deref(), Some("Delete branch"));
    finish(&mut st, fx);
    assert!(!git_run(&dir, &args(&["rev-parse", "--verify", "--quiet", "refs/heads/develop"])).0);
    // Without the dialog open, the button's message does nothing.
    assert!(git_job(&flow(&mut st, GitFlowMsg::DeleteBaseConfirmed)).is_none());
    cleanup(&dir);
}

#[test]
fn worktree_remove_warns_for_bases() {
    let (mut st, dir) = initialized("base-worktree");
    git(&dir, &["branch", "feature/x", "develop"]);
    let root = trellis_core::paths::temp_dir().join(format!("trellis-test-base-worktrees-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (folder, branch) in [("dev", "develop"), ("x", "feature/x")] {
        git(&dir, &["worktree", "add", "-q", &root.join(folder).display().to_string(), branch]);
    }
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    let form = |branch: &str| {
        let wt = snap.worktrees.iter().find(|w| w.branch.as_deref() == Some(branch)).expect("listed");
        RemoveForm::new(wt).with_snap(snap, &[])
    };
    assert_eq!(form("develop").flow_base.as_deref(), Some(DEVELOP));
    assert_eq!(form("feature/x").flow_base, None);
    for folder in ["dev", "x"] {
        git(&dir, &["worktree", "remove", "--force", &root.join(folder).display().to_string()]);
    }
    let _ = std::fs::remove_dir_all(&root);
    cleanup(&dir);
}
