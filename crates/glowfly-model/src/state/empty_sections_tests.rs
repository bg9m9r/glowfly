//! The folding of empty left-panel sections: collapsed at first, toggled apart from the
//! populated section's flag, kept open when emptied in view, and reset by another repository.

use super::{EmptySections, Folding};
use std::path::Path;

#[test]
fn an_empty_section_starts_collapsed_and_toggles_its_own_flag() {
    let mut e = EmptySections::default();
    let mut open = true;
    let repo = Path::new("/r");
    assert!(!*e.open_mut(repo, Folding::Worktrees, 0, &mut open), "collapsed while empty");
    *e.open_mut(repo, Folding::Worktrees, 0, &mut open) = true;
    assert!(*e.open_mut(repo, Folding::Worktrees, 0, &mut open), "the user expanded it");
    assert!(open, "the populated section's flag is untouched");
    assert!(!*e.open_mut(repo, Folding::Stashes, 0, &mut open), "each section has its own flag");
}

#[test]
fn a_populated_section_uses_its_own_flag() {
    let mut e = EmptySections::default();
    let mut open = false;
    *e.open_mut(Path::new("/r"), Folding::Stashes, 2, &mut open) = true;
    assert!(open);
}

#[test]
fn a_section_emptied_in_view_keeps_its_open_state() {
    let mut e = EmptySections::default();
    let repo = Path::new("/r");
    let mut open = true;
    assert!(*e.open_mut(repo, Folding::Stashes, 1, &mut open));
    assert!(*e.open_mut(repo, Folding::Stashes, 0, &mut open), "the last stash popped: still open");
    let mut closed = false;
    e.open_mut(repo, Folding::Remote, 1, &mut closed);
    assert!(!*e.open_mut(repo, Folding::Remote, 0, &mut closed), "a closed one stays closed");
}

#[test]
fn another_repository_starts_collapsed() {
    let mut e = EmptySections::default();
    let mut open = true;
    *e.open_mut(Path::new("/a"), Folding::Worktrees, 0, &mut open) = true;
    assert!(!*e.open_mut(Path::new("/b"), Folding::Worktrees, 0, &mut open));
}
