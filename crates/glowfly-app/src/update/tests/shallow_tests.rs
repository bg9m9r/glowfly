//! Headless tests for shallow clones: the palette entries and their commit count step, the
//! fetch jobs (which remote, refusals), and what the reload after a deepen invalidates.

use super::palette_tests::repo_with_topic;
use super::tests::{drive, finish, git, scratch_repo, state};
use super::update;
use crate::effect::partial::PartialEffect;
use crate::effect::{Effect, LoadKind};
use crate::msg::{JobMsg, Msg, PaletteMsg, RepoMsg};
use crate::runner;
use crate::state::AppState;
use crate::state::Selection;
use crate::state::palette::{InputKind, Mode, PaletteCommand as C, entries};
use glowfly_core::clone_ops::DepthProblem;
use glowfly_core::shallow_ops::Deepen;
use std::path::{Path, PathBuf};
use std::time::Instant;

const FULL: &str = "Fetch full history";
const MORE: &str = "Fetch more commits…";

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// A source repository with `commits` commits on main and a depth-`depth` clone of it, opened
/// in a fresh state. Returns (state, source, clone).
pub(super) fn shallow_clone(name: &str, commits: usize, depth: u32) -> (AppState, PathBuf, PathBuf) {
    let src = scratch_repo(name);
    for i in 1..commits {
        std::fs::write(src.join("b.txt"), format!("{i}\n")).unwrap();
        git(&src, &["add", "b.txt"]);
        git(&src, &["commit", "-q", "-m", &format!("c{i}")]);
    }
    let dest = src.with_file_name(format!("{}-shallow", src.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&dest);
    let (s, d) = (src.display().to_string(), dest.display().to_string());
    git(src.parent().unwrap(), &["clone", "-q", &format!("--depth={depth}"), "--no-local", &s, &d]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dest.clone())));
    assert!(st.snap.as_ref().is_some_and(|s| s.is_shallow()), "the clone is shallow");
    (st, src, dest)
}

pub(super) fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// The palette entries with this label, as (label, detail).
fn listed(st: &AppState, label: &str) -> Vec<Option<String>> {
    entries(st).into_iter().filter(|e| e.label == label).map(|e| e.detail).collect()
}

/// The argv lists of the one RunGit effect in `fx`.
fn git_cmds(fx: &[Effect]) -> Vec<Vec<String>> {
    match fx {
        [Effect::RunGit { cmds, .. }] => cmds.clone(),
        other => panic!("expected one RunGit, got {} effects", other.len()),
    }
}

fn argv(a: &[&str]) -> Vec<Vec<String>> {
    vec![a.iter().map(|s| s.to_string()).collect()]
}

fn deepen(how: Deepen) -> Msg {
    Msg::Job(JobMsg::Deepen(how))
}

#[test]
fn entries_only_in_shallow_repos() {
    let (st, src, dest) = shallow_clone("shallow-entries", 3, 1);
    let from = vec![Some("from origin".to_owned())];
    assert_eq!(listed(&st, FULL), from);
    assert_eq!(listed(&st, MORE), from);
    let labels: Vec<String> = entries(&st).into_iter().map(|e| e.label).collect();
    let pull = labels.iter().position(|l| l == "Pull").unwrap();
    assert_eq!(labels[pull + 1..pull + 3], [FULL, MORE], "right after Pull");

    let (full, dir) = repo_with_topic("shallow-entries-full");
    assert!(listed(&full, FULL).is_empty() && listed(&full, MORE).is_empty(), "a full clone");
    cleanup(&[&dir]);

    let mut st = st;
    git(&dest, &["remote", "remove", "origin"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.snap.as_ref().unwrap().is_shallow());
    assert!(listed(&st, FULL).is_empty() && listed(&st, MORE).is_empty(), "no remote to fetch from");
    assert!(!st.can_fetch_history());
    cleanup(&[&src, &dest]);
}

#[test]
fn fetch_full_history_job() {
    let (mut st, src, dest) = shallow_clone("shallow-full", 4, 1);
    assert_eq!(st.snap.as_ref().unwrap().commits.len(), 1);
    // The full history is sized first: the source is a folder on this computer.
    let mut fx = run(&mut st, deepen(Deepen::Full));
    match &fx[..] {
        [Effect::Partial(PartialEffect::SizeHistory(r))] => {
            assert_eq!(r.url.as_deref(), Some(src.display().to_string().as_str()));
            assert!(r.service.is_none(), "no hosting service to ask");
        }
        other => panic!("expected the size check, got {} effects", other.len()),
    }
    assert!(st.partial.sizing.is_some() && st.job.is_none());
    assert!(run(&mut st, deepen(Deepen::Full)).is_empty(), "a second click waits for the first check");
    let sized = runner::execute(fx.pop().unwrap()).expect("the size");
    let fx = run(&mut st, sized);
    assert!(st.partial.sizing.is_none() && st.modal.is_none(), "a small history doesn't ask");
    match &fx[..] {
        [Effect::RunGit { cmds, prompts, .. }] => {
            assert_eq!(cmds, &argv(&["fetch", "--unshallow", "origin"]));
            assert!(prompts, "a job the user started may ask for credentials");
        }
        other => panic!("expected one RunGit, got {} effects", other.len()),
    }
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some(FULL));
    finish(&mut st, fx);
    assert!(st.job.is_none());
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.is_shallow());
    assert_eq!(snap.commits.len(), 4);
    assert!(listed(&st, FULL).is_empty() && listed(&st, MORE).is_empty());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Fetch full history succeeded"));

    // A message sent before that reload landed.
    assert!(run(&mut st, deepen(Deepen::Full)).is_empty());
    assert!(st.log.last().is_some_and(|l| l.text.contains("already has its full history")));
    cleanup(&[&src, &dest]);
}

#[test]
fn deepen_reaches_the_remote_of_the_upstream() {
    let (mut st, src, dest) = shallow_clone("shallow-upstream", 4, 1);
    git(&dest, &["remote", "add", "up", &src.display().to_string()]);
    git(&dest, &["fetch", "-q", "up"]);
    git(&dest, &["branch", "--set-upstream-to=up/main"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(listed(&st, FULL), [Some("from up".to_owned())]);
    let fx = run(&mut st, deepen(Deepen::By(2)));
    assert_eq!(git_cmds(&fx), argv(&["fetch", "--deepen=2", "up"]));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Fetch 2 more commits"));
    st.job = None;

    git(&dest, &["branch", "--unset-upstream"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = run(&mut st, deepen(Deepen::By(2)));
    assert_eq!(git_cmds(&fx), argv(&["fetch", "--deepen=2", "origin"]), "origin when no upstream says otherwise");
    cleanup(&[&src, &dest]);
}

#[test]
fn fetch_more_commits_step() {
    let (mut st, src, dest) = shallow_clone("shallow-step", 4, 1);
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    assert!(run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::FetchMoreCommits))).is_empty());
    let p = st.palette.as_ref().unwrap();
    assert!(matches!(p.mode, Mode::Input(InputKind::DeepenBy)));
    assert!(p.query.is_empty());

    let k = InputKind::DeepenBy;
    assert_eq!(k.command(""), None);
    assert_eq!(k.command("abc"), None);
    assert_eq!(k.command("0"), None);
    assert_eq!(k.problem(""), None);
    assert_eq!(k.problem("abc"), Some(DepthProblem::NotANumber.message()));
    assert_eq!(k.problem("0"), Some(DepthProblem::Zero.message()));
    assert_eq!(k.problem("20"), None);
    assert_eq!(k.command(" 2 "), Some(C::FetchMoreCommitsBy(2)));
    assert_eq!(k.title(), "Fetch more commits");
    assert_eq!(k.footer(), "Enter to fetch, Backspace or Esc to go back");

    let before = st.snap.as_ref().unwrap().shallow_roots.clone();
    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::FetchMoreCommitsBy(1))));
    assert!(st.palette.is_none());
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.commits.len(), 2);
    assert!(snap.is_shallow());
    assert_ne!(snap.shallow_roots, before);
    cleanup(&[&src, &dest]);

    // The step doesn't open in a full repository.
    let (mut full, dir) = repo_with_topic("shallow-step-full");
    run(&mut full, Msg::Palette(PaletteMsg::RunPaletteCommand(C::FetchMoreCommits)));
    assert!(full.palette.is_none());
    cleanup(&[&dir]);
}

#[test]
fn deepen_drops_stats_of_old_boundary() {
    let (mut st, src, dest) = shallow_clone("shallow-stats", 3, 1);
    let root = st.snap.as_ref().unwrap().shallow_roots[0];
    st.session.reads.stats.insert(root, (99, 99, 99));
    st.selection = Selection::Commit(root);

    // Step through the job by hand to see the effects of the reload that follows it.
    let fx = run(&mut st, deepen(Deepen::By(1)));
    let [Effect::RunGit { .. }] = &fx[..] else { panic!("expected one RunGit") };
    let done = runner::execute(fx.into_iter().next().unwrap()).expect("a job result");
    let fx = run(&mut st, done);
    let load = fx.into_iter().find(|e| matches!(e, Effect::LoadRepo { kind: LoadKind::Refresh, .. })).expect("a refresh");
    let loaded = runner::execute(load).expect("a load result");
    let fx = run(&mut st, loaded);
    assert!(!st.session.reads.stats.contains_key(&root), "line counts made as a root commit are dropped");
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadDetail { oid, .. } if *oid == root)), "the selected commit's detail reloads");
    finish(&mut st, fx);
    assert!(!st.snap.as_ref().unwrap().shallow_roots.contains(&root));
    cleanup(&[&src, &dest]);
}

#[test]
fn deepen_refused() {
    let (mut st, dir) = repo_with_topic("shallow-refused-full");
    assert!(run(&mut st, deepen(Deepen::Full)).is_empty());
    assert!(st.log.last().is_some_and(|l| l.text.contains("already has its full history")));
    cleanup(&[&dir]);

    let (mut st, src, dest) = shallow_clone("shallow-refused", 2, 1);
    assert!(run(&mut st, deepen(Deepen::By(0))).is_empty(), "zero commits");
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text == "Fetch at least one more commit."));

    let fx = run(&mut st, Msg::git("Fetch", &["fetch"]));
    assert!(matches!(&fx[..], [Effect::RunGit { .. }]));
    let fx = run(&mut st, deepen(Deepen::Full));
    assert!(!fx.iter().any(|e| matches!(e, Effect::RunGit { .. })), "another job is running");
    assert!(st.log.last().is_some_and(|l| l.text.contains("still running")));
    st.job = None;

    git(&dest, &["remote", "remove", "origin"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(run(&mut st, deepen(Deepen::Full)).is_empty());
    assert!(st.log.last().is_some_and(|l| l.text == "This repository has no remote to fetch its history from."));
    cleanup(&[&src, &dest]);
}

#[test]
fn recall_keys_and_inactive_tabs() {
    assert_eq!(C::FetchFullHistory.recall_key(), Some("fetch-full-history"));
    assert_eq!(C::FetchMoreCommits.recall_key(), Some("fetch-more-commits"));
    assert_eq!(C::FetchMoreCommitsBy(3).recall_key(), None);
    assert!(JobMsg::Deepen(Deepen::Full).on_inactive().is_none());
}
