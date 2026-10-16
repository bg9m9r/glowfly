use super::*;
use crate::testutil::{git, scratch_repo, write};
use std::path::PathBuf;

fn head(dir: &Path) -> Oid {
    git2::Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

/// Commits everything in `dir` with message `msg` and returns the commit.
fn commit(dir: &Path, msg: &str) -> Oid {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", msg]);
    head(dir)
}

/// A file next to the fixture `dir` (outside its working tree).
fn beside(dir: &Path, name: &str) -> PathBuf {
    dir.with_file_name(format!("{}-{name}", dir.file_name().unwrap().to_string_lossy()))
}

/// `git show <oid>:<path>` as bytes.
fn blob(dir: &Path, oid: Oid, path: &str) -> Vec<u8> {
    let repo = git2::Repository::open(dir).unwrap();
    let tree = repo.find_commit(oid).unwrap().tree().unwrap();
    let entry = tree.get_path(Path::new(path)).unwrap();
    repo.find_blob(entry.id()).unwrap().content().to_vec()
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn file_name_slugs() {
    assert_eq!(file_name("Fix: the B/line (quoted)", 1), "Fix-the-B-line-quoted.patch");
    assert_eq!(file_name("  --Add  \"search\" -- & filters!!  ", 1), "Add-search-filters.patch");
    assert_eq!(file_name("Ünïcödé 日本語 snake_case", 1), "Ünïcödé-日本語-snake_case.patch");
    assert_eq!(file_name("v1.2.3. release...", 1), "v1-2-3-release.patch");
    assert_eq!(file_name("", 1), "commit.patch");
    assert_eq!(file_name("!!! ... ---", 1), "commit.patch");
    assert_eq!(file_name("CON", 1), "CON-commit.patch");
    assert_eq!(file_name("nul", 1), "nul-commit.patch");
    assert_eq!(file_name("Com7", 1), "Com7-commit.patch");
    assert_eq!(file_name("lpt9", 2), "lpt9-commit-and-1-more.patch");
    assert_eq!(file_name("CONsole", 1), "CONsole.patch");
    assert_eq!(file_name("COM10", 1), "COM10.patch");
    assert_eq!(file_name("COM\u{b3}", 1), "COM\u{b3}-commit.patch");
    assert_eq!(file_name("lpt0", 1), "lpt0-commit.patch");
    assert_eq!(file_name("Add search", 3), "Add-search-and-2-more.patch");
}

#[test]
fn file_name_cuts_long_subjects_on_a_char_boundary() {
    // 49 letters then a space: the cut at 50 would end on the dash, which is trimmed.
    let subject = format!("{} tail of the subject", "é".repeat(49));
    assert_eq!(file_name(&subject, 1), format!("{}.patch", "é".repeat(49)));
    let long = "a".repeat(80);
    assert_eq!(file_name(&long, 1), format!("{}.patch", "a".repeat(50)));
    let mixed = "日本語のコミットメッセージ ".repeat(6);
    let name = file_name(&mixed, 1);
    let slug = name.strip_suffix(".patch").unwrap();
    assert!(slug.chars().count() <= 50 && !slug.ends_with('-'), "{name}");
}

#[test]
fn format_patch_cmd_shape() {
    let oid = Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap();
    assert_eq!(format_patch_cmd(oid), ["format-patch", "-1", "--stdout", "--no-color", "0123456789abcdef0123456789abcdef01234567"]);
}

#[test]
fn apply_cmds_shape() {
    assert_eq!(apply_cmds("C:/p/x y.patch"), [args(&["apply", "--check", "C:/p/x y.patch"]), args(&["apply", "C:/p/x y.patch"])]);
}

#[test]
fn export_round_trip_is_byte_exact() {
    let dir = scratch_repo("patch-export");
    write(&dir, "a.txt", "one\n");
    let c1 = commit(&dir, "base");
    std::fs::write(dir.join("a.txt"), b"caf\xe9\n").unwrap();
    std::fs::write(dir.join("bin.dat"), b"\x00\x01\x02\x00binary\x00\xff").unwrap();
    let c2 = commit(&dir, "Latin-1 and binary");
    std::fs::write(dir.join("a.txt"), b"caf\xe9\nna\xefve\n").unwrap();
    let c3 = commit(&dir, "More Latin-1");
    let dest = beside(&dir, "out.patch");
    let _ = std::fs::remove_file(&dest);

    let report = export(&dir, &[c2, c3], &dest).unwrap();
    assert_eq!(report, ExportReport { written: 2, skipped: Vec::new() });
    let bytes = std::fs::read(&dest).unwrap();
    assert!(bytes.starts_with(format!("From {c2} ").as_bytes()));
    let second = format!("\nFrom {c3} ");
    assert!(bytes.windows(second.len()).any(|w| w == second.as_bytes()), "the second commit follows");

    git(&dir, &["reset", "-q", "--hard", &c1.to_string()]);
    git(&dir, &["apply", &dest.display().to_string()]);
    assert_eq!(std::fs::read(dir.join("a.txt")).unwrap(), blob(&dir, c3, "a.txt"));
    assert_eq!(std::fs::read(dir.join("bin.dat")).unwrap(), blob(&dir, c3, "bin.dat"));
    let _ = std::fs::remove_file(&dest);
    cleanup(&dir);
}

#[test]
fn export_leaves_out_empty_commits() {
    let dir = scratch_repo("patch-empty");
    write(&dir, "a.txt", "one\n");
    commit(&dir, "base");
    write(&dir, "a.txt", "two\n");
    let change = commit(&dir, "change");
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "nothing"]);
    let empty = head(&dir);
    let dest = beside(&dir, "out.patch");
    let _ = std::fs::remove_file(&dest);

    let report = export(&dir, &[change, empty], &dest).unwrap();
    assert_eq!(report, ExportReport { written: 1, skipped: vec![empty] });
    std::fs::remove_file(&dest).unwrap();

    let err = export(&dir, &[empty], &dest).unwrap_err();
    assert_eq!(err.to_string(), NOTHING_TO_SAVE);
    assert!(!dest.exists(), "nothing is written");
    cleanup(&dir);
}

#[test]
fn export_fails_on_bad_destination() {
    let dir = scratch_repo("patch-bad-dest");
    write(&dir, "a.txt", "one\n");
    let c1 = commit(&dir, "base");
    let blocker = beside(&dir, "file");
    std::fs::write(&blocker, "not a folder").unwrap();
    let dest = blocker.join("out.patch");

    let err = export(&dir, &[c1], &dest).unwrap_err();
    assert!(matches!(err, Error::Io(_)), "{err:?}");
    let parent = blocker.parent().unwrap();
    let prefix = format!(".{}", "out.patch");
    let leftovers: Vec<_> =
        std::fs::read_dir(parent).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with(&prefix)).collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    let _ = std::fs::remove_file(&blocker);
    cleanup(&dir);
}

/// What `git apply --check` (the first of `apply_cmds`) says about `patch` in `dir`.
fn check(dir: &Path, patch: &Path) -> Vec<ApplyProblem> {
    let out = gitcli::output(dir, &apply_cmds(&patch.display().to_string())[0], "", &[], None);
    assert!(!out.ok, "the check should fail: {out:?}");
    apply_problems(&out.stderr)
}

/// Saves commit `oid` of `dir` as a patch next to it.
fn patch_of(dir: &Path, oid: Oid, name: &str) -> PathBuf {
    let dest = beside(dir, name);
    export(dir, &[oid], &dest).unwrap();
    dest
}

#[test]
fn apply_problems_from_real_git() {
    let dir = scratch_repo("patch-apply-errors");
    write(&dir, "a.txt", "one\n");
    let c1 = commit(&dir, "base");
    write(&dir, "a.txt", "one\ntwo\n");
    let edit = patch_of(&dir, commit(&dir, "edit a"), "edit.patch");
    git(&dir, &["reset", "-q", "--hard", &c1.to_string()]);
    write(&dir, "n.txt", "new\n");
    let create = patch_of(&dir, commit(&dir, "add n"), "create.patch");
    git(&dir, &["reset", "-q", "--hard", &c1.to_string()]);

    // (1) The file changed since the patch was made.
    write(&dir, "a.txt", "uno\n");
    assert_eq!(check(&dir, &edit), [ApplyProblem::Hunk { file: "a.txt".into(), line: 1 }]);
    // (2) The file is gone.
    std::fs::remove_file(dir.join("a.txt")).unwrap();
    assert_eq!(check(&dir, &edit), [ApplyProblem::Missing { file: "a.txt".into() }]);
    git(&dir, &["checkout", "--", "a.txt"]);
    // (3) The file the patch creates is already there.
    write(&dir, "n.txt", "mine\n");
    assert_eq!(check(&dir, &create), [ApplyProblem::Exists { file: "n.txt".into() }]);
    // (4) Not a patch at all.
    let empty = beside(&dir, "empty.patch");
    std::fs::write(&empty, "").unwrap();
    assert_eq!(check(&dir, &empty), [ApplyProblem::NoPatch]);
    let garbage = beside(&dir, "garbage.patch");
    std::fs::write(&garbage, "garbage\n").unwrap();
    assert_eq!(check(&dir, &garbage), [ApplyProblem::NoPatch]);

    for p in [edit, create, empty, garbage] {
        let _ = std::fs::remove_file(p);
    }
    cleanup(&dir);
}

#[test]
fn apply_problems_parses_job_output() {
    let output = "$ git apply --check C:/x/p.patch\n\
        warning: p.patch has whitespace errors.\n\
        error: patch failed: src/a b.txt:12\n\
        error: src/a b.txt: patch does not apply\n\
        error: b.txt: No such file or directory\n\
        error: patch failed: src/a b.txt:12\n\
        error: c.txt: patch does not apply\n\
        error: corrupt patch at line 7\n\
        error: something new\n\
        $ git apply C:/x/p.patch";
    let problems = apply_problems(output);
    assert_eq!(
        problems,
        [
            ApplyProblem::Hunk { file: "src/a b.txt".into(), line: 12 },
            ApplyProblem::Missing { file: "b.txt".into() },
            ApplyProblem::Mismatch { file: "c.txt".into() },
            ApplyProblem::Corrupt { line: 7 },
            ApplyProblem::Other("something new".into()),
        ]
    );
    assert_eq!(
        failure_summary(output).as_deref(),
        Some("src/a b.txt has changed where the hunk at line 12 goes (and 4 more; see the log)")
    );
    assert_eq!(failure_summary("error: c.txt: already exists in working directory").as_deref(), Some("c.txt already exists"));
    assert_eq!(
        failure_summary("error: No valid patches in input (allow with \"--allow-empty\")").as_deref(),
        Some("the file holds no changes git can apply")
    );
    assert_eq!(failure_summary("$ git apply --check p\nwarning: something"), None);
    assert_eq!(failure_summary(""), None);
}
