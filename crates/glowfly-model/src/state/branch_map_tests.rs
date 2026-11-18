//! The Branch map's remembered trial merges and its words, on maps built by hand.

use super::*;
use git2::Oid;
use glowfly_core::branch_map::{Pair, Row};

pub(crate) fn oid(n: u32) -> Oid {
    Oid::from_str(&format!("{n:040x}")).unwrap()
}

fn row(name: &str, n: u32, ahead: usize, behind: usize, files: usize) -> Row {
    Row {
        branch: BranchRef::Local(name.into()),
        tip: oid(n),
        time: 1_000 - i64::from(n),
        ahead,
        behind,
        base: Some(oid(900)),
        files: (0..files).map(|i| format!("f{i}.txt")).collect(),
        files_total: files,
    }
}

/// main, `feature` (3 behind, 5 ahead, 4 files; checked out), `merged` and `same`.
pub(crate) fn sample() -> BranchMap {
    let mut unrelated = row("island", 5, 2, 0, 0);
    unrelated.base = None;
    BranchMap {
        rows: vec![row("main", 1, 0, 0, 0), row("feature", 2, 5, 3, 4), row("merged", 3, 0, 4, 0), row("same", 4, 0, 0, 0), unrelated],
        pairs: vec![Pair { a: 0, b: 1, shared: vec!["a.txt".into()], forecast: Forecast::Conflicts(vec!["a.txt".into(), "b.txt".into()]) }],
        current: Some(1),
        total: 4,
        merged: 1,
    }
}

#[test]
fn rows_say_their_numbers_in_every_shape() {
    let (t, m) = (Lang::En, sample());
    assert_eq!(stats_line(t, &m, 0), "Every other branch is measured against this one");
    assert_eq!(stats_line(t, &m, 1), "3 behind · 5 ahead · 4 files changed");
    assert_eq!(stats_line(t, &m, 2), "4 behind · merged into main");
    assert_eq!(stats_line(t, &m, 3), "Same commit as main");
    assert_eq!(stats_line(t, &m, 4), "No shared history with main");
}

#[test]
fn a_row_is_named_with_the_same_numbers() {
    let (t, m) = (Lang::En, sample());
    assert_eq!(
        row_name(t, &m, 1, "2 days ago"),
        "feature, checked out: 3 behind, 5 ahead, 4 files changed, last commit 2 days ago, would conflict with main"
    );
    assert_eq!(row_name(t, &m, 0, "today"), "main, the main branch: last commit today");
    assert_eq!(row_name(t, &m, 2, "3 days ago"), "merged: 4 behind, merged into main, last commit 3 days ago");
}

#[test]
fn the_conflict_tip_counts_the_files() {
    let (t, m) = (Lang::En, sample());
    assert_eq!(conflict_tip(t, &m, 1).as_deref(), Some("Merging feature and main would stop with conflicts in 2 files."));
    assert_eq!(conflict_tip(t, &m, 2), None);
    assert_eq!(badge(t, &Forecast::Clean), "No conflicts");
    assert_eq!(badge(t, &Forecast::Conflicts(vec!["x".into()])), "Conflicts in 1 file");
    assert_eq!(badge(t, &Forecast::Unknown("why".into())), "Couldn't check");
    assert_eq!(short_base(&m, 1).as_deref(), Some("0000000"));
    assert_eq!(short_base(&m, 4), None);
}

fn view(map: BranchMap, loading: bool) -> BranchMapView {
    BranchMapView { generation: 1, loading, plan: Err(Empty::OneBranch), result: Some(Ok(Box::new(map))), selected: None }
}

#[test]
fn the_subtitle_names_the_main_branch_and_the_cap() {
    let (t, mut m) = (Lang::En, sample());
    assert_eq!(subtitle(t, &view(m.clone(), false)), "All branches, measured against main");
    m.total = 57;
    assert_eq!(subtitle(t, &view(m.clone(), false)), "The 4 most recently committed of 57 branches, measured against main");
    assert_eq!(subtitle(t, &view(m, true)), "Updating…");
}

#[test]
fn forecasts_are_remembered_by_tip_pair_but_not_the_unknown_ones() {
    let mut m = sample();
    m.pairs.push(Pair { a: 1, b: 2, shared: vec!["c.txt".into()], forecast: Forecast::Clean });
    m.pairs.push(Pair { a: 1, b: 3, shared: vec!["d.txt".into()], forecast: Forecast::Unknown("x".into()) });
    let mut slot = BranchMapSlot::default();
    slot.remember(&m);
    assert_eq!(slot.known.len(), 2);
    assert!(matches!(slot.known.get(&pair_key(oid(2), oid(1))), Some(Forecast::Conflicts(_))));
    assert_eq!(slot.known.get(&pair_key(oid(2), oid(3))), Some(&Forecast::Clean));
    assert!(!slot.known.contains_key(&pair_key(oid(2), oid(4))));
    slot.forget();
    assert!(slot.known.is_empty());
}

#[test]
fn past_the_limit_only_this_maps_forecasts_stay() {
    let mut slot = BranchMapSlot::default();
    let old = Arc::make_mut(&mut slot.known);
    for n in 0..MAX_KNOWN as u32 {
        old.insert(pair_key(oid(10_000 + n), oid(20_000 + n)), Forecast::Clean);
    }
    slot.remember(&sample());
    assert_eq!(slot.known.len(), 1);
    assert!(slot.known.contains_key(&pair_key(oid(1), oid(2))));
}
