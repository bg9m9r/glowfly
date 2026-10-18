//! The interactive rebase panel: its rows and actions, the message editor, a blocked Start,
//! the header buttons, Esc, and dragging a row. Shares the fixture repository with
//! `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::{UiCache, avatars::Avatars, draw, theme::color};
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::msg::RepoMsg;
use crate::state::AppState;
use crate::state::rebase::RebasePanel;
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::rebase::{RebaseAction, RebaseBase, RebasePlan, TodoItem, TodoStep, Upstream};
use trellis_core::settings::ThemeChoice;
use trellis_core::spell::Speller;

const NEW_MESSAGE: &str = "Add the app skeleton\n\nProject layout, CI and a README.";

/// The fixture's commits, oldest first, in a loaded panel with these actions (one per commit,
/// from the oldest). A clean working tree, so only the list decides whether Start is enabled.
pub(super) fn with_panel(theme: ThemeChoice, name: &str, actions: &[RebaseAction]) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged.clear();
    let items: Vec<TodoItem> = snap
        .commits
        .iter()
        .rev()
        .zip(actions)
        .map(|(c, a)| TodoItem {
            oid: c.oid,
            action: *a,
            summary: c.summary.clone(),
            message: if c.body.is_empty() { c.summary.clone() } else { format!("{}\n\n{}", c.summary, c.body) },
            new_message: None,
            combined_message: None,
            author: c.author.clone(),
            time: c.time,
        })
        .collect();
    let plan = RebasePlan {
        head: snap.head_oid.unwrap(),
        branch: "main".into(),
        upstream: Upstream::Root,
        onto_label: String::new(),
        steps: items.iter().cloned().map(TodoStep::Commit).collect(),
    };
    let mut panel = RebasePanel::loading(RebaseBase::FromCommit(items[0].oid));
    panel.fill(plan);
    panel.items = items.into_iter().map(TodoStep::Commit).collect();
    st.rebase = Some(panel);
    (st, dir)
}

#[test]
fn rebase_panel() {
    use RebaseAction::*;
    let (mut st, dir) = with_panel(ThemeChoice::Dark, "rebase-panel", &[Pick, Reword, Edit, Squash, Fixup, Drop]);
    let p = st.rebase.as_mut().unwrap();
    p.items[1].commit_mut().unwrap().new_message = Some(NEW_MESSAGE.into());
    p.open_editor(1);
    assert_eq!(p.draft, NEW_MESSAGE);
    assert_eq!(p.subtitle(), "Rebasing 6 commits of main from the first commit");
    let blocker = p.start_blocker(st.snap.as_ref().unwrap(), false);
    assert_eq!(blocker, None, "Start is enabled");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("rebase_panel");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rebase_panel_dirty() {
    use RebaseAction::*;
    let (mut st, dir) = with_panel(ThemeChoice::Dark, "rebase-dirty", &[Pick, Pick, Drop, Pick, Pick, Pick]);
    st.snap.as_mut().unwrap().unstaged.push(trellis_core::repo::FileEntry::new("README.md", 'M'));
    let blocker = st.rebase.as_ref().unwrap().start_blocker(st.snap.as_ref().unwrap(), false);
    assert_eq!(blocker, None, "uncommitted changes are stashed, not refused");
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("rebase_panel_dirty");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn rebase_panel_blocked_light() {
    use RebaseAction::*;
    let (st, dir) = with_panel(ThemeChoice::Light, "rebase-blocked", &[Squash, Pick, Drop, Pick, Pick, Pick]);
    let blocker = st.rebase.as_ref().unwrap().start_blocker(st.snap.as_ref().unwrap(), false);
    assert_eq!(blocker.as_deref(), Some(trellis_core::rebase::FIRST_COMBINES));
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("rebase_panel_blocked_light");
    h.get_by_label("Start rebase").click();
    h.run();
    assert!(!h.state().1.iter().any(|m| matches!(m, Msg::Rebase(RebaseMsg::StartRebase { .. }))), "a disabled Start sends nothing");
    drop(h);
    remove_fixture(&dir);
}

/// Like `snapshot_tests::harness`, but also running the shell's Esc handling.
pub(super) fn harness_with_keys(st: AppState) -> Harness<'static, (AppState, Vec<Msg>)> {
    let mut st = st;
    st.view.now = 1_769_947_200;
    let mut cache: Option<UiCache> = None;
    Harness::builder().with_size(egui::vec2(1400.0, 820.0)).build_state(
        move |ctx, (st, msgs): &mut (AppState, Vec<Msg>)| {
            color::set_dark(true);
            super::theme::apply_theme(ctx);
            let cache = cache.get_or_insert_with(|| UiCache {
                avatars: Avatars::default(),
                speller: Speller::new(),
                diff_rows: Default::default(),
            });
            draw(ctx, st, cache, msgs);
            super::escape_key(ctx, st, msgs);
        },
        (st, Vec::new()),
    )
}

#[test]
fn header_buttons_and_esc() {
    use RebaseAction::*;
    let (st, dir) = with_panel(ThemeChoice::Dark, "rebase-buttons", &[Pick, Pick, Drop, Pick, Pick, Pick]);
    let mut h = harness_with_keys(st);
    h.run();
    h.get_by_label("Start rebase").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false }))));
    h.get_by_label("Reset").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Rebase(RebaseMsg::RebaseReset))));
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Rebase(RebaseMsg::CloseRebase))));
    h.state_mut().1.clear();
    h.press_key(Key::Escape);
    h.run();
    assert!(matches!(h.state().1[..], [Msg::Rebase(RebaseMsg::CloseRebase)]), "Esc closes the panel");
    drop(h);
    remove_fixture(&dir);
}

fn pointer(h: &mut Harness<'_, (AppState, Vec<Msg>)>, pos: Pos2, button: Option<bool>) {
    let events = &mut h.input_mut().events;
    events.push(Event::PointerMoved(pos));
    if let Some(pressed) = button {
        events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
}

#[test]
fn row_drag_moves_and_snapshots() {
    use RebaseAction::*;
    let (st, dir) = with_panel(ThemeChoice::Dark, "rebase-drag", &[Pick, Pick, Squash, Pick, Drop, Pick]);
    let mut h = harness(st, true);
    h.run();
    // Rows are labelled with their commit's summary.
    let bounds = |h: &Harness<'_, (AppState, Vec<Msg>)>, label: &str| {
        let b = h.get_by_label(label).bounding_box().expect("the row has bounds");
        (b.x0 as f32, b.y0 as f32, b.y1 as f32)
    };
    let (x0, top0, bottom0) = bounds(&h, "Initial commit");
    let (_, top1, _) = bounds(&h, "Add app skeleton");
    let pitch = top1 - top0;
    let start = pos2(x0 + 16.0, (top0 + bottom0) / 2.0);
    pointer(&mut h, start, Some(true));
    h.step();
    pointer(&mut h, start + egui::vec2(0.0, 12.0), None);
    h.step();
    let over = start + egui::vec2(0.0, 2.0 * pitch + 6.0);
    pointer(&mut h, over, None);
    h.step();
    h.run();
    assert!(h.state().1.is_empty(), "nothing is sent while dragging");
    h.check_snapshot("rebase_dragging");

    pointer(&mut h, over, Some(false));
    h.run();
    let moves: Vec<(usize, usize)> = h
        .state()
        .1
        .iter()
        .filter_map(|m| if let Msg::Rebase(RebaseMsg::RebaseMove { from, to }) = m { Some((*from, *to)) } else { None })
        .collect();
    assert_eq!(moves, [(0, 2)]);
    assert!(
        !h.state().1.iter().any(|m| matches!(m, Msg::Repo(RepoMsg::SelectCommit(_)) | Msg::Rebase(RebaseMsg::RebaseSelect(_)))),
        "dragging does not select the row"
    );
    drop(h);
    remove_fixture(&dir);
}
