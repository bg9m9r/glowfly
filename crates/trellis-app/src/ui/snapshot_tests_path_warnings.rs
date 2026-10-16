//! The working tree with paths Trellis won't act on: the banner about index paths that share
//! one file on this disk (and a core.ignorecase that disagrees with it), and the notes after a
//! case twin's and a non-UTF-8 name's paths.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use trellis_core::repo::{Collision, FileEntry, Folding, Hazard};
use trellis_core::settings::ThemeChoice;

#[test]
fn path_warnings() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "path-warnings");
    let snap = st.snap.as_mut().unwrap();
    snap.collisions = vec![Collision { paths: vec!["Docs/Readme.md".into(), "docs/readme.md".into()], case: true, unicode: false }];
    snap.folding = Folding { case: Some(true), unicode: Some(false) };
    snap.ignorecase = false;
    let twin = Hazard::Twins(vec!["Docs/Readme.md".into()]);
    snap.unstaged.push(FileEntry { hazard: Some(twin), ..FileEntry::new("docs/readme.md", 'M') });
    let odd = Hazard::NotUtf8(b"caf\xe9.txt".to_vec());
    snap.unstaged.push(FileEntry { hazard: Some(odd), ..FileEntry::new("caf\\xE9.txt", '?') });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("path_warnings");
    assert!(h.query_by_label_contains("One file on this disk").is_some());
    assert!(h.query_by_label_contains("core.ignorecase is false").is_some());
    assert!(h.query_by_label("(name isn't UTF-8)").is_some());
    assert!(h.query_by_label("(same file as another path)").is_some());
    drop(h);
    remove_fixture(&dir);
}
