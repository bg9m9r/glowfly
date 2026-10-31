//! Headless tests for hiding and soloing refs in the graph: effects, the reloaded graph, the
//! filter saved per repository, and stale entries dropped after a load.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::GraphMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::TabsMsg;
use crate::runner;
use crate::state::AppState;
use crate::state::branches::HEAD_ALWAYS_SHOWN;
use glowfly_core::repo::LoadOptions;
use glowfly_core::settings::{FilterOp, GraphFilter};
use std::path::{Path, PathBuf};
use std::time::Instant;

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
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn heads(b: &str) -> String {
    format!("refs/heads/{b}")
}

fn op(st: &mut AppState, op: FilterOp) -> Vec<Effect> {
    update(st, Msg::Graph(GraphMsg::GraphFilter(op)), Instant::now())
}

/// Runs effects already returned by `update` to completion.
fn run(st: &mut AppState, fx: Vec<Effect>) {
    for e in fx {
        if matches!(e, Effect::LoadRepo { .. })
            && let Some(m) = runner::testing::execute(e)
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
    st.settings.graph_filter_for(&st.snap.as_ref().unwrap().real_workdir)
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
    drive(&mut st, Msg::Graph(GraphMsg::GraphFilter(FilterOp::Hide(heads("feature")))));
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
fn checking_out_hidden_branch_shows_it_again() {
    let (mut st, dir) = opened("gf-checkout-hidden", &["feature", "other"]);
    drive(&mut st, Msg::Graph(GraphMsg::GraphFilter(FilterOp::Hide(heads("feature")))));
    drive(&mut st, Msg::Graph(GraphMsg::GraphFilter(FilterOp::Hide(heads("other")))));
    git(&dir, &["checkout", "-q", "feature"]);

    let fx = update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    let loaded = fx.into_iter().find_map(|e| matches!(e, Effect::LoadRepo { .. }).then(|| runner::testing::execute(e)).flatten()).unwrap();
    let fx = update(&mut st, loaded, Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))));
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadRepo { .. })), "the load already showed it");
    assert_eq!(filter(&st).hidden, [heads("other")], "only the checked-out branch is unhidden");
    assert!(has_label(&st, "feature") && has_commit(&st, "feature work"));
    assert!(!has_label(&st, "other"));
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
    drive(&mut st, Msg::Graph(GraphMsg::GraphFilter(FilterOp::Solo(heads("feature")))));
    assert!(has_commit(&st, "feature work") && has_commit(&st, "first"));
    assert!(!has_commit(&st, "other work") && !has_label(&st, "other"));
    assert!(has_label(&st, "main"), "the checked-out branch stays");

    drive(&mut st, Msg::Graph(GraphMsg::GraphFilter(FilterOp::Solo(heads("other")))));
    assert!(has_commit(&st, "feature work") && has_commit(&st, "other work"));
    assert_eq!(filter(&st).solo, [heads("feature"), heads("other")]);

    assert!(op(&mut st, FilterOp::Unsolo(heads("main"))).is_empty(), "not soloed: nothing to do");
    drive(&mut st, Msg::Graph(GraphMsg::GraphFilter(FilterOp::StopSoloing)));
    assert!(st.settings.graph_filters.is_empty());
    assert!(has_commit(&st, "feature work") && has_commit(&st, "other work") && has_label(&st, "other"));
    assert!(op(&mut st, FilterOp::StopSoloing).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn filters_are_per_repository_across_tabs() {
    let (mut st, a) = opened("gf-tabs-a", &["feature"]);
    let b = repo_with("gf-tabs-b", &["feature"]);
    drive(&mut st, Msg::Graph(GraphMsg::GraphFilter(FilterOp::Hide(heads("feature")))));

    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(b.clone())), Instant::now());
    let opts = load_opts(&fx);
    assert!(opts.hidden.is_empty() && opts.solo.is_empty(), "B has no filter");
    run(&mut st, fx);
    assert!(has_label(&st, "feature"));

    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
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

    // A fresh start that opens the folder as the user typed it, with the filter saved by an
    // earlier version under libgit2's spelling (`C:/x/repo/`): another spelling of the same path.
    let mut st = state();
    st.settings.graph_filters.insert(workdir.display().to_string(), hidden.clone());
    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())), Instant::now());
    assert_eq!(load_opts(&fx).hidden, hidden.hidden, "found under the old spelling");
    run(&mut st, fx);
    assert!(!has_label(&st, "feature") && !has_commit(&st, "feature work"));
    assert_eq!(filter(&st), hidden);
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    assert_eq!(st.settings.graph_filters.keys().collect::<Vec<_>>(), [&glowfly_core::settings::repo_keys::key(&real)], "moved");

    // Opened through a subfolder.
    let sub: &Path = &dir.join("sub");
    std::fs::create_dir_all(sub).unwrap();
    let mut st = state();
    st.settings.set_graph_filter(&workdir, hidden);
    drive(&mut st, Msg::Repo(RepoMsg::Open(sub.to_path_buf())));
    assert!(!has_label(&st, "feature"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deleted_refs_are_pruned() {
    let (mut st, dir) = opened("gf-prune", &["feature", "other", "keep"]);
    drive(&mut st, Msg::Graph(GraphMsg::GraphFilter(FilterOp::Hide(heads("feature")))));
    drive(&mut st, Msg::Graph(GraphMsg::GraphFilter(FilterOp::Solo(heads("other")))));
    assert!(!has_commit(&st, "keep work"));
    git(&dir, &["branch", "-D", "feature", "other"]);

    let fx = update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    let loaded = fx.into_iter().find_map(|e| matches!(e, Effect::LoadRepo { .. }).then(|| runner::testing::execute(e)).flatten()).unwrap();
    let fx = update(&mut st, loaded, Instant::now());
    assert!(st.settings.graph_filters.is_empty(), "both entries are gone");
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))));
    let opts = load_opts(&fx);
    assert!(opts.hidden.is_empty() && opts.solo.is_empty(), "reloaded without the solo");
    run(&mut st, fx);
    assert!(has_commit(&st, "keep work") && has_label(&st, "keep"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hovering_a_ref_highlights_its_branch_rows_in_update() {
    let (mut st, dir) = opened("graph-hover", &["topic"]);
    let snap = st.snap.as_ref().unwrap();
    let topic = snap.locals.iter().find(|b| b.name == "topic").and_then(|b| b.oid).unwrap();
    let main = snap.locals.iter().find(|b| b.name == "main").and_then(|b| b.oid).unwrap();
    update(&mut st, Msg::Graph(GraphMsg::HoverRef(Some(topic))), Instant::now());
    let (tip, rows) = st.view.highlight.clone().expect("the hovered branch is highlighted");
    assert_eq!(tip, topic);
    assert!(rows.contains(&topic) && !rows.contains(&main), "only the branch's own commits: {rows:?}");
    assert!(st.view.ref_rows.base.as_ref().is_some_and(|b| b.contains(&main)), "the default branch's history is kept");
    update(&mut st, Msg::Graph(GraphMsg::HoverRef(None)), Instant::now());
    assert!(st.view.highlight.is_none());
    // A reload walks the default branch again on the next hover.
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(st.view.ref_rows.base.is_none() && st.view.ref_rows.rows.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clearing_the_activity_log_is_a_message() {
    let mut st = state();
    st.log(true, "one");
    update(&mut st, Msg::ClearLog, Instant::now());
    assert!(st.log.is_empty());
}

fn tags(t: &str) -> String {
    format!("refs/tags/{t}")
}

#[test]
fn hide_show_and_solo_a_tag() {
    let (mut st, dir) = opened("gf-tag", &["feature", "other"]);
    git(&dir, &["tag", "rel", "feature"]);
    git(&dir, &["branch", "-D", "feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(has_label(&st, "rel") && has_commit(&st, "feature work"));

    let fx = op(&mut st, FilterOp::Hide(tags("rel")));
    assert!(matches!(&fx[..], [Effect::SaveSettings(_), Effect::LoadRepo { .. }]));
    assert_eq!(load_opts(&fx).hidden, [tags("rel")]);
    run(&mut st, fx);
    assert!(!has_label(&st, "rel") && !has_commit(&st, "feature work"));
    assert!(has_commit(&st, "other work"));
    assert!(st.snap.as_ref().unwrap().tags.iter().any(|(n, _)| n == "rel"), "still listed");
    assert_eq!(filter(&st).hidden, [tags("rel")]);

    drive(&mut st, Msg::Graph(GraphMsg::GraphFilter(FilterOp::Show(tags("rel")))));
    assert!(st.settings.graph_filters.is_empty());
    assert!(has_label(&st, "rel") && has_commit(&st, "feature work"));

    drive(&mut st, Msg::Graph(GraphMsg::GraphFilter(FilterOp::Solo(tags("rel")))));
    assert!(has_commit(&st, "feature work") && has_commit(&st, "first"));
    assert!(!has_commit(&st, "other work") && !has_label(&st, "other"));
    assert!(has_label(&st, "main"), "the checked-out branch stays");

    drive(&mut st, Msg::Graph(GraphMsg::GraphFilter(FilterOp::StopSoloing)));
    assert!(st.settings.graph_filters.is_empty());
    assert!(has_commit(&st, "feature work") && has_commit(&st, "other work") && has_label(&st, "other"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deleted_tags_are_pruned() {
    let (mut st, dir) = opened("gf-prune-tags", &["feature", "other"]);
    git(&dir, &["tag", "rel", "feature"]);
    git(&dir, &["tag", "keep", "other"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Graph(GraphMsg::GraphFilter(FilterOp::Hide(tags("keep")))));
    drive(&mut st, Msg::Graph(GraphMsg::GraphFilter(FilterOp::Solo(tags("rel")))));
    assert!(!has_commit(&st, "other work"));
    git(&dir, &["tag", "-d", "rel", "keep"]);

    let fx = update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    let loaded = fx.into_iter().find_map(|e| matches!(e, Effect::LoadRepo { .. }).then(|| runner::testing::execute(e)).flatten()).unwrap();
    let fx = update(&mut st, loaded, Instant::now());
    assert!(st.settings.graph_filters.is_empty(), "both entries are gone");
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))));
    let opts = load_opts(&fx);
    assert!(opts.hidden.is_empty() && opts.solo.is_empty(), "reloaded without the solo");
    run(&mut st, fx);
    assert!(has_commit(&st, "other work"));
    let _ = std::fs::remove_dir_all(&dir);
}
