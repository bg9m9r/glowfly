//! Gitattributes read from the version shown: a commit's own files for commit diffs, the File
//! view, blame, line counts and diff tool copies; the index's for staged diffs and unstaging;
//! the working tree's for the rest. Each fixture's working tree disagrees with the version read.

use crate::repo::{
    BINARY_NOT_SHOWN, BlameOptions, DiffTarget, HunkOp, LoadDiffOptions, RevertPart, RevertRequest, apply_hunk, apply_lines, blame_file,
    blob_bytes, commit_line_counts, commit_stats, file_at, load_diff_for_view, revert, wip_line_counts,
};
use crate::testutil::{git, scratch_repo, write};
use git2::{Oid, Repository};
use std::path::Path;

fn commit(dir: &Path, message: &str) -> Oid {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn binary(dir: &Path, target: DiffTarget) -> bool {
    load_diff_for_view(dir, &target, LoadDiffOptions::default()).expect("loads").diff.binary
}

fn language(dir: &Path, target: DiffTarget) -> Option<String> {
    load_diff_for_view(dir, &target, LoadDiffOptions::default()).expect("loads").language
}

#[test]
fn commit_diffs_and_counts_take_binary_from_the_commit() {
    let dir = scratch_repo("attr-source-commit-binary");
    write(&dir, ".gitattributes", "*.dat -diff\n");
    write(&dir, "a.dat", "one\n");
    commit(&dir, "one");
    write(&dir, "a.dat", "two\n");
    let c2 = commit(&dir, "two");
    std::fs::remove_file(dir.join(".gitattributes")).unwrap();
    write(&dir, "a.dat", "three\n");
    let c3 = commit(&dir, "three");

    // The working tree says -diff: only the commit that has it shows the file as binary.
    write(&dir, ".gitattributes", "*.dat -diff\n");
    assert!(binary(&dir, DiffTarget::Commit(c2, "a.dat".into())));
    assert!(!binary(&dir, DiffTarget::Commit(c3, "a.dat".into())));
    assert!(!binary(&dir, DiffTarget::Between(c2, c3, "a.dat".into())), "the newer end's attributes");
    assert!(commit_line_counts(&dir, c2).unwrap().files.is_empty(), "binary files aren't counted");
    assert!(commit_line_counts(&dir, c3).unwrap().files.contains_key("a.dat"));
    let stats = commit_stats(&dir, &[c2, c3]);
    assert_eq!(stats, vec![(c2, 1, 0, 0), (c3, 2, 1, 2)]);

    // The working tree says diff (text): the commit's -diff still wins.
    write(&dir, ".gitattributes", "*.dat diff\n");
    assert!(binary(&dir, DiffTarget::Commit(c2, "a.dat".into())));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_diffs_read_the_index_and_unstaged_ones_the_working_tree() {
    let dir = scratch_repo("attr-source-staged");
    write(&dir, "a.dat", "one\n");
    write(&dir, "b.inc", "one\n");
    commit(&dir, "one");
    write(&dir, ".gitattributes", "*.dat -diff\n*.inc linguist-language=Python\n");
    write(&dir, "a.dat", "two\n");
    write(&dir, "b.inc", "two\n");
    git(&dir, &["add", "-A"]);
    // libgit2 reads the index's file under the working tree's (git reads only one), so the
    // working tree's says text outright.
    write(&dir, ".gitattributes", "*.dat diff\n*.inc linguist-language=Rust\n");
    write(&dir, "a.dat", "three\n");
    write(&dir, "b.inc", "three\n");

    assert!(binary(&dir, DiffTarget::Staged("a.dat".into())));
    assert!(!binary(&dir, DiffTarget::Unstaged("a.dat".into())));
    assert_eq!(language(&dir, DiffTarget::Staged("b.inc".into())).as_deref(), Some("Python"));
    assert_eq!(language(&dir, DiffTarget::Unstaged("b.inc".into())).as_deref(), Some("Rust"));
    let counts = wip_line_counts(&dir).unwrap();
    assert!(!counts.staged.files.contains_key("a.dat") && counts.staged.files.contains_key("b.inc"));
    assert!(counts.unstaged.files.contains_key("a.dat"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_staged_text_file_unstages_by_hunk_when_the_working_tree_says_binary() {
    let dir = scratch_repo("attr-source-unstage");
    write(&dir, "a.dat", "one\n");
    commit(&dir, "one");
    write(&dir, "a.dat", "two\n");
    git(&dir, &["add", "a.dat"]);
    write(&dir, ".gitattributes", "*.dat -diff\n");

    let shown = load_diff_for_view(&dir, &DiffTarget::Staged("a.dat".into()), LoadDiffOptions::default()).unwrap().diff;
    assert!(!shown.binary && shown.hunks.len() == 1, "{shown:?}");
    apply_hunk(&dir, "a.dat", 0, &shown.hunks[0], HunkOp::Unstage).expect("the hunk shown unstages");
    let err = load_diff_for_view(&dir, &DiffTarget::Staged("a.dat".into()), LoadDiffOptions::default()).unwrap_err();
    assert!(err.is_not_found(), "nothing staged now: {err}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The index's content of `path`.
fn staged_text(dir: &Path, path: &str) -> String {
    let repo = Repository::open(dir).unwrap();
    let id = repo.index().unwrap().get_path(Path::new(path), 0).unwrap().id;
    String::from_utf8(repo.find_blob(id).unwrap().content().to_vec()).unwrap()
}

#[test]
fn a_staged_text_file_unstages_by_line_when_the_working_tree_says_binary() {
    let dir = scratch_repo("attr-source-unstage-lines");
    write(
        &dir, "a.dat", "one
two
",
    );
    commit(&dir, "one");
    write(
        &dir, "a.dat", "ONE
TWO
",
    );
    git(&dir, &["add", "a.dat"]);
    write(
        &dir,
        ".gitattributes",
        "*.dat -diff
",
    );

    let shown = load_diff_for_view(&dir, &DiffTarget::Staged("a.dat".into()), LoadDiffOptions::default()).unwrap().diff;
    assert!(!shown.binary && shown.hunks.len() == 1, "{shown:?}");
    let hunk = &shown.hunks[0];
    let pick = |text: &str| hunk.lines.iter().position(|l| l.text == text).unwrap();
    // The first line's change only: "-one" and "+ONE".
    let lines = [pick("one"), pick("ONE")];
    apply_lines(&dir, "a.dat", 0, hunk, &lines, HunkOp::Unstage).expect("the lines shown unstage");
    assert_eq!(
        staged_text(&dir, "a.dat"),
        "one
TWO
"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("a.dat")).unwrap(),
        "ONE
TWO
",
        "the working file is untouched"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_commits_text_hunk_reverts_when_the_working_tree_says_binary() {
    let dir = scratch_repo("attr-source-revert");
    let lines: Vec<String> = (1..=20).map(|i| format!("line {i}")).collect();
    let text = |ls: &[String]| {
        ls.iter()
            .map(|l| {
                format!(
                    "{l}
"
                )
            })
            .collect::<String>()
    };
    write(&dir, "a.dat", &text(&lines));
    commit(&dir, "one");
    let mut changed = lines.clone();
    changed[0] = "first".into();
    changed[19] = "last".into();
    write(&dir, "a.dat", &text(&changed));
    let c2 = commit(&dir, "two");
    write(
        &dir,
        ".gitattributes",
        "*.dat -diff
",
    );

    let target = DiffTarget::Commit(c2, "a.dat".into());
    let shown = load_diff_for_view(&dir, &target, LoadDiffOptions::default()).unwrap().diff;
    assert!(!shown.binary && shown.hunks.len() == 2, "{shown:?}");
    let part = RevertPart::Hunk { index: 0, expected: shown.hunks[0].clone() };
    revert(&dir, &RevertRequest { target, part, dest: None, ignore_whitespace: false }).expect("the hunk shown reverts");
    let mut want = changed.clone();
    want[0] = "line 1".into();
    assert_eq!(std::fs::read_to_string(dir.join("a.dat")).unwrap(), text(&want), "only the first hunk reverted");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_file_view_and_blame_read_the_commits_gitattributes() {
    let dir = scratch_repo("attr-source-file-view");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    write(&dir, ".gitattributes", "[attr]opaque -diff\n*.dat -diff\n*.bin opaque\n*.inc linguist-language=PHP\n");
    write(&dir, "sub/.gitattributes", "*.inc linguist-language=Python\n");
    for f in ["a.dat", "x.bin", "b.inc", "sub/c.inc"] {
        write(&dir, f, "one\n");
    }
    let c1 = commit(&dir, "one");
    std::fs::remove_file(dir.join(".gitattributes")).unwrap();
    std::fs::remove_file(dir.join("sub/.gitattributes")).unwrap();
    for f in ["a.dat", "x.bin", "b.inc", "sub/c.inc"] {
        write(&dir, f, "two\n");
    }
    let c2 = commit(&dir, "two");
    write(&dir, ".gitattributes", "*.dat -diff\n*.inc linguist-language=Rust\n");

    let refused = |oid, path: &str| file_at(&dir, oid, path).unwrap_err().to_string();
    assert!(refused(c1, "a.dat").contains(BINARY_NOT_SHOWN));
    assert!(refused(c1, "x.bin").contains(BINARY_NOT_SHOWN), "a macro from the commit's root file");
    assert!(file_at(&dir, c2, "a.dat").is_ok(), "the commit has no -diff");
    assert_eq!(file_at(&dir, c1, "b.inc").unwrap().language.as_deref(), Some("PHP"));
    assert_eq!(file_at(&dir, c1, "sub/c.inc").unwrap().language.as_deref(), Some("Python"), "a folder's own file");
    assert_eq!(file_at(&dir, c2, "b.inc").unwrap().language, None);

    let blame = |oid, path: &str| blame_file(&dir, path, Some(oid), &BlameOptions::default());
    assert!(blame(c1, "a.dat").unwrap_err().to_string().contains("binary"));
    assert!(blame(c2, "a.dat").is_ok());
    assert_eq!(blame(c1, "sub/c.inc").unwrap().language.as_deref(), Some("Python"));
    assert_eq!(blame(c2, "b.inc").unwrap().language, None);

    // `.git/info/attributes` wins over the commit's files.
    std::fs::create_dir_all(dir.join(".git/info")).unwrap();
    write(&dir, ".git/info/attributes", "b.inc linguist-language=Go\na.dat diff\n");
    assert_eq!(file_at(&dir, c1, "b.inc").unwrap().language.as_deref(), Some("Go"));
    assert!(file_at(&dir, c1, "a.dat").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn diff_tool_copies_follow_each_commits_line_endings() {
    let dir = scratch_repo("attr-source-tool-eol");
    if crate::gitcli::version(&dir).is_none_or(|v| (v.0, v.1) < (2, 40)) {
        eprintln!("skipped: GIT_ATTR_SOURCE needs git 2.40");
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    write(&dir, ".gitattributes", "*.txt text eol=crlf\n");
    write(&dir, "a.txt", "one\n");
    let c1 = commit(&dir, "one");
    write(&dir, ".gitattributes", "*.txt text eol=lf\n");
    write(&dir, "a.txt", "one\ntwo\n");
    let c2 = commit(&dir, "two");
    write(&dir, ".gitattributes", "*.txt -text\n");
    let id = Repository::open(&dir).unwrap().find_commit(c1).unwrap().tree().unwrap().get_path(Path::new("a.txt")).unwrap().id();
    assert_eq!(blob_bytes(&dir, id, "a.txt", true, Some(c1)).unwrap(), b"one\r\n", "the commit's eol=crlf");
    assert_eq!(blob_bytes(&dir, id, "a.txt", true, None).unwrap(), b"one\n", "the working tree's -text");

    let root = dir.join(".git").join("tool-copies");
    std::fs::create_dir_all(&root).unwrap();
    let files = crate::external_tools::compare::diff_files(&root, &dir, &DiffTarget::Commit(c2, "a.txt".into()), true).unwrap();
    assert_eq!(std::fs::read(&files.local).unwrap(), b"one\r\n", "the parent's copy");
    assert_eq!(std::fs::read(&files.remote).unwrap(), b"one\ntwo\n", "the commit's copy");
    let _ = files.session.remove();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_linked_worktree_reads_the_commits_gitattributes() {
    let dir = scratch_repo("attr-source-worktree");
    write(&dir, ".gitattributes", "*.inc linguist-language=PHP\n");
    write(&dir, "a.inc", "one\n");
    commit(&dir, "one");
    write(&dir, "a.inc", "two\n");
    let c2 = commit(&dir, "two");
    let wt = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&dir, &["worktree", "add", "-q", "--detach", &wt.to_string_lossy()]);
    write(&wt, ".gitattributes", "*.inc linguist-language=Rust\n");
    assert_eq!(language(&wt, DiffTarget::Commit(c2, "a.inc".into())).as_deref(), Some("PHP"));
    write(&wt, "a.inc", "three\n");
    assert_eq!(language(&wt, DiffTarget::Unstaged("a.inc".into())).as_deref(), Some("Rust"));
    let _ = std::fs::remove_dir_all(&wt);
    let _ = std::fs::remove_dir_all(&dir);
}
