//! Trello snapshots: its Integrations page (API key and token, no host or Git Credential
//! Manager sign-in), the Issue Tracker page's Board picker (open, and after a failed load) and
//! the ISSUES menu of a card.

use super::snapshot_tests::{harness, remove_fixture, settings, with_repo};
use super::snapshot_tests_gitflow::{center, initialized, pushed};
use super::snapshot_tests_select::click;
use crate::msg::{BranchMsg, ForgeMsg, Msg};
use crate::state::trello::BoardList;
use crate::state::{AppState, ForgeState, PrefPage, PrefsState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::PointerButton;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use glowfly_core::forge::{Account, Connection, Issue, Provider, Target, TrelloBoard};
use glowfly_core::settings::{ThemeChoice, TrackerChoice};
use std::time::Instant;

const BOARD: &str = "5f1a2b3c4d5e6f7a8b9c0d1e";

fn acct() -> Account {
    Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() }
}

fn connection() -> Connection {
    Connection { account: acct(), user: "Ada Lovelace".into(), method: "access token".into() }
}

#[test]
fn prefs_integrations_trello() {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::Integrations);
    (prefs.int_provider, prefs.int_host) = (Provider::Trello, "trello.com".into());
    prefs.int_user = "0123456789abcdef0123456789abcdef".into();
    st.prefs = Some(prefs);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("prefs_integrations_trello");
    // The row labels ("API Key", "Token") are painted, so the image shows them.
    for label in ["Get your API key", "Allow a read-only token", "Allow a token that can create cards", "Trello"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.query_by_label("Host domain").is_none() && h.query_by_label("Connect to Trello").is_none());
    h.get_by_label("Allow a read-only token").click();
    h.run();
    let want = |m: &Msg| matches!(m, Msg::OpenUrl(u) if u.starts_with("https://trello.com/1/authorize?") && u.contains("scope=read&"));
    assert!(pushed(&h, &want));
    h.get_by_label("Allow a token that can create cards").click();
    h.run();
    assert!(pushed(&h, &|m: &Msg| matches!(m, Msg::OpenUrl(u) if u.contains("scope=read,write&"))));
}

/// The Issue Tracker page with Trello and the Website board chosen.
fn tracker_page(theme: ThemeChoice, name: &str, boards: BoardList) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    st.settings.connections.push(connection());
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    st.settings.set_tracker(&real, TrackerChoice::Trello { account: acct(), board: BOARD.into(), board_name: "Website".into() });
    let mut prefs = PrefsState::new(PrefPage::RepoTracker);
    prefs.trello_boards = boards;
    st.prefs = Some(prefs);
    (st, dir)
}

#[test]
fn prefs_tracker_trello() {
    let boards = vec![
        TrelloBoard { id: "6a6a6a6a6a6a6a6a6a6a6a6a".into(), name: "Ops".into() },
        TrelloBoard { id: BOARD.into(), name: "Website".into() },
    ];
    let list = BoardList { account: Some(acct()), loading: false, boards, error: None };
    let (st, dir) = tracker_page(ThemeChoice::Light, "trello-tracker", list);
    let mut h = harness(st, false);
    h.run();
    h.get_by_role(Role::ComboBox).click();
    h.run();
    h.check_snapshot("prefs_tracker_trello");
    h.get_by_label("Ops").click();
    h.run();
    let want = |m: &Msg| {
        matches!(m, Msg::Forge(ForgeMsg::SetTracker(TrackerChoice::Trello { board, board_name, .. }))
            if board == "6a6a6a6a6a6a6a6a6a6a6a6a" && board_name == "Ops")
    };
    assert!(pushed(&h, &want));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn prefs_tracker_trello_error() {
    let error = "Trello's API rate limit was exceeded; wait a few seconds, then refresh".to_owned();
    let list = BoardList { account: Some(acct()), loading: false, boards: Vec::new(), error: Some(error) };
    let (st, dir) = tracker_page(ThemeChoice::Dark, "trello-tracker-error", list);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("prefs_tracker_trello_error");
    h.get_by_label("Try again").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Forge(ForgeMsg::LoadTrelloBoards(a)) if *a == acct())));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn trello_cards_menu() {
    let (mut st, dir) = initialized(ThemeChoice::Dark, "trello-cards-menu");
    st.settings.connections.push(connection());
    let gh = Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    let mut f = ForgeState::new(Some(gh), "origin".into(), Some(Target::trello(&acct(), BOARD, "Website")));
    let card = |key: &str, title: &str, list: &str| Issue {
        key: key.into(),
        title: title.into(),
        state: list.into(),
        kind: "Website".into(),
        assignee: Some("Ada Lovelace".into()),
        url: format!("https://trello.com/c/{key}"),
        facets: Default::default(),
    };
    f.my_issues = vec![card("AbCd1234", "Fix login", "Doing"), card("Zz9Yy8Xx", "Speed up the search page", "Review")];
    st.forge = Some(f);
    let mut h = harness(st, true);
    h.run();
    let at = center(&h, "AbCd1234  Fix login");
    click(&mut h, at, PointerButton::Secondary);
    h.check_snapshot("trello_cards_menu");
    for label in ["Create branch from card…", "Start Git Flow feature from card…", "View card in Trello", "Copy card link"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.query_by_label("Open in browser").is_none());
    h.get_by_label("View card in Trello").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::OpenUrl(u) if u == "https://trello.com/c/AbCd1234")));
    let at = center(&h, "AbCd1234  Fix login");
    click(&mut h, at, PointerButton::Secondary);
    h.get_by_label("Create branch from card…").click();
    h.run();
    assert!(pushed(&h, &|m| matches!(m, Msg::Branch(BranchMsg::NewBranch { name, .. }) if name == "AbCd1234-fix-login")));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn trello_link_needs_a_key() {
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    let mut prefs = PrefsState::new(PrefPage::Integrations);
    (prefs.int_provider, prefs.int_host) = (Provider::Trello, "trello.com".into());
    prefs.int_user = "too-short".into();
    st.prefs = Some(prefs);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Allow a read-only token").click();
    h.get_by_label("Allow a token that can create cards").click();
    h.run();
    assert!(!pushed(&h, &|m| matches!(m, Msg::OpenUrl(_))), "disabled until the key is whole");
}
