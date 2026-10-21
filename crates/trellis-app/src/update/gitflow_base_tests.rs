//! Headless tests for deleting the Git Flow production or development branch: the shared danger
//! confirmation made firmer for it (and left alone otherwise), running or cancelling it, the
//! remote and pull request variants, and the Remove worktree dialog's role.

use super::gitflow_tests::{cleanup, git_job, initialized, job_label, open, run};
use super::tests::{drive, finish, git};
use crate::msg::{JobMsg, Msg, RepoMsg};
use crate::state::gitflow_base::{Deleting, guard};
use crate::state::worktree_manage::RemoveForm;
use crate::state::{AppState, Modal};
use trellis_core::forge::PrState;
use trellis_core::gitcli::{args, run as git_run};
use trellis_core::gitflow::FlowBase;
use trellis_core::i18n::Lang;

const DEVELOP: &str = "develop is the Git Flow development branch: features and releases start from it and every Finish \
                       merges into it. Until it exists again or Preferences > Git Flow names another branch, those steps \
                       can't run.";
const MAIN: &str = "main is the Git Flow production branch: releases and hotfixes are merged and tagged there, and \
                    hotfixes start from it. Until it exists again or Preferences > Git Flow names another branch, those \
                    steps can't run.";
const REMOTE: &str = "origin holds the copy everyone else uses.";
const T: Lang = Lang::En;

/// The branch menu's three plain delete confirmations for `branch`, as main's
/// `branch_menu::delete_items` builds them before `guard`.
fn plain(branch: &str) -> [Msg; 3] {
    [
        Msg::confirm_danger(
            T.tr("branch.delete.title"),
            T.trf("branch.delete.body", &[("branch", &branch)]),
            T.tr("common.delete.button"),
            Msg::git("Delete branch", &["branch", "-d", branch]),
        ),
        Msg::confirm_danger(
            T.tr("branch.force_delete.title"),
            T.trf("branch.force_delete.body", &[("branch", &branch)]),
            T.tr("common.delete.button"),
            Msg::git("Delete branch", &["branch", "-D", branch]),
        ),
        Msg::confirm_danger(
            T.tr("branch.delete_both.title"),
            format!("Delete {branch} and origin/{branch}?"),
            T.tr("branch.delete_both.button"),
            Msg::git("Delete branch", &["branch", "-d", branch]),
        ),
    ]
}

/// `plain`, through `guard` as the menu does it.
fn menu(st: &AppState, branch: &str) -> [Msg; 3] {
    let snap = st.snap.as_ref().unwrap();
    let [delete, force, both] = plain(branch);
    [
        guard(T, snap, branch, Deleting::Local, delete),
        guard(T, snap, branch, Deleting::Local, force),
        guard(T, snap, branch, Deleting::LocalAndRemote("origin"), both),
    ]
}

/// Title, body, button, the acknowledgement and the command the confirmation runs.
type Parts = (String, String, String, Option<String>, Vec<Vec<String>>);

fn parts(m: &Msg) -> Parts {
    let Msg::ConfirmDanger { title, body, button, ack, then } = m else { panic!("a danger confirmation") };
    let Msg::Job(JobMsg::Git { cmds, .. }) = then.as_ref() else { panic!("a git job") };
    (title.clone(), body.clone(), button.clone(), ack.clone(), cmds.clone())
}

fn all(ms: &[Msg; 3]) -> Vec<Parts> {
    ms.iter().map(parts).collect()
}

#[test]
fn confirm_delete_for_bases() {
    let (mut st, dir) = initialized("base-confirm");
    let [delete, force, both]: [Parts; 3] = all(&menu(&st, "develop")).try_into().unwrap();
    let question = T.trf("branch.delete.body", &[("branch", &"develop")]);
    assert_eq!(
        delete,
        (
            "Delete development branch".into(),
            format!("{question}\n\n{DEVELOP}"),
            "Delete anyway".into(),
            None,
            vec![args(&["branch", "-d", "develop"])]
        )
    );
    let force_q = T.trf("branch.force_delete.body", &[("branch", &"develop")]);
    assert_eq!((force.1.as_str(), force.2.as_str()), (format!("{force_q}\n\n{DEVELOP}").as_str(), "Delete anyway"));
    assert_eq!(force.4, vec![args(&["branch", "-D", "develop"])]);
    assert_eq!(both.0, "Delete development branch");
    assert_eq!(both.1, format!("Delete develop and origin/develop?\n\n{DEVELOP} {REMOTE}"));
    assert_eq!((both.2.as_str(), both.3.as_ref()), ("Delete both anyway", None));

    let [delete, ..]: [Parts; 3] = all(&menu(&st, "main")).try_into().unwrap();
    assert_eq!(delete.0, "Delete production branch");
    assert!(delete.1.ends_with(&format!("\n\n{MAIN}")), "{}", delete.1);

    git(&dir, &["branch", "feature/x"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(all(&menu(&st, "feature/x")), all(&plain("feature/x")));
    assert_eq!(all(&menu(&st, "Develop")), all(&plain("Develop")), "exact names only");
    cleanup(&dir);

    let (st, dir) = open("base-off");
    assert_eq!(all(&menu(&st, "develop")), all(&plain("develop")), "Git Flow isn't set up");
    cleanup(&dir);
}

#[test]
fn delete_base_runs_or_cancels() {
    let (mut st, dir) = initialized("base-run");
    let [delete, ..] = menu(&st, "develop");
    run(&mut st, delete);
    assert!(matches!(&st.modal, Some(Modal::ConfirmDanger { title, .. }) if title == "Delete development branch"));
    run(&mut st, Msg::CloseModal);
    assert!(st.modal.is_none() && st.job.is_none());

    let [delete, ..] = menu(&st, "develop");
    run(&mut st, delete);
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert!(st.modal.is_none());
    assert_eq!(git_job(&fx), Some(vec![args(&["branch", "-d", "develop"])]));
    assert_eq!(job_label(&st).as_deref(), Some("Delete branch"));
    finish(&mut st, fx);
    assert!(!git_run(&dir, &args(&["rev-parse", "--verify", "--quiet", "refs/heads/develop"])).0);
    // Without the dialog open, accepting does nothing.
    assert!(git_job(&run(&mut st, Msg::ConfirmAccepted)).is_none());
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
    let root = trellis_core::paths::temp_dir().join(format!("trellis-test-base-worktrees-{}", std::process::id()));
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
