//! The Branch map end to end with the real runner, across tabs, and from the command palette:
//! the read on a repository whose branches conflict, that it changes nothing, that a second read
//! of unchanged tips makes no trial merge, parking with a read in flight, and the palette entry.

use super::tests::{clean_tree, drive, git, scratch_repo, state};
use super::update;
use glowfly_core::branch_map::{BranchMap, Forecast};
use glowfly_core::branch_ref::BranchRef;
use glowfly_core::gitcli::{args, run};
use glowfly_model::branch_map::BranchMapMsg;
use glowfly_model::effect::partial::StopKey;
use glowfly_model::effect::{ControlEffect, Effect};
use glowfly_model::msg::{Msg, PaletteMsg, RepoMsg, TabsMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::palette::{PaletteCommand, shown};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn file(edits: &[(usize, &str)]) -> String {
    (0..20).map(|i| edits.iter().find(|(n, _)| *n == i).map_or(format!("line {i}"), |(_, t)| (*t).to_owned()) + "\n").collect()
}

fn commit(dir: &Path, text: &str, msg: &str) {
    std::fs::write(dir.join("f.txt"), text).unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", msg]);
}

/// main, `one` and `two` (they conflict with each other), and `topic` (which conflicts with
/// main, which moved on after the three were cut); opened with main checked out.
fn repo(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    clean_tree(&dir);
    commit(&dir, &file(&[]), "base");
    for (b, text) in [("one", file(&[(5, "one")])), ("two", file(&[(5, "two")])), ("topic", file(&[(12, "topic")]))] {
        git(&dir, &["checkout", "-q", "-b", b, "main"]);
        commit(&dir, &text, b);
    }
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, &file(&[(12, "main")]), "main moves");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn map(st: &AppState) -> &BranchMap {
    st.session.branch_map.view.as_ref().expect("open").map().expect("a map")
}

/// Every ref, and git's status.
fn untouched(dir: &Path) -> (Vec<(String, git2::Oid)>, String) {
    let repo = git2::Repository::open(dir).unwrap();
    let mut refs: Vec<(String, git2::Oid)> =
        repo.references().unwrap().flatten().filter_map(|r| Some((r.name()?.to_owned(), r.target()?))).collect();
    refs.sort();
    let (ok, status) = run(dir, &args(&["status", "--porcelain=v1"]));
    assert!(ok);
    (refs, status)
}

fn jobs(fx: &[Effect]) -> Vec<u64> {
    fx.iter().filter_map(|e| if let Effect::BranchMap(j) = e { Some(j.generation) } else { None }).collect()
}

fn local(name: &str) -> BranchRef {
    BranchRef::Local(name.into())
}

#[test]
fn the_map_end_to_end() {
    let (mut st, dir) = repo("bm-flow-e2e");
    let before = untouched(&dir);
    drive(&mut st, Msg::BranchMap(BranchMapMsg::Open));
    let first = map(&st).clone();
    assert_eq!(first.rows[0].branch, local("main"));
    let at = |name: &str| first.row_of(&local(name)).unwrap_or_else(|| panic!("a row for {name}"));
    let (one, two, topic) = (at("one"), at("two"), at("topic"));
    assert_eq!((first.rows[one].ahead, first.rows[one].behind), (1, 1));
    let pair = first.pairs.iter().find(|p| (p.a, p.b) == (one.min(two), one.max(two))).expect("one and two share f.txt");
    assert_eq!(pair.forecast, Forecast::Conflicts(vec!["f.txt".into()]));
    assert_eq!(first.conflicts_with_main(topic), Some(&["f.txt".to_owned()][..]), "topic conflicts with main");
    assert!(first.conflicts_with_main(one).is_none(), "one merges into main cleanly");
    assert_eq!(first.current, Some(0));
    assert_eq!(st.session.branch_map.view.as_ref().unwrap().selected, Some(local("main")), "the checked-out branch");
    assert_eq!(untouched(&dir), before, "the forecast changed no ref and no file");

    drive(&mut st, Msg::BranchMap(BranchMapMsg::Close));
    assert!(st.session.branch_map.view.is_none());
    drive(&mut st, Msg::BranchMap(BranchMapMsg::Open));
    let second = map(&st);
    assert_eq!((&second.rows, &second.pairs), (&first.rows, &first.pairs));
    assert!(first.merged >= 1 && second.merged == 0, "the second read made no trial merge");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn parking_asks_again_and_drops_the_old_result() {
    let (mut st, dir) = repo("bm-flow-parking");
    let fx = update(&mut st, Msg::BranchMap(BranchMapMsg::Open), Instant::now());
    let [g] = jobs(&fx)[..] else { panic!("one job: {fx:?}") };
    let fx = update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    assert!(fx.iter().any(|e| matches!(e, Effect::Control(ControlEffect::StopGit(StopKey::BranchMap(s))) if *s == g)), "{fx:?}");
    assert!(st.session.branch_map.view.is_none(), "the new tab has none");

    let workdir = st.tabs[0].path.clone().unwrap();
    let late = Msg::BranchMap(BranchMapMsg::Loaded { generation: g, result: Err(glowfly_model::testing::said("late")) });
    update(&mut st, Msg::Scoped { workdir, msg: Box::new(late) }, Instant::now());
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    let view = st.session.branch_map.view.as_ref().expect("back with its tab");
    assert!(view.loading && view.result.is_none(), "the late result was dropped");
    let [again] = jobs(&fx)[..] else { panic!("asked again: {fx:?}") };
    assert!(again > g && view.generation == again);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_palette_opens_it_with_a_repository_only() {
    let (mut st, dir) = repo("bm-flow-palette");
    let has = |st: &AppState| shown(st, "").iter().any(|s| s.entry.label == "Open branch map");
    assert!(has(&st));
    assert!(!has(&state()), "not without a repository");
    let fx = update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::OpenBranchMap)), Instant::now());
    assert!(st.session.branch_map.view.is_some() && jobs(&fx).len() == 1);
    let _ = std::fs::remove_dir_all(&dir);
}
