//! Matched text marked in the row's full ink on the find fill (`Palette::find_fill`), which hugs
//! the glyphs 2 px across and 1 px up and down and stops where a cut line's "…" starts: where a
//! needle falls in a line (`runs`), a cut line painted with those runs marked
//! (`paint_cut_marked`, the graph's rows and the activity rows' title and summary) and a wrapped
//! label built from marked pieces (`append_marked`, the open activity rows and the log tabs).

use crate::theme::sizes;
use crate::theme::tokens::radius;
use crate::widgets::Cut;
use crate::widgets::truncate::cut_galley;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Color32, CornerRadius, Painter, Pos2, Rect, pos2, vec2};
use std::ops::Range;

/// How far the fill reaches past the matched glyphs, across and up and down.
const PAD: (f32, f32) = (2.0, 1.0);

/// Matched character ranges (char indices, sorted, merged where they touch).
pub(crate) type Runs = Vec<Range<usize>>;

/// How a matching row marks its matched text: the fill behind it and the ink of its glyphs.
#[derive(Clone, Copy)]
pub(crate) struct Mark {
    pub fill: Color32,
    pub ink: Color32,
}

/// The char ranges of `hay` where lowercase `needle` occurs, ignoring case (each char compared by
/// its first lowercase char), merged where they touch or overlap.
pub(crate) fn runs(hay: &str, needle: &str) -> Runs {
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() {
        return Vec::new();
    }
    let h: Vec<char> = hay.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect();
    let found = (0..h.len().saturating_sub(n.len() - 1)).filter(|&i| h[i..i + n.len()] == n[..]).map(|i| i..i + n.len());
    merge(found.collect())
}

/// `r` sorted, with ranges that touch or overlap joined.
pub(crate) fn merge(mut r: Runs) -> Runs {
    r.sort_by_key(|x| x.start);
    let mut out: Runs = Vec::new();
    for x in r {
        match out.last_mut() {
            Some(last) if x.start <= last.end => last.end = last.end.max(x.end),
            _ => out.push(x),
        }
    }
    out
}

/// `job` with the chars in `runs` (char indices into its text) recoloured `ink`.
pub(crate) fn ink_runs(job: LayoutJob, runs: &[Range<usize>], ink: Color32) -> LayoutJob {
    if runs.is_empty() {
        return job;
    }
    let byte = |ci: usize| job.text.char_indices().nth(ci).map_or(job.text.len(), |(b, _)| b);
    let cuts: Vec<Range<usize>> = runs.iter().map(|r| byte(r.start)..byte(r.end)).collect();
    let mut out = job.clone();
    out.sections.clear();
    for s in &job.sections {
        let end = s.byte_range.end;
        // The section's pieces: plain, matched, plain, …; only the first keeps its leading space.
        let mut pieces: Vec<(Range<usize>, Color32)> = Vec::new();
        let mut at = s.byte_range.start;
        for c in &cuts {
            let (a, b) = (c.start.max(at), c.end.min(end));
            if a < b {
                pieces.push((at..a, s.format.color));
                pieces.push((a..b, ink));
                at = b;
            }
        }
        pieces.push((at..end, s.format.color));
        let mut lead = s.leading_space;
        for (range, color) in pieces.into_iter().filter(|(r, _)| !r.is_empty()) {
            let mut sec = s.clone();
            sec.leading_space = std::mem::take(&mut lead);
            sec.byte_range = range;
            sec.format.color = color;
            out.sections.push(sec);
        }
    }
    out
}

/// Paints `job` as `widgets::paint_cut` does (its left end at `at.x`, centred on `at.y`, cut to
/// `max_w` with "…"), first filling the chars in `runs` with `mark`'s fill and inking them.
pub(crate) fn paint_cut_marked(p: &Painter, at: Pos2, job: LayoutJob, max_w: f32, runs: &[Range<usize>], mark: Option<Mark>) -> Cut {
    let Some(mark) = mark.filter(|_| !runs.is_empty() && !job.sections.is_empty() && max_w > 0.0) else {
        return crate::widgets::paint_cut(p, at, job, max_w);
    };
    let fallback = job.sections[0].format.color;
    let galley = cut_galley(p.ctx(), ink_runs(job, runs, mark.ink), max_w);
    let min = pos2(at.x, sizes::centred_on(at.y, galley.size().y));
    if let Some(row) = galley.rows.first() {
        // The glyphs shown before the "…" (one glyph per char on a single line).
        let shown = row.glyphs.len().saturating_sub(usize::from(galley.elided));
        for r in runs {
            let end = r.end.min(shown);
            if r.start >= end {
                continue;
            }
            let (first, last) = (&row.glyphs[r.start], &row.glyphs[end - 1]);
            let (x0, x1) = (min.x + first.pos.x, min.x + last.pos.x + last.advance_width);
            let rect = Rect::from_min_max(pos2(x0, min.y), pos2(x1, min.y + galley.size().y)).expand2(vec2(PAD.0, PAD.1));
            p.rect_filled(rect, CornerRadius::same(radius::XS), mark.fill);
        }
    }
    let cut = Cut { rect: Rect::from_min_size(min, galley.size()), cut: galley.elided };
    p.galley(min, galley, fallback);
    cut
}

/// Appends `text` to `job` in `format`, the chars where lowercase `needle` occurs in `mark`'s ink
/// on its fill; one piece when there is no needle or no match. For wrapped, multi-line labels,
/// where painting a rounded fill per run isn't practical.
pub(crate) fn append_marked(job: &mut LayoutJob, text: &str, format: TextFormat, needle: &str, mark: Mark) {
    let found = runs(text, needle);
    if found.is_empty() {
        job.append(text, 0.0, format);
        return;
    }
    let byte = |ci: usize| text.char_indices().nth(ci).map_or(text.len(), |(b, _)| b);
    let mut at = 0;
    for r in &found {
        let (a, b) = (byte(r.start), byte(r.end));
        if a > at {
            job.append(&text[at..a], 0.0, format.clone());
        }
        job.append(&text[a..b], 0.0, TextFormat { color: mark.ink, background: mark.fill, ..format.clone() });
        at = b;
    }
    if at < text.len() {
        job.append(&text[at..], 0.0, format);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_ignore_case_and_merge() {
        assert_eq!(runs("Login page: LOGIN", "login"), vec![0..5, 12..17]);
        assert_eq!(runs("aaaa", "aa"), vec![0..4], "overlapping runs join");
        assert_eq!(runs("Ünïcode ünï", "ünï"), vec![0..3, 8..11], "char indices, not bytes");
        assert!(runs("anything", "").is_empty());
    }

    #[test]
    fn inked_runs_split_sections_and_keep_the_rest() {
        let job = LayoutJob::simple_singleline("Add login page".into(), Default::default(), Color32::GRAY);
        let out = ink_runs(job, std::slice::from_ref(&(4..9)), Color32::WHITE);
        let parts: Vec<_> = out.sections.iter().map(|s| (&out.text[s.byte_range.clone()], s.format.color)).collect();
        assert_eq!(parts, vec![("Add ", Color32::GRAY), ("login", Color32::WHITE), (" page", Color32::GRAY)]);
    }

    #[test]
    fn append_marked_splits_a_line_and_fills_the_match() {
        let (ink, fill) = (Color32::WHITE, Color32::YELLOW);
        let mark = Mark { fill, ink };
        let format = TextFormat { color: Color32::GRAY, ..Default::default() };
        let mut job = LayoutJob::default();
        append_marked(&mut job, "remote: Needle and needle", format.clone(), "needle", mark);
        let parts: Vec<_> = job.sections.iter().map(|s| (&job.text[s.byte_range.clone()], s.format.color, s.format.background)).collect();
        let (g, t) = (Color32::GRAY, Color32::TRANSPARENT);
        assert_eq!(parts, vec![("remote: ", g, t), ("Needle", ink, fill), (" and ", g, t), ("needle", ink, fill)]);

        let mut job = LayoutJob::default();
        append_marked(&mut job, "no match here", format.clone(), "zzz", mark);
        append_marked(&mut job, " plain", format.clone(), "", mark);
        assert_eq!(job.sections.len(), 2, "an empty needle or no match appends one piece");
        assert_eq!(job.text, "no match here plain");

        // Char boundaries, not bytes.
        let mut job = LayoutJob::default();
        append_marked(&mut job, "Ünïcode ünï!", format, "ünï", mark);
        let marked: Vec<_> = job.sections.iter().filter(|s| s.format.background == fill).map(|s| &job.text[s.byte_range.clone()]).collect();
        assert_eq!(marked, vec!["Ünï", "ünï"]);
    }
}
