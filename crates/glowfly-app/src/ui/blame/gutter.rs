//! A run's gutter: its author, date, commit and summary, the age strip and tint, and the hover
//! text with the commit's whole message.

use crate::state::AppState;
use crate::ui::common::short;
use crate::ui::theme::color;
use crate::ui::widgets::paint_cut_text;
use eframe::egui::{Align2, FontId, Painter, Rect, Ui, pos2, vec2};
use glowfly_core::repo::BlameCommit;

/// Width of the author, date and commit gutter.
pub(super) const GUTTER_W: f32 = 300.0;
/// Space between the gutter's edges and its text.
pub(super) const PAD: f32 = 8.0;
/// Width of the age strip at the gutter's left edge.
const STRIP_W: f32 = 3.0;
/// The most of a commit body the hover shows.
const BODY_LINES: usize = 30;
const BODY_CHARS: usize = 2000;

pub(super) struct Fonts {
    pub name: FontId,
    pub sha: FontId,
}

/// The run's commit on `first` (author, then date and short SHA at the right) and its summary on
/// `second`.
pub(super) fn text(ui: &Ui, st: &AppState, fonts: &Fonts, c: &BlameCommit, first: Rect, second: Option<Rect>) {
    let (left, right) = (first.left() + PAD, first.left() + GUTTER_W - PAD);
    let y = first.center().y;
    let sha = ui.painter().text(pos2(right, y), Align2::RIGHT_CENTER, short(c.oid), fonts.sha.clone(), color::weak());
    let date = st.settings.format_day(c.time, st.view.now, st.view.utc_offset);
    let date = ui.painter().text(pos2(sha.left() - 10.0, y), Align2::RIGHT_CENTER, date, fonts.name.clone(), color::weak());
    let author = if c.author.is_empty() { st.settings.lang().tr("blame.gutter.unknown") } else { c.author.as_str() };
    paint_cut_text(ui.painter(), pos2(left, y), author, fonts.name.clone(), color::text(), date.left() - 10.0 - left);
    if let Some(r) = second {
        paint_cut_text(ui.painter(), pos2(left, r.center().y), &c.summary, fonts.name.clone(), color::faint(), right - left);
    }
}

/// How new the run's commit is (`t`: 0 oldest, 1 newest): a strip at the gutter's left edge and
/// a light tint over it, both stronger for newer commits.
pub(super) fn paint_age(painter: &Painter, gutter: Rect, t: f32) {
    let t = t.clamp(0.0, 1.0);
    painter.rect_filled(gutter, 0.0, color::accent().gamma_multiply(0.03 + 0.12 * t));
    let strip = Rect::from_min_size(gutter.min, vec2(STRIP_W, gutter.height()));
    painter.rect_filled(strip, 0.0, color::accent().gamma_multiply(0.25 + 0.75 * t));
}

/// The body cut to `BODY_LINES` lines and `BODY_CHARS` characters, with "…" when cut.
fn cut_body(body: &str) -> String {
    let mut out: String = body.lines().take(BODY_LINES).collect::<Vec<_>>().join("\n");
    let mut cut = body.lines().count() > BODY_LINES;
    if let Some((i, _)) = out.char_indices().nth(BODY_CHARS) {
        out.truncate(i);
        cut = true;
    }
    if cut {
        out.push('…');
    }
    out
}

/// The gutter's hover: the author, the date, the whole message, and what clicks do.
pub(super) fn tooltip(st: &AppState, c: &BlameCommit) -> String {
    let t = st.settings.lang();
    let author = if c.author.is_empty() { t.tr("blame.gutter.unknown") } else { c.author.as_str() };
    let when = st.settings.format_verbose(c.time, st.view.utc_offset);
    let mut tip = format!("{author} <{}>\n{when}\n\n{}", c.email, c.summary);
    if !c.body.is_empty() {
        tip.push_str("\n\n");
        tip.push_str(&cut_body(&c.body));
    }
    tip.push_str("\n\n");
    tip.push_str(t.tr("blame.gutter.hint"));
    tip
}

#[cfg(test)]
mod tests {
    use super::cut_body;

    #[test]
    fn long_bodies_are_cut() {
        assert_eq!(cut_body("one\ntwo"), "one\ntwo");
        let many: String = (0..40).map(|i| format!("line {i}\n")).collect();
        let cut = cut_body(&many);
        assert_eq!(cut.lines().count(), 30);
        assert!(cut.ends_with("line 29…"), "{cut}");
        let wide = "é".repeat(2500);
        let cut = cut_body(&wide);
        assert_eq!(cut.chars().count(), 2001);
        assert!(cut.ends_with('…'));
    }
}
