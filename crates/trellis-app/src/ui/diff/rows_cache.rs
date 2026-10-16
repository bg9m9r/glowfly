//! The open diff's row list and widest line, built when the diff, the layout, the open folds or
//! the tab width change instead of on every frame: both walk every line of the file. With long
//! lines wrapped, also the rows' heights, rebuilt when the rows, the wrap width or the font
//! change.

use super::wrap::{RowMetrics, Wrap};

use trellis_core::diff_rows::{self, DiffRow, SplitRow};
use trellis_core::repo::FileDiff;
use trellis_core::settings::DiffLayout;
use trellis_core::tabs;
use trellis_core::whole_file::{self, Expanded, WholeFile};
use trellis_core::wrap;

/// What the rows were built from. A loaded, reloaded or restored diff always comes with a new
/// `diff_gen` or `diff_shown_gen`; the addresses also catch the whole file being dropped in
/// place when the view switches back to hunks.
#[derive(PartialEq)]
struct Key {
    generation: (u64, u64),
    hunks: usize,
    whole: Option<usize>,
    layout: DiffLayout,
    expanded: Expanded,
    tab: usize,
}

/// What the row heights were built from: the rows build, the wrap columns and line height
/// (`None` unwrapped), the row height and the tab size (bit patterns, so the key compares
/// exactly).
#[derive(PartialEq, Clone, Copy)]
struct MetricsKey {
    rows: usize,
    wrap: Option<(usize, u32)>,
    row_h: u32,
    tab: usize,
}

#[derive(Default)]
pub struct DiffRowsCache {
    key: Option<Key>,
    /// The rows of the inline layout (empty in the split layout).
    pub inline: Vec<DiffRow>,
    /// The rows of the split layout (empty in the inline layout).
    pub split: Vec<SplitRow>,
    /// The widest drawn line, in columns.
    pub widest: usize,
    /// How many times the rows were built (for tests).
    pub builds: usize,
    /// With wrapping on, the top of each row and, last, the bottom of the last row; empty
    /// when the rows are all one height.
    pub offsets: Vec<f32>,
    metrics_key: Option<MetricsKey>,
    /// How many times the row heights were built (for tests).
    pub height_builds: usize,
}

/// What the rows are built from: the diff and, in the whole-file view, its whole file.
pub struct Source<'a> {
    pub diff: &'a FileDiff,
    pub whole: Option<&'a WholeFile>,
    pub generation: (u64, u64),
    pub layout: DiffLayout,
    pub expanded: &'a Expanded,
    pub tab: usize,
}

impl DiffRowsCache {
    /// Rebuilds the rows unless they were built from the same source.
    pub fn refresh(&mut self, s: Source) {
        let hunks = s.diff.hunks.as_ptr() as usize;
        let whole = s.whole.map(|w| w as *const WholeFile as usize);
        let same = self.key.as_ref().is_some_and(|k| {
            k.generation == s.generation
                && k.hunks == hunks
                && k.whole == whole
                && k.layout == s.layout
                && k.tab == s.tab
                && k.expanded == *s.expanded
        });
        if same {
            return;
        }
        let (inline, split) = match (s.layout, s.whole) {
            (DiffLayout::Inline, None) => (diff_rows::inline_rows(s.diff), Vec::new()),
            (DiffLayout::Inline, Some(w)) => (whole_file::inline_rows(w, s.expanded), Vec::new()),
            (DiffLayout::Split, None) => (Vec::new(), diff_rows::split_rows(s.diff)),
            (DiffLayout::Split, Some(w)) => (Vec::new(), whole_file::split_rows(w, s.expanded)),
        };
        let lines = s.whole.map_or(s.diff, |w| &w.diff);
        self.widest = lines.hunks.iter().flat_map(|h| &h.lines).map(|l| tabs::columns(&l.text, s.tab)).max().unwrap_or(0);
        self.inline = inline;
        self.split = split;
        self.builds += 1;
        self.key = Some(Key { generation: s.generation, hunks, whole, layout: s.layout, expanded: s.expanded.clone(), tab: s.tab });
    }

    /// Rebuilds the row heights for `wrap` (clears them for `None`) unless they were built
    /// for the same rows, wrap, row height and tab size. `lines` is the diff the rows draw
    /// from (`DiffView::lines()`). Returns whether the same rows changed height, so the view
    /// should keep its top row in place.
    pub(super) fn refresh_heights(&mut self, lines: &FileDiff, row_h: f32, tab: usize, wrap: Option<Wrap>) -> bool {
        let key = MetricsKey { rows: self.builds, wrap: wrap.map(|w| (w.cols, w.line_h.to_bits())), row_h: row_h.to_bits(), tab };
        let old = self.metrics_key.replace(key);
        if old == Some(key) {
            return false;
        }
        let anchor = old.is_some_and(|o| o.rows == key.rows);
        self.offsets.clear();
        let Some(w) = wrap else { return anchor };
        let height = |n: usize| row_h + n.saturating_sub(1) as f32 * w.line_h;
        let count = |hunk: usize, line: usize| wrap::line_count(&lines.hunks[hunk].lines[line].text, tab, w.cols);
        let heights: Vec<f32> = if self.inline.is_empty() {
            self.split
                .iter()
                .map(|r| match *r {
                    SplitRow::Pair { hunk, old, new } => {
                        height([old, new].into_iter().flatten().map(|l| count(hunk, l)).max().unwrap_or(1))
                    }
                    _ => row_h,
                })
                .collect()
        } else {
            self.inline
                .iter()
                .map(|r| match *r {
                    DiffRow::Line { hunk, line } => height(count(hunk, line)),
                    _ => row_h,
                })
                .collect()
        };
        self.offsets.reserve(heights.len() + 1);
        self.offsets.push(0.0);
        let mut y = 0.0;
        for h in heights {
            y += h;
            self.offsets.push(y);
        }
        self.height_builds += 1;
        anchor
    }

    /// Where the rows sit: varied heights when wrapped, else all `row_h` high.
    pub(super) fn row_metrics(&self, row_h: f32) -> RowMetrics<'_> {
        if self.offsets.is_empty() { RowMetrics::Uniform(row_h) } else { RowMetrics::Varied(&self.offsets) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trellis_core::repo::{DiffLine, Hunk};

    fn diff(lines: &[(char, &str)]) -> FileDiff {
        let lines = lines.iter().map(|&(origin, text)| DiffLine { origin, old: None, new: None, text: text.into() }).collect();
        FileDiff {
            binary: false,
            hunks: vec![Hunk { header: "@@ -1 +1 @@".into(), lines }],
            highlight: None,
            intraline: Default::default(),
            whole: None,
        }
    }

    fn src<'a>(diff: &'a FileDiff, generation: (u64, u64), layout: DiffLayout, tab: usize, expanded: &'a Expanded) -> Source<'a> {
        Source { diff, whole: None, generation, layout, expanded, tab }
    }

    #[test]
    fn rows_are_built_once_per_diff_layout_and_tab_width() {
        let mut cache = DiffRowsCache::default();
        let expanded = Expanded::default();
        let d = diff(&[(' ', "same"), ('-', "\told"), ('+', "new text")]);
        for _ in 0..3 {
            cache.refresh(src(&d, (1, 1), DiffLayout::Inline, 4, &expanded));
        }
        assert_eq!(cache.builds, 1, "redrawing the same diff reuses its rows");
        assert_eq!(cache.inline, diff_rows::inline_rows(&d));
        assert_eq!(cache.widest, 8, "the tab expands to four columns");

        cache.refresh(src(&d, (1, 1), DiffLayout::Inline, 8, &expanded));
        assert_eq!((cache.builds, cache.widest), (2, 11));
        cache.refresh(src(&d, (1, 1), DiffLayout::Split, 8, &expanded));
        assert_eq!(cache.builds, 3);
        assert_eq!(cache.split, diff_rows::split_rows(&d));
        assert!(cache.inline.is_empty());

        // A reload brings a new generation, even when the new diff happens to reuse the memory.
        let reloaded = diff(&[('+', "only")]);
        cache.refresh(src(&reloaded, (2, 2), DiffLayout::Split, 8, &expanded));
        assert_eq!(cache.builds, 4);
        assert_eq!(cache.split, diff_rows::split_rows(&reloaded));
    }

    #[test]
    fn heights_follow_cols_font_and_rows() {
        let mut cache = DiffRowsCache::default();
        let expanded = Expanded::default();
        let long = "abcdefghij klmnopqrst uvwxy"; // 27 chars: three lines of at most 10
        let d = diff(&[(' ', "same"), ('-', long), ('+', "abcdefghij klmnop")]);
        cache.refresh(src(&d, (1, 1), DiffLayout::Inline, 4, &expanded));
        let wrap = |cols: usize, line_h: f32| Some(Wrap { cols, line_h });
        assert!(!cache.refresh_heights(&d, 19.0, 4, wrap(10, 15.0)), "first build: nothing to keep");
        // Rows: header, same, long, short.
        let m = cache.row_metrics(19.0);
        let heights: Vec<f32> = (0..4).map(|i| m.height(i)).collect();
        assert_eq!(heights, [19.0, 19.0, 19.0 + 2.0 * 15.0, 19.0 + 15.0]);
        for _ in 0..3 {
            assert!(!cache.refresh_heights(&d, 19.0, 4, wrap(10, 15.0)));
        }
        assert_eq!(cache.height_builds, 1, "same key, no rebuild");

        assert!(cache.refresh_heights(&d, 19.0, 4, wrap(30, 15.0)), "a new width keeps the top row");
        assert_eq!((cache.height_builds, cache.row_metrics(19.0).height(2)), (2, 19.0));
        assert!(cache.refresh_heights(&d, 19.0, 4, wrap(30, 16.0)), "a new font keeps the top row");
        assert_eq!(cache.height_builds, 3);

        // New rows (a reload) rebuild without anchoring.
        cache.refresh(src(&d, (2, 2), DiffLayout::Inline, 4, &expanded));
        assert!(!cache.refresh_heights(&d, 19.0, 4, wrap(30, 16.0)));
        assert_eq!(cache.height_builds, 4);

        // Unwrapped: one height for all.
        assert!(cache.refresh_heights(&d, 19.0, 4, None));
        assert!(cache.offsets.is_empty());
        assert!(matches!(cache.row_metrics(19.0), RowMetrics::Uniform(h) if h == 19.0));

        // Split: the pair takes the taller side's height.
        cache.refresh(src(&d, (2, 2), DiffLayout::Split, 4, &expanded));
        cache.refresh_heights(&d, 19.0, 4, wrap(10, 15.0));
        let pair = cache.split.iter().position(|r| matches!(r, SplitRow::Pair { old: Some(1), new: Some(2), .. })).unwrap();
        assert_eq!(cache.row_metrics(19.0).height(pair), 49.0);
        assert_eq!(cache.row_metrics(19.0).total(cache.split.len()), 19.0 * (cache.split.len() - 1) as f32 + 49.0);
    }
}
