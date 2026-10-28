//! Lists that recreate merges, against fixture repositories: their shape, and running them.

use super::plan_tests::{commit, head};
use super::run_tests::{log, message, out, start_copy};
use super::*;
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

/// The commits of [`merged`].
pub(super) struct Merged {
    pub c2: Oid,
    pub s1: Oid,
    pub s2: Oid,
    pub c3: Oid,
    pub m: Oid,
    pub c4: Oid,
}

/// main: c1, c2; side: s1, s2 from c2; main: c3, then M merging side, then c4.
pub(super) fn merged(name: &str) -> (PathBuf, Merged) {
    let dir = scratch_repo(name);
    commit(&dir, "1.txt", "1\n", "c1");
    let c2 = commit(&dir, "2.txt", "2\n", "c2");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    let s1 = commit(&dir, "s1.txt", "s1\n", "s1");
    let s2 = commit(&dir, "s2.txt", "s2\n", "s2");
    git(&dir, &["checkout", "-q", "main"]);
    let c3 = commit(&dir, "3.txt", "3\n", "c3");
    git(&dir, &["merge", "-q", "--no-ff", "side", "-m", "Merge side"]);
    let m = head(&dir);
    let c4 = commit(&dir, "4.txt", "4\n", "c4");
    (dir, Merged { c2, s1, s2, c3, m, c4 })
}

pub(super) fn label(o: Oid) -> String {
    format!("at-{}", &o.to_string()[..7])
}

/// Each step in a few words: "pick c3", "label at-…", "reset onto", "merge Merge side at-…".
fn shape(steps: &[TodoStep]) -> Vec<String> {
    steps
        .iter()
        .map(|s| match s {
            TodoStep::Commit(i) => format!("{} {}", i.action.keyword(), i.summary),
            TodoStep::Label { name, .. } => format!("label {name}"),
            TodoStep::UpdateRef(r) => format!("update-ref {r}"),
            TodoStep::Reset(ResetTo::Onto) => "reset onto".into(),
            TodoStep::Reset(ResetTo::Label(name)) => format!("reset {name}"),
            TodoStep::Reset(ResetTo::Commit(o)) => format!("reset {o}"),
            TodoStep::Reset(ResetTo::NewRoot) => "reset [new root]".into(),
            TodoStep::Merge(m) => {
                let parents: Vec<String> = m
                    .parents
                    .iter()
                    .map(|p| match p {
                        MergeParent::Label(name) => name.clone(),
                        MergeParent::Commit(o) => o.to_string(),
                    })
                    .collect();
                format!("merge {} {}", m.item.summary, parents.join(" "))
            }
            TodoStep::Exec(c) => format!("exec {c}"),
            TodoStep::Break => "break".into(),
        })
        .collect()
}

fn steps_of(dir: &Path, base: RebaseBase) -> Vec<TodoStep> {
    plan(dir, &base).unwrap().steps
}

fn parents(dir: &Path, rev: &str) -> Vec<Oid> {
    Repository::open(dir).unwrap().revparse_single(rev).unwrap().peel_to_commit().unwrap().parent_ids().collect()
}

#[test]
fn the_script_replays_each_line_after_a_reset() {
    let (dir, f) = merged("rebase-merges-script");
    let steps = steps_of(&dir, RebaseBase::FromCommit(f.c3));
    let s2 = label(f.s2);
    let want =
        ["reset onto", "pick s1", "pick s2", &format!("label {s2}"), "reset onto", "pick c3", &format!("merge Merge side {s2}"), "pick c4"];
    assert_eq!(shape(&steps), want);
    assert_eq!(steps[3], TodoStep::Label { name: s2.clone(), at: f.s2 });
    assert_eq!(steps[6].oid(), Some(f.m));
    assert!(merges::has_structure(&steps));

    let (text, files) = build(&steps, Path::new("/repo/.git/glowfly-rebase"), false);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[..3], ["label onto", "reset onto", &format!("pick {} s1", f.s1)]);
    assert_eq!(lines[4], format!("label {s2}"));
    assert_eq!(lines[7], format!("merge -C {} {s2} # Merge side", f.m));
    assert_eq!(lines.len(), 9);
    assert!(files.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn branch_points_outside_parents_and_roots() {
    // A side line forked from a commit inside the range: that commit gets a label.
    let dir = scratch_repo("rebase-merges-branch-point");
    commit(&dir, "1.txt", "1\n", "c1");
    let c2 = commit(&dir, "2.txt", "2\n", "c2");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    let s1 = commit(&dir, "s.txt", "s\n", "s1");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "3.txt", "3\n", "c3");
    git(&dir, &["merge", "-q", "--no-ff", "side", "-m", "Merge side"]);
    let steps = steps_of(&dir, RebaseBase::FromCommit(c2));
    let (at2, at_s) = (label(c2), label(s1));
    let want = [
        "reset onto".to_owned(),
        "pick c2".into(),
        format!("label {at2}"),
        "pick s1".into(),
        format!("label {at_s}"),
        format!("reset {at2}"),
        "pick c3".into(),
        format!("merge Merge side {at_s}"),
    ];
    assert_eq!(shape(&steps), want);
    assert_eq!(steps[5], TodoStep::Reset(ResetTo::Label(at2)));

    // The same history from the root: the first line starts a new root commit.
    let root = Repository::open(&dir).unwrap().revparse_single("HEAD~2").unwrap().peel_to_commit().unwrap().parent_id(0).unwrap();
    let p = plan(&dir, &RebaseBase::FromCommit(root)).unwrap();
    assert_eq!(p.upstream, Upstream::Root);
    assert_eq!(p.steps[0], TodoStep::Reset(ResetTo::NewRoot));
    let names: Vec<&str> =
        p.steps.iter().filter_map(|s| if let TodoStep::Label { name, .. } = s { Some(name.as_str()) } else { None }).collect();
    let unique: std::collections::HashSet<&&str> = names.iter().collect();
    assert_eq!(unique.len(), names.len(), "label names stay unique: {names:?}");
    assert!(matches!(
        start_copy(&dir, &StartRequest { head: p.head, upstream: p.upstream, items: p.steps, autostash: false }),
        Outcome::Finished { .. }
    ));
    assert_eq!(parents(&dir, "HEAD").len(), 2, "the merge is recreated");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lines_and_parents_outside_the_range_stay_where_they_are() {
    // A side line forked below the upstream keeps its base.
    let dir = scratch_repo("rebase-merges-kept");
    let c1 = commit(&dir, "1.txt", "1\n", "c1");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    commit(&dir, "s.txt", "s\n", "s1");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "2.txt", "2\n", "c2");
    let c3 = commit(&dir, "3.txt", "3\n", "c3");
    git(&dir, &["merge", "-q", "--no-ff", "side", "-m", "Merge side"]);
    let steps = steps_of(&dir, RebaseBase::FromCommit(c3));
    assert_eq!(steps[0], TodoStep::Reset(ResetTo::Commit(c1)), "{:?}", shape(&steps));
    let _ = std::fs::remove_dir_all(&dir);

    // main merged into the branch: the merge's later parent is outside the range.
    let dir = scratch_repo("rebase-merges-outside-parent");
    commit(&dir, "1.txt", "1\n", "c1");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    commit(&dir, "f.txt", "f\n", "f1");
    git(&dir, &["checkout", "-q", "main"]);
    let m2 = commit(&dir, "m.txt", "m\n", "m2");
    git(&dir, &["checkout", "-q", "feature"]);
    git(&dir, &["merge", "-q", "--no-ff", "main", "-m", "Merge main"]);
    commit(&dir, "g.txt", "g\n", "f2");
    let steps = steps_of(&dir, RebaseBase::OntoBranch("main".into()));
    let merge = steps.iter().find_map(TodoStep::merge).expect("a merge step");
    assert_eq!(merge.parents, [MergeParent::Commit(m2)]);
    let (text, _) = build(&steps, Path::new("/x"), false);
    assert!(text.contains(&format!("merge -C {} {m2} # Merge main", merge.item.oid)), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_reword_and_a_drop_around_a_merge_run() {
    let (dir, f) = merged("rebase-merges-run");
    let mut steps = steps_of(&dir, RebaseBase::FromCommit(f.c3));
    steps[1].commit_mut().unwrap().action = RebaseAction::Drop;
    let c3 = steps[5].commit_mut().unwrap();
    c3.action = RebaseAction::Reword;
    c3.new_message = Some("c3, reworded".into());
    let req = StartRequest { head: f.c4, upstream: Upstream::Commit(f.c2), items: steps, autostash: false };
    let outcome = start_copy(&dir, &req);
    assert!(matches!(outcome, Outcome::Finished { .. }), "{outcome:?}");
    let merge = parents(&dir, "HEAD~1");
    assert_eq!(merge.len(), 2, "HEAD~1 is still a merge");
    assert_eq!(message(&dir, "HEAD~1"), "Merge side", "the merge keeps its message");
    assert_eq!(message(&dir, "HEAD~2"), "c3, reworded");
    let side = out(&dir, &["log", "--format=%s", &format!("{}..{}", parents(&dir, "HEAD~1")[0], merge[1])]);
    assert_eq!(side.lines().collect::<Vec<_>>(), ["s2"], "the side line holds only s2");
    assert!(!dir.join("s1.txt").exists() && dir.join("s2.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);

    // A reworded merge row: the merge gets the typed message and keeps both parents.
    let (dir, f) = merged("rebase-merges-run-reword");
    let mut steps = steps_of(&dir, RebaseBase::FromCommit(f.c3));
    let TodoStep::Merge(m) = &mut steps[6] else { panic!("expected the merge row") };
    m.item.action = RebaseAction::Reword;
    m.item.new_message = Some("Merged side, reworded".into());
    let req = StartRequest { head: f.c4, upstream: Upstream::Commit(f.c2), items: steps, autostash: false };
    assert!(matches!(start_copy(&dir, &req), Outcome::Finished { .. }));
    assert_eq!(message(&dir, "HEAD~1"), "Merged side, reworded");
    assert_eq!(parents(&dir, "HEAD~1").len(), 2);
    assert_eq!(log(&dir)[0], "c4");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn onto_a_branch_moves_the_line_and_recreates_the_merge() {
    let dir = scratch_repo("rebase-merges-onto");
    commit(&dir, "0.txt", "0\n", "b0");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    commit(&dir, "f1.txt", "1\n", "f1");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    commit(&dir, "s.txt", "s\n", "s1");
    git(&dir, &["checkout", "-q", "feature"]);
    commit(&dir, "f2.txt", "2\n", "f2");
    git(&dir, &["merge", "-q", "--no-ff", "side", "-m", "Merge side"]);
    git(&dir, &["checkout", "-q", "main"]);
    let tip = commit(&dir, "m.txt", "m\n", "m1");
    git(&dir, &["checkout", "-q", "feature"]);
    let p = plan(&dir, &RebaseBase::OntoBranch("main".into())).unwrap();
    assert_eq!(p.steps.iter().filter_map(TodoStep::oid).count(), 4, "f1, s1, f2 and the merge");
    let req = StartRequest { head: p.head, upstream: p.upstream, items: p.steps, autostash: false };
    let outcome = start_copy(&dir, &req);
    assert!(matches!(outcome, Outcome::Finished { .. }), "{outcome:?}");
    let repo = Repository::open(&dir).unwrap();
    assert_eq!(repo.merge_base(head(&dir), tip).unwrap(), tip, "the branch now starts on main's tip");
    assert_eq!(parents(&dir, "HEAD").len(), 2, "the merge is recreated");
    assert!(dir.join("m.txt").exists() && dir.join("s.txt").exists() && dir.join("f2.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
