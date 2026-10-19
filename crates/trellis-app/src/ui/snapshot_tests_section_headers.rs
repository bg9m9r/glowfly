//! The left-panel section headers' + and ⋯: shown while the header is hovered or focused, still
//! reachable by name while hidden, the ⋯ menu, a disabled + with its reason, the Git Flow +'s
//! Start menu, the WORKTREES and SUBMODULES empty states (hidden while filtering), and the
//! REMOTE count of remotes.

use crate::gitflow::GitFlowMsg;
use crate::msg::{BranchMsg, Msg};
use crate::remote::RemoteMsg;
use crate::submodule::SubmoduleMsg;
use crate::ui::snapshot_check::CheckSnapshot;
use crate::ui::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::ui::snapshot_tests_gitflow::initialized;
use crate::ui::snapshot_tests_pr_no_host::rest_on;
use crate::ui::snapshot_tests_select::click;
use crate::ui::snapshot_tests_worktrees::{H, center, pushed};
use crate::worktree::WorktreeMsg;
use eframe::egui::{Event, PointerButton, pos2};
use egui_kittest::kittest::Queryable;
use trellis_core::gitflow::FlowKind;
use trellis_core::repo::RemoteInfo;
use trellis_core::settings::ThemeChoice;
use trellis_core::worktree_ops::WorktreeBase;

/// Moves the pointer onto the header whose + is named `add`, left of its buttons.
fn hover_header(h: &mut H, add: &str) {
    let at = center(h, add);
    h.input_mut().events.push(Event::PointerMoved(pos2(at.x - 80.0, at.y)));
    h.run();
}

#[test]
fn section_header_hover() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "header-hover");
    let mut h = harness(st, true);
    h.run();
    hover_header(&mut h, "New branch");
    h.check_snapshot("section_header_hover");
    let at = center(&h, "New branch");
    click(&mut h, at, PointerButton::Primary);
    assert!(pushed(&h, &|m| matches!(m, Msg::Branch(BranchMsg::NewBranch { start: None, .. }))));
    assert!(h.state().0.view.sections.local, "the + leaves the section open");
    let at = center(&h, "Local actions");
    click(&mut h, at, PointerButton::Primary);
    assert!(h.state().0.view.sections.local, "the ⋯ leaves the section open");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn section_header_hidden() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "header-hidden");
    let mut h = harness(st, true);
    h.run();
    // Not painted, but reachable by name.
    h.check_snapshot("section_header_hidden");
    assert!(h.query_by_label("New branch").is_some() && h.query_by_label("Local actions").is_some());
    h.get_by_label("New branch").focus();
    h.run();
    h.check_snapshot("section_header_focus");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn section_header_more_menu() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "header-more");
    let mut h = harness(st, true);
    h.run();
    let at = center(&h, "Local actions");
    click(&mut h, at, PointerButton::Primary);
    assert!(h.query_by_label("New branch…").is_some(), "the menu is open");
    h.check_snapshot("section_header_more_menu");
    h.get_by_label("New branch…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Branch(BranchMsg::NewBranch { start: None, .. }))));
    assert!(h.query_by_label("New branch…").is_none(), "the menu closed");
    assert!(h.state().0.view.sections.local);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn stashes_header_blocked() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "header-stash-blocked");
    let snap = st.snap.as_mut().unwrap();
    snap.staged.clear();
    snap.unstaged.clear();
    let mut h = harness(st, true);
    h.run();
    assert!(h.get_by_label("Stash changes").is_disabled());
    let at = center(&h, "Stash changes");
    rest_on(&mut h, at);
    assert!(h.query_by_label_contains("No changes to stash").is_some(), "the reason shows on hover");
    h.check_snapshot("stashes_header_blocked");
    click(&mut h, at, PointerButton::Primary);
    assert!(!pushed(&h, &|m| matches!(m, Msg::Stash(_))), "a disabled + sends nothing");
    assert!(h.state().0.view.sections.stashes, "a disabled + leaves the section open");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn gitflow_header_start_menu() {
    let (st, dir) = initialized(ThemeChoice::Dark, "header-gitflow");
    let mut h = harness(st, true);
    h.run();
    let at = center(&h, "Start Git Flow branch");
    click(&mut h, at, PointerButton::Primary);
    for item in ["Start feature…", "Start release…", "Start hotfix…"] {
        assert!(h.query_by_label(item).is_some(), "{item}");
    }
    assert!(h.state().1.is_empty(), "the + only opens its menu");
    h.check_snapshot("gitflow_header_start_menu");
    h.get_by_label("Start feature…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::GitFlow(GitFlowMsg::StartDialog(FlowKind::Feature)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn worktrees_submodules_empty() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "header-empty");
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Add worktree…").is_some() && h.query_by_label("Add submodule…").is_some());
    h.check_snapshot("worktrees_submodules_empty");
    h.get_by_label("Add worktree…").click();
    h.run();
    let base = WorktreeBase::Branch("main".into());
    assert!(pushed(&h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::CreateDialog(b)) if *b == base)));
    h.get_by_label("Add submodule…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Submodule(SubmoduleMsg::AddDialog))));
    drop(h);
    let (st, _) = with_repo(ThemeChoice::Light, "header-empty");
    render("worktrees_submodules_empty_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn empty_sections_filtered() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "header-empty-filtered");
    st.view.ref_filter = "zzz".into();
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Worktrees actions").is_some() && h.query_by_label("Submodules actions").is_some(), "the headers stay");
    assert!(h.query_by_label("Add worktree…").is_none() && h.query_by_label("Add submodule…").is_none());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn remote_count() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "header-remote-count");
    let snap = st.snap.as_mut().unwrap();
    let branches: Vec<_> = snap.locals.iter().filter_map(|b| Some((b.name.clone(), b.oid?))).take(3).collect();
    assert_eq!(branches.len(), 3);
    snap.remotes = vec![("origin".into(), branches), ("backup".into(), Vec::new())];
    let info =
        |name: &str, url: &str| RemoteInfo { name: name.into(), urls: vec![url.into()], push_urls: Vec::new(), skip_fetch_all: false };
    snap.remote_info = vec![info("origin", "https://example.com/team/app.git"), info("backup", "https://example.com/team/backup.git")];
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("remote_count");
    h.get_by_label("Add remote").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Remote(RemoteMsg::AddDialog))));
    drop(h);
    remove_fixture(&dir);
}

/// A header with a disabled + and no ⋯ (so no context menu that would take the click): clicking
/// the + leaves the section open and does not open the +'s menu.
#[test]
fn blocked_add_without_menu_keeps_section() {
    use crate::ui::widgets::section_header::{AddAction, Header, HeaderAdd, show};
    use eframe::egui;
    use egui_kittest::Harness;
    let mut h = Harness::builder().with_size(egui::vec2(320.0, 120.0)).build_ui_state(
        |ui, (open, msgs): &mut (bool, Vec<Msg>)| {
            let items = |ui: &mut egui::Ui, msgs: &mut Vec<Msg>| {
                if ui.button("Probe item").clicked() {
                    msgs.push(Msg::Remote(RemoteMsg::AddDialog));
                }
            };
            let add = HeaderAdd { name: "Probe add".into(), tip: None, blocked: Some("Not now".into()), action: AddAction::Menu(&items) };
            show(ui, Header { title: "Probe", count: Some(1), add: Some(add), menu: None }, open, msgs);
        },
        (true, Vec::new()),
    );
    h.run();
    let b = h.get_by_label("Probe add").bounding_box().expect("the +");
    let at = pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerMoved(at));
        h.input_mut().events.push(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Default::default() });
        h.step();
    }
    h.run();
    assert!(h.state().0, "a disabled + leaves the section open");
    assert!(h.query_by_label("Probe item").is_none(), "a disabled + opens no menu");
    // A click on the row itself still toggles it.
    let row = pos2(b.x0 as f32 - 120.0, at.y);
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerMoved(row));
        h.input_mut().events.push(Event::PointerButton {
            pos: row,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
        h.step();
    }
    h.run();
    assert!(!h.state().0, "the row toggles");
}
