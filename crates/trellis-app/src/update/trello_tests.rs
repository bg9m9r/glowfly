//! Trello as a repository's issue tracker: choosing it and a board, the Board picker's loading,
//! and its words in the interface.

use super::tests::{drive, scratch_repo, state};
use super::update;
use crate::effect::{Effect, ForgeCall, ForgeError, ForgeReply, ForgeResult};
use crate::msg::{ForgeMsg, Msg, PrefsMsg, RepoMsg};
use crate::state::{AppState, ForgeState, PrefPage, PrefsState, Status};
use std::time::Instant;
use trellis_core::forge::{Account, Auth, Connection, Issue, Provider, Target, TrelloBoard};
use trellis_core::settings::{RepoTrust, TrackerChoice};

const BOARD: &str = "5f1a2b3c4d5e6f7a8b9c0d1e";

fn acct() -> Account {
    Account { provider: Provider::Trello, host: "trello.com".into(), org: String::new() }
}

fn trello(board: &str, name: &str) -> TrackerChoice {
    TrackerChoice::Trello { account: acct(), board: board.into(), board_name: name.into() }
}

/// A trusted repository with no remote, in a profile that may or may not have Trello connected.
fn open(name: &str, connected: bool) -> AppState {
    let dir = scratch_repo(name);
    let mut st = state();
    if connected {
        st.settings.connections.push(Connection { account: acct(), user: "Ada".into(), method: "access token".into() });
    }
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir)));
    let real = st.snap.as_ref().expect("opened").real_workdir.clone();
    crate::state::trust::record(&mut st.settings, &real, RepoTrust::Trusted);
    st
}

fn run(st: &mut AppState, msg: ForgeMsg) -> Vec<Effect> {
    update(st, Msg::Forge(msg), Instant::now())
}

fn my_issues_calls(fx: &[Effect]) -> Vec<bool> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Forge { call: ForgeCall::MyIssues, connected, .. } => Some(*connected),
            _ => None,
        })
        .collect()
}

fn board_loads(fx: &[Effect]) -> usize {
    fx.iter().filter(|e| matches!(e, Effect::TrelloBoards { account, .. } if *account == acct())).count()
}

fn card(key: &str, title: &str) -> Issue {
    Issue {
        key: key.into(),
        title: title.into(),
        state: "Doing".into(),
        kind: "Website".into(),
        assignee: Some("Ada".into()),
        url: format!("https://trello.com/c/{key}"),
    }
}

#[test]
fn choosing_trello_detects_the_tracker_and_lists_cards() {
    let mut st = open("trello-choose", true);
    let fx = run(&mut st, ForgeMsg::SetTracker(trello("", "")));
    let fs = st.forge.as_ref().expect("a tracker");
    assert_eq!(fs.tracker, Some(Target::trello(&acct(), "", "")));
    assert_eq!(fs.host, None);
    assert_eq!(my_issues_calls(&fx), [true]);
    assert_eq!(board_loads(&fx), 0, "Preferences aren't open");
}

#[test]
fn board_choice_reloads_the_cards() {
    let mut st = open("trello-board", true);
    run(&mut st, ForgeMsg::SetTracker(trello("", "")));
    let fs = st.forge.as_mut().unwrap();
    fs.my_issues = vec![card("AbCd1234", "Fix login")];
    fs.cache.insert("AbCd1234".into(), card("AbCd1234", "Fix login"));
    let fx = run(&mut st, ForgeMsg::SetTracker(trello(BOARD, "Website")));
    let fs = st.forge.as_ref().unwrap();
    let t = fs.tracker.as_ref().unwrap();
    assert_eq!((t.owner.as_str(), t.project.as_str()), (BOARD, "Website"));
    assert!(fs.my_issues.is_empty() && fs.cache.is_empty());
    assert_eq!(my_issues_calls(&fx), [true]);
}

#[test]
fn issue_tracker_page_loads_boards_once() {
    let mut st = open("trello-page", true);
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    st.settings.set_tracker(&real, trello(BOARD, "Website"));
    st.prefs = Some(PrefsState::new(PrefPage::General));
    let fx = run(&mut st, ForgeMsg::ShowTrackerPage);
    assert_eq!(st.prefs.as_ref().unwrap().page, PrefPage::RepoTracker);
    assert_eq!(board_loads(&fx), 1);
    assert!(st.prefs.as_ref().unwrap().trello_boards.loading);
    assert_eq!(board_loads(&run(&mut st, ForgeMsg::ShowTrackerPage)), 0, "already loading");
    assert_eq!(board_loads(&run(&mut st, ForgeMsg::LoadTrelloBoards(acct()))), 1, "Try again reloads");
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::RepoTracker)), Instant::now());
    assert_eq!(board_loads(&fx), 1);
    // Choosing Trello on the open page loads its boards too.
    st.prefs = Some(PrefsState::new(PrefPage::RepoTracker));
    st.settings.set_tracker(&real, TrackerChoice::None);
    assert_eq!(board_loads(&run(&mut st, ForgeMsg::SetTracker(trello("", "")))), 1);
}

#[test]
fn boards_reply_fills_the_picker_and_stale_replies_are_ignored() {
    let mut st = open("trello-reply", true);
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    st.settings.set_tracker(&real, trello("", ""));
    st.prefs = Some(PrefsState::new(PrefPage::General));
    run(&mut st, ForgeMsg::ShowTrackerPage);
    let boards = vec![
        TrelloBoard { id: BOARD.into(), name: "Website".into() },
        TrelloBoard { id: "6a6a6a6a6a6a6a6a6a6a6a6a".into(), name: "Ops".into() },
    ];
    run(&mut st, ForgeMsg::TrelloBoardsLoaded { account: acct(), result: Ok(boards) });
    let list = &st.prefs.as_ref().unwrap().trello_boards;
    assert!(!list.loading && list.error.is_none() && list.has(BOARD) && list.boards.len() == 2);

    let other = Account { host: "other.example".into(), ..acct() };
    run(&mut st, ForgeMsg::TrelloBoardsLoaded { account: other, result: Err("boom".into()) });
    assert_eq!(st.prefs.as_ref().unwrap().trello_boards.error, None, "another account's reply");

    run(&mut st, ForgeMsg::TrelloBoardsLoaded { account: acct(), result: Err("rate limit".into()) });
    assert_eq!(st.prefs.as_ref().unwrap().trello_boards.error.as_deref(), Some("rate limit"));
}

#[test]
fn unconnected_trello_is_never_contacted() {
    let mut st = open("trello-unconnected", false);
    run(&mut st, ForgeMsg::SetTracker(trello(BOARD, "Website")));
    let fx = run(&mut st, ForgeMsg::ForgeRefresh { interactive: true });
    assert!(!fx.iter().any(|e| matches!(e, Effect::Forge { .. })), "nothing is called");
    let fs = st.forge.as_ref().unwrap();
    assert!(matches!(&fs.tracker_status, Status::Error(e) if e.contains("not connected")), "{:?}", fs.tracker_status);

    st.prefs = Some(PrefsState::new(PrefPage::General));
    assert_eq!(board_loads(&run(&mut st, ForgeMsg::ShowTrackerPage)), 0);
    let list = &st.prefs.as_ref().unwrap().trello_boards;
    assert!(list.error.as_deref().is_some_and(|e| e.contains("not connected")) && !list.loading);
}

#[test]
fn trello_words_and_refs() {
    let gh = Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    let tracker = Target::trello(&acct(), BOARD, "Website");
    let mut st = state();
    st.forge = Some(ForgeState::new(Some(gh), "origin".into(), Some(tracker.clone())));
    let fs = st.forge.as_mut().unwrap();
    assert_eq!((fs.issue_noun(), fs.open_issue_label(), fs.tracker_hint()), ("card", "View card in Trello", "Trello · Website".to_owned()));
    let c = card("AbCd1234", "Fix login");
    assert_eq!(fs.copy_issue_item(&c), ("Copy card link".to_owned(), "https://trello.com/c/AbCd1234".to_owned()));
    assert_eq!(fs.issue_refs("Fix login\n\nCard: https://trello.com/c/AbCd1234/7-fix-login"), ["AbCd1234"]);
    assert_eq!(fs.branch_issue("feature/AbCd1234-x").as_deref(), Some("AbCd1234"));
    assert_eq!(fs.issue_url("Zz9Yy8Xx"), "https://trello.com/c/Zz9Yy8Xx");

    // An expired token drops the cached sign-in, so the next refresh reads the saved one again.
    fs.auth.insert(acct().key(), Auth::from_token(Provider::Trello, Some("k"), "t", "saved token"));
    fs.loading = 1;
    let error = ForgeError { message: "authentication failed: the Trello token has expired".into(), auth: true };
    let reply = ForgeReply { target: tracker, auth: None, results: vec![ForgeResult::MyIssues(Err(error))] };
    run(&mut st, ForgeMsg::ForgeReplied(reply));
    let fs = st.forge.as_ref().unwrap();
    assert!(!fs.auth.contains_key(&acct().key()));
    assert!(matches!(&fs.tracker_status, Status::Error(e) if e.contains("expired")));

    // Other trackers keep their words.
    let mut other = ForgeState::new(None, String::new(), Some(Target::jira(&Account { provider: Provider::JiraCloud, ..acct() }, "")));
    other.tracker.as_mut().unwrap().host = "x.atlassian.net".into();
    assert_eq!((other.open_issue_label(), other.copy_issue_item(&c).0), ("Open in browser", "Copy AbCd1234".to_owned()));
}
