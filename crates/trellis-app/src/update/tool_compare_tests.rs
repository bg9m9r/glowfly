//! Headless tests for the external diff and merge tools: opening Preferences when no program
//! is set, refusals of unusable arguments,
//! the requests planned for each diff target (with git's filters only in a trusted
//! repository), the merge tool started as the single active job without the trust prompt, and
//! how its end is reported (and that the conflict view writes nothing over the tool's work).

use super::conflict_tests::conflicted_repo;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{ConflictMsg, Msg, RepoMsg, StageMsg};
use crate::state::AppState;
use crate::state::PrefPage;
use crate::tools::{DiffToolRequest, LaunchFailure, MergeEnd, MergeToolRequest, ToolEffect, ToolsMsg};
use git2::Oid;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::{DiffTarget, Side};
use trellis_core::settings::ExternalTools;
use trellis_core::undo::Action;

fn run(st: &mut AppState, m: ToolsMsg) -> Vec<Effect> {
    update(st, Msg::Tools(m), Instant::now())
}

fn opened(dir: &Path) -> AppState {
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    assert!(st.snap.is_some());
    st
}

/// The only effect, which must open the diff tool.
fn diff_request(fx: Vec<Effect>) -> DiffToolRequest {
    match <[Effect; 1]>::try_from(fx) {
        Ok([Effect::Tool(ToolEffect::OpenDiff(r))]) => r,
        other => panic!("expected one OpenDiff, got {other:?}"),
    }
}

/// The only effect, which must run the merge tool.
fn merge_request(fx: Vec<Effect>) -> MergeToolRequest {
    match <[Effect; 1]>::try_from(fx) {
        Ok([Effect::Tool(ToolEffect::RunMerge(r))]) => r,
        other => panic!("expected one RunMerge, got {other:?}"),
    }
}

/// The last toast: its success, text and action label.
fn toast(st: &AppState) -> (bool, String, Option<String>) {
    let t = st.toasts.last().expect("a toast");
    (t.ok, t.text.clone(), t.action.as_ref().map(|a| a.label.clone()))
}

/// Makes `dir` look untrusted: a repository-configured ssh command is something git would run.
fn make_untrusted(dir: &Path) {
    git(dir, &["config", "core.sshCommand", "ssh -v"]);
}

/// Whether `fx` holds any tool effect (a launch, a diff or a merge).
fn any_tool(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Tool(_)))
}

#[test]
fn opening_a_diff_without_a_tool_opens_preferences() {
    let dir = scratch_repo("tc-no-diff-tool");
    let mut st = opened(&dir);
    let (toasts, last_log) = (st.toasts.len(), st.log.last().map(|l| l.text.clone()));
    let fx = run(&mut st, ToolsMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into())));
    assert!(!any_tool(&fx), "{fx:?}");
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Tools));
    assert_eq!(st.toasts.len(), toasts, "no failure toast");
    assert_eq!(st.log.last().map(|l| l.text.clone()), last_log, "no failure in the log");
    st.prefs = None;

    st.settings.external_tools.diff_tool = "meld".into();
    st.settings.external_tools.diff_tool_args = "\"{local} {remote}".into();
    assert!(run(&mut st, ToolsMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into()))).is_empty());
    let (ok, text, action) = toast(&st);
    assert!(!ok && text.contains("Diff tool arguments") && action.as_deref() == Some("Details"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_a_diff_sends_the_target_with_filters_when_trusted() {
    let dir = scratch_repo("tc-open-diff");
    let mut st = opened(&dir);
    st.settings.external_tools.diff_tool = "meld".into();
    let w = st.workdir().unwrap();
    let head = st.snap.as_ref().unwrap().head_oid.expect("a commit");
    for target in [DiffTarget::Unstaged("a.txt".into()), DiffTarget::Staged("a.txt".into()), DiffTarget::Commit(head, "a.txt".into())] {
        let r = diff_request(run(&mut st, ToolsMsg::OpenDiff(target.clone())));
        let want = DiffToolRequest {
            program: PathBuf::from("meld"),
            template: ExternalTools::DIFF_TOOL_ARGS.into(),
            workdir: w.clone(),
            target,
            filters: true,
        };
        assert_eq!(r, want);
    }
    let _ = std::fs::remove_dir_all(&dir);

    // An untrusted repository's copies are made without its filters.
    let dir = scratch_repo("tc-open-diff-untrusted");
    make_untrusted(&dir);
    let mut st = opened(&dir);
    assert!(st.held());
    st.settings.external_tools.diff_tool = "meld".into();
    assert!(!diff_request(run(&mut st, ToolsMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into())))).filters);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resolving_starts_the_merge_tool_as_the_active_job() {
    let dir = conflicted_repo("tc-resolve");
    let mut st = opened(&dir);
    // No tool set: nothing starts, and Preferences opens on External Tools.
    let toasts = st.toasts.len();
    let fx = run(&mut st, ToolsMsg::ResolveInMergeTool("a.txt".into()));
    assert!(!any_tool(&fx), "{fx:?}");
    assert!(st.job.is_none());
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Tools));
    assert_eq!(st.toasts.len(), toasts, "no toast");
    st.prefs = None;

    st.settings.external_tools.merge_tool = "kdiff3".into();
    assert!(run(&mut st, ToolsMsg::ResolveInMergeTool("notes.txt".into())).is_empty());
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some("notes.txt is not conflicted"));

    let r = merge_request(run(&mut st, ToolsMsg::ResolveInMergeTool("a.txt".into())));
    assert_eq!((r.program, r.path.as_str(), r.filters), (PathBuf::from("kdiff3"), "a.txt", true));
    assert_eq!(r.workdir, st.workdir().unwrap());
    assert_eq!(r.template, ExternalTools::MERGE_TOOL_ARGS);
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Resolve a.txt in the merge tool"));
    // One job at a time.
    assert!(run(&mut st, ToolsMsg::ResolveInMergeTool("a.txt".into())).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resolving_a_clean_file_without_a_tool_does_not_open_preferences() {
    let dir = conflicted_repo("tc-resolve-clean");
    let mut st = opened(&dir);
    assert!(run(&mut st, ToolsMsg::ResolveInMergeTool("notes.txt".into())).is_empty());
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some("notes.txt is not conflicted"));
    assert!(st.prefs.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_trust_prompt_does_not_hold_the_merge_tool() {
    let dir = conflicted_repo("tc-resolve-untrusted");
    make_untrusted(&dir);
    let mut st = opened(&dir);
    assert!(st.held());
    st.settings.external_tools.merge_tool = "kdiff3".into();
    let r = merge_request(run(&mut st, ToolsMsg::ResolveInMergeTool("a.txt".into())));
    assert!(!r.filters, "the stored bytes, without the repository's filters");
    assert!(st.job.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

/// `conflicted_repo` with the merge tool running on a.txt as the active job.
pub(super) fn running(name: &str) -> (AppState, PathBuf) {
    let dir = conflicted_repo(name);
    let mut st = opened(&dir);
    st.settings.external_tools.merge_tool = "kdiff3".into();
    merge_request(run(&mut st, ToolsMsg::ResolveInMergeTool("a.txt".into())));
    (st, dir)
}

fn exited(st: &mut AppState, end: MergeEnd) -> Vec<Effect> {
    let workdir = st.workdir().unwrap();
    run(st, ToolsMsg::MergeToolExited { workdir, path: "a.txt".into(), program: "kdiff3".into(), end })
}

#[test]
fn a_saved_merge_ends_the_job_with_mark_resolved() {
    let (mut st, dir) = running("tc-saved");
    let fx = exited(&mut st, MergeEnd::Saved);
    assert!(st.job.is_none(), "the job ended");
    assert!(!fx.is_empty(), "status is read again");
    let (ok, text, action) = toast(&st);
    assert!(ok && action.as_deref() == Some("Mark resolved"), "{text}");
    assert_eq!(text, "The merge tool saved a.txt. Mark it resolved to stage the result.");
    let id = st.toasts.last().unwrap().id;
    let fx = update(&mut st, Msg::ToastAction(id), Instant::now());
    assert!(checks_markers(&fx), "checks the file for markers, then stages it: {fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn other_merge_ends_are_reported() {
    let (mut st, dir) = running("tc-ends");
    let cases = [
        (MergeEnd::Unchanged, false, Some("Details")),
        (MergeEnd::Exited { code: Some(1), said: "bad".into(), changed: false }, false, Some("Details")),
        (MergeEnd::Failed(LaunchFailure::NotFound), false, Some("Open preferences")),
        (MergeEnd::Resolved, true, None),
    ];
    for (end, want_ok, want_action) in cases {
        if st.job.is_none() {
            merge_request(run(&mut st, ToolsMsg::ResolveInMergeTool("a.txt".into())));
        }
        exited(&mut st, end.clone());
        assert!(st.job.is_none(), "{end:?}");
        let (ok, text, action) = toast(&st);
        assert_eq!((ok, action.as_deref()), (want_ok, want_action), "{end:?}: {text}");
        assert!(text.contains("a.txt") || text.contains("kdiff3"), "{text}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_merge_tool_run_keeps_the_undo_history() {
    let (mut st, dir) = running("tc-undo");
    let deleted = Action::DeleteBranch { name: "b".into(), oid: Oid::zero(), upstream: None };
    for end in [MergeEnd::Saved, MergeEnd::Resolved] {
        if st.job.is_none() {
            merge_request(run(&mut st, ToolsMsg::ResolveInMergeTool("a.txt".into())));
        }
        assert!(st.job.as_ref().is_some_and(|j| j.keeps_undo));
        st.undo.clear();
        st.undo.record(deleted.clone());
        exited(&mut st, end.clone());
        assert!(st.job.is_none() && toast(&st).0, "{end:?}");
        assert_eq!(st.undo.undo, std::slice::from_ref(&deleted), "{end:?}: the merge tool moves no refs");
        assert!(st.undo.blocked.is_none(), "{end:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// `running` with the conflict view loaded on a.txt and a side picked for every chunk.
fn running_in_view(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = running(name);
    drive(&mut st, Msg::Conflict(ConflictMsg::OpenConflict("a.txt".into())));
    assert!(st.conflict.as_ref().is_some_and(|c| c.file.is_some()), "the view loaded");
    update(&mut st, Msg::Conflict(ConflictMsg::ConflictTakeAll(Side::Ours)), Instant::now());
    assert!(st.conflict.as_ref().unwrap().can_save());
    (st, dir)
}

fn writes(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::LocalOp { .. }))
}

/// Mark resolved's first step: the working file is read for conflict markers.
fn checks_markers(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::ReadConflictFiles { purpose: crate::effect::ConflictRead::BeforeMarkResolved, .. }))
}

#[test]
fn the_conflict_view_writes_nothing_while_the_merge_tool_has_the_file() {
    let (mut st, dir) = running_in_view("tc-view-held");
    let now = Instant::now();
    for m in [
        Msg::Conflict(ConflictMsg::SaveConflict),
        Msg::Conflict(ConflictMsg::SaveConflictKeepMarkers),
        Msg::Conflict(ConflictMsg::ResolveWithSide { path: "a.txt".into(), side: Side::Theirs }),
        Msg::Stage(StageMsg::StageFile("a.txt".into())),
    ] {
        let fx = update(&mut st, m, now);
        assert!(!writes(&fx), "nothing is written or staged while the tool runs: {fx:?}");
        assert_eq!(st.log.last().map(|l| l.text.as_str()), Some("a.txt is open in the merge tool: save and close it there first."));
    }
    assert!(!st.conflict.as_ref().unwrap().saving);
    // Other files stage as usual.
    assert!(writes(&update(&mut st, Msg::Stage(StageMsg::StageFile("b.txt".into())), now)));

    // The tool left the file alone: the view and its picks stay, and Save works again.
    exited(&mut st, MergeEnd::Unchanged);
    assert!(st.conflict.as_ref().is_some_and(|c| c.can_save()), "the picks are kept");
    assert!(writes(&update(&mut st, Msg::Conflict(ConflictMsg::SaveConflict), now)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_merge_tool_that_changed_the_file_closes_the_conflict_view() {
    for (i, end) in [MergeEnd::Saved, MergeEnd::Exited { code: Some(1), said: String::new(), changed: true }].into_iter().enumerate() {
        let (mut st, dir) = running_in_view(&format!("tc-view-closes-{i}"));
        exited(&mut st, end.clone());
        assert!(st.conflict.is_none(), "{end:?}: its picks would overwrite the tool's result");
        // Mark resolved stages the tool's result once it has exited (after the markers check).
        assert!(checks_markers(&update(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())), Instant::now())), "{end:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_merge_for_a_tab_no_longer_active_still_ends_the_job() {
    let (mut st, dir) = running("tc-elsewhere");
    st.job.as_mut().unwrap().workdir = Some(PathBuf::from("C:/somewhere/else"));
    let fx = run(
        &mut st,
        ToolsMsg::MergeToolExited {
            workdir: "C:/somewhere/else".into(),
            path: "a.txt".into(),
            program: "kdiff3".into(),
            end: MergeEnd::Saved,
        },
    );
    assert!(st.job.is_none());
    assert!(
        !fx.iter().any(|e| matches!(e, Effect::LoadStatus { .. } | Effect::LoadRepo { .. })),
        "the active tab is not refreshed: {fx:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_new_message_is_unscoped() {
    for m in [
        ToolsMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into())),
        ToolsMsg::ResolveInMergeTool("a.txt".into()),
        ToolsMsg::MergeToolExited { workdir: "C:/r".into(), path: "a.txt".into(), program: "m".into(), end: MergeEnd::Saved },
    ] {
        assert!(m.on_inactive().is_none());
    }
}
