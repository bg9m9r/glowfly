use super::*;
use crate::state::pull_request::{Page, Tips};
use crate::state::pull_request_actions::tests::{detail, view};
use glowfly_core::forge::ThreadComment;
use glowfly_core::repo::{BranchChanges, Comparison, FileEntry, LineChange, SourceFrom, SourceTip};

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap()
}

fn thread(id: &str, path: &str, line: Option<u32>) -> ReviewThread {
    ReviewThread {
        id: id.into(),
        path: path.into(),
        side: LineSide::New,
        line,
        start_line: None,
        original_line: None,
        outdated: false,
        resolved: false,
        resolved_by: None,
        can_reply: true,
        can_resolve: true,
        can_unresolve: false,
        context: String::new(),
        comments: vec![ThreadComment { id: Some(1), author: "ada".into(), time: 0, body: "Hm".into(), url: String::new(), pending: false }],
        more: 0,
    }
}

/// A view whose page names head 2 and whose files are loaded from source tip `local`, with
/// merge base 3 and target 1.
fn at(local: Oid, threads: Vec<ReviewThread>) -> PrView {
    let mut d = detail();
    d.head_sha = oid(2).to_string();
    let mut v = view(Some(d));
    if let Page::Loaded(p) = &mut v.page {
        p.threads = threads;
    }
    v.tips = Tips { target: Some(oid(1)), source: Some(local) };
    let files = vec![FileEntry::new("src/a.rs", 'M'), FileEntry::new("src/old.rs", 'M')];
    let changes = BranchChanges { base: oid(3), comparison: Comparison { files, absent: Default::default() } };
    v.set_loaded(changes, SourceTip { oid: local, from: SourceFrom::Branch }, Default::default());
    v
}

/// `at` tip 4, with `files` mapped for the current key.
fn mapped(threads: Vec<ReviewThread>, old_kept: bool, files: &[(&str, LineMap)]) -> PrView {
    let mut v = at(oid(4), threads);
    let files = files.iter().map(|(p, m)| ((*p).to_owned(), m.clone())).collect();
    v.lines = LineMaps::Loaded { key: wanted(&v).unwrap(), maps: PrLineMaps::Mapped { old_kept, files } };
    v
}

/// Head lines 1 and 2 are gone locally; the rest moved up two.
fn up_two(path: &str) -> LineMap {
    LineMap { path: Some(path.into()), changes: Some(vec![LineChange { old_start: 1, old_lines: 2, new_start: 0, new_lines: 0 }]) }
}

#[test]
fn wanted_only_when_the_files_are_of_another_tip() {
    let mut old = thread("O", "src/z.rs", Some(1));
    old.outdated = true;
    let threads = vec![thread("B", "src/b.rs", Some(1)), thread("A", "src/a.rs", None), thread("A2", "src/a.rs", Some(3)), old];
    let key = wanted(&at(oid(4), threads.clone())).unwrap();
    assert_eq!(key.head, oid(2).to_string());
    assert_eq!((key.local, key.base, key.target), (oid(4), oid(3), oid(1)));
    assert_eq!(key.paths, ["src/a.rs", "src/b.rs"], "sorted, once each, outdated left out");

    let mut synced = at(oid(2), threads.clone());
    if let Page::Loaded(p) = &mut synced.page {
        p.detail.head_sha = oid(2).to_string().to_uppercase();
    }
    assert_eq!(wanted(&synced), None, "synced, ignoring case");
    let mut loading = at(oid(4), threads.clone());
    loading.files = Files::Loading;
    assert_eq!(wanted(&loading), None, "files loading");
    let mut no_page = at(oid(4), threads.clone());
    no_page.page = Page::Loading;
    assert_eq!(wanted(&no_page), None, "no page");
    let mut odd = at(oid(4), threads);
    if let Page::Loaded(p) = &mut odd.page {
        p.detail.head_sha = "not-a-sha".into();
    }
    assert_eq!(wanted(&odd), None, "the head isn't a commit id");
}

#[test]
fn current_ignores_maps_for_another_key() {
    let mut v = mapped(vec![thread("A", "src/a.rs", Some(4))], true, &[("src/a.rs", up_two("src/a.rs"))]);
    assert!(current(&v).is_some());
    shown_at(&mut v, oid(5));
    assert_eq!(current(&v), None, "the local tip moved");
    assert!(spot(&v, &thread("A", "src/a.rs", Some(4))).is_none());
}

fn shown_at(v: &mut PrView, tip: Oid) {
    v.shown.as_mut().unwrap().source = SourceTip { oid: tip, from: SourceFrom::Branch };
}

#[test]
fn spots_shift_collapse_and_keep_old_lines() {
    let files = [("src/a.rs", up_two("src/a.rs")), ("src/gone.rs", LineMap::gone())];
    let v = mapped(Vec::new(), true, &files);
    let s = spot(&v, &thread("T", "src/a.rs", Some(6))).unwrap();
    assert_eq!((s.path.as_str(), s.side, s.start_line, s.line), ("src/a.rs", LineSide::New, None, Some(4)));
    let mut range = thread("T", "src/a.rs", Some(6));
    range.start_line = Some(2);
    let s = spot(&v, &range).unwrap();
    assert_eq!((s.start_line, s.line), (None, Some(4)), "a range whose first line changed is its last line");
    range.start_line = Some(4);
    assert_eq!(spot(&v, &range).unwrap().start_line, Some(2));
    let mut old = thread("T", "src/a.rs", Some(12));
    old.side = LineSide::Old;
    assert_eq!(spot(&v, &old).unwrap().line, Some(12), "the merge base is the same");
    assert_eq!(spot(&mapped(Vec::new(), false, &files), &old), None, "the merge base moved");
    assert_eq!(spot(&v, &thread("T", "src/gone.rs", None)), None, "a whole-file comment on a gone file");
    assert_eq!(spot(&v, &thread("T", "src/a.rs", None)).unwrap().line, None, "a whole-file comment");
    let mut outdated = thread("T", "src/a.rs", Some(6));
    outdated.outdated = true;
    assert_eq!(spot(&v, &outdated), None);
}

#[test]
fn deleted_lines_of_a_file_the_head_deletes_keep_their_place() {
    let files = [("d.txt", LineMap::gone())];
    let mut old = thread("T", "d.txt", Some(5));
    old.side = LineSide::Old;
    let s = spot(&mapped(Vec::new(), true, &files), &old).unwrap();
    assert_eq!((s.path.as_str(), s.side, s.line), ("d.txt", LineSide::Old, Some(5)), "the merge base's file, by its own name");
    assert_eq!(spot(&mapped(Vec::new(), false, &files), &old), None, "the merge base moved");
    assert_eq!(spot(&mapped(Vec::new(), true, &files), &thread("T", "d.txt", Some(5))), None, "an added line of a gone file");
}

#[test]
fn head_path_finds_a_renamed_files_threads() {
    let threads = vec![thread("R", "src/new.rs", Some(1)), thread("A", "src/a.rs", Some(1))];
    let files = [("src/new.rs", LineMap::same("src/old.rs")), ("src/a.rs", LineMap::same("src/a.rs"))];
    let v = mapped(threads.clone(), true, &files);
    assert_eq!(head_path(&v, "src/old.rs"), "src/new.rs");
    assert_eq!(head_path(&v, "src/a.rs"), "src/a.rs");
    assert_eq!(head_path(&v, "src/other.rs"), "src/other.rs");
    assert_eq!(head_path(&at(oid(4), threads.clone()), "src/old.rs"), "src/old.rs", "nothing mapped yet");
    // The Files changed pane opens for the local path's threads, so a renamed file's opens.
    assert_eq!(on_local_file(&v, "src/old.rs"), 1, "a renamed file's threads open its pane");
    assert_eq!(on_local_file(&v, "src/a.rs"), 1);
    assert_eq!(on_local_file(&v, "src/new.rs"), 1, "no local file takes the head's name, so it keeps its threads");
    assert_eq!(on_local_file(&at(oid(4), threads), "src/old.rs"), 0, "nothing mapped yet");
    assert_eq!(
        mapped_note(&v, oid(4)),
        format!(
            "These comments were made on {} and are shown on the local version ({}).",
            &oid(2).to_string()[..7],
            &oid(4).to_string()[..7]
        )
    );
}
