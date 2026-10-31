//! What the blame view measures once per blame rather than every frame: the widest line (for
//! the sideways scroll) and, with wrapping on, where each row starts.

use crate::ui::diff::wrap::{self, Wrap};

/// What was measured: the blame's generation (`BlameView::data_gen`), the tab size, the row
/// height's bits and the wrap (columns, line height bits).
type Key = (u64, usize, u32, Option<(usize, u32)>);

#[derive(Default)]
pub struct BlameRowsCache {
    key: Option<Key>,
    /// The widest line, in columns.
    pub widest: usize,
    /// With wrapping on, the top of each row and then the total (n + 1 entries, from 0); empty
    /// otherwise.
    pub offsets: Vec<f32>,
}

impl BlameRowsCache {
    /// Measures `lines` again unless they were measured for the same blame, tab size, row
    /// height and wrap. The widest line is measured again only for a new blame or tab size.
    pub(crate) fn refresh(&mut self, data_gen: u64, lines: &[String], tab: usize, row_h: f32, wrap: Option<Wrap>) {
        let key = (data_gen, tab, row_h.to_bits(), wrap.map(|w| (w.cols, w.line_h.to_bits())));
        if self.key == Some(key) {
            return;
        }
        if self.key.is_none_or(|(g, t, ..)| g != data_gen || t != tab) {
            self.widest = wrap::widest(lines, tab);
        }
        match wrap {
            Some(w) => wrap::row_offsets(lines, tab, row_h, w, &mut self.offsets),
            None => self.offsets.clear(),
        }
        self.key = Some(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn measures_widest_and_wrapped_offsets() {
        let l = lines(&["short", "a line of twenty chr", "\t\t\t\t\tx"]);
        let mut c = BlameRowsCache::default();
        c.refresh(1, &l, 4, 19.0, None);
        assert_eq!(c.widest, 21, "five tabs of four and an x");
        assert!(c.offsets.is_empty());
        let w = Wrap { cols: 8, line_h: 15.0 };
        c.refresh(1, &l, 4, 19.0, Some(w));
        // "a line of twenty chr" breaks at spaces into four visual lines of up to 8 columns.
        assert_eq!(c.offsets[..3], [0.0, 19.0, 19.0 + 19.0 + 45.0]);
        assert_eq!(c.offsets.len(), 4);

        // The same key measures nothing again, even for other lines.
        let other = lines(&["x"]);
        c.refresh(1, &other, 4, 19.0, Some(w));
        assert_eq!((c.widest, c.offsets.len()), (21, 4));
        // A new blame is measured again.
        c.refresh(2, &other, 4, 19.0, Some(w));
        assert_eq!((c.widest, &c.offsets[..]), (1, &[0.0, 19.0][..]));
        c.refresh(2, &other, 4, 19.0, None);
        assert!(c.offsets.is_empty(), "unwrapped again");
    }
}
