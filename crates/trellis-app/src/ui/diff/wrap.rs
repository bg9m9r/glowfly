//! The diff viewer's scroll area, with rows of one height or, when long lines wrap, of varied
//! heights. State and update work in row indexes; only this converts them to pixel offsets.

use super::style::DiffStyle;
use eframe::egui::{self, Rect, Ui, UiBuilder};
use trellis_core::settings::DiffLayout;
use trellis_core::{tabs, wrap};

/// The narrowest a wrapped line gets, in columns; anything past it is clipped by the painter.
pub(in crate::ui) const MIN_COLS: usize = 8;
/// Room kept free after the text column so wrapped text clears the scrollbar and the edge.
const PAD: f32 = 8.0;

/// How wrapped lines are broken and stacked: at most `cols` columns per visual line, each
/// `line_h` below the one before.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::ui) struct Wrap {
    pub cols: usize,
    pub line_h: f32,
}

/// The wrap of the text column in `layout` at the width `ui` offers, `None` unless the
/// setting is on. In split, the right side (the narrower half) decides.
pub(super) fn wrap_for(ui: &Ui, st: &DiffStyle, layout: DiffLayout) -> Option<Wrap> {
    if !st.wrap {
        return None;
    }
    let char_w = ui.fonts(|f| f.glyph_width(&st.font, 'M'));
    let line_h = ui.fonts(|f| f.row_height(&st.font));
    let scroll = &ui.spacing().scroll;
    let avail = ui.available_width() - scroll.allocated_width().max(scroll.bar_width);
    let text_w = match layout {
        DiffLayout::Inline => avail - st.inline_cols().text - PAD,
        DiffLayout::Split => avail - (avail / 2.0).round() - st.split_cols().text - PAD,
    };
    let cols = if char_w > 0.0 { (text_w / char_w).floor().max(0.0) as usize } else { 0 };
    Some(Wrap { cols: cols.max(MIN_COLS), line_h })
}

/// The widest of `lines`, in columns with tabs expanded to `tab`.
pub(in crate::ui) fn widest(lines: &[String], tab: usize) -> usize {
    lines.iter().map(|l| tabs::columns(l, tab)).max().unwrap_or(0)
}

/// Writes into `out` where each of `lines` starts when wrapped by `w`, then the total (n + 1
/// offsets from 0): each row is `row_h` plus `w.line_h` for every visual line past the first.
pub(in crate::ui) fn row_offsets(lines: &[String], tab: usize, row_h: f32, w: Wrap, out: &mut Vec<f32>) {
    out.clear();
    out.reserve(lines.len() + 1);
    let mut y = 0.0;
    out.push(y);
    for line in lines {
        y += row_h + wrap::line_count(line, tab, w.cols).saturating_sub(1) as f32 * w.line_h;
        out.push(y);
    }
}

/// Where the rows sit: all `h` high, or (wrapped) at prefix offsets, one per row plus the
/// total, starting at 0.
#[derive(Clone, Copy, Debug)]
pub(in crate::ui) enum RowMetrics<'a> {
    Uniform(f32),
    Varied(&'a [f32]),
}

impl RowMetrics<'_> {
    /// The top of row `i`, relative to the first; past the end, the bottom of the last.
    pub fn top(&self, i: usize) -> f32 {
        match self {
            Self::Uniform(h) => i as f32 * h,
            Self::Varied(o) => o.get(i.min(o.len().saturating_sub(1))).copied().unwrap_or(0.0),
        }
    }

    pub fn height(&self, i: usize) -> f32 {
        match self {
            Self::Uniform(h) => *h,
            Self::Varied(_) => self.top(i + 1) - self.top(i),
        }
    }

    pub fn total(&self, rows: usize) -> f32 {
        self.top(rows)
    }

    /// The row whose extent holds `y`, clamped to the rows there are.
    pub fn row_containing(&self, y: f32, rows: usize) -> usize {
        let i = match self {
            Self::Uniform(h) => (y / h).floor().max(0.0) as usize,
            Self::Varied(o) => o.partition_point(|t| *t <= y).saturating_sub(1),
        };
        i.min(rows.saturating_sub(1))
    }

    /// The row whose top is nearest `y` (a scroll offset), so a row scrolled more than half
    /// out of view no longer counts as the top one.
    pub fn row_at(&self, y: f32, rows: usize) -> usize {
        match self {
            Self::Uniform(h) => (y / h).round() as usize,
            Self::Varied(_) => {
                let i = self.row_containing(y, rows);
                let past_middle = y - self.top(i) >= self.height(i) / 2.0;
                if past_middle && i + 1 < rows { i + 1 } else { i }
            }
        }
    }

    /// How many whole rows from `top` fit in `viewport_h`.
    pub fn visible_from(&self, top: usize, viewport_h: f32, rows: usize) -> usize {
        match self {
            Self::Uniform(h) => (viewport_h / h).floor() as usize,
            Self::Varied(_) => {
                let limit = self.top(top) + viewport_h;
                (top..rows).take_while(|k| self.top(k + 1) <= limit).count()
            }
        }
    }
}

/// The rows in a scroll area named `id_salt` (the diff's both layouts share "diff", so switching
/// layout doesn't leave a stale second scroll state), drawing only those in view; `add_row`
/// gets each row's index and height. Scrolls `scroll_to` to the top if set, and returns the
/// first visible row and how many rows fit in the viewport.
pub(in crate::ui) fn scroll(
    ui: &mut Ui,
    id_salt: &str,
    rows: usize,
    m: RowMetrics,
    scroll_to: Option<usize>,
    mut add_row: impl FnMut(&mut Ui, usize, f32),
) -> (usize, usize) {
    ui.spacing_mut().item_spacing.y = 0.0;
    let mut area = egui::ScrollArea::both().id_salt(id_salt).auto_shrink([false, false]);
    if let Some(row) = scroll_to {
        area = area.vertical_scroll_offset(m.top(row.min(rows)));
    }
    let out = area.show_viewport(ui, |ui, vp| {
        ui.set_height(m.total(rows));
        if rows == 0 {
            return;
        }
        let first = m.row_containing(vp.min.y, rows);
        let end = (m.row_containing(vp.max.y, rows) + 2).min(rows);
        let top = ui.max_rect().top();
        let rect = Rect::from_x_y_ranges(ui.max_rect().x_range(), top + m.top(first)..=top + m.top(end));
        ui.allocate_new_ui(UiBuilder::new().max_rect(rect), |ui| {
            ui.skip_ahead_auto_ids(first); // consistent ids, as `show_rows` does
            for i in first..end {
                add_row(ui, i, m.height(i));
            }
        });
    });
    let top = m.row_at(out.state.offset.y, rows);
    (top, m.visible_from(top, out.inner_rect.height(), rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_rows_match_the_fixed_layout() {
        let m = RowMetrics::Uniform(19.0);
        assert_eq!(m.top(3), 57.0);
        assert_eq!(m.height(7), 19.0);
        assert_eq!(m.total(4), 76.0);
        for y in [0.0, 9.0, 9.5, 10.0, 37.9, 190.0, 1000.3] {
            assert_eq!(m.row_at(y, 100), (y / 19.0_f32).round() as usize, "{y}");
        }
        for h in [0.0, 18.9, 19.0, 400.0, 401.7] {
            assert_eq!(m.visible_from(5, h, 100), (h / 19.0_f32).floor() as usize, "{h}");
        }
        assert_eq!(m.row_containing(40.0, 100), 2);
        assert_eq!(m.row_containing(4000.0, 10), 9, "clamped");
    }

    #[test]
    fn varied_rows_follow_their_offsets() {
        let m = RowMetrics::Varied(&[0.0, 19.0, 57.0, 76.0]);
        assert_eq!(m.height(1), 38.0);
        assert_eq!(m.total(3), 76.0);
        assert_eq!(m.top(9), 76.0, "clamped past the end");
        assert_eq!(m.row_containing(40.0, 3), 1);
        assert_eq!(m.row_containing(-5.0, 3), 0);
        assert_eq!(m.row_containing(500.0, 3), 2);
        assert_eq!(m.row_at(30.0, 3), 1);
        assert_eq!(m.row_at(39.0, 3), 2, "past the middle of row 1");
        assert_eq!(m.row_at(70.0, 3), 2, "no row after the last");
        assert_eq!(m.visible_from(0, 60.0, 3), 2);
        assert_eq!(m.visible_from(1, 60.0, 3), 2);
        assert_eq!(m.visible_from(0, 10.0, 3), 0);
        let empty = RowMetrics::Varied(&[0.0]);
        assert_eq!((empty.total(0), empty.visible_from(0, 100.0, 0)), (0.0, 0));
    }
}
