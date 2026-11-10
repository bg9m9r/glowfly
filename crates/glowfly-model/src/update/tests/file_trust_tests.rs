//! Headless tests for the trust gate of working-tree file work (update/file_trust.rs): in a
//! repository where git is held (undecided or not trusted), file work that starts git waits for
//! the trust prompt and runs on Trust, while work that only uses libgit2 goes ahead; in a
//! trusted one it is sent with the runner's re-check. One group of operations per test.

use super::tests::state;
use super::{update, update_sealed};
use crate::effect::partial::{ContentRequest, PartialEffect};
use crate::effect::{DeleteMode, DiscardEach, Effect, FileOp, FilesEffect, LocalOp, Sealed, WaitingKind};
use crate::msg::{ConflictMsg, FilesMsg, Msg, StageMsg, TrustMsg, WorkFileMsg};
use crate::pull_request::{HeadJob, PrEffect};
use crate::state::files::PathForm;
use crate::state::op_fixture::{rows, snapshot};
use crate::state::trust::{record, record_trusted};
use crate::state::{AppState, Modal};
use glowfly_core::settings::RepoTrust;
use glowfly_core::trust::ConfigCommand;
use std::path::PathBuf;
use std::time::Instant;

/// A loaded repository with a.txt committed, whose scan found nothing.
fn clean(name: &str) -> AppState {
    let mut st = state();
    st.snap = Some(snapshot(name));
    st
}

/// `clean`, whose scan lists a filter program: undecided, so git is held.
fn held(name: &str) -> AppState {
    let mut st = clean(name);
    let snap = st.snap.as_mut().unwrap();
    snap.trust_scan.config.push(ConfigCommand { key: "filter.x.clean".into(), value: "x".into() });
    snap.trust_print = snap.trust_scan.print();
    assert!(st.held());
    st
}

/// `clean` the user chose not to trust.
fn refused(name: &str) -> AppState {
    let mut st = clean(name);
    let real = real(&st);
    record(&mut st.settings, &real, RepoTrust::NotTrusted);
    assert!(st.held());
    st
}

/// `held`, trusted on what its scan lists.
fn trusted(name: &str) -> AppState {
    let mut st = held(name);
    let print = st.snap.as_ref().unwrap().trust_print.clone();
    let real = real(&st);
    record_trusted(&mut st.settings, &real, print);
    assert!(st.runs_git_unasked());
    st
}

fn real(st: &AppState) -> PathBuf {
    st.snap.as_ref().unwrap().real_workdir.clone()
}

fn send(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update_sealed(st, msg, Instant::now())
}

/// The file effects among `fx`, each with what its re-check is sealed as.
fn files(fx: Vec<Effect>) -> Vec<(FilesEffect, Option<Sealed>)> {
    let mut out = Vec::new();
    for e in fx {
        let sealed = e.trust_check().map(|c| c.sealed.clone());
        if let Effect::Files(f) = e.unsealed() {
            out.push((f, sealed));
        }
    }
    out
}

/// The label of what the open trust prompt says waits.
fn waits(st: &AppState) -> Option<String> {
    match &st.modal {
        Some(Modal::Trust(p)) => p.held.clone(),
        _ => None,
    }
}

fn decide(st: &mut AppState, trusted: bool) -> Vec<Effect> {
    let real = real(st);
    send(st, Msg::Trust(TrustMsg::Decide { real, trusted }))
}

fn waiting(label: &str) -> Option<Sealed> {
    Some(Sealed::Waiting { kind: WaitingKind::Read, label: Some(label.to_owned()) })
}

/// `msg` starts no file work while git is held, the prompt names `label`, and Trust runs it
/// with the runner's re-check.
fn waits_then_runs(mut st: AppState, msg: Msg, label: &str) -> FilesEffect {
    let fx = send(&mut st, msg);
    assert!(files(fx).is_empty(), "nothing runs before the decision");
    assert_eq!(waits(&st).as_deref(), Some(label), "the prompt says what waits");
    assert!(st.log.iter().any(|l| l.text.contains(label)), "and the log says why it didn't run");
    let mut ran = files(decide(&mut st, true));
    assert_eq!(ran.len(), 1, "Trust runs it");
    let (effect, sealed) = ran.remove(0);
    assert_eq!(sealed, waiting(label), "checked again by the runner");
    effect
}

fn stage_file() -> Msg {
    Msg::Stage(StageMsg::StageFile("a.txt".into()))
}

#[test]
fn staging_waits_for_trust_in_an_undecided_repository_and_runs_on_trust() {
    let ran = waits_then_runs(held("ft-stage"), stage_file(), "Stage");
    assert!(matches!(ran, FilesEffect::LocalOp { op: LocalOp::StageFile(p), .. } if p == "a.txt"));
    let ran = waits_then_runs(held("ft-stage-all"), Msg::Stage(StageMsg::StageAll), "Stage");
    assert!(matches!(ran, FilesEffect::LocalOp { op: LocalOp::StageAll, .. }));
    let mut st = held("ft-stage-paths");
    st.snap.as_mut().unwrap().unstaged = rows(&[("a.txt", 'M'), ("b.txt", '?')]);
    let ran = waits_then_runs(st, Msg::Stage(StageMsg::StagePaths(vec!["a.txt".into(), "b.txt".into()])), "Stage");
    assert!(matches!(ran, FilesEffect::LocalOp { op: LocalOp::StagePaths(p), .. } if p.len() == 2));
}

#[test]
fn staging_waits_in_a_repository_the_user_chose_not_to_trust() {
    let ran = waits_then_runs(refused("ft-stage-refused"), stage_file(), "Stage");
    assert!(matches!(ran, FilesEffect::LocalOp { op: LocalOp::StageFile(_), .. }));
}

#[test]
fn dont_trust_and_cancel_drop_the_waiting_stage() {
    let mut st = held("ft-stage-dropped");
    send(&mut st, stage_file());
    assert!(files(decide(&mut st, false)).is_empty(), "Don't trust runs nothing");
    // Trusting it later starts nothing that waited before.
    let real = real(&st);
    send(&mut st, Msg::Trust(TrustMsg::Ask { real }));
    assert_eq!(waits(&st), None, "the prompt brought back names nothing");
    assert!(files(decide(&mut st, true)).is_empty(), "a later Trust doesn't stage");

    let mut st = held("ft-stage-cancelled");
    send(&mut st, stage_file());
    send(&mut st, Msg::CloseModal);
    assert!(st.trust.waiting.is_none(), "closing the prompt drops it");
}

#[test]
fn staging_in_a_trusted_or_clean_repository_is_sent_with_a_recheck() {
    for st in [trusted("ft-stage-trusted"), clean("ft-stage-clean")] {
        let mut st = st;
        let sent = files(send(&mut st, stage_file()));
        assert!(matches!(&sent[..], [(FilesEffect::LocalOp { op: LocalOp::StageFile(_), .. }, Some(Sealed::Read))]), "{sent:?}");
        assert!(st.modal.is_none());
    }
}

#[test]
fn work_that_starts_no_git_runs_unchecked_while_git_is_held() {
    for name in ["ft-libgit2-held", "ft-libgit2-refused"] {
        let mut st = if name.ends_with("held") { held(name) } else { refused(name) };
        st.snap.as_mut().unwrap().staged = rows(&[("a.txt", 'M')]);
        let sent = files(send(&mut st, Msg::Stage(StageMsg::UnstageFile("a.txt".into()))));
        assert!(matches!(&sent[..], [(FilesEffect::LocalOp { op: LocalOp::UnstagePaths(_), .. }, None)]), "{sent:?}");
        let delete = FileOp::Delete { path: "a.txt".into(), mode: DeleteMode::Trash };
        let sent = files(send(&mut st, Msg::WorkFile(WorkFileMsg::DeleteConfirmed(delete))));
        assert!(matches!(&sent[..], [(FilesEffect::Work { .. }, None)]), "{sent:?}");
        // Only untracked files: nothing goes back through git.
        let only_new = vec![DiscardEach { path: "n.txt".into(), untracked: true }];
        let msg = Msg::WorkFile(WorkFileMsg::DiscardPathsConfirmed { files: only_new, mode: DeleteMode::Trash });
        let sent = files(send(&mut st, msg));
        assert!(matches!(&sent[..], [(FilesEffect::DiscardPaths { .. }, None)]), "{sent:?}");
        assert!(st.modal.is_none(), "no prompt");
    }
}

#[test]
fn discarding_a_tracked_file_waits_for_trust() {
    for (name, make) in [("ft-discard-held", held as fn(&str) -> AppState), ("ft-discard-refused", refused)] {
        let ran = waits_then_runs(make(name), Msg::Stage(StageMsg::DiscardFile("a.txt".into())), "Discard");
        assert!(matches!(ran, FilesEffect::LocalOp { op: LocalOp::DiscardFile(_), .. }));
        let both = vec![DiscardEach { path: "a.txt".into(), untracked: false }, DiscardEach { path: "n.txt".into(), untracked: true }];
        let msg = Msg::WorkFile(WorkFileMsg::DiscardPathsConfirmed { files: both, mode: DeleteMode::Trash });
        let ran = waits_then_runs(make(&format!("{name}-many")), msg, "Discard");
        assert!(matches!(ran, FilesEffect::DiscardPaths { files, .. } if files.len() == 2));
    }
    let mut st = trusted("ft-discard-trusted");
    let sent = files(send(&mut st, Msg::Stage(StageMsg::DiscardFile("a.txt".into()))));
    assert!(matches!(&sent[..], [(FilesEffect::LocalOp { .. }, Some(Sealed::Read))]), "{sent:?}");
}

#[test]
fn restoring_a_file_waits_for_trust() {
    let restore = |st: &AppState| {
        let oid = st.snap.as_ref().unwrap().commits[0].oid;
        Msg::Files(FilesMsg::RestoreFile { oid, path: "a.txt".into(), deleted: false, confirmed: true, overwrite: false })
    };
    for (name, make) in [("ft-restore-held", held as fn(&str) -> AppState), ("ft-restore-refused", refused)] {
        let st = make(name);
        let msg = restore(&st);
        assert!(matches!(waits_then_runs(st, msg, "Restore file"), FilesEffect::Restore { .. }));
    }
    let mut st = trusted("ft-restore-trusted");
    let msg = restore(&st);
    let sent = files(send(&mut st, msg));
    assert!(matches!(&sent[..], [(FilesEffect::Restore { .. }, Some(Sealed::Read))]), "{sent:?}");
}

/// `st` with a.txt conflicted.
fn conflicted(mut st: AppState) -> AppState {
    st.snap.as_mut().unwrap().unstaged = rows(&[("a.txt", '!')]);
    st
}

#[test]
fn marking_a_conflict_resolved_waits_for_trust_and_marks_nothing_in_flight() {
    let resolve = || Msg::Conflict(ConflictMsg::MarkResolvedConfirmed(vec!["a.txt".into()]));
    for (name, make) in [("ft-conflict-held", held as fn(&str) -> AppState), ("ft-conflict-refused", refused)] {
        let ran = waits_then_runs(conflicted(make(name)), resolve(), "Resolve conflict");
        assert!(matches!(ran, FilesEffect::LocalOp { op: LocalOp::MarkResolved(_), .. }));
    }
    let mut st = conflicted(trusted("ft-conflict-trusted"));
    let sent = files(send(&mut st, resolve()));
    assert!(matches!(&sent[..], [(FilesEffect::LocalOp { op: LocalOp::MarkResolved(_), .. }, Some(Sealed::Read))]), "{sent:?}");
}

#[test]
fn a_staged_rename_waits_for_trust_and_a_plain_one_does_not() {
    let rename = |stage| Msg::WorkFile(WorkFileMsg::RenameFile { from: "a.txt".into(), to: "b.txt".into(), stage });
    let tracked = |mut st: AppState| {
        st.snap.as_mut().unwrap().unstaged = rows(&[("a.txt", 'M')]);
        st
    };
    let ran = waits_then_runs(tracked(held("ft-rename-held")), rename(true), "Rename");
    assert!(matches!(ran, FilesEffect::Work { op: FileOp::Rename { stage: true, .. }, .. }));
    let mut st = tracked(refused("ft-rename-plain"));
    let sent = files(send(&mut st, rename(false)));
    assert!(matches!(&sent[..], [(FilesEffect::Work { op: FileOp::Rename { stage: false, .. }, .. }, None)]), "{sent:?}");
    // From its dialog, which no prompt opens over: the dialog says why and takes another try.
    let mut st = tracked(refused("ft-rename-dialog"));
    st.modal = Some(Modal::RenameFile { old: "a.txt".into(), form: PathForm::new("a.txt") });
    assert!(files(send(&mut st, rename(true))).is_empty());
    let Some(Modal::RenameFile { form, .. }) = &st.modal else { panic!("the dialog stays open") };
    assert!(!form.busy, "not waiting for a result");
    assert!(form.error.as_ref().is_some_and(|(_, why)| why.contains("Rename")), "{:?}", form.error);
    assert!(st.trust.waiting.is_none());
}

#[test]
fn every_file_operation_says_whether_it_starts_git() {
    use glowfly_core::repo::{ConflictIds, Side};
    let ids = || ConflictIds { ancestor: None, ours: None, theirs: None };
    let p = || String::from("a.txt");
    let git = [
        LocalOp::StageFile(p()),
        LocalOp::StagePaths(vec![p()]),
        LocalOp::StageAll,
        LocalOp::DiscardFile(p()),
        LocalOp::ResolveConflict { path: p(), contents: String::new(), expected: ids() },
        LocalOp::SavePartialResolution { path: p(), contents: String::new(), expected: ids() },
        LocalOp::ResolveWithSide { path: p(), side: Side::Ours, expected: ids() },
        LocalOp::MarkResolved(vec![p()]),
        LocalOp::MarkConflicted(vec![p()]),
    ];
    assert!(git.iter().all(LocalOp::runs_git));
    assert!(!LocalOp::UnstagePaths(vec![p()]).runs_git());
    assert!(FileOp::Rename { from: p(), to: p(), stage: true }.runs_git());
    let plain = [
        FileOp::Rename { from: p(), to: p(), stage: false },
        FileOp::Create(p()),
        FileOp::Delete { path: p(), mode: DeleteMode::Trash },
        FileOp::UnstageDelete { path: p(), mode: DeleteMode::Trash },
        FileOp::UnstageGone(p()),
        FileOp::Discard { path: p(), mode: DeleteMode::Trash, seen: None },
    ];
    assert!(!plain.iter().any(FileOp::runs_git));
}

#[test]
fn a_fetch_update_starts_by_itself_is_checked_again_by_the_runner() {
    let mut st = clean("ft-fetches");
    let workdir = st.workdir().unwrap();
    let cmd = vec!["fetch".to_owned()];
    let head = HeadJob { workdir: workdir.clone(), id: 7, head: "abc".into(), cmd: cmd.clone(), prompts: false };
    let content = ContentRequest { workdir, id: 1, cmds: vec![cmd] };
    let mut fx = vec![Effect::PullRequest(PrEffect::FetchHead(head)), Effect::Partial(PartialEffect::FetchContent(content))];
    super::trust_seal::wrap(&mut st, &mut fx, 0);
    for e in &fx {
        assert_eq!(e.trust_check().map(|c| c.sealed.clone()), Some(Sealed::Read), "{e:?}");
    }
    // `update` hands the plain effects to callers that don't run the check.
    let plain = update(&mut st, stage_file(), Instant::now());
    assert!(plain.iter().all(|e| e.trust_check().is_none()));
}
