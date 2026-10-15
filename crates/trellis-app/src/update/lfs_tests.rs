//! Headless tests for the Git LFS page: reading the status, the job each action starts, the
//! push remote, the prune confirmation, what follows a finished Initialize, the palette entries,
//! and an end-to-end setup against a real repository (skipped without git-lfs).

use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::lfs::LfsMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::palette::{PaletteCommand as C, entries};
use crate::state::{AppState, Modal, PrefPage, RunningJob};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::gitcli::args;
use trellis_core::lfs::{LfsCommand, LfsPattern, LfsStatus};
use trellis_core::repo::Tracking;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn lfs(st: &mut AppState, m: LfsMsg) -> Vec<Effect> {
    run(st, Msg::Lfs(m))
}

/// A scratch repository opened in a fresh state.
fn open(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_some());
    (st, dir)
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn pattern(raw: &str) -> LfsPattern {
    LfsPattern { raw: raw.into(), display: trellis_core::lfs::unescape_pattern(raw), lockable: false }
}

fn status(installed: bool, initialized: bool, patterns: &[&str]) -> LfsStatus {
    LfsStatus {
        version: installed.then(|| "3.7.1".into()),
        initialized,
        patterns: patterns.iter().map(|p| pattern(p)).collect(),
        has_gitattributes: !patterns.is_empty(),
        foreign_hooks: vec![],
    }
}

fn lfs_gen(st: &AppState) -> u64 {
    st.prefs.as_ref().unwrap().lfs.generation
}

/// Opens the page and delivers `s` as the status.
fn page_with(st: &mut AppState, s: LfsStatus) -> Vec<Effect> {
    run(st, Msg::OpenPrefs(PrefPage::RepoLfs));
    let generation = lfs_gen(st);
    lfs(st, LfsMsg::Loaded { generation, result: Ok(s) })
}

fn git_cmds(fx: &[Effect]) -> Option<Vec<Vec<String>>> {
    fx.iter().find_map(|e| match e {
        Effect::RunGit { cmds, .. } => Some(cmds.clone()),
        _ => None,
    })
}

fn loads(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::LoadLfs { .. }))
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn add_remote(st: &mut AppState, name: &str) {
    st.snap.as_mut().unwrap().remotes.push((name.into(), vec![]));
}

#[test]
fn opening_the_page_reads_the_status() {
    let (mut st, dir) = open("lfs-open");
    let fx = run(&mut st, Msg::OpenPrefs(PrefPage::RepoLfs));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadLfs { workdir, generation: 1 } if *workdir == dir)));
    assert!(st.prefs.as_ref().unwrap().lfs.status.is_none());
    cleanup(&dir);

    let mut st = state();
    assert!(!loads(&run(&mut st, Msg::OpenPrefs(PrefPage::RepoLfs))));
}

#[test]
fn show_page_and_loaded_results() {
    let (mut st, dir) = open("lfs-show");
    run(&mut st, Msg::OpenPrefs(PrefPage::General));
    let fx = lfs(&mut st, LfsMsg::ShowPage);
    assert_eq!(st.prefs.as_ref().unwrap().page, PrefPage::RepoLfs);
    assert!(loads(&fx));
    let current = lfs_gen(&st);
    lfs(&mut st, LfsMsg::Loaded { generation: current - 1, result: Ok(status(true, true, &[])) });
    assert!(st.prefs.as_ref().unwrap().lfs.status.is_none(), "stale results are ignored");
    lfs(&mut st, LfsMsg::Loaded { generation: current, result: Ok(status(true, true, &["*.a"])) });
    assert_eq!(st.prefs.as_ref().unwrap().lfs.status, Some(status(true, true, &["*.a"])));
    lfs(&mut st, LfsMsg::Reload);
    let current = lfs_gen(&st);
    lfs(&mut st, LfsMsg::Loaded { generation: current, result: Err("boom".into()) });
    let page = &st.prefs.as_ref().unwrap().lfs;
    assert_eq!((page.status.is_none(), page.error.as_deref()), (true, Some("boom")));

    run(&mut st, Msg::ClosePrefs);
    lfs(&mut st, LfsMsg::Loaded { generation: current, result: Ok(status(true, true, &[])) });
    assert!(st.prefs.is_none());
    cleanup(&dir);
}

#[test]
fn scoped_result_for_another_tab_is_dropped() {
    let (mut st, dir) = open("lfs-scoped");
    run(&mut st, Msg::OpenPrefs(PrefPage::RepoLfs));
    let msg = Msg::Lfs(LfsMsg::Loaded { generation: 1, result: Ok(status(true, true, &[])) });
    run(&mut st, Msg::Scoped { workdir: dir.join("elsewhere"), msg: Box::new(msg) });
    assert!(st.prefs.as_ref().unwrap().lfs.status.is_none());
    cleanup(&dir);
}

#[test]
fn track_checks_the_pattern_first() {
    let (mut st, dir) = open("lfs-track");
    page_with(&mut st, status(true, true, &["my[[:space:]]dir/*.bin"]));
    assert!(lfs(&mut st, LfsMsg::Track("  ".into())).is_empty());
    assert_eq!(st.prefs.as_ref().unwrap().lfs.pattern_error.as_deref(), Some("Enter a pattern."));

    lfs(&mut st, LfsMsg::Track("my dir/*.bin".into()));
    assert_eq!(st.prefs.as_ref().unwrap().lfs.pattern_error.as_deref(), Some("my dir/*.bin is already tracked."));

    st.prefs.as_mut().unwrap().lfs.new_pattern = " *.psd ".into();
    let fx = lfs(&mut st, LfsMsg::Track(" *.psd ".into()));
    assert_eq!(git_cmds(&fx), Some(vec![args(&["lfs", "track", "--", "*.psd"])]));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Track *.psd"));
    let page = &st.prefs.as_ref().unwrap().lfs;
    assert!(page.new_pattern.is_empty() && page.pattern_error.is_none());
    cleanup(&dir);
}

#[test]
fn track_and_untrack_need_git_lfs() {
    let (mut st, dir) = open("lfs-missing");
    page_with(&mut st, status(false, false, &["*.a"]));
    assert!(git_cmds(&lfs(&mut st, LfsMsg::Track("*.b".into()))).is_none());
    assert_eq!(last_log(&st), "Git LFS isn't installed.");
    assert!(git_cmds(&lfs(&mut st, LfsMsg::Untrack("*.a".into()))).is_none());
    assert!(git_cmds(&lfs(&mut st, LfsMsg::Initialize)).is_none());
    assert!(st.job.is_none());
    cleanup(&dir);
}

#[test]
fn untrack_passes_the_typed_pattern() {
    let (mut st, dir) = open("lfs-untrack");
    page_with(&mut st, status(true, true, &["my[[:space:]]dir/*.bin", r"a\#b"]));
    let fx = lfs(&mut st, LfsMsg::Untrack("my[[:space:]]dir/*.bin".into()));
    assert_eq!(git_cmds(&fx), Some(vec![args(&["lfs", "untrack", "--", "my dir/*.bin"])]));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Untrack my dir/*.bin"));
    st.job = None;
    let fx = lfs(&mut st, LfsMsg::Untrack(r"a\#b".into()));
    assert_eq!(git_cmds(&fx), Some(vec![args(&["lfs", "untrack", "--", "a#b"])]));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Untrack a#b"));
    cleanup(&dir);
}

#[test]
fn push_goes_to_the_upstream_remote() {
    let (mut st, dir) = open("lfs-push");
    let push = LfsMsg::Run { command: LfsCommand::Push, confirmed: false };
    assert!(lfs(&mut st, LfsMsg::Run { command: LfsCommand::Push, confirmed: false }).is_empty());
    assert_eq!(last_log(&st), "Add a remote first.");
    assert!(lfs(&mut st, LfsMsg::Run { command: LfsCommand::Fetch, confirmed: false }).is_empty());

    add_remote(&mut st, "backup");
    add_remote(&mut st, "origin");
    assert_eq!(git_cmds(&lfs(&mut st, push)), Some(vec![args(&["lfs", "push", "--all", "origin"])]));
    st.job = None;

    add_remote(&mut st, "upstream");
    let head = st.snap.as_mut().unwrap().locals.iter_mut().find(|b| b.is_head).unwrap();
    head.tracking = Some(Tracking { remote: "upstream".into(), merge: "main".into() });
    let fx = lfs(&mut st, LfsMsg::Run { command: LfsCommand::Push, confirmed: false });
    assert_eq!(git_cmds(&fx), Some(vec![args(&["lfs", "push", "--all", "upstream"])]));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("LFS push"));
    cleanup(&dir);
}

#[test]
fn prune_asks_and_one_job_at_a_time() {
    let (mut st, dir) = open("lfs-prune");
    let fx = lfs(&mut st, LfsMsg::Run { command: LfsCommand::Prune, confirmed: false });
    assert!(fx.is_empty());
    assert!(matches!(&st.modal, Some(Modal::Confirm { title, button, .. }) if title == "Prune LFS files" && button == "Prune"));
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(git_cmds(&fx), Some(vec![args(&["lfs", "prune"])]));

    add_remote(&mut st, "origin");
    assert!(lfs(&mut st, LfsMsg::Run { command: LfsCommand::Fetch, confirmed: false }).is_empty());
    assert_eq!(last_log(&st), "Another operation is still running.");
    cleanup(&dir);
}

fn init_job(st: &mut AppState) {
    st.job = Some(RunningJob {
        label: crate::state::lfs::INIT_LABEL.into(),
        workdir: st.workdir(),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        keeps_undo: true,
    });
}

#[test]
fn initialize_then_offer_to_pull() {
    let (mut st, dir) = open("lfs-offer");
    add_remote(&mut st, "origin");
    page_with(&mut st, status(true, false, &[]));
    init_job(&mut st);
    let fx = run(&mut st, Msg::JobDone { ok: true, output: String::new(), undo: None });
    assert!(loads(&fx));
    assert!(st.prefs.as_ref().unwrap().lfs.offer_pull);
    let generation = lfs_gen(&st);
    lfs(&mut st, LfsMsg::Loaded { generation, result: Ok(status(true, true, &["*.bin"])) });
    let Some(Modal::Confirm { title, then, .. }) = &st.modal else { panic!("expected the pull offer") };
    assert_eq!(title, "Download LFS files?");
    assert!(matches!(**then, Msg::Lfs(LfsMsg::Run { command: LfsCommand::Pull, confirmed: true })));

    // No patterns: nothing to pull.
    st.modal = None;
    init_job(&mut st);
    run(&mut st, Msg::JobDone { ok: true, output: String::new(), undo: None });
    let generation = lfs_gen(&st);
    lfs(&mut st, LfsMsg::Loaded { generation, result: Ok(status(true, true, &[])) });
    assert!(st.modal.is_none());

    // Preferences closed: no reload.
    run(&mut st, Msg::ClosePrefs);
    init_job(&mut st);
    assert!(!loads(&run(&mut st, Msg::JobDone { ok: true, output: String::new(), undo: None })));
    cleanup(&dir);
}

#[test]
fn finished_jobs_keep_the_page_and_auto_fetch_leaves_it_alone() {
    let (mut st, dir) = open("lfs-auto-fetch");
    page_with(&mut st, status(true, true, &["*.a"]));
    st.prefs.as_mut().unwrap().lfs.new_pattern = "*.ps".into();
    let generation = lfs_gen(&st);
    init_job(&mut st);
    st.job.as_mut().unwrap().quiet = true;
    let fx = run(&mut st, Msg::JobDone { ok: true, output: String::new(), undo: None });
    assert!(!loads(&fx), "{fx:?}");
    let page = &st.prefs.as_ref().unwrap().lfs;
    assert_eq!((page.generation, page.status.is_some(), page.new_pattern.as_str()), (generation, true, "*.ps"));
    assert!(!page.offer_pull);

    // Another job reloads it, keeping the status and the typed text on screen meanwhile.
    init_job(&mut st);
    st.job.as_mut().unwrap().label = "LFS pull".into();
    assert!(loads(&run(&mut st, Msg::JobDone { ok: true, output: String::new(), undo: None })));
    let page = &st.prefs.as_ref().unwrap().lfs;
    assert_eq!((page.status.is_some(), page.new_pattern.as_str()), (true, "*.ps"));
    cleanup(&dir);
}

#[test]
fn initialize_when_ready() {
    let (mut st, dir) = open("lfs-when-ready");
    let fx = lfs(&mut st, LfsMsg::InitializeWhenReady);
    assert!(loads(&fx));
    let page = &st.prefs.as_ref().unwrap();
    assert!(page.page == PrefPage::RepoLfs && page.lfs.init_when_ready);
    let generation = lfs_gen(&st);
    let fx = lfs(&mut st, LfsMsg::Loaded { generation, result: Ok(status(true, false, &[])) });
    assert_eq!(git_cmds(&fx), Some(vec![args(&["lfs", "install", "--local"])]));
    st.job = None;

    lfs(&mut st, LfsMsg::InitializeWhenReady);
    let generation = lfs_gen(&st);
    assert!(git_cmds(&lfs(&mut st, LfsMsg::Loaded { generation, result: Ok(status(false, false, &[])) })).is_none());

    lfs(&mut st, LfsMsg::InitializeWhenReady);
    let generation = lfs_gen(&st);
    assert!(git_cmds(&lfs(&mut st, LfsMsg::Loaded { generation, result: Ok(status(true, true, &[])) })).is_none());
    assert!(st.toasts.iter().any(|t| t.text == "Git LFS is already set up in this repository"));
    cleanup(&dir);
}

#[test]
fn lfs_jobs_and_the_undo_history() {
    let keeps = |a: &[&str]| super::undo::keeps_history(&[args(a)]);
    assert!(keeps(&["lfs", "fetch"]));
    assert!(keeps(&["lfs", "push", "--all", "origin"]));
    assert!(keeps(&["lfs", "install", "--local"]));
    assert!(keeps(&["lfs", "prune"]));
    assert!(!keeps(&["lfs", "pull"]));
    assert!(!keeps(&["lfs", "track", "--", "*.x"]));
}

#[test]
fn palette_entries() {
    let (mut st, dir) = open("lfs-palette");
    let labels: Vec<String> = entries(&st).into_iter().map(|e| e.label).collect();
    let at = |l: &str| labels.iter().position(|x| x == l).unwrap_or_else(|| panic!("{l} missing"));
    assert!(at("Initialize Git LFS") < at("Refresh") && at("Configure Git LFS…") < at("Refresh"));
    run(&mut st, Msg::TogglePalette);
    let fx = run(&mut st, Msg::RunPaletteCommand(C::ConfigureLfs));
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::RepoLfs));
    assert!(loads(&fx));
    let empty = state();
    assert!(!entries(&empty).iter().any(|e| e.label.contains("Git LFS")));
    cleanup(&dir);
}

#[test]
fn set_up_and_track_end_to_end() {
    if trellis_core::lfs::installed(&std::env::temp_dir()).is_none() {
        eprintln!("set_up_and_track_end_to_end: git-lfs is not installed, skipping");
        return;
    }
    let (mut st, dir) = open("lfs-e2e");
    drive(&mut st, Msg::OpenPrefs(PrefPage::RepoLfs));
    let s = st.prefs.as_ref().unwrap().lfs.status.clone().expect("status");
    assert!(s.installed() && !s.initialized);
    drive(&mut st, Msg::Lfs(LfsMsg::Initialize));
    assert!(st.prefs.as_ref().unwrap().lfs.status.as_ref().is_some_and(|s| s.initialized));
    drive(&mut st, Msg::Lfs(LfsMsg::Track("*.bin".into())));
    let s = st.prefs.as_ref().unwrap().lfs.status.clone().expect("status");
    assert_eq!(s.patterns.iter().map(|p| p.display.as_str()).collect::<Vec<_>>(), ["*.bin"]);
    assert!(s.has_gitattributes);
    assert!(st.toasts.iter().any(|t| t.text == "Track *.bin succeeded"), "{:?}", st.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());
    cleanup(&dir);
}
