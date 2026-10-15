//! Headless tests for Git Flow: filling the preferences form, the Initialize / Save job and its
//! refusals, refilling on new snapshots, the palette entries, the Start dialog and Start itself,
//! end to end against real repositories.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::msg::PrefsMsg;
use crate::msg::RepoMsg;
use crate::state::gitflow::GitFlowPage;
use crate::state::palette::{PaletteCommand as C, entries};
use crate::state::{AppState, Modal, PrefPage};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::gitcli::{args, run as git_run};
use trellis_core::gitflow::{FlowKind, GitFlowSettings};

pub(super) fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

pub(super) fn flow(st: &mut AppState, m: GitFlowMsg) -> Vec<Effect> {
    run(st, Msg::GitFlow(m))
}

/// A scratch repository (one commit on main) opened in a fresh state.
pub(super) fn open(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(&format!("gitflow-{name}"));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_some());
    (st, dir)
}

/// The same, initialized through the page with the default settings.
pub(super) fn initialized(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = open(name);
    drive(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoGitFlow)));
    drive(&mut st, Msg::GitFlow(GitFlowMsg::Apply));
    assert!(st.snap.as_ref().unwrap().gitflow.is_initialized(), "{:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    drive(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs));
    (st, dir)
}

pub(super) fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn page(st: &AppState) -> &GitFlowPage {
    &st.prefs.as_ref().expect("prefs open").gitflow
}

fn page_mut(st: &mut AppState) -> &mut GitFlowPage {
    &mut st.prefs.as_mut().expect("prefs open").gitflow
}

pub(super) fn git_job(fx: &[Effect]) -> Option<Vec<Vec<String>>> {
    fx.iter().find_map(|e| match e {
        Effect::RunGit { cmds, .. } => Some(cmds.clone()),
        _ => None,
    })
}

pub(super) fn job_label(st: &AppState) -> Option<String> {
    st.job.as_ref().map(|j| j.label.clone())
}

pub(super) fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn defaults() -> GitFlowSettings {
    GitFlowSettings {
        production: "main".into(),
        development: "develop".into(),
        feature: "feature/".into(),
        release: "release/".into(),
        hotfix: "hotfix/".into(),
        version_tag: String::new(),
    }
}

pub(super) fn rev(dir: &Path, r: &str) -> String {
    let (ok, out) = git_run(dir, &args(&["rev-parse", "--verify", r]));
    assert!(ok, "{r}: {out}");
    out.trim().to_owned()
}

#[test]
fn page_fill() {
    let (mut st, dir) = open("fill");
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoGitFlow)));
    let p = page(&st);
    assert_eq!(p.form, defaults());
    assert_eq!(p.filled, p.form);
    assert_eq!(p.workdir.as_deref(), Some(dir.as_path()));
    assert!(!p.edited());

    run(&mut st, Msg::Prefs(PrefsMsg::ClosePrefs));
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)));
    assert_eq!(page(&st).workdir, None);
    flow(&mut st, GitFlowMsg::ShowPage);
    assert_eq!(st.prefs.as_ref().unwrap().page, PrefPage::RepoGitFlow);
    assert_eq!(page(&st).form, defaults());
    cleanup(&dir);
}

#[test]
fn apply_initialize_cmds() {
    let (mut st, dir) = open("apply-cmds");
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoGitFlow)));
    let cmds = git_job(&flow(&mut st, GitFlowMsg::Apply)).expect("a git job");
    assert_eq!(cmds[0], args(&["branch", "--no-track", "develop", "refs/heads/main"]));
    assert_eq!(cmds[1], args(&["config", "--local", "gitflow.branch.master", "main"]));
    assert_eq!(cmds.last().unwrap(), &args(&["config", "--local", "gitflow.prefix.versiontag", ""]));
    assert!(cmds[1..].iter().all(|c| c[0] == "config"));
    assert_eq!(job_label(&st).as_deref(), Some("Initialize Git Flow"));
    cleanup(&dir);
}

#[test]
fn apply_refused() {
    let (mut st, dir) = open("apply-refused");
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoGitFlow)));
    page_mut(&mut st).form.feature.clear();
    assert!(git_job(&flow(&mut st, GitFlowMsg::Apply)).is_none());
    assert_eq!(page(&st).error.as_deref(), Some("Enter a feature prefix."));
    assert!(st.job.is_none());

    flow(&mut st, GitFlowMsg::Discard);
    assert_eq!(page(&st).form, page(&st).filled);
    assert_eq!(page(&st).error, None);

    page_mut(&mut st).form.production = "nope".into();
    assert!(git_job(&flow(&mut st, GitFlowMsg::Apply)).is_none());
    assert_eq!(page(&st).error.as_deref(), Some("nope doesn't exist."));
    flow(&mut st, GitFlowMsg::Discard);
    assert_eq!(page(&st).error, None);
    assert_eq!(page(&st).form.production, "main");
    cleanup(&dir);
}

#[test]
fn apply_end_to_end() {
    let (mut st, dir) = open("apply-e2e");
    drive(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoGitFlow)));
    drive(&mut st, Msg::GitFlow(GitFlowMsg::Apply));
    assert_eq!(rev(&dir, "develop"), rev(&dir, "main"));
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("main"));
    let cfg = st.snap.as_ref().unwrap().gitflow.clone();
    assert!(cfg.is_initialized());
    assert_eq!(page(&st).base, cfg);
    assert_eq!(page(&st).form, defaults());
    assert!(!page(&st).edited());
    assert!(st.toasts.iter().any(|t| t.text == "Initialize Git Flow succeeded"));

    page_mut(&mut st).form.feature = "feat/".into();
    assert!(page(&st).prefixes_changed());
    let fx = flow(&mut st, GitFlowMsg::Apply);
    assert_eq!(job_label(&st).as_deref(), Some("Save Git Flow settings"));
    // Saving writes config only: no branch to create, and bugfix/support already exist.
    let cmds = git_job(&fx).unwrap();
    assert!(cmds.iter().all(|c| c[0] == "config") && cmds.len() == 6, "{cmds:?}");
    st.job = None;
    drive(&mut st, Msg::GitFlow(GitFlowMsg::Apply));
    let (_, out) = git_run(&dir, &args(&["config", "--get", "gitflow.prefix.feature"]));
    assert_eq!(out.trim(), "feat/");
    // The saved values fill the form again, so it no longer counts as edited.
    assert_eq!(page(&st).form.feature, "feat/");
    assert!(!page(&st).edited());
    cleanup(&dir);
}

#[test]
fn snapshot_changed_refills() {
    let (mut st, dir) = open("refill");
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoGitFlow)));
    page_mut(&mut st).form.feature = "x/".into();
    git(&dir, &["config", "gitflow.prefix.release", "rel/"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(page(&st).form.feature, "x/");
    assert_eq!(page(&st).base.release.as_deref(), Some("rel/"));

    flow(&mut st, GitFlowMsg::Discard);
    git(&dir, &["config", "gitflow.prefix.hotfix", "fix/"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(page(&st).form.hotfix, "fix/");
    assert_eq!(page(&st).form.release, "rel/");

    // A snapshot from another repository always refills.
    let (other, other_dir) = open("refill-other");
    page_mut(&mut st).form.feature = "y/".into();
    st.snap = other.snap;
    super::gitflow::snapshot_changed(&mut st);
    assert_eq!(page(&st).workdir.as_deref(), Some(other_dir.as_path()));
    assert_eq!(page(&st).form, defaults());
    cleanup(&dir);
    cleanup(&other_dir);
}

#[test]
fn palette_entries() {
    let (mut st, dir) = open("palette");
    let all = entries(&st);
    assert!(all.iter().any(|e| e.command == C::ConfigureGitFlow && e.label == "Configure Git Flow…"));
    assert!(!all.iter().any(|e| matches!(e.command, C::StartGitFlow(_))));
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::ConfigureGitFlow)));
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::RepoGitFlow));
    assert_eq!(page(&st).form, defaults());
    cleanup(&dir);

    let (mut st, dir) = initialized("palette-init");
    let starts: Vec<(C, Option<String>)> =
        entries(&st).into_iter().filter(|e| matches!(e.command, C::StartGitFlow(_))).map(|e| (e.command, e.detail)).collect();
    let from = |b: &str| Some(format!("from {b}"));
    assert_eq!(
        starts,
        [
            (C::StartGitFlow(FlowKind::Feature), from("develop")),
            (C::StartGitFlow(FlowKind::Release), from("develop")),
            (C::StartGitFlow(FlowKind::Hotfix), from("main")),
        ]
    );
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::StartGitFlow(FlowKind::Feature))));
    assert!(matches!(&st.modal, Some(Modal::GitFlowStart(f)) if f.kind == FlowKind::Feature && f.name.is_empty()));
    cleanup(&dir);
}

#[test]
fn start_dialog() {
    let (mut st, dir) = open("dialog-off");
    flow(&mut st, GitFlowMsg::StartDialog(FlowKind::Feature));
    assert!(st.modal.is_none());
    assert!(last_log(&st).contains("Preferences > Git Flow"));
    cleanup(&dir);

    let (mut st, dir) = initialized("dialog");
    let snap = st.snap.as_mut().unwrap();
    let oid = snap.head_oid.unwrap();
    snap.tags = vec![("v0.1.0".into(), oid)];
    snap.gitflow.version_tag = Some("v".into());
    let opened = |st: &mut AppState, kind| {
        st.modal = None;
        flow(st, GitFlowMsg::StartDialog(kind));
        match &st.modal {
            Some(Modal::GitFlowStart(f)) => Some(f.name.clone()),
            _ => None,
        }
    };
    assert_eq!(opened(&mut st, FlowKind::Release).as_deref(), Some("0.2.0"));
    assert_eq!(opened(&mut st, FlowKind::Hotfix).as_deref(), Some("0.1.1"));
    assert_eq!(opened(&mut st, FlowKind::Feature).as_deref(), Some(""));

    st.snap.as_mut().unwrap().locals.retain(|b| b.name != "develop");
    assert_eq!(opened(&mut st, FlowKind::Feature), None);
    assert_eq!(last_log(&st), "develop doesn't exist. Save the Git Flow settings to create it.");
    assert!(opened(&mut st, FlowKind::Hotfix).is_some());
    st.snap.as_mut().unwrap().locals.retain(|b| b.name != "main");
    assert_eq!(opened(&mut st, FlowKind::Hotfix), None);
    assert_eq!(last_log(&st), "main is needed to start a hotfix, and it doesn't exist.");
    cleanup(&dir);
}

#[test]
fn start() {
    let (mut st, dir) = initialized("start");
    git(&dir, &["branch", "feature/taken"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let start = |kind, name: &str| Msg::GitFlow(GitFlowMsg::Start { kind, name: name.into() });

    flow(&mut st, GitFlowMsg::StartDialog(FlowKind::Feature));
    for (name, why) in [("taken", "A branch named feature/taken already exists."), ("bad name", "Not a valid branch name.")] {
        assert!(git_job(&run(&mut st, start(FlowKind::Feature, name))).is_none());
        assert_eq!(last_log(&st), why);
        assert!(matches!(st.modal, Some(Modal::GitFlowStart(_))));
    }

    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoGitFlow)));
    let fx = run(&mut st, start(FlowKind::Feature, " login "));
    assert_eq!(git_job(&fx), Some(vec![args(&["checkout", "-b", "feature/login", "refs/heads/develop"])]));
    assert_eq!(job_label(&st).as_deref(), Some("Start feature/login"));
    assert!(st.modal.is_none());
    assert!(st.prefs.is_none());

    flow(&mut st, GitFlowMsg::StartDialog(FlowKind::Feature));
    assert!(git_job(&run(&mut st, start(FlowKind::Feature, "other"))).is_none());
    assert_eq!(last_log(&st), "Another operation is still running.");
    assert!(matches!(st.modal, Some(Modal::GitFlowStart(_))), "the dialog stays open");
    cleanup(&dir);
}

#[test]
fn start_end_to_end() {
    let (mut st, dir) = initialized("start-e2e");
    drive(&mut st, Msg::GitFlow(GitFlowMsg::Start { kind: FlowKind::Feature, name: "login".into() }));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.head_branch.as_deref(), Some("feature/login"));
    assert_eq!(rev(&dir, "feature/login"), rev(&dir, "develop"));
    cleanup(&dir);
}
