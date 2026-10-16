//! The Stash dialog: it opens from the palette and the WIP menus, stashes every change, only the
//! staged changes or chosen files as one git job (held by Repository trust like any other), and
//! refuses a clean tree, conflicts, an empty staged area or an empty selection with a log line.

use super::palette_tests::repo_with_topic;
use super::tests::{drive, git, scratch_repo, state};
use super::trust_held_tests::held_repo;
use super::update;
use crate::effect::Effect;
use crate::msg::{JobMsg, Msg, PaletteMsg, RepoMsg, StashMsg, TrustMsg};
use crate::state::palette::{PaletteCommand as C, shown};
use crate::state::stashes::{CONFLICTED, NO_FILES, NOTHING, NOTHING_STAGED, StashPushForm, wip_files};
use crate::state::{AppState, Modal, RunningJob};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::repo::{FileEntry, Hazard};
use trellis_core::stash_ops::{STAGED_MIN_GIT, StashScope};

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// A repository with a staged change to a.txt, an unstaged change to b.txt and untracked c.txt.
fn wip_repo(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    git(&dir, &["add", "b.txt"]);
    git(&dir, &["commit", "-q", "-m", "second"]);
    git(&dir, &["add", "a.txt"]);
    std::fs::write(dir.join("b.txt"), "b\nedit\n").unwrap();
    std::fs::write(dir.join("c.txt"), "c\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.staged.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["a.txt"]);
    assert_eq!(snap.unstaged.iter().map(|f| (f.path.as_str(), f.status)).collect::<Vec<_>>(), [("b.txt", 'M'), ("c.txt", '?')]);
    (st, dir)
}

fn dialog(st: &mut AppState, scope: StashScope) {
    run(st, Msg::Stash(StashMsg::PushDialog { scope, path: None }));
    assert!(matches!(st.modal, Some(Modal::StashPush(_))), "the dialog opened");
}

fn form(st: &AppState) -> &StashPushForm {
    match &st.modal {
        Some(Modal::StashPush(f)) => f,
        _ => panic!("the Stash dialog is not open"),
    }
}

fn push(scope: StashScope, message: &str, paths: &[&str]) -> Msg {
    Msg::Stash(StashMsg::Push { scope, message: message.into(), paths: paths.iter().map(|p| p.to_string()).collect() })
}

fn argv(a: &[&str]) -> Vec<Vec<String>> {
    vec![a.iter().map(|s| s.to_string()).collect()]
}

/// The commands and file of the one RunGit effect in `fx`.
fn git_run(fx: &[Effect]) -> (Vec<Vec<String>>, Option<(PathBuf, String)>) {
    match fx {
        [Effect::RunGit { cmds, write_file, .. }] => (cmds.clone(), write_file.clone()),
        other => panic!("expected one RunGit, got {} effects", other.len()),
    }
}

fn last_log(st: &AppState) -> String {
    st.log.last().expect("a log line").text.clone()
}

fn job_label(st: &AppState) -> Option<&str> {
    st.job.as_ref().map(|j| j.label.as_str())
}

fn job(label: &str) -> RunningJob {
    RunningJob {
        label: label.into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
    }
}

fn new_git(dir: &std::path::Path) -> bool {
    let v = trellis_core::gitcli::version(dir).unwrap_or((0, 0, 0));
    (v.0, v.1) >= STAGED_MIN_GIT
}

#[test]
fn the_dialog_opens_from_each_entry_point() {
    let (mut st, dir) = wip_repo("stash-push-open");
    dialog(&mut st, StashScope::All);
    let f = form(&st);
    assert!(f.scope == StashScope::All && f.chosen.is_empty() && f.focus_message && f.message.is_empty());
    run(&mut st, Msg::Stash(StashMsg::PushDialog { scope: StashScope::Files, path: Some("b.txt".into()) }));
    assert_eq!(form(&st).scope, StashScope::Files);
    assert_eq!(form(&st).chosen.iter().collect::<Vec<_>>(), ["b.txt"]);
    dialog(&mut st, StashScope::Staged);
    assert_eq!(form(&st).scope, StashScope::Staged);
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir) = repo_with_topic("stash-push-open-clean");
    git(&dir, &["checkout", "--", "a.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    run(&mut st, Msg::Stash(StashMsg::PushDialog { scope: StashScope::All, path: None }));
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), NOTHING);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stashing_everything_runs_push_u() {
    let (mut st, dir) = wip_repo("stash-push-all");
    dialog(&mut st, StashScope::All);
    let fx = run(&mut st, push(StashScope::All, "  wip \n note ", &[]));
    assert_eq!(git_run(&fx).0, argv(&["stash", "push", "-u", "-m", "wip note"]));
    assert_eq!(job_label(&st), Some("Stash"));
    assert!(st.modal.is_none(), "closed once the job started");

    st.job = None;
    dialog(&mut st, StashScope::All);
    let fx = run(&mut st, push(StashScope::All, "   ", &[]));
    assert_eq!(git_run(&fx).0, argv(&["stash", "push", "-u"]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stashing_staged_changes_runs_push_staged() {
    let (mut st, dir) = wip_repo("stash-push-staged");
    let fx = run(&mut st, push(StashScope::Staged, "keep", &[]));
    assert_eq!(git_run(&fx).0, argv(&["stash", "push", "--staged", "-m", "keep"]));
    assert_eq!(job_label(&st), Some("Stash staged changes"));
    st.job = None;
    let fx = run(&mut st, push(StashScope::Staged, "", &[]));
    assert_eq!(git_run(&fx).0, argv(&["stash", "push", "--staged"]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_with_nothing_staged_is_refused() {
    let (mut st, dir) = repo_with_topic("stash-push-nothing-staged");
    dialog(&mut st, StashScope::Staged);
    assert!(run(&mut st, push(StashScope::Staged, "m", &[])).is_empty());
    assert_eq!(last_log(&st), NOTHING_STAGED);
    assert!(matches!(st.modal, Some(Modal::StashPush(_))), "the dialog stays open");
    assert!(st.job.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stashing_files_runs_a_literal_pathspec_push() {
    let (mut st, dir) = wip_repo("stash-push-files");
    let fx = run(&mut st, push(StashScope::Files, "m", &["b.txt", "c.txt"]));
    let (cmds, file) = git_run(&fx);
    assert_eq!(cmds, argv(&["--literal-pathspecs", "stash", "push", "-u", "-m", "m", "--", "b.txt", "c.txt"]));
    assert_eq!(file, None);
    assert_eq!(job_label(&st), Some("Stash selected files"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unchanged_paths_are_dropped_and_none_left_is_refused() {
    let (mut st, dir) = wip_repo("stash-push-dropped");
    let fx = run(&mut st, push(StashScope::Files, "", &["b.txt", "gone.txt"]));
    assert_eq!(git_run(&fx).0, argv(&["--literal-pathspecs", "stash", "push", "-u", "--", "b.txt"]));
    st.job = None;
    dialog(&mut st, StashScope::Files);
    for paths in [&["gone.txt"][..], &[]] {
        assert!(run(&mut st, push(StashScope::Files, "", paths)).is_empty());
        assert_eq!(last_log(&st), NO_FILES);
        assert!(matches!(st.modal, Some(Modal::StashPush(_))));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn paths_trellis_cannot_name_safely_are_never_stashed() {
    let (mut st, dir) = wip_repo("stash-push-hazards");
    let snap = st.snap.as_mut().unwrap();
    let odd = Hazard::NotUtf8(b"caf\xe9.txt".to_vec());
    snap.unstaged.push(FileEntry { hazard: Some(odd.clone()), ..FileEntry::new("caf\\xE9.txt", '?') });
    let twin = Hazard::Twins(vec!["A.txt".into()]);
    snap.staged[0].hazard = Some(twin.clone());
    // The staged row's hazard survives the merge with an unstaged row for the same path.
    snap.unstaged.push(FileEntry::new("a.txt", 'M'));
    let listed = wip_files(st.snap.as_ref().unwrap());
    let hazard = |p: &str| listed.iter().find(|f| f.path == p).and_then(|f| f.hazard.clone());
    assert_eq!((hazard("a.txt"), hazard("caf\\xE9.txt"), hazard("b.txt")), (Some(twin.clone()), Some(odd.clone()), None));

    let fx = run(&mut st, push(StashScope::Files, "", &["a.txt", "b.txt", "caf\\xE9.txt"]));
    assert_eq!(git_run(&fx).0, argv(&["--literal-pathspecs", "stash", "push", "-u", "--", "b.txt"]));
    st.job = None;
    dialog(&mut st, StashScope::Files);
    assert!(run(&mut st, push(StashScope::Files, "", &["a.txt", "caf\\xE9.txt"])).is_empty());
    assert_eq!(last_log(&st), NO_FILES);

    st.modal = None;
    for (path, h) in [("a.txt", twin), ("caf\\xE9.txt", odd)] {
        let fx = run(&mut st, Msg::Stash(StashMsg::PushDialog { scope: StashScope::Files, path: Some(path.into()) }));
        assert!(fx.is_empty() && st.modal.is_none(), "{path}: the dialog stays shut");
        assert_eq!(st.toasts.last().map(|t| t.text.clone()), Some(h.reason()));
    }
    let mut form = StashPushForm::new(StashScope::Files, None);
    form.chosen.extend(["a.txt", "b.txt", "caf\\xE9.txt"].map(String::from));
    assert_eq!(form.paths(st.snap.as_ref().unwrap()), ["b.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn conflicts_and_a_clean_tree_are_refused() {
    let (mut st, dir) = wip_repo("stash-push-refused");
    st.snap.as_mut().unwrap().unstaged.push(FileEntry::new("d.txt", '!'));
    dialog(&mut st, StashScope::All);
    assert!(run(&mut st, push(StashScope::All, "", &[])).is_empty());
    assert_eq!(last_log(&st), CONFLICTED);
    assert!(matches!(st.modal, Some(Modal::StashPush(_))));

    let snap = st.snap.as_mut().unwrap();
    snap.unstaged.clear();
    snap.staged.clear();
    assert!(run(&mut st, push(StashScope::All, "", &[])).is_empty());
    assert_eq!(last_log(&st), NOTHING);
    assert!(st.modal.is_none(), "nothing to stash closes the dialog");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_busy_job_keeps_the_dialog() {
    let (mut st, dir) = wip_repo("stash-push-busy");
    dialog(&mut st, StashScope::All);
    st.job = Some(job("Fetch all"));
    assert!(run(&mut st, push(StashScope::All, "", &[])).is_empty());
    assert!(matches!(st.modal, Some(Modal::StashPush(_))));
    assert_eq!(job_label(&st), Some("Fetch all"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_long_selection_writes_the_pathspec_file() {
    let (mut st, dir) = wip_repo("stash-push-long");
    let paths: Vec<String> = (0..200).map(|i| format!("dir/{i:03}-{}.txt", "x".repeat(60))).collect();
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged.extend(paths.iter().map(|p| FileEntry::new(p.clone(), '?')));
    let git_dir = snap.git_dir.clone();
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    let fx = run(&mut st, push(StashScope::Files, "", &refs));
    let (cmds, file) = git_run(&fx);
    let pathspec = git_dir.join("TRELLIS_STASH_PATHS");
    assert_eq!(file, Some((pathspec.clone(), paths.join("\0"))));
    assert!(cmds[0].contains(&format!("--pathspec-from-file={}", pathspec.display())), "{cmds:?}");
    assert!(cmds[0].contains(&"--pathspec-file-nul".to_owned()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stash_runs_are_held_by_trust() {
    let (mut st, dir) = held_repo("stash-push-held");
    dialog(&mut st, StashScope::All);
    let fx = run(&mut st, push(StashScope::All, "", &[]));
    assert!(!fx.iter().any(|e| matches!(e, Effect::RunGit { .. })), "{fx:?}");
    assert!(matches!(st.modal, Some(Modal::Trust(_))), "the trust prompt replaces the dialog");
    assert_eq!(st.trust.held.as_ref().map(|h| h.job.label.as_str()), Some("Stash"));
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    let fx = run(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }));
    let gits: Vec<_> = fx.iter().filter(|e| matches!(e, Effect::RunGit { .. })).collect();
    assert_eq!(gits.len(), 1, "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_stash_end_to_end() {
    let (mut st, dir) = wip_repo("stash-push-staged-e2e");
    if !new_git(&dir) {
        eprintln!("skipped: git is older than {}.{}", STAGED_MIN_GIT.0, STAGED_MIN_GIT.1);
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    drive(&mut st, push(StashScope::Staged, "only staged", &[]));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.stashes.len(), 1);
    assert!(snap.stashes[0].message.ends_with(": only staged"), "{}", snap.stashes[0].message);
    assert!(snap.staged.is_empty());
    assert_eq!(snap.unstaged.iter().map(|f| (f.path.as_str(), f.status)).collect::<Vec<_>>(), [("b.txt", 'M'), ("c.txt", '?')]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn files_stash_end_to_end() {
    let (mut st, dir) = wip_repo("stash-push-files-e2e");
    drive(&mut st, push(StashScope::Files, "", &["c.txt"]));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.stashes.len(), 1);
    assert!(!snap.unstaged.iter().any(|f| f.path == "c.txt"));
    assert!(snap.unstaged.iter().any(|f| f.path == "b.txt"));
    assert!(snap.staged.iter().any(|f| f.path == "a.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn too_old_git_explains_itself() {
    let (mut st, dir) = wip_repo("stash-push-old-git");
    let output = "$ git stash push --staged\nerror: unknown option `staged'\nusage: git stash list [<options>]";
    let done = || Msg::Job(JobMsg::JobDone { ok: false, output: output.into(), undo: None });
    st.job = Some(job("Stash staged changes"));
    run(&mut st, done());
    assert!(last_log(&st).contains("needs git 2.35 or newer"), "{}", last_log(&st));
    st.job = Some(job("Stash"));
    run(&mut st, done());
    assert!(!last_log(&st).contains("needs git 2.35 or newer"), "{}", last_log(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn palette_offers_the_stash_dialog() {
    let (mut st, dir) = wip_repo("stash-push-palette");
    let entries = shown(&st, "");
    let at = entries.iter().position(|s| s.entry.label == "Stash changes").unwrap();
    let next: Vec<(&str, Option<&str>)> =
        entries[at + 1..at + 4].iter().map(|s| (s.entry.label.as_str(), s.entry.detail.as_deref())).collect();
    assert_eq!(
        next,
        [("Stash with message…", None), ("Stash staged changes…", Some("1 staged")), ("Stash selected files…", Some("3 files"))]
    );

    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::StashDialog(StashScope::Staged))));
    assert!(st.palette.is_none());
    assert_eq!(form(&st).scope, StashScope::Staged);
    assert_eq!(st.palette_recent.keys(), ["stash-staged"]);
    let _ = std::fs::remove_dir_all(&dir);

    let (mut st, dir) = repo_with_topic("stash-push-palette-clean");
    git(&dir, &["checkout", "--", "a.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let labels: Vec<String> = shown(&st, "").into_iter().map(|s| s.entry.label).collect();
    assert!(!labels.iter().any(|l| l == "Stash staged changes…" || l == "Stash selected files…"), "{labels:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
