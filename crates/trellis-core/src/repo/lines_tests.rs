//! Line staging against real repositories: exact index and working-tree bytes after staging,
//! unstaging or discarding some of a hunk's lines.

use super::*;
use crate::testutil::*;
use git2::Repository;
use std::path::Path;

const TEN: &str = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";
/// `5` replaced by `five`, and `6a`, `6b` inserted after `6`: one hunk.
const EDITED: &str = "1\n2\n3\n4\nfive\n6\n6a\n6b\n7\n8\n9\n10\n";

fn index_bytes(dir: &Path, path: &str) -> Option<Vec<u8>> {
    let repo = Repository::open(dir).unwrap();
    let entry = repo.index().unwrap().get_path(Path::new(path), 0)?;
    Some(repo.find_blob(entry.id).unwrap().content().to_vec())
}

fn index_text(dir: &Path, path: &str) -> String {
    String::from_utf8(index_bytes(dir, path).expect("in the index")).unwrap()
}

fn read(dir: &Path, path: &str) -> Vec<u8> {
    std::fs::read(dir.join(path)).unwrap()
}

/// Hunk 0 as displayed and the indexes of its lines with these (origin, text) pairs.
fn pick(dir: &Path, target: &DiffTarget, want: &[(char, &str)]) -> (Hunk, Vec<usize>) {
    let d = load_diff(dir, target).unwrap();
    let h = &d.hunks[0];
    let idx = want
        .iter()
        .map(|(o, t)| h.lines.iter().position(|l| l.origin == *o && l.text == *t).unwrap_or_else(|| panic!("no {o}{t} line")))
        .collect();
    (h.clone(), idx)
}

fn apply(dir: &Path, path: &str, op: HunkOp, want: &[(char, &str)]) {
    let target = match op {
        HunkOp::Unstage => DiffTarget::Staged(path.into()),
        _ => DiffTarget::Unstaged(path.into()),
    };
    let (hunk, lines) = pick(dir, &target, want);
    apply_lines(dir, path, 0, &hunk, &lines, op).unwrap();
}

/// A repository with `f.txt` committed as `TEN` and edited to `EDITED`.
fn ten_lines(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "f.txt", TEN);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);
    write(&dir, "f.txt", EDITED);
    dir
}

#[test]
fn stage_some_lines() {
    let dir = ten_lines("lines-stage");
    apply(&dir, "f.txt", HunkOp::Stage, &[('+', "6a")]);
    assert_eq!(index_text(&dir, "f.txt"), "1\n2\n3\n4\n5\n6\n6a\n7\n8\n9\n10\n");
    assert_eq!(read(&dir, "f.txt"), EDITED.as_bytes());
    let rest = load_diff(&dir, &DiffTarget::Unstaged("f.txt".into())).unwrap();
    let changed: Vec<String> =
        rest.hunks.iter().flat_map(|h| &h.lines).filter(|l| l.origin != ' ').map(|l| format!("{}{}", l.origin, l.text)).collect();
    assert_eq!(changed, ["-5", "+five", "+6b"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stage_removed_line_only() {
    let dir = ten_lines("lines-stage-rm");
    apply(&dir, "f.txt", HunkOp::Stage, &[('-', "5")]);
    assert_eq!(index_text(&dir, "f.txt"), "1\n2\n3\n4\n6\n7\n8\n9\n10\n");
    assert_eq!(read(&dir, "f.txt"), EDITED.as_bytes());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_some_lines() {
    let dir = ten_lines("lines-unstage");
    git(&dir, &["add", "f.txt"]);
    apply(&dir, "f.txt", HunkOp::Unstage, &[('+', "6b")]);
    assert_eq!(index_text(&dir, "f.txt"), "1\n2\n3\n4\nfive\n6\n6a\n7\n8\n9\n10\n");
    assert_eq!(read(&dir, "f.txt"), EDITED.as_bytes());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_some_lines() {
    let dir = ten_lines("lines-discard");
    apply(&dir, "f.txt", HunkOp::Discard, &[('+', "6a"), ('-', "5")]);
    assert_eq!(String::from_utf8(read(&dir, "f.txt")).unwrap(), "1\n2\n3\n4\n5\nfive\n6\n6b\n7\n8\n9\n10\n");
    assert_eq!(index_text(&dir, "f.txt"), TEN);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_newline_at_eof() {
    let dir = scratch_repo("lines-eofnl");
    // (file, old, new, index after staging only `+b`, workdir after discarding only `-a`)
    let cases = [
        ("both.txt", "x\na", "x\nb", "x\na\nb", "x\na\nb"),
        ("new.txt", "x\na\n", "x\nb", "x\na\nb", "x\na\nb"),
        ("old.txt", "x\na", "x\nb\n", "x\na\nb\n", "x\na\nb\n"),
    ];
    for (f, old, _, _, _) in cases {
        write(&dir, f, old);
    }
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);
    for (f, old, new, staged_b, discarded_a) in cases {
        write(&dir, f, new);
        apply(&dir, f, HunkOp::Stage, &[('+', "b")]);
        assert_eq!(index_text(&dir, f), staged_b, "{f}: stage +b");
        git(&dir, &["reset", "-q", "--", f]);
        apply(&dir, f, HunkOp::Stage, &[('-', "a")]);
        assert_eq!(index_text(&dir, f), "x\n", "{f}: stage -a");
        git(&dir, &["reset", "-q", "--", f]);
        assert_eq!(index_text(&dir, f), old);
        apply(&dir, f, HunkOp::Discard, &[('-', "a")]);
        assert_eq!(String::from_utf8(read(&dir, f)).unwrap(), discarded_a, "{f}: discard -a");
        assert_eq!(index_text(&dir, f), old);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn untracked_partial_stage() {
    let dir = scratch_repo("lines-untracked");
    write(&dir, "base.txt", "base\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);
    write(&dir, "u.txt", "a\nb\nc\n");
    let target = DiffTarget::Unstaged("u.txt".into());
    let hunk = load_diff(&dir, &target).unwrap().hunks[0].clone();
    apply_lines(&dir, "u.txt", 0, &hunk, &[0, 2], HunkOp::Stage).unwrap();
    assert_eq!(index_text(&dir, "u.txt"), "a\nc\n");
    assert_eq!(read(&dir, "u.txt"), b"a\nb\nc\n");
    let (_, staged, _) = status_at(&dir).unwrap();
    assert!(staged.iter().any(|f| f.path == "u.txt" && f.status == 'A'));

    // Discarding lines of an untracked file edits it in place.
    write(&dir, "v.txt", "a\nb\nc\n");
    apply(&dir, "v.txt", HunkOp::Discard, &[('+', "b")]);
    assert_eq!(read(&dir, "v.txt"), b"a\nc\n");
    assert_eq!(index_bytes(&dir, "v.txt"), None, "still untracked");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deleted_file_partial() {
    let dir = scratch_repo("lines-deleted");
    write(&dir, "d.txt", "a\nb\nc\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);
    std::fs::remove_file(dir.join("d.txt")).unwrap();
    apply(&dir, "d.txt", HunkOp::Stage, &[('-', "a"), ('-', "c")]);
    assert_eq!(index_text(&dir, "d.txt"), "b\n");
    assert!(!dir.join("d.txt").exists());

    git(&dir, &["reset", "-q", "--", "d.txt"]);
    apply(&dir, "d.txt", HunkOp::Discard, &[('-', "b"), ('-', "c")]);
    assert_eq!(read(&dir, "d.txt"), b"b\nc\n");
    assert_eq!(index_text(&dir, "d.txt"), "a\nb\nc\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn crlf_file_keeps_crlf() {
    let dir = scratch_repo("lines-crlf");
    write(&dir, "w.txt", "a\r\nb\r\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);
    write(&dir, "w.txt", "a\r\nnew\r\nb\r\nmore\r\n");
    apply(&dir, "w.txt", HunkOp::Stage, &[('+', "more")]);
    assert_eq!(index_bytes(&dir, "w.txt").unwrap(), b"a\r\nb\r\nmore\r\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn path_with_space_and_non_ascii() {
    let dir = scratch_repo("lines-paths");
    for f in ["my file.txt", "café.txt"] {
        write(&dir, f, "a\n");
    }
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);
    for f in ["my file.txt", "café.txt"] {
        write(&dir, f, "a\nb\nc\n");
        apply(&dir, f, HunkOp::Stage, &[('+', "c")]);
        assert_eq!(index_text(&dir, f), "a\nc\n", "{f}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keeps_the_executable_bit() {
    let dir = scratch_repo("lines-mode");
    write(&dir, "run.sh", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["update-index", "--chmod=+x", "run.sh"]);
    git(&dir, &["commit", "-m", "base"]);
    write(&dir, "run.sh", "a\nb\nc\n");
    apply(&dir, "run.sh", HunkOp::Stage, &[('+', "b")]);
    let repo = Repository::open(&dir).unwrap();
    let entry = repo.index().unwrap().get_path(Path::new("run.sh"), 0).unwrap();
    assert_eq!(entry.mode, 0o100755);
    assert_eq!(index_text(&dir, "run.sh"), "a\nb\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_selection_refused() {
    let dir = ten_lines("lines-stale");
    let (hunk, plus) = pick(&dir, &DiffTarget::Unstaged("f.txt".into()), &[('+', "6a")]);
    let (_, ctx) = pick(&dir, &DiffTarget::Unstaged("f.txt".into()), &[(' ', "6")]);
    let mut other_header = hunk.clone();
    other_header.header = "@@ -1,9 +1,9 @@".into();
    let attempts = [
        apply_lines(&dir, "f.txt", 0, &other_header, &plus, HunkOp::Stage),
        apply_lines(&dir, "f.txt", 1, &hunk, &plus, HunkOp::Stage),
        apply_lines(&dir, "f.txt", 0, &hunk, &ctx, HunkOp::Stage),
        apply_lines(&dir, "f.txt", 0, &hunk, &[999], HunkOp::Discard),
    ];
    for (i, r) in attempts.into_iter().enumerate() {
        assert!(r.as_ref().is_err_and(|e| e.is_not_found()), "attempt {i}: {:?}", r.err());
    }
    assert_eq!(index_text(&dir, "f.txt"), TEN);
    assert_eq!(read(&dir, "f.txt"), EDITED.as_bytes());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn changed_lines_under_the_same_header_are_refused() {
    // The file changes after the lines were picked, keeping the hunk's shape (and so its
    // header): index 6 now holds `6x` instead of the selected `6a`. Discarding must not touch it.
    let dir = ten_lines("lines-same-header");
    let (hunk, lines) = pick(&dir, &DiffTarget::Unstaged("f.txt".into()), &[('+', "6a")]);
    let changed = EDITED.replace("6a", "6x");
    write(&dir, "f.txt", &changed);
    let now = load_diff(&dir, &DiffTarget::Unstaged("f.txt".into())).unwrap();
    assert_eq!(now.hunks[0].header, hunk.header, "same header, so a header check alone would pass");

    let r = apply_lines(&dir, "f.txt", 0, &hunk, &lines, HunkOp::Discard);
    assert!(r.as_ref().is_err_and(|e| e.is_not_found()), "{:?}", r.err());
    assert_eq!(read(&dir, "f.txt"), changed.as_bytes());
    let _ = std::fs::remove_dir_all(&dir);
}
