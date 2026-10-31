//! Headless tests for opening a glowfly:// link's repository and selecting its target: the
//! search's answer (an open tab, a parked tab, a recent repository, several copies, none), a
//! target that is missing (the Fetch toast, the check after the fetch), and the Locate and Clone
//! ways out of the Repository not found dialog.

use super::branch_tests::with_origin;
use super::deep_link_tests::{cleanup, link_text, send};
use super::tests::{drive, finish, git, scratch_repo, state};
use super::update;
use crate::deep_link::{DeepLinkEffect, DeepLinkMsg, FindOrigin};
use crate::effect::OsEffect;
use crate::effect::{Effect, LoadKind};
use crate::msg::{FolderPurpose, JobMsg, Msg, PrefsMsg, RepoMsg};
use crate::state::deep_link::{LinkDialog, PickReason, pick_reason};
use crate::state::{AppState, Modal, Selection, Tab};
use git2::Oid;
use glowfly_core::link::find::{Found, TargetState};
use glowfly_core::link::{Link, Target};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn tip(dir: &Path, rev: &str) -> Oid {
    git2::Repository::open(dir).unwrap().revparse_single(rev).unwrap().peel_to_commit().unwrap().id()
}

fn present(oid: Oid) -> TargetState {
    TargetState::Present { oid, label: "feature".into() }
}

/// A match for the repository at `dir`, its own common dir.
fn found(dir: &Path, target: TargetState) -> Found {
    Found {
        workdir: dir.to_path_buf(),
        common_dir: dir.join(".git"),
        main_worktree: true,
        by_root: true,
        by_url: true,
        remote: Some("origin".into()),
        target,
    }
}

/// Opens `text` and answers its search with `matches`.
fn answer(st: &mut AppState, text: &str, matches: Vec<Found>) -> Vec<Effect> {
    send(st, DeepLinkMsg::Open(text.to_owned()));
    let seq = st.deep_link.seq;
    send(st, DeepLinkMsg::Found { seq, origin: FindOrigin::Search, result: Ok(matches) })
}

fn loads(fx: &[Effect], kind: LoadKind) -> Option<PathBuf> {
    fx.iter().find_map(|e| match e {
        Effect::LoadRepo { path, kind: k, .. } if *k == kind => Some(path.clone()),
        _ => None,
    })
}

fn find(fx: &[Effect]) -> Option<(Vec<PathBuf>, FindOrigin)> {
    fx.iter().find_map(|e| match e {
        Effect::DeepLink(DeepLinkEffect::Find { candidates, origin, .. }) => Some((candidates.clone(), *origin)),
        _ => None,
    })
}

#[test]
fn a_match_in_the_active_tab_is_selected_there() {
    let (mut st, work, bare) = with_origin("link-go-active");
    let oid = tip(&work, "feature");
    st.view.scroll_to_row = None;
    let fx = answer(&mut st, &link_text("branch/feature"), vec![found(&work, present(oid))]);
    assert_eq!(st.session.selection, Selection::Commit(oid));
    assert!(st.view.scroll_to_row.is_some(), "scrolled into view");
    assert_eq!(loads(&fx, LoadKind::Open), None, "nothing new opens");
    assert!(st.deep_link.pending.is_none());
    cleanup(&[&work, &bare]);
}

#[test]
fn a_match_in_a_parked_tab_is_switched_to() {
    let (mut st, work, bare) = with_origin("link-go-parked");
    let other = scratch_repo("link-go-parked-other");
    drive(&mut st, Msg::Repo(RepoMsg::Open(other.clone())));
    assert_eq!(st.active_tab, 1);
    let oid = tip(&work, "feature");
    answer(&mut st, &link_text("branch/feature"), vec![found(&work, present(oid))]);
    assert_eq!(st.active_tab, 0);
    assert_eq!(st.session.selection, Selection::Commit(oid));
    cleanup(&[&work, &bare, &other]);
}

#[test]
fn a_recent_match_opens_then_selects() {
    let (_, work, bare) = with_origin("link-go-recent");
    let oid = tip(&work, "feature");
    let mut st = state();
    st.recent = vec![work.clone()];
    let fx = answer(&mut st, &link_text("branch/feature"), vec![found(&work, present(oid))]);
    assert_eq!(loads(&fx, LoadKind::Open), Some(work.clone()));
    assert!(st.deep_link.pending.is_some(), "waits for the load");
    finish(&mut st, fx);
    assert!(st.snap.is_some());
    assert_eq!(st.session.selection, Selection::Commit(oid));
    assert!(st.deep_link.pending.is_none());
    cleanup(&[&work, &bare]);
}

#[test]
fn several_copies_ask_which_one() {
    let base = glowfly_core::paths::temp_dir().join(format!("glowfly-link-choose-{}", std::process::id()));
    let (a, b) = (base.join("a"), base.join("b"));
    let oid = Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap();
    let mut st = state();
    st.recent = vec![a.clone(), b.clone()];
    answer(&mut st, &link_text("repo"), vec![found(&a, present(oid)), found(&b, present(oid))]);
    match &st.modal {
        Some(Modal::DeepLink(LinkDialog::Choose { paths, picked, what, reason })) => {
            assert_eq!((paths.clone(), *picked, what.as_str()), (vec![a.clone(), b.clone()], 0, "repository"));
            assert_eq!(*reason, Some(PickReason::Recent), "the first was opened last");
        }
        _ => panic!("expected the Choose dialog"),
    }
    send(&mut st, DeepLinkMsg::Choose(1));
    let fx = send(&mut st, DeepLinkMsg::OpenChosen);
    assert_eq!(loads(&fx, LoadKind::Open), Some(b));
    assert!(st.modal.is_none());
}

#[test]
fn worktrees_of_one_repository_are_one_match() {
    let (mut st, work, bare) = with_origin("link-go-worktrees");
    let oid = tip(&work, "feature");
    // A linked worktree that isn't open, and the open main worktree, sharing one common dir.
    let mut linked = found(&work.with_file_name("not-open-worktree"), present(oid));
    linked.common_dir = work.join(".git");
    linked.main_worktree = false;
    answer(&mut st, &link_text("branch/feature"), vec![linked, found(&work, present(oid))]);
    assert!(st.modal.is_none(), "no question");
    assert_eq!(st.session.selection, Selection::Commit(oid), "the open tab won");
    cleanup(&[&work, &bare]);
}

#[test]
fn no_match_offers_locate_and_clone() {
    let mut st = state();
    answer(&mut st, &link_text("branch/feature%2Fx"), Vec::new());
    match &st.modal {
        Some(Modal::DeepLink(LinkDialog::NotFound { what, repo, can_clone, problem })) => {
            assert_eq!(
                (what.as_str(), repo.as_str(), *can_clone, problem),
                ("branch feature/x", "the repository with root commit 0123456", false, &None)
            );
        }
        _ => panic!("expected Repository not found"),
    }
    let with_url = Link { url: Some("https://example.com/o/r.git".into()), root: None, target: Target::Repository }.to_uri();
    answer(&mut st, &with_url, Vec::new());
    assert!(matches!(&st.modal, Some(Modal::DeepLink(LinkDialog::NotFound { can_clone: true, .. }))));

    // Locate on disk… picks a folder, which is checked alone.
    let fx = send(&mut st, DeepLinkMsg::Locate);
    assert!(matches!(&fx[..], [Effect::Os(OsEffect::PickFolder { purpose: FolderPurpose::LocateLinkedRepo, .. })]));
    let folder = glowfly_core::paths::temp_dir().join("not-this-one");
    let picked = PrefsMsg::FolderPicked { purpose: FolderPurpose::LocateLinkedRepo, path: Some(folder.clone()) };
    let fx = update(&mut st, Msg::Prefs(picked), Instant::now());
    assert_eq!(find(&fx), Some((vec![folder.clone()], FindOrigin::Located)));
    let seq = st.deep_link.seq;
    send(&mut st, DeepLinkMsg::Found { seq, origin: FindOrigin::Located, result: Ok(Vec::new()) });
    match &st.modal {
        Some(Modal::DeepLink(LinkDialog::NotFound { problem: Some(p), .. })) => {
            assert!(p.starts_with(&glowfly_core::native_path::shown(&folder)), "{p}");
            assert!(p.ends_with("isn't a copy of this repository: its history and remotes don't match the link."), "{p}");
        }
        _ => panic!("expected the dialog to stay, with the problem"),
    }

    // Closing the dialog gives the link up.
    update(&mut st, Msg::CloseModal, Instant::now());
    assert!(st.deep_link.pending.is_none());
}

#[test]
fn a_missing_target_offers_fetch_and_looks_again() {
    let (mut st, work, bare) = with_origin("link-fetch");
    // The branch exists only on the remote, not yet fetched.
    git(&bare, &["branch", "later", "main"]);
    // The real root, so the check after the fetch finds this repository.
    let root = glowfly_core::link::find::root_commit(&work).unwrap();
    let link = Link { url: None, root, target: Target::Branch("later".into()) }.to_uri();
    answer(&mut st, &link, vec![found(&work, TargetState::Missing)]);
    let toast = st.toasts.last().unwrap();
    assert!(!toast.ok);
    assert!(toast.text.starts_with("Branch later isn't in "), "{}", toast.text);
    assert!(toast.text.ends_with("Fetching origin may bring it."), "{}", toast.text);
    let action = toast.action.as_ref().unwrap();
    assert_eq!(action.label, "Fetch");
    assert!(matches!(&*action.msg, Msg::DeepLink(DeepLinkMsg::Fetch)));

    let mut fx = send(&mut st, DeepLinkMsg::Fetch);
    assert_eq!(fx.len(), 1, "{fx:?}");
    let job = fx.remove(0);
    assert!(matches!(&job, Effect::RunGit { cmds, .. } if cmds[0] == glowfly_core::remote_ops::fetch_cmd("origin")), "{job:?}");
    let run = crate::runner::testing::execute(job).unwrap();
    assert!(st.job.as_ref().is_some_and(|j| j.then_link));
    let fx = update(&mut st, run, Instant::now());
    let (candidates, origin) = find(&fx).expect("looks again");
    assert_eq!(origin, FindOrigin::AfterFetch);
    assert_eq!(candidates.len(), 1);
    // The real check finds the fetched branch, and selects it.
    let check = fx.into_iter().find(|e| matches!(e, Effect::DeepLink(DeepLinkEffect::Find { .. }))).unwrap();
    let answered = crate::runner::testing::execute(check).unwrap();
    let fx = update(&mut st, answered, Instant::now());
    finish(&mut st, fx);
    assert_eq!(st.session.selection, Selection::Commit(tip(&work, "main")));
    assert!(st.deep_link.pending.is_none());

    // Still missing after the fetch: said, and the link given up.
    answer(&mut st, &link_text("branch/never"), vec![found(&work, TargetState::Missing)]);
    send(&mut st, DeepLinkMsg::Fetch);
    update(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: None }), Instant::now());
    let seq = st.deep_link.seq;
    send(&mut st, DeepLinkMsg::Found { seq, origin: FindOrigin::AfterFetch, result: Ok(vec![found(&work, TargetState::Missing)]) });
    let last = st.toasts.iter().rev().find(|t| t.text.starts_with("Fetched")).expect("the toast");
    assert_eq!(last.text, "Fetched origin, but branch never still isn't there; it may have been renamed or deleted.");
    assert!(!last.ok);
    assert!(st.deep_link.pending.is_none());
    cleanup(&[&work, &bare]);
}

#[test]
fn a_repository_without_remotes_gets_no_fetch_button() {
    let dir = scratch_repo("link-no-remotes");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let mut f = found(&dir, TargetState::Missing);
    f.remote = None;
    answer(&mut st, &link_text("tag/v9"), vec![f]);
    let toast = st.toasts.last().unwrap();
    assert!(toast.text.starts_with("Tag v9 isn't in "), "{}", toast.text);
    assert!(toast.action.is_none());
    assert!(st.deep_link.pending.is_none());
    cleanup(&[&dir]);
}

#[test]
fn clone_fills_the_url_and_the_clone_is_checked() {
    let (_, work, bare) = with_origin("link-clone");
    let url = bare.display().to_string();
    let mut st = state();
    let text = Link { url: Some(url.clone()), root: None, target: Target::Branch("feature".into()) }.to_uri();
    answer(&mut st, &text, Vec::new());
    send(&mut st, DeepLinkMsg::Clone);
    match &st.modal {
        Some(Modal::Clone(form)) => assert_eq!(form.url, url),
        _ => panic!("expected the Clone dialog"),
    }
    // The clone opens (here: a copy with that origin): it is checked for the link.
    update(&mut st, Msg::CloseModal, Instant::now());
    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(work.clone())), Instant::now());
    let load = fx.into_iter().find(|e| matches!(e, Effect::LoadRepo { .. })).unwrap();
    let loaded = crate::runner::testing::execute(load).unwrap();
    let fx = update(&mut st, loaded, Instant::now());
    let (candidates, origin) = find(&fx).expect("the clone is checked");
    assert_eq!(origin, FindOrigin::AfterClone);
    assert_eq!(candidates, vec![st.snap.as_ref().unwrap().workdir.clone()]);
    cleanup(&[&work, &bare]);
}

#[test]
fn a_search_answer_waits_for_a_dialog_opened_meanwhile() {
    let base = glowfly_core::paths::temp_dir().join(format!("glowfly-link-busy-{}", std::process::id()));
    let (a, b) = (base.join("a"), base.join("b"));
    let text = link_text("tag/v1");
    let waits = |st: &AppState| {
        assert!(st.deep_link.pending.is_none());
        let toast = st.toasts.last().unwrap();
        assert_eq!(toast.text, crate::state::deep_link::LINK_WAITING);
        let action = toast.action.as_ref().unwrap();
        let Msg::DeepLink(DeepLinkMsg::Open(t)) = &*action.msg else { panic!("expected Open link") };
        assert_eq!(glowfly_core::link::parse(t).unwrap().target, Target::Tag("v1".into()));
    };
    // None or several matches: the open dialog isn't replaced.
    for matches in [Vec::new(), vec![found(&a, TargetState::Missing), found(&b, TargetState::Missing)]] {
        let mut st = state();
        send(&mut st, DeepLinkMsg::Open(text.clone()));
        st.modal = Some(Modal::Shortcuts { filter: "typed".into() });
        let seq = st.deep_link.seq;
        send(&mut st, DeepLinkMsg::Found { seq, origin: FindOrigin::Search, result: Ok(matches) });
        assert!(matches!(&st.modal, Some(Modal::Shortcuts { filter }) if filter == "typed"));
        waits(&st);
    }
    // One match: no tab opens under Preferences.
    let mut st = state();
    send(&mut st, DeepLinkMsg::Open(text.clone()));
    st.prefs = Some(crate::state::PrefsState::new(crate::state::PrefPage::General));
    let seq = st.deep_link.seq;
    let fx = send(&mut st, DeepLinkMsg::Found { seq, origin: FindOrigin::Search, result: Ok(vec![found(&a, TargetState::Missing)]) });
    assert_eq!(loads(&fx, LoadKind::Open), None);
    assert!(st.tabs.iter().all(|t| t.path.as_deref() != Some(a.as_path())));
    waits(&st);
}

#[test]
fn the_preselected_copy_says_why() {
    let (mut st, work, bare) = with_origin("link-pick-reason");
    let oid = tip(&work, "feature");
    let base = work.with_file_name("link-pick-reason-others");
    let (tab, fav, recent, none) = (base.join("tab"), base.join("fav"), base.join("recent"), base.join("none"));
    st.tabs.push(Tab::unloaded(tab.clone()));
    st.settings.favorite_repos = vec![glowfly_core::paths::encode(&fav)];
    st.recent = vec![recent.clone()];
    let why = |st: &AppState, dir: &Path| pick_reason(st, &found(dir, present(oid)));
    assert_eq!(why(&st, &work), Some(PickReason::Active));
    assert_eq!(why(&st, &tab), Some(PickReason::OpenTab));
    assert_eq!(why(&st, &fav), Some(PickReason::Favorite));
    assert_eq!(why(&st, &recent), Some(PickReason::Recent));
    assert_eq!(why(&st, &none), None);
    cleanup(&[&work, &bare]);
}
