use super::*;
use crate::testutil::*;
use git2::Repository;
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

fn summaries(h: &FileHistory) -> Vec<&str> {
    h.entries.iter().map(|e| e.summary.as_str()).collect()
}

fn statuses(h: &FileHistory) -> String {
    h.entries.iter().map(|e| e.status).collect()
}

fn oids(h: &FileHistory) -> Vec<Oid> {
    h.entries.iter().map(|e| e.oid).collect()
}

fn lines(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

#[test]
fn history_lists_commits_that_touch_the_path() {
    let dir = scratch_repo("history-basic");
    commit(&dir, "a.txt", "a\n", "add a");
    commit(&dir, "b.txt", "b\n", "add b");
    commit(&dir, "a.txt", "a\na\n", "edit a");
    commit(&dir, "b.txt", "b\nb\n", "edit b");
    let h = file_history(&dir, "a.txt", &HistoryStart::Head, 100).unwrap();
    assert_eq!(summaries(&h), ["edit a", "add a"]);
    assert_eq!(statuses(&h), "MA");
    assert!(h.entries.iter().all(|e| e.author == "Test" && e.path == "a.txt" && e.old_path.is_none()));
    assert_eq!(h.entries[0].email, "test@example.com");
    assert!(h.resume.is_empty() && h.searched_until.is_none() && h.from_head);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn history_follows_a_rename() {
    let dir = scratch_repo("history-rename");
    commit(&dir, "old.txt", &lines(10), "add old");
    commit(&dir, "old.txt", &lines(10).replace("line 3\n", "line three\n"), "edit old");
    git(&dir, &["mv", "old.txt", "new.txt"]);
    let rename = commit(&dir, "new.txt", &lines(10).replace("line 3\n", "line three\n").replace("line 8\n", "line eight\n"), "rename");
    commit(&dir, "new.txt", &lines(10).replace("line 3\n", "line three\n").replace("line 8\n", "line 8!\n"), "edit new");

    let h = file_history(&dir, "new.txt", &HistoryStart::Head, 100).unwrap();
    assert_eq!(summaries(&h), ["edit new", "rename", "edit old", "add old"]);
    assert_eq!(statuses(&h), "MRMA");
    let paths: Vec<&str> = h.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, ["new.txt", "new.txt", "old.txt", "old.txt"]);
    assert_eq!(h.entries[1].old_path.as_deref(), Some("old.txt"));
    assert_eq!(h.entries[1].oid, rename);
    assert!(h.resume.is_empty());
    // The entry's path opens that commit's diff of the file.
    let d = load_diff(&dir, &DiffTarget::Commit(rename, h.entries[1].path.clone())).unwrap();
    assert!(!d.hunks.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn history_simplifies_merges() {
    // Only the side branch touches f.txt: the merge takes it unchanged from the side.
    let dir = scratch_repo("history-merge");
    commit(&dir, "f.txt", &lines(10), "base");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    commit(&dir, "f.txt", &lines(10).replace("line 2\n", "side\n"), "side edit");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "g.txt", "g\n", "main edit");
    git(&dir, &["merge", "-q", "--no-ff", "side", "-m", "merge side"]);
    let h = file_history(&dir, "f.txt", &HistoryStart::Head, 100).unwrap();
    assert_eq!(summaries(&h), ["side edit", "base"]);

    // Both sides edit f.txt and the merge resolves it to new content.
    git(&dir, &["checkout", "-q", "-b", "other"]);
    commit(&dir, "f.txt", &lines(10).replace("line 2\n", "other\n"), "other edit");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "f.txt", &lines(10).replace("line 2\n", "main\n"), "main f edit");
    let (ok, _) = crate::gitcli::run(&dir, &crate::gitcli::args(&["merge", "other", "-m", "merge other"]));
    assert!(!ok, "the merge conflicts");
    write(&dir, "f.txt", &lines(10).replace("line 2\n", "resolved\n"));
    git(&dir, &["add", "f.txt"]);
    git(&dir, &["commit", "-q", "-m", "merge other"]);
    let h = file_history(&dir, "f.txt", &HistoryStart::Head, 100).unwrap();
    let s = summaries(&h);
    assert_eq!(s[0], "merge other");
    for want in ["other edit", "main f edit", "side edit", "base"] {
        assert_eq!(s.iter().filter(|x| **x == want).count(), 1, "{want} listed once in {s:?}");
    }
    assert_eq!(s.len(), 5);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn history_stops_at_the_limit_and_resumes() {
    let dir = scratch_repo("history-limit");
    let mut made = Vec::new();
    for i in 0..6 {
        let file = if i % 2 == 0 { "f.txt" } else { "other.txt" };
        made.push(commit(&dir, file, &format!("{i}\n"), &format!("commit {i}")));
    }
    let full = file_history(&dir, "f.txt", &HistoryStart::Head, 100).unwrap();
    assert_eq!(summaries(&full), ["commit 4", "commit 2", "commit 0"]);

    let part = file_history(&dir, "f.txt", &HistoryStart::Head, 3).unwrap();
    assert!(part.entries.len() < full.entries.len());
    // Examined newest first: commits 5, 4 and 3.
    let third = made[3];
    let time = Repository::open(&dir).unwrap().find_commit(third).unwrap().time().seconds();
    assert_eq!(part.searched_until, Some((third, time)));
    assert!(!part.resume.is_empty());

    let rest = file_history(&dir, "f.txt", &HistoryStart::Resume(part.resume.clone()), 100).unwrap();
    assert!(rest.resume.is_empty() && rest.searched_until.is_none());
    let mut all = oids(&part);
    all.extend(oids(&rest));
    assert_eq!(all, oids(&full));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn history_of_a_deleted_file_starts_at_the_deleting_commit() {
    let dir = scratch_repo("history-deleted");
    commit(&dir, "d.txt", "d\n", "add d");
    commit(&dir, "d.txt", "d\nd\n", "edit d");
    git(&dir, &["rm", "-q", "d.txt"]);
    git(&dir, &["commit", "-q", "-m", "remove d"]);
    let removed = head(&dir);
    commit(&dir, "x.txt", "x\n", "unrelated");

    let h = file_history(&dir, "d.txt", &HistoryStart::At(removed), 100).unwrap();
    assert_eq!(summaries(&h), ["remove d", "edit d", "add d"]);
    assert_eq!(statuses(&h), "DMA");
    assert!(!h.from_head);
    let at_head = file_history(&dir, "d.txt", &HistoryStart::Head, 100).unwrap();
    assert!(at_head.entries.is_empty() && at_head.resume.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn start_at_an_ancestor_uses_head() {
    let dir = scratch_repo("history-start");
    let older = commit(&dir, "f.txt", "1\n", "first");
    commit(&dir, "f.txt", "2\n", "second");
    git(&dir, &["checkout", "-q", "-b", "topic", &older.to_string()]);
    let topic = commit(&dir, "f.txt", "topic\n", "topic edit");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "f.txt", "3\n", "third");

    let h = file_history(&dir, "f.txt", &HistoryStart::At(older), 100).unwrap();
    assert!(h.from_head);
    assert_eq!(summaries(&h), ["third", "second", "first"]);

    let h = file_history(&dir, "f.txt", &HistoryStart::At(topic), 100).unwrap();
    assert!(!h.from_head);
    assert_eq!(summaries(&h), ["topic edit", "first"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn history_of_an_unborn_repository_is_empty() {
    let dir = scratch_repo("history-unborn");
    let h = file_history(&dir, "a.txt", &HistoryStart::Head, 100).unwrap();
    assert!(h.entries.is_empty() && h.resume.is_empty() && h.searched_until.is_none() && h.from_head);
    let _ = std::fs::remove_dir_all(&dir);
}
