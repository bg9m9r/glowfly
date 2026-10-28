//! Headless tests for deleting the Git Flow production or development branch: the branch Delete
//! dialog's and the shared danger confirmation's firmer words for it (and the plain ones
//! otherwise), running or cancelling the delete, the remote and pull request variants, and the
//! Remove worktree dialog's role.

use super::gitflow_tests::{cleanup, git_job, initialized, job_label, open, run};
use super::tests::{drive, finish, git};
use crate::msg::{BranchMsg, JobMsg, Msg, RepoMsg};
use crate::state::gitflow_base::{Deleting, guard};
use crate::state::worktree_manage::RemoveForm;
use crate::state::{AppState, Modal};
use glowfly_core::forge::PrState;
use glowfly_core::gitcli::{args, run as git_run};
use glowfly_core::gitflow::FlowBase;
use glowfly_core::i18n::Lang;

const DEVELOP: &str = "develop is the Git Flow development branch: features and releases start from it and every Finish \
                       merges into it. Until it exists again or Preferences > Git Flow names another branch, those steps \
                       can't run.";
const MAIN: &str = "main is the Git Flow production branch: releases and hotfixes are merged and tagged there, and \
                    hotfixes start from it. Until it exists again or Preferences > Git Flow names another branch, those \
                    steps can't run.";
const REMOTE: &str = "origin holds the copy everyone else uses.";
const T: Lang = Lang::En;

/// The branch menu's plain Delete together with the remote branch confirmation for `branch`,
/// as `branch_menu::delete_items` builds it before `guard`.
fn plain(branch: &str) -> Msg {
    Msg::confirm_danger(
        T.tr("branch.delete_both.title"),
        format!("Delete {branch} and origin/{branch}?"),
        T.tr("branch.delete_both.button"),
        Msg::git("Delete branch", &["branch", "-d", branch]),
    )
}

/// `plain`, through `guard` as the menu does it.
fn menu(st: &AppState, branch: &str) -> Msg {
    guard(T, st.snap.as_ref().unwrap(), branch, Deleting::LocalAndRemote("origin"), plain(branch))
}

/// Title, body, button, the acknowledgement and the command the confirmation runs.
type Parts = (String, String, String, Option<String>, Vec<Vec<String>>);

fn parts(m: &Msg) -> Parts {
    let Msg::ConfirmDanger { title, body, button, ack, then } = m else { panic!("a danger confirmation") };
    let Msg::Job(JobMsg::Git { cmds, .. }) = then.as_ref() else { panic!("a git job") };
    (title.clone(), body.clone(), button.clone(), ack.clone(), cmds.clone())
}

/// The local Delete dialog `branch`'s menu item opens: its title, button and warning.
fn dialog(st: &mut AppState, branch: &str) -> (String, String, Option<FlowBase>) {
    drive(st, Msg::Branch(BranchMsg::DeleteDialog(branch.into())));
    let Some(Modal::DeleteBranch(form)) = st.modal.take() else { panic!("the Delete dialog for {branch}") };
    (form.title(T).into(), form.button(T).into(), form.role)
}

#[test]
fn confirm_delete_for_bases() {
    let (mut st, dir) = initialized("base-confirm");
    let both = parts(&menu(&st, "develop"));
    assert_eq!(both.0, "Delete development branch");
    assert_eq!(
        both.1,
        format!(
            "Delete develop and origin/develop?

{DEVELOP} {REMOTE}"
        )
    );
    assert_eq!((both.2.as_str(), both.3.as_ref()), ("Delete both anyway", None));
    let warn = crate::state::gitflow_base::warning(T, FlowBase::Development, "develop");
    assert_eq!(warn, DEVELOP, "the Delete dialog's warning");
    let development = ("Delete development branch".into(), "Delete anyway".into(), Some(FlowBase::Development));
    assert_eq!(dialog(&mut st, "develop"), development);
    git(&dir, &["branch", "feature/x"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(dialog(&mut st, "feature/x"), ("Delete branch".into(), "Delete".into(), None));
    assert_eq!(parts(&menu(&st, "feature/x")), parts(&plain("feature/x")));
    assert_eq!(parts(&menu(&st, "Develop")), parts(&plain("Develop")), "exact names only");
    cleanup(&dir);

    let (mut st, dir) = initialized("base-confirm-main");
    git(&dir, &["checkout", "-q", "develop"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(dialog(&mut st, "main").0, "Delete production branch");
    assert!(parts(&menu(&st, "main")).1.ends_with(&format!(
        "

{MAIN} {REMOTE}"
    )));
    cleanup(&dir);

    let (mut st, dir) = open("base-off");
    git(&dir, &["branch", "develop"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(dialog(&mut st, "develop"), ("Delete branch".into(), "Delete".into(), None), "Git Flow isn't set up");
    assert_eq!(parts(&menu(&st, "develop")), parts(&plain("develop")));
    cleanup(&dir);
}

#[test]
fn delete_base_runs_or_cancels() {
    let (mut st, dir) = initialized("base-run");
    drive(&mut st, Msg::Branch(BranchMsg::DeleteDialog("develop".into())));
    assert!(matches!(&st.modal, Some(Modal::DeleteBranch(f)) if f.title(T) == "Delete development branch"));
    run(&mut st, Msg::CloseModal);
    assert!(st.modal.is_none() && st.job.is_none());

    drive(&mut st, Msg::Branch(BranchMsg::DeleteDialog("develop".into())));
    let Some(Modal::DeleteBranch(form)) = &st.modal else { panic!("the Delete dialog") };
    let delete = form.delete_msg().expect("nothing to count: main contains develop");
    let fx = run(&mut st, delete);
    assert!(st.modal.is_none());
    assert_eq!(git_job(&fx), Some(vec![args(&["branch", "-d", "develop"])]));
    assert_eq!(job_label(&st).as_deref(), Some("Delete branch"));
    finish(&mut st, fx);
    assert!(!git_run(&dir, &args(&["rev-parse", "--verify", "--quiet", "refs/heads/develop"])).0);
    cleanup(&dir);
}

#[test]
fn remote_and_pull_request_deletes_warn() {
    let (mut st, dir) = initialized("base-remote");
    let confirm = Msg::confirm_danger(
        T.tr("remote_branch.delete.title"),
        T.trf("remote_branch.delete.body", &[("branch", &"develop"), ("remote", &"origin")]),
        T.tr("common.delete.button"),
        Msg::git("Delete remote branch", &["push", "origin", "--delete", "develop"]),
    );
    let m = parts(&guard(T, st.snap.as_ref().unwrap(), "develop", Deleting::Remote("origin"), confirm));
    assert_eq!((m.0.as_str(), m.2.as_str()), ("Delete development branch", "Delete anyway"));
    assert!(m.1.ends_with(&format!("\n\n{DEVELOP} {REMOTE}")), "{}", m.1);
    assert_eq!(m.4, vec![args(&["push", "origin", "--delete", "develop"])]);

    // A merged pull request whose source is develop (not the default branch).
    let mut d = crate::state::pull_request_actions::tests::detail();
    d.state = PrState::Merged;
    let mut v = crate::state::pull_request_actions::tests::view(Some(d));
    v.pr.source = "develop".into();
    let snap = st.snap.as_mut().unwrap();
    snap.remotes = vec![("origin".into(), vec![("develop".into(), git2::Oid::zero())])];
    let offer = crate::state::pull_request_actions::delete_branch_offer(T, &v, snap, Some("main")).expect("offered");
    let m = parts(&offer);
    assert_eq!((m.0.as_str(), m.2.as_str()), ("Delete development branch", "Delete anyway"));
    assert!(m.1.starts_with("Delete branch develop on origin?"), "{}", m.1);
    assert!(m.1.ends_with(REMOTE), "{}", m.1);
    v.pr.source = "search".into();
    snap.remotes = vec![("origin".into(), vec![("search".into(), git2::Oid::zero())])];
    let plain = parts(&crate::state::pull_request_actions::delete_branch_offer(T, &v, snap, Some("main")).expect("offered"));
    assert_eq!((plain.0.as_str(), plain.2.as_str()), ("Delete remote branch", "Delete"));
    cleanup(&dir);
}

#[test]
fn worktree_remove_warns_for_bases() {
    let (mut st, dir) = initialized("base-worktree");
    git(&dir, &["branch", "feature/x", "develop"]);
    let root = glowfly_core::paths::temp_dir().join(format!("glowfly-test-base-worktrees-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (folder, branch) in [("dev", "develop"), ("x", "feature/x")] {
        git(&dir, &["worktree", "add", "-q", &root.join(folder).display().to_string(), branch]);
    }
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let form = |st: &AppState, branch: &str| {
        let snap = st.snap.as_ref().unwrap();
        let wt = snap.worktrees.iter().find(|w| w.branch.as_deref() == Some(branch)).expect("listed");
        (RemoveForm::new(wt).flow_base, RemoveForm::new(wt).with_snap(snap, &[]).flow_base)
    };
    assert_eq!(form(&st, "develop"), (None, Some(FlowBase::Development)));
    assert_eq!(form(&st, "feature/x"), (None, None));
    git(&dir, &["config", "--unset", "gitflow.branch.develop"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(form(&st, "develop"), (None, None), "Git Flow isn't set up");
    for folder in ["dev", "x"] {
        git(&dir, &["worktree", "remove", "--force", &root.join(folder).display().to_string()]);
    }
    let _ = std::fs::remove_dir_all(&root);
    cleanup(&dir);
}
