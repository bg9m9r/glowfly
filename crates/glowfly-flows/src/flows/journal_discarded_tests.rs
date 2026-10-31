//! Headless tests for the discarded changes in the journal's Lost tab: what Glowfly discarded is
//! listed again after the session is read from disk (a restart), Restore brings it back without
//! ever writing over a file (offering a copy next to it instead), a dropped stash goes back in
//! the stash list, and nothing is restored while a job runs.

use super::tests::{drive, git, scratch_repo, state};
use crate::journal::{JournalMsg, LostMsg};
use crate::msg::{JobMsg, Msg, RepoMsg, StageMsg};
use crate::state::AppState;
use crate::state::journal_lost::{JournalTab, LostView};
use crate::state::palette::StashAction;
use git2::Oid;
use glowfly_core::journal::LostKind;
use glowfly_core::undo::kept::KeptWhat;
use std::path::{Path, PathBuf};

fn read(dir: &Path, f: &str) -> String {
    std::fs::read_to_string(dir.join(f)).unwrap()
}

/// A new app state on `dir`, as after a restart: no undo history.
fn restarted(dir: &Path) -> AppState {
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    assert!(st.session.undo.undo.is_empty());
    st
}

/// Opens the journal on its Lost tab and lets `git fsck` and the kept records load.
fn lost_tab(st: &mut AppState) -> &LostView {
    drive(st, Msg::Journal(JournalMsg::Open));
    drive(st, Msg::Journal(JournalMsg::Tab(JournalTab::Lost)));
    lost(st)
}

fn lost(st: &AppState) -> &LostView {
    let view = st.session.journal.view.as_ref().and_then(|v| v.lost.as_ref()).expect("the Lost tab");
    assert!(matches!(view.result, Some(Ok(_))), "{:?}", view.result);
    view
}

fn last_toast(st: &AppState) -> (bool, String) {
    st.toasts.last().map(|t| (t.ok, t.text.clone())).expect("a toast")
}

/// a.txt edited to `work` and discarded through Glowfly, then the app restarted.
fn discarded(name: &str, work: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    std::fs::write(dir.join("a.txt"), work).unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Stage(StageMsg::DiscardFile("a.txt".into())));
    assert_ne!(read(&dir, "a.txt"), work, "discarded");
    drop(st);
    (restarted(&dir), dir)
}

fn kept_id(view: &LostView) -> Oid {
    let [c] = view.discarded() else { panic!("one discarded entry: {:?}", view.discarded()) };
    assert!(matches!(&c.kind, LostKind::Discarded { path, what: KeptWhat::File, .. } if path == "a.txt"), "{c:?}");
    c.oid
}

#[test]
fn a_discard_is_restored_from_the_journal_after_a_restart() {
    let (mut st, dir) = discarded("journal-discarded-restart", "precious work\n");
    let id = kept_id(lost_tab(&mut st));

    // a.txt is there (as committed): Restore refuses and offers a copy next to it.
    drive(&mut st, Msg::Journal(JournalMsg::Lost(LostMsg::RestoreFile { id, copy: false })));
    let (ok, text) = last_toast(&st);
    assert!(!ok && text.contains("a.txt is in your working tree") && text.contains("never replaces"), "{text}");
    assert_ne!(read(&dir, "a.txt"), "precious work\n", "untouched");
    let toast = st.toasts.last().unwrap();
    assert_eq!(toast.action.as_ref().map(|a| a.label.as_str()), Some("Restore as a copy"));
    let toast_id = toast.id;
    drive(&mut st, Msg::ToastAction(toast_id));
    assert_eq!(read(&dir, "a (restored).txt"), "precious work\n", "the copy holds the discarded work");
    assert_eq!(last_toast(&st), (true, "Restored a.txt as a (restored).txt".to_owned()));

    // With a.txt gone, Restore writes it at its own path, and the list no longer offers it.
    std::fs::remove_file(dir.join("a.txt")).unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Journal(JournalMsg::Lost(LostMsg::RestoreFile { id, copy: false })));
    assert_eq!(read(&dir, "a.txt"), "precious work\n", "the content comes back");
    assert_eq!(last_toast(&st), (true, "Restored a.txt".to_owned()));
    assert!(lost(&st).discarded().is_empty(), "back in place: {:?}", lost(&st).discarded());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_is_restored_while_a_job_runs() {
    let (mut st, dir) = discarded("journal-discarded-busy", "work\n");
    let id = kept_id(lost_tab(&mut st));
    std::fs::remove_file(dir.join("a.txt")).unwrap();
    st.job = Some(crate::state::RunningJob {
        label: "Fetch".into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: true,
    });
    let fx = super::update(&mut st, Msg::Journal(JournalMsg::Lost(LostMsg::RestoreFile { id, copy: false })), std::time::Instant::now());
    assert!(fx.is_empty(), "{fx:?}");
    assert!(!dir.join("a.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dropped_stash_goes_back_in_the_list_after_a_restart() {
    let dir = scratch_repo("journal-discarded-stash");
    git(&dir, &["stash", "push", "-q", "-m", "keep me"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let oid = st.snap.as_ref().unwrap().stashes[0].oid;
    drive(&mut st, Msg::Job(JobMsg::Stash { action: StashAction::Drop, index: 0, oid }));
    assert!(st.snap.as_ref().unwrap().stashes.is_empty());
    drop(st);

    let mut st = restarted(&dir);
    let view = lost_tab(&mut st);
    let [c] = view.discarded() else { panic!("{:?}", view.discarded()) };
    assert_eq!(c.oid, oid);
    assert!(matches!(c.kind, LostKind::Stash { .. }) && c.summary == "On main: keep me", "{c:?}");
    assert!(view.commits().iter().all(|c| c.oid != oid), "listed once, under the discarded changes");
    drive(&mut st, Msg::Journal(JournalMsg::Lost(LostMsg::Store(oid))));
    let back = st.snap.as_ref().unwrap().stashes.iter().find(|s| s.oid == oid).cloned().expect("the stash is back");
    assert_eq!(back.message, "On main: keep me");
    assert!(lost(&st).discarded().is_empty(), "in the stash list again");
    let _ = std::fs::remove_dir_all(&dir);
}
