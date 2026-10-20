//! The rebase banner's Continue and Skip: a rebase that stops again is reported with the step,
//! never as "succeeded", and Continue waits for conflicted files.

use super::rebase_tests::{repo, with_panel};
use super::tests::drive;
use super::update;
use crate::effect::{Effect, RebaseEffect};
use crate::msg::{Msg, RebaseMsg};
use crate::state::rebase_banner::action_msg;
use crate::state::{AppState, Selection};
use git2::{Oid, RepositoryState};
use std::time::Instant;
use trellis_core::i18n::Lang;
use trellis_core::rebase::{self, RebaseAction, RebaseBase, Resume};
use trellis_core::repo::FileEntry;

fn short(o: Oid) -> String {
    o.to_string()[..7].to_owned()
}

fn repo_state(st: &AppState) -> RepositoryState {
    st.snap.as_ref().unwrap().state
}

fn last_toast(st: &AppState) -> (bool, &str) {
    st.toasts.last().map(|t| (t.ok, t.text.as_str())).unwrap_or((false, ""))
}

/// The banner's button for `label`, as the WIP panel sends it.
fn banner(st: &AppState, label: &str) -> Msg {
    let actions = crate::ui::common::state_actions(repo_state(st)).expect("an operation in progress");
    let (l, a) = actions.iter().find(|(l, _)| *l == label).expect("the action is offered");
    action_msg(repo_state(st), l, a)
}

#[test]
fn continue_reports_the_next_stop() {
    let (mut st, dir, c) = with_panel("resume-edits", RebaseBase::FromCommit(Oid::zero()));
    drive(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 0, action: RebaseAction::Edit }));
    drive(&mut st, Msg::Rebase(RebaseMsg::RebaseSetAction { index: 1, action: RebaseAction::Edit }));
    drive(&mut st, Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false }));
    let text = format!("Interactive rebase paused at {} (step 1 of 3) so you can edit it. Use Continue when you're done.", short(c[0]));
    assert_eq!(last_toast(&st), (true, text.as_str()));
    let p = st.snap.as_ref().unwrap().rebase.clone().expect("the snapshot knows where the rebase is");
    assert_eq!((p.step, p.total, p.stop.oid), (1, 3, Some(c[0])));

    st.selection = Selection::None;
    let msg = banner(&st, "Continue");
    assert!(matches!(msg, Msg::Rebase(RebaseMsg::Resume(Resume::Continue))));
    drive(&mut st, msg);
    let text = format!("Interactive rebase paused at {} (step 2 of 3) so you can edit it. Use Continue when you're done.", short(c[1]));
    assert_eq!(last_toast(&st), (true, text.as_str()));
    assert_eq!(repo_state(&st), RepositoryState::RebaseInteractive);
    assert_eq!(st.selection, Selection::Wip, "the banner is in view");

    let msg = banner(&st, "Continue");
    drive(&mut st, msg);
    assert_eq!(last_toast(&st), (true, "Interactive rebase succeeded"));
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert!(st.snap.as_ref().unwrap().rebase.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn continue_waits_for_conflicts_and_skip_resumes() {
    let (mut st, dir, _) = repo("resume-refused");
    let up = |st: &mut AppState, how| update(st, Msg::Rebase(RebaseMsg::Resume(how)), Instant::now());
    assert!(up(&mut st, Resume::Continue).is_empty());
    assert_eq!(last_toast(&st), (false, rebase::NOT_REBASING));

    let snap = st.snap.as_mut().unwrap();
    snap.state = RepositoryState::RebaseInteractive;
    snap.unstaged.push(FileEntry::new("c1.txt", '!'));
    assert!(up(&mut st, Resume::Continue).is_empty());
    assert_eq!(last_toast(&st), (false, Lang::En.tr("details.banner.resolve_first.tip")));

    // The refusal follows the language setting, like the banner's tooltip.
    let was = std::mem::replace(&mut st.settings.language, "de".into());
    assert!(up(&mut st, Resume::Continue).is_empty());
    assert_eq!(last_toast(&st), (false, Lang::De.tr("details.banner.resolve_first.tip")));
    st.settings.language = was;

    let fx = up(&mut st, Resume::Skip);
    assert!(matches!(&fx[..], [Effect::Rebase(RebaseEffect::Resume { how: Resume::Skip, .. })]), "{fx:?}");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Interactive rebase"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn headlines_for_commands_and_breaks() {
    use super::rebase_resume::stopped_headline;
    use trellis_core::rebase::{RebaseProgress, Stop, StopKind};
    let oid = Oid::from_bytes(&[0x1a; 20]).unwrap();
    let p = |kind| RebaseProgress {
        step: 2,
        total: 4,
        stop: Stop { kind, oid: Some(oid) },
        summary: "Fix it".into(),
        autostash: false,
        interactive: true,
        command: Some("false".into()),
    };
    let label = "Interactive rebase";
    assert_eq!(
        stopped_headline(label, &p(StopKind::Command), false, ""),
        "Interactive rebase stopped: the command after 1a1a1a1 (step 2 of 4) failed."
    );
    assert_eq!(
        stopped_headline(label, &p(StopKind::Break), true, ""),
        "Interactive rebase paused at a break step after 1a1a1a1 (step 2 of 4). Use Continue to go on."
    );
    assert_eq!(
        stopped_headline(label, &p(StopKind::Exec), false, ""),
        "Interactive rebase stopped: rewording 1a1a1a1 (step 2 of 4) failed."
    );
}
