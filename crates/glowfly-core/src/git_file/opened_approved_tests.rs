//! The network places approved for the opened folder (Open anyway): a folder whose `.git` file
//! names `testutil::net_place()` opens once that place is approved for it, and for no other
//! folder; a place that isn't approved is still named (every one found, not only the first); a
//! file whose value can't be told is never followed, whatever is approved; and the folders
//! inside an approved one (a submodule) are checked as strictly as before.

use super::*;
use crate::error::Error;
use crate::testutil::{git, git_spelling, net_place, scratch_repo, with_submodule, write};

/// How a refusal names the network place `p`.
fn text(p: &Path) -> String {
    crate::net_path::place_text(&crate::net_path::root_of(p).unwrap()).unwrap()
}

/// A second network place, never reached.
fn other_place() -> PathBuf {
    if cfg!(windows) { PathBuf::from(r"\\glowfly-other.invalid\share") } else { PathBuf::from("/net/glowfly-other.invalid") }
}

fn approve(folder: &Path, places: &[String]) -> Approved {
    Approved::new(vec![(folder.to_path_buf(), places.to_vec())])
}

#[test]
fn an_approved_place_is_followed_for_its_own_folder_only() {
    let dir = scratch_repo("approved-git-file");
    let inner = dir.join("inner");
    std::fs::create_dir_all(inner.join("src")).unwrap();
    write(&inner, ".git", &format!("gitdir: {}\n", git_spelling(&net_place().join("x/.git"))));
    let net = text(&net_place());
    let refused = Err(Refused::Network { folder: inner.clone(), places: vec![net.clone()] });
    assert_eq!(check(&inner, false, &Approved::default()), refused);
    // Approved for the folder above it, or one inside it: still refused.
    assert_eq!(check(&inner, false, &approve(&dir, std::slice::from_ref(&net))), refused);
    assert_eq!(check(&inner, false, &approve(&inner.join("src"), std::slice::from_ref(&net))), refused);
    // Another place approved for it: still refused.
    assert_eq!(check(&inner, false, &approve(&inner, &[text(&other_place())])), refused);
    // Approved for it: followed (the open itself then fails, as nothing is there).
    let approved = approve(&inner, std::slice::from_ref(&net));
    assert_eq!(check(&inner, false, &approved), Ok(()));
    assert!(!matches!(
        crate::safe_directory::check_with(&inner, false, &approved),
        Err(Error::NetworkPointer(_) | Error::UncheckedGitData(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_place_found_is_named_and_an_unapproved_one_still_refuses() {
    let dir = scratch_repo("approved-two-places");
    let (a, b) = (net_place(), other_place());
    git(&dir, &["config", "include.path", &git_spelling(&a.join("shared.cfg"))]);
    write(&dir.join(".git/objects/info"), "alternates", &format!("{}\n", git_spelling(&b.join("store/objects"))));
    let (ta, tb) = (text(&a), text(&b));
    // Both, the alternates' first as libgit2 reads them.
    assert_eq!(
        check(&dir, false, &Approved::default()),
        Err(Refused::Network { folder: dir.clone(), places: vec![tb.clone(), ta.clone()] })
    );
    let only_a = approve(&dir, std::slice::from_ref(&ta));
    assert_eq!(check(&dir, false, &only_a), Err(Refused::Network { folder: dir.clone(), places: vec![tb.clone()] }));
    let named = crate::safe_directory::check_with(&dir, false, &only_a);
    assert!(matches!(named, Err(Error::NetworkPointer(ref n)) if n.places == [tb.clone()] && n.folder == dir), "{named:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn git_data_that_cant_be_told_is_never_approved() {
    let dir = scratch_repo("approved-untold");
    let inner = dir.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    write(&inner, ".git", &format!("gitdir: {}\nmore\n", git_spelling(&net_place().join("x/.git"))));
    let all = approve(&inner, &[text(&net_place()), text(&other_place())]);
    assert_eq!(check(&inner, true, &all), Err(Refused::Unchecked(inner.join(".git"))));
    assert!(matches!(crate::safe_directory::check_with(&inner, true, &all), Err(Error::UncheckedGitData(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_submodule_of_an_approved_folder_is_checked_as_before() {
    let (parent, sub) = with_submodule("approved-sub");
    let net = text(&net_place());
    git(&parent, &["config", "include.path", &git_spelling(&net_place().join("shared.cfg"))]);
    write(&sub, ".git", &format!("gitdir: {}\n", git_spelling(&net_place().join("lib/.git"))));
    let approved = approve(&parent, std::slice::from_ref(&net));
    assert_eq!(check(&parent, false, &approved), Ok(()), "the folder approved");
    assert_eq!(check(&sub, true, &approved), Err(Refused::Network { folder: sub.clone(), places: vec![net] }));
    assert!(matches!(super::super::other_repo(&sub, &parent), Found::Refused(_)), "listed from the superproject: refused");
    assert!(super::super::offsite_gitlinks(&git2::Repository::open(&parent).unwrap()).iter().any(|o| o.path == "libs/lib"));
    let _ = std::fs::remove_dir_all(&parent);
    let _ = std::fs::remove_dir_all(parent.with_file_name(format!("{}-lib", parent.file_name().unwrap().to_string_lossy())));
}
