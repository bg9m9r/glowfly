//! The Branch map's state (`RepoSession::branch_map`, parked with its tab), the trial merges
//! remembered for the tab's repository, and the pure words the view shows: the subtitle, the
//! empty states, each row's numbers as a line and as an accessible name, the conflict hint and
//! the detail pane's badges.

use super::AppState;
use super::palette::commands::{Entry, PaletteCommand, entry};
use glowfly_core::branch_map::{BranchMap, Empty, Forecast, Forecasts, Request, pair_key};
use glowfly_core::branch_ref::BranchRef;
use glowfly_core::error::ReadFailed;
use glowfly_core::i18n::Lang;
use std::sync::Arc;

/// How many remembered trial merges a tab keeps.
pub const MAX_KNOWN: usize = 4_096;
/// How many paths one block of the detail pane lists before "and N more files".
pub const SHOWN_FILES: usize = 30;

const KEYWORDS: &str = "branches map drift diverged stale behind ahead conflict collide overlap overview";

/// A tab's Branch map: the view when open, the counter the reads are numbered with, and the
/// trial merges made so far.
#[derive(Default)]
pub struct BranchMapSlot {
    /// Shown in place of the graph (update/branch_map.rs).
    pub view: Option<BranchMapView>,
    pub generation: u64,
    /// Trial merges by the pair of tips they merged: handed to each read as an `Arc`.
    pub known: Arc<Forecasts>,
}

/// The open Branch map.
pub struct BranchMapView {
    /// The read in flight or last asked for (`BranchMapSlot::generation`).
    pub generation: u64,
    pub loading: bool,
    /// What the snapshot says to measure now, or why nothing is.
    pub plan: Result<Request, Empty>,
    /// The last read's result; `None` until the first arrives.
    pub result: Option<Result<Box<BranchMap>, ReadFailed>>,
    pub selected: Option<BranchRef>,
}

impl BranchMapSlot {
    /// Remembers the clean and the conflicting trial merges of `map` (a merge that failed is
    /// tried again next time). Past `MAX_KNOWN` only this map's are kept.
    pub fn remember(&mut self, map: &BranchMap) {
        let known = Arc::make_mut(&mut self.known);
        let key = |a: usize, b: usize| pair_key(map.rows[a].tip, map.rows[b].tip);
        let fresh = map.pairs.iter().filter(|p| !matches!(p.forecast, Forecast::Unknown(_)));
        let fresh: Vec<_> = fresh.map(|p| (key(p.a, p.b), p.forecast.clone())).collect();
        if known.len() + fresh.len() > MAX_KNOWN {
            known.clear();
        }
        known.extend(fresh);
    }

    /// Forgets the remembered trial merges (the tab opened another repository).
    pub fn forget(&mut self) {
        self.known = Arc::default();
    }
}

impl BranchMapView {
    pub fn map(&self) -> Option<&BranchMap> {
        self.result.as_ref()?.as_ref().ok().map(|m| &**m)
    }

    /// The selected branch's row.
    pub fn selected_row(&self) -> Option<usize> {
        self.map()?.row_of(self.selected.as_ref()?)
    }

    /// The main branch's name as the plan or the map says it.
    fn main_name(&self) -> Option<&str> {
        match self.map() {
            Some(m) => m.rows.first().map(|r| r.branch.shown()),
            None => self.plan.as_ref().ok().map(|r| r.main.branch.shown()),
        }
    }
}

/// The main branch of `map`, as shown.
fn main_of(map: &BranchMap) -> &str {
    map.rows[0].branch.shown()
}

/// The header's subtitle: what everything is measured against, how many branches were left
/// out, or that a newer read is running.
pub fn subtitle(t: Lang, view: &BranchMapView) -> String {
    if view.loading && view.map().is_some() {
        return t.tr("branch_map.updating").to_owned();
    }
    let Some(main) = view.main_name() else { return String::new() };
    match view.map() {
        Some(m) if m.total > m.rows.len() - 1 => {
            t.trf("branch_map.subtitle.capped", &[("shown", &(m.rows.len() - 1)), ("total", &m.total), ("main", &main)])
        }
        _ => t.trf("branch_map.subtitle.all", &[("main", &main)]),
    }
}

/// Why a read failed, in `t`'s words.
pub fn failure(t: Lang, e: &ReadFailed) -> String {
    super::error_words::typed::read(t, e)
}

/// The title and the hint of the empty state for `why`.
pub fn empty_words(t: Lang, why: Empty, main: Option<&str>) -> (&'static str, String) {
    let main = main.unwrap_or_default();
    match why {
        Empty::NoCommits => (t.tr("branch_map.empty.no_commits.title"), t.tr("branch_map.empty.no_commits.hint").to_owned()),
        Empty::NoMain => (t.tr("branch_map.empty.no_main.title"), t.tr("branch_map.empty.no_main.hint").to_owned()),
        Empty::OneBranch => (t.tr("branch_map.empty.one_branch.title"), t.trf("branch_map.empty.one_branch.hint", &[("main", &main)])),
    }
}

/// The phrases of a row's numbers: "3 behind", "5 ahead", "4 files changed", or the special
/// sentence for the main branch, a merged branch, a branch at the main branch's commit and one
/// with no shared history.
fn stats_parts(t: Lang, map: &BranchMap, i: usize) -> Vec<String> {
    let (row, main) = (&map.rows[i], main_of(map));
    if i == 0 {
        return vec![t.tr("branch_map.row.main_line").to_owned()];
    }
    let behind = t.trf("branch_map.row.behind", &[("n", &row.behind)]);
    match (row.base, row.ahead, row.behind) {
        (None, ..) => vec![t.trf("branch_map.row.unrelated", &[("main", &main)])],
        (_, 0, 0) => vec![t.trf("branch_map.row.same", &[("main", &main)])],
        (_, 0, _) => vec![behind, t.trf("branch_map.row.merged", &[("main", &main)])],
        _ => vec![behind, t.trf("branch_map.row.ahead", &[("n", &row.ahead)]), t.trn("branch_map.row.files", row.files_total, &[])],
    }
}

/// Row `i`'s second line: "3 behind · 5 ahead · 4 files changed".
pub fn stats_line(t: Lang, map: &BranchMap, i: usize) -> String {
    stats_parts(t, map, i).join(" · ")
}

/// How long ago `time` was at `now`: "3 days ago".
pub fn age(t: Lang, now: i64, time: i64) -> String {
    glowfly_core::dates::relative(t, now - time)
}

/// The words a screen reader gets for row `i`: its name, whether it is checked out or the main
/// branch, the same numbers as text, the time of its last commit (`when`) and whether it would
/// conflict with the main branch.
pub fn row_name(t: Lang, map: &BranchMap, i: usize, when: &str) -> String {
    let name = map.rows[i].branch.shown();
    let mut head = if map.current == Some(i) { t.trf("access.checked_out", &[("name", &name)]) } else { name.to_owned() };
    if i == 0 {
        head = format!("{head}, {}", t.tr("branch_map.row.main_note"));
    }
    let mut parts = if i == 0 { Vec::new() } else { stats_parts(t, map, i) };
    parts.push(t.trf("branch_map.row.last_commit", &[("when", &when)]));
    if map.conflicts_with_main(i).is_some() {
        parts.push(t.trf("branch_map.row.conflicts_main", &[("main", &main_of(map))]));
    }
    format!("{head}: {}", parts.join(", "))
}

/// The hover text of a row's warning mark: merging it and the main branch would stop with
/// conflicts in N files. None when it wouldn't.
pub fn conflict_tip(t: Lang, map: &BranchMap, i: usize) -> Option<String> {
    let paths = map.conflicts_with_main(i)?;
    let branch = map.rows[i].branch.shown();
    Some(t.trn("branch_map.row.conflict.tip", paths.len(), &[("branch", &branch), ("main", &main_of(map))]))
}

/// A forecast as a badge: "Conflicts in 2 files", "No conflicts" or "Couldn't check".
pub fn badge(t: Lang, forecast: &Forecast) -> String {
    match forecast {
        Forecast::Clean => t.tr("branch_map.detail.clean").to_owned(),
        Forecast::Conflicts(paths) => t.trn("branch_map.detail.conflicts", paths.len(), &[]),
        Forecast::Unknown(_) => t.tr("branch_map.detail.unknown").to_owned(),
    }
}

/// The first seven characters of the commit row `i` split from the main branch at; None
/// without shared history.
pub fn short_base(map: &BranchMap, i: usize) -> Option<String> {
    map.rows[i].base.map(|b| b.to_string()[..7].to_owned())
}

/// The palette's entry for opening the map.
pub(crate) fn palette_entries(st: &AppState, out: &mut Vec<Entry>) {
    out.push(entry(PaletteCommand::OpenBranchMap, st.settings.lang().tr("branch_map.palette.open"), KEYWORDS));
}

#[cfg(test)]
#[path = "branch_map_tests.rs"]
pub(crate) mod tests;
