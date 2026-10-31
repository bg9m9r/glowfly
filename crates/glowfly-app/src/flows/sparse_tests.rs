//! Headless tests for the Sparse Checkout page: reading the status, filling the form, the job
//! each action starts (or why it doesn't), what follows a finished job, the palette entry, and
//! turning sparse checkout on and off end to end against a real repository.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::effect::SparseEffect;
use crate::msg::{Msg, PaletteMsg, PrefsMsg, RepoMsg, SparseMsg};
use crate::state::palette::{PaletteCommand as C, entries};
use crate::state::sparse::SparsePage;
use crate::state::{AppState, PrefPage};
use glowfly_core::sparse_ops::{SparseCommand, SparseMode, SparseStatus};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn sparse(st: &mut AppState, m: SparseMsg) -> Vec<Effect> {
    run(st, Msg::Sparse(m))
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

fn page(st: &AppState) -> &SparsePage {
    &st.prefs.as_ref().unwrap().sparse
}

fn page_mut(st: &mut AppState) -> &mut SparsePage {
    &mut st.prefs.as_mut().unwrap().sparse
}

fn on(paths: &[&str], mode: SparseMode) -> SparseStatus {
    SparseStatus {
        enabled: true,
        mode,
        paths: paths.iter().map(|p| (*p).to_owned()).collect(),
        tracked: 3,
        skipped: 1,
        ..Default::default()
    }
}

fn off() -> SparseStatus {
    SparseStatus { tracked: 3, ..Default::default() }
}

/// Opens the page and delivers `s` as the status.
fn page_with(st: &mut AppState, s: SparseStatus) {
    run(st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoSparse)));
    let generation = page(st).generation;
    sparse(st, SparseMsg::Loaded { generation, result: Ok(s) });
}

fn loads(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Sparse(SparseEffect::Load { .. })))
}

fn command(fx: &[Effect]) -> Option<SparseCommand> {
    let commands: Vec<&SparseCommand> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::Sparse(SparseEffect::Checkout { command, .. }) => Some(command),
            _ => None,
        })
        .collect();
    assert!(commands.len() <= 1, "one job at a time");
    commands.first().map(|c| (*c).clone())
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn toasts(st: &AppState) -> Vec<String> {
    st.toasts.iter().map(|t| t.text.clone()).collect()
}

#[test]
fn opening_the_page_reads_the_status() {
    let (mut st, dir) = open("sparse-open");
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)));
    page_mut(&mut st).status = Some(off());
    let fx = sparse(&mut st, SparseMsg::ShowPage);
    assert_eq!(st.prefs.as_ref().unwrap().page, PrefPage::RepoSparse);
    let generation = page(&st).generation;
    assert_eq!(generation, 1);
    assert!(page(&st).status.is_none(), "Reading… until the status arrives");
    assert!(fx.iter().any(|e| matches!(e, Effect::Sparse(SparseEffect::Load { workdir, generation: 1, .. }) if *workdir == dir)));

    let fx = run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoSparse)));
    assert!(loads(&fx));
    cleanup(&dir);
}

#[test]
fn loaded_results_fill_the_form() {
    let (mut st, dir) = open("sparse-loaded");
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoSparse)));
    let current = page(&st).generation;
    sparse(&mut st, SparseMsg::Loaded { generation: current + 7, result: Ok(off()) });
    assert!(page(&st).status.is_none(), "a stale read is ignored");

    sparse(&mut st, SparseMsg::Loaded { generation: current, result: Ok(on(&["src", "docs/guide"], SparseMode::Patterns)) });
    assert_eq!((page(&st).paths.as_str(), page(&st).mode), ("src\ndocs/guide", SparseMode::Patterns));
    assert!(!page(&st).edited());

    // A new status after the user edited keeps the text, unless a job asked for a refill.
    page_mut(&mut st).paths = "mine".into();
    sparse(&mut st, SparseMsg::Reload);
    let g = page(&st).generation;
    sparse(&mut st, SparseMsg::Loaded { generation: g, result: Ok(on(&["lib"], SparseMode::Cone)) });
    assert_eq!(page(&st).paths, "mine");
    assert!(page(&st).edited());
    run(&mut st, Msg::Prefs(PrefsMsg::RevertPage));
    assert_eq!((page(&st).paths.as_str(), page(&st).mode), ("lib", SparseMode::Cone));

    page_mut(&mut st).paths = "mine again".into();
    page_mut(&mut st).refill = true;
    sparse(&mut st, SparseMsg::Reload);
    let g = page(&st).generation;
    sparse(&mut st, SparseMsg::Loaded { generation: g, result: Ok(on(&["new"], SparseMode::Cone)) });
    assert_eq!(page(&st).paths, "new");

    sparse(&mut st, SparseMsg::Reload);
    let g = page(&st).generation;
    sparse(&mut st, SparseMsg::Loaded { generation: g, result: Err("git is missing".into()) });
    assert_eq!((page(&st).status.is_none(), page(&st).error.as_deref()), (true, Some("git is missing")));
    cleanup(&dir);
}

#[test]
fn apply_starts_one_set_job() {
    let (mut st, dir) = open("sparse-apply");
    page_with(&mut st, off());
    page_mut(&mut st).paths = "src\n\ndocs/".into();
    let fx = sparse(&mut st, SparseMsg::Apply);
    let set = SparseCommand::Set { mode: SparseMode::Cone, paths: vec!["src".into(), "docs".into()] };
    assert_eq!(command(&fx), Some(set));
    let job = st.job.as_ref().unwrap();
    assert_eq!((job.label.as_str(), job.keeps_undo), ("Turn on sparse checkout", false));

    st.job = None;
    page_mut(&mut st).status = Some(on(&["src"], SparseMode::Cone));
    let fx = sparse(&mut st, SparseMsg::Apply);
    assert!(command(&fx).is_some());
    assert_eq!(st.job.as_ref().unwrap().label, "Save sparse paths");

    // Another job is still running.
    let fx = sparse(&mut st, SparseMsg::Apply);
    assert!(command(&fx).is_none());
    assert_eq!(last_log(&st), "Another operation is still running.");
    cleanup(&dir);
}

#[test]
fn apply_refuses_bad_paths() {
    let (mut st, dir) = open("sparse-refuse");
    page_with(&mut st, off());
    page_mut(&mut st).paths = "*.md".into();
    let fx = sparse(&mut st, SparseMsg::Apply);
    assert!(command(&fx).is_none() && st.job.is_none());
    assert!(page(&st).paths_error.as_deref().is_some_and(|e| e.contains("*.md")));

    page_mut(&mut st).mode = SparseMode::Patterns;
    page_mut(&mut st).paths = " \n".into();
    let fx = sparse(&mut st, SparseMsg::Apply);
    assert!(command(&fx).is_none() && st.job.is_none());
    assert!(page(&st).paths_error.is_some());
    cleanup(&dir);
}

#[test]
fn reapply_and_turn_off_need_sparse_checkout_on() {
    let (mut st, dir) = open("sparse-reapply");
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoSparse)));
    for m in [SparseMsg::Reapply, SparseMsg::Disable] {
        assert!(command(&sparse(&mut st, m)).is_none());
        assert_eq!(last_log(&st), "Sparse checkout is off in this repository.");
    }
    let g = page(&st).generation;
    sparse(&mut st, SparseMsg::Loaded { generation: g, result: Ok(off()) });
    assert!(command(&sparse(&mut st, SparseMsg::Reapply)).is_none() && st.job.is_none());

    page_mut(&mut st).status = Some(on(&["src"], SparseMode::Cone));
    assert_eq!(command(&sparse(&mut st, SparseMsg::Reapply)), Some(SparseCommand::Reapply));
    assert_eq!(st.job.take().unwrap().label, "Reapply sparse paths");
    assert_eq!(command(&sparse(&mut st, SparseMsg::Disable)), Some(SparseCommand::Disable));
    assert_eq!(st.job.as_ref().unwrap().label, "Turn off sparse checkout");
    cleanup(&dir);
}

#[test]
fn a_finished_job_reloads_the_tab_and_the_page() {
    let (mut st, dir) = open("sparse-done");
    page_with(&mut st, on(&["src"], SparseMode::Cone));
    sparse(&mut st, SparseMsg::Reapply);
    let fx = sparse(&mut st, SparseMsg::Done { ok: true, output: "$ git sparse-checkout reapply".into() });
    assert!(st.job.is_none());
    assert!(toasts(&st).contains(&"Reapply sparse paths succeeded".to_owned()), "{:?}", toasts(&st));
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { known: None, .. })), "a full reload");
    assert!(loads(&fx));
    assert!(page(&st).status.is_some(), "the page keeps its status until the new one arrives");
    assert!(page(&st).refill);

    page_mut(&mut st).paths = "src\ndocs".into();
    sparse(&mut st, SparseMsg::Apply);
    let output = "$ git sparse-checkout set --cone --no-sparse-index --stdin\n  src\nwarning: The following paths are not up to date \
                  and were left despite sparse patterns:\n\tdocs/d.txt\n\tdocs/e.txt\n\nAfter fixing the above paths, you may want \
                  to run `git sparse-checkout reapply`.";
    sparse(&mut st, SparseMsg::Done { ok: true, output: output.into() });
    let kept = "Save sparse paths: 2 changed files outside the paths were kept. Commit or discard them, then Reapply.";
    assert!(toasts(&st).contains(&kept.to_owned()), "{:?}", toasts(&st));

    sparse(&mut st, SparseMsg::Disable);
    let fx = sparse(&mut st, SparseMsg::Done { ok: false, output: "fatal: nope".into() });
    assert!(toasts(&st).contains(&"Turn off sparse checkout failed: fatal: nope".to_owned()));
    assert!(loads(&fx) && !page(&st).refill, "a failed job keeps the user's text");

    // The tab changed while the job ran.
    sparse(&mut st, SparseMsg::Reapply);
    st.job.as_mut().unwrap().workdir = Some(dir.join("elsewhere"));
    let fx = sparse(&mut st, SparseMsg::Done { ok: true, output: String::new() });
    assert!(st.job.is_none());
    assert!(!loads(&fx) && !fx.iter().any(|e| matches!(e, Effect::LoadRepo { .. })), "the other tab is left alone");
    cleanup(&dir);
}

#[test]
fn a_status_for_another_tab_is_dropped() {
    let (mut st, dir) = open("sparse-scoped");
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoSparse)));
    let msg = Msg::Sparse(SparseMsg::Loaded { generation: 1, result: Ok(off()) });
    run(&mut st, Msg::Scoped { workdir: dir.join("elsewhere"), msg: Box::new(msg) });
    assert!(page(&st).status.is_none());
    cleanup(&dir);
}

#[test]
fn palette_entry() {
    let (mut st, dir) = open("sparse-palette");
    let labels: Vec<String> = entries(&st).into_iter().map(|e| e.label).collect();
    let at = |l: &str| labels.iter().position(|x| x == l).unwrap_or_else(|| panic!("{l} missing"));
    assert!(at("Initialize Git LFS") < at("Configure Git LFS…"));
    assert!(at("Configure Git LFS…") < at("Configure sparse checkout…"));
    assert!(at("Configure sparse checkout…") < at("Configure Git Flow…"));
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    let fx = run(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::ConfigureSparse)));
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::RepoSparse));
    assert!(loads(&fx));
    assert!(!entries(&state()).iter().any(|e| e.label.contains("sparse")));
    cleanup(&dir);
}

#[test]
fn turn_on_and_off_end_to_end() {
    let dir = scratch_repo("sparse-e2e");
    git(&dir, &["checkout", "--", "a.txt"]);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::create_dir_all(dir.join("docs")).unwrap();
    std::fs::write(dir.join("src/x.txt"), "x\n").unwrap();
    std::fs::write(dir.join("docs/d.txt"), "d\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "folders"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoSparse)));
    assert!(page(&st).status.as_ref().is_some_and(|s| !s.enabled));

    page_mut(&mut st).paths = "src".into();
    let before = st.load_gen;
    drive(&mut st, Msg::Sparse(SparseMsg::Apply));
    assert!(st.load_gen > before && !st.loading, "the tab was loaded again");
    assert!(st.job.is_none(), "{:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    assert!(!dir.join("docs/d.txt").exists() && dir.join("src/x.txt").exists());
    let s = page(&st).status.clone().unwrap();
    assert!(s.enabled && s.paths == ["src"], "{s:?}");
    assert_eq!(page(&st).paths, "src");
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.unstaged.is_empty(), "left-out files aren't listed as deleted: {:?}", snap.unstaged);

    drive(&mut st, Msg::Sparse(SparseMsg::Disable));
    assert!(dir.join("docs/d.txt").exists());
    assert!(page(&st).status.as_ref().is_some_and(|s| !s.enabled));
    assert!(toasts(&st).contains(&"Turn off sparse checkout succeeded".to_owned()), "{:?}", toasts(&st));
    cleanup(&dir);
}
