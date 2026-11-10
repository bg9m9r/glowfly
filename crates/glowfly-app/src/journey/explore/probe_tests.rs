//! Tests of what the probes tell from readings on disk.

use super::super::super::fixture::{Fixture, commit_files, git, git_try, write};
use super::super::action::Action;
use super::{read, took_effect};
use std::path::PathBuf;

fn repo(name: &str) -> (Fixture, PathBuf) {
    let fx = Fixture::new(&format!("probe-{name}"));
    let dir = fx.work().join("r");
    fx.init(&dir, &[("a.txt", "base\n"), ("sub/b.txt", "b\n")]);
    (fx, dir)
}

#[test]
fn commit_probe() {
    let (_fx, dir) = repo("commit");
    let before = read(&dir);
    commit_files(&dir, &[("a.txt", "base\nmore\n")], "typed text");
    let after = read(&dir);
    assert_eq!(took_effect(&Action::Commit("typed text".into()), &before, &after), Some(true));
    assert_eq!(took_effect(&Action::Commit("other".into()), &before, &after), Some(false));
    assert_eq!(took_effect(&Action::Commit("typed text".into()), &after, &after), Some(false));
}

#[test]
fn amend_probe() {
    let (_fx, dir) = repo("amend");
    let before = read(&dir);
    git(&dir, &["commit", "-q", "--amend", "-m", "reworded"]);
    let after = read(&dir);
    assert_eq!(took_effect(&Action::Amend, &before, &after), Some(true));
    assert_eq!(took_effect(&Action::Amend, &after, &after), Some(false));
}

#[test]
fn stash_probe() {
    let (_fx, dir) = repo("stash");
    write(&dir, "a.txt", "changed\n");
    let before = read(&dir);
    git(&dir, &["stash", "push", "-q"]);
    let after = read(&dir);
    assert_eq!(took_effect(&Action::Stash, &before, &after), Some(true));
    assert_eq!(took_effect(&Action::DropStash, &after, &before), Some(true));
    assert_eq!(took_effect(&Action::Stash, &after, &after), Some(false));
}

#[test]
fn checkout_probe() {
    let (_fx, dir) = repo("checkout");
    git(&dir, &["branch", "side"]);
    let before = read(&dir);
    assert_eq!(took_effect(&Action::CreateBranch("side".into()), &before, &before), Some(true));
    assert_eq!(took_effect(&Action::Checkout("side".into()), &before, &before), Some(false));
    git(&dir, &["checkout", "-q", "side"]);
    let after = read(&dir);
    assert_eq!(took_effect(&Action::Checkout("side".into()), &before, &after), Some(true));
    assert_eq!(took_effect(&Action::DeleteBranch("side".into()), &before, &after), Some(false));
}

#[test]
fn discard_probe() {
    let (_fx, dir) = repo("discard");
    write(&dir, "sub/b.txt", "edit\n");
    write(&dir, "new.txt", "new\n");
    let before = read(&dir);
    assert!(before.changed.contains("sub/b.txt") && before.changed.contains("new.txt"));
    git(&dir, &["checkout", "-q", "--", "sub/b.txt"]);
    let after = read(&dir);
    assert_eq!(took_effect(&Action::Discard("sub/b.txt".into()), &before, &after), Some(true));
    assert_eq!(took_effect(&Action::Discard("new.txt".into()), &before, &after), Some(false));
}

#[test]
fn stage_probes() {
    let (_fx, dir) = repo("stage");
    write(&dir, "a.txt", "edit\n");
    write(&dir, "new.txt", "new\n");
    let before = read(&dir);
    git(&dir, &["add", "a.txt"]);
    let one = read(&dir);
    assert_eq!(took_effect(&Action::Stage("a.txt".into()), &before, &one), Some(true));
    assert_eq!(took_effect(&Action::StageAll, &before, &one), Some(false));
    git(&dir, &["add", "-A"]);
    let all = read(&dir);
    assert_eq!(took_effect(&Action::StageAll, &one, &all), Some(true));
    assert_eq!(took_effect(&Action::Unstage("a.txt".into()), &all, &all), Some(false));
}

#[test]
fn merge_conflict_counts_as_effect() {
    let (_fx, dir) = repo("merge");
    git(&dir, &["branch", "other"]);
    commit_files(&dir, &[("a.txt", "mine\n")], "mine");
    git(&dir, &["checkout", "-q", "other"]);
    commit_files(&dir, &[("a.txt", "theirs\n")], "theirs");
    git(&dir, &["checkout", "-q", "-"]);
    let before = read(&dir);
    assert!(!git_try(&dir, &["merge", "other"]));
    let after = read(&dir);
    assert_eq!(took_effect(&Action::Merge("other".into()), &before, &after), Some(true));
    assert!(after.conflicted.contains("a.txt"));
    git(&dir, &["checkout", "--ours", "a.txt"]);
    git(&dir, &["add", "a.txt"]);
    let resolved = read(&dir);
    assert_eq!(took_effect(&Action::ResolveOurs("a.txt".into()), &after, &resolved), Some(true));
    assert_eq!(took_effect(&Action::ResolveTheirs("a.txt".into()), &after, &after), Some(false));
    git(&dir, &["merge", "--abort"]);
    assert_eq!(took_effect(&Action::Abort, &after, &read(&dir)), Some(true));
    assert_eq!(took_effect(&Action::Abort, &before, &after), Some(false));
}

#[test]
fn nothing_to_probe() {
    let (_fx, dir) = repo("none");
    let r = read(&dir);
    for a in [Action::SelectWip, Action::Refresh, Action::SwitchTab("x".into()), Action::Confirm("Cancel".into())] {
        assert_eq!(took_effect(&a, &r, &r), None);
    }
}

#[test]
fn an_old_conflict_is_not_the_steps_effect() {
    let (_fx, dir) = repo("old-conflict");
    git(&dir, &["branch", "other"]);
    commit_files(
        &dir,
        &[(
            "a.txt", "mine
",
        )],
        "mine",
    );
    git(&dir, &["checkout", "-q", "other"]);
    commit_files(
        &dir,
        &[(
            "a.txt", "theirs
",
        )],
        "theirs",
    );
    git(&dir, &["checkout", "-q", "-"]);
    assert!(!git_try(&dir, &["merge", "other"]));
    let mid = read(&dir);
    assert_eq!(took_effect(&Action::Merge("other".into()), &mid, &mid), Some(false));
    assert_eq!(took_effect(&Action::Rebase("other".into()), &mid, &mid), Some(false));
    assert_eq!(took_effect(&Action::Unstash, &mid, &mid), Some(false));
    assert_eq!(took_effect(&Action::Continue, &read(&repo("clean").1), &read(&repo("clean2").1)), Some(false));
}

#[test]
fn unstash_conflict_is_an_effect_only_when_new() {
    let (_fx, dir) = repo("unstash");
    write(
        &dir, "a.txt", "stashed
",
    );
    git(&dir, &["stash", "push", "-q"]);
    commit_files(
        &dir,
        &[(
            "a.txt",
            "committed
",
        )],
        "other change",
    );
    let before = read(&dir);
    assert!(!git_try(&dir, &["stash", "pop"]));
    let after = read(&dir);
    assert_eq!(after.stashes, before.stashes);
    assert_eq!(took_effect(&Action::Unstash, &before, &after), Some(true));
}

#[test]
fn unstage_all_has_nothing_to_probe_when_nothing_was_staged() {
    let (_fx, dir) = repo("unstage-all");
    let r = read(&dir);
    assert_eq!(took_effect(&Action::UnstageAll, &r, &r), None);
}

#[test]
fn merging_a_branch_the_head_has_has_nothing_to_probe() {
    let (_fx, dir) = repo("contained");
    git(&dir, &["branch", "same"]);
    git(&dir, &["branch", "behind"]);
    commit_files(&dir, &[("a.txt", "ahead\n")], "ahead");
    git(&dir, &["branch", "-f", "behind", "HEAD~1"]);
    git(&dir, &["checkout", "-q", "-b", "later"]);
    commit_files(&dir, &[("a.txt", "later\n")], "later");
    git(&dir, &["checkout", "-q", "main"]);
    let r = read(&dir);
    assert_eq!(r.contained.iter().map(String::as_str).collect::<Vec<_>>(), ["behind", "main", "same"]);
    for b in ["same", "behind"] {
        assert_eq!(took_effect(&Action::Merge(b.into()), &r, &r), None, "{b}");
        assert_eq!(took_effect(&Action::Rebase(b.into()), &r, &r), None, "{b}");
    }
    assert_eq!(took_effect(&Action::Merge("later".into()), &r, &r), Some(false));
}

#[test]
fn undoing_a_checkout_between_branches_at_one_commit_is_an_effect() {
    let (_fx, dir) = repo("undo-checkout");
    git(&dir, &["branch", "twin"]);
    let before = read(&dir);
    git(&dir, &["checkout", "-q", "twin"]);
    let after = read(&dir);
    assert_eq!(before.head, after.head);
    assert_eq!(took_effect(&Action::Undo, &before, &after), Some(true));
    assert_eq!(took_effect(&Action::Redo, &after, &after), Some(false));
}

#[test]
fn names_are_the_ones_the_app_shows() {
    assert_eq!(super::name(b"src/b.txt"), "src/b.txt");
    assert_eq!(super::name("caf\u{e9}.txt".as_bytes()), "caf\u{e9}.txt");
    // A byte that isn't UTF-8 is named as the app's file lists name it, not as U+FFFD.
    assert_eq!(super::name(b"caf\xe9.txt"), r"caf\xE9.txt");
    assert_eq!(super::name(b"dir\xff/\xfe"), r"dir\xFF/\xFE");
}

#[test]
fn a_conflicted_name_that_is_not_utf8_is_read() {
    // Written into the index only, so it runs where such a file can't be made (Windows, macOS).
    let (_fx, dir) = repo("not-utf8");
    let r = git2::Repository::open(&dir).unwrap();
    let mut index = r.index().unwrap();
    for stage in 1..=3u16 {
        let base = index.get_path(std::path::Path::new("a.txt"), 0).unwrap();
        let path = b"caf\xe9.txt".to_vec();
        let flags = (stage << 12) | path.len() as u16;
        index.add(&git2::IndexEntry { path, flags, ..base }).unwrap();
    }
    index.write().unwrap();
    let reading = read(&dir);
    assert!(reading.conflicted.contains(r"caf\xE9.txt"), "{:?}", reading.conflicted);
    assert!(reading.changed.contains(r"caf\xE9.txt"), "{:?}", reading.changed);
}
