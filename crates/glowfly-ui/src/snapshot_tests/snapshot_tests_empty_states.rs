//! The left panel's empty sections: collapsed and muted at first, expanded by their own header
//! into a sentence and an action (Worktrees, Submodules, Stashes, Remote with Publish… and a quiet
//! Add remote…, wrapping in a narrow German panel), and the PULL REQUESTS section while its
//! account isn't connected (Connect…) or is connected but failing (Sign in), shared with ISSUES
//! only while PULL REQUESTS is open.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use super::snapshot_tests_forge_filters::target;
use super::snapshot_tests_language_main::german;
use crate::snapshot_check::CheckSnapshot;
use crate::snapshot_tests::snapshot_tests_select::click;
use crate::snapshot_tests::snapshot_tests_worktrees::{H, center, pushed};
use eframe::egui::PointerButton;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{Connection, Provider};
use glowfly_core::settings::{SizedPanel, ThemeChoice};
use glowfly_model::msg::{ForgeMsg, Msg, PrefsMsg, PublishMsg};
use glowfly_model::remote::RemoteMsg;
use glowfly_model::state::session_view::empty_sections::Folding;
use glowfly_model::state::{AppState, ForgeState, PrefPage, Status};
use glowfly_model::worktree::WorktreeMsg;

/// Opens `which` as if its header had been clicked while empty.
pub(crate) fn expand(st: &mut AppState, which: Folding) {
    let repo = st.snap.as_ref().unwrap().real_workdir.clone();
    let mut open = true;
    *st.session.tab_view.empty_sections.open_mut(&repo, which, 0, &mut open) = true;
}

/// Clicks the header whose ⋯ is named `more`, left of its buttons.
fn click_header(h: &mut H, more: &str) {
    let at = center(h, more);
    click(h, eframe::egui::pos2(at.x - 80.0, at.y), PointerButton::Primary);
}

#[test]
fn empty_sections_start_collapsed() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "empty-collapsed");
    let mut h = harness(st, true);
    h.run();
    for action in ["Add worktree…", "Add submodule…", "Stash changes…", "Publish…"] {
        assert!(h.query_by_label(action).is_none(), "{action} waits until its section is expanded");
    }
    click_header(&mut h, "Worktrees actions");
    assert!(h.state().0.view.sections.worktrees, "the populated section's flag is untouched");
    h.get_by_label("Add worktree…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Worktree(WorktreeMsg::CreateDialog(_)))));
    click_header(&mut h, "Worktrees actions");
    assert!(h.query_by_label("Add worktree…").is_none(), "collapsed again");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn empty_worktrees_expanded() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "empty-worktrees");
    expand(&mut st, Folding::Worktrees);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Check out another branch in its own folder and work on both at once, no stashing needed.").is_some());
    h.check_snapshot("sidebar_empty_worktrees");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn empty_remote_publish_or_add() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "empty-remote");
    expand(&mut st, Folding::Remote);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("sidebar_empty_remote");
    h.get_by_label("Publish…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Publish(PublishMsg::PublishDialog))));
    h.get_by_label("Add remote…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Remote(RemoteMsg::AddDialog))));
    drop(h);
    remove_fixture(&dir);
}

/// A narrow German panel: the two buttons wrap onto their own lines rather than clip, and the
/// headers below keep their counts in view.
#[test]
fn empty_remote_narrow_german() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "empty-remote-de");
    german(&mut st);
    st.settings.panel_sizes.set(SizedPanel::Sidebar, 240.0);
    expand(&mut st, Folding::Remote);
    let mut h = harness(st, true);
    h.run();
    let publish = h.get_by_label("Veröffentlichen…").bounding_box().unwrap();
    let add = h.get_by_label("Remote hinzufügen…").bounding_box().unwrap();
    assert!(add.y0 >= publish.y1, "Remote hinzufügen… wraps below Veröffentlichen…");
    assert!(add.x1 <= 240.0, "and stays inside the panel");
    h.check_snapshot("sidebar_empty_remote_de");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn empty_stashes_blocked() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "empty-stashes");
    let snap = st.snap.as_mut().unwrap();
    snap.staged.clear();
    snap.unstaged.clear();
    expand(&mut st, Folding::Stashes);
    let mut h = harness(st, true);
    h.run();
    assert!(h.get_by_label("Stash changes…").is_disabled(), "nothing to stash");
    h.check_snapshot("sidebar_empty_stashes");
    drop(h);
    remove_fixture(&dir);
}

/// The fixture on GitHub (pull requests and issues) whose service failed with `error`.
fn failing(name: &str, error: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let github = target(Provider::GitHub, "github.com");
    let mut f = ForgeState::new(Some(github.clone()), "origin".into(), Some(github));
    f.host_status = Status::Error(error.into());
    f.tracker_status = Status::Error(error.into());
    st.session.forge = Some(f);
    (st, dir)
}

#[test]
fn prs_not_connected() {
    let (st, dir) = failing("empty-prs-connect", "not connected to GitHub");
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Connect GitHub to see its pull requests here.").is_some());
    assert!(h.query_by_label("Connect GitHub to see its issues here.").is_some());
    // One for both sections, besides the status bar's.
    assert_eq!(h.query_all_by_label("Connect…").count(), 2, "one Connect… for both sections");
    assert_eq!(h.query_all_by_label("Sign in").count(), 1, "only the status bar's Sign in");
    h.check_snapshot("sidebar_prs_connect");
    h.get_all_by_label("Connect…").next().unwrap().click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Integrations)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn prs_connected_but_failing() {
    let (mut st, dir) = failing("empty-prs-sign-in", "GitHub refused the saved token (401)");
    let account = target(Provider::GitHub, "github.com").account();
    st.settings.connections = vec![Connection { account, user: "ada-l".into(), method: "access token".into() }];
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Connect GitHub to see its pull requests here.").is_none(), "already connected");
    assert_eq!(h.query_all_by_label("Sign in").count(), 2, "one for the account (the issues share it), and the status bar's");
    h.check_snapshot("sidebar_prs_sign_in");
    h.get_all_by_label("Sign in").next().unwrap().click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Forge(ForgeMsg::ForgeRefresh { interactive: true }))));
    drop(h);
    remove_fixture(&dir);
}

/// With PULL REQUESTS collapsed, ISSUES offers its own Connect….
#[test]
fn prs_collapsed_issues_connect() {
    let (mut st, dir) = failing("empty-prs-collapsed", "not connected to GitHub");
    st.view.sections.prs = false;
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Connect GitHub to see its pull requests here.").is_none(), "collapsed");
    assert!(h.query_by_label("Connect GitHub to see its issues here.").is_some());
    assert_eq!(h.query_all_by_label("Connect…").count(), 2, "the issues' own, and the status bar's");
    h.check_snapshot("sidebar_prs_collapsed_connect");
    drop(h);
    remove_fixture(&dir);
}
