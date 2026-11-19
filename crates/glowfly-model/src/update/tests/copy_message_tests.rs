//! Copy > Message (update/copy_message.rs): the read it asks for and its metadata, the text it
//! copies, the toast for a commit that can't be read in each language, no repository, and a copy
//! that lands whichever tab is active.

use super::tests::{said, scratch_repo, state};
use super::update;
use crate::effect::{CommitEffect, Effect, EffectMeta, SandboxRule, UiEffect};
use crate::msg::{Msg, RepoMsg, TabsMsg};
use crate::state::AppState;
use git2::Oid;
use glowfly_core::repo::{self, LoadOptions};
use std::path::Path;
use std::time::Instant;

fn oid() -> Oid {
    Oid::from_str("1234567890123456789012345678901234567890").unwrap()
}

/// A state showing the repository in `dir`.
fn opened(dir: &Path) -> AppState {
    let mut st = state();
    st.snap = Some(repo::load(dir, &LoadOptions::all(10)).unwrap());
    st
}

fn send(st: &mut AppState, msg: RepoMsg) -> Vec<Effect> {
    update(st, Msg::Repo(msg), Instant::now())
}

#[test]
fn copy_message_asks_for_the_read() {
    let dir = scratch_repo("copy-message-update");
    let mut st = opened(&dir);
    let workdir = st.workdir().unwrap();
    let fx = send(&mut st, RepoMsg::CopyMessage(oid()));
    let [Effect::Commit(read @ CommitEffect::CopyMessage { workdir: w, oid: o })] = fx.as_slice() else { panic!("one read: {fx:?}") };
    assert_eq!((w, *o), (&workdir, oid()));
    assert_eq!(read.name(), "copy-message");
    assert!(read.scope().is_none(), "the clipboard isn't a tab's");
    assert!(read.queue().is_none());
    assert!(read.holds().is_none());
    assert!(read.timed());
    assert_eq!(read.sandbox(), SandboxRule::Run);
    assert!(st.toasts.is_empty() && st.log.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_message_read_is_copied_whole() {
    let mut st = state();
    let text = "Subject\n\nBody one\nBody two";
    let fx = send(&mut st, RepoMsg::MessageRead(Ok(text.into())));
    assert!(matches!(fx.as_slice(), [Effect::Ui(UiEffect::CopyText(t))] if t == text), "{fx:?}");
    assert!(st.toasts.is_empty() && st.log.is_empty());
}

#[test]
fn a_commit_that_cant_be_read_is_toasted_in_each_language() {
    for (code, text) in [
        ("en", "Couldn't copy the commit message: boom"),
        ("de", "Die Commit-Nachricht konnte nicht kopiert werden: boom"),
        ("es", "No se pudo copiar el mensaje del commit: boom"),
    ] {
        let mut st = state();
        st.settings.language = code.into();
        let fx = send(&mut st, RepoMsg::MessageRead(Err(said("boom"))));
        assert!(fx.is_empty(), "nothing is copied: {fx:?}");
        let toast = st.toasts.last().expect("a toast");
        assert!(!toast.ok, "{code}");
        assert_eq!(toast.text, text);
    }
}

#[test]
fn without_a_repository_nothing_is_read() {
    let mut st = state();
    st.snap = None;
    assert!(send(&mut st, RepoMsg::CopyMessage(oid())).is_empty());
}

#[test]
fn the_copy_lands_whichever_tab_is_active() {
    let dir = scratch_repo("copy-message-parked");
    let mut st = opened(&dir);
    assert!(RepoMsg::CopyMessage(oid()).on_inactive().is_none());
    assert!(RepoMsg::MessageRead(Ok("x".into())).on_inactive().is_none());
    update(&mut st, Msg::Tabs(TabsMsg::NewTab), Instant::now());
    let fx = send(&mut st, RepoMsg::MessageRead(Ok("x".into())));
    assert!(matches!(fx.as_slice(), [Effect::Ui(UiEffect::CopyText(t))] if t == "x"), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
