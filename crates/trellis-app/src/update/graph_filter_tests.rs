//! Headless tests for hiding and soloing refs in the graph: effects, the reloaded graph, the
//! filter saved per repository, and stale entries dropped after a load.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::runner;
use crate::state::AppState;
use crate::state::branches::HEAD_ALWAYS_SHOWN;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::LoadOptions;
use trellis_core::settings::{FilterOp, GraphFilter};

/// A repository on `main` with one branch per name, each one commit ("<name> work") ahead.
fn repo_with(name: &str, branches: &[&str]) -> PathBuf {
    let dir = scratch_repo(name);
    for b in branches {
        git(&dir, &["checkout", "-q", "-b", b, "main"]);
        std::fs::write(dir.join(format!("{b}.txt")), b).unwrap();
        git(&dir, &["add", &format!("{b}.txt")]);
        git(&dir, &["commit", "-q", "-m", &format!("{b} work")]);
    }
    git(&dir, &["checkout", "-q", "main"]);
    dir
}

fn opened(name: &str, branches: &[&str]) -> (AppState, PathBuf) {
    let dir = repo_with(name, branches);
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));
    (st, dir)
}

fn heads(b: &str) -> String {
    format!("refs/heads/{b}")
}

fn op(st: &mut AppState, op: FilterOp) -> Vec<Effect> {
    update(st, Msg::GraphFilter(op), Instant::now())
}

/// Runs effects already returned by `update` to completion.
fn run(st: &mut AppState, fx: Vec<Effect>) {
    for e in fx {
        if matches!(e, Effect::LoadRepo { .. })
            && let Some(m) = runner::execute(e)
        {
            drive(st, m);
        }
    }
}

fn has_commit(st: &AppState, summary: &str) -> bool {
    st.snap.as_ref().unwrap().commits.iter().any(|c| c.summary == summary)
}

fn has_label(st: &AppState, name: &str) -> bool {
    st.snap.as_ref().unwrap().refs.values().flatten().any(|l| l.name == name)
}

fn filter(st: &AppState) -> GraphFilter {
    st.settings.graph_filter_for(&st.snap.as_ref().unwrap().workdir)
}

/// The options of the single LoadRepo in `fx`.
fn load_opts(fx: &[Effect]) -> &LoadOptions {
    let mut loads = fx.iter().filter_map(|e| match e {
        Effect::LoadRepo { opts, .. } => Some(opts),
        _ => None,
    });
    let opts = loads.next().expect("a LoadRepo");
    assert!(loads.next().is_none(), "one LoadRepo");
    opts
}

#[test]
fn hide_saves_and_reloads_without_branch() {
    let (mut st, dir) = opened("gf-hide", &["feature"]);
    let before = st.snap.as_ref().unwrap().commits.len();
    assert!(has_label(&st, "feature") && has_commit(&st, "feature work"));

    let fx = op(&mut st, FilterOp::Hide(heads("feature")));
    assert!(matches!(&fx[..], [Effect::SaveSettings(_), Effect::LoadRepo { .. }]));
    assert_eq!(load_opts(&fx).hidden, [heads("feature")]);
    run(&mut st, fx);
    assert!(!has_label(&st, "feature") && !has_commit(&st, "feature work"));
    assert_eq!(st.snap.as_ref().unwrap().commits.len(), before - 1);
    assert_eq!(filter(&st).hidden, [heads("feature")]);
    assert!(st.snap.as_ref().unwrap().locals.iter().any(|b| b.name == "feature"), "still listed");

    // Repeating it changes nothing.
    assert!(op(&mut st, FilterOp::Hide(heads("feature"))).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn show_restores_branch() {
    let (mut st, dir) = opened("gf-show", &["feature"]);
    drive(&mut st, Msg::GraphFilter(FilterOp::Hide(heads("feature"))));
    assert!(!has_label(&st, "feature"));
    let fx = op(&mut st, FilterOp::Show(heads("feature")));
    assert!(load_opts(&fx).hidden.is_empty());
    run(&mut st, fx);
    assert!(st.settings.graph_filters.is_empty(), "the empty filter is removed");
    assert!(has_label(&st, "feature") && has_commit(&st, "feature work"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hiding_checked_out_branch_is_refused() {
    let (mut st, dir) = opened("gf-head", &["feature"]);
    assert!(op(&mut st, FilterOp::Hide(heads("main"))).is_empty());
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some(HEAD_ALWAYS_SHOWN));
    assert!(st.settings.graph_filters.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn filter_messages_need_a_repository() {
    let mut st = state();
    assert!(op(&mut st, FilterOp::Solo(heads("feature"))).is_empty());
    assert!(st.settings.graph_filters.is_empty());
}

#[test]
fn solo_and_stop_soloing() {
    let (mut st, dir) = opened("gf-solo", &["feature", "other"]);
    drive(&mut st, Msg::GraphFilter(FilterOp::Solo(heads("feature"))));
    assert!(has_commit(&st, "feature work") && has_commit(&st, "first"));
    assert!(!has_commit(&st, "other work") && !has_label(&st, "other"));
    assert!(has_label(&st, "main"), "the checked-out branch stays");

    drive(&mut st, Msg::GraphFilter(FilterOp::Solo(heads("other"))));
    assert!(has_commit(&st, "feature work") && has_commit(&st, "other work"));
    assert_eq!(filter(&st).solo, [heads("feature"), heads("other")]);

    assert!(op(&mut st, FilterOp::Unsolo(heads("main"))).is_empty(), "not soloed: nothing to do");
    drive(&mut st, Msg::GraphFilter(FilterOp::StopSoloing));
    assert!(st.settings.graph_filters.is_empty());
    assert!(has_commit(&st, "feature work") && has_commit(&st, "other work") && has_label(&st, "other"));
    assert!(op(&mut st, FilterOp::StopSoloing).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn filters_are_per_repository_across_tabs() {
    let (mut st, a) = opened("gf-tabs-a", &["feature"]);
    let b = repo_with("gf-tabs-b", &["feature"]);
    drive(&mut st, Msg::GraphFilter(FilterOp::Hide(heads("feature"))));

    let fx = update(&mut st, Msg::Open(b.clone()), Instant::now());
    let opts = load_opts(&fx);
    assert!(opts.hidden.is_empty() && opts.solo.is_empty(), "B has no filter");
    run(&mut st, fx);
    assert!(has_label(&st, "feature"));

    let fx = update(&mut st, Msg::SelectTab(0), Instant::now());
    assert_eq!(load_opts(&fx).hidden, [heads("feature")], "A's filter comes back with its tab");
    run(&mut st, fx);
    assert!(!has_label(&st, "feature"));
    for d in [&a, &b] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn reopen_with_other_spelling_applies_saved_filter() {
    let (st, dir) = opened("gf-spelling", &["feature"]);
    let workdir = st.snap.as_ref().unwrap().workdir.clone();
    let hidden = GraphFilter { hidden: vec![heads("feature")], solo: vec![] };

    // A fresh start that opens the folder as the user typed it, not as libgit2 spells it.
    let mut st = state();
    st.settings.set_graph_filter(&workdir, hidden.clone());
    let fx = update(&mut st, Msg::Open(dir.clone()), Instant::now());
    assert!(load_opts(&fx).hidden.is_empty(), "the requested spelling has no filter saved");
    run(&mut st, fx);
    assert!(!has_label(&st, "feature") && !has_commit(&st, "feature work"));
    assert_eq!(filter(&st), hidden);

    // Opened through a subfolder.
    let sub: &Path = &dir.join("sub");
    std::fs::create_dir_all(sub).unwrap();
    let mut st = state();
    st.settings.set_graph_filter(&workdir, hidden);
    drive(&mut st, Msg::Open(sub.to_path_buf()));
    assert!(!has_label(&st, "feature"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deleted_refs_are_pruned() {
    let (mut st, dir) = opened("gf-prune", &["feature", "other", "keep"]);
    drive(&mut st, Msg::GraphFilter(FilterOp::Hide(heads("feature"))));
    drive(&mut st, Msg::GraphFilter(FilterOp::Solo(heads("other"))));
    assert!(!has_commit(&st, "keep work"));
    git(&dir, &["branch", "-D", "feature", "other"]);

    let fx = update(&mut st, Msg::Refresh, Instant::now());
    let loaded = fx.into_iter().find_map(|e| matches!(e, Effect::LoadRepo { .. }).then(|| runner::execute(e)).flatten()).unwrap();
    let fx = update(&mut st, loaded, Instant::now());
    assert!(st.settings.graph_filters.is_empty(), "both entries are gone");
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))));
    let opts = load_opts(&fx);
    assert!(opts.hidden.is_empty() && opts.solo.is_empty(), "reloaded without the solo");
    run(&mut st, fx);
    assert!(has_commit(&st, "keep work") && has_label(&st, "keep"));
    let _ = std::fs::remove_dir_all(&dir);
}
