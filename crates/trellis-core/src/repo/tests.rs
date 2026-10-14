use super::*;
use crate::gitcli::{args, run};
use crate::testutil::*;

#[test]
fn history_graph_and_refs() {
    let dir = scratch_repo("graph");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "first"]);
    git(&dir, &["checkout", "-b", "feature"]);
    write(&dir, "b.txt", "feature\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "feature work"]);
    git(&dir, &["checkout", "main"]);
    write(&dir, "a.txt", "one\ntwo\n");
    git(&dir, &["commit", "-am", "main work"]);
    git(&dir, &["merge", "--no-ff", "feature", "-m", "merge feature"]);
    git(&dir, &["tag", "v1"]);

    let s = load(&dir, &LoadOptions::all(1000)).unwrap();
    assert_eq!(s.commits.len(), 4);
    assert_eq!(s.head_branch.as_deref(), Some("main"));
    assert!(!s.is_dirty() && s.graph.wip.is_none());
    let head = &s.commits[0];
    assert_eq!(head.summary, "merge feature");
    assert_eq!(head.parents.len(), 2);
    // Merge node forks into two lanes, which rejoin at the root commit.
    assert_eq!(s.graph.rows[0].bottom.len(), 2);
    assert_eq!(s.graph.max_width, 2);
    assert_eq!(s.graph.rows[3].top.len(), 2, "both lanes converge on the root");
    let labels: Vec<_> = s.refs[&head.oid].iter().map(|l| l.name.as_str()).collect();
    assert_eq!(labels, ["main", "v1"]);
    assert!(s.refs[&head.oid][0].is_head);
    assert_eq!(s.locals.len(), 2);
    assert_eq!(s.tags.len(), 1);

    let d = commit_detail(&dir, head.oid).unwrap();
    assert_eq!(d.files.len(), 1, "merge diff vs first parent brings in b.txt");
    assert_eq!(d.files[0].path, "b.txt");
    // Smart visibility keeps HEAD's branch and the default branch; hiding drops a branch.
    git(&dir, &["branch", "side", "HEAD~2"]);
    git(&dir, &["checkout", "-q", "side"]);
    write(&dir, "c.txt", "side\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "side work"]);
    git(&dir, &["checkout", "-q", "-b", "other", "main"]);
    write(&dir, "d.txt", "other\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "other work"]);
    let all = load(&dir, &LoadOptions::all(1000)).unwrap();
    assert!(all.commits.iter().any(|c| c.summary == "side work"));
    let smart = load(&dir, &LoadOptions { max_commits: 1000, smart: true, ..Default::default() }).unwrap();
    assert!(smart.commits.iter().any(|c| c.summary == "other work"), "current branch");
    assert!(smart.commits.iter().any(|c| c.summary == "merge feature"), "default branch");
    assert!(!smart.commits.iter().any(|c| c.summary == "side work"), "unrelated branch hidden");
    let hidden = load(&dir, &LoadOptions { max_commits: 1000, hidden: vec!["refs/heads/side".into()], ..Default::default() }).unwrap();
    assert!(!hidden.commits.iter().any(|c| c.summary == "side work"));
    assert!(hidden.refs.values().flatten().all(|l| l.name != "side"));
    let main_tip = all.locals.iter().find(|b| b.name == "main").and_then(|b| b.oid);
    let other_tip = all.locals.iter().find(|b| b.name == "other").and_then(|b| b.oid).unwrap();
    let only_other: Vec<_> = all.commits_between(other_tip, main_tip).iter().map(|c| c.summary.as_str()).collect();
    assert_eq!(only_other, ["other work"]);
    assert_eq!(all.head_message.as_deref(), Some("other work"));
    let stats = commit_stats(&dir, &[all.commits[0].oid]);
    assert_eq!((stats[0].1, stats[0].2), (1, 1), "one file, one line added");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stage_unstage_discard_hunks() {
    let dir = scratch_repo("hunks");
    let base: String = (1..=30).map(|i| format!("line {i}\n")).collect();
    write(&dir, "f.txt", &base);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);

    // Two edits far apart produce two hunks.
    let edited = base.replace("line 2\n", "line 2 changed\n").replace("line 28\n", "line 28 changed\n");
    write(&dir, "f.txt", &edited);
    write(&dir, "new.txt", "brand new\n");

    let s = load(&dir, &LoadOptions::all(1000)).unwrap();
    assert!(s.graph.wip.is_some());
    let st: Vec<_> = s.unstaged.iter().map(|f| (f.path.as_str(), f.status)).collect();
    assert_eq!(st, [("f.txt", 'M'), ("new.txt", '?')]);

    let diff = load_diff(&dir, &DiffTarget::Unstaged("f.txt".into())).unwrap();
    assert_eq!(diff.hunks.len(), 2);
    let untracked = load_diff(&dir, &DiffTarget::Unstaged("new.txt".into())).unwrap();
    assert_eq!(untracked.hunks[0].lines[0].text, "brand new");

    // Stage only the second hunk.
    apply_hunk(&dir, "f.txt", 1, HunkOp::Stage).unwrap();
    let staged = load_diff(&dir, &DiffTarget::Staged("f.txt".into())).unwrap();
    assert_eq!(staged.hunks.len(), 1);
    assert!(staged.hunks[0].lines.iter().any(|l| l.text == "line 28 changed"));
    let unstaged = load_diff(&dir, &DiffTarget::Unstaged("f.txt".into())).unwrap();
    assert_eq!(unstaged.hunks.len(), 1);
    assert!(unstaged.hunks[0].lines.iter().any(|l| l.text == "line 2 changed"));

    // Unstage it again.
    apply_hunk(&dir, "f.txt", 0, HunkOp::Unstage).unwrap();
    assert!(load_diff(&dir, &DiffTarget::Staged("f.txt".into())).is_err());

    // Discard the first hunk from the working tree.
    apply_hunk(&dir, "f.txt", 0, HunkOp::Discard).unwrap();
    let on_disk = std::fs::read_to_string(dir.join("f.txt")).unwrap();
    assert!(!on_disk.contains("line 2 changed") && on_disk.contains("line 28 changed"));

    // Whole-file operations.
    stage_file(&dir, "new.txt").unwrap();
    let (_, staged, _) = status_at(&dir).unwrap();
    assert!(staged.iter().any(|f| f.path == "new.txt" && f.status == 'A'));
    unstage_paths(&dir, &["new.txt".into()]).unwrap();
    discard_untracked(&dir, "new.txt", Removal::Permanent).unwrap();
    discard_file(&dir, "f.txt").unwrap();
    let (unstaged, staged, _) = status_at(&dir).unwrap();
    assert!(unstaged.is_empty() && staged.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every line of every hunk lands in the split layout exactly once per side it belongs to,
/// and line numbers increase down each column.
fn assert_split_covers(d: &FileDiff) {
    use crate::diff_rows::{SplitRow, split_rows};
    let rows = split_rows(d);
    assert_eq!(rows.iter().filter(|r| matches!(r, SplitRow::Header(_))).count(), d.hunks.len());
    for (h, hunk) in d.hunks.iter().enumerate() {
        let (mut old_seen, mut new_seen) = (vec![0; hunk.lines.len()], vec![0; hunk.lines.len()]);
        let (mut last_old, mut last_new) = (0, 0);
        // A "no newline" marker repeats the number of the line it annotates.
        let marker = |i: usize| matches!(hunk.lines[i].origin, '<' | '>' | '=');
        for r in &rows {
            let SplitRow::Pair { hunk: rh, old, new } = *r else { continue };
            if rh != h {
                continue;
            }
            if let Some(i) = old {
                old_seen[i] += 1;
                if let (Some(n), false) = (hunk.lines[i].old, marker(i)) {
                    assert!(n > last_old, "old numbers increase on the left");
                    last_old = n;
                }
            }
            if let Some(i) = new {
                new_seen[i] += 1;
                if let (Some(n), false) = (hunk.lines[i].new, marker(i)) {
                    assert!(n > last_new, "new numbers increase on the right");
                    last_new = n;
                }
            }
        }
        for (i, line) in hunk.lines.iter().enumerate() {
            // libgit2's `>` (ADD_EOFNL) follows a removed line, `<` (DEL_EOFNL) an added one.
            let want = match line.origin {
                '-' | '>' => (1, 0),
                '+' | '<' => (0, 1),
                _ => (1, 1),
            };
            assert_eq!((old_seen[i], new_seen[i]), want, "hunk {h} line {i} ({:?})", line.origin);
        }
    }
}

#[test]
fn split_rows_from_real_diff() {
    let dir = scratch_repo("split-rows");
    let base: String = (1..=30)
        .map(|i| {
            format!(
                "line {i}
"
            )
        })
        .collect();
    write(&dir, "f.txt", &base);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);
    // A 1-for-2 replacement, a lone deletion and a pure addition: three uneven hunks.
    let edited = base
        .replace(
            "line 2
",
            "line 2 changed
line 2 extra
",
        )
        .replace(
            "line 15
", "",
        )
        .replace(
            "line 28
",
            "line 28
added a
added b
",
        );
    write(&dir, "f.txt", &edited);
    let d = load_diff(&dir, &DiffTarget::Unstaged("f.txt".into())).unwrap();
    assert_eq!(d.hunks.len(), 3);
    assert_split_covers(&d);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn split_rows_place_real_no_newline_markers() {
    use crate::diff_rows::{SplitRow, split_rows};
    let dir = scratch_repo("split-eofnl");
    write(
        &dir, "both.txt", "x
a",
    );
    write(
        &dir, "new.txt", "x
a
",
    );
    write(
        &dir, "old.txt", "x
a",
    );
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);
    write(
        &dir, "both.txt", "x
b",
    );
    write(
        &dir, "new.txt", "x
b",
    );
    write(
        &dir, "old.txt", "x
b
",
    );
    let pair = |old, new| SplitRow::Pair { hunk: 0, old, new };
    let load = |path: &str| {
        let d = load_diff(&dir, &DiffTarget::Unstaged(path.into())).unwrap();
        assert_split_covers(&d);
        let origins: String = d.hunks[0].lines.iter().map(|l| l.origin).collect();
        (origins, split_rows(&d))
    };

    // Neither file ends in a newline: `a` pairs with `b`, each marker under its own file.
    let (origins, rows) = load("both.txt");
    assert_eq!(origins, " ->+<");
    assert_eq!(rows[1..], [pair(Some(0), Some(0)), pair(Some(1), Some(3)), pair(Some(2), Some(4))]);
    // Only the new file lacks the newline: the marker is on the right.
    let (origins, rows) = load("new.txt");
    assert_eq!(origins, " -+<");
    assert_eq!(rows[1..], [pair(Some(0), Some(0)), pair(Some(1), Some(2)), pair(None, Some(3))]);
    // Only the old file lacks it: the marker is on the left.
    let (origins, rows) = load("old.txt");
    assert_eq!(origins, " ->+");
    assert_eq!(rows[1..], [pair(Some(0), Some(0)), pair(Some(1), Some(3)), pair(Some(2), None)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unborn_repo_and_conflict_state() {
    let dir = scratch_repo("conflict");
    let s = load(&dir, &LoadOptions::all(1000)).unwrap();
    assert!(s.commits.is_empty() && s.head_oid.is_none());
    assert_eq!(s.head_branch.as_deref(), Some("main"));

    write(&dir, "c.txt", "base\n");
    stage_all(&dir).unwrap();
    unstage_paths(&dir, &["c.txt".into()]).unwrap(); // works without HEAD
    stage_all(&dir).unwrap();
    git(&dir, &["commit", "-m", "base"]);
    git(&dir, &["checkout", "-b", "other"]);
    write(&dir, "c.txt", "theirs\n");
    git(&dir, &["commit", "-am", "theirs"]);
    git(&dir, &["checkout", "main"]);
    write(&dir, "c.txt", "ours\n");
    git(&dir, &["commit", "-am", "ours"]);
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "merge should conflict");

    let s = load(&dir, &LoadOptions::all(1000)).unwrap();
    assert_eq!(s.state, RepositoryState::Merge);
    assert_eq!(s.unstaged[0].status, '!');
    stage_file(&dir, "c.txt").unwrap(); // marks resolved
    let s = load(&dir, &LoadOptions::all(1000)).unwrap();
    assert!(s.unstaged.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_reads_commit_template() {
    let dir = scratch_repo("load-template");
    assert_eq!(load(&dir, &LoadOptions::all(10)).unwrap().commit_template, None);
    write(&dir, "msg.txt", "chore: \n\n# details\n");
    git(&dir, &["config", "commit.template", "msg.txt"]);
    let t = load(&dir, &LoadOptions::all(10)).unwrap().commit_template.expect("template loaded");
    assert_eq!((t.summary.as_str(), t.description.as_str(), t.comment_char), ("chore:", "# details", '#'));
}
