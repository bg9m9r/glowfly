//! The words of a failed write to the working tree or index, revert, restore from a commit,
//! working-tree file operation and stack push (#367, `WriteFailed`): every variant through
//! `typed::write` in English (exactly its `Display`), German and Spanish; where the results show
//! them, in the user's language; and a parked tab wording them in its own log, keeping the undo.

use super::tab_glance_tests::job;
use super::tests::{scratch_repo, state};
use super::update;
use crate::effect::{FileOp, StackPushJob};
use crate::msg::{FilesMsg, Msg, RevertWhat, StackMsg, StageMsg, TabsMsg, WorkFileMsg};
use crate::state::error_words::typed;
use crate::state::{AppState, Tab};
use glowfly_core::error::WriteFailed as W;
use glowfly_core::i18n::Lang;

use glowfly_core::undo::{Action, Discarded};
use std::path::Path;
use std::time::Instant;

/// A number per variant; a new variant fails to compile here until it is listed in `cases()`.
fn index(e: &W) -> usize {
    match e {
        W::Said { .. } => 0,
        W::MissingContent { .. } => 1,
        W::Panicked { .. } => 2,
        W::Other { .. } => 3,
    }
}

/// Each variant, with its English, German and Spanish words.
fn cases() -> Vec<(W, [&'static str; 3])> {
    vec![
        (W::Said { said: "index.lock exists".into() }, ["index.lock exists"; 3]),
        (W::Other { said: "not found: a.txt".into() }, ["not found: a.txt"; 3]),
        (
            W::MissingContent { path: "big.bin".into(), remote: "origin".into() },
            [
                "the contents of big.bin haven't been downloaded from origin yet",
                "der Inhalt von big.bin wurde noch nicht von origin heruntergeladen",
                "el contenido de big.bin aún no se ha descargado de origin",
            ],
        ),
        (
            W::Panicked { effect: "local-op".into(), panic: "boom".into() },
            [
                "Glowfly hit an internal error (local-op): boom",
                "In Glowfly ist ein interner Fehler aufgetreten (local-op): boom",
                "Glowfly tuvo un error interno (local-op): boom",
            ],
        ),
    ]
}

const LANGS: [Lang; 3] = [Lang::En, Lang::De, Lang::Es];

fn missing() -> W {
    W::MissingContent { path: "big.bin".into(), remote: "origin".into() }
}

const MISSING_DE: &str = "der Inhalt von big.bin wurde noch nicht von origin heruntergeladen";

#[test]
fn every_variant_in_each_language() {
    let all = cases();
    let mut seen: Vec<usize> = all.iter().map(|(e, _)| index(e)).collect();
    seen.sort_unstable();
    assert_eq!(seen, [0, 1, 2, 3]);
    for (e, words) in all {
        assert_eq!(typed::write(Lang::En, &e), e.to_string(), "English is its Display");
        for (t, want) in LANGS.into_iter().zip(words) {
            assert_eq!(typed::write(t, &e), want, "{t:?} {e:?}");
        }
    }
}

fn send(st: &mut AppState, msg: Msg) {
    update(st, msg, Instant::now());
}

/// A state in `code` showing `dir`.
fn shown(dir: &Path, code: &str) -> AppState {
    let mut st = state();
    st.settings.language = code.into();
    st.tabs[0] = Tab::unloaded(dir.to_path_buf());
    st
}

fn last_failure(st: &AppState) -> &str {
    let line = st.log.last().expect("a log line");
    assert!(!line.ok, "{}", line.text);
    &line.text
}

fn undo() -> Action {
    Action::Discard { path: "f.txt".into(), what: Discarded::RevertedHunk, before: None, after: None }
}

#[test]
fn a_failed_local_op_is_worded_in_the_users_language() {
    let dir = scratch_repo("write-errors-local-op");
    let mut st = shown(&dir, "de");
    send(&mut st, Msg::Stage(StageMsg::LocalOpDone(Err(missing()))));
    assert_eq!(last_failure(&st), MISSING_DE);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conflict_change_that_failed_partway_says_undo_puts_it_back() {
    let dir = scratch_repo("write-errors-partial");
    for (code, want) in [
        ("en", "b.txt failed. Undo puts back the files it changed before it stopped."),
        ("es", "b.txt failed. Deshacer devuelve los archivos que cambió antes de detenerse."),
    ] {
        let mut st = shown(&dir, code);
        let error = W::Said { said: "b.txt failed".into() };
        send(&mut st, Msg::Stage(StageMsg::LocalOpFailed { error, undo: undo() }));
        assert_eq!(last_failure(&st), want);
        assert_eq!(st.session.undo.undo.last(), Some(&undo()));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_revert_restore_or_file_op_gives_the_reason_in_the_users_language() {
    let dir = scratch_repo("write-errors-results");
    let mut st = shown(&dir, "de");
    let reverted = StageMsg::Reverted { path: "f.txt".into(), what: RevertWhat::File, result: Err(missing()), undo: None };
    send(&mut st, Msg::Stage(reverted));
    assert_eq!(last_failure(&st), format!("Could not revert f.txt: {MISSING_DE}"));

    let restored = FilesMsg::FileRestored { path: "a.txt".into(), oid: git2::Oid::zero(), result: Err(missing()), undo: None };
    send(&mut st, Msg::Files(restored));
    assert_eq!(last_failure(&st), format!("Could not restore a.txt: {MISSING_DE}"));

    send(&mut st, Msg::WorkFile(WorkFileMsg::WorkFileDone { op: FileOp::Create("x.txt".into()), result: Err(missing()) }));
    assert!(last_failure(&st).ends_with(MISSING_DE), "{}", last_failure(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_stack_push_ends_its_job_with_the_reason() {
    let dir = scratch_repo("write-errors-push");
    let mut st = shown(&dir, "de");
    let mut running = job(&dir);
    running.label = "Push stack".into();
    running.fetches = false;
    st.job = Some(running);
    let push = Box::new(StackPushJob { targets: vec![], then_prs: None });
    let error = W::Panicked { effect: "stack-push".into(), panic: "boom".into() };
    send(&mut st, Msg::Stack(StackMsg::Pushed { result: Err(error), job: push }));
    assert!(st.job.is_none());
    let want = "In Glowfly ist ein interner Fehler aufgetreten (stack-push): boom";
    assert!(st.log.iter().any(|l| !l.ok && l.text.contains(want)), "{:?}", st.log.last().map(|l| &l.text));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A state in `code` whose first tab shows `dir`, parked behind a new empty tab.
fn parked(dir: &Path, code: &str) -> AppState {
    let mut st = shown(dir, code);
    st.snap = Some(glowfly_core::repo::load(dir, &glowfly_core::repo::LoadOptions::all(10)).unwrap());
    send(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert_eq!(st.active_tab, 1);
    st
}

fn scoped(dir: &Path, msg: Msg) -> Msg {
    Msg::Scoped { workdir: dir.to_path_buf(), msg: Box::new(msg) }
}

#[test]
fn a_parked_tab_words_its_failed_writes_and_keeps_their_undo() {
    let dir = scratch_repo("write-errors-parked");
    let mut st = parked(&dir, "es");
    let es = "el contenido de big.bin aún no se ha descargado de origin";
    let cases: Vec<(Msg, String, bool)> = vec![
        (Msg::Stage(StageMsg::LocalOpDone(Err(missing()))), es.to_owned(), false),
        (
            Msg::Stage(StageMsg::LocalOpFailed { error: missing(), undo: undo() }),
            format!("{es}. Deshacer devuelve los archivos que cambió antes de detenerse."),
            true,
        ),
        (
            Msg::Stage(StageMsg::Reverted { path: "f.txt".into(), what: RevertWhat::Hunk, result: Err(missing()), undo: Some(undo()) }),
            format!("Could not revert the hunk in f.txt: {es}"),
            true,
        ),
        (
            Msg::Files(FilesMsg::FileRestored { path: "a.txt".into(), oid: git2::Oid::zero(), result: Err(missing()), undo: Some(undo()) }),
            format!("Could not restore a.txt: {es}. Undo puts back what it held before."),
            true,
        ),
    ];
    for (msg, want, keeps_undo) in cases {
        let before = st.tabs[0].parked.as_ref().map_or(0, |p| p.session.undo.undo.len());
        assert!(update(&mut st, scoped(&dir, msg), Instant::now()).is_empty());
        let line = st.tabs[0].log.last().expect("a line in the parked tab's log");
        assert_eq!((line.ok, line.text.as_str()), (false, want.as_str()));
        let after = st.tabs[0].parked.as_ref().map_or(0, |p| p.session.undo.undo.len());
        assert_eq!(after, before + usize::from(keeps_undo), "{want}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
