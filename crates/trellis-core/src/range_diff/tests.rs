//! Comparing on fixture repositories: a branch rewritten onto a moved main, with and without
//! the mainline cut; finding the last rewrite in a branch's and a remote-tracking ref's reflog;
//! the refusals; reading git's exit; and that nothing in the repository changes.

use super::*;
use crate::testutil::{git, scratch_repo, with_origin, write};
use std::path::{Path, PathBuf};

fn out(code: i32, stderr: &str) -> GitOutput {
    GitOutput { ok: code == 0, code: Some(code), stdout: String::new(), stderr: stderr.into(), timed_out: false }
}

fn head(dir: &Path, rev: &str) -> Oid {
    Repository::open(dir).unwrap().revparse_single(rev).unwrap().peel_to_commit().unwrap().id()
}

fn commit(dir: &Path, file: &str, text: &str, msg: &str) -> Oid {
    write(dir, file, text);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", msg]);
    head(dir, "HEAD")
}

/// base (f.txt: a, b, c); feature = one (f.txt + d and a few more lines), two (g.txt), three (h.txt); main moves (m.txt); then
/// feature is reset onto a rewritten line on main: one amended (content and message), three
/// picked again, and a new four. Returns the folder and feature's tip before the rewrite.
fn rewritten(name: &str) -> (PathBuf, Oid) {
    let dir = scratch_repo(name);
    write(&dir, "base.txt", "base\n");
    commit(&dir, "f.txt", "a\nb\nc\n", "base");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    // Long enough that the edited version still counts as the same commit, changed.
    let one = commit(&dir, "f.txt", "a\nb\nc\nd\ne\nf\ng\nh\n", "one: add d");
    commit(&dir, "g.txt", "g\n", "two: add g");
    let three = commit(&dir, "h.txt", "h\n", "three: add h");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "m.txt", "m\n", "main moves");
    git(&dir, &["checkout", "-q", "-b", "tmp"]);
    git(&dir, &["cherry-pick", &one.to_string()]);
    write(&dir, "f.txt", "a\nb\nc\nD changed\ne\nf\ng\nh\n");
    git(&dir, &["commit", "-q", "-a", "--amend", "-m", "one: add d (edited)"]);
    git(&dir, &["cherry-pick", &three.to_string()]);
    commit(&dir, "z.txt", "z\n", "four: add z");
    git(&dir, &["checkout", "-q", "feature"]);
    git(&dir, &["reset", "-q", "--hard", "tmp"]);
    git(&dir, &["branch", "-q", "-D", "tmp"]);
    (dir, three)
}

fn compared(o: Outcome) -> Compared {
    match o {
        Outcome::Compared(c) => c,
        other => panic!("expected a comparison, got {other:?}"),
    }
}

fn by_subject(c: &Compared) -> Vec<(Status, String)> {
    c.pairs.iter().map(|p| (p.status, p.summary().to_owned())).collect()
}

fn main_line(dir: &Path) -> Option<(String, Oid)> {
    Some(("main".into(), head(dir, "main")))
}

fn feature() -> Ends {
    Ends::LastRewrite { refname: "refs/heads/feature".into() }
}

#[test]
fn a_rewritten_branch_against_its_mainline() {
    let (dir, before) = rewritten("rd-rewritten");
    let c = compared(compare(&dir, &feature(), main_line(&dir)).unwrap());
    assert_eq!(
        by_subject(&c),
        [
            (Status::Changed, "one: add d (edited)".into()),
            (Status::Removed, "two: add g".into()),
            (Status::Unchanged, "three: add h".into()),
            (Status::Added, "four: add z".into()),
        ]
    );
    assert_eq!(c.mainline.as_deref(), Some("main"));
    assert_eq!(c.old.tip, before);
    assert_eq!(c.old.base, head(&dir, "main~1"), "the original base");
    assert_eq!(c.new.base, head(&dir, "main"), "main's tip");
    assert_eq!(c.new.tip, head(&dir, "feature"));
    assert_eq!(c.rewrite.as_ref().map(|r| r.before), Some(before));
    let changed = &c.pairs[0];
    let lines: Vec<(char, &str)> =
        changed.interdiff.iter().flat_map(|d| &d.hunks).flat_map(|h| &h.lines).map(|l| (l.origin, l.text.as_str())).collect();
    assert!(lines.contains(&('-', "+d")) && lines.contains(&('+', "+D changed")), "{lines:?}");
    let old = changed.old.as_ref().unwrap();
    assert_eq!((old.index, old.oid, old.summary.as_str()), (1, head(&dir, &format!("{before}~2")), "one: add d"));
    let new = changed.new.as_ref().unwrap();
    assert_eq!((new.index, new.oid, new.summary.as_str()), (1, head(&dir, "feature~2"), "one: add d (edited)"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_a_mainline_the_merge_base_cuts() {
    let (dir, _) = rewritten("rd-merge-base");
    let c = compared(compare(&dir, &feature(), None).unwrap());
    assert_eq!(c.mainline, None);
    assert_eq!(c.old.base, c.new.base);
    assert!(by_subject(&c).contains(&(Status::Added, "main moves".into())), "main's own commit counts as new");
    let tip = head(&dir, "feature");
    let at_tip = compared(compare(&dir, &feature(), Some(("feature".into(), tip))).unwrap());
    assert_eq!((at_tip.mainline, at_tip.old.base), (None, c.old.base), "a mainline at a tip falls back to the merge base");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_last_rewrite_skips_later_fast_forwards() {
    let dir = scratch_repo("rd-rebase");
    commit(&dir, "base.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    commit(&dir, "f.txt", "f\n", "feature work");
    let before = commit(&dir, "g.txt", "g\n", "more work");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "m.txt", "m\n", "main moves");
    git(&dir, &["checkout", "-q", "feature"]);
    git(&dir, &["rebase", "-q", "main"]);
    commit(&dir, "h.txt", "h\n", "after the rebase");
    let repo = Repository::open(&dir).unwrap();
    let r = bases::last_rewrite(&repo, "refs/heads/feature").unwrap().expect("the rebase is found");
    assert_eq!(r.before, before);
    assert!(r.message.starts_with("rebase"), "{}", r.message);
    let c = compared(compare(&dir, &feature(), main_line(&dir)).unwrap());
    let statuses: Vec<Status> = c.pairs.iter().map(|p| p.status).collect();
    assert_eq!(statuses, [Status::Unchanged, Status::Unchanged, Status::Added]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fast_forwards_and_resets_in_place_are_no_rewrite() {
    let dir = scratch_repo("rd-no-rewrite");
    commit(&dir, "base.txt", "base\n", "base");
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    commit(&dir, "f.txt", "f\n", "one");
    commit(&dir, "g.txt", "g\n", "two");
    git(&dir, &["reset", "-q", "--hard", "HEAD"]);
    assert_eq!(compare(&dir, &feature(), None).unwrap(), Outcome::NoRewrite { refname: "refs/heads/feature".into() });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_force_pushed_remote_tracking_ref() {
    let (work, bare) = with_origin("rd-force-push");
    git(&work, &["checkout", "-q", "-b", "feature"]);
    let pushed = commit(&work, "f.txt", "1\n2\n3\n4\n5\n6\n7\n8\n", "feature");
    git(&work, &["push", "-q", "-u", "origin", "feature"]);
    write(&work, "f.txt", "1\n2\n3\nfour\n5\n6\n7\n8\n");
    git(&work, &["commit", "-q", "-a", "--amend", "--no-edit"]);
    git(&work, &["push", "-q", "-f", "origin", "feature"]);
    let repo = Repository::open(&work).unwrap();
    let r = bases::last_rewrite(&repo, "refs/remotes/origin/feature").unwrap().expect("the force push is found");
    assert_eq!(r.before, pushed);
    let ends = Ends::LastRewrite { refname: "refs/remotes/origin/feature".into() };
    let c = compared(compare(&work, &ends, Some(("origin/main".into(), head(&work, "origin/main")))).unwrap());
    assert_eq!(by_subject(&c), [(Status::Changed, "feature".into())]);
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn refusals_and_reading_only() {
    let (dir, before) = rewritten("rd-refusals");
    git(&dir, &["checkout", "-q", "--orphan", "lonely"]);
    git(&dir, &["rm", "-q", "-r", "-f", "."]);
    let lonely = commit(&dir, "x.txt", "x\n", "unrelated");
    git(&dir, &["checkout", "-q", "feature"]);
    let tip = head(&dir, "feature");
    assert_eq!(compare(&dir, &Ends::Tips { old: lonely, new: tip }, None).unwrap(), Outcome::NoSharedHistory);
    let capped = compare_capped(&dir, &Ends::Tips { old: before, new: tip }, main_line(&dir), 2).unwrap();
    assert!(matches!(capped, Outcome::TooMany { old: 3, .. }), "{capped:?}");
    let refs = |d: &Path| {
        let (_, o) = crate::gitcli::run(d, &crate::gitcli::args(&["show-ref", "--head"]));
        let (_, s) = crate::gitcli::run(d, &crate::gitcli::args(&["status", "--porcelain"]));
        (o, s)
    };
    let was = refs(&dir);
    let same = compared(compare(&dir, &Ends::Tips { old: tip, new: tip }, main_line(&dir)).unwrap());
    assert!(!same.pairs.is_empty() && same.pairs.iter().all(|p| p.status == Status::Unchanged), "{same:?}");
    compare(&dir, &feature(), main_line(&dir)).unwrap();
    assert_eq!(refs(&dir), was, "refs, HEAD and the working tree are as they were");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reading_git_exit() {
    assert!(read_output(&out(0, "")).is_ok());
    let slow = GitOutput { timed_out: true, ..GitOutput::default() };
    assert_eq!(read_output(&slow).unwrap_err().to_string(), "Comparing took too long, and git was stopped.");
    let missing = read_output(&out(1, "git: 'range-diff' is not a git command. See 'git --help'.\n")).unwrap_err();
    assert!(matches!(missing, Error::Unsupported(_)), "{missing:?}");
    assert!(missing.to_string().contains("2.19"), "{missing}");
    // A current git's argument error keeps git's own message, not the usage text, and isn't
    // taken for a git without range-diff.
    let usage = "fatal: need two commit ranges\n\nusage: git range-diff [<options>] <old-base>..<old-tip> <new-base>..<new-tip>\n   or: git range-diff [<options>] <old-tip>...<new-tip>\n";
    let usage = read_output(&out(129, usage)).unwrap_err();
    assert!(matches!(&usage, Error::Other(t) if t == "fatal: need two commit ranges"), "{usage:?}");
    let bad =
        read_output(&out(255, "fatal: Invalid revision range main..0000\nerror: could not parse log for 'main..0000'\n")).unwrap_err();
    assert!(matches!(&bad, Error::Other(t) if t.starts_with("fatal: Invalid revision range")), "{bad:?}");
    assert_eq!(read_output(&out(2, "")).unwrap_err().to_string(), "git range-diff failed");
}

#[test]
fn the_command_line() {
    let (a, b, c, d) = (
        Oid::from_bytes(&[1; 20]).unwrap(),
        Oid::from_bytes(&[2; 20]).unwrap(),
        Oid::from_bytes(&[3; 20]).unwrap(),
        Oid::from_bytes(&[4; 20]).unwrap(),
    );
    let args = command(Range { base: a, tip: b }, Range { base: c, tip: d });
    assert_eq!(args, ["range-diff".to_string(), "--no-color".into(), "--abbrev=40".into(), format!("{a}..{b}"), format!("{c}..{d}")]);
    let shared = command(Range { base: a, tip: a }, Range { base: a, tip: d });
    assert_eq!(shared, ["range-diff".to_string(), "--no-color".into(), "--abbrev=40".into(), a.to_string(), a.to_string(), d.to_string()]);
}

#[test]
fn one_version_an_ancestor_of_the_other() {
    let dir = scratch_repo("rd-ancestor");
    let first = commit(&dir, "base.txt", "base\n", "base");
    let earlier = commit(&dir, "a.txt", "a\n", "one");
    commit(&dir, "b.txt", "b\n", "two");
    let tip = commit(&dir, "c.txt", "c\n", "three");
    // An earlier state of main against now, main itself the mainline: every commit since is new.
    let later = compared(compare(&dir, &Ends::Tips { old: earlier, new: tip }, main_line(&dir)).unwrap());
    assert_eq!(by_subject(&later), [(Status::Added, "two".into()), (Status::Added, "three".into())]);
    assert_eq!((later.mainline, later.old, later.new.base), (None, Range { base: earlier, tip: earlier }, earlier));
    // Two commits on main, without a mainline.
    let picked = compared(compare(&dir, &Ends::Tips { old: first, new: earlier }, None).unwrap());
    assert_eq!(by_subject(&picked), [(Status::Added, "one".into())]);
    // main reset back a commit: its last rewrite dropped one.
    git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
    let reset = compared(compare(&dir, &Ends::LastRewrite { refname: "refs/heads/main".into() }, None).unwrap());
    assert_eq!(by_subject(&reset), [(Status::Removed, "three".into())]);
    assert_eq!(reset.new, Range { base: head(&dir, "main"), tip: head(&dir, "main") });
    // The same commit on both sides, without a mainline, has nothing of its own to compare.
    let same = compared(compare(&dir, &Ends::Tips { old: tip, new: tip }, None).unwrap());
    assert!(same.pairs.is_empty(), "{same:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
