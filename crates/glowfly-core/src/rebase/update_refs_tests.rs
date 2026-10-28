//! `rebase.updateRefs`: the update-ref steps the plan predicts, starting a rebase that moves the
//! branches stacked inside it, and editing a running rebase's update-ref lines.

use super::plan_tests::linear;
use super::remaining_tests::rebase_i;
use super::run_tests::{log, out, start_copy, state};
use super::*;
use crate::error::Error;
use crate::testutil::*;
use git2::RepositoryState;
use std::path::{Path, PathBuf};

/// main: c1..c4 with `rebase.updateRefs` on, `lower` at c2 and `mid` at c3.
pub(super) fn stacked(name: &str) -> (PathBuf, [Oid; 4]) {
    let (dir, c) = linear(name);
    set_config(&dir, &[("rebase.updateRefs", "true")]);
    git(&dir, &["branch", "lower", &c[1].to_string()]);
    git(&dir, &["branch", "mid", &c[2].to_string()]);
    (dir, c)
}

fn ur(branch: &str) -> TodoStep {
    TodoStep::UpdateRef(format!("refs/heads/{branch}"))
}

/// Each step as "pick c2" or "update-ref lower".
pub(super) fn kinds(steps: &[TodoStep]) -> Vec<String> {
    steps
        .iter()
        .map(|s| match s {
            TodoStep::Commit(i) => format!("{} {}", i.action.keyword(), i.summary),
            TodoStep::UpdateRef(_) => format!("update-ref {}", s.update_ref().unwrap()),
            TodoStep::Exec(c) => format!("exec {c}"),
            TodoStep::Break => "break".into(),
            other => format!("{other:?}"),
        })
        .collect()
}

fn rev(dir: &Path, r: &str) -> String {
    out(dir, &["rev-parse", r]).trim().to_owned()
}

/// The repository's own git folder (a worktree's is not `.git`).
pub(super) fn git_dir(dir: &Path) -> PathBuf {
    git2::Repository::open(dir).unwrap().path().to_path_buf()
}

pub(super) fn plan_from(dir: &Path, c: Oid) -> RebasePlan {
    plan(dir, &RebaseBase::FromCommit(c)).unwrap()
}

fn start_steps(dir: &Path, p: &RebasePlan, steps: Vec<TodoStep>) -> Outcome {
    start_copy(dir, &StartRequest { head: p.head, upstream: p.upstream, items: steps, autostash: false })
}

pub(super) fn unsupported<T: std::fmt::Debug>(r: crate::error::Result<T>) -> String {
    match r {
        Err(Error::Unsupported(t)) => t,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// The list git itself writes for the rebase `p` plans, saved by a sequence editor that then
/// fails, so nothing starts.
pub(super) fn gits_todo(dir: &Path, p: &RebasePlan) -> String {
    let saved = git_dir(dir).join("gits-todo");
    let editor = SequenceEditor::Shell(format!("sh -c 'cp \"$1\" {} && exit 1' sh", shell_quote(&saved.to_string_lossy())));
    let req = StartRequest { head: p.head, upstream: p.upstream, items: p.steps.clone(), autostash: false };
    let _ = start(dir, &req, &editor);
    assert_eq!(state(dir), RepositoryState::Clean);
    String::from_utf8_lossy(&std::fs::read(&saved).unwrap()).into_owned()
}

/// Whether git accepts the list built from `steps` in place of its own `theirs`.
pub(super) fn installs(dir: &Path, theirs: &str, steps: &[TodoStep]) -> crate::error::Result<()> {
    let (saved, ours) = (git_dir(dir).join("gits-todo"), git_dir(dir).join("our-todo"));
    std::fs::write(&saved, theirs).unwrap();
    std::fs::write(&ours, build(steps, &git_dir(dir), false).0).unwrap();
    install_todo(&ours, &saved)
}

#[test]
fn the_plan_adds_an_update_ref_after_each_stacked_branchs_commit() {
    let (dir, [_, c2, ..]) = stacked("update-refs-plan");
    git(&dir, &["branch", "also-head", "main"]);
    let p = plan_from(&dir, c2);
    assert_eq!(kinds(&p.steps), ["pick c2", "update-ref lower", "pick c3", "update-ref mid", "pick c4", "update-ref also-head"]);

    let theirs = gits_todo(&dir, &p);
    assert_eq!(listed_refs(&theirs).len(), 3, "{theirs}");
    installs(&dir, &theirs, &p.steps).expect("the plan's list matches git's");

    set_config(&dir, &[("rebase.updateRefs", "false")]);
    let off = plan_from(&dir, c2);
    assert_eq!(kinds(&off.steps), ["pick c2", "pick c3", "pick c4"], "off: no update-ref steps");
    assert!(listed_refs(&gits_todo(&dir, &off)).is_empty());
    let why = unsupported(installs(&dir, &gits_todo(&dir, &off), &p.steps));
    assert!(why.starts_with("git won't move lower, mid, also-head with this rebase"), "{why}");
    assert_eq!(unsupported(installs(&dir, &theirs, &off.steps)), REFS_CHANGED, "git moving branches the list doesn't");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A range with a merge: the side branch's update follows its last commit, before the label
/// that names it, and moves to the rewritten side line.
#[test]
fn a_list_that_recreates_merges_moves_the_side_branch() {
    let (dir, f) = super::merges_tests::merged("update-refs-merges");
    set_config(&dir, &[("rebase.updateRefs", "true")]);
    let p = plan_from(&dir, f.s1);
    let at = p.steps.iter().position(|s| s.oid() == Some(f.s2)).unwrap();
    assert_eq!(p.steps[at + 1], ur("side"));
    assert!(matches!(p.steps[at + 2], TodoStep::Label { .. }));
    installs(&dir, &gits_todo(&dir, &p), &p.steps).expect("the plan's list matches git's");

    let mut steps = p.steps.clone();
    let s1 = steps.iter_mut().find_map(|s| s.commit_mut().filter(|i| i.oid == f.s1)).unwrap();
    s1.action = RebaseAction::Reword;
    s1.new_message = Some("s1 again".into());
    assert!(matches!(start_steps(&dir, &p, steps), Outcome::Finished { .. }));
    assert_eq!(rev(&dir, "side"), rev(&dir, "main~1^2"));
    assert_ne!(rev(&dir, "side"), f.s2.to_string());
    assert_eq!(out(&dir, &["log", "-1", "--format=%s", "side~1"]).trim(), "s1 again");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A branch checked out in another worktree: git writes a comment in place of its update-ref
/// (and would move it anyway if the list named it), so the plan leaves it out, and a list that
/// names it is refused.
#[test]
fn a_branch_checked_out_in_another_worktree_is_left_alone() {
    let (dir, [_, c2, c3, _]) = stacked("update-refs-worktree");
    let wt = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&dir, &["worktree", "add", "-q", "-b", "elsewhere", &wt.to_string_lossy(), &c3.to_string()]);
    let p = plan_from(&dir, c2);
    assert_eq!(kinds(&p.steps), ["pick c2", "update-ref lower", "pick c3", "update-ref mid", "pick c4"]);

    let theirs = gits_todo(&dir, &p);
    assert!(theirs.contains("# Ref refs/heads/elsewhere checked out at"), "{theirs}");

    installs(&dir, &theirs, &p.steps).expect("the plan's list matches git's");
    let mut more = p.steps.clone();
    more.insert(3, ur("elsewhere"));
    let why = unsupported(installs(&dir, &theirs, &more));
    assert!(why.starts_with("git won't move elsewhere (checked out at "), "{why}");
    let fewer: Vec<TodoStep> = p.steps.iter().filter(|s| s.update_ref() != Some("mid")).cloned().collect();
    assert_eq!(unsupported(installs(&dir, &theirs, &fewer)), REFS_CHANGED);

    git(&dir, &["worktree", "remove", "--force", &wt.to_string_lossy()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn starting_moves_the_stacked_branches() {
    let (dir, [_, c2, c3, _]) = stacked("update-refs-start");
    let p = plan_from(&dir, c2);
    let mut steps = p.steps.clone();
    // Reword c3 (its row is 2): every commit from it on is rewritten.
    let item = steps[2].commit_mut().unwrap();
    item.action = RebaseAction::Reword;
    item.new_message = Some("c3 again".into());
    assert!(matches!(start_steps(&dir, &p, steps), Outcome::Finished { .. }));
    assert_eq!(log(&dir), ["c4", "c3 again", "c2", "c1"]);
    assert_eq!(rev(&dir, "lower"), rev(&dir, "main~2"));
    assert_eq!(rev(&dir, "mid"), rev(&dir, "main~1"), "mid moved to the reworded commit");
    assert_ne!(rev(&dir, "mid"), c3.to_string());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_moved_commit_takes_its_branch_along() {
    let (dir, [_, c2, ..]) = stacked("update-refs-move");
    let p = plan_from(&dir, c2);
    let s = &p.steps;
    let moved = vec![s[2].clone(), s[3].clone(), s[0].clone(), s[1].clone(), s[4].clone()];
    assert_eq!(kinds(&moved), ["pick c3", "update-ref mid", "pick c2", "update-ref lower", "pick c4"]);
    assert!(matches!(start_steps(&dir, &p, moved), Outcome::Finished { .. }));
    assert_eq!(log(&dir), ["c4", "c2", "c3", "c1"]);
    assert_eq!(rev(&dir, "mid"), rev(&dir, "main~2"));
    assert_eq!(rev(&dir, "lower"), rev(&dir, "main~1"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A fixup into the lower branch's tip: the branch moves to the combined commit, not to the
/// one before the fixup.
#[test]
fn a_fixup_into_a_branch_tip_moves_the_branch_to_the_combined_commit() {
    let (dir, [_, c2, ..]) = stacked("update-refs-fixup");
    let p = plan_from(&dir, c2);
    let mut steps = p.steps.clone();
    steps[2].commit_mut().unwrap().action = RebaseAction::Fixup;
    assert_eq!(group_end(&steps, 0), Some(2), "the update-ref doesn't end the group");
    assert_eq!(problem(&steps), None);
    let text = build(&steps, &dir, false).0;
    let shape: Vec<&str> = text.lines().map(|l| if l.starts_with("update-ref") { l } else { l.split(' ').next().unwrap() }).collect();
    assert_eq!(shape, ["pick", "fixup", "update-ref refs/heads/lower", "update-ref refs/heads/mid", "pick"]);
    assert!(matches!(start_steps(&dir, &p, steps), Outcome::Finished { .. }));
    assert_eq!(log(&dir), ["c4", "c2", "c1"]);
    assert_eq!(rev(&dir, "lower"), rev(&dir, "main~1"));
    assert_eq!(rev(&dir, "mid"), rev(&dir, "main~1"));
    assert!(out(&dir, &["ls-tree", "--name-only", "lower"]).contains("3.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unchanged_list_with_update_refs_is_unchanged() {
    let (dir, [_, c2, ..]) = stacked("update-refs-unchanged");
    let p = plan_from(&dir, c2);
    assert!(is_unchanged(&p.steps, &p.steps));
    let mut other = p.steps.clone();
    other.swap(1, 3);
    assert!(!is_unchanged(&other, &p.steps));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Stopped at c2 with the lower branch's update next: the remaining steps show the update-ref
/// lines, an edit keeps them, and the branches end where their commits do.
#[test]
fn editing_a_running_rebase_keeps_its_update_refs() {
    let (dir, [c1, c2, c3, c4]) = stacked("update-refs-remaining");
    let todo = format!("edit {c2} c2\nu refs/heads/lower\npick {c3} c3\nupdate-ref refs/heads/mid\npick {c4} c4\n");
    assert!(rebase_i(&dir, &c1.to_string(), &todo));
    let r = remaining(&dir).unwrap();
    assert_eq!(kinds(&r.steps), ["update-ref lower", "pick c3", "update-ref mid", "pick c4"]);

    let req =
        |steps: Vec<TodoStep>| EditTodoRequest { steps, locked_text: r.locked_text.clone(), todo: r.todo.clone(), first_may_combine: true };
    let s = &r.steps;
    let without_mid = vec![s[0].clone(), s[1].clone(), s[3].clone()];
    assert_eq!(unsupported(edit_todo(&dir, &req(without_mid), &SequenceEditor::Copy)), REFS_CHANGED);
    let with_more = [s.clone(), vec![ur("main-copy")]].concat();
    assert_eq!(unsupported(edit_todo(&dir, &req(with_more), &SequenceEditor::Copy)), REFS_CHANGED);

    // c4 before c3, which takes mid along.
    let moved = vec![s[0].clone(), s[3].clone(), s[1].clone(), s[2].clone()];
    edit_todo(&dir, &req(moved), &SequenceEditor::Copy).unwrap();
    let after = remaining(&dir).unwrap();
    assert_eq!(kinds(&after.steps), ["update-ref lower", "pick c4", "pick c3", "update-ref mid"]);
    assert!(matches!(resume(&dir, Resume::Continue).unwrap(), Outcome::Finished { .. }));
    assert_eq!(log(&dir), ["c3", "c4", "c2", "c1"]);
    assert_eq!(rev(&dir, "lower"), rev(&dir, "main~2"));
    assert_eq!(rev(&dir, "mid"), rev(&dir, "main"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// An update-ref between a commit and a fixup into it can't be written back where it is (the
/// list keeps a branch's update after its group), so the steps are refused.
#[test]
fn an_update_ref_inside_a_squash_group_is_refused() {
    let (dir, [c1, c2, c3, c4]) = stacked("update-refs-in-group");
    for todo in [
        format!("edit {c2} c2\npick {c3} c3\nupdate-ref refs/heads/mid\nfixup {c4} c4\n"),
        format!("edit {c2} c2\nupdate-ref refs/heads/lower\ndrop {c3} c3\nsquash {c4} c4\n"),
    ] {
        assert!(rebase_i(&dir, &c1.to_string(), &todo), "{todo}");
        assert_eq!(unsupported(remaining(&dir)), UPDATE_REF_IN_GROUP, "{todo}");
        git(&dir, &["rebase", "--abort"]);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn listed_refs_reads_both_spellings_and_skips_comments() {
    let text = "pick abc c1\nupdate-ref refs/heads/a\n# Ref refs/heads/b checked out at '/x'\nu refs/heads/c\n";
    assert_eq!(listed_refs(text), ["refs/heads/a", "refs/heads/c"]);
}

/// The graph's quick edits run the same list: a reword moves the branches above it, and a
/// squash across a branch's tip is left to the panel.
#[test]
fn quick_edits_keep_the_stacked_branches() {
    let (dir, [_, c2, ..]) = stacked("update-refs-quick");
    let mut q = prepare_edit(&dir, &QuickEdit::Reword(c2)).unwrap();
    assert_eq!(kinds(&q.request.items), ["reword c2", "update-ref lower", "pick c3", "update-ref mid", "pick c4"]);
    q.request.items[0].commit_mut().unwrap().new_message = Some("c2 again".into());
    assert!(matches!(start_copy(&dir, &q.request), Outcome::Finished { .. }));
    assert_eq!(log(&dir), ["c4", "c3", "c2 again", "c1"]);
    assert_eq!(rev(&dir, "lower"), rev(&dir, "main~2"));
    assert_eq!(rev(&dir, "mid"), rev(&dir, "main~1"));

    let c3 = Oid::from_str(&rev(&dir, "main~1")).unwrap();
    assert_eq!(unsupported(prepare_edit(&dir, &QuickEdit::SquashIntoParent(c3))), SPLIT_BY_BRANCH);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A quick drop of a commit another branch points at would take it from that branch too.
#[test]
fn a_quick_drop_of_a_branch_tip_is_refused() {
    let (dir, [_, c2, c3, c4]) = stacked("update-refs-quick-drop");
    assert_eq!(unsupported(prepare_edit(&dir, &QuickEdit::Drop(vec![c3]))), BRANCH_TIP);
    assert_eq!(unsupported(prepare_edit(&dir, &QuickEdit::Drop(vec![c2, c4]))), SOME_BRANCH_TIPS);
    prepare_edit(&dir, &QuickEdit::Drop(vec![c4])).expect("no other branch points at c4");
    let _ = std::fs::remove_dir_all(&dir);
}
