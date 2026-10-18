//! Blame's origins (stepping back past a commit), its options and commit details.

use super::*;
use crate::testutil::*;
use std::path::Path;

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn commit(dir: &Path, file: &str, text: &str, message: &str) -> Oid {
    write(dir, file, text);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    head(dir)
}

fn blame(dir: &Path, path: &str, at: Option<Oid>, opts: &BlameOptions) -> Blame {
    blame_file(dir, path, at, opts).unwrap()
}

fn line_commit(b: &Blame, line: usize) -> Option<Oid> {
    b.commit_of_line(line).map(|c| c.oid)
}

/// Lines long enough (over 20 letters and digits) for git to follow them when moved.
fn long_block(tag: &str) -> String {
    (1..=3).map(|i| format!("{tag} line {i} of the block that moves around the repository\n")).collect()
}

#[test]
fn blame_records_the_parent_to_step_back_to() {
    let dir = scratch_repo("blame-previous");
    let c1 = commit(&dir, "a.txt", "1\n2\n3\n", "add");
    let c2 = commit(&dir, "a.txt", "1\ntwo\n3\n", "edit two");
    let b = blame(&dir, "a.txt", None, &BlameOptions::default());
    assert_eq!(line_commit(&b, 1), Some(c2));
    let edited = b.origin_of_line(1).unwrap();
    assert_eq!(edited.path, "a.txt");
    assert_eq!(edited.previous, Some((c1, "a.txt".to_owned())));
    let first = b.origin_of_line(0).unwrap();
    assert_eq!(b.commits[first.commit].oid, c1);
    assert_eq!(first.previous, None, "c1 added the file");
    assert_eq!(b.orig_lines, [0, 1, 2]);

    let before = blame(&dir, "a.txt", Some(c1), &BlameOptions::default());
    assert_eq!(line_commit(&before, 1), Some(c1));
    assert_eq!(before.lines, ["1", "2", "3"]);
    assert_eq!(before.orig_lines, [0, 1, 2]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blame_parent_follows_a_rename() {
    let dir = scratch_repo("blame-rename");
    let text = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\n";
    let c1 = commit(&dir, "old.txt", text, "add old");
    git(&dir, &["mv", "old.txt", "new.txt"]);
    let c2 = commit(&dir, "new.txt", &text.replace("four", "FOUR"), "rename and edit");
    let b = blame(&dir, "new.txt", None, &BlameOptions::default());
    assert_eq!(line_commit(&b, 3), Some(c2));
    let edited = b.origin_of_line(3).unwrap();
    assert_eq!(edited.path, "new.txt");
    assert_eq!(edited.previous, Some((c1, "old.txt".to_owned())));
    let kept = b.origin_of_line(0).unwrap();
    assert_eq!((b.commits[kept.commit].oid, kept.path.as_str()), (c1, "old.txt"));
    let before = blame(&dir, "old.txt", Some(c1), &BlameOptions::default());
    assert_eq!(before.lines.len(), 8);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ignore_rev_skips_a_chosen_commit() {
    let dir = scratch_repo("blame-ignore-rev");
    let c1 = commit(&dir, "a.txt", "alpha\nbeta\ngamma\n", "add");
    let c2 = commit(&dir, "a.txt", "alpha\nbeta;\ngamma\n", "reformat beta");
    let c3 = commit(&dir, "a.txt", "alpha\nbeta;\ngamma;\n", "reformat gamma");
    let skip = BlameOptions { ignore_revs: vec![c2], ..BlameOptions::default() };
    assert_eq!(line_commit(&blame(&dir, "a.txt", None, &BlameOptions::default()), 1), Some(c2));
    assert_eq!(line_commit(&blame(&dir, "a.txt", None, &skip), 1), Some(c1));

    // With a configured file that skips c3 as well.
    write(&dir, ".git-blame-ignore-revs", &format!("{c3}\n"));
    set_config(&dir, &[("blame.ignoreRevsFile", ".git-blame-ignore-revs")]);
    let b = blame(&dir, "a.txt", None, &skip);
    assert_eq!((line_commit(&b, 1), line_commit(&b, 2)), (Some(c1), Some(c1)));

    // With a configured file that is missing (`--no-ignore-revs-file` comes first).
    set_config(&dir, &[("blame.ignoreRevsFile", "does-not-exist")]);
    let b = blame(&dir, "a.txt", None, &skip);
    assert_eq!((line_commit(&b, 1), line_commit(&b, 2)), (Some(c1), Some(c3)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ignore_whitespace_option() {
    let dir = scratch_repo("blame-whitespace");
    let c1 = commit(&dir, "a.txt", "fn a() {\nx = 1;\n}\n", "add");
    let c2 = commit(&dir, "a.txt", "fn a() {\n    x = 1;\n}\n", "indent");
    assert_eq!(line_commit(&blame(&dir, "a.txt", None, &BlameOptions::default()), 1), Some(c2));
    let w = BlameOptions { ignore_whitespace: true, ..BlameOptions::default() };
    assert_eq!(line_commit(&blame(&dir, "a.txt", None, &w), 1), Some(c1));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn moves_within_the_file() {
    let dir = scratch_repo("blame-moves");
    let rest = "f1\nf2\nf3\nf4\nf5\nf6\nf7\n";
    let block = long_block("moved");
    let c1 = commit(&dir, "a.txt", &format!("{block}{rest}"), "add");
    let c2 = commit(&dir, "a.txt", &format!("{rest}{block}"), "move the block down");
    let off = blame(&dir, "a.txt", None, &BlameOptions::default());
    assert_eq!(off.lines[7], "moved line 1 of the block that moves around the repository");
    assert!((7..10).all(|l| line_commit(&off, l) == Some(c2)), "{:?}", off.runs);
    let m = BlameOptions { moves: BlameMoves::WithinFile, ..BlameOptions::default() };
    let within = blame(&dir, "a.txt", None, &m);
    assert!((7..10).all(|l| line_commit(&within, l) == Some(c1)), "{:?}", within.runs);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn copies_across_files() {
    let dir = scratch_repo("blame-copies");
    let block = long_block("copied");
    let c1 = commit(&dir, "a.txt", &format!("a1\na2\n{block}a3\n"), "add a");
    write(&dir, "a.txt", "a1\na2\na3\n");
    let c2 = commit(&dir, "b.txt", &format!("b1\n{block}b2\n"), "move the block to b");
    let off = blame(&dir, "b.txt", None, &BlameOptions::default());
    assert!((0..5).all(|l| line_commit(&off, l) == Some(c2)), "{:?}", off.runs);
    let c = BlameOptions { moves: BlameMoves::AcrossFiles, ..BlameOptions::default() };
    let across = blame(&dir, "b.txt", None, &c);
    for l in 1..4 {
        assert_eq!(line_commit(&across, l), Some(c1), "line {l}: {:?}", across.runs);
        assert_eq!(across.origin_of_line(l).unwrap().path, "a.txt");
    }
    assert_eq!(line_commit(&across, 0), Some(c2));
    assert_eq!(across.origin_of_line(0).unwrap().path, "b.txt");
    assert_ne!(across.run_of(0), across.run_of(1), "the copied lines are their own run");
    assert_eq!(across.orig_lines[1], 2, "line 3 of a.txt in c1");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commit_body_is_read() {
    let dir = scratch_repo("blame-body");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "subject", "-m", "line one\nline two"]);
    commit(&dir, "a.txt", "one\ntwo\n", "one line only");
    let b = blame(&dir, "a.txt", None, &BlameOptions::default());
    assert_eq!(b.commit_of_line(0).unwrap().summary, "subject");
    assert_eq!(b.commit_of_line(0).unwrap().body, "line one\nline two");
    assert_eq!(b.commit_of_line(1).unwrap().body, "");
    let _ = std::fs::remove_dir_all(&dir);
}

fn literal(times: &[i64]) -> Blame {
    let commit = |i: usize, time: i64| BlameCommit {
        oid: Oid::from_str(&format!("{:040x}", i + 1)).unwrap(),
        author: String::new(),
        email: String::new(),
        time,
        summary: String::new(),
        body: String::new(),
    };
    Blame {
        path: "a.txt".into(),
        at: Oid::zero(),
        commits: times.iter().enumerate().map(|(i, t)| commit(i, *t)).collect(),
        lines: Vec::new(),
        runs: Vec::new(),
        origins: Vec::new(),
        orig_lines: Vec::new(),
        language: None,
    }
}

#[test]
fn ages_rank_commits_by_time() {
    assert_eq!(literal(&[100, 300, 200, 300]).ages(), [0.0, 1.0, 0.5, 1.0]);
    assert_eq!(literal(&[100]).ages(), [1.0]);
    assert_eq!(literal(&[5, 5]).ages(), [1.0, 1.0], "one distinct time");
    let mut b = literal(&[100, 200, 50]);
    b.commits[2].oid = Oid::zero();
    assert_eq!(b.ages(), [0.0, 1.0, 1.0], "not committed yet counts as newest");
    assert!(literal(&[]).ages().is_empty());
}
