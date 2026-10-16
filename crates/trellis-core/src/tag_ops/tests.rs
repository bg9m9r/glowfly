use super::*;
use crate::gitcli;
use crate::repo::{LoadOptions, Snapshot, load};
use crate::testutil::{git, scratch_repo, with_origin, write};
use std::path::Path;

const SPACES: &str = "Tag names can't contain spaces.";
const DASH: &str = "Tag names can't start with a dash.";
const DOTS: &str = "Tag names can't contain two dots in a row.";
const CHARS: &str = "Tag names can't contain ~ ^ : ? * [ or a backslash.";
const AT_BRACE: &str = "Tag names can't contain @{.";
const LOCK: &str = "No part of a tag name can end in .lock.";
const SLASH: &str = "Tag names can't start or end with a slash, or have two in a row.";
const DOT: &str = "Parts of a tag name can't start with a dot, and the name can't end with one.";

#[test]
fn valid_tag_names() {
    for name in ["v1.0.0", "v1.0.0-rc.1", "release/2024-10", "a/b/c", "version-ü-ñ"] {
        assert!(is_valid_tag_name(name), "{name}");
        assert_eq!(check_tag_name(name, &[]), Ok(()), "{name}");
    }
}

#[test]
fn invalid_tag_names_with_reasons() {
    let cases = [
        ("has space", SPACES),
        ("has\ttab", SPACES),
        ("-x", DASH),
        ("a..b", DOTS),
        ("a~b", CHARS),
        ("a^b", CHARS),
        ("a:b", CHARS),
        ("a?b", CHARS),
        ("a*b", CHARS),
        ("a[b", CHARS),
        ("a\\b", CHARS),
        ("a@{b", AT_BRACE),
        ("v1.lock", LOCK),
        ("dir/x.lock", LOCK),
        ("/x", SLASH),
        ("x/", SLASH),
        ("a//b", SLASH),
        (".hidden", DOT),
        ("dir/.x", DOT),
        ("trailing.", DOT),
    ];
    for (name, reason) in cases {
        assert!(!is_valid_tag_name(name), "{name:?} should be invalid");
        assert_eq!(check_tag_name(name, &[]), Err(TagNameProblem::Invalid(reason)), "{name:?}");
    }
}

#[test]
fn nul_and_control_chars_do_not_panic() {
    let nul = format!("a{}b", char::from(0));
    let bell = format!("a{}b", char::from(7));
    assert!(!is_valid_tag_name(&nul));
    assert!(!is_valid_tag_name(&bell));
    let control = TagNameProblem::Invalid("Tag names can't contain control characters.");
    assert_eq!(check_tag_name(&nul, &[]), Err(control));
}

#[test]
fn existing_and_clashing_tags() {
    let existing = ["v0.1.0", "v1"];
    assert_eq!(check_tag_name("v0.1.0", &existing), Err(TagNameProblem::Exists("v0.1.0".into())));
    assert_eq!(check_tag_name("V0.1.0", &existing), Err(TagNameProblem::Exists("v0.1.0".into())));
    assert_eq!(check_tag_name("v1/rc", &existing), Err(TagNameProblem::Clashes("v1".into())));
    assert_eq!(check_tag_name("v0.2.0", &existing), Ok(()));
    assert_eq!(check_tag_name("", &existing), Err(TagNameProblem::Empty));
    assert_eq!(check_tag_name("v2", &["v2/rc"]), Err(TagNameProblem::Clashes("v2/rc".into())));

    assert_eq!(TagNameProblem::Exists("v0.1.0".into()).message("V0.1.0"), "Tag v0.1.0 already exists.");
    assert_eq!(TagNameProblem::Clashes("v1".into()).message("v1/rc"), "v1/rc clashes with tag v1.");
    assert_eq!(TagNameProblem::Invalid(SPACES).message("a b"), SPACES);
    assert_eq!(TagNameProblem::Empty.message(""), "Enter a tag name.");
}

#[test]
fn tag_and_push_commands() {
    let sha = "0123456789abcdef0123456789abcdef01234567";
    assert_eq!(tag_cmd("v1", sha, ""), ["tag", "v1", sha]);
    assert_eq!(tag_cmd("v1", sha, "notes"), ["tag", "-a", "v1", sha, "-m", "notes"]);
    assert_eq!(push_tag_cmd("origin", "v1"), ["push", "origin", "refs/tags/v1:refs/tags/v1"]);
    assert_eq!(delete_remote_tag_cmd("origin", "v1"), ["push", "origin", "--delete", "refs/tags/v1"]);
    assert_eq!(delete_remote_tag_cmd("origin", "release/1.0"), ["push", "origin", "--delete", "refs/tags/release/1.0"]);
    assert_eq!(annotate_tag_cmd("v1", sha, "notes"), ["tag", "-a", "-f", "v1", sha, "-m", "notes"]);
    assert_eq!(annotate_tag_cmd("release/1.0", sha, "notes"), ["tag", "-a", "-f", "release/1.0", sha, "-m", "notes"]);
}

#[test]
fn delete_remote_tag_keeps_same_named_branch() {
    let (work, bare) = with_origin("tag-delete-remote");
    git(&work, &["branch", "v1"]);
    git(&work, &["tag", "v1"]);
    git(&work, &["push", "-q", "origin", "refs/heads/v1:refs/heads/v1", "refs/tags/v1:refs/tags/v1"]);

    let (ok, out) = gitcli::run(&work, &delete_remote_tag_cmd("origin", "v1"));
    assert!(ok, "{out}");
    let remote = git2::Repository::open_bare(&bare).unwrap();
    assert!(remote.find_reference("refs/tags/v1").is_err(), "the remote tag is gone");
    assert!(remote.find_reference("refs/heads/v1").is_ok(), "the same-named branch stays");
    let local = git2::Repository::open(&work).unwrap();
    assert!(local.find_reference("refs/tags/v1").is_ok(), "the local tag stays");

    // Deleting it again touches nothing else. With the full ref, git sends the delete anyway and
    // the remote only warns; older or stricter servers refuse it.
    let (ok, out) = gitcli::run(&work, &delete_remote_tag_cmd("origin", "v1"));
    assert!(out.contains("non-existent ref") || (!ok && out.contains("remote ref does not exist")), "{out}");
    assert!(remote.find_reference("refs/heads/v1").is_ok(), "the same-named branch still stays");
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

fn snap(dir: &Path) -> Snapshot {
    load(dir, &LoadOptions::all(10)).unwrap()
}

/// A repository with two commits; returns it with the first commit's sha.
fn two_commits(name: &str) -> (std::path::PathBuf, git2::Oid) {
    let dir = scratch_repo(name);
    write(
        &dir, "a.txt", "one
",
    );
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write(
        &dir, "a.txt", "two
",
    );
    git(&dir, &["commit", "-q", "-am", "second"]);
    let first = git2::Repository::open(&dir).unwrap().revparse_single("HEAD~1").unwrap().id();
    (dir, first)
}

#[test]
fn lightweight_tags_are_only_tags_without_a_tag_object() {
    let (dir, _) = two_commits("tag-lightweight-set");
    git(&dir, &["tag", "v1"]);
    git(&dir, &["tag", "-a", "v2", "-m", "x"]);
    git(&dir, &["tag", "t", "HEAD^{tree}"]);
    let s = snap(&dir);
    assert!(s.lightweight_tags.contains("v1"));
    assert!(!s.lightweight_tags.contains("v2"), "an annotated tag");
    assert!(s.tags.iter().any(|(n, _)| n == "v2"));
    assert!(!s.lightweight_tags.contains("t"), "a tag at a tree");
    assert!(s.other_tags.contains(&"t".to_owned()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn annotate_replaces_lightweight_tag() {
    let (dir, first) = two_commits("tag-annotate");
    git(&dir, &["tag", "v1", "HEAD~1"]);
    assert!(snap(&dir).lightweight_tags.contains("v1"));

    let (ok, out) = gitcli::run(&dir, &annotate_tag_cmd("v1", &first.to_string(), "Release notes"));
    assert!(ok, "{out}");
    let repo = git2::Repository::open(&dir).unwrap();
    let target = repo.find_reference("refs/tags/v1").unwrap().target().unwrap();
    let tag = repo.find_tag(target).expect("the ref names a tag object");
    assert_eq!(tag.target_id(), first, "same commit");
    assert_eq!(tag.message().unwrap().trim(), "Release notes");

    let s = snap(&dir);
    assert!(!s.lightweight_tags.contains("v1"));
    assert!(s.tags.iter().any(|(n, oid)| n == "v1" && *oid == first));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn annotate_honours_tag_signing() {
    let (dir, first) = two_commits("tag-annotate-sign");
    git(&dir, &["tag", "v1", "HEAD~1"]);
    git(&dir, &["config", "tag.gpgSign", "true"]);
    let missing = dir.join("no-such-signer").display().to_string().replace('\\', "/");
    git(&dir, &["config", "gpg.program", &missing]);

    let (ok, out) = gitcli::run(&dir, &annotate_tag_cmd("v1", &first.to_string(), "Release notes"));
    assert!(!ok, "signing with a missing program fails: {out}");
    let lower = out.to_lowercase();
    assert!(lower.contains("gpg") || lower.contains("sign"), "{out}");
    let repo = git2::Repository::open(&dir).unwrap();
    let target = repo.find_reference("refs/tags/v1").unwrap().target().unwrap();
    assert_eq!(target, first, "the lightweight tag is untouched");
    assert_eq!(repo.find_object(target, None).unwrap().kind(), Some(git2::ObjectType::Commit));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tag_folders_and_windows_names() {
    let existing = ["release/1.0"];
    assert_eq!(
        check_tag_name_for("Release/2.0", &existing, false),
        Err(TagNameProblem::FolderCase { typed: "Release".into(), existing: "release".into() })
    );
    assert_eq!(check_tag_name_for("release/2.0", &existing, false), Ok(()));
    assert_eq!(check_tag_name_for("con", &[], false), Ok(()));
    let device = crate::names::WinNameProblem::Device("con".into());
    assert_eq!(check_tag_name_for("con", &[], true), Err(TagNameProblem::Windows("con".into(), device)));
    assert_eq!(
        TagNameProblem::FolderCase { typed: "Release".into(), existing: "release".into() }.message("Release/2.0"),
        "The folder Release already exists as release; use that spelling."
    );
}
