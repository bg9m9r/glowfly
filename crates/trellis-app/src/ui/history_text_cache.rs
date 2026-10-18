//! What the file history's File view measures once per text rather than every frame: the
//! widest line (for the sideways scroll) and, with wrapping on, where each row starts.

use super::diff::wrap::Wrap;
use crate::state::history::FileTextData;
use std::sync::Arc;
use trellis_core::{tabs, wrap};

#[derive(Default)]
pub struct HistoryTextCache {
    /// The text measured; held so its address can't be reused by another.
    data: Option<Arc<FileTextData>>,
    tab: usize,
    row_h: u32,
    wrap: Option<(usize, u32)>,
    /// The widest line, in columns.
    pub widest: usize,
    /// With wrapping on, the top of each row and then the total (n + 1 entries, from 0); empty
    /// otherwise.
    pub offsets: Vec<f32>,
}

impl HistoryTextCache {
    /// Measures `data` again unless it was measured with the same tab size, row height and wrap.
    pub(in crate::ui) fn refresh(&mut self, data: &Arc<FileTextData>, tab: usize, row_h: f32, wrap: Option<Wrap>) {
        let wrap_key = wrap.map(|w| (w.cols, w.line_h.to_bits()));
        let same = self.data.as_ref().is_some_and(|d| Arc::ptr_eq(d, data));
        if same && self.tab == tab && self.row_h == row_h.to_bits() && self.wrap == wrap_key {
            return;
        }
        if !same || self.tab != tab {
            self.widest = data.lines.iter().map(|l| tabs::columns(l, tab)).max().unwrap_or(0);
        }
        self.offsets.clear();
        if let Some(w) = wrap {
            self.offsets.reserve(data.lines.len() + 1);
            let mut y = 0.0;
            self.offsets.push(y);
            for line in &data.lines {
                y += row_h + wrap::line_count(line, tab, w.cols).saturating_sub(1) as f32 * w.line_h;
                self.offsets.push(y);
            }
        }
        self.data = Some(data.clone());
        self.tab = tab;
        self.row_h = row_h.to_bits();
        self.wrap = wrap_key;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trellis_core::repo::LfsState;

    fn data(lines: &[&str]) -> Arc<FileTextData> {
        Arc::new(FileTextData { lines: lines.iter().map(|l| l.to_string()).collect(), spans: None, lfs: LfsState::None })
    }

    #[test]
    fn measures_widths_and_wrapped_heights() {
        let d = data(&["short", "a line of twenty chr", "\tx"]);
        let mut c = HistoryTextCache::default();
        c.refresh(&d, 4, 19.0, None);
        assert_eq!(c.widest, 20);
        assert!(c.offsets.is_empty());
        c.refresh(&d, 4, 19.0, Some(Wrap { cols: 8, line_h: 15.0 }));
        // "a line of twenty chr" breaks at spaces into four visual lines of up to 8 columns.
        assert_eq!(c.offsets, [0.0, 19.0, 19.0 + 19.0 + 45.0, 19.0 + 19.0 + 45.0 + 19.0]);
        c.refresh(&d, 4, 19.0, None);
        assert!(c.offsets.is_empty(), "unwrapped again");
    }
}
