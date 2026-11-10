//! The repository's failed reads in each language (#367, `ReadFailed`): where a load's, the
//! status's, a commit detail's, a diff's, a signature check's and line counts' errors end up
//! (activity line, held load-more failure, a freed tab's pane, the badge's tooltip, or only
//! tracing), and that a parked tab drops them.

use super::read_errors_tests::GIT_SAID;
use super::tests::{said, scratch_repo, state};
use super::update;
use crate::effect::{LoadKind, StatusRead};
use crate::git_env::{GitEnvMsg, not_owned_text};
use crate::msg::{BlameMsg, LoadMsg, Msg, RepoMsg, TabsMsg};
use crate::state::signatures::{BadgeState, badge};
use crate::state::{AppState, Selection, Tab};
use glowfly_core::error::ReadFailed as R;
use glowfly_core::repo::{self, DiffTarget, LoadOptions};
use glowfly_core::safe_directory::Refusal;
use glowfly_core::signing::SigningFormat;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The languages with the words `not_a_repo()` reads as in each.
const NOT_A_REPO: [(&str, &str); 3] =
    [("en", "/r is not a git repository"), ("de", "/r ist kein Git-Repository"), ("es", "/r no es un repositorio git")];

/// The headline of a failed open of `/r`, in the order of `NOT_A_REPO`.
const OPEN_FAILED: [&str; 3] = [
    "Could not open /r: /r is not a git repository",
    "/r konnte nicht geöffnet werden: /r ist kein Git-Repository",
    "No se pudo abrir /r: /r no es un repositorio git",
];

fn not_a_repo() -> R {
    R::NotARepository { path: "/r".into() }
}

fn send(st: &mut AppState, msg: Msg) {
    update(st, msg, Instant::now());
}

/// The text of the last activity line, which must be a failure.
fn last_failure(st: &AppState) -> &str {
    let line = st.log.last().expect("a log line");
    assert!(!line.ok, "{}", line.text);
    &line.text
}

/// A state in `code` whose only tab shows `dir` loaded in full (no load for more pending).
fn opened(dir: &Path, code: &str) -> AppState {
    let mut st = state();
    st.settings.language = code.into();
    st.tabs[0] = Tab::unloaded(dir.to_path_buf());
    let mut snap = repo::load(dir, &LoadOptions::all(10)).unwrap();
    snap.loaded_with.max_commits = st.commits_wanted();
    st.snap = Some(snap);
    assert!(!st.loading_more());
    st
}

fn status_failed(generation: u64, result: R) -> Msg {
    Msg::Load(LoadMsg::StatusLoaded {
        generation,
        result: Err(result),
        submodules: None,
        open_stamp: None,
        slow: None,
        lfs_patterns: None,
        index: None,
    })
}

fn unchanged_failed(generation: u64, result: R) -> Msg {
    Msg::Load(LoadMsg::RepoUnchanged {
        generation,
        result: Err(result),
        submodules: None,
        open_stamp: None,
        slow: None,
        lfs_patterns: None,
        index: None,
    })
}

#[test]
fn a_failed_open_is_logged_in_each_language_and_closes_its_tab() {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-test-open-failed-{}", std::process::id()));
    for ((code, _), want) in NOT_A_REPO.into_iter().zip(OPEN_FAILED) {
        let mut st = state();
        st.settings.language = code.into();
        send(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
        assert!(st.tabs.iter().any(|t| t.path.as_deref() == Some(dir.as_path())), "{code}: a tab for it");
        let why = Box::new(not_a_repo());
        let result = Err(R::OpenFailed { path: "/r".into(), why });
        let g = st.load_gen;
        send(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation: g, kind: LoadKind::Open, result }));
        assert_eq!(last_failure(&st), want, "{code}");
        assert!(st.tabs.iter().all(|t| t.path.as_deref() != Some(dir.as_path())), "{code}: its tab closes");
    }
}

#[test]
fn a_failed_refresh_while_loading_more_is_held_worded() {
    let dir = scratch_repo("read-errors-more");
    for (code, want) in NOT_A_REPO {
        let mut st = opened(&dir, code);
        st.snap.as_mut().unwrap().loaded_with.max_commits = 1;
        assert!(st.loading_more());
        let result = Err(not_a_repo());
        let g = st.load_gen;
        send(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation: g, kind: LoadKind::Refresh, result }));
        assert_eq!(st.refresh.more_failed.as_deref(), Some(want), "{code}");
        assert_eq!(last_failure(&st), want, "{code}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_freed_tabs_failed_reload_keeps_the_worded_text() {
    let dir = scratch_repo("read-errors-freed");
    for (code, want) in NOT_A_REPO {
        let mut st = opened(&dir, code);
        st.session.tab_view.history_freed = Some(5);
        let result = Err(not_a_repo());
        let g = st.load_gen;
        send(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation: g, kind: LoadKind::Refresh, result }));
        assert_eq!(st.session.tab_view.history_reload_error.as_deref(), Some(want), "{code}");
        assert_eq!(last_failure(&st), want, "{code}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_load_for_more_is_held_worded() {
    let dir = scratch_repo("read-errors-extended");
    for (code, want) in NOT_A_REPO {
        let mut st = opened(&dir, code);
        let g = st.load_gen;
        send(&mut st, Msg::Load(LoadMsg::HistoryExtended { generation: g, result: Err(not_a_repo()) }));
        assert_eq!(st.refresh.more_failed.as_deref(), Some(want), "{code}");
        assert_eq!(last_failure(&st), want, "{code}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failed_detail_diff_and_status_reads_are_logged_worded() {
    let dir = scratch_repo("read-errors-logged");
    for (code, want) in NOT_A_REPO {
        let mut st = opened(&dir, code);
        let oid = st.snap.as_ref().unwrap().commits[0].oid;
        st.session.selection = Selection::Commit(oid);
        send(&mut st, Msg::Repo(RepoMsg::DetailLoaded { oid, result: Err(not_a_repo()) }));
        assert_eq!(last_failure(&st), want, "{code}: detail");

        st.log.clear();
        let target = DiffTarget::Unstaged("a.txt".into());
        let generation = st.session.diff.generation;
        let diff = |reload| RepoMsg::DiffLoaded {
            target: target.clone(),
            reload,
            generation,
            result: Err(not_a_repo()),
            lfs: None,
            raw: None,
            gitlink: None,
        };
        send(&mut st, Msg::Repo(diff(false)));
        assert_eq!(last_failure(&st), want, "{code}: diff");
        st.log.clear();
        send(&mut st, Msg::Repo(diff(true)));
        assert!(st.log.is_empty(), "{code}: a reload that fails closes or follows, as before");

        let g = st.load_gen;
        send(&mut st, unchanged_failed(g, not_a_repo()));
        assert_eq!(last_failure(&st), want, "{code}: unchanged refs, failed status");
        st.log.clear();
        let g = st.load_gen;
        send(&mut st, status_failed(g, not_a_repo()));
        assert_eq!(last_failure(&st), want, "{code}: status");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn libgit2s_words_are_kept_in_every_language() {
    let dir = scratch_repo("read-errors-said");
    for (code, _) in NOT_A_REPO {
        let mut st = opened(&dir, code);
        let g = st.load_gen;
        send(&mut st, status_failed(g, said(GIT_SAID)));
        assert_eq!(last_failure(&st), GIT_SAID, "{code}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_signature_check_reads_in_the_badge_tooltip() {
    let dir = scratch_repo("read-errors-signature");
    for (code, want) in NOT_A_REPO {
        let mut st = opened(&dir, code);
        let oid = st.snap.as_ref().unwrap().commits[0].oid;
        let generation = st.session.reads.signatures.start(oid);
        send(&mut st, Msg::Repo(RepoMsg::SignatureChecked { oid, generation, result: Err(not_a_repo()) }));
        let result = st.session.reads.signatures.get(oid).expect("the check is kept");
        let tip = badge(st.settings.lang(), SigningFormat::Ssh, BadgeState::Done(result)).tooltip;
        assert!(tip.contains(want), "{code}: {tip}");
        assert!(st.log.is_empty(), "{code}: the badge says it, not the log");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failed_line_counts_log_nothing() {
    let dir = scratch_repo("read-errors-line-counts");
    let mut st = opened(&dir, "de");
    let oid = st.snap.as_ref().unwrap().commits[0].oid;
    send(&mut st, Msg::Repo(RepoMsg::CommitLineCountsLoaded { oid, result: Err(not_a_repo()) }));
    send(&mut st, Msg::Repo(RepoMsg::WipLineCountsLoaded { generation: 0, result: Err(not_a_repo()) }));
    assert!(st.log.is_empty() && st.toasts.is_empty(), "{:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_owned_by_another_user_is_logged_once() {
    let root = glowfly_core::paths::temp_dir().join(format!("glowfly-test-not-owned-{}", std::process::id()));
    for (code, _) in NOT_A_REPO {
        let mut st = state();
        st.settings.language = code.into();
        send(&mut st, Msg::Repo(RepoMsg::Open(root.clone())));
        let refusal = || Refusal { root: root.clone(), owner: None, admins: false };
        let g = st.load_gen;
        send(&mut st, Msg::GitEnv(GitEnvMsg::NotOwned { generation: g, kind: LoadKind::Open, refusal: refusal() }));
        let text = not_owned_text(st.settings.lang(), &refusal());
        assert_eq!(st.log.iter().filter(|l| l.text == text).count(), 1, "{code}: {:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    }
}

/// A state whose first tab shows `dir` (one modified file), parked behind a new empty tab.
fn parked(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    std::fs::write(dir.join("a.txt"), "changed\n").unwrap();
    let mut st = opened(&dir, "de");
    send(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert_eq!(st.active_tab, 1);
    (st, dir)
}

/// `msg` for the repository at `dir`, as the runner scopes its results.
fn scoped(dir: &Path, msg: Msg) -> Msg {
    Msg::Scoped { workdir: dir.to_path_buf(), msg: Box::new(msg) }
}

#[test]
fn a_parked_tab_drops_its_failed_reads() {
    let loaded = || RepoMsg::RepoLoaded { generation: 0, kind: LoadKind::Refresh, result: Err(not_a_repo()) };
    let blame = || BlameMsg::Loaded { path: "a.txt".into(), generation: 0, result: Err(not_a_repo()) };
    let status = |m: Msg| match m {
        Msg::Load(m) => m,
        _ => unreachable!(),
    };
    assert!(loaded().on_inactive().is_none());
    assert!(blame().on_inactive().is_none());
    assert!(status(unchanged_failed(0, not_a_repo())).on_inactive().is_none());
    assert!(status(status_failed(0, not_a_repo())).on_inactive().is_none());

    let (mut st, dir) = parked("read-errors-parked");
    let before = st.tab_glance(0).counts.map(|c| c.changed);
    assert_eq!(before, Some(1));
    for msg in [Msg::Repo(loaded()), unchanged_failed(0, not_a_repo()), status_failed(0, not_a_repo()), Msg::Blame(blame())] {
        send(&mut st, scoped(&dir, msg));
    }
    assert_eq!(st.active_tab, 1, "still parked");
    assert!(st.log.is_empty() && st.toasts.is_empty(), "{:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    assert_eq!(st.tab_glance(0).counts.map(|c| c.changed), before, "its counts stay");

    // A load that succeeds still moves them.
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    let snap = repo::load(&dir, &LoadOptions::all(10)).unwrap();
    let result = Ok(Box::new(snap));
    send(&mut st, scoped(&dir, Msg::Repo(RepoMsg::RepoLoaded { generation: 0, kind: LoadKind::Refresh, result })));
    assert_eq!(st.tab_glance(0).counts.map(|c| c.changed), Some(2));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_whole_status_for_a_parked_tab_is_still_kept() {
    let (mut st, dir) = parked("read-errors-parked-status");
    let rows = vec![glowfly_core::repo::FileEntry::new("a.txt", 'M'), glowfly_core::repo::FileEntry::new("c.txt", '?')];
    let read = StatusRead::Whole(rows, Vec::new(), git2::RepositoryState::Clean);
    let msg = Msg::Load(LoadMsg::StatusLoaded {
        generation: 0,
        result: Ok(read),
        submodules: None,
        open_stamp: None,
        slow: None,
        lfs_patterns: None,
        index: None,
    });
    send(&mut st, scoped(&dir, msg));
    assert_eq!(st.tab_glance(0).counts.map(|c| c.changed), Some(2));
    let _ = std::fs::remove_dir_all(&dir);
}
