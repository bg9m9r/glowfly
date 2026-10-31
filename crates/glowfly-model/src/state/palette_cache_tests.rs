//! The palette's command cache: what it keeps between frames, what makes it build again, and
//! that what it lists is always what `palette::listed` builds from scratch.

use super::super::{Listed, Mode, Palette, Pick, PickKind, listed, listed_open, row_count};
use super::*;
use glowfly_core::settings::{DiffLayout, Settings};
use std::time::Instant;

/// A state with a one-commit repository open, two more local branches and `tags` tags.
fn state(name: &str, tags: usize) -> (AppState, std::path::PathBuf) {
    let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
    let mut snap = crate::state::op_fixture::snapshot(&format!("palette-cache-{name}"));
    let dir = snap.workdir.clone();
    let head = snap.head_oid.expect("born");
    let main = snap.locals.iter().find(|b| b.is_head).expect("head branch").clone();
    for n in ["feature/cache", "fix/checkout"] {
        snap.locals.push(BranchInfo { name: n.into(), is_head: false, ..main.clone() });
    }
    snap.tags = (0..tags).map(|i| (format!("v1.{i}"), head)).collect();
    st.snap = Some(snap);
    (st, dir)
}

/// The list as plain data: (label, detail, matched) per row, the total and the groups.
type Plain = (Vec<(String, Option<String>, Vec<usize>)>, usize, Vec<(Group, usize)>);

fn plain(l: Listed) -> Plain {
    let rows = l.rows.into_iter().map(|s| (s.entry.label, s.entry.detail, s.matched)).collect();
    (rows, l.total, l.groups)
}

/// The cached list for `query` and the list built from scratch, which must be equal.
fn check(cache: &mut CommandCache, st: &AppState, query: &str) -> Plain {
    let cached = plain(cache.listed(st, query));
    assert_eq!(cached, plain(listed(st, query)), "cached list for {query:?} is the fresh one");
    cached
}

#[test]
fn frames_with_the_same_inputs_reuse_the_entries_and_ranking() {
    let (st, dir) = state("same", 300);
    let mut cache = CommandCache::default();
    for query in ["", "", ""] {
        check(&mut cache, &st, query);
    }
    assert_eq!((cache.ref_builds, cache.rankings), (1, 1), "built once for three frames");
    for _ in 0..3 {
        check(&mut cache, &st, "v1.2");
    }
    assert_eq!((cache.ref_builds, cache.rankings), (1, 2), "a new query ranks once more, the entries stay");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_changed_ref_rebuilds_the_checkout_entries() {
    let (mut st, dir) = state("refs", 20);
    let mut cache = CommandCache::default();
    check(&mut cache, &st, "checkout");
    let head = st.snap.as_ref().and_then(|s| s.head_oid).expect("born");
    st.snap.as_mut().expect("open").tags.push(("v9.9".into(), head));
    let (rows, ..) = check(&mut cache, &st, "checkout");
    assert!(rows.iter().any(|r| r.0 == "Checkout tag v9.9"), "the new tag is listed");
    assert_eq!((cache.ref_builds, cache.rankings), (2, 2));

    let s = st.snap.as_mut().expect("open");
    s.locals.retain(|b| b.name != "fix/checkout");
    check(&mut cache, &st, "checkout");
    assert_eq!(cache.ref_builds, 3, "a deleted branch");

    let snap = st.snap.take();
    check(&mut cache, &st, "checkout");
    assert_eq!(cache.ref_builds, 4, "the repository closed");
    st.snap = snap;
    check(&mut cache, &st, "checkout");
    assert_eq!(cache.ref_builds, 5, "and opened again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn another_query_ranks_again_and_narrowing_matches_a_full_ranking() {
    let (st, dir) = state("query", 120);
    let mut cache = CommandCache::default();
    let mut rankings = 0;
    for query in ["c", "ch", "che", "check out", "checkout v1.1", "checkout v1.10", "checkout", "", "zzz", "zzzq", "v"] {
        check(&mut cache, &st, query);
        rankings += 1;
        assert_eq!(cache.rankings, rankings, "{query:?} ranked");
        check(&mut cache, &st, query);
        assert_eq!(cache.rankings, rankings, "{query:?} reused");
    }
    assert_eq!(cache.ref_builds, 1, "typing never rebuilds the entries");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_changed_command_or_recent_list_ranks_again_without_rebuilding_refs() {
    let (mut st, dir) = state("fixed", 50);
    let mut cache = CommandCache::default();
    check(&mut cache, &st, "");
    st.settings.diff_layout = DiffLayout::Split;
    let (rows, ..) = check(&mut cache, &st, "");
    assert!(rows.iter().any(|r| r.0 == "Use inline diff layout"), "the entry reads the new layout");
    assert_eq!((cache.ref_builds, cache.rankings), (1, 2));

    st.palette_recent.record("refresh");
    let (rows, _, groups) = check(&mut cache, &st, "");
    assert_eq!(rows[0].0, "Refresh", "the recent command comes first");
    assert_eq!(groups.first().map(|g| g.0), Some(Group::Recent));
    assert_eq!((cache.ref_builds, cache.rankings), (1, 3));

    st.snap.as_mut().expect("open").unstaged = crate::state::op_fixture::rows(&[("a.txt", 'M')]);
    check(&mut cache, &st, "");
    assert_eq!((cache.ref_builds, cache.rankings), (1, 4), "Stage all files is listed and suggested now");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_open_palette_keeps_its_list_across_frames_and_modes() {
    let (mut st, dir) = state("open", 30);
    st.palette = Some(Palette { query: "tag".into(), ..Default::default() });
    let fresh = plain(listed(&st, "tag"));
    assert_eq!(plain(listed_open(&mut st)), fresh);
    assert_eq!(row_count(&mut st), fresh.0.len());
    let p = st.palette.as_mut().expect("open");
    assert_eq!((p.commands.ref_builds, p.commands.rankings), (1, 1), "the view's frame and update share one ranking");

    p.mode = Mode::Pick(Pick::new(PickKind::Blame));
    p.mode = Mode::Commands;
    p.query = "checkout v1.2".into();
    assert_eq!(plain(listed_open(&mut st)), plain(listed(&st, "checkout v1.2")), "back from a sub-list");
    let p = st.palette.as_ref().expect("open");
    assert_eq!((p.commands.ref_builds, p.commands.rankings), (1, 2));

    st.palette = None;
    assert!(listed_open(&mut st).rows.is_empty(), "nothing while closed");
    let _ = std::fs::remove_dir_all(&dir);
}
