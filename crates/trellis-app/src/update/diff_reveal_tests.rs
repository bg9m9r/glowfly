//! Headless tests for `ViewState::file_reveal`: set when a next/previous change step lands, so
//! the file list scrolls to that file once; a click open, a reload and closing the diff don't
//! set it, and the latter two clear it.

use super::diff_nav_tests::{at_end_of, finish, nav_repo, run, shown, unstaged};
use crate::msg::{DiffMsg, Msg, RepoMsg};
use trellis_core::repo::DiffTarget;

#[test]
fn a_landed_step_reveals_its_file() {
    let (mut st, dir) = nav_repo("reveal");
    at_end_of(&mut st, unstaged("a.txt"));
    assert_eq!(st.view.file_reveal, None, "a click open reveals nothing");
    let fx = run(&mut st, Msg::Diff(DiffMsg::JumpToChange(1)));
    assert_eq!(st.view.file_reveal, None, "not before it lands");
    finish(&mut st, fx);
    // b.bin has nothing to land on: the step goes on to c.txt.
    let landed = shown(&st).cloned();
    assert_eq!(landed, Some(unstaged("c.txt")));
    assert_eq!(st.view.file_reveal, Some((unstaged("c.txt"), st.diff_shown_gen)));

    // A reload keeps the landing it already revealed.
    let fx = run(&mut st, Msg::Repo(RepoMsg::Refresh));
    finish(&mut st, fx);
    assert_eq!(shown(&st).cloned(), landed);
    assert!(st.view.file_reveal.as_ref().is_none_or(|(t, g)| Some(t) == landed.as_ref() && *g < st.diff_shown_gen));

    // Opening a file from the list, or closing the diff, clears it.
    let fx = run(&mut st, Msg::Diff(DiffMsg::JumpToChange(-1)));
    finish(&mut st, fx);
    assert_eq!(shown(&st).cloned(), Some(unstaged("a.txt")));
    assert!(st.view.file_reveal.is_some());
    let fx = run(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Staged("d.txt".into()))));
    assert_eq!(st.view.file_reveal, None, "a click open");
    finish(&mut st, fx);
    assert_eq!(st.view.file_reveal, None);
    let fx = run(&mut st, Msg::Diff(DiffMsg::JumpToChange(-1)));
    finish(&mut st, fx);
    assert!(st.view.file_reveal.is_some());
    run(&mut st, Msg::Repo(RepoMsg::CloseDiff));
    assert_eq!(st.view.file_reveal, None, "closing the diff");
    let _ = std::fs::remove_dir_all(&dir);
}
