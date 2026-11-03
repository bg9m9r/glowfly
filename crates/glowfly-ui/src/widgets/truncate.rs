//! Single-line text cut to a width with an ellipsis, with the whole text on hover when cut (a
//! counted title, "Staged files (3)", is cut before its count, never in it).
//!
//! egui ends a cut line on the last whole glyph that fits followed by "…" and marks the galley
//! `elided`; `galley.text()` keeps the whole text, for tooltips and accessible names.
//!
//! Names that often share a long start (branches, tags, worktrees, repositories) are cut in
//! their middle instead (`middle`): a short head ending before a separator, "…", and a tail that
//! fills the rest, so `worktree-agent-a1…` names stay apart.

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, Color32, FontId, Galley, Painter, Pos2, Rect, Response, Sense, TextStyle, TextWrapMode, Ui, WidgetText, pos2, vec2,
};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

/// The overflow character egui draws at the end of a cut line.
const ELLIPSIS: char = '…';

/// `text` as a one-line job in `font` and `color`, ready to be cut.
pub fn cut_job(text: impl Into<String>, font: FontId, color: Color32) -> LayoutJob {
    LayoutJob::simple_singleline(text.into(), font, color)
}

/// `job` laid out on one line no wider than `max_w`, ending in "…" when it doesn't fit.
pub fn cut_galley(ctx: &egui::Context, mut job: LayoutJob, max_w: f32) -> Arc<Galley> {
    job.wrap = TextWrapping::truncate_at_width(max_w.max(0.0));
    ctx.fonts(|f| f.layout_job(job))
}

/// Where a cut line was painted, and whether it was cut.
#[derive(Clone, Copy, Debug)]
pub struct Cut {
    pub rect: Rect,
    pub cut: bool,
}

impl Default for Cut {
    /// Nothing painted (a hidden cell).
    fn default() -> Self {
        Self { rect: Rect::NOTHING, cut: false }
    }
}

impl Cut {
    /// Whether the pointer is over a cut line (one that has the whole text to show on hover).
    pub fn hovered(&self, pointer: Option<Pos2>) -> bool {
        self.cut && pointer.is_some_and(|p| self.rect.contains(p))
    }
}

/// Paints `job` from `at` (its left end, vertically centred on `at.y`), cut to `max_w`. Below
/// the width of one "…" (or for a NaN width) nothing is painted, and non-empty text counts as cut.
pub fn paint_cut(p: &Painter, at: Pos2, job: LayoutJob, max_w: f32) -> Cut {
    let Some(first) = job.sections.first() else {
        return Cut { rect: Rect::from_min_size(at, vec2(0.0, 0.0)), cut: false };
    };
    let fallback = first.format.color;
    let ellipsis_w = p.ctx().fonts(|f| f.glyph_width(&first.format.font_id, ELLIPSIS));
    if max_w.is_nan() || max_w < ellipsis_w {
        return Cut { rect: Rect::from_min_size(at, vec2(0.0, 0.0)), cut: !job.text.is_empty() };
    }
    let galley = cut_galley(p.ctx(), job, max_w);
    let min = pos2(at.x, at.y - galley.size().y / 2.0);
    let cut = Cut { rect: Rect::from_min_size(min, galley.size()), cut: galley.elided };
    p.galley(min, galley, fallback);
    cut
}

/// One line of `text` in `font` and `color`, painted as `paint_cut` does.
pub fn paint_cut_text(p: &Painter, at: Pos2, text: &str, font: FontId, color: Color32, max_w: f32) -> Cut {
    paint_cut(p, at, cut_job(text, font, color), max_w)
}

/// A non-selectable one-line label cut to `max_w` (and the room left), named by its whole text.
/// In a wrapping row the room is a whole row (`available_width` there), so a label that doesn't
/// fit what is left of the row moves onto the next one instead of being cut.
/// Returns its response and whether it was cut; it attaches no tooltip, so the caller decides.
pub fn cut_label(ui: &mut Ui, text: impl Into<WidgetText>, max_w: f32) -> (Response, bool) {
    let width = max_w.min(ui.available_width()).max(0.0);
    let galley = text.into().into_galley(ui, Some(TextWrapMode::Truncate), width, TextStyle::Body);
    let (rect, resp) = ui.allocate_exact_size(galley.size(), Sense::hover());
    let full = galley.text().to_owned();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), &full));
    let cut = galley.elided;
    if ui.is_rect_visible(rect) {
        ui.painter().galley(rect.min, galley, ui.visuals().text_color());
    }
    (resp, cut)
}

/// `cut_label` with the whole text on hover when it was cut.
pub fn truncate_label(ui: &mut Ui, text: impl Into<WidgetText>, max_w: f32) -> Response {
    let text = text.into();
    let full = text.text().to_owned();
    let (resp, cut) = cut_label(ui, text, max_w);
    if cut { resp.on_hover_text(full) } else { resp }
}

/// `text` split before a count at its end, as in "Staged files (3)": the label and the count
/// with the space before it (" (3)"), or the whole text and "" when it doesn't end in one.
pub fn split_count(text: &str) -> (&str, &str) {
    let Some(inner) = text.strip_suffix(')') else { return (text, "") };
    let Some(open) = inner.rfind(" (") else { return (text, "") };
    let digits = &inner[open + 2..];
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return (text, "");
    }
    (&text[..open], &text[open..])
}

/// `text` on one line in `max_w`, cut before its count (`split_count`) when it doesn't fit: the
/// label ends in "…" and the count follows whole. One galley either way, and whether it was cut.
pub(crate) fn counted_galley(ctx: &egui::Context, text: &str, font: FontId, color: Color32, max_w: f32) -> (Arc<Galley>, bool) {
    let whole = ctx.fonts(|f| f.layout_no_wrap(text.to_owned(), font.clone(), color));
    if whole.size().x <= max_w {
        return (whole, false);
    }
    let (label, count) = split_count(text);
    let count_w = ctx.fonts(|f| f.layout_no_wrap(count.to_owned(), font.clone(), color)).size().x;
    let cut = cut_galley(ctx, cut_job(label, font.clone(), color), (max_w - count_w).max(0.0));
    let shown: String = cut.rows.iter().flat_map(|r| r.glyphs.iter().map(|g| g.chr)).collect();
    (ctx.fonts(|f| f.layout_no_wrap(format!("{shown}{count}"), font, color)), true)
}

/// A non-selectable one-line label cut to `max_w` (and the room left) whose count is never cut:
/// the label before it takes the cut ("Nicht bereitg… (1)"), down to just "…". Named by the whole
/// text, which shows on hover when cut.
pub fn cut_counted_label(ui: &mut Ui, text: &str, font: FontId, color: Color32, max_w: f32) -> Response {
    let width = max_w.min(ui.available_width()).max(0.0);
    let (galley, cut) = counted_galley(ui.ctx(), text, font, color, width);
    let (rect, resp) = ui.allocate_exact_size(galley.size(), Sense::hover());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), text));
    if ui.is_rect_visible(rect) {
        ui.painter().galley(rect.min, galley, color);
    }
    if cut { resp.on_hover_text(text) } else { resp }
}

/// `text` with characters taken off its start, behind "…", until it fits in `max_w` in `font`
/// (just "…" when nothing does): a path whose end matters more than its start.
pub fn cut_left(ui: &Ui, text: &str, font: &FontId, max_w: f32) -> String {
    cut_left_in(ui.painter(), text, font, max_w)
}

/// `cut_left`, measured with painter `p` (for text painted outside a widget).
pub fn cut_left_in(p: &Painter, text: &str, font: &FontId, max_w: f32) -> String {
    let width = |s: &str| p.layout_no_wrap(s.to_owned(), font.clone(), Color32::PLACEHOLDER).size().x;
    if width(text) <= max_w {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    for start in 1..chars.len() {
        let cut = format!("{ELLIPSIS}{}", chars[start..].iter().collect::<String>());
        if width(&cut) <= max_w {
            return cut;
        }
    }
    ELLIPSIS.to_string()
}

/// `text` with characters taken out of its middle, behind "…", until it fits in `max_w` in
/// `font` (just "…" when not even one character each side does): a branch name whose start and
/// end both matter. Works on whole characters, from the longer side (the start on a tie).
pub fn cut_middle_in(p: &Painter, text: &str, font: &FontId, max_w: f32) -> String {
    let width = |s: &str| p.layout_no_wrap(s.to_owned(), font.clone(), Color32::PLACEHOLDER).size().x;
    if width(text) <= max_w {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut head, mut tail) = (chars.len() / 2, chars.len() / 2);
    while head > 0 && tail > 0 {
        let cut: String = chars[..head].iter().chain(&[ELLIPSIS]).chain(&chars[chars.len() - tail..]).collect();
        if width(&cut) <= max_w {
            return cut;
        }
        if head >= tail {
            head -= 1;
        } else {
            tail -= 1;
        }
    }
    ELLIPSIS.to_string()
}

/// The characters a middle cut's head ends before (the last one within its share).
const SEPARATORS: [&str; 4] = ["/", "-", "_", "."];
/// The share of the room a middle cut's head may take.
const HEAD_SHARE: f32 = 0.4;
/// A head ends before a separator only when that keeps at least this much of its share.
const SNAP_MIN: f32 = 0.5;
/// The fewest characters a middle cut's tail keeps.
const MIN_TAIL: usize = 8;
/// The fewest characters a middle cut's head gives way to before the name is cut at its end.
const MIN_HEAD: usize = 3;

/// Where a middle cut splits `text` to fit `max_w` as `width` measures it: the byte end of the
/// head and the byte start of the tail ("…" goes between), or None when `text` fits whole.
/// The head is the first 40% of the room, ending before its last separator (`/ - _ .`, or the
/// run of them it ends) when that keeps at least half of it; the tail fills the rest. A tail under 8 characters takes
/// room from the head, down to 3; when even 3 + "…" + 8 don't fit, the name is cut at its end
/// instead (the tail is then empty: `text.len()`). Cuts land on grapheme boundaries.
pub fn middle_cut(text: &str, max_w: f32, width: impl Fn(&str) -> f32) -> Option<(usize, usize)> {
    if text.is_empty() || width(text) <= max_w {
        return None;
    }
    // bounds[k]: the byte offset after k graphemes.
    let bounds: Vec<usize> = text.grapheme_indices(true).map(|(i, _)| i).chain([text.len()]).collect();
    let n = bounds.len() - 1;
    let fits = |head: usize, tail: usize| width(&format!("{}{ELLIPSIS}{}", &text[..bounds[head]], &text[bounds[n - tail]..])) <= max_w;
    let share = max_w * HEAD_SHARE;
    let mut head = 0;
    while head < n && width(&text[..bounds[head + 1]]) <= share {
        head += 1;
    }
    let snaps = |k: usize| SEPARATORS.contains(&&text[bounds[k]..bounds[k + 1]]) && width(&text[..bounds[k]]) >= share * SNAP_MIN;
    if let Some(mut k) = (1..=head.min(n - 1)).rev().find(|&k| snaps(k)) {
        // Before a whole run of separators ("please--branches"), not inside it.
        while k > 1 && snaps(k - 1) {
            k -= 1;
        }
        head = k;
    }
    // At least one grapheme always goes (the whole text doesn't fit).
    let most = |head: usize| n - head - 1;
    while head > MIN_HEAD && !fits(head, MIN_TAIL.min(most(head))) {
        head -= 1;
    }
    let mut tail = MIN_TAIL.min(most(head));
    if !fits(head, tail) {
        let end = (0..n).rev().find(|&h| fits(h, 0)).unwrap_or(0);
        return Some((bounds[end], text.len()));
    }
    while tail < most(head) && fits(head, tail + 1) {
        tail += 1;
    }
    Some((bounds[head], bounds[n - tail]))
}

/// `text` cut in its middle to fit `max_w` in `font` (`middle_cut`): unchanged when it fits,
/// just "…" when nothing else does. For names, never prose or file paths.
pub fn middle(p: &Painter, text: &str, font: &FontId, max_w: f32) -> String {
    let width = |s: &str| p.layout_no_wrap(s.to_owned(), font.clone(), Color32::PLACEHOLDER).size().x;
    match middle_cut(text, max_w, width) {
        None => text.to_owned(),
        Some((head, tail)) => format!("{}{ELLIPSIS}{}", &text[..head], &text[tail..]),
    }
}

/// `paint_cut_text` for a name: cut in its middle (`middle`) rather than at its end.
pub fn paint_middle(p: &Painter, at: Pos2, text: &str, font: FontId, color: Color32, max_w: f32) -> Cut {
    let ellipsis_w = p.ctx().fonts(|f| f.glyph_width(&font, ELLIPSIS));
    if max_w.is_nan() || max_w < ellipsis_w {
        return Cut { rect: Rect::from_min_size(at, vec2(0.0, 0.0)), cut: !text.is_empty() };
    }
    let shown = middle(p, text, &font, max_w);
    let cut = shown != text;
    let galley = p.layout_no_wrap(shown, font, color);
    let min = pos2(at.x, at.y - galley.size().y / 2.0);
    let rect = Rect::from_min_size(min, galley.size());
    p.galley(min, galley, color);
    Cut { rect, cut }
}

/// A folder path cut to `max_w` at whole folders: its first folder (a drive, a share's server,
/// `~`, with the separators before it), "…", and as many of its last folders as fit
/// (`C:\…\team\services`), split at `sep`. A path of two folders, or one whose first and last
/// folders alone don't fit, is cut in the middle instead (`cut_middle_in`).
pub fn cut_path_in(p: &Painter, text: &str, font: &FontId, max_w: f32, sep: char) -> String {
    let width = |s: &str| p.layout_no_wrap(s.to_owned(), font.clone(), Color32::PLACEHOLDER).size().x;
    if width(text) <= max_w {
        return text.to_owned();
    }
    let parts: Vec<&str> = text.split(sep).collect();
    let lead = parts.iter().take_while(|s| s.is_empty()).count();
    if parts.len() < lead + 3 {
        return cut_middle_in(p, text, font, max_w);
    }
    let head = parts[..=lead].join(&sep.to_string());
    let rest = &parts[lead + 1..];
    for keep in (1..rest.len()).rev() {
        let cut = format!("{head}{sep}{ELLIPSIS}{sep}{}", rest[rest.len() - keep..].join(&sep.to_string()));
        if width(&cut) <= max_w {
            return cut;
        }
    }
    cut_middle_in(p, text, font, max_w)
}

/// A text field's hint cut to the field: `TextEdit::hint_text` keeps a laid-out galley as it
/// is (it lays a plain hint out on one uncut line). Drawn in egui's weak hint colour. Returns
/// the hint and whether it was cut, for the caller's hover with the whole hint.
pub fn hint(ui: &Ui, text: &str, field_w: f32) -> (WidgetText, bool) {
    // The field keeps a 4 px margin on each side of its text.
    let room = field_w.min(ui.available_width() - 8.0);
    let job = cut_job(text, TextStyle::Body.resolve(ui.style()), Color32::PLACEHOLDER);
    let galley = cut_galley(ui.ctx(), job, room);
    let cut = galley.elided;
    (WidgetText::Galley(galley), cut)
}

/// A row's tooltip: its own `tip`, with the whole `text` as the first line when the row's text
/// was cut (the whole text alone when it has no tooltip of its own).
pub fn full_tip(cut: bool, text: &str, tip: Option<String>) -> Option<String> {
    match (cut, tip) {
        (true, Some(tip)) => Some(format!("{text}\n{tip}")),
        (true, None) => Some(text.to_owned()),
        (false, tip) => tip,
    }
}

#[cfg(test)]
#[path = "../snapshot_tests/snapshot_tests_truncate.rs"]
mod snapshot_tests;

#[cfg(test)]
#[path = "truncate_tests.rs"]
pub(crate) mod tests;
