//! Threads placed on the local version's lines (Placement::Mapped): their markers, the texts
//! that locate them, and going to them.

use super::tests::{oid, shown_at, synced, thread};
use super::*;
use crate::state::pull_request_line_map::{LineMaps, wanted};
use glowfly_core::forge::Provider;
use glowfly_core::repo::{LineChange, LineMap, PrLineMaps};

/// The head's first two lines are gone locally: every later line is two lines up.
fn up_two(path: &str) -> LineMap {
    LineMap { path: Some(path.into()), changes: Some(vec![LineChange { old_start: 1, old_lines: 2, new_start: 0, new_lines: 0 }]) }
}

/// `synced` with the files loaded from source tip 4 instead, and `files` mapped onto it.
fn mapped(threads: Vec<ReviewThread>, old_kept: bool, files: &[(&str, LineMap)]) -> PrView {
    let mut v = synced(threads);
    shown_at(&mut v, Some(oid(4)));
    let files = files.iter().map(|(p, m)| ((*p).to_owned(), m.clone())).collect();
    v.lines = LineMaps::Loaded { key: wanted(&v).unwrap(), maps: PrLineMaps::Mapped { old_kept, files } };
    v
}

fn short() -> String {
    oid(2).to_string().to_uppercase()[..7].to_owned()
}

fn old(mut t: ReviewThread) -> ReviewThread {
    t.side = LineSide::Old;
    t
}

#[test]
fn marks_on_mapped_lines() {
    let threads = vec![
        thread("T4", "src/a.rs", Some(4), 0),
        thread("T6", "src/a.rs", Some(6), 0),
        thread("GONE", "src/a.rs", Some(2), 0),
        thread("R", "src/new.rs", Some(3), 0),
        old(thread("O", "src/a.rs", Some(9), 0)),
    ];
    let files = [("src/a.rs", up_two("src/a.rs")), ("src/new.rs", LineMap::same("src/b.rs"))];
    let v = mapped(threads.clone(), true, &files);
    assert!(matches!(placement(&v), Placement::Mapped(_)));
    let m = marks(&v, "src/a.rs").unwrap();
    let mut keys: Vec<_> = m.keys().copied().collect();
    keys.sort_by_key(|(s, n)| (*s == LineSide::New, *n));
    assert_eq!(keys, [(LineSide::Old, 9), (LineSide::New, 2), (LineSide::New, 4)], "the changed line has no marker");
    assert_eq!(m[&(LineSide::New, 2)].thread, "T4");
    let b = marks(&v, "src/b.rs").unwrap();
    assert_eq!(b[&(LineSide::New, 3)].thread, "R", "a renamed file's threads are on its local path");
    assert_eq!(marks(&v, "src/new.rs").unwrap().len(), 0);
    let v = mapped(threads, false, &files);
    assert!(!marks(&v, "src/a.rs").unwrap().contains_key(&(LineSide::Old, 9)), "the merge base moved");
}

#[test]
fn where_text_mapped_forms() {
    let s = short();
    let files = [("src/a.rs", up_two("src/a.rs")), ("src/gone.rs", LineMap::gone())];
    let v = mapped(Vec::new(), false, &files);
    let mut t = thread("T", "src/a.rs", Some(42), 0);
    assert_eq!(where_text(&v, &t), format!("line 40 (line 42 on {s})"));
    t.start_line = Some(40);
    assert_eq!(where_text(&v, &t), format!("lines 38\u{2013}40 (lines 40\u{2013}42 on {s})"));
    t.start_line = Some(2);
    assert_eq!(where_text(&v, &t), format!("line 40 (lines 2\u{2013}42 on {s})"), "a range whose first line changed");
    assert_eq!(where_text(&v, &thread("T", "src/gone.rs", Some(42), 0)), format!("line 42 on {s}, not in the local version"));
    assert_eq!(where_text(&v, &old(thread("T", "src/a.rs", Some(12), 0))), format!("deleted line 12 on {s}, not in the local version"));
    assert_eq!(where_text(&v, &thread("T", "src/gone.rs", None, 0)), format!("the file on {s}, not in the local version"));
    let kept = mapped(Vec::new(), true, &[("src/a.rs", LineMap::same("src/a.rs"))]);
    assert_eq!(where_text(&kept, &old(thread("T", "src/a.rs", Some(12), 0))), "deleted line 12");
    assert_eq!(where_text(&kept, &thread("T", "src/a.rs", Some(7), 0)), "line 7", "the same numbers");
}

#[test]
fn go_to_mapped_lines() {
    let files = [("src/a.rs", up_two("src/a.rs")), ("src/new.rs", LineMap::same("src/b.rs")), ("src/gone.rs", LineMap::gone())];
    let v = mapped(Vec::new(), true, &files);
    let (target, side, n) = go_to(&v, &thread("T", "src/a.rs", Some(6), 0)).unwrap();
    assert_eq!((target, side, n), (DiffTarget::Between(oid(3), oid(4), "src/a.rs".into()), LineSide::New, 4));
    assert_eq!(
        go_to(&v, &thread("T", "src/a.rs", Some(1), 0)).unwrap_err(),
        "The commented line was changed or removed in the local version"
    );
    let (target, _, n) = go_to(&v, &thread("T", "src/new.rs", Some(5), 0)).unwrap();
    assert_eq!((target, n), (DiffTarget::Between(oid(3), oid(4), "src/b.rs".into()), 5), "a renamed file opens its local path");
    assert_eq!(go_to(&v, &thread("T", "src/gone.rs", None, 0)).unwrap_err(), "src/gone.rs isn't in the local version");
}

#[test]
fn a_new_comment_stays_blocked_while_mapped() {
    let v = mapped(Vec::new(), true, &[]);
    assert!(matches!(placement(&v), Placement::Mapped(_)));
    let why = crate::state::pull_request_line_comment::start_blocked(Some(Provider::GitHub), &v);
    assert!(why.is_some_and(|w| w.starts_with("The service's head is")));
}
