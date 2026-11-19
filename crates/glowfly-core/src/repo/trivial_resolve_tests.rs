use super::*;
use crate::conflict_pick::ChunkPick;
use crate::conflict_restore::SavedPicks;
use crate::gitcli::{args, run};
use crate::repo::{Chunk, Operation, Segment, SideLabels, resolved_paths};
use crate::testkit::trivial::mixed_trivial_merge;
use crate::testutil::*;
use std::path::PathBuf;

fn merging(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    mixed_trivial_merge(&dir);
    dir
}

fn all() -> Vec<String> {
    ["tidy.txt", "mixed.txt", "logo.bin", "also.txt"].map(String::from).to_vec()
}

fn stages(dir: &Path, f: &str) -> String {
    let (ok, out) = run(dir, &args(&["ls-files", "-s", "--", f]));
    assert!(ok, "{out}");
    out
}

fn bytes(dir: &Path, f: &str) -> Vec<u8> {
    std::fs::read(dir.join(f)).unwrap()
}

#[test]
fn plan_sorts_a_mixed_merge() {
    let dir = merging("trivial-plan");
    let plan = trivial_plan(&dir, &all());
    assert_eq!(plan.paths(), ["tidy.txt", "also.txt"], "in input order");
    assert_eq!(plan.left, ["mixed.txt"]);
    assert_eq!(plan.skipped, [("logo.bin".to_owned(), TrivialSkip::Binary)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apply_resolves_only_the_planned_files_and_keeps_resolve_undo() {
    let dir = merging("trivial-apply");
    let (mixed, logo) = (stages(&dir, "mixed.txt"), stages(&dir, "logo.bin"));
    let (mixed_work, logo_work) = (bytes(&dir, "mixed.txt"), bytes(&dir, "logo.bin"));
    let plan = trivial_plan(&dir, &all());
    plan.apply(&dir).unwrap();
    for f in ["tidy.txt", "also.txt"] {
        let s = stages(&dir, f);
        assert!(s.lines().count() == 1 && s.contains(" 0\t"), "{f}: {s}");
        let text = std::fs::read_to_string(dir.join(f)).unwrap().replace("\r\n", "\n");
        assert_eq!(text.lines().nth(1), Some("    let x = a + b;"), "{f}");
    }
    let resolved = resolved_paths(&dir).unwrap();
    let mut names: Vec<_> = resolved.iter().map(|r| (r.entry.path.as_str(), r.both_sides)).collect();
    names.sort();
    assert_eq!(names, [("also.txt", true), ("tidy.txt", true)]);
    assert_eq!((stages(&dir, "mixed.txt"), stages(&dir, "logo.bin")), (mixed, logo));
    assert_eq!((bytes(&dir, "mixed.txt"), bytes(&dir, "logo.bin")), (mixed_work, logo_work));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_working_copy_that_isnt_as_git_wrote_it_is_skipped() {
    let dir = merging("trivial-edited");
    let paths = ["tidy.txt".to_owned()];
    let before = stages(&dir, "tidy.txt");
    let text = std::fs::read_to_string(dir.join("tidy.txt")).unwrap();

    write(&dir, "tidy.txt", &format!("<<<<<<< x\n{text}"));
    let plan = trivial_plan(&dir, &paths);
    assert_eq!((plan.paths(), plan.skipped), (vec![], vec![("tidy.txt".to_owned(), TrivialSkip::Edited)]));

    let ours: String = (0..14).map(|i| if i == 1 { "    let x = a + b;\n".to_owned() } else { format!("line {}\n", i + 1) }).collect();
    write(&dir, "tidy.txt", &ours);
    let plan = trivial_plan(&dir, &paths);
    assert_eq!(plan.skipped, [("tidy.txt".to_owned(), TrivialSkip::SavedPicks)]);

    std::fs::remove_file(dir.join("tidy.txt")).unwrap();
    let plan = trivial_plan(&dir, &paths);
    assert_eq!((plan.paths(), plan.skipped), (vec![], vec![("tidy.txt".to_owned(), TrivialSkip::Unreadable)]));
    assert_eq!(stages(&dir, "tidy.txt"), before, "nothing changed in the index");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_git_converts_is_skipped_and_named() {
    for (attrs, named) in [("tidy.txt filter=noop\n", "filter=noop"), ("tidy.txt ident\n", "ident")] {
        let dir = merging(&format!("trivial-converted-{}", &named[..5]));
        write(&dir, ".gitattributes", attrs);
        let plan = trivial_plan(&dir, &["tidy.txt".to_owned()]);
        assert_eq!(plan.skipped, [("tidy.txt".to_owned(), TrivialSkip::Converted(named.to_owned()))]);
        assert!(plan.paths().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn paths_no_longer_conflicted_are_dropped_and_other_conflicts_are_not_text() {
    let dir = merging("trivial-dropped");
    git(&dir, &["add", "mixed.txt"]);
    let plan = trivial_plan(&dir, &["mixed.txt".to_owned(), "nothing.txt".to_owned()]);
    assert!(plan.paths().is_empty() && plan.left.is_empty() && plan.skipped.is_empty());
    let _ = std::fs::remove_dir_all(&dir);

    // Changed on one side, deleted on the other.
    let dir = scratch_repo("trivial-deleted");
    write(&dir, "gone.txt", "one\ntwo\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    git(&dir, &["rm", "-q", "gone.txt"]);
    git(&dir, &["commit", "-q", "-m", "delete"]);
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "gone.txt", "one\n2\n");
    git(&dir, &["commit", "-q", "-am", "change"]);
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok);
    let plan = trivial_plan(&dir, &["gone.txt".to_owned()]);
    assert_eq!(plan.skipped, [("gone.txt".to_owned(), TrivialSkip::NotText)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_text_conflict_without_chunks_is_left() {
    let file = ConflictFile {
        path: "f.txt".into(),
        ids: ConflictIds { ancestor: None, ours: None, theirs: None },
        labels: SideLabels { operation: Operation::Merge, ours: "main".into(), theirs: "other".into() },
        content: ConflictContent::Text(vec![Segment::Clean(vec!["a\n".into()])]),
        no_ancestor: false,
        base_markers: false,
    };
    assert!(matches!(decide(&file, WorkingCopy::Untouched), Decided::Left));
    let chunk = Chunk { base: vec!["a\n".into()], ours: vec!["b\n".into()], theirs: vec!["a\n".into()] };
    let with_chunk = ConflictFile { content: ConflictContent::Text(vec![Segment::Conflict(chunk)]), ..file };
    assert!(matches!(decide(&with_chunk, WorkingCopy::Untouched), Decided::Resolve(c) if c == "b\n"));
    let saved = SavedPicks { picks: vec![ChunkPick::default()], cleans: Default::default() };
    assert!(matches!(decide(&with_chunk, WorkingCopy::Saved(saved)), Decided::Skip(TrivialSkip::SavedPicks)));
}
