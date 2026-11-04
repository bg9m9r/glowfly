//! A search's matched text on a matching graph row: where the query's free text falls in the
//! summary, the body preview and the author's name, the matched start of the short SHA, and an
//! `author:` term's match in the name (the whole name when the term matched only the email).
//! Each run is drawn in the row's full ink on the find fill (`Palette::find_fill`), which hugs
//! the glyphs 2 px across and 1 px up and down and stops where a cut line's "…" starts.

use crate::theme::sizes;
use crate::theme::tokens::radius;
use crate::widgets::Cut;
use crate::widgets::truncate::cut_galley;
use eframe::egui::text::LayoutJob;
use eframe::egui::{Color32, CornerRadius, Painter, Pos2, Rect, pos2, vec2};
use glowfly_core::commit_query::CommitQuery;
use glowfly_core::matching::{contains_lower, hex_starts_with};
use glowfly_core::repo::CommitInfo;
use std::ops::Range;

/// How far the fill reaches past the matched glyphs, across and up and down.
const PAD: (f32, f32) = (2.0, 1.0);

/// Matched character ranges (char indices, sorted, merged where they touch).
pub(super) type Runs = Vec<Range<usize>>;

/// How a matching row marks its matched text: the fill behind it and the ink of its glyphs.
#[derive(Clone, Copy)]
pub(super) struct Mark {
    pub fill: Color32,
    pub ink: Color32,
}

/// Where the query matched one commit's shown text.
#[derive(Default)]
pub(super) struct RowRuns {
    pub summary: Runs,
    pub body: Runs,
    pub author: Runs,
    pub sha: Runs,
}

impl RowRuns {
    /// The runs of `q` in commit `c` (whose body preview, if shown, is `body`).
    pub fn of(q: &CommitQuery, c: &CommitInfo, body: Option<&str>) -> RowRuns {
        let text = q.text();
        let name = c.author.name();
        let mut author = runs(name, text);
        for a in q.authors() {
            let found = runs(name, a);
            // A term that matched only the address (a picked email, say) marks the whole name.
            if found.is_empty() && contains_lower(c.author.email(), a) {
                author.push(0..name.chars().count());
            }
            author.extend(found);
        }
        let sha = match text.len() {
            n @ 1..=40 if hex_starts_with(c.oid.as_bytes(), text) => std::iter::once(0..n.min(7)).collect(),
            _ => Vec::new(),
        };
        RowRuns { summary: runs(&c.summary, text), body: body.map_or_else(Vec::new, |b| runs(b, text)), author: merge(author), sha }
    }
}

/// The char ranges of `hay` where lowercase `needle` occurs, ignoring case (each char compared by
/// its first lowercase char), merged where they touch or overlap.
pub(super) fn runs(hay: &str, needle: &str) -> Runs {
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() {
        return Vec::new();
    }
    let h: Vec<char> = hay.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect();
    let found = (0..h.len().saturating_sub(n.len() - 1)).filter(|&i| h[i..i + n.len()] == n[..]).map(|i| i..i + n.len());
    merge(found.collect())
}

/// `r` sorted, with ranges that touch or overlap joined.
fn merge(mut r: Runs) -> Runs {
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
pub(super) fn ink_runs(job: LayoutJob, runs: &[Range<usize>], ink: Color32) -> LayoutJob {
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
pub(super) fn paint_cut_marked(p: &Painter, at: Pos2, job: LayoutJob, max_w: f32, runs: &[Range<usize>], mark: Option<Mark>) -> Cut {
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
}
