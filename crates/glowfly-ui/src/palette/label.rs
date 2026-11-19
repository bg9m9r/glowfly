//! A command palette row's label (`palette.rs` draws the rows): its runs of colour (the matched
//! characters in the accent colour, a file's folder part dimmed), and for a ref or repository
//! that doesn't fit, its middle cut with the matched characters moved to the cut text.

use super::super::theme::color;
use eframe::egui::{self, Color32, FontId, text::LayoutJob};

/// The share of a row the label keeps before a long detail is cut.
pub(super) const LABEL_SHARE: f32 = 0.4;

/// The width a row's detail may take: what is left of a `row_w` wide row after the label's 10 px
/// inset and the 8 px the detail reserves to its left (18), `taken` (the shortcut's width and
/// the detail's right gap) and the label, which keeps up to `LABEL_SHARE` of the row.
pub(super) fn detail_room(row_w: f32, taken: f32, label_w: f32) -> f32 {
    (row_w - 18.0 - taken - label_w.min(row_w * LABEL_SHARE)).max(0.0)
}

/// `label` cut in its middle at `(head, tail)` (`truncate::middle_cut`'s byte offsets), and its
/// `matched` character indices moved to the cut text (those in the part cut out are dropped).
pub(super) fn cut_marks(label: &str, (head, tail): (usize, usize), matched: &[usize]) -> (String, Vec<usize>) {
    let head_chars = label[..head].chars().count();
    let tail_from = label[..tail].chars().count();
    let shown = format!("{}…{}", &label[..head], &label[tail..]);
    let moved = matched
        .iter()
        .filter_map(|&i| match i {
            i if i < head_chars => Some(i),
            i if i >= tail_from => Some(i - tail_from + head_chars + 1),
            _ => None,
        })
        .collect();
    (shown, moved)
}

/// The label as runs of one colour each: matched chars in the accent colour, the chars before
/// `dim_before` dimmed, the rest in the text colour, at `size` px.
pub(super) fn label_job(label: &str, matched: &[usize], dim_before: usize, size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let format = |col: Color32| egui::TextFormat { font_id: FontId::proportional(size), color: col, ..Default::default() };
    let colour = |i: usize| {
        if matched.contains(&i) {
            color::accent()
        } else if i < dim_before {
            color::weak()
        } else {
            color::text()
        }
    };
    let mut run = String::new();
    let mut run_col = color::text();
    for (i, c) in label.chars().enumerate() {
        let col = colour(i);
        if col != run_col && !run.is_empty() {
            job.append(&std::mem::take(&mut run), 0.0, format(run_col));
        }
        run_col = col;
        run.push(c);
    }
    if !run.is_empty() {
        job.append(&run, 0.0, format(run_col));
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_label_keeps_at_most_its_share_of_the_row() {
        // A short label leaves the detail nearly the whole row.
        assert_eq!(detail_room(560.0, 0.0, 30.0), 512.0);
        // A wide label is capped at 40%: 560 - 18 - 224.
        assert_eq!(detail_room(560.0, 0.0, 400.0), 318.0);
        // A shortcut's width is subtracted.
        assert_eq!(detail_room(560.0, 60.0, 400.0), 258.0);
        // Never negative.
        assert_eq!(detail_room(100.0, 200.0, 10.0), 0.0);
    }

    #[test]
    fn matched_characters_follow_the_cut() {
        // "Checkout feature/long-name": head "Checkout" (8 chars), tail from 'n' of "name" (22).
        let label = "Checkout feature/long-name";
        let tail = label.find("name").unwrap();
        let (shown, moved) = cut_marks(label, (8, tail), &[0, 9, 22, 25]);
        assert_eq!(shown, "Checkout…name");
        // 'C' stays, 'f' was cut out, 'n' and 'e' move to just after the "…".
        assert_eq!(moved, vec![0, 9, 12]);
    }
}
