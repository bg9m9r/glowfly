//! The interactive rebase panel: its rows and actions, the message editor, a blocked Start,
//! the header buttons, Esc, and dragging a row. Shares the fixture repository with
//! `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::{UiCache, avatars::Avatars, draw, theme::color};
use crate::snapshot_check::CheckSnapshot;
use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::rebase::{RebaseAction, RebaseBase, RebasePlan, TodoItem, TodoStep, Upstream};
use glowfly_core::settings::ThemeChoice;
use glowfly_core::spell::Speller;
use glowfly_model::msg::Msg;
use glowfly_model::msg::RebaseMsg;
use glowfly_model::msg::RepoMsg;
use glowfly_model::state::AppState;
use glowfly_model::state::rebase::RebasePanel;
use std::path::PathBuf;

const NEW_MESSAGE: &str = "Add the app skeleton\n\nProject layout, CI and a README.";

/// The fixture's commits, oldest first, in a loaded panel with these actions (one per commit,
/// from the oldest). A clean working tree, so only the list decides whether Start is enabled.
pub(crate) fn with_panel(theme: ThemeChoice, name: &str, actions: &[RebaseAction]) -> (AppState, PathBuf) {
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
            summary: c.summary.to_string(),
            message: if c.body.is_empty() { c.summary.to_string() } else { format!("{}\n\n{}", c.summary, c.body) },
            new_message: None,
            combined_message: None,
            author: c.author.name().to_owned(),
            time: c.time,
        })
        .collect();
    let plan = RebasePlan {
        head: snap.head_oid.unwrap(),
        branch: "main".into(),
        upstream: Upstream::Root,
        onto_label: String::new(),
        steps: items.iter().cloned().map(TodoStep::Commit).collect(),
        held: Vec::new(),
    };
    let mut panel = RebasePanel::loading(RebaseBase::FromCommit(items[0].oid));
    panel.fill(plan);
    panel.items = items.into_iter().map(TodoStep::Commit).collect();
    st.session.rebase.view = Some(panel);
    (st, dir)
}

#[test]
fn rebase_panel() {
    use RebaseAction::*;
    let (mut st, dir) = with_panel(ThemeChoice::Dark, "rebase-panel", &[Pick, Reword, Edit, Squash, Fixup, Drop]);
    let p = st.session.rebase.view.as_mut().unwrap();
    p.items[1].commit_mut().unwrap().new_message = Some(NEW_MESSAGE.into());
    p.open_editor(1);
    assert_eq!(p.draft, NEW_MESSAGE);
    assert_eq!(p.subtitle(glowfly_core::i18n::Lang::En), "Rebasing 6 commits of main from the first commit.");
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
    st.snap.as_mut().unwrap().unstaged.push(glowfly_core::repo::FileEntry::new("README.md", 'M'));
    let blocker = st.session.rebase.view.as_ref().unwrap().start_blocker(st.snap.as_ref().unwrap(), false);
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
    let blocker = st.session.rebase.view.as_ref().unwrap().start_blocker(st.snap.as_ref().unwrap(), false);
    assert_eq!(blocker.as_deref(), Some(glowfly_core::rebase::FIRST_COMBINES));
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
pub(crate) fn harness_with_keys(st: AppState) -> Harness<'static, (AppState, Vec<Msg>)> {
    let mut st = st;
    st.view.now = 1_769_947_200;
    let mut cache: Option<UiCache> = None;
    crate::snapshot_check::builder().with_size(egui::vec2(1400.0, 820.0)).build_state(
        move |ctx, (st, msgs): &mut (AppState, Vec<Msg>)| {
            color::set_dark(true);
            super::theme::apply_theme(ctx);
            let cache = cache.get_or_insert_with(|| UiCache {
                avatars: Avatars::default(),
                speller: Speller::new(),
                diff_rows: Default::default(),
                diff_header: Default::default(),
                history_text: Default::default(),
                history_preview: Default::default(),
                blame_rows: Default::default(),
                diff_images: Default::default(),
                markdown: Default::default(),
                ref_upstreams: Default::default(),
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
    // The header's buttons come in reading order (Tab follows the tree), left to right on screen.
    let want = ["More rebase actions", "Cancel", "Start rebase"];
    let buttons: Vec<(String, f64)> = h
        .query_all_by_role(egui::accesskit::Role::Button)
        .filter_map(|n| Some((n.label()?, n.bounding_box()?.x0)))
        .filter(|(l, _)| want.contains(&l.as_str()))
        .collect();
    assert_eq!(buttons.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(), want, "tree order");
    assert!(buttons.windows(2).all(|w| w[0].1 < w[1].1), "left to right: {buttons:?}");
    h.get_by_label("Start rebase").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false }))));
    // Restore original order is in ⋯, and off while the list is as it was.
    h.get_by_label("More rebase actions").click();
    h.run();
    assert!(h.get_by_label("Restore original order").is_disabled(), "nothing has changed yet");
    h.press_key(Key::Escape);
    h.run();
    h.state_mut().1.clear();
    h.state_mut().0.session.rebase.view.as_mut().unwrap().items.swap(0, 1);
    h.run();
    h.get_by_label("More rebase actions").click();
    h.run();
    h.get_by_label("Restore original order").click();
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

pub(super) fn pointer(h: &mut Harness<'_, (AppState, Vec<Msg>)>, pos: Pos2, button: Option<bool>) {
    let events = &mut h.input_mut().events;
    events.push(Event::PointerMoved(pos));
    if let Some(pressed) = button {
        events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
}

/// Presses on the first row's grip and drags it two rows down, still held: returns where.
fn drag_two_down(h: &mut Harness<'_, (AppState, Vec<Msg>)>) -> Pos2 {
    // Rows are labelled with their commit's summary.
    let bounds = |h: &Harness<'_, (AppState, Vec<Msg>)>, label: &str| {
        let b = h.get_by_label(label).bounding_box().expect("the row has bounds");
        (b.x0 as f32, b.y0 as f32, b.y1 as f32)
    };
    let (x0, top0, bottom0) = bounds(h, "Initial commit");
    let (_, top1, _) = bounds(h, "Add app skeleton");
    let pitch = top1 - top0;
    let start = pos2(x0 + 16.0, (top0 + bottom0) / 2.0);
    pointer(h, start, Some(true));
    h.step();
    pointer(h, start + egui::vec2(0.0, 12.0), None);
    h.step();
    let over = start + egui::vec2(0.0, 2.0 * pitch + 6.0);
    pointer(h, over, None);
    h.step();
    h.run();
    over
}

#[test]
fn row_drag_moves_and_snapshots() {
    use RebaseAction::*;
    let (st, dir) = with_panel(ThemeChoice::Dark, "rebase-drag", &[Pick, Pick, Squash, Pick, Drop, Pick]);
    let mut h = harness(st, true);
    h.run();
    let over = drag_two_down(&mut h);
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

/// The lifted row in a light theme: the raised fill on the lighter `Lift` shadow.
#[test]
fn rebase_dragging_light() {
    use RebaseAction::*;
    let (st, dir) = with_panel(ThemeChoice::Light, "rebase-drag-light", &[Pick, Pick, Squash, Pick, Drop, Pick]);
    let mut h = harness(st, false);
    h.run();
    drag_two_down(&mut h);
    h.check_snapshot("rebase_dragging_light");
    drop(h);
    remove_fixture(&dir);
}
