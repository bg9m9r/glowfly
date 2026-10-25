//! Several commits selected: the graph rows, the details panel, the two-commit diff, the
//! multi-commit menu and Esc. Shares the fixture repository and renderer with `snapshot_tests`.

use super::menu_testing::{open_submenu, submenu_item};
use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::SelectionMsg;
use crate::patch::PatchMsg;
use crate::state::selection::{CommitBlock, MultiSelection};
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::intraline;
use glowfly_core::repo::{self, DiffTarget};
use glowfly_core::settings::ThemeChoice;
use std::path::PathBuf;

/// The fixture's commit with this summary.
fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap().oid
}

/// `summaries` selected, as Ctrl-clicks would leave them, with the two-commit file list (or
/// the stacked list) loaded.
pub(in crate::ui) fn with_selection(theme: ThemeChoice, name: &str, summaries: &[&str]) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let oids: Vec<Oid> = summaries.iter().map(|s| commit(&st, s)).collect();
    let mut sel = MultiSelection::build(oids.iter().copied(), oids[0], st.snap.as_ref().unwrap());
    if let Selection::Commits(m) = &mut sel {
        if let Some((older, newer)) = m.compare_ends() {
            m.changes = Some(repo::compare(&dir, older, newer).map_err(|e| e.to_string()));
        }
        load_stacked(m, &dir);
    }
    st.selection = sel;
    (st, dir)
}

/// Fills in the stacked list, as the runner would, when the selection has one.
pub(in crate::ui) fn load_stacked(m: &mut MultiSelection, dir: &std::path::Path) {
    if let Some(oids) = m.stack_oids().map(<[Oid]>::to_vec) {
        m.stacked = Some(repo::stacked_changes(dir, &oids).map_err(|e| e.to_string()));
    }
}

fn multi(st: &AppState) -> &MultiSelection {
    let Selection::Commits(m) = &st.selection else { panic!("not a multi-selection") };
    m
}

#[test]
fn graph_two_commits_selected() {
    let (st, dir) = with_selection(ThemeChoice::Dark, "multi-two", &["Add login page", "Search results page"]);
    let m = multi(&st);
    let names: Vec<(&str, char)> =
        m.changes.as_ref().unwrap().as_ref().unwrap().files.iter().map(|f| (f.path.as_str(), f.status)).collect();
    assert_eq!(names, [("app.txt", 'M'), ("search.txt", 'A')]);
    let (older, newer) = m.compare_ends().unwrap();
    assert_eq!((older, newer), (commit(&st, "Add login page"), commit(&st, "Search results page")));
    render("graph_two_commits_selected", st, true);

    // The same selection with the diff of app.txt between the two open.
    let (mut st, _) = with_selection(ThemeChoice::Dark, "multi-two", &["Add login page", "Search results page"]);
    let target = DiffTarget::Between(older, newer, "app.txt".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff.into()));
    render("graph_two_commits_diff", st, true);
    remove_fixture(&dir);
}

#[test]
fn graph_many_commits_selected_light() {
    let (st, dir) =
        with_selection(ThemeChoice::Light, "multi-many", &["Merge branch 'login-page'", "Fix build on Windows", "Add login page"]);
    let m = multi(&st);
    assert_eq!(m.len(), 3);
    assert_eq!(m.pick_blocked, Some(CommitBlock::PickOnHead), "they are on the current branch");
    assert_eq!(m.gap(), Some(commit(&st, "Fix build on Windows")), "its parent isn't selected");
    assert!(m.stacked.as_ref().is_some_and(|s| s.as_ref().is_ok_and(|s| s.len() == 3)));
    render("graph_many_commits_selected_light", st, false);
    remove_fixture(&dir);
}

pub(in crate::ui) fn click(h: &mut Harness<'_, (AppState, Vec<Msg>)>, pos: Pos2, button: PointerButton) {
    for pressed in [true, false] {
        let events = &mut h.input_mut().events;
        events.push(Event::PointerMoved(pos));
        events.push(Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE });
        h.step();
    }
    // A loading pane's spinner keeps repainting; stop at the step limit then.
    h.run_ok();
}

/// A primary double-click at `pos` with `modifiers` held. Both clicks go in one frame: each
/// harness step is a quarter second, past the double-click delay.
pub(in crate::ui) fn double_click(h: &mut Harness<'_, (AppState, Vec<Msg>)>, pos: Pos2, modifiers: Modifiers) {
    h.input_mut().modifiers = modifiers;
    let events = &mut h.input_mut().events;
    events.push(Event::PointerMoved(pos));
    for pressed in [true, false, true, false] {
        events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers });
    }
    h.run_ok();
    h.input_mut().modifiers = Modifiers::NONE;
}

/// The y of graph row `row` (the WIP row is 0) in the window the harness draws.
pub(in crate::ui) fn row_y(row: usize) -> f32 {
    use crate::ui::graph::ROW_H;
    ROWS_TOP + ROW_H * row as f32 + ROW_H / 2.0
}

/// Where the graph's first row starts (below the tab bar, toolbar and column header).
const ROWS_TOP: f32 = 117.0;

#[test]
fn graph_multi_menu() {
    let (mut st, dir) = with_selection(ThemeChoice::Dark, "multi-menu", &["Add login page", "Search results page"]);
    // As on a branch without these commits, so the cherry-pick is offered.
    if let Selection::Commits(m) = &mut st.selection {
        m.pick_blocked = None;
    }
    let (login, search) = (commit(&st, "Add login page"), commit(&st, "Search results page"));
    let row_of = |o: Oid| st.snap.as_ref().unwrap().row_of[&o] + 1;
    let (search_row, fix_row) = (row_of(search), row_of(commit(&st, "Fix build on Windows")));
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(700.0, row_y(search_row)), PointerButton::Secondary);
    h.check_snapshot("graph_multi_menu");
    assert_eq!(h.get_all_by_label("2 commits selected").count(), 1, "the menu's header");
    assert!(h.query_by_label("2 commits selected · Esc to clear").is_some(), "the details panel's title");
    h.get_by_label("Cherry-pick").click();
    h.run();
    let picked = h.state().1.iter().find_map(|m| if let Msg::Selection(SelectionMsg::CherryPick(o)) = m { Some(o.clone()) } else { None });
    assert_eq!(picked, Some(vec![search, login]));
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::SelectCommit(_)))), "a right-click does not change the selection");

    // The menu offers the selection as one patch.
    click(&mut h, pos2(700.0, row_y(search_row)), PointerButton::Secondary);
    h.state_mut().1.clear();
    h.get_by_label("Create patch…").click();
    h.run();
    let exported = h.state().1.iter().find_map(|m| if let Msg::Patch(PatchMsg::Export(o)) = m { Some(o.clone()) } else { None });
    assert_eq!(exported, Some(vec![search, login]));
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::SelectCommit(_)))), "a right-click does not change the selection");
    // Or copies it.
    click(&mut h, pos2(700.0, row_y(search_row)), PointerButton::Secondary);
    h.state_mut().1.clear();
    open_submenu(&mut h, "Copy");
    submenu_item(&h, "Copy", "Patch").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Patch(PatchMsg::Copy(o)) if *o == [search, login])));
    // And their SHAs, one per line.
    click(&mut h, pos2(700.0, row_y(search_row)), PointerButton::Secondary);
    h.state_mut().1.clear();
    open_submenu(&mut h, "Copy");
    submenu_item(&h, "Copy", "SHAs").click();
    h.run();
    let shas = format!("{search}\n{login}");
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Copy(t) if *t == shas)));

    // Any other row gets the single-commit menu.
    click(&mut h, pos2(700.0, row_y(fix_row)), PointerButton::Secondary);
    assert!(h.query_by_label("Cherry-pick").is_some());
    assert!(h.query_by_label("2 commits selected").is_none(), "the single-commit menu has its own header");
    assert_eq!(h.query_all_by_label("Cherry-pick 2 commits").count(), 1, "only the details panel's button");
    drop(h);
    remove_fixture(&dir);
}

/// A harness that only runs the shell's Esc handling.
fn esc_harness(st: AppState) -> Harness<'static, (AppState, Vec<Msg>)> {
    crate::ui::snapshot_check::builder()
        .build_state(|ctx, (st, msgs): &mut (AppState, Vec<Msg>)| super::escape_key(ctx, st, msgs), (st, Vec::new()))
}

fn press_esc(h: &mut Harness<'_, (AppState, Vec<Msg>)>) {
    let key = |pressed| Event::Key { key: egui::Key::Escape, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
    h.input_mut().events.extend([key(true), key(false)]);
    h.run();
}

#[test]
fn esc_collapses_after_closing_the_diff() {
    let (st, dir) = with_selection(ThemeChoice::Dark, "multi-esc", &["Add login page", "Search results page"]);
    let mut h = esc_harness(st);
    press_esc(&mut h);
    assert!(matches!(&h.state().1[..], [Msg::Selection(SelectionMsg::CollapseSelection)]));

    let (mut st, _) = with_selection(ThemeChoice::Dark, "multi-esc", &["Add login page", "Search results page"]);
    let (older, newer) = multi(&st).compare_ends().unwrap();
    let target = DiffTarget::Between(older, newer, "app.txt".into());
    st.diff = Some((target.clone(), repo::load_diff(&dir, &target).unwrap().into()));
    let mut h = esc_harness(st);
    press_esc(&mut h);
    assert!(matches!(&h.state().1[..], [Msg::Repo(RepoMsg::CloseDiff)]));
    remove_fixture(&dir);
}

#[test]
fn graph_commit_menu_reset_submenu() {
    // The submenu opens beside the menu at its items' full width, one line each.
    let (st, dir) = with_repo(ThemeChoice::Dark, "commit-menu-reset");
    let fix = commit(&st, "Fix build on Windows");
    let row = st.snap.as_ref().unwrap().row_of[&fix] + 1;
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(700.0, row_y(row)), PointerButton::Secondary);
    open_submenu(&mut h, "Reset main to this commit");
    h.check_snapshot("graph_commit_menu_reset_submenu");
    let reset = ["commits.menu.reset.soft", "commits.menu.reset.mixed", "commits.menu.reset.hard", "commits.menu.reset.hard_hint"];
    for item in reset.map(|k| Lang::En.tr(k)) {
        assert!(h.query_by_label(item).is_some(), "{item}");
    }
    h.get_by_label(Lang::En.tr("commits.menu.reset.hard")).click();
    h.run();
    // The fixture has uncommitted changes: the reset goes straight on and the auto-stash prompt
    // asks, so the user is asked once.
    let sent = h.state().1.iter().any(|m| matches!(m, Msg::Job(JobMsg::Git { cmds, .. }) if cmds[0][..2] == ["reset", "--hard"]));
    assert!(sent, "{:?}", h.state().1.len());
    drop(h);

    // On a clean tree the danger confirmation asks, naming the commit.
    let (mut st, clean_dir) = with_repo(ThemeChoice::Dark, "commit-menu-reset-clean");
    let snap = st.snap.as_mut().unwrap();
    (snap.staged, snap.unstaged) = (Vec::new(), Vec::new());
    let row = st.snap.as_ref().unwrap().row_of[&fix] + 1;
    let mut h = harness(st, true);
    h.run();
    click(&mut h, pos2(700.0, row_y(row)), PointerButton::Secondary);
    open_submenu(&mut h, "Reset main to this commit");
    h.get_by_label(Lang::En.tr("commits.menu.reset.hard")).click();
    h.run();
    let sha = fix.to_string()[..7].to_owned();
    let named = format!("\"Fix build on Windows\" ({sha})");
    let asked = h.state().1.iter().any(
        |m| matches!(m, Msg::ConfirmDanger { title, body, .. } if title == "Hard reset" && body.contains(&named) && body.contains("Untracked files")),
    );
    assert!(asked, "the confirmation names the commit by its summary");
    drop(h);
    remove_fixture(&dir);
    remove_fixture(&clean_dir);
}

#[test]
fn graph_commit_menu() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "commit-menu");
    let (fix, merge) = (commit(&st, "Fix build on Windows"), commit(&st, "Merge branch 'login-page'"));
    let row_of = |o: Oid| st.snap.as_ref().unwrap().row_of[&o] + 1;
    let (fix_row, merge_row) = (row_of(fix), row_of(merge));
    let root_row = row_of(commit(&st, "Initial commit"));
    let search = commit(&st, "Search results page");
    let search_row = row_of(search);
    let sha = |o: Oid| o.to_string()[..7].to_owned();
    let mut h = harness(st, true);
    h.run();
    // A commit main doesn't contain: main can be rebased onto it, plainly or interactively.
    click(&mut h, pos2(700.0, row_y(search_row)), PointerButton::Secondary);
    h.check_snapshot("graph_commit_menu");
    // Groups in order: branch and tag, integrate, rewrite, view, copy.
    let order = [
        "Checkout (detached)",
        "Create tag here…",
        "Cherry-pick",
        "Interactive rebase from here…",
        "Compare with working directory",
        "Create patch…",
    ];
    let ys: Vec<f64> = order.iter().map(|l| h.get_by_label(l).bounding_box().expect("a laid-out item").y0).collect();
    assert!(ys.windows(2).all(|w| w[0] < w[1]), "{order:?} top to bottom, got {ys:?}");
    assert!(!h.get_by_label("Rebase main onto this commit").is_disabled());
    h.state_mut().1.clear();
    h.get_by_label("Interactive rebase main onto this commit…").click();
    h.run();
    let want = glowfly_core::rebase::RebaseBase::OntoCommit(search);
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Rebase(crate::msg::RebaseMsg::OpenRebase(b)) if *b == want)));

    click(&mut h, pos2(700.0, row_y(fix_row)), PointerButton::Secondary);
    assert!(h.query_by_label(&format!("Fix build on Windows  {}", sha(fix))).is_some(), "the header");
    assert!(h.get_by_label("Rebase main onto this commit").is_disabled(), "main already contains it");
    h.state_mut().1.clear();
    h.get_by_label("Create patch…").click();
    h.run();
    let exported = h.state().1.iter().find_map(|m| if let Msg::Patch(PatchMsg::Export(o)) = m { Some(o.clone()) } else { None });
    assert_eq!(exported, Some(vec![fix]));
    click(&mut h, pos2(700.0, row_y(fix_row)), PointerButton::Secondary);
    h.state_mut().1.clear();
    open_submenu(&mut h, "Copy");
    submenu_item(&h, "Copy", "Patch").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Patch(PatchMsg::Copy(o)) if *o == [fix])));

    click(&mut h, pos2(700.0, row_y(fix_row)), PointerButton::Secondary);
    open_submenu(&mut h, "Copy");
    submenu_item(&h, "Copy", "Link").click();
    h.run();
    let want = crate::deep_link::LinkTarget::Commit(fix);
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::DeepLink(crate::deep_link::DeepLinkMsg::Copy(t)) if *t == want)));

    // A merge commit has no patch of its own.
    click(&mut h, pos2(700.0, row_y(merge_row)), PointerButton::Secondary);
    assert!(h.get_by_label("Create patch…").is_disabled());
    // History edits need a linear run up to HEAD, which is a merge here.
    assert!(h.get_by_label("Squash into parent…").is_disabled());
    open_submenu(&mut h, "Copy");
    assert!(submenu_item(&h, "Copy", "Patch").is_disabled());
    click(&mut h, pos2(700.0, row_y(root_row)), PointerButton::Secondary);
    assert!(h.get_by_label("Squash into parent…").is_disabled());
    assert!(h.get_by_label("Reword…").is_disabled());
    drop(h);
    remove_fixture(&dir);
}
