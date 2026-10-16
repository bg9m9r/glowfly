use super::conflict::{marker_size, parse_merged};
use super::*;
use crate::gitcli::{args, run};
use crate::testutil::*;
use git2::Repository;
use std::path::{Path, PathBuf};

fn base_lines() -> String {
    (1..=12).map(|i| format!("line {i}\n")).collect()
}

/// `f` committed on main as `base`, changed to `theirs` on branch `other` and to `ours` on
/// main, then `git merge other`, which must conflict.
fn merge_conflict(name: &str, f: &str, base: &str, ours: &str, theirs: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, f, base);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    write(&dir, f, theirs);
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, f, ours);
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "merge should conflict");
    dir
}

/// f.txt with lines 2 and 10 changed differently on each side and line 6 changed the same way.
fn two_chunk_conflict(name: &str) -> PathBuf {
    let base = base_lines();
    let ours = base.replace("line 2\n", "line 2 ours\n").replace("line 10\n", "line 10 ours\n").replace("line 6\n", "line 6 both\n");
    let theirs = base.replace("line 2\n", "line 2 theirs\n").replace("line 10\n", "line 10 theirs\n").replace("line 6\n", "line 6 both\n");
    merge_conflict(name, "f.txt", &base, &ours, &theirs)
}

fn kinds(f: &ConflictFile) -> String {
    let ConflictContent::Text(segs) = &f.content else { panic!("expected text, got {:?}", f.content) };
    segs.iter().map(|s| if matches!(s, Segment::Clean(_)) { 'C' } else { 'X' }).collect()
}

fn lines(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| format!("{s}\n")).collect()
}

fn read(dir: &Path, f: &str) -> String {
    std::fs::read_to_string(dir.join(f)).unwrap()
}

#[test]
fn conflict_chunks_from_a_real_merge() {
    let dir = two_chunk_conflict("conflict-chunks");
    let f = load_conflict(&dir, "f.txt").unwrap();
    assert_eq!(kinds(&f), "CXCXC");
    let chunks: Vec<&Chunk> = f.chunks().collect();
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0], &Chunk { base: lines(&["line 2"]), ours: lines(&["line 2 ours"]), theirs: lines(&["line 2 theirs"]) });
    assert_eq!(chunks[1].theirs, lines(&["line 10 theirs"]));
    assert!(!f.no_ancestor);
    assert_eq!(f.labels, SideLabels { operation: Operation::Merge, ours: "main".into(), theirs: "other".into() });
    assert!(f.ids.ancestor.is_some() && f.ids.ours.is_some() && f.ids.theirs.is_some());
    assert!(load_conflict(&dir, "missing.txt").unwrap_err().is_not_found());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn parse_merged_ignores_marker_like_content() {
    let text = "a\n<<<<<<< ours\nmine\n||||||| base\nold\n=======\ntheirs\n>>>>>>> theirs\nb\n";
    let segs = parse_merged(text, 7).unwrap();
    assert_eq!(
        segs,
        [
            Segment::Clean(lines(&["a"])),
            Segment::Conflict(Chunk { base: lines(&["old"]), ours: lines(&["mine"]), theirs: lines(&["theirs"]) }),
            Segment::Clean(lines(&["b"])),
        ]
    );
    assert!(parse_merged("a\n<<<<<<< ours\nmine\n=======\n", 7).is_err(), "unterminated chunk");
    // With no common ancestor the base marker has no label.
    let segs = parse_merged("<<<<<<< ours\nx\n|||||||\n=======\ny\n>>>>>>> theirs\n", 7).unwrap();
    assert_eq!(segs, [Segment::Conflict(Chunk { base: Vec::new(), ours: lines(&["x"]), theirs: lines(&["y"]) })]);
    assert_eq!(marker_size(["x\n=========\n"].into_iter()), 10);
    assert_eq!(marker_size(["x\n"].into_iter()), 7);

    // Content lines that look like markers stay content.
    let base = format!("{}=======\n<<<<<<< x\n", base_lines());
    let ours = base.replace("line 2\n", "line 2 ours\n");
    let theirs = base.replace("line 2\n", "line 2 theirs\n");
    let dir = merge_conflict("conflict-markers", "m.txt", &base, &ours, &theirs);
    let f = load_conflict(&dir, "m.txt").unwrap();
    assert_eq!(kinds(&f), "CXC");
    let ConflictContent::Text(segs) = &f.content else { unreachable!() };
    let Segment::Clean(tail) = &segs[2] else { unreachable!() };
    assert!(tail.ends_with(&lines(&["=======", "<<<<<<< x"])));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn render_output_follows_choices() {
    let dir = two_chunk_conflict("conflict-render");
    let f = load_conflict(&dir, "f.txt").unwrap();
    assert_eq!(render_output(&f, &[Choice::Ours, Choice::Unresolved]), None);
    assert_eq!(render_output(&f, &[Choice::Ours]), None, "one choice per chunk");
    let expected =
        base_lines().replace("line 2\n", "line 2 ours\n").replace("line 6\n", "line 6 both\n").replace("line 10\n", "line 10 theirs\n");
    assert_eq!(render_output(&f, &[Choice::Ours, Choice::Theirs]).as_deref(), Some(expected.as_str()));
    let both = render_output(&f, &[Choice::OursThenTheirs, Choice::TheirsThenOurs]).unwrap();
    assert!(both.contains("line 2 ours\nline 2 theirs\n"));
    assert!(both.contains("line 10 theirs\nline 10 ours\n"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_resolution_stages_and_clears_conflict() {
    let dir = two_chunk_conflict("conflict-write");
    let f = load_conflict(&dir, "f.txt").unwrap();
    let out = render_output(&f, &[Choice::Theirs, Choice::Ours]).unwrap();
    write_resolution(&dir, "f.txt", &out, &f.ids).unwrap();
    assert_eq!(read(&dir, "f.txt"), out);
    let repo = Repository::open(&dir).unwrap();
    assert!(!repo.index().unwrap().has_conflicts());
    let (unstaged, staged, state) = status_at(&dir, true).unwrap();
    assert!(staged.iter().any(|e| e.path == "f.txt" && e.status == 'M'));
    assert!(!unstaged.iter().any(|e| e.path == "f.txt" && e.status == '!'));
    assert_eq!(state, RepositoryState::Merge);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_resolution_refuses_a_changed_conflict() {
    let dir = two_chunk_conflict("conflict-stale");
    let f = load_conflict(&dir, "f.txt").unwrap();
    let before = read(&dir, "f.txt");
    stage_file(&dir, "f.txt").unwrap();
    let err = write_resolution(&dir, "f.txt", "new\n", &f.ids).unwrap_err();
    assert!(err.to_string().contains("changed since it was opened"));
    assert_eq!(read(&dir, "f.txt"), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn add_add_conflict_has_no_ancestor() {
    let dir = scratch_repo("conflict-addadd");
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    write(&dir, "n.txt", "same\ntheirs\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "n.txt", "same\nours\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "ours"]);
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok);
    let f = load_conflict(&dir, "n.txt").unwrap();
    assert!(f.no_ancestor && f.ids.ancestor.is_none());
    assert!(f.chunk_count() >= 1);
    assert!(f.chunks().all(|c| c.base.is_empty()));
    let out = render_output(&f, &vec![Choice::Ours; f.chunk_count()]).unwrap();
    assert_eq!(out, "same\nours\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn binary_and_deleted_conflicts_fall_back() {
    let dir = scratch_repo("conflict-binary");
    write(&dir, "bin.dat", "base\0data");
    write(&dir, "d.txt", "keep\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    write(&dir, "bin.dat", "theirs\0data");
    git(&dir, &["rm", "-q", "d.txt"]);
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "bin.dat", "ours\0data");
    write(&dir, "d.txt", "keep\nchanged\n");
    git(&dir, &["commit", "-q", "-am", "ours"]);
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok);

    let bin = load_conflict(&dir, "bin.dat").unwrap();
    assert_eq!(bin.content, ConflictContent::Binary);
    assert_eq!(bin.chunk_count(), 0);
    let del = load_conflict(&dir, "d.txt").unwrap();
    assert_eq!(del.content, ConflictContent::DeletedBy(Side::Theirs));
    assert!(del.ids.theirs.is_none());

    resolve_with_side(&dir, "d.txt", Side::Theirs, &del.ids).unwrap();
    assert!(!dir.join("d.txt").exists());
    write(&dir, "bin.dat", "something else");
    resolve_with_side(&dir, "bin.dat", Side::Ours, &bin.ids).unwrap();
    assert_eq!(std::fs::read(dir.join("bin.dat")).unwrap(), b"ours\0data");
    let repo = Repository::open(&dir).unwrap();
    assert!(!repo.index().unwrap().has_conflicts());
    // Already resolved: the ids no longer match.
    assert!(resolve_with_side(&dir, "bin.dat", Side::Theirs, &bin.ids).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn crlf_and_missing_final_newline_are_kept() {
    let base = "one\r\ntwo\r\nthree\r\nfour\r\n";
    let dir = merge_conflict("conflict-crlf", "w.txt", base, &base.replace("two", "two ours"), &base.replace("two", "two theirs"));
    let f = load_conflict(&dir, "w.txt").unwrap();
    let out = render_output(&f, &[Choice::Theirs]).unwrap();
    assert_eq!(out, "one\r\ntwo theirs\r\nthree\r\nfour\r\n");
    let _ = std::fs::remove_dir_all(&dir);

    let dir = merge_conflict("conflict-eofnl", "e.txt", "a\nb\nc\n", "a\nb\nours\n", "a\nb\ntheirs");
    let f = load_conflict(&dir, "e.txt").unwrap();
    assert!(matches!(f.content, ConflictContent::Text(ref s) if matches!(s.last(), Some(Segment::Conflict(_)))));
    assert_eq!(render_output(&f, &[Choice::Theirs]).unwrap(), "a\nb\ntheirs");
    assert_eq!(render_output(&f, &[Choice::Ours]).unwrap(), "a\nb\nours\n");
    assert_eq!(render_output(&f, &[Choice::TheirsThenOurs]).unwrap(), "a\nb\ntheirs\nours\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rebase_labels() {
    let dir = scratch_repo("conflict-rebase");
    write(&dir, "r.txt", "base\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    write(&dir, "r.txt", "feature\n");
    git(&dir, &["commit", "-q", "-am", "feature change"]);
    let feature_tip = Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "r.txt", "main\n");
    git(&dir, &["commit", "-q", "-am", "main change"]);
    git(&dir, &["checkout", "-q", "feature"]);
    let (ok, _) = run(&dir, &args(&["rebase", "main"]));
    assert!(!ok, "rebase should conflict");
    let f = load_conflict(&dir, "r.txt").unwrap();
    assert_eq!(f.labels, SideLabels { operation: Operation::Rebase, ours: "main".into(), theirs: "feature".into() });
    git(&dir, &["rebase", "--abort"]);

    // Cherry-picking the feature commit onto main.
    git(&dir, &["checkout", "-q", "main"]);
    let (ok, _) = run(&dir, &args(&["cherry-pick", &feature_tip.to_string()]));
    assert!(!ok, "cherry-pick should conflict");
    let f = load_conflict(&dir, "r.txt").unwrap();
    assert_eq!(f.labels.operation, Operation::CherryPick);
    assert_eq!(f.labels.ours, "main");
    assert!(f.labels.theirs.starts_with(&feature_tip.to_string()[..7]), "{}", f.labels.theirs);
    assert!(f.labels.theirs.ends_with("feature change"));
    let _ = std::fs::remove_dir_all(&dir);
}
