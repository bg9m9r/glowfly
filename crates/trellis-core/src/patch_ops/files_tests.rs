//! One file of a commit saved as a patch, a commit's patch as clipboard text, and the
//! suggested names of file and uncommitted-changes patches.

use super::*;
use crate::testutil::{git, scratch_repo, write};

fn head(dir: &Path) -> Oid {
    git2::Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn commit(dir: &Path, msg: &str) -> Oid {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", msg]);
    head(dir)
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn paths(p: &[&str]) -> Vec<String> {
    p.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn one_file_of_a_commit() {
    let dir = scratch_repo("patch-one-file");
    write(&dir, "a.txt", "one\n");
    write(&dir, "b.txt", "bee\n");
    let base = commit(&dir, "base");
    write(&dir, "a.txt", "one\ntwo\n");
    write(&dir, "b.txt", "bee\nbuzz\n");
    let both = commit(&dir, "Change both");
    write(&dir, "c.txt", "sea\n");
    let later = commit(&dir, "Add c");

    let src = PatchSource::Commits { oids: vec![both], paths: paths(&["a.txt"]) };
    let (bytes, report) = build(&dir, &src).unwrap();
    assert_eq!(report, SaveReport::Commits(ExportReport { written: 1, skipped: Vec::new() }));
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.starts_with(&format!("From {both} ")), "{text}");
    assert!(text.contains("Subject: [PATCH] Change both") && text.contains("diff --git a/a.txt b/a.txt"), "{text}");
    assert!(!text.contains("b.txt"), "only a.txt: {text}");

    // git am onto the parent makes a commit that changes only a.txt, keeping the message.
    let dest = dir.with_file_name(format!("{}-a.patch", dir.file_name().unwrap().to_string_lossy()));
    save(&dir, &src, &dest).unwrap();
    git(&dir, &["reset", "-q", "--hard", &base.to_string()]);
    git(&dir, &["am", "-q", &dest.display().to_string()]);
    let repo = git2::Repository::open(&dir).unwrap();
    let made = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(made.summary(), Some("Change both"));
    assert_eq!(made.author().name(), Some("Test"));
    let diff = repo.diff_tree_to_tree(Some(&made.parent(0).unwrap().tree().unwrap()), Some(&made.tree().unwrap()), None).unwrap();
    let changed: Vec<_> = diff.deltas().map(|d| d.new_file().path().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(changed, ["a.txt"]);

    // A file the commit doesn't change: never an older commit that does.
    let err = build(&dir, &PatchSource::Commits { oids: vec![later], paths: paths(&["a.txt"]) }).unwrap_err();
    assert_eq!(err.to_string(), FILE_UNCHANGED);
    let _ = std::fs::remove_file(dest);
    cleanup(&dir);
}

#[test]
fn patch_text_of_commits() {
    let dir = scratch_repo("patch-text");
    write(&dir, "a.txt", "one\n");
    commit(&dir, "base");
    write(&dir, "a.txt", "one\ntwo\n");
    let c2 = commit(&dir, "Second");
    write(&dir, "a.txt", "one\ntwo\nthree\n");
    let c3 = commit(&dir, "Third");
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "nothing"]);
    let empty = head(&dir);

    let (text, report) = patch_text(&dir, &[c2, c3, empty]).unwrap();
    assert_eq!(report, ExportReport { written: 2, skipped: vec![empty] });
    let subjects: Vec<&str> = text.lines().filter(|l| l.starts_with("Subject: ")).collect();
    assert_eq!(subjects, ["Subject: [PATCH] Second", "Subject: [PATCH] Third"]);
    assert_eq!(patch_text(&dir, &[empty]).unwrap_err().to_string(), NOTHING_TO_COPY);

    std::fs::write(dir.join("a.txt"), b"caf\xe9\n").unwrap();
    let latin = commit(&dir, "Latin-1");
    assert_eq!(patch_text(&dir, &[latin]).unwrap_err().to_string(), NOT_TEXT);
    cleanup(&dir);
}

#[test]
fn clipboard_limit() {
    assert!(fits_clipboard(MAX_COPY));
    assert!(!fits_clipboard(MAX_COPY + 1));
    assert_eq!(MAX_COPY, 4 * 1024 * 1024);
}

#[test]
fn file_patch_names() {
    assert_eq!(files_file_name("Fix the login form", &paths(&["src/ui/login.rs"])), "Fix-the-login-form-login-rs.patch");
    assert_eq!(files_file_name("Fix it", &paths(&["a.txt", "b.txt"])), "Fix-it-2-files.patch");
    assert_eq!(files_file_name("", &paths(&["CON"])), "commit-CON-commit.patch");
    let long = files_file_name(&"é".repeat(45), &paths(&["données.txt"]));
    let slug = long.strip_suffix(".patch").unwrap();
    assert!(slug.chars().count() <= 50 && !slug.ends_with('-'), "{long}");
    assert!(slug.starts_with(&"é".repeat(45)), "{long}");
}

#[test]
fn changes_patch_names() {
    assert_eq!(changes_file_name(Some("main"), &paths(&["docs/read me.md"])), "read-me-md-changes.patch");
    assert_eq!(changes_file_name(Some("feature/login"), &[]), "feature-login-changes.patch");
    assert_eq!(changes_file_name(Some("feature/login"), &paths(&["a", "b"])), "feature-login-changes.patch");
    assert_eq!(changes_file_name(None, &[]), "uncommitted-changes.patch");
    assert_eq!(changes_file_name(Some("con"), &[]), "con-commit-changes.patch");
    assert_eq!(changes_file_name(None, &paths(&["nul"])), "nul-commit-changes.patch");
    let long = changes_file_name(Some(&"日本".repeat(40)), &[]);
    assert_eq!(long, format!("{}-changes.patch", "日本".repeat(25)));
}
