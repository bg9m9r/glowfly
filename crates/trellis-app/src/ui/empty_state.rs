//! A calm placeholder for a pane with nothing to draw (a binary file's diff, a dashboard with no
//! repositories, a search with no matches): an outline icon, a title and a one-line hint,
//! centred and set about a third of the way down the space left, with an optional row of
//! buttons under it. The loading and error forms of the same block are in `pane_status.rs`;
//! the icons are in `empty_state_icons.rs`.

pub use super::empty_state_icons::EmptyIcon;
pub(super) use super::empty_state_icons::paint;
use super::theme::color;
use super::widgets::buttons::{self, StyledButton};
use super::widgets::severity::{self, Severity};
use eframe::egui::{self, Align, Color32, FontId, Layout, RichText, Ui, UiBuilder, pos2, vec2};
use trellis_core::lfs::pointer::size_text;

/// The width the hint wraps at.
const HINT_WIDTH: f32 = 360.0;
/// The icon's square.
const ICON: f32 = 40.0;
/// The spinner's square, in the icon's place.
const SPINNER: f32 = 24.0;
/// The least space above the block, on a short pane (and in a scroll area).
const MIN_TOP: f32 = 24.0;
/// The height of a row of buttons.
const BUTTON_H: f32 = 28.0;

pub(super) const BINARY_TITLE: &str = "Binary file";
pub(super) const BINARY_HINT: &str = "Changes inside binary files aren't shown.";
pub(super) const WHITESPACE_TITLE: &str = "Only whitespace changed";
pub(super) const WHITESPACE_HINT: &str = "Ignore whitespace is hiding the changes in this file.";
pub(super) const SHOW_WHITESPACE: &str = "Show whitespace changes";
pub(super) const NO_CHANGES_TITLE: &str = "No changes to show";
pub(super) const NO_CHANGES_HINT: &str = "Only the file's mode changed, or the file is empty.";

/// A binary file's diff (and its File view in the file history).
pub(super) fn binary_file(ui: &mut Ui) {
    empty_state(ui, EmptyIcon::Binary, BINARY_TITLE, BINARY_HINT, &[]);
}

/// A binary file's diff with its sizes known (a file that isn't an image preview).
pub(super) fn binary_sizes(ui: &mut Ui, old: Option<u64>, new: Option<u64>) {
    empty_state(ui, EmptyIcon::Binary, BINARY_TITLE, &sizes_hint(old, new), &[]);
}

/// "12.4 KB before, 14.1 KB after", "Added, 14.1 KB" or "Deleted, 12.4 KB"; the plain hint
/// without either size.
pub(super) fn sizes_hint(old: Option<u64>, new: Option<u64>) -> String {
    match (old, new) {
        (Some(a), Some(b)) => format!("{} before, {} after", size_text(a), size_text(b)),
        (None, Some(b)) => format!("Added, {}", size_text(b)),
        (Some(a), None) => format!("Deleted, {}", size_text(a)),
        (None, None) => BINARY_HINT.to_owned(),
    }
}

/// A diff whose changes Ignore whitespace hides all of; true when its button asks to show them.
pub(super) fn whitespace_only(ui: &mut Ui) -> bool {
    let show = [PaneButton { text: SHOW_WHITESPACE, primary: false }];
    empty_state(ui, EmptyIcon::Whitespace, WHITESPACE_TITLE, WHITESPACE_HINT, &show).is_some()
}

/// A diff without hunks for another reason: a mode change or an empty file.
pub(super) fn no_changes(ui: &mut Ui) {
    empty_state(ui, EmptyIcon::File, NO_CHANGES_TITLE, NO_CHANGES_HINT, &[]);
}

/// A button under the hint: the first may be the primary one, the rest are secondary.
#[derive(Clone, Copy, Debug)]
pub struct PaneButton<'a> {
    pub text: &'a str,
    pub primary: bool,
}

/// What sits above the title.
pub(super) enum Glyph {
    Icon(EmptyIcon),
    /// A severity mark in its own colour (an error pane's warning triangle).
    Severity(Severity),
    /// Something is loading.
    Spinner,
}

/// Draws the empty state in the space left in `ui` (an empty `hint` leaves the hint out);
/// returns the index of the button clicked.
pub fn empty_state(ui: &mut Ui, icon: EmptyIcon, title: &str, hint: &str, buttons: &[PaneButton]) -> Option<usize> {
    let hint = Some(hint).filter(|h| !h.is_empty());
    block(ui, Glyph::Icon(icon), RichText::new(title).strong().color(color::text()), hint, buttons)
}

fn styled<'a>(b: &PaneButton<'a>) -> StyledButton<'a> {
    if b.primary { buttons::primary(b.text) } else { buttons::secondary(b.text) }
}

/// The buttons packed into rows no wider than `width`: each row's buttons and its width.
fn rows(ui: &Ui, list: &[PaneButton], width: f32) -> Vec<(std::ops::Range<usize>, f32)> {
    let gap = ui.spacing().item_spacing.x;
    let mut out: Vec<(std::ops::Range<usize>, f32)> = Vec::new();
    for (i, b) in list.iter().enumerate() {
        let w = styled(b).desired_width(ui);
        match out.last_mut() {
            Some((r, row_w)) if *row_w + gap + w <= width => {
                r.end = i + 1;
                *row_w += gap + w;
            }
            _ => out.push((i..i + 1, w)),
        }
    }
    out
}

/// The block: `glyph`, `title`, the weak `hint` and `list`'s buttons, centred in the space left
/// and a third of the way down it. Returns the index of the button clicked.
pub(super) fn block(ui: &mut Ui, glyph: Glyph, title: RichText, hint: Option<&str>, list: &[PaneButton]) -> Option<usize> {
    let area = ui.available_rect_before_wrap();
    let width = HINT_WIDTH.min(area.width() - 32.0).max(120.0);
    let measure = |text: &str| ui.painter().layout(text.to_owned(), body_font(ui), Color32::PLACEHOLDER, width).size().y;
    let title_h = measure(title.text());
    let hint_h = hint.map_or(0.0, |h| 8.0 + measure(h));
    let rows = rows(ui, list, width);
    let gap_y = ui.spacing().item_spacing.y;
    let n = rows.len() as f32;
    let buttons_h = if rows.is_empty() { 0.0 } else { 16.0 + n * BUTTON_H + (n - 1.0) * gap_y };
    let glyph_h = if matches!(glyph, Glyph::Spinner) { SPINNER } else { ICON };
    let block_h = glyph_h + 16.0 + title_h + hint_h + buttons_h;
    let free = if area.height().is_finite() { area.height() - block_h } else { 0.0 };
    let top = area.top() + (free / 3.0).max(MIN_TOP);
    let block = egui::Rect::from_min_size(pos2(area.center().x - width / 2.0, top), vec2(width, block_h.max(0.0)));
    let mut clicked = None;
    ui.scope_builder(UiBuilder::new().max_rect(block).layout(Layout::top_down(Align::Center)), |ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(glyph_h, glyph_h), egui::Sense::hover());
        match glyph {
            Glyph::Icon(icon) => super::empty_state_icons::paint(ui.painter(), rect.center(), icon, color::weak()),
            Glyph::Severity(s) => severity::paint(ui.painter(), rect.center(), ICON, s, s.color()),
            Glyph::Spinner => egui::Spinner::new().size(SPINNER).color(color::weak()).paint_at(ui, rect),
        }
        ui.add_space(16.0);
        ui.add(egui::Label::new(title).wrap());
        if let Some(hint) = hint {
            ui.add_space(8.0);
            ui.add(egui::Label::new(RichText::new(hint).color(color::weak())).wrap());
        }
        if !rows.is_empty() {
            ui.add_space(16.0);
        }
        for (range, row_w) in rows {
            ui.allocate_ui_with_layout(vec2(row_w, BUTTON_H), Layout::left_to_right(Align::Center), |ui| {
                for i in range {
                    if ui.add(styled(&list[i])).clicked() {
                        clicked = Some(i);
                    }
                }
            });
        }
    });
    clicked
}

fn body_font(ui: &Ui) -> FontId {
    egui::TextStyle::Body.resolve(ui.style())
}
