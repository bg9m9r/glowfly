//! The open diff's row list and widest line, built when the diff, the layout, the open folds or
//! the tab width change instead of on every frame: both walk every line of the file.

use trellis_core::diff_rows::{self, DiffRow, SplitRow};
use trellis_core::repo::FileDiff;
use trellis_core::settings::DiffLayout;
use trellis_core::tabs;
use trellis_core::whole_file::{self, Expanded, WholeFile};

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
}
