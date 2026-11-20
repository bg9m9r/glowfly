//! A long left-panel list of same-height rows (TAGS, a remote's branches) visits only the rows in
//! view. Rows above and below take their room in one allocation each, with the widget ids the
//! skipped rows would have taken (`list_row_hidden` takes one each), so ids, positions and the
//! scroll extent match drawing every row. Which rows pass the left-panel filter is kept per
//! snapshot and filter text.

use crate::frame_memo;
use crate::widgets::{LIST_ROW_H, list_row_hidden};
use eframe::egui::{Context, Id, Ui, vec2};
use std::sync::Arc;

/// Draws rows `0..n` through `draw`, calling it only for those in or next to the view. `draw`
/// must allocate exactly one `LIST_ROW_H` row (`name_row` and `name_row_cut` do).
pub(crate) fn rows(ui: &mut Ui, n: usize, mut draw: impl FnMut(&mut Ui, usize)) {
    let step = LIST_ROW_H + ui.spacing().item_spacing.y;
    let (top, clip) = (ui.cursor().min.y, ui.clip_rect());
    // One row of slack each side; rows in [first, last) still go through `list_row_hidden`, which
    // decides exactly as before. Clamped before casting: an infinite clip must not overflow.
    let at = |y: f32| f64::from((y - top) / step);
    let first = (at(clip.min.y).floor() - 1.0).clamp(0.0, n as f64) as usize;
    let last = ((at(clip.max.y).ceil() + 1.0).clamp(0.0, n as f64) as usize).max(first);
    skip(ui, first);
    for i in first..last {
        if list_row_hidden(ui) {
            continue;
        }
        draw(ui, i);
    }
    skip(ui, n - last);
}

/// `k` rows' room in one allocation, taking the `k` widget ids the rows would have.
fn skip(ui: &mut Ui, k: usize) {
    if k == 0 {
        return;
    }
    let sp = ui.spacing().item_spacing.y;
    ui.allocate_space(vec2(ui.available_width(), k as f32 * (LIST_ROW_H + sp) - sp));
    ui.skip_ahead_auto_ids(k - 1);
}

/// Which of a list's rows pass the left-panel filter.
pub(crate) enum Shown {
    /// No filter text: every row, none listed.
    All(usize),
    /// The indices of the matching rows, in the list's order.
    Some(Arc<[usize]>),
}

impl Shown {
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::All(n) => *n,
            Self::Some(list) => list.len(),
        }
    }

    /// The list's index of the `k`th shown row.
    pub(crate) fn at(&self, k: usize) -> usize {
        match self {
            Self::All(_) => k,
            Self::Some(list) => list[k],
        }
    }
}

/// The rows of an `n`-row list that `matches` (the left-panel filter's predicate on row `i`),
/// kept under `id` for the snapshot's `stamp` and the lowercased `filter` text. An empty filter
/// matches every row, so nothing is listed or kept.
pub(crate) fn shown(ctx: &Context, id: Id, (stamp, filter): (u64, &str), n: usize, matches: impl Fn(usize) -> bool) -> Shown {
    if filter.is_empty() {
        return Shown::All(n);
    }
    Shown::Some(frame_memo::kept(ctx, id, (stamp, filter.to_owned()), || (0..n).filter(|&i| matches(i)).collect::<Arc<[usize]>>()))
}

#[cfg(test)]
#[path = "long_list_tests.rs"]
mod tests;
