//! Previewing an operation: opening asks the runner, results fill only the dialog that asked,
//! a merge on an old git says why at once, Apply sends what the menu item sends (and is refused
//! while busy or stale), Preview again asks anew, and a drop end to end.

use super::rebase_quick_tests::{last_toast, up};
use super::rebase_tests::{repo, running_job};
use super::selection_tests::{commit_on, git};
use super::tests::drive;
use crate::effect::{Effect, OpPreviewJob, RebaseEffect};
use crate::msg::{Msg, OpPreviewMsg, RepoMsg, ResetMode};
use crate::runner;
use crate::state::op_preview::PreviewDialog;
use crate::state::{AppState, Modal};
use git2::Oid;
use std::path::Path;
use trellis_core::git_version::GitInfo;
use trellis_core::gitcli::args;
use trellis_core::op_preview::{GoneWhy, PreviewOp};
use trellis_core::rebase::QuickEdit;

fn open(st: &mut AppState, op: PreviewOp) -> Vec<Effect> {
    up(st, Msg::OpPreview(OpPreviewMsg::Open(op)))
}

fn dialog(st: &AppState) -> &PreviewDialog {
    match &st.modal {
        Some(Modal::OpPreview(d)) => d,
        _ => panic!("expected the preview"),
    }
}

fn job(fx: &[Effect]) -> &OpPreviewJob {
    match fx {
        [Effect::OpPreview(j)] => j,
        other => panic!("expected one preview, got {other:?}"),
    }
}

/// `repo`'s main with a `feature` branch one commit past c1.
fn with_feature(name: &str) -> (AppState, std::path::PathBuf, [Oid; 3], Oid) {
    let (mut st, dir, c) = repo(name);
    git(&dir, &["checkout", "-q", "-b", "feature", &c[0].to_string()]);
    std::fs::write(dir.join("f.txt"), "f\n").unwrap();
    let f = commit_on(&dir, "f", 9);
    git(&dir, &["checkout", "-q", "main"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, dir, c, f)
}

fn merge(f: Oid) -> PreviewOp {
    PreviewOp::Merge { theirs: "feature".into(), oid: f }
}

/// Runs a held preview and hands its result to `update`.
fn deliver(st: &mut AppState, fx: Vec<Effect>) -> Vec<Effect> {
    let effect = fx.into_iter().next().expect("a preview");
    let msg = runner::execute(effect).expect("a result");
    up(st, msg)
}

fn git_cmds(fx: &[Effect]) -> Vec<Vec<String>> {
    fx.iter()
        .find_map(|e| match e {
            Effect::RunGit { cmds, .. } => Some(cmds.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected a git job, got {fx:?}"))
}

#[test]
fn opening_asks_and_results_fill_only_the_dialog_that_asked() {
    let (mut st, dir, _, f) = with_feature("opp-open");
    let fx = open(&mut st, merge(f));
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    assert_eq!(job(&fx).request.head, head);
    assert!(dialog(&st).result.is_none(), "loading");
    assert!(fx[0].scope().is_some_and(|w| *w == st.snap.as_ref().unwrap().workdir), "its result is for this tab");

    // A result for another request changes nothing.
    let mut other = job(&fx).clone();
    other.request.op = PreviewOp::Reset { to: f };
    let stray = Msg::OpPreview(OpPreviewMsg::Loaded { job: other, result: Err("no".into()) });
    assert!(up(&mut st, stray).is_empty());
    assert!(dialog(&st).result.is_none());

    assert!(deliver(&mut st, fx).is_empty());
    let p = dialog(&st).result.clone().expect("loaded").expect("worked out");
    assert_eq!(p.subject.as_deref(), Some("main"));

    // After Cancel a late result opens nothing.
    let fx = open(&mut st, merge(f));
    st.modal = None;
    deliver(&mut st, fx);
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_merge_on_an_old_git_says_why_at_once() {
    let (mut st, dir, c, f) = with_feature("opp-old-git");
    st.git.info = Some(GitInfo { path: Some("git".into()), version: Some((2, 37, 1)), chosen: false });
    assert!(open(&mut st, merge(f)).is_empty(), "no effect");
    let Some(Err(why)) = &dialog(&st).result else { panic!("expected the version error") };
    assert!(why.contains("needs git 2.38"), "{why}");
    let fx = open(&mut st, PreviewOp::Rebase { onto: "feature".into(), oid: f });
    job(&fx);
    assert_eq!(job(&open(&mut st, PreviewOp::Reset { to: c[0] })).request.op, PreviewOp::Reset { to: c[0] });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apply_sends_what_the_menu_item_sends() {
    let (mut st, dir, c, f) = with_feature("opp-apply");
    deliver_open(&mut st, merge(f));
    let fx = up(&mut st, Msg::OpPreview(OpPreviewMsg::Apply(None)));
    assert_eq!(git_cmds(&fx), [args(&["merge", "feature"])]);
    assert!(!matches!(st.modal, Some(Modal::OpPreview(_))), "the dialog closed");
    st.job = None;

    deliver_open(&mut st, PreviewOp::Drop(vec![c[1]]));
    let fx = up(&mut st, Msg::OpPreview(OpPreviewMsg::Apply(None)));
    match &fx[..] {
        [Effect::Rebase(RebaseEffect::PrepareEdit { edit, .. })] => assert_eq!(*edit, QuickEdit::Drop(vec![c[1]])),
        other => panic!("expected the drop's preparation, got {other:?}"),
    }

    deliver_open(&mut st, PreviewOp::Reset { to: c[0] });
    let fx = up(&mut st, Msg::OpPreview(OpPreviewMsg::Apply(Some(ResetMode::Soft))));
    assert_eq!(git_cmds(&fx), [args(&["reset", "--soft", &c[0].to_string()])]);
    st.job = None;
    deliver_open(&mut st, PreviewOp::Reset { to: c[0] });
    assert!(up(&mut st, Msg::OpPreview(OpPreviewMsg::Apply(Some(ResetMode::Hard)))).is_empty());
    assert!(matches!(st.modal, Some(Modal::ConfirmDanger { .. })), "a hard reset still asks");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Opens the preview of `op` and loads its result.
fn deliver_open(st: &mut AppState, op: PreviewOp) {
    let fx = open(st, op);
    deliver(st, fx);
    assert!(matches!(dialog(st).result, Some(Ok(_))), "{:?}", dialog(st).result);
}

#[test]
fn apply_is_refused_while_busy_or_after_head_moved_and_again_asks_anew() {
    let (mut st, dir, c, _) = with_feature("opp-refused");
    deliver_open(&mut st, PreviewOp::Reset { to: c[0] });
    st.job = Some(running_job(&dir));
    assert!(up(&mut st, Msg::OpPreview(OpPreviewMsg::Apply(Some(ResetMode::Mixed)))).is_empty());
    assert_eq!(last_toast(&st), (false, "Another operation is still running."));
    assert!(matches!(st.modal, Some(Modal::OpPreview(_))), "the dialog stays");
    st.job = None;

    std::fs::write(dir.join("n.txt"), "n\n").unwrap();
    let moved = commit_on(&dir, "new", 20);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(up(&mut st, Msg::OpPreview(OpPreviewMsg::Apply(Some(ResetMode::Mixed)))).is_empty());
    assert_eq!(last_toast(&st), (false, "The branch changed since this preview. Preview again first."));
    assert!(matches!(st.modal, Some(Modal::OpPreview(_))));

    let fx = up(&mut st, Msg::OpPreview(OpPreviewMsg::Again));
    assert_eq!(job(&fx).request.head, moved);
    assert!(dialog(&st).result.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_drop_end_to_end() {
    let (mut st, dir, c) = repo("opp-drop");
    let fx = open(&mut st, PreviewOp::Drop(vec![c[1]]));
    deliver(&mut st, fx);
    let p = dialog(&st).result.clone().unwrap().unwrap();
    assert_eq!(p.gone.iter().map(|g| (g.line.oid, g.why)).collect::<Vec<_>>(), [(c[1], GoneWhy::Dropped)]);
    assert_eq!(p.moved.iter().map(|l| l.oid).collect::<Vec<_>>(), [c[2]]);
    assert_eq!(head_of(&dir), c[2], "nothing changed on disk");
    let _ = std::fs::remove_dir_all(&dir);
}

fn head_of(dir: &Path) -> Oid {
    super::selection_tests::head(dir)
}

#[test]
fn opening_needs_a_commit() {
    let mut st = super::tests::state();
    assert!(open(&mut st, PreviewOp::Drop(Vec::new())).is_empty());
    assert!(st.modal.is_none());
}
