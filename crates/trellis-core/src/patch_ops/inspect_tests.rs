//! Reading what a patch file holds without git.

use super::*;

const SERIES: &str = "From 1111111111111111111111111111111111111111 Mon Sep 17 00:00:00 2001
From: Test <test@example.com>
Date: Mon, 5 Jan 2026 10:00:00 +0000
Subject: [PATCH 1/3] Add the
 login form

---
 a.txt | 1 +
diff --git a/a.txt b/a.txt
--- a/a.txt
+++ b/a.txt
@@ -1 +1,2 @@
 one
+two
--
2.45.0

From 2222222222222222222222222222222222222222 Mon Sep 17 00:00:00 2001
From: Test <test@example.com>
Subject: [PATCH 2/3] Second

diff --git a/b c.txt b/b c.txt
new file mode 100644
--- /dev/null
+++ b/b c.txt
@@ -0,0 +1 @@
+--- not a header
From 3333333333333333333333333333333333333333 Mon Sep 17 00:00:00 2001
Date: Mon, 5 Jan 2026 10:00:00 +0000
Subject: [PATCH 3/3] Third

diff --git a/a.txt b/a.txt
--- a/a.txt
+++ b/a.txt
@@ -1 +1 @@
-one
+uno
";

#[test]
fn a_series_of_three() {
    let s = summarize(SERIES.as_bytes());
    assert_eq!(s.commits, 3);
    assert_eq!(s.first_subject.as_deref(), Some("Add the login form"));
    assert_eq!(s.files, ["a.txt", "b c.txt"]);
}

#[test]
fn a_plain_git_diff() {
    let diff =
        "diff --git a/x.rs b/y.rs\nsimilarity index 90%\nrename from x.rs\nrename to y.rs\ndiff --git a/z b/z\ndeleted file mode 100644\n";
    let s = summarize(diff.as_bytes());
    assert_eq!((s.commits, s.first_subject), (0, None));
    assert_eq!(s.files, ["y.rs", "z"]);
}

#[test]
fn a_single_email_without_an_mbox_line() {
    let mail = "From: Test <t@example.com>\nSubject: [PATCH] One thing\n\ndiff --git a/a.txt b/a.txt\n";
    let s = summarize(mail.as_bytes());
    assert_eq!(s.commits, 1);
    assert_eq!(s.first_subject.as_deref(), Some("One thing"));
    assert_eq!(s.files, ["a.txt"]);
}

#[test]
fn quoted_names_are_unquoted() {
    let diff = "diff --git \"a/caf\\303\\251 \\\"q\\\".txt\" \"b/caf\\303\\251 \\\"q\\\".txt\"\n\
                diff --git a/plain b/tab\\there\n\
                diff --git a/x \"b/new\\\\line\"\n";
    let s = summarize(diff.as_bytes());
    assert_eq!(s.files, ["café \"q\".txt", "tab\\there", "new\\line"]);
}

#[test]
fn non_git_unified_diffs() {
    let diff = "--- old/a.txt\t2026-01-05 10:00:00\n+++ new/a.txt\t2026-01-05 10:00:01\n@@ -1 +1 @@\n-x\n+y\n\
                --- b/gone.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-x\n";
    let s = summarize(diff.as_bytes());
    assert_eq!(s.files, ["new/a.txt", "gone.txt"]);
}

#[test]
fn nothing_to_read() {
    assert_eq!(summarize(b""), PatchSummary::default());
    assert_eq!(summarize(b"just some text\nFrom nowhere\n"), PatchSummary::default());
    assert_eq!(summarize(b"\xff\xfe binary \x00").files, Vec::<String>::new());
}

#[test]
fn inspect_refuses_huge_files() {
    let path = crate::paths::temp_dir().join(format!("trellis-test-huge-{}.patch", std::process::id()));
    let f = std::fs::File::create(&path).unwrap();
    f.set_len(MAX_INSPECT + 1).unwrap();
    drop(f);
    let err = inspect(&path).unwrap_err().to_string();
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(err, format!("{name} is larger than 64 MB; Trellis doesn't open patches that large."));
    std::fs::write(&path, SERIES).unwrap();
    assert_eq!(inspect(&path).unwrap().commits, 3);
    let _ = std::fs::remove_file(&path);
}
