//! A commit row's text cells: the message (pull request chip, then the summary and the dim body
//! preview as one line), changes, the commit (the author with the short SHA under it) and the
//! date, right-aligned. Each is cut to its column with "…"; `RowText` says which were cut, for
//! the row's tooltip.

use super::paint::Columns;
use crate::ui::common::{added_color, removed_color};
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::truncate::{cut_galley, cut_job};
use crate::ui::widgets::{self, Cut, paint_cut, paint_cut_text};
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Align, Color32, FontId, Label, Painter, Pos2, Rect, RichText, Ui, pos2};
use trellis_core::repo::CommitInfo;
use trellis_core::settings::GraphColumns;

/// The widest the hover with a cut message gets before it wraps.
const TIP_W: f32 = 420.0;
/// How far the author line sits above the row's middle, and the SHA line below it, when both show.
const AUTHOR_DY: f32 = -7.0;
const SHA_DY: f32 = 8.0;

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
    pub commit: &'a CommitInfo,
    /// The pull request chip's label, and whether it is a draft.
    pub pr: Option<&'a (String, bool)>,
    pub show_body: bool,
    /// The message's colour and the dim colour (both faded on rows a search dims).
    pub colors: (Color32, Color32),
    /// The formatted date, when the DATE column shows.
    pub date: Option<String>,
    /// Files changed, lines added and lines deleted, once read.
    pub stats: Option<(usize, usize, usize)>,
}

/// The summary in `fg` and, when given, the body preview after it in the smaller `weak` text.
pub(super) fn message_job(summary: &str, body: Option<&str>, fg: Color32, weak: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(summary, 0.0, TextFormat { valign: Align::Center, ..TextFormat::simple(type_scale::font(type_scale::BODY), fg) });
    if let Some(body) = body.filter(|b| !b.is_empty()) {
        job.append(body, 10.0, TextFormat { valign: Align::Center, ..TextFormat::simple(type_scale::font(type_scale::SMALL), weak) });
    }
    job
}

/// Paints the row's text cells, each clipped and cut to its column.
pub(super) fn draw(painter: &Painter, row: &Cells) -> RowText {
    let (cols, rect, cfg, c) = (row.cols, row.rect, row.cfg, row.commit);
    let (fg, weak) = row.colors;
    let mid = rect.center().y;
    let width = |col: (f32, f32)| col.1 - col.0;
    let mut text = RowText::default();
    if cfg.message {
        let mp = painter.with_clip_rect(cols.rect(cols.msg, rect));
        let mut x = cols.msg.0;
        if let Some((label, draft)) = row.pr {
            let col = if *draft { color::weak() } else { color::progress() };
            x += widgets::chip(&mp, x, mid, label, col.gamma_multiply(0.35), col) + 6.0;
        }
        let body = row.show_body.then_some(c.body.as_str());
        text.msg = paint_cut(&mp, pos2(x, mid), message_job(&c.summary, body, fg, weak), cols.msg.1 - x);
    }
    if cfg.changes
        && let Some((_files, adds, dels)) = row.stats
    {
        let cp = painter.with_clip_rect(cols.rect(cols.changes, rect));
        let font = type_scale::font(type_scale::SMALL);
        let added = paint_cut_text(&cp, pos2(cols.changes.0, mid), &format!("+{adds}"), font.clone(), added_color(), width(cols.changes));
        if !added.cut {
            let x = added.rect.right() + 6.0;
            paint_cut_text(&cp, pos2(x, mid), &format!("−{dels}"), font, removed_color(), cols.changes.1 - x);
        }
    }
    if cfg.commit {
        let cp = painter.with_clip_rect(cols.rect(cols.commit, rect));
        let w = width(cols.commit);
        let author_y = if cfg.commit_sha { mid + AUTHOR_DY } else { mid };
        text.author = paint_cut_text(&cp, pos2(cols.commit.0, author_y), &c.author, type_scale::font(type_scale::SMALL), weak, w);
        if cfg.commit_sha {
            let sha = &c.oid.to_string()[..7];
            text.sha = paint_cut_text(&cp, pos2(cols.commit.0, mid + SHA_DY), sha, type_scale::mono(type_scale::SMALL), color::faint(), w);
        }
    }
    if let Some(date) = row.date.as_deref().filter(|_| cfg.date) {
        let dp = painter.with_clip_rect(cols.rect(cols.date, rect));
        // A cut date keeps the row's date tooltip, which has the whole date.
        paint_cut_right(&dp, pos2(cols.date.1, mid), date, type_scale::font(type_scale::SMALL), weak, width(cols.date));
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
/// showed it, wrapped.
pub(super) fn message_tip(ui: &mut Ui, c: &CommitInfo, with_body: bool) {
    ui.set_max_width(TIP_W);
    ui.add(Label::new(RichText::new(&c.summary).font(type_scale::strong(ui, type_scale::BODY))).wrap());
    if with_body && !c.body.is_empty() {
        ui.add(Label::new(RichText::new(&c.body).color(color::weak())).wrap());
    }
}
