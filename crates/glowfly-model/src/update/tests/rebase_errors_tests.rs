//! The words of a refused or failed rebase, graph history edit, remaining-steps edit and restack
//! (#367, `RebaseError`): every variant through `typed::rebase` in English (exactly its
//! `Display`, the sentence it has always read), German and Spanish with short ids, branch names
//! and paths kept as they came; and where `RebaseMsg`'s results show them in the user's language.

use super::tab_glance_tests::job;
use super::tests::{scratch_repo, state};
use super::update;
use crate::msg::{Msg, RebaseMsg};
use crate::state::error_words::typed;
use crate::state::rebase::RebasePanel;
use crate::state::rebase_quick::QUICK_STALE;
use crate::state::{AppState, Tab};
use glowfly_core::i18n::Lang;
use glowfly_core::rebase::{Busy, LeftOut, RebaseBase, RebaseError as E};
use glowfly_core::stack::RestackPlan;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// How many variants `RebaseError` has; `index` must give each its own number below it.
const VARIANTS: usize = 46;

/// A number per variant. A new variant fails to compile here until it is numbered and listed
/// in `every()`.
fn index(e: &E) -> usize {
    match e {
        E::Stale => 0,
        E::Changed => 1,
        E::Detached => 2,
        E::FirstCombines => 3,
        E::EmptyReword => 4,
        E::NotOnBranch => 5,
        E::SomeNotOnBranch => 6,
        E::MergeSelected => 7,
        E::SomeMergeSelected => 8,
        E::SideLine => 9,
        E::SomeSideLine => 10,
        E::BranchTip => 11,
        E::SomeBranchTips => 12,
        E::SplitByBranch => 13,
        E::CombineAfterMerge => 14,
        E::LostToReset => 15,
        E::RootSquash => 16,
        E::ParentMerge => 17,
        E::NotConsecutive => 18,
        E::TooFewToSquash => 19,
        E::DropsEverything => 20,
        E::NotRebasing => 21,
        E::EmptyCombined => 22,
        E::EmptyExec => 23,
        E::ExecOneLine => 24,
        E::CombineAfterStep => 25,
        E::NotInteractive => 26,
        E::UnsupportedSteps => 27,
        E::UpdateRefInGroup => 28,
        E::RefsChanged => 29,
        E::TodoMoved => 30,
        E::NotUtf8Branch => 31,
        E::StackChanged => 32,
        E::StillRebasing => 33,
        E::NoCommitsYet => 34,
        E::EditTodoFailed => 35,
        E::InProgress { .. } => 36,
        E::CommitNotOnBranch { .. } => 37,
        E::NothingToRebase { .. } => 38,
        E::BranchNotFound { .. } => 39,
        E::CommitNotFound { .. } => 40,
        E::Held { .. } => 41,
        E::NotMoved { .. } => 42,
        E::Said { .. } => 43,
        E::Panicked { .. } => 44,
        E::Other { .. } => 45,
    }
}

/// The variants without fields: one sentence each.
fn sentences() -> Vec<E> {
    vec![
        E::Stale,
        E::Changed,
        E::Detached,
        E::FirstCombines,
        E::EmptyReword,
        E::NotOnBranch,
        E::SomeNotOnBranch,
        E::MergeSelected,
        E::SomeMergeSelected,
        E::SideLine,
        E::SomeSideLine,
        E::BranchTip,
        E::SomeBranchTips,
        E::SplitByBranch,
        E::CombineAfterMerge,
        E::LostToReset,
        E::RootSquash,
        E::ParentMerge,
        E::NotConsecutive,
        E::TooFewToSquash,
        E::DropsEverything,
        E::NotRebasing,
        E::EmptyCombined,
        E::EmptyExec,
        E::ExecOneLine,
        E::CombineAfterStep,
        E::NotInteractive,
        E::UnsupportedSteps,
        E::UpdateRefInGroup,
        E::RefsChanged,
        E::TodoMoved,
        E::NotUtf8Branch,
        E::StackChanged,
        E::StillRebasing,
        E::NoCommitsYet,
        E::EditTodoFailed,
    ]
}

fn left_out(name: &str, at: Option<&str>) -> LeftOut {
    LeftOut { name: name.into(), checked_out_at: at.map(str::to_owned) }
}

/// The variants that name something, with the words they name (kept in every language).
fn named() -> Vec<(E, Vec<&'static str>)> {
    vec![
        (E::InProgress { op: Busy::Merge }, vec![]),
        (E::InProgress { op: Busy::Bisect }, vec![]),
        (E::CommitNotOnBranch { commit: "1a2b3c4".into() }, vec!["1a2b3c4"]),
        (E::NothingToRebase { branch: "topic".into(), onto: "main".into() }, vec!["topic", "main"]),
        (E::BranchNotFound { name: "gone".into() }, vec!["gone"]),
        (E::CommitNotFound { commit: "9f8e7d6".into() }, vec!["9f8e7d6"]),
        (E::Held { alias: "refs/heads/alias".into(), target: "feature".into(), elsewhere: true }, vec!["refs/heads/alias", "feature"]),
        (E::Held { alias: "HEADS".into(), target: "refs/tags/v1".into(), elsewhere: false }, vec!["HEADS", "refs/tags/v1"]),
        (E::NotMoved { branches: vec![left_out("side", None), left_out("other", None)] }, vec!["side, other"]),
        (E::NotMoved { branches: vec![left_out("side", Some("/work/wt")), left_out("b", None)] }, vec!["side", "/work/wt", "b"]),
        (E::Panicked { effect: "rebase".into(), panic: "boom".into() }, vec!["rebase", "boom"]),
    ]
}

/// Every variant at least once.
fn every() -> Vec<E> {
    let mut all = sentences();
    all.extend(named().into_iter().map(|(e, _)| e));
    all.push(E::Said { said: "error: could not apply 1a2b3c4".into() });
    all.push(E::Other { said: "invalid utf-8 in the message".into() });
    all
}

#[test]
fn every_variant_is_listed() {
    let seen: BTreeSet<usize> = every().iter().map(index).collect();
    assert_eq!(seen, (0..VARIANTS).collect::<BTreeSet<_>>());
}

#[test]
fn english_reads_as_it_always_has() {
    for e in every() {
        assert_eq!(typed::rebase(Lang::En, &e), e.to_string(), "{e:?}");
    }
}

#[test]
fn german_and_spanish_word_it_and_keep_the_names() {
    for t in [Lang::De, Lang::Es] {
        for e in sentences() {
            let words = typed::rebase(t, &e);
            assert!(!words.is_empty() && words != e.to_string(), "{t:?} {e:?}: {words}");
        }
        for (e, kept) in named() {
            let words = typed::rebase(t, &e);
            assert_ne!(words, e.to_string(), "{t:?} {e:?}");
            for k in kept {
                assert!(words.contains(k), "{t:?} {e:?} keeps {k}: {words}");
            }
        }
    }
}

#[test]
fn git_and_other_words_pass_through() {
    for t in [Lang::En, Lang::De, Lang::Es] {
        assert_eq!(typed::rebase(t, &E::Said { said: "fatal: bad revision".into() }), "fatal: bad revision");
        assert_eq!(typed::rebase(t, &E::Other { said: "not found: x".into() }), "not found: x");
    }
}

#[test]
fn the_words_in_german_and_spanish() {
    let cases: [(E, &str, &str); 5] = [
        (
            E::Stale,
            "Der Branch hat sich geändert, seit diese Liste erstellt wurde. Setze sie zurück, um sie neu zu laden.",
            "La rama cambió desde que se creó esta lista. Restablécela para volver a cargarla.",
        ),
        (
            E::CommitNotOnBranch { commit: "1a2b3c4".into() },
            "1a2b3c4 ist nicht auf dem aktuellen Branch",
            "1a2b3c4 no está en la rama actual",
        ),
        (
            E::Held { alias: "a".into(), target: "t".into(), elsewhere: true },
            "a zeigt auf t, das in einem anderen Worktree ausgecheckt ist; Git würde t mit diesem Rebase verschieben (rebase.updateRefs). Lösche zuerst a oder wechsle in diesem Worktree auf einen anderen Branch.",
            "a apunta a t, que tiene checkout en otro worktree; git movería t con este rebase (rebase.updateRefs). Elimina a o cambia ese worktree a otra rama primero.",
        ),
        (
            E::NotMoved { branches: vec![left_out("side", Some("/work/wt"))] },
            "Git verschiebt side (ausgecheckt in /work/wt) mit diesem Rebase nicht (rebase.updateRefs), die Liste aber schon. Wechsle in diesem Worktree auf einen anderen Branch oder schalte rebase.updateRefs aus und versuche es noch einmal.",
            "git no moverá side (con checkout en /work/wt) con este rebase (rebase.updateRefs), aunque la lista sí. Cambia ese worktree a otra rama o desactiva rebase.updateRefs, e inténtalo de nuevo.",
        ),
        (
            E::InProgress { op: Busy::Merge },
            "Schließe den laufenden Merge ab oder brich ihn ab, bevor du einen interaktiven Rebase startest.",
            "Termina o cancela la fusión en curso antes de iniciar un rebase interactivo.",
        ),
    ];
    for (e, de, es) in cases {
        assert_eq!(typed::rebase(Lang::De, &e), de);
        assert_eq!(typed::rebase(Lang::Es, &e), es);
    }
}

fn send(st: &mut AppState, msg: Msg) {
    update(st, msg, Instant::now());
}

/// A state in `code` showing `dir` loaded, with an interactive rebase job running there.
fn rebasing(dir: &Path, code: &str) -> AppState {
    let mut st = state();
    st.settings.language = code.into();
    st.tabs[0] = Tab::unloaded(dir.to_path_buf());
    st.snap = Some(glowfly_core::repo::load(dir, &glowfly_core::repo::LoadOptions::all(10)).unwrap());
    let mut running = job(dir);
    running.label = "Interactive rebase".into();
    running.fetches = false;
    st.job = Some(running);
    st
}

fn panel() -> RebasePanel {
    RebasePanel::loading(RebaseBase::FromCommit(git2::Oid::zero()))
}

fn last_failure(st: &AppState) -> (&str, &str) {
    let log = st.log.last().expect("a log line");
    let toast = st.toasts.last().expect("a toast");
    assert!(!log.ok && !toast.ok);
    (&log.text, &toast.text)
}

#[test]
fn a_refused_plan_is_worded_in_the_users_language() {
    let dir = scratch_repo("rebase-errors-plan");
    let mut st = rebasing(&dir, "de");
    st.job = None;
    st.session.rebase.view = Some(panel());
    let generation = st.session.rebase.generation;
    let result = Err(E::CommitNotOnBranch { commit: "1a2b3c4".into() });
    send(&mut st, Msg::Rebase(RebaseMsg::RebasePlanLoaded { generation, result }));
    assert!(st.session.rebase.view.is_none());
    let want = "1a2b3c4 ist nicht auf dem aktuellen Branch";
    assert_eq!(last_failure(&st), (want, want));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stale_edit_without_the_panel_says_to_try_again() {
    let dir = scratch_repo("rebase-errors-quick-stale");
    let mut st = rebasing(&dir, "en");
    send(&mut st, Msg::Rebase(RebaseMsg::RebaseFinished { result: Err(E::Stale), undo: None }));
    assert!(st.job.is_none());
    let toast = st.toasts.last().expect("the failure toast");
    assert_eq!(toast.text, format!("Interactive rebase failed: {QUICK_STALE}"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refused_start_keeps_the_panel_with_its_words() {
    let dir = scratch_repo("rebase-errors-start");
    let mut st = rebasing(&dir, "es");
    st.session.rebase.view = Some(panel());
    send(&mut st, Msg::Rebase(RebaseMsg::RebaseFinished { result: Err(E::Stale), undo: None }));
    let es = "La rama cambió desde que se creó esta lista. Restablécela para volver a cargarla.";
    let p = st.session.rebase.view.as_ref().expect("the panel stays");
    assert_eq!((p.error.as_deref(), p.stale), (Some(es), true));
    assert!(st.toasts.iter().any(|t| !t.ok && t.text == format!("Interactive rebase failed: {es}")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_a_moved_todo_marks_the_steps_stale() {
    let dir = scratch_repo("rebase-errors-edited");
    for (e, stale) in [(E::TodoMoved, true), (E::Changed, false)] {
        let mut st = rebasing(&dir, "de");
        st.session.rebase.view = Some(panel());
        let words = typed::rebase(Lang::De, &e);
        send(&mut st, Msg::Rebase(RebaseMsg::RemainingEdited { result: Err(e) }));
        let p = st.session.rebase.view.as_ref().expect("the panel stays");
        assert_eq!((p.error.as_deref(), p.stale), (Some(words.as_str()), stale));
        assert!(st.toasts.iter().any(|t| !t.ok && t.text == format!("Couldn't update the rebase steps: {words}")));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refused_graph_edit_is_worded_in_the_users_language() {
    let dir = scratch_repo("rebase-errors-prepared");
    let mut st = rebasing(&dir, "de");
    st.job = None;
    let workdir: PathBuf = dir.clone();
    send(&mut st, Msg::Rebase(RebaseMsg::EditPrepared { workdir, result: Err(E::RootSquash) }));
    let want = "Der erste Commit hat keinen Eltern-Commit, in den er gesquasht werden könnte.";
    assert_eq!(last_failure(&st), (want, want));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_restack_that_could_not_end_logs_its_words() {
    let dir = scratch_repo("rebase-errors-restack");
    let mut st = rebasing(&dir, "es");
    let plan = RestackPlan { steps: vec![], return_to: None, autostash: false, stash: None };
    let restack = crate::effect::RestackJob { plan, before: None, detail: String::new(), stopped: None, stash_op: None };
    let msg = RebaseMsg::RestackEnded { result: Err(E::StackChanged), undo: None, job: Box::new(restack) };
    send(&mut st, Msg::Rebase(msg));
    assert!(st.job.is_none());
    let es = "La pila cambió desde que se planificó este reapilado. Inténtalo de nuevo.";
    assert!(st.log.iter().any(|l| !l.ok && l.text.contains(es)), "{:?}", st.log.last().map(|l| &l.text));
    let _ = std::fs::remove_dir_all(&dir);
}
