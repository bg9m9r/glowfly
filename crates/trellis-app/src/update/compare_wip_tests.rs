//! Headless tests for comparing a commit with the working directory: entering and leaving it,
//! stale reads, and end to end (the list, a diff, re-reads after status reads, and the commit
//! going away).

use super::selection_tests::{fixture, git, up};
use super::tests::drive;
use crate::effect::Effect;
use crate::msg::{Msg, RepoMsg, SelectionMsg};
use crate::state::history::FileHistoryView;
use crate::state::selection_wip::{WORKDIR_NEEDS_ONE, WorkdirCompare};
use crate::state::{AppState, Selection};
use git2::Oid;
use trellis_core::repo::{Comparison, DiffTarget, FileDiff, FileEntry};

fn compared(st: &AppState) -> &WorkdirCompare {
    match &st.selection {
        Selection::VsWorkdir(w) => w,
        other => panic!("expected a working-directory comparison, got {other:?}"),
    }
}

fn reads(fx: &[Effect]) -> Vec<(Oid, u64)> {
    fx.iter()
        .filter_map(|e| if let Effect::LoadWorkdirCompare { oid, generation, .. } = e { Some((*oid, *generation)) } else { None })
        .collect()
}

fn empty_diff() -> FileDiff {
    FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None }
}

#[test]
fn entering_and_leaving_the_comparison() {
    let (mut st, f) = fixture("wip-compare-toggle");
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(3))));
    let fx = up(&mut st, Msg::Selection(SelectionMsg::ToggleWip));
    assert_eq!(compared(&st).oid, f.c(3));
    assert_eq!(reads(&fx), [(f.c(3), 1)]);
    assert_eq!(fx.len(), 1, "{fx:?}");
    assert!(st.selection.wip_shown() && st.selection.includes(f.c(3)));

    let fx = up(&mut st, Msg::Selection(SelectionMsg::CollapseSelection));
    assert_eq!(st.selection, Selection::Commit(f.c(3)), "Esc returns to the commit");
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadDetail { oid, .. } if *oid == f.c(3))));

    // From the commit menu, closing what showed the commit before.
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    st.diff = Some((DiffTarget::Commit(f.c(5), "f.txt".into()), empty_diff().into()));
    st.history = Some(FileHistoryView::loading("f.txt".into(), None));
    let fx = up(&mut st, Msg::Selection(SelectionMsg::CompareWithWorkdir(f.c(3))));
    assert_eq!(compared(&st).oid, f.c(3));
    assert!(st.diff.is_none() && st.history.is_none() && st.blame.is_none() && st.detail.is_none());
    assert_eq!(reads(&fx), [(f.c(3), 1)]);

    // Ctrl-click on the WIP row again drops the commit; on a commit adds it back.
    up(&mut st, Msg::Selection(SelectionMsg::ToggleWip));
    assert_eq!(st.selection, Selection::Commit(f.c(3)));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleWip));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(3))));
    assert_eq!(st.selection, Selection::Wip, "Ctrl-click on the compared commit leaves the WIP row");

    // Several commits can't be compared with it.
    up(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    up(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    let before = st.selection.clone();
    assert!(up(&mut st, Msg::Selection(SelectionMsg::ToggleWip)).is_empty());
    assert_eq!(st.selection, before);
    assert_eq!(st.log.last().map(|l| l.text.as_str()), Some(WORKDIR_NEEDS_ONE));
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn only_the_latest_read_is_kept() {
    let (mut st, f) = fixture("wip-compare-stale");
    up(&mut st, Msg::Selection(SelectionMsg::CompareWithWorkdir(f.c(3))));
    up(&mut st, Msg::Repo(RepoMsg::WindowFocused));
    let c = Comparison { files: vec![FileEntry::new("f.txt", 'M')], absent: Default::default() };
    let generation = compared(&st).generation;
    let stale = [(f.c(3), generation.wrapping_sub(1)), (f.c(4), generation)];
    for (oid, generation) in stale {
        up(&mut st, Msg::Selection(SelectionMsg::WorkdirCompareLoaded { oid, generation, result: Ok(c.clone()) }));
        assert_eq!(compared(&st).changes, None, "{oid} {generation}");
    }
    up(&mut st, Msg::Selection(SelectionMsg::WorkdirCompareLoaded { oid: f.c(3), generation, result: Ok(c.clone()) }));
    assert_eq!(compared(&st).changes, Some(Ok(c)));
    up(&mut st, Msg::Selection(SelectionMsg::WorkdirCompareLoaded { oid: f.c(3), generation, result: Err("bad tree".into()) }));
    assert_eq!(compared(&st).changes, Some(Err("bad tree".into())));
    assert!(st.log.iter().any(|l| !l.ok && l.text == "bad tree"));
    let _ = std::fs::remove_dir_all(&f.dir);
}

fn listed(st: &AppState) -> Vec<(String, char)> {
    let c = compared(st).changes.clone().expect("loaded").expect("no error");
    c.files.iter().map(|f| (f.path.clone(), f.status)).collect()
}

#[test]
fn the_comparison_follows_the_working_tree() {
    let (mut st, f) = fixture("wip-compare-e2e");
    std::fs::write(f.dir.join("f.txt"), "edited\n").unwrap();
    std::fs::write(f.dir.join("new.txt"), "new\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Selection(SelectionMsg::CompareWithWorkdir(f.c(3))));
    assert_eq!(listed(&st), [("f.txt".to_owned(), 'M'), ("new.txt".to_owned(), 'A')], "s.txt is only on side");

    let target = DiffTarget::ToWorkdir(f.c(3), "f.txt".into());
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(target.clone())));
    let (shown, diff) = st.diff.clone().expect("the diff opens");
    assert_eq!(shown, target);
    assert!(diff.hunks.iter().flat_map(|h| &h.lines).any(|l| l.origin == '+' && l.text == "edited"));

    // A status read (the window gaining focus) reads the list again, and the open diff.
    std::fs::write(f.dir.join("more.txt"), "more\n").unwrap();
    std::fs::write(f.dir.join("f.txt"), "edited again\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::WindowFocused));
    assert!(listed(&st).contains(&("more.txt".to_owned(), 'A')), "{:?}", listed(&st));
    let (_, diff) = st.diff.clone().expect("still open");
    assert!(diff.hunks.iter().flat_map(|h| &h.lines).any(|l| l.origin == '+' && l.text == "edited again"));

    // The commit leaves the history: the WIP row is selected (the tree is dirty), the diff closes.
    git(&f.dir, &["reset", "-q", "--hard", &f.c(2).to_string()]);
    git(&f.dir, &["branch", "-q", "-D", "side"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(st.selection, Selection::Wip);
    assert!(st.diff.is_none());
    let _ = std::fs::remove_dir_all(&f.dir);
}
