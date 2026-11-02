//! Headless tests for the tips' triggers (state/hints.rs, update/hints.rs): Undo after a recorded
//! reset or branch delete but never after a push, Tidy after WIP commits or two amends on a
//! branch with an upstream, and Connect on opening a repository whose hosting service has no
//! account. The rules, ignoring, retiring and the settings are in hints_rules_tests.rs.

use super::tests::{git, scratch_repo, state};
use super::update;
use crate::effect::{LoadKind, UndoResult};
use crate::msg::{JobMsg, Msg, RepoMsg};
use crate::state::hints::{self, HintId};
use crate::state::{AppState, RunningJob};
use git2::Oid;
use glowfly_core::forge::Connection;
use glowfly_core::repo::{self, LoadOptions};
use glowfly_core::undo::{Action, ResetMode};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Loads the repository at `dir` into `st` as a load of `kind` would.
pub(super) fn load_kind(st: &mut AppState, dir: &Path, kind: LoadKind) {
    let snap = Box::new(repo::load(dir, &LoadOptions::all(100)).unwrap());
    st.loading = true;
    let m = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind, result: Ok(snap) });
    update(st, m, Instant::now());
}

/// A returning user (the welcome done) with the scratch repository `name` open.
pub(super) fn open_repo(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    st.settings.welcome_done = true;
    load_kind(&mut st, &dir, LoadKind::Open);
    (st, dir)
}

/// A job in `dir` finishing well, having recorded `undo` (None: one that keeps the history).
pub(super) fn job_done(st: &mut AppState, dir: &Path, label: &str, undo: Option<Action>) {
    st.job = Some(RunningJob {
        label: label.into(),
        workdir: Some(dir.to_path_buf()),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_pr: None,
        then_link: false,
        keeps_undo: undo.is_none(),
        fetches: false,
    });
    let undo = undo.map(|a| UndoResult::Recorded(Some(a)));
    update(st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo }), Instant::now());
}

pub(super) fn shown(st: &AppState) -> Option<HintId> {
    st.onboarding.hint.map(|h| h.id)
}

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn reset() -> Action {
    Action::Reset { mode: ResetMode::Hard, branch: Some("main".into()), from: oid(1), to: oid(2), stash: None }
}

fn commit(amend: bool) -> Action {
    Action::Commit { branch: Some("main".into()), from: oid(1), to: oid(2), message: "m".into(), amend }
}

#[test]
fn a_reset_offers_the_undo_tip() {
    let (mut st, dir) = open_repo("hint-undo-reset");
    assert_eq!(shown(&st), None);
    job_done(&mut st, &dir, "Reset", Some(reset()));
    assert_eq!(shown(&st), Some(HintId::Undo));
    assert_eq!(st.onboarding.hints_shown_this_session, 1);
    let v = hints::view(&st).unwrap();
    let key = st.keymap.hint(crate::keymap::BindingId::Undo).unwrap().to_owned();
    assert!(v.text.starts_with(&format!("{key} undoes that")), "{}", v.text);
    assert_eq!(v.button, "Open journal");
}

#[test]
fn a_branch_delete_offers_the_undo_tip() {
    let (mut st, dir) = open_repo("hint-undo-delete");
    job_done(&mut st, &dir, "Delete branch", Some(Action::DeleteBranch { name: "old".into(), oid: oid(3), upstream: None }));
    assert_eq!(shown(&st), Some(HintId::Undo));
}

#[test]
fn a_force_push_never_offers_the_undo_tip() {
    let (mut st, dir) = open_repo("hint-undo-push");
    job_done(&mut st, &dir, "Force push main", None);
    assert_eq!(shown(&st), None, "Undo can't reverse a push");
}

#[test]
fn the_undo_tip_without_a_key_names_undo() {
    let (mut st, dir) = open_repo("hint-undo-unbound");
    st.keymap.set(crate::keymap::BindingId::Undo, Vec::new());
    job_done(&mut st, &dir, "Reset", Some(reset()));
    assert!(hints::view(&st).unwrap().text.starts_with("Undo reverses that"));
}

/// The scratch repository `name` with an upstream (origin/main at its first commit) and
/// `summaries` committed on top, opened.
fn ahead_repo(name: &str, summaries: &[&str], upstream: bool) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    if upstream {
        git(&dir, &["remote", "add", "origin", "https://example.invalid/r.git"]);
        git(&dir, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        git(&dir, &["config", "branch.main.remote", "origin"]);
        git(&dir, &["config", "branch.main.merge", "refs/heads/main"]);
    }
    for s in summaries {
        git(&dir, &["commit", "-q", "--allow-empty", "-m", s]);
    }
    let mut st = state();
    st.settings.welcome_done = true;
    load_kind(&mut st, &dir, LoadKind::Open);
    (st, dir)
}

/// A commit (or amend) finishing in `dir`, then the refresh after it.
fn committed(st: &mut AppState, dir: &Path, amend: bool) {
    job_done(st, dir, "Commit", Some(commit(amend)));
    load_kind(st, dir, LoadKind::Refresh);
}

#[test]
fn two_wip_commits_of_three_ahead_offer_tidy() {
    let (mut st, dir) = ahead_repo("hint-tidy-wip", &["WIP: parser", "Add the lexer", "fixup! Add the lexer"], true);
    assert_eq!(shown(&st), None, "only after a commit");
    committed(&mut st, &dir, false);
    assert_eq!(shown(&st), Some(HintId::TidyCommits));
    let snap = st.snap.as_ref().unwrap();
    let oldest = snap.commits.iter().find(|c| &*c.summary == "WIP: parser").unwrap().oid;
    assert_eq!(hints::tidy_base(snap), Some(oldest));
}

#[test]
fn one_wip_commit_of_three_ahead_does_not() {
    let (mut st, dir) = ahead_repo("hint-tidy-one-wip", &["wip parser", "Add the lexer", "Wipe the cache"], true);
    committed(&mut st, &dir, false);
    assert_eq!(shown(&st), None, "\"Wipe\" isn't wip");
}

#[test]
fn two_amends_in_a_row_offer_tidy() {
    let (mut st, dir) = ahead_repo("hint-tidy-amends", &["Add the lexer"], true);
    committed(&mut st, &dir, true);
    assert_eq!(shown(&st), None);
    committed(&mut st, &dir, true);
    assert_eq!(shown(&st), Some(HintId::TidyCommits));
}

#[test]
fn an_amend_a_commit_and_an_amend_do_not() {
    let (mut st, dir) = ahead_repo("hint-tidy-broken-run", &["Add the lexer"], true);
    committed(&mut st, &dir, true);
    committed(&mut st, &dir, false);
    committed(&mut st, &dir, true);
    assert_eq!(shown(&st), None);
    assert_eq!(st.session.hint_commits.amends_in_row, 1);
}

#[test]
fn a_branch_without_an_upstream_never_offers_tidy() {
    let (mut st, dir) = ahead_repo("hint-tidy-no-upstream", &["wip one", "wip two", "wip three"], false);
    committed(&mut st, &dir, true);
    committed(&mut st, &dir, true);
    assert_eq!(shown(&st), None);
}

#[test]
fn a_local_merge_in_the_ahead_commits_never_reaches_pushed_ones() {
    // Pushed: two WIP commits up to U. Local: A, then M merging F1..F3 forked from U. `ahead` is
    // 5, but only M and A are first parents ahead, so the pushed WIPs neither count nor get
    // rebased.
    let dir = scratch_repo("hint-tidy-merge");
    for s in ["wip pushed one", "wip pushed two"] {
        git(&dir, &["commit", "-q", "--allow-empty", "-m", s]);
    }
    git(&dir, &["remote", "add", "origin", "https://example.invalid/r.git"]);
    git(&dir, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&dir, &["config", "branch.main.remote", "origin"]);
    git(&dir, &["config", "branch.main.merge", "refs/heads/main"]);
    git(&dir, &["checkout", "-q", "-b", "side"]);
    for s in ["F1", "F2", "F3"] {
        git(&dir, &["commit", "-q", "--allow-empty", "-m", s]);
    }
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "wip local"]);
    git(&dir, &["merge", "-q", "--no-ff", "-m", "Merge side", "side"]);
    let mut st = state();
    st.settings.welcome_done = true;
    load_kind(&mut st, &dir, LoadKind::Open);
    committed(&mut st, &dir, false);
    assert_eq!(shown(&st), None, "the pushed WIP commits don't count");

    committed(&mut st, &dir, true);
    committed(&mut st, &dir, true);
    assert_eq!(shown(&st), Some(HintId::TidyCommits));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.locals.iter().find(|b| b.is_head).unwrap().ahead, 5);
    let local = snap.commits.iter().find(|c| &*c.summary == "wip local").unwrap().oid;
    assert_eq!(hints::tidy_base(snap), Some(local), "the rebase starts at the oldest commit ahead");
}

/// The scratch repository `name` with an `origin` at `url`, opened by `st`.
fn with_remote(st: &mut AppState, name: &str, url: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["remote", "add", "origin", url]);
    st.settings.welcome_done = true;
    load_kind(st, &dir, LoadKind::Open);
    dir
}

#[test]
fn a_github_remote_without_an_account_offers_connect() {
    let mut st = state();
    st.view.sections.prs = false;
    with_remote(&mut st, "hint-connect-github", "https://github.com/acme/widgets.git");
    assert_eq!(shown(&st), Some(HintId::Connect));
    let v = hints::view(&st).unwrap();
    assert_eq!(v.text, "Connect your GitHub account to see this repository's pull requests here.");
    assert_eq!((v.button, v.place), ("Connect…", hints::Place::LeftPanel));
}

#[test]
fn an_account_or_a_plain_remote_does_not_offer_connect() {
    let mut st = state();
    with_remote(&mut st, "hint-connect-account-probe", "https://github.com/acme/widgets.git");
    let account = st.session.forge.as_ref().unwrap().host.as_ref().unwrap().account();
    let mut st = state();
    st.settings.connections.push(Connection { account, user: "ada".into(), method: "token".into() });
    with_remote(&mut st, "hint-connect-account", "https://github.com/acme/widgets.git");
    assert_eq!(shown(&st), None, "already connected");
    let mut st = state();
    with_remote(&mut st, "hint-connect-plain", "https://example.invalid/r.git");
    assert_eq!(shown(&st), None, "not a hosting service");
}

#[test]
fn a_hidden_left_panel_keeps_connect_away() {
    let mut st = state();
    st.settings.show_left_panel = false;
    with_remote(&mut st, "hint-connect-no-panel", "https://github.com/acme/widgets.git");
    assert_eq!(shown(&st), None);
}

/// An open PULL REQUESTS section shows its own Connect… while no account is connected: the
/// tip is not offered over it.
#[test]
fn an_open_pull_requests_section_keeps_connect_away() {
    let mut st = state();
    with_remote(&mut st, "hint-connect-prs-open", "https://github.com/acme/widgets.git");
    assert!(st.view.sections.prs);
    assert_eq!(shown(&st), None);
    assert!(hints::view(&st).is_none());
}

/// Opening PULL REQUESTS while the tip shows hides it without counting it as ignored.
#[test]
fn opening_pull_requests_hides_connect_unignored() {
    let mut st = state();
    st.view.sections.prs = false;
    with_remote(&mut st, "hint-connect-prs-opened", "https://github.com/acme/widgets.git");
    assert_eq!(shown(&st), Some(HintId::Connect));
    st.view.sections.prs = true;
    assert!(hints::view(&st).is_none(), "not drawn under the open section");
    update(&mut st, Msg::Tick(Instant::now()), Instant::now());
    assert_eq!(shown(&st), None, "hidden");
    assert!(!st.settings.hints_ignored.contains(HintId::Connect.id()), "not ignored");
    assert!(!st.settings.hints_seen.contains(HintId::Connect.id()), "may show again later");
}
