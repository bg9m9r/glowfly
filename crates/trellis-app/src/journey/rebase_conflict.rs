//! Rebase a branch onto a diverged main from the branch menu, stop on the conflict, resolve it
//! in the conflict view (take theirs, save) and continue to a linear history.

use super::find::{Pick, Region};
use super::fixture::{self as disk, Fixture, commit_files, git};
use super::harness::Journey;
use git2::RepositoryState;

#[test]
fn rebase_conflict_resolved_in_merge_view() {
    let fx = Fixture::new("rebase");
    let repo = fx.work().join("app");
    fx.init(&repo, &[("f.txt", "base\n")]);
    git(&repo, &["checkout", "-q", "-b", "topic"]);
    commit_files(&repo, &[("f.txt", "topic\n")], "Topic change");
    git(&repo, &["checkout", "-q", "main"]);
    commit_files(&repo, &[("f.txt", "main\n")], "Main change");
    commit_files(&repo, &[("g.txt", "more\n")], "Another main commit");
    let main_tip = disk::head_oid(&repo);
    git(&repo, &["checkout", "-q", "topic"]);
    let mut j = Journey::start(fx, Some(repo.clone()), vec![repo.clone()]);
    j.see("✔  topic", Region::Sidebar);

    // step 1: the rebase stops on f.txt, with the conflict markers in the working file.
    j.branch_menu("main", "Rebase topic onto this branch");
    j.toast(false, "Rebase failed");
    let rebasing = |repo: &std::path::Path| {
        matches!(disk::state(repo), RepositoryState::Rebase | RepositoryState::RebaseMerge | RepositoryState::RebaseInteractive)
    };
    assert!(rebasing(&repo));
    assert_eq!(disk::conflicted_paths(&repo), ["f.txt"]);
    assert!(disk::work_text(&repo, "f.txt").unwrap().contains("<<<<<<<"));

    // step 2: the working tree's banner names the conflict and holds Continue back; viewing it
    // changes nothing on disk.
    j.click("View uncommitted changes", Region::Details, Pick::Only);
    j.see("1 conflicted file", Region::Details);
    j.see("View conflicts", Region::Details);
    j.disabled("Continue", Region::Details);
    j.see("Abort rebase", Region::Details);
    assert!(rebasing(&repo));
    assert_eq!(disk::conflicted_paths(&repo), ["f.txt"]);

    // step 3: the conflict view; theirs (the commit being replayed) wins, and only Save and mark
    // resolved writes it: the file is then resolved and staged, so Continue is offered.
    j.click("View conflicts", Region::Details, Pick::Only);
    j.see("Ours", Region::Central);
    j.see("Theirs", Region::Central);
    j.disabled("Save and mark resolved", Region::Central);
    j.click("Take all", Region::Central, Pick::Rightmost);
    j.enabled("Save and mark resolved", Region::Central);
    assert_eq!(disk::conflicted_paths(&repo), ["f.txt"]);
    assert!(disk::work_text(&repo, "f.txt").unwrap().contains("<<<<<<<"));
    j.click("Save and mark resolved", Region::Central, Pick::Only);
    assert_eq!(disk::work_text(&repo, "f.txt").as_deref(), Some("topic\n"));
    assert!(disk::conflicted_paths(&repo).is_empty());
    assert_eq!(disk::index_text(&repo, "f.txt").as_deref(), Some("topic\n"));
    assert!(rebasing(&repo));
    j.absent("1 conflicted file", Region::Details);
    j.enabled("Continue", Region::Details);

    // step 4: continuing finishes the rebase: topic's commit now sits on main's tip.
    j.click("Continue", Region::Details, Pick::Only);
    assert_eq!(disk::state(&repo), RepositoryState::Clean);
    assert_eq!(disk::head_branch(&repo).as_deref(), Some("topic"));
    let head = disk::head_oid(&repo);
    assert_eq!(disk::summary(&repo, head), "Topic change");
    assert_eq!(disk::parents(&repo, head), [main_tip]);
    let text = disk::work_text(&repo, "f.txt").unwrap();
    assert_eq!(text, "topic\n");
    assert!(!text.contains("<<<<<<<"));
    assert!(disk::is_clean(&repo));
    j.absent("Continue", Region::Details);
    j.absent("Abort rebase", Region::Details);
    j.see("topic", Region::Toolbar);
}
