//! Taking the trivial picks in several files is one undo entry: undone it brings every
//! conflict back exactly, redone it resolves the files again, and it is refused once a file
//! changed.

use super::marked_tests::{apply, bytes, refusal, stages};
use super::*;
use crate::repo::trivial_plan;
use crate::testkit::trivial::mixed_trivial_merge;
use crate::testutil::*;

#[test]
fn taking_trivial_picks_is_one_entry_undone_and_redone() {
    let dir = scratch_repo("undo-trivial");
    mixed_trivial_merge(&dir);
    let files = ["tidy.txt", "also.txt"];
    let before: Vec<_> = files.iter().map(|f| (stages(&dir, f), bytes(&dir, f))).collect();

    let plan = trivial_plan(&dir, &["tidy.txt".to_owned(), "also.txt".to_owned(), "mixed.txt".to_owned()]);
    let paths = plan.paths();
    let a = conflicts_saving(&dir, ConflictChange::TrivialPicks, &paths, || plan.apply(&dir)).unwrap().expect("undoable");
    assert!(matches!(&a, Action::Conflicts { change: ConflictChange::TrivialPicks, files, .. } if files.len() == 2));
    assert_eq!(a.describe(), "taking the trivial picks in 2 files");
    let resolved: Vec<_> = files.iter().map(|f| (stages(&dir, f), bytes(&dir, f))).collect();

    assert_eq!(apply(&dir, &a, false), "Brought back the conflicts in 2 files");
    let now: Vec<_> = files.iter().map(|f| (stages(&dir, f), bytes(&dir, f))).collect();
    assert_eq!(now, before, "both files as they were");
    assert_eq!(apply(&dir, &a, true), "Resolved 2 files again with the trivial picks");
    let again: Vec<_> = files.iter().map(|f| (stages(&dir, f), bytes(&dir, f))).collect();
    assert_eq!(again, resolved);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_entry_is_refused_once_a_file_changed() {
    let dir = scratch_repo("undo-trivial-stale");
    mixed_trivial_merge(&dir);
    let plan = trivial_plan(&dir, &["tidy.txt".to_owned(), "also.txt".to_owned()]);
    let a = conflicts_saving(&dir, ConflictChange::TrivialPicks, &plan.paths(), || plan.apply(&dir)).unwrap().expect("undoable");
    let edited = "edited\n";
    write(&dir, "tidy.txt", edited);
    let also = (stages(&dir, "also.txt"), bytes(&dir, "also.txt"));
    refusal(&dir, &a, false);
    assert_eq!(std::fs::read_to_string(dir.join("tidy.txt")).unwrap(), edited);
    assert_eq!((stages(&dir, "also.txt"), bytes(&dir, "also.txt")), also, "nothing written");
    let _ = std::fs::remove_dir_all(&dir);
}
