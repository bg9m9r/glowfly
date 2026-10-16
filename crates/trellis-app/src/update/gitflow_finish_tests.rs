//! Headless tests for Git Flow Finish: opening the dialog and its refusals, the checks before
//! the job starts, the job's commands, the headline when it ends (finished, or stopped by a
//! conflict), and the palette entries, end to end against real repositories.

use super::gitflow_finish::stopped;
use super::gitflow_tests::{cleanup, flow, git_job, initialized, job_label, last_log, open, rev, run};
use super::tests::{drive, git};
use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::msg::PrefsMsg;
use crate::msg::RepoMsg;
use crate::state::gitflow::NOT_SET_UP;
use crate::state::palette::{PaletteCommand as C, entries};
use crate::state::{AppState, Modal, PrefPage};
use git2::RepositoryState;
use std::path::{Path, PathBuf};
use trellis_core::gitcli::{args, run as git_run};
use trellis_core::gitflow::{FinishOptions, FlowKind};

/// Set up, with a clean working tree, and HEAD on feature/login (one commit ahead of develop).
fn ready(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = initialized(&format!("finish-{name}"));
    git(&dir, &["config", "tag.gpgsign", "false"]);
    git(&dir, &["checkout", "--", "a.txt"]);
    git(&dir, &["checkout", "-q", "-b", "feature/login", "develop"]);
    commit(&dir, "login.txt", "login\n");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, dir)
}

fn commit(dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", &format!("change {file}")]);
}

fn finish_msg(branch: &str, options: FinishOptions) -> Msg {
    Msg::GitFlow(GitFlowMsg::Finish { branch: branch.into(), options })
}

fn release_opts(tag: &str, message: &str, keep: bool) -> FinishOptions {
    FinishOptions { tag: tag.into(), message: message.into(), keep }
}

fn finish_open(st: &AppState) -> bool {
    matches!(st.modal, Some(Modal::GitFlowFinish(_)))
}

fn exists(dir: &Path, r: &str) -> bool {
    git_run(dir, &args(&["rev-parse", "--verify", "--quiet", r])).0
}

#[test]
fn dialog_refusals() {
    let (mut st, dir) = open("finish-off");
    git(&dir, &["branch", "feature/x"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    flow(&mut st, GitFlowMsg::FinishDialog("feature/x".into()));
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), NOT_SET_UP);
    cleanup(&dir);

    let (mut st, dir) = initialized("finish-dialog");
    flow(&mut st, GitFlowMsg::FinishDialog("main".into()));
    assert_eq!(last_log(&st), "main isn't a feature, release or hotfix branch.");
    flow(&mut st, GitFlowMsg::FinishDialog("feature/gone".into()));
    assert_eq!(last_log(&st), "feature/gone doesn't exist.");
    assert!(st.modal.is_none());

    git(&dir, &["branch", "release/1.2.0", "develop"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    flow(&mut st, GitFlowMsg::FinishDialog("release/1.2.0".into()));
    let Some(Modal::GitFlowFinish(f)) = &st.modal else { panic!("the dialog opens") };
    assert_eq!((f.kind, f.branch.as_str()), (FlowKind::Release, "release/1.2.0"));
    assert_eq!((f.tag.as_str(), f.message.as_str(), f.keep, f.focus_tag), ("1.2.0", "Release 1.2.0", false, true));
    cleanup(&dir);
}

#[test]
fn finish_refusals() {
    let (mut st, dir) = ready("refusals");
    let refused = |st: &mut AppState, msg: Msg, why: &str| {
        flow(st, GitFlowMsg::FinishDialog("feature/login".into()));
        assert!(git_job(&run(st, msg)).is_none(), "{why}");
        assert_eq!(last_log(st), why);
        assert!(finish_open(st), "the dialog stays open");
        assert!(st.job.is_none() && st.gitflow_finish.is_none());
    };
    let feature = || finish_msg("feature/login", FinishOptions::default());

    std::fs::write(dir.join("a.txt"), "changed\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    refused(&mut st, feature(), "Commit or stash your changes before finishing feature/login.");
    git(&dir, &["checkout", "--", "a.txt"]);

    st.snap.as_mut().unwrap().state = RepositoryState::Merge;
    refused(&mut st, feature(), "Finish or abort the merge in progress first.");

    git(&dir, &["branch", "release/1.2.0", "develop"]);
    git(&dir, &["tag", "1.2.0", "develop"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    refused(&mut st, finish_msg("release/1.2.0", release_opts("1.2.0", "Release 1.2.0", false)), "The tag 1.2.0 already exists.");
    refused(&mut st, finish_msg("release/1.2.0", release_opts("1.2.1", "  ", false)), "Enter a tag message.");

    st.snap.as_mut().unwrap().locals.retain(|b| b.name != "develop");
    refused(&mut st, feature(), "Finishing a feature needs develop, and it doesn't exist.");

    // Untracked files don't block.
    std::fs::write(dir.join("notes.txt"), "untracked\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(git_job(&run(&mut st, feature())).is_some());
    assert_eq!(job_label(&st).as_deref(), Some("Finish feature login"));

    // Another job is still running.
    st.gitflow_finish = None;
    flow(&mut st, GitFlowMsg::FinishDialog("feature/login".into()));
    assert!(git_job(&run(&mut st, feature())).is_none());
    assert_eq!(last_log(&st), "Another operation is still running.");
    assert!(finish_open(&st));
    assert!(st.gitflow_finish.is_none());
    cleanup(&dir);
}

#[test]
fn finish_cmds() {
    let (mut st, dir) = ready("cmds");
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoGitFlow)));
    flow(&mut st, GitFlowMsg::FinishDialog("feature/login".into()));
    let fx = run(&mut st, finish_msg("feature/login", FinishOptions::default()));
    assert_eq!(
        git_job(&fx),
        Some(vec![
            args(&["checkout", "develop", "--"]),
            args(&["merge", "--no-ff", "--no-edit", "refs/heads/feature/login"]),
            args(&["branch", "-d", "feature/login"]),
        ])
    );
    assert_eq!(job_label(&st).as_deref(), Some("Finish feature login"));
    assert!(st.modal.is_none());
    assert!(st.prefs.is_none());
    let pending = st.gitflow_finish.as_ref().expect("pending finish");
    assert_eq!(pending.label, "Finish feature login");
    assert_eq!(pending.steps, ["check out develop", "merge feature/login into develop", "delete feature/login"]);
    assert_eq!(pending.done, "Finished feature login: merged into develop and deleted feature/login.");

    st.job = None;
    st.gitflow_finish = None;
    git(&dir, &["branch", "release/1.2.0", "develop"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let cmds = git_job(&run(&mut st, finish_msg("release/1.2.0", release_opts("1.2.0", "Release 1.2.0", true)))).unwrap();
    assert_eq!(cmds.len(), 5);
    assert!(cmds.iter().all(|c| c[0] != "branch"), "{cmds:?}");
    assert_eq!(cmds[2], args(&["tag", "-a", "1.2.0", "-m", "Release 1.2.0", "refs/heads/main"]));
    assert_eq!(job_label(&st).as_deref(), Some("Finish release 1.2.0"));
    cleanup(&dir);
}

#[test]
fn finish_end_to_end() {
    let (mut st, dir) = ready("e2e");
    drive(&mut st, finish_msg("feature/login", FinishOptions::default()));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.head_branch.as_deref(), Some("develop"));
    assert!(!exists(&dir, "refs/heads/feature/login"));
    assert!(!rev(&dir, "develop^2").is_empty());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Finished feature login: merged into develop and deleted feature/login."));
    assert!(st.gitflow_finish.is_none());
    cleanup(&dir);
}

#[test]
fn finish_conflict_end_to_end() {
    let (mut st, dir) = initialized("finish-conflict");
    git(&dir, &["checkout", "--", "a.txt"]);
    git(&dir, &["checkout", "-q", "develop"]);
    commit(&dir, "a.txt", "one\ndevelop\n");
    git(&dir, &["checkout", "-q", "-b", "feature/login", "main"]);
    commit(&dir, "a.txt", "one\nfeature\n");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, finish_msg("feature/login", FinishOptions::default()));
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::Merge);
    let toast = st.toasts.last().map(|t| t.text.clone()).unwrap_or_default();
    assert_eq!(
        toast,
        "Finish feature login stopped: \"merge feature/login into develop\" has conflicts. Resolve them and commit the merge. \
         Still to do: delete feature/login."
    );
    assert!(st.log.iter().any(|l| !l.ok && l.text == toast), "the headline is logged too");
    assert!(st.gitflow_finish.is_none());
    assert!(exists(&dir, "refs/heads/feature/login"));
    cleanup(&dir);
}

#[test]
fn stopped_headlines() {
    let steps: Vec<String> = [
        "check out main",
        "merge release/1.2.0 into main",
        "tag main as v1.2.0",
        "check out develop",
        "merge release/1.2.0 into develop",
        "delete release/1.2.0",
    ]
    .map(String::from)
    .to_vec();
    let conflict = "$ git checkout main --\nSwitched to branch 'main'\n$ git merge --no-ff --no-edit refs/heads/release/1.2.0\n\
                    CONFLICT (content): Merge conflict in a.txt\nAutomatic merge failed; fix conflicts and then commit the result.";
    assert_eq!(
        stopped("Finish release 1.2.0", &steps, conflict),
        "Finish release 1.2.0 stopped: \"merge release/1.2.0 into main\" has conflicts. Resolve them and commit the merge. \
         Still to do: tag main as v1.2.0, check out develop, merge release/1.2.0 into develop, delete release/1.2.0."
    );

    let feature: Vec<String> = ["check out develop", "merge feature/x into develop", "delete feature/x"].map(String::from).to_vec();
    let not_merged = "$ git checkout develop --\n$ git merge --no-ff --no-edit refs/heads/feature/x\nMerge made.\n$ git branch -d feature/x\n\
                      error: the branch 'feature/x' is not fully merged\nhint: If you are sure you want to delete it, run 'git branch -D feature/x'";
    assert_eq!(
        stopped("Finish feature x", &feature, not_merged),
        "Finish feature x stopped at \"delete feature/x\": error: the branch 'feature/x' is not fully merged."
    );

    assert_eq!(
        stopped("Finish feature x", &feature, "Could not start git."),
        "Finish feature x stopped at \"check out develop\": Could not start git. Still to do: merge feature/x into develop, delete feature/x."
    );
}

#[test]
fn palette_entries() {
    let (mut st, dir) = open("finish-palette-off");
    git(&dir, &["branch", "feature/x"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(!entries(&st).iter().any(|e| matches!(e.command, C::FinishGitFlow(_))));
    cleanup(&dir);

    let (mut st, dir) = ready("palette");
    git(&dir, &["branch", "feature/a", "develop"]);
    git(&dir, &["branch", "release/1.2.0", "develop"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let finishes: Vec<(C, String, Option<String>)> =
        entries(&st).into_iter().filter(|e| matches!(e.command, C::FinishGitFlow(_))).map(|e| (e.command, e.label, e.detail)).collect();
    let into = |t: &str| Some(format!("into {t}"));
    assert_eq!(
        finishes,
        [
            (C::FinishGitFlow("feature/login".into()), "Finish Git Flow feature login…".into(), into("develop")),
            (C::FinishGitFlow("feature/a".into()), "Finish Git Flow feature a…".into(), into("develop")),
            (C::FinishGitFlow("release/1.2.0".into()), "Finish Git Flow release 1.2.0…".into(), into("main and develop")),
        ]
    );
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::FinishGitFlow("release/1.2.0".into()))));
    assert!(matches!(&st.modal, Some(Modal::GitFlowFinish(f)) if f.branch == "release/1.2.0"));
    cleanup(&dir);
}
