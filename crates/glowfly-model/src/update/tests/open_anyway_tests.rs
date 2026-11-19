//! Opening a folder whose Git data points to network places that aren't approved for it: the
//! open fails as any failed open does and asks; Open anyway approves exactly those places for
//! the folder the check named and opens it again (the approval reaching every later open
//! before the load starts); Cancel leaves the failed open; a new place asks again; a restored
//! session asks for the tab on screen only, the others when they are shown; Preferences
//! forgets a folder's places.

use super::tests::state;
use super::update;
use crate::effect::{ControlEffect, Effect, LoadEffect, LoadKind};
use crate::msg::{Msg, RepoMsg, TabsMsg, TrustMsg};
use crate::state::{AppState, Modal};
use glowfly_core::error::{NetworkPlaces, ReadFailed as R};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn send(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn dir(name: &str) -> PathBuf {
    glowfly_core::paths::temp_dir().join(format!("glowfly-test-open-anyway-{name}-{}", std::process::id()))
}

/// The active open fails, refused for `places` named by `folder`'s Git data.
fn refused(st: &mut AppState, folder: &Path, places: &[&str]) -> Vec<Effect> {
    let n = NetworkPlaces { folder: folder.to_path_buf(), places: places.iter().map(|p| p.to_string()).collect() };
    let result = Err(R::OpenFailed { path: glowfly_core::native_path::shown(folder), why: Box::new(R::NetworkPointer(n)) });
    let generation = st.load_gen;
    send(st, Msg::Repo(RepoMsg::RepoLoaded { generation, kind: LoadKind::Open, result }))
}

/// The open load effects among `fx`, by path.
fn loads(fx: &[Effect]) -> Vec<PathBuf> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Load(LoadEffect::Repo { path, kind: LoadKind::Open, .. }) => Some(path.clone()),
            _ => None,
        })
        .collect()
}

fn has_tab(st: &AppState, p: &Path) -> bool {
    st.tabs.iter().any(|t| t.path.as_deref() == Some(p))
}

#[test]
fn a_refused_open_asks_and_open_anyway_approves_those_places_and_opens_again() {
    let (repo, places) = (dir("ask"), [r"\\nas\team", "Z:\\"]);
    let mut st = state();
    send(&mut st, Msg::Repo(RepoMsg::Open(repo.clone())));
    refused(&mut st, &repo, &places);
    assert!(!has_tab(&st, &repo), "it fails as any open does");
    let Some(Modal::Confirm { title, body, button, cancel_default, .. }) = &st.modal else { panic!("asks: {:?}", st.modal.is_some()) };
    assert!(*cancel_default, "Cancel is the default: Enter never opens anyway");
    assert_eq!(title, "Open a folder that points to a network place?");
    let shown = glowfly_core::native_path::shown(&repo);
    let names = format!(r"The Git data of {shown} points to 2 network places: \\nas\team, Z:\. Opening it connects to those servers");
    assert!(body.starts_with(&names), "{body}");
    assert_eq!(cfg!(windows), body.contains("Windows may send your sign-in to them"), "{body}");
    let line = &st.log.last().unwrap().text;
    assert_eq!(line, &format!(r"{shown}: its Git data points to \\nas\team, Z:\. Asking before opening it."), "a neutral line");
    assert_eq!(button, "Open anyway");
    assert!(st.settings.repo_network_places.is_empty(), "nothing approved yet");

    let fx = send(&mut st, Msg::ConfirmAccepted);
    assert!(st.modal.is_none());
    assert_eq!(st.settings.network_places_for(&repo), ["Z:\\".to_owned(), r"\\nas\team".to_owned()], "sorted");
    assert!(st.settings.network_places_for(&repo.join("sub")).is_empty() && st.settings.network_places_for(&dir("other")).is_empty());
    // The approval reaches the opens before the load starts.
    let given = fx.iter().position(
        |e| matches!(e, Effect::Control(ControlEffect::NetworkPlaces(a)) if a.places_for(&repo) == st.settings.network_places_for(&repo)),
    );
    let load = fx.iter().position(|e| matches!(e, Effect::Load(LoadEffect::Repo { path, .. }) if path == &repo));
    assert!(matches!((given, load), (Some(g), Some(l)) if g < l), "{fx:?}");
    assert!(has_tab(&st, &repo), "opening again");
    assert!(fx.iter().any(|e| matches!(e, Effect::Save(crate::effect::SaveEffect::Settings(_)))), "saved");
}

#[test]
fn cancel_leaves_the_failed_open() {
    let repo = dir("cancel");
    let mut plain = state();
    send(&mut plain, Msg::Repo(RepoMsg::Open(repo.clone())));
    let result = Err(R::OpenFailed { path: "/r".into(), why: Box::new(R::NotARepository { path: "/r".into() }) });
    let generation = plain.load_gen;
    send(&mut plain, Msg::Repo(RepoMsg::RepoLoaded { generation, kind: LoadKind::Open, result }));

    let mut st = state();
    send(&mut st, Msg::Repo(RepoMsg::Open(repo.clone())));
    refused(&mut st, &repo, &[r"\\nas\team"]);
    let fx = send(&mut st, Msg::CloseModal);
    assert!(st.modal.is_none() && fx.is_empty() && st.settings.repo_network_places.is_empty());
    let shape = |s: &AppState| (s.tabs.len(), s.active_tab, s.snap.is_some(), s.loading, s.log.len());
    assert_eq!(shape(&st), shape(&plain));
    assert!(!st.log.last().unwrap().ok, "the failure is logged");
}

#[test]
fn a_new_place_asks_again_for_it_alone() {
    let repo = dir("new-place");
    let mut st = state();
    st.settings.approve_network_places(&repo, &[r"\\nas\team".to_owned()]);
    send(&mut st, Msg::Repo(RepoMsg::Open(repo.clone())));
    // The check names only the places not approved.
    refused(&mut st, &repo, &[r"\\other\share"]);
    let Some(Modal::Confirm { body, then, .. }) = &st.modal else { panic!("asks again") };
    assert!(body.contains(r"\\other\share") && !body.contains("team"), "{body}");
    assert!(body.contains("Opening it connects to that server") && !body.contains("network places:"), "one place: {body}");
    assert!(matches!(&**then, Msg::Repo(RepoMsg::OpenAnyway { places, .. }) if places == &[r"\\other\share".to_owned()]));
    send(&mut st, Msg::ConfirmAccepted);
    assert_eq!(st.settings.network_places_for(&repo), [r"\\nas\team".to_owned(), r"\\other\share".to_owned()]);
}

#[test]
fn a_refresh_and_another_open_in_progress_dont_ask() {
    let repo = dir("quiet");
    let mut st = state();
    send(&mut st, Msg::Repo(RepoMsg::Open(repo.clone())));
    let n = NetworkPlaces { folder: repo.clone(), places: vec![r"\\nas\team".into()] };
    let result = || Err(R::OpenFailed { path: "/r".into(), why: Box::new(R::NetworkPointer(n.clone())) });
    // A late result for an older load.
    let generation = st.load_gen - 1;
    send(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation, kind: LoadKind::Open, result: result() }));
    assert!(st.modal.is_none());
    let generation = st.load_gen;
    send(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation, kind: LoadKind::Refresh, result: result() }));
    assert!(st.modal.is_none(), "a refresh only logs");
    // Something else is asking: the open fails without stacking a second question.
    send(&mut st, Msg::Repo(RepoMsg::Open(repo.clone())));
    st.modal = Some(Modal::Confirm {
        title: "t".into(),
        body: "b".into(),
        button: "OK".into(),
        then: Box::new(Msg::Repo(RepoMsg::Refresh)),
        cancel_default: false,
    });
    refused(&mut st, &repo, &[r"\\nas\team"]);
    assert!(matches!(&st.modal, Some(Modal::Confirm { title, .. }) if title == "t"));
}

#[test]
fn a_restored_session_asks_for_the_tab_on_screen_and_the_others_when_shown() {
    let (a, b) = (dir("restored-a"), dir("restored-b"));
    let mut st = state();
    let fx = send(&mut st, Msg::Tabs(TabsMsg::RestoreTabs { paths: vec![a.clone(), b.clone()], labels: Vec::new(), active: 1 }));
    assert_eq!(loads(&fx), std::slice::from_ref(&b), "only the tab on screen loads");
    refused(&mut st, &b, &[r"\\nas\team"]);
    assert!(
        matches!(&st.modal, Some(Modal::Confirm { then, .. }) if matches!(&**then, Msg::Repo(RepoMsg::OpenAnyway { path, .. }) if path == &b))
    );
    send(&mut st, Msg::CloseModal);
    // The other tab asks once it is shown.
    let i = st.tabs.iter().position(|t| t.path.as_deref() == Some(a.as_path())).expect("still listed");
    let fx = send(&mut st, Msg::Tabs(TabsMsg::SelectTab(i)));
    assert_eq!(loads(&fx), std::slice::from_ref(&a));
    refused(&mut st, &a, &[r"\\nas\team"]);
    assert!(
        matches!(&st.modal, Some(Modal::Confirm { then, .. }) if matches!(&**then, Msg::Repo(RepoMsg::OpenAnyway { path, .. }) if path == &a))
    );
}

#[test]
fn preferences_forget_a_folders_places() {
    let (repo, other) = (dir("forget"), dir("forget-other"));
    let mut st = state();
    st.settings.approve_network_places(&repo, &[r"\\nas\team".to_owned()]);
    st.settings.approve_network_places(&other, &["Z:\\".to_owned()]);
    let fx = send(&mut st, Msg::Trust(TrustMsg::ForgetNetworkPlaces(repo.clone())));
    assert!(st.settings.network_places_for(&repo).is_empty());
    assert_eq!(st.settings.network_places_for(&other), ["Z:\\".to_owned()], "another folder keeps its own");
    assert!(fx.iter().any(|e| matches!(e, Effect::Control(ControlEffect::NetworkPlaces(a)) if a.places_for(&repo).is_empty() && !a.places_for(&other).is_empty())));
    assert!(st.log.last().is_some_and(|l| l.ok));
    // Nothing to forget: nothing happens.
    assert!(send(&mut st, Msg::Trust(TrustMsg::ForgetNetworkPlaces(repo))).is_empty());
}

#[test]
fn the_question_names_the_folder_the_approval_is_for() {
    // The open searched upward: the Git data the check named is the parent's.
    let (repo, sub) = (dir("upward"), dir("upward").join("src"));
    let mut st = state();
    send(&mut st, Msg::Repo(RepoMsg::Open(sub.clone())));
    let n = NetworkPlaces { folder: repo.clone(), places: vec!["/net/nas".into()] };
    let result = Err(R::OpenFailed { path: glowfly_core::native_path::shown(&sub), why: Box::new(R::NetworkPointer(n)) });
    let generation = st.load_gen;
    send(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation, kind: LoadKind::Open, result }));
    let Some(Modal::Confirm { body, .. }) = &st.modal else { panic!("asks") };
    let want = format!("The Git data of {} points to /net/nas.", glowfly_core::native_path::shown(&repo));
    assert!(body.starts_with(&want), "{body}");
    send(&mut st, Msg::ConfirmAccepted);
    assert_eq!(st.settings.network_places_for(&repo), ["/net/nas".to_owned()]);
    assert!(st.settings.network_places_for(&sub).is_empty(), "approved for the folder named");
    assert!(has_tab(&st, &sub), "opens what was opened");
}
