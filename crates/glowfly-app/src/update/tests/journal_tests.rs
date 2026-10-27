//! Headless tests for the operation journal: opening it (and what it closes), loads and
//! previews with their generations, what closes it, the refresh hook, parking it with its tab
//! and the palette entry.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::gitflow::GitFlowMsg;
use crate::journal::{JournalEffect, JournalMsg};
use crate::msg::{Msg, PaletteMsg, RepoMsg, TabsMsg};
use crate::state::AppState;
use crate::state::blame::BlameView;
use crate::state::history::FileHistoryView;
use crate::state::layout::{ToolView, tool_view};
use crate::state::palette::{PaletteCommand, shown};
use crate::state::rebase::RebasePanel;
use glowfly_core::journal::{RowKey, Subject, Timeline};
use glowfly_core::rebase::RebaseBase;
use glowfly_core::repo::{self, LoadOptions};
use std::path::PathBuf;
use std::time::Instant;

fn run(st: &mut AppState, m: JournalMsg) -> Vec<Effect> {
    update(st, Msg::Journal(m), Instant::now())
}

/// A repository with commits on main and a branch `topic` one commit behind, opened.
pub(super) fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    super::tests::clean_tree(&dir);
    git(&dir, &["branch", "topic"]);
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "second"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(st.snap.is_some());
    (st, dir)
}

fn loads(fx: &[Effect]) -> Vec<u64> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Journal(JournalEffect::Load { generation, .. }) => Some(*generation),
            _ => None,
        })
        .collect()
}

fn timeline(dir: &std::path::Path) -> Box<Timeline> {
    Box::new(glowfly_core::journal::load(dir).unwrap())
}

/// Opens the journal and lands its load.
pub(super) fn open_loaded(st: &mut AppState, dir: &std::path::Path) {
    let fx = run(st, JournalMsg::Open);
    let [g] = loads(&fx)[..] else { panic!("one load") };
    run(st, JournalMsg::Loaded { generation: g, result: Ok(timeline(dir)) });
}

fn key_of(st: &AppState, subject: &Subject) -> RowKey {
    st.session.journal.view.as_ref().unwrap().rows().iter().find(|r| r.subject == *subject).expect("a row").key()
}

#[test]
fn opening_from_the_palette_reads_it_and_closes_the_views_it_replaces() {
    let (mut st, dir) = opened("journal-open");
    let h = st.snap.as_ref().unwrap().head_oid.unwrap();
    st.session.history.view = Some(FileHistoryView::loading("a.txt".into(), None));
    st.session.blame.view = Some(BlameView::loading("a.txt".into(), None));
    st.session.rebase.view = Some(RebasePanel::loading(RebaseBase::FromCommit(h)));
    st.session.gitflow_panel = true;
    let fx = update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::OpenJournal)), Instant::now());
    assert!(st.session.journal.view.is_some());
    assert!(
        st.session.history.view.is_none()
            && st.session.blame.view.is_none()
            && st.session.rebase.view.is_none()
            && !st.session.gitflow_panel
            && st.session.diff.view.is_none()
    );
    let load = fx.iter().find(|e| matches!(e, Effect::Journal(JournalEffect::Load { .. }))).expect("a load");
    assert!(load.scope().is_some_and(|w| super::tests::same_place(w, &dir)), "scoped to the repository");
    assert_eq!(tool_view(&st), Some(ToolView::Journal), "the details panel becomes its rail");
    assert!(!st.graph_shown());
    let _ = std::fs::remove_dir_all(&dir);

    let mut st = state();
    assert!(run(&mut st, JournalMsg::Open).is_empty());
    assert!(st.session.journal.view.is_none(), "nothing without a repository");
}

#[test]
fn stale_or_closed_loads_are_dropped() {
    let (mut st, dir) = opened("journal-stale");
    let first = loads(&run(&mut st, JournalMsg::Open))[0];
    let second = loads(&run(&mut st, JournalMsg::Refresh))[0];
    assert!(second > first);
    run(&mut st, JournalMsg::Loaded { generation: first, result: Ok(timeline(&dir)) });
    assert!(st.session.journal.view.as_ref().unwrap().timeline.is_none(), "an older load is dropped");
    run(&mut st, JournalMsg::Loaded { generation: second, result: Ok(timeline(&dir)) });
    let v = st.session.journal.view.as_ref().unwrap();
    assert!(!v.loading && !v.rows().is_empty());
    run(&mut st, JournalMsg::Close);
    run(&mut st, JournalMsg::Loaded { generation: second, result: Ok(timeline(&dir)) });
    assert!(st.session.journal.view.is_none(), "a closed view stays closed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn select_previews_against_the_subjects_tip_now() {
    let (mut st, dir) = opened("journal-select");
    git(&dir, &["checkout", "-q", "topic"]);
    git(&dir, &["checkout", "-q", "main"]);
    open_loaded(&mut st, &dir);
    let snap = st.snap.as_ref().unwrap();
    let topic_tip = snap.locals.iter().find(|b| b.name == "topic").unwrap().oid;
    let head = snap.head_oid;
    let key = key_of(&st, &Subject::Branch("topic".into()));
    let fx = run(&mut st, JournalMsg::Select(key.clone()));
    let [Effect::Journal(JournalEffect::Preview { key: k, against, generation, .. })] = &fx[..] else { panic!("one preview") };
    assert_eq!((k, *against), (&key, topic_tip));
    let g = *generation;
    let head_key = key_of(&st, &Subject::Head);
    let fx = run(&mut st, JournalMsg::Select(head_key.clone()));
    let [Effect::Journal(JournalEffect::Preview { against, generation: g2, .. })] = &fx[..] else { panic!("one preview") };
    assert_eq!(*against, head, "HEAD rows compare with HEAD's commit");
    let g2 = *g2;
    let result = glowfly_core::journal::preview(&dir, key.oid, topic_tip).unwrap();
    run(&mut st, JournalMsg::PreviewLoaded { generation: g, key: key.clone(), result: Ok(result.clone()) });
    assert!(st.session.journal.view.as_ref().unwrap().preview.as_ref().unwrap().result.is_none(), "a stale preview is dropped");
    run(&mut st, JournalMsg::PreviewLoaded { generation: g2, key: head_key.clone(), result: Ok(result) });
    assert!(st.session.journal.view.as_ref().unwrap().preview_of(&head_key).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_reload_keeps_the_selection_by_its_key() {
    let (mut st, dir) = opened("journal-keep");
    open_loaded(&mut st, &dir);
    let key = key_of(&st, &Subject::Branch("topic".into()));
    run(&mut st, JournalMsg::Select(key.clone()));
    let g = loads(&run(&mut st, JournalMsg::Refresh))[0];
    let fx = run(&mut st, JournalMsg::Loaded { generation: g, result: Ok(timeline(&dir)) });
    assert_eq!(st.session.journal.view.as_ref().unwrap().selected, Some(key), "kept");
    assert!(fx.iter().any(|e| matches!(e, Effect::Journal(JournalEffect::Preview { .. }))), "its preview is read again");
    let g = loads(&run(&mut st, JournalMsg::Refresh))[0];
    run(&mut st, JournalMsg::Loaded { generation: g, result: Ok(Box::new(Timeline::default())) });
    assert_eq!(st.session.journal.view.as_ref().unwrap().selected, None, "the row is gone");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn close_select_commit_git_flow_and_another_repository_close_it() {
    let (mut st, dir) = opened("journal-close");
    run(&mut st, JournalMsg::Open);
    run(&mut st, JournalMsg::Close);
    assert!(st.session.journal.view.is_none() && st.graph_shown());
    run(&mut st, JournalMsg::Toggle);
    assert!(st.session.journal.view.is_some());
    run(&mut st, JournalMsg::Toggle);
    assert!(st.session.journal.view.is_none(), "the View menu's item toggles it");

    run(&mut st, JournalMsg::Open);
    let h = st.snap.as_ref().unwrap().head_oid.unwrap();
    update(&mut st, Msg::Repo(RepoMsg::JumpTo(h)), Instant::now());
    assert!(st.session.journal.view.is_none(), "selecting a graph commit");

    run(&mut st, JournalMsg::Open);
    update(&mut st, Msg::GitFlow(GitFlowMsg::OpenPanel), Instant::now());
    assert!(st.session.journal.view.is_none(), "opening Git Flow");

    run(&mut st, JournalMsg::Open);
    let snap = Box::new(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    st.loading = true;
    let generation = st.load_gen;
    update(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation, kind: LoadKind::Open, result: Ok(snap) }), Instant::now());
    assert!(st.session.journal.view.is_none(), "loading a repository");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refresh_rereads_it_only_while_open() {
    let (mut st, dir) = opened("journal-refresh");
    let refreshed = |st: &mut AppState| {
        let snap = Box::new(repo::load(&dir, &LoadOptions::all(100)).unwrap());
        st.loading = true;
        let m = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Refresh, result: Ok(snap) });
        update(st, m, Instant::now())
    };
    assert!(loads(&refreshed(&mut st)).is_empty(), "closed: nothing to read");
    run(&mut st, JournalMsg::Open);
    assert_eq!(loads(&refreshed(&mut st)).len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn it_is_parked_with_its_tab_and_reread_on_return() {
    let (mut st, dir) = opened("journal-tabs");
    run(&mut st, JournalMsg::Open);
    let before = st.session.journal.generation;
    drive(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert!(st.session.journal.view.is_none(), "the new tab has none");
    let fx = update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    let v = st.session.journal.view.as_ref().expect("back with its tab");
    assert!(v.loading);
    let again = loads(&fx);
    assert_eq!(again.len(), 1, "the load in flight is asked for again");
    assert!(again[0] > before && v.generation == again[0]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_result_for_an_inactive_tab_is_dropped() {
    let (mut st, dir) = opened("journal-inactive");
    run(&mut st, JournalMsg::Open);
    let g = st.session.journal.generation;
    let other = glowfly_core::paths::temp_dir().join("glowfly-journal-elsewhere");
    let msg = Msg::Journal(JournalMsg::Loaded { generation: g, result: Ok(timeline(&dir)) });
    update(&mut st, Msg::Scoped { workdir: other, msg: Box::new(msg) }, Instant::now());
    assert!(st.session.journal.view.as_ref().unwrap().timeline.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_palette_lists_it_only_with_a_repository() {
    let has = |st: &AppState| shown(st, "").iter().any(|s| s.entry.label == "Open operation journal");
    assert!(!has(&state()));
    let (st, dir) = opened("journal-palette");
    assert!(has(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_reset_away_state_is_marked_unreachable() {
    let (mut st, dir) = opened("journal-unreachable");
    let second = st.snap.as_ref().unwrap().head_oid.unwrap();
    git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    open_loaded(&mut st, &dir);
    let rows = st.session.journal.view.as_ref().unwrap().rows();
    let marked: Vec<_> = rows.iter().filter(|r| r.oid == second).collect();
    assert!(!marked.is_empty() && marked.iter().all(|r| r.unreachable), "{rows:#?}");
    assert!(rows.iter().filter(|r| r.current).all(|r| !r.unreachable));
    let _ = std::fs::remove_dir_all(&dir);
}
