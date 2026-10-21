//! Line endings through conflict resolution: a whole side is staged as its own blob and a
//! chunk-by-chunk resolution keeps each line's ending from the stage it came from, whatever the
//! working file (with its conflict markers) used. git writes the working file from the index,
//! so `core.autocrlf` applies as it would to a checkout.

use super::*;
use crate::gitcli::{args, run};
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::{Path, PathBuf};

/// `f.txt` committed as `base`, changed to `theirs` on `other` and to `ours` on `main`, merged
/// so it conflicts. The bytes are written as given; `autocrlf` is set before anything.
fn conflicted(name: &str, autocrlf: &str, base: &[u8], ours: &[u8], theirs: &[u8]) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", autocrlf]);
    let put = |bytes: &[u8]| std::fs::write(dir.join("f.txt"), bytes).unwrap();
    put(base);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    put(theirs);
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    put(ours);
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let (ok, _) = run(&dir, &args(&["merge", "-q", "other"]));
    assert!(!ok, "the merge should conflict");
    dir
}

fn staged(dir: &Path) -> (Oid, Vec<u8>) {
    let repo = Repository::open(dir).unwrap();
    let id = repo.index().unwrap().get_path(Path::new("f.txt"), 0).expect("resolved").id;
    let bytes = repo.find_blob(id).unwrap().content().to_vec();
    (id, bytes)
}

fn status(dir: &Path) -> String {
    run(dir, &args(&["status", "--porcelain"])).1
}

const BASE: &[u8] = b"one\ntwo\nthree\nfour\n";
const OURS: &[u8] = b"one\ntwo ours\nthree\nfour\n";
/// Theirs added CR LF to the line it changed and to the last line.
const THEIRS: &[u8] = b"one\ntwo theirs\r\nthree\nfour\r\n";

#[test]
fn taking_ours_stages_our_blob_unchanged_when_theirs_has_crlf() {
    let dir = conflicted("eol-ours", "false", BASE, OURS, THEIRS);
    let f = load_conflict(&dir, "f.txt").unwrap();
    // The working file with markers has CR LF from their lines.
    assert!(std::fs::read(dir.join("f.txt")).unwrap().windows(2).any(|w| w == b"\r\n"));
    resolve_with_side(&dir, "f.txt", Side::Ours, &f.ids).unwrap();
    assert_eq!(Some(staged(&dir).0), f.ids.ours, "our blob, not a CR LF copy of it");
    assert_eq!(std::fs::read(dir.join("f.txt")).unwrap(), OURS);
    assert_eq!(status(&dir), "", "the same as HEAD, nothing unstaged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn taking_theirs_keeps_their_mixed_endings() {
    let dir = conflicted("eol-theirs", "false", BASE, OURS, THEIRS);
    let f = load_conflict(&dir, "f.txt").unwrap();
    resolve_with_side(&dir, "f.txt", Side::Theirs, &f.ids).unwrap();
    assert_eq!(Some(staged(&dir).0), f.ids.theirs);
    assert_eq!(std::fs::read(dir.join("f.txt")).unwrap(), THEIRS);
    assert_eq!(status(&dir), "M  f.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn chunk_picks_keep_each_lines_own_ending() {
    let dir = conflicted("eol-chunks", "false", BASE, OURS, THEIRS);
    let f = load_conflict(&dir, "f.txt").unwrap();
    let out = render_output(&f, &vec![Choice::OursThenTheirs; f.chunk_count()]).unwrap();
    assert_eq!(out, "one\ntwo ours\ntwo theirs\r\nthree\nfour\r\n", "LF lines stay LF, CR LF lines stay CR LF");
    write_resolution(&dir, "f.txt", &out, &f.ids).unwrap();
    assert_eq!(staged(&dir).1, out.as_bytes());
    assert_eq!(std::fs::read(dir.join("f.txt")).unwrap(), out.as_bytes());
    assert_eq!(status(&dir), "M  f.txt", "nothing unstaged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn with_autocrlf_the_index_stays_lf_and_git_writes_crlf() {
    let dir = conflicted("eol-autocrlf", "true", BASE, OURS, &THEIRS.iter().copied().filter(|&b| b != b'\r').collect::<Vec<u8>>());
    let f = load_conflict(&dir, "f.txt").unwrap();
    let out = render_output(&f, &vec![Choice::TheirsThenOurs; f.chunk_count()]).unwrap();
    assert!(!out.contains('\r'), "repository form: {out:?}");
    write_resolution(&dir, "f.txt", &out, &f.ids).unwrap();
    assert_eq!(staged(&dir).1, out.as_bytes());
    assert_eq!(std::fs::read(dir.join("f.txt")).unwrap(), out.replace('\n', "\r\n").as_bytes(), "checked out as git would");
    assert_eq!(status(&dir), "M  f.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_side_without_a_final_newline_is_joined_with_the_files_own_ending() {
    let base: &[u8] = b"a\r\nb\r\nc\r\n";
    let dir = conflicted("eol-join", "false", base, b"a\r\nb\r\nours\r\n", b"a\r\nb\r\ntheirs");
    let f = load_conflict(&dir, "f.txt").unwrap();
    let out = render_output(&f, &[Choice::TheirsThenOurs]).unwrap();
    assert_eq!(out, "a\r\nb\r\ntheirs\r\nours\r\n");
    let _ = std::fs::remove_dir_all(&dir);
}
