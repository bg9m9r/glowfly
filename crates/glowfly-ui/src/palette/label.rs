//! A command palette row's label (`palette.rs` draws the rows): its runs of colour (the matched
//! characters in the accent colour, a file's folder part dimmed), and for a ref or repository
//! that doesn't fit, its middle cut with the matched characters moved to the cut text.

use super::super::theme::color;
use eframe::egui::{self, Color32, FontId, text::LayoutJob};

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
