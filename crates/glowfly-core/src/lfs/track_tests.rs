//! Tracking a file row in LFS: the targets a path offers, the patterns they write (values seen
//! with git-lfs 3.7.1), the checks, the root patterns a snapshot carries, and tracking each
//! target with git-lfs (skipped without it).

use super::tests::{lfs_or_skip, ok, pat};
use super::*;
use crate::repo::{LoadOptions, load};
use crate::testutil::{scratch_repo, write};

fn file(p: &str) -> TrackTarget {
    TrackTarget::File(p.into())
}

fn ext(p: &str) -> TrackTarget {
    TrackTarget::Extension(p.into())
}

fn folder(p: &str) -> TrackTarget {
    TrackTarget::Folder(p.into())
}

#[test]
fn targets_for_paths() {
    assert_eq!(track_targets("assets/tex/a.psd"), [file("assets/tex/a.psd"), ext("*.psd"), folder("assets/tex/**")]);
    assert_eq!(track_targets("README"), [file("README")]);
    assert_eq!(track_targets(".env"), [file(".env")]);
    assert_eq!(track_targets("a."), [file("a.")]);
    assert_eq!(track_targets("pkg/a.tar.gz"), [file("pkg/a.tar.gz"), ext("*.gz"), folder("pkg/**")]);
    assert_eq!(track_targets("dir.v2/Makefile"), [file("dir.v2/Makefile"), folder("dir.v2/**")]);
    assert_eq!(track_targets("x[1]/b*?.bin"), [file("x[1]/b*?.bin"), ext("*.bin"), folder("x[[]1]/**")]);
    assert_eq!(track_targets("A.PSD")[1], ext("*.PSD"), "the case is kept");
    assert_eq!(extension("a/b.c"), Some("c"));
    assert_eq!(extension_pattern("a.p[s]d"), Some("*.p[[]s]d".into()));
    assert_eq!(folder_pattern("top.bin"), None);
}

#[test]
fn raw_matches_git_lfs_escaping() {
    assert_eq!(file("sub dir/a[1] #b.bin").raw(), r"sub[[:space:]]dir/a\[1\][[:space:]]\#b.bin");
    assert_eq!(file("a*b?c.bin").raw(), r"a\*b\?c.bin");
    assert_eq!(folder("y[[]2] z/**").raw(), "y[[]2][[:space:]]z/**");
    assert_eq!(ext("*.psd").raw(), "*.psd");
    assert_eq!(literal_glob("a[b]*?"), "a[[]b][*][?]");
    assert_eq!(literal_glob("plain name#1"), "plain name#1");
}

#[test]
fn target_args() {
    assert_eq!(file("-x.bin").args(), ["lfs", "track", "--filename", "--", "-x.bin"]);
    assert_eq!(folder("a/**").args(), ["lfs", "track", "--", "a/**"]);
    assert_eq!(ext("*.psd").args(), ["lfs", "track", "--", "*.psd"]);
    assert_eq!(track_filename_args("a b.bin"), ["lfs", "track", "--filename", "--", "a b.bin"]);
    assert_eq!((file("a").display(), ext("*.a").display(), folder("d/**").display()), ("a", "*.a", "d/**"));
}

#[test]
fn target_checks() {
    assert_eq!(check_target(&file(".gitattributes"), &[]), Err(PatternProblem::GitFile));
    assert_eq!(check_target(&file("sub/.gitignore"), &[]), Err(PatternProblem::GitFile));
    assert_eq!(check_target(&ext("*.lfsconfig"), &[]), Err(PatternProblem::GitFile));
    assert_eq!(check_target(&file("!x.bin"), &[]), Err(PatternProblem::Negated));
    assert_eq!(check_target(&folder("!dir/**"), &[]), Err(PatternProblem::Negated));
    assert_eq!(check_target(&file("a\nb.bin"), &[]), Err(PatternProblem::Control));
    let psd = [pat("*.psd")];
    assert_eq!(check_target(&ext("*.psd"), &psd), Err(PatternProblem::AlreadyTracked("*.psd".into())));
    assert_eq!(check_target(&ext("*.psd"), &psd).unwrap_err().message(Lang::En), "*.psd is already tracked.");
    let hashed = [pat(r"my[[:space:]]dir/a\#b.bin")];
    assert_eq!(check_target(&file("my dir/a#b.bin"), &hashed), Err(PatternProblem::AlreadyTracked("my dir/a#b.bin".into())));
    assert_eq!(check_target(&ext("*.bin"), &psd), Ok(()));
    assert_eq!(check_target(&file("assets/a.psd"), &psd), Ok(()), "a broader pattern doesn't block the file itself");
    assert_eq!(check_target(&folder("assets/**"), &psd), Ok(()));
}

#[test]
fn root_patterns_and_snapshot() {
    let dir = scratch_repo("lfs-root-patterns");
    write(&dir, "a.txt", "one\n");
    crate::testutil::git(&dir, &["add", "."]);
    crate::testutil::git(&dir, &["commit", "-q", "-m", "first"]);
    assert!(root_patterns(&dir).is_empty());
    assert!(load(&dir, &LoadOptions::all(50)).unwrap().lfs_patterns.is_empty());
    write(&dir, ".gitattributes", "*.psd filter=lfs diff=lfs merge=lfs -text\n*.txt text\n");
    let raws = |p: Vec<LfsPattern>| p.into_iter().map(|p| p.raw).collect::<Vec<_>>();
    assert_eq!(raws(root_patterns(&dir)), ["*.psd"]);
    assert_eq!(raws(load(&dir, &LoadOptions::all(50)).unwrap().lfs_patterns), ["*.psd"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tracking_targets_with_git_lfs() {
    if lfs_or_skip("tracking_targets_with_git_lfs").is_none() {
        return;
    }
    let dir = scratch_repo("lfs-track-targets");
    ok(&dir, install_args());
    let paths = ["top.psd", "sub dir/a[1] #b.bin", "x[1]/f.bin"];
    for p in paths {
        if let Some((folder, _)) = p.rsplit_once('/') {
            std::fs::create_dir_all(dir.join(folder)).unwrap();
        }
        write(&dir, p, "data\n");
        for t in track_targets(p) {
            ok(&dir, t.args());
        }
    }
    let patterns = root_patterns(&dir);
    let raws: Vec<&str> = patterns.iter().map(|p| p.raw.as_str()).collect();
    for p in paths {
        for t in track_targets(p) {
            assert!(raws.contains(&t.raw().as_str()), "{t:?} wrote something else: {raws:?}");
            assert_eq!(check_target(&t, &patterns), Err(PatternProblem::AlreadyTracked(t.display().to_owned())));
        }
        let out = ok(&dir, crate::gitcli::args(&["check-attr", "filter", "--", p]));
        assert!(out.trim_end().ends_with("filter: lfs"), "{p}: {out}");
    }
    // The character-class form round-trips through untrack.
    ok(&dir, untrack_args("x[[]1]/**"));
    ok(&dir, untrack_args("*.psd"));
    let left: Vec<String> = root_patterns(&dir).into_iter().map(|p| p.raw).collect();
    assert!(!left.iter().any(|r| r == "x[[]1]/**" || r == "*.psd"), "{left:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
