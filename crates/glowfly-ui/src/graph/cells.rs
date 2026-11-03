//! A commit row's text cells, all on one line: the message (pull request chip, then the summary
//! and the dim body preview), changes, the author's name (`weak`), the short SHA (mono, `faint`)
//! and the date, right-aligned (Noto Sans's figures are tabular, so ages and dates line up).
//! Each is cut to its column with "…"; `RowText` says which were cut, for the row's tooltip.

use super::find_marks::{Mark, RowRuns, Runs, paint_cut_marked};
use super::layout::Columns;
use crate::state::ForgeState;
use crate::ui::common::{added_color, removed_color};
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::truncate::{cut_galley, cut_job};
use crate::ui::widgets::{Cut, chip, paint_cut_text};
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Align, Color32, FontId, Label, Painter, Pos2, Rect, RichText, Ui, pos2};
use git2::Oid;
use glowfly_core::repo::{CommitInfo, RefLabel};
use glowfly_core::settings::{GraphColumns, Settings};
use std::collections::HashMap;

/// The widest the hover with a cut message gets before it wraps.
const TIP_W: f32 = 420.0;

/// Where the row's cut-able cells were painted, and whether each was cut (empty when hidden).
#[derive(Default)]
pub(super) struct RowText {
    pub msg: Cut,
    pub sha: Cut,
    pub author: Cut,
}

/// What one commit row's cells show.
pub(super) struct Cells<'a> {
    pub cols: &'a Columns,
    pub rect: Rect,
    pub cfg: &'a GraphColumns,
    /// Where the message starts: its column's content, or after the chips leading it.
    pub msg_x: f32,
    pub commit: &'a CommitInfo,
    /// The pull request chip's label, and whether it is a draft.
    pub pr: Option<&'a (String, bool)>,
    pub show_body: bool,
    /// The colours of the row's text (quieter on rows a search dims).
    pub inks: Inks,
    /// Where a search matched the row's text, and how the matches are marked.
    pub found: Option<(&'a RowRuns, Mark)>,
    /// The formatted date, when the DATE column shows.
    pub date: Option<String>,
    /// Files changed, lines added and lines deleted, once read.
    pub stats: Option<(usize, usize, usize)>,
}

/// The colours of a commit row's text.
#[derive(Clone, Copy)]
pub(super) struct Inks {
    pub summary: Color32,
    /// The body preview.
    pub body: Color32,
    /// The author's name and the date.
    pub meta: Color32,
    pub sha: Color32,
    /// A search dims the row: the change counts are `faint` and the PR chip neutral.
    pub dimmed: bool,
}

impl Inks {
    /// A row's inks: its summary in `summary` (`strong` when selected), or on a row a search
    /// dims, `row_look::dim_inks`' message and metadata colours.
    pub fn new(summary: Color32, dimmed: bool) -> Inks {
        let pal = color::current();
        match dimmed {
            false => Inks { summary, body: pal.weak, meta: pal.weak, sha: pal.faint, dimmed },
            true => {
                let (message, meta) = super::row_look::dim_inks(&pal);
                Inks { summary: message, body: meta, meta, sha: meta, dimmed }
            }
        }
    }
}

/// Names commit row `resp` by `c`'s summary, and describes it by what the row shows after it:
/// "Ada Lovelace, 222c0ad, Jan 10, main" (author, short SHA, `date`, then its refs).
pub(super) fn name_row(resp: &egui::Response, c: &CommitInfo, refs: Option<&Vec<RefLabel>>, date: String) {
    crate::ui::access::name_button(resp, true, &c.summary);
    let mut parts = vec![c.author.name().to_owned(), crate::ui::common::short(c.oid), date];
    parts.extend(refs.into_iter().flatten().map(|r| r.name.clone()));
    crate::ui::access::describe(resp, &parts.join(", "));
}

/// Each open pull request's chip label ("PR #12", "Draft #3") and whether it is a draft, by
/// its head commit.
pub(super) fn pr_badges(f: &ForgeState) -> HashMap<Oid, (String, bool)> {
    let word = if f.pr_noun() == "merge request" { "MR" } else { "PR" };
    let label = |p: &glowfly_core::forge::PullRequest| format!("{} {}", if p.draft { "Draft" } else { word }, f.pr_label(p.id));
    f.prs.iter().filter_map(|p| Some((*f.pr_heads.get(&p.id)?, (label(p), p.draft)))).collect()
}

/// Each month's 20th at 22:58 UTC in 2026: two-digit days (the figures are tabular) late in
/// the day, so each month's dates are about as wide as it shows them.
const WIDE_DAYS: [i64; 12] = [
    1_768_949_880,
    1_771_628_280,
    1_774_047_480,
    1_776_725_880,
    1_779_317_880,
    1_781_996_280,
    1_784_588_280,
    1_787_266_680,
    1_789_945_080,
    1_792_537_080,
    1_795_215_480,
    1_797_807_480,
];

/// The widest dates the DATE column shows, for its least width: each month as the graph formats
/// it at `now` (absolute dates), or, while dates are ages, its short form: with the year ("Dec
/// 20, 2025") when the `oldest` loaded commit is from before `now`'s year in the zone `offset`
/// from UTC, else without ("Dec 20").
pub(super) fn wide_dates(s: &Settings, now: i64, offset: i32, oldest: Option<i64>) -> Vec<String> {
    let year = |t: i64| chrono::DateTime::from_timestamp(t + i64::from(offset), 0).map(|d| chrono::Datelike::year(&d));
    let earlier = oldest.is_some_and(|o| year(o) < year(now));
    let seen_at = |t: i64| match (s.relative_dates, earlier) {
        (false, _) => now,
        (true, true) => t + 400 * 86_400,
        (true, false) => t + 8 * 86_400,
    };
    WIDE_DAYS.iter().map(|&t| s.format_graph_time(t, seen_at(t), 0)).collect()
}

/// The summary in `fg` and, when given, the body preview after it in the smaller `weak` text.
pub(super) fn message_job(summary: &str, body: Option<(&str, bool)>, fg: Color32, weak: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(summary, 0.0, TextFormat { valign: Align::Center, ..TextFormat::simple(type_scale::font(type_scale::BODY), fg) });
    if let Some((body, cut)) = body.filter(|b| !b.0.is_empty()) {
        let format = TextFormat { valign: Align::Center, ..TextFormat::simple(type_scale::font(type_scale::SMALL), weak) };
        job.append(body, 10.0, format.clone());
        if cut {
            job.append("…", 0.0, format);
        }
    }
    job
}

/// Paints the row's text cells, each clipped and cut to its column.
pub(super) fn draw(painter: &Painter, row: &Cells) -> RowText {
    let (cols, rect, cfg, c) = (row.cols, row.rect, row.cfg, row.commit);
    let ink = row.inks;
    let none = Vec::new();
    let runs = |pick: fn(&RowRuns) -> &Runs| row.found.map_or(&none, |(r, _)| pick(r));
    let mark = row.found.map(|(_, m)| m);
    let mid = rect.center().y;
    let width = |col: (f32, f32)| col.1 - col.0;
    let mut text = RowText::default();
    if cfg.message {
        let mp = painter.with_clip_rect(cols.rect(cols.msg, rect));
        let mut x = row.msg_x;
        if let Some((label, draft)) = row.pr {
            let col = if *draft || ink.dimmed { color::weak() } else { color::progress() };
            let look = chip::Look::new(color::wash(col, 0.35), col);
            x = chip::Chip::new(chip::ChipSize::Badge, label.as_str()).look(look).paint(&mp, x, mid).right() + 6.0;
        }
        let body = row.show_body.then(|| c.body_preview());
        // The body's runs follow the summary's chars in the job.
        let n = c.summary.chars().count();
        let msg_runs: Runs = runs(|r| &r.summary).iter().cloned().chain(runs(|r| &r.body).iter().map(|b| b.start + n..b.end + n)).collect();
        let job = message_job(&c.summary, body, ink.summary, ink.body);
        text.msg = paint_cut_marked(&mp, pos2(x, mid), job, cols.msg.1 - x, &msg_runs, mark);
    }
    if cfg.changes
        && let Some((_files, adds, dels)) = row.stats
    {
        let cp = painter.with_clip_rect(cols.rect(cols.changes, rect));
        let font = type_scale::font(type_scale::SMALL);
        let (plus, minus) = if ink.dimmed { (ink.meta, ink.meta) } else { (added_color(), removed_color()) };
        let added = paint_cut_text(&cp, pos2(cols.changes.0, mid), &format!("+{adds}"), font.clone(), plus, width(cols.changes));
        if !added.cut {
            let x = added.rect.right() + 6.0;
            paint_cut_text(&cp, pos2(x, mid), &format!("−{dels}"), font, minus, cols.changes.1 - x);
        }
    }
    if cfg.author {
        let ap = painter.with_clip_rect(cols.rect(cols.author, rect));
        let font = type_scale::font(type_scale::SMALL);
        let job = cut_job(c.author.name(), font, ink.meta);
        text.author = paint_cut_marked(&ap, pos2(cols.author.0, mid), job, width(cols.author), runs(|r| &r.author), mark);
    }
    if cfg.commit {
        let cp = painter.with_clip_rect(cols.rect(cols.commit, rect));
        let sha = &c.oid.to_string()[..7];
        let font = type_scale::mono(type_scale::SMALL);
        text.sha = paint_cut_marked(&cp, pos2(cols.commit.0, mid), cut_job(sha, font, ink.sha), width(cols.commit), runs(|r| &r.sha), mark);
    }
    if let Some(date) = row.date.as_deref().filter(|_| cfg.date) {
        let dp = painter.with_clip_rect(cols.rect(cols.date, rect));
        // A cut date keeps the row's date tooltip, which has the whole date.
        paint_cut_right(&dp, pos2(cols.date.1, mid), date, type_scale::font(type_scale::SMALL), ink.meta, width(cols.date));
    }
    text
}

/// One line of `text` with its right end at `right` (vertically centred on `right.y`), cut to
/// `max_w` with "…".
fn paint_cut_right(p: &Painter, right: Pos2, text: &str, font: FontId, color: Color32, max_w: f32) -> Cut {
    let galley = cut_galley(p.ctx(), cut_job(text, font, color), max_w);
    let min = pos2(right.x - galley.size().x, right.y - galley.size().y / 2.0);
    let cut = Cut { rect: Rect::from_min_size(min, galley.size()), cut: galley.elided };
    p.galley(min, galley, color);
    cut
}

/// The hover of a cut message: the summary in strong text, then the body preview when the row
/// showed it, wrapped (the start of the body, as the row shows it, not all of a long one).
pub(super) fn message_tip(ui: &mut Ui, c: &CommitInfo, with_body: bool) {
    ui.set_max_width(TIP_W);
    ui.add(Label::new(RichText::new(&c.summary).font(type_scale::strong(ui, type_scale::BODY))).wrap());
    if with_body && !c.body.is_empty() {
        let text = match c.body_preview() {
            (start, true) => format!("{start}…"),
            (all, false) => all.to_owned(),
        };
        ui.add(Label::new(RichText::new(text).color(color::weak())).wrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    #[test]
    fn date_figures_are_tabular() {
        let ctx = egui::Context::default();
        crate::ui::theme::fonts::install(&ctx);
        // The fonts arrive on the pass after they are set.
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let w = |d: char| ctx.fonts(|f| f.layout_no_wrap(d.to_string(), type_scale::font(type_scale::SMALL), Color32::WHITE).size().x);
            let one = w('1');
            for d in '0'..='9' {
                assert!((w(d) - one).abs() < 0.01, "{d} is {} wide, 1 is {one}", w(d));
            }
        });
    }
}
