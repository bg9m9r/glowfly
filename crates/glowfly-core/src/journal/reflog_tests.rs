//! Loading the journal of a fixture repository, each command run at a fixed time.

use super::notes::{Op, append, capture};
use super::*;
use crate::gitcli::{args, output};
use crate::testutil::*;
use std::path::Path;

/// Runs `git <a>` in `dir` with its reflog time set to `at` (unix seconds).
const T: i64 = 1_700_000_000;

fn git_at(dir: &Path, at: i64, a: &[&str]) {
    let date = format!("{at} +0000");
    let out = output(dir, &args(a), "", &[("GIT_COMMITTER_DATE", &date), ("GIT_AUTHOR_DATE", &date)], None);
    assert!(out.ok, "git {a:?} failed: {}", out.combined());
}

fn commit_at(dir: &Path, at: i64, text: &str) {
    write(dir, "a.txt", text);
    git(dir, &["add", "."]);
    git_at(dir, at, &["commit", "-q", "-m", text]);
}

fn tip(dir: &Path, name: &str) -> Oid {
    Repository::open(dir).unwrap().revparse_single(name).unwrap().peel_to_commit().unwrap().id()
}

fn row_shape(r: &Row) -> (Subject, i64, &Event) {
    (r.subject.clone(), r.at, &r.event)
}

#[test]
fn a_session_of_commands_reads_back_newest_first() {
    let dir = scratch_repo("journal-load");
    write(&dir, "b.txt", "main\n");
    commit_at(&dir, T, "one");
    git_at(&dir, T + 10, &["branch", "feature"]);
    git_at(&dir, T + 20, &["checkout", "-q", "feature"]);
    commit_at(&dir, T + 30, "two");
    git_at(&dir, T + 40, &["commit", "-q", "--amend", "-m", "two again"]);
    commit_at(&dir, T + 50, "three");
    git_at(&dir, T + 60, &["reset", "-q", "--hard", "HEAD~1"]);
    git_at(&dir, T + 70, &["checkout", "-q", "main"]);
    write(&dir, "b.txt", "main two\n");
    git(&dir, &["add", "."]);
    git_at(&dir, T + 80, &["commit", "-q", "-m", "main two"]);
    git_at(&dir, T + 90, &["rebase", "-q", "feature"]);

    let t = load(&dir).unwrap();
    let main = || Subject::Branch("main".into());
    let feature = || Subject::Branch("feature".into());
    let s = |x: &str| x.to_owned();
    let shapes: Vec<_> = t.rows.iter().map(row_shape).collect();
    let onto = tip(&dir, "feature").to_string()[..7].to_owned();
    // Older gits name HEAD, newer ones the branch it pointed at.
    let created = t.rows.get(8).map(|r| r.event.clone()).unwrap_or(Event::Other);
    assert!(matches!(&created, Event::BranchCreated { from } if from == "HEAD" || from == "main"), "{created:?}");
    assert_eq!(
        shapes,
        vec![
            (main(), T + 90, &Event::RebaseFinish { onto: Some(onto) }),
            (main(), T + 80, &Event::Commit { subject: s("main two") }),
            (Subject::Head, T + 70, &Event::Checkout { from: s("feature"), to: s("main") }),
            (feature(), T + 60, &Event::Reset { to: s("HEAD~1") }),
            (feature(), T + 50, &Event::Commit { subject: s("three") }),
            (feature(), T + 40, &Event::Amend { subject: s("two again") }),
            (feature(), T + 30, &Event::Commit { subject: s("two") }),
            (Subject::Head, T + 20, &Event::Checkout { from: s("main"), to: s("feature") }),
            (feature(), T + 10, &created),
            (main(), T, &Event::Initial { subject: s("one") }),
        ],
        "no rebase steps, HEAD's copies folded into the branches"
    );
    assert_eq!(t.rows[0].oid, tip(&dir, "main"));
    assert!(t.rows[0].current && t.rows[3].current, "main's and feature's newest rows");
    assert!(!t.rows[2].current, "HEAD has moved on from that checkout");
    assert_eq!(t.rows[3].summary.as_deref(), Some("two again"));
    assert_eq!(t.head, Some(HeadPos::Branch("main".into())));
    assert!(!t.capped);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_deleted_branch_keeps_a_row_through_its_note() {
    let dir = scratch_repo("journal-deleted");
    commit_at(&dir, T, "one");
    git_at(&dir, T + 10, &["branch", "old"]);
    let c = capture(&dir, Op::DeleteBranch { name: "old".into() }, T + 20).unwrap();
    git_at(&dir, T + 20, &["branch", "-D", "old"]);
    assert!(c.moved(&dir));
    append(&dir, &c.into_note()).unwrap();
    let t = load(&dir).unwrap();
    let old: Vec<&Row> = t.rows.iter().filter(|r| r.subject == Subject::Branch("old".into())).collect();
    assert_eq!(old.len(), 1, "its reflog went with it: {:?}", t.rows);
    assert_eq!(old[0].note.as_ref().map(|n| &n.op), Some(&Op::DeleteBranch { name: "old".into() }));
    assert_eq!((old[0].oid, old[0].at, old[0].summary.as_deref()), (tip(&dir, "main"), T + 20, Some("one")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_state_whose_commit_is_gone_has_no_summary() {
    let dir = scratch_repo("journal-gone");
    commit_at(&dir, T, "one");
    let missing = Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap();
    {
        let repo = Repository::open(&dir).unwrap();
        let mut log = repo.reflog("refs/heads/main").unwrap();
        let sig = git2::Signature::new("Test", "test@example.com", &git2::Time::new(T + 1000, 0)).unwrap();
        log.append(missing, &sig, Some("reset: moving to elsewhere")).unwrap();
        log.write().unwrap();
    }
    let t = load(&dir).unwrap();
    assert_eq!((t.rows[0].oid, t.rows[0].summary.as_ref()), (missing, None));
    assert_eq!(t.rows[1].summary.as_deref(), Some("one"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unborn_repository_has_no_states() {
    let dir = scratch_repo("journal-unborn");
    let t = load(&dir).unwrap();
    assert!(t.rows.is_empty() && t.tips.is_empty() && !t.capped, "{t:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
