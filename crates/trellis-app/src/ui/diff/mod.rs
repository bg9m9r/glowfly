//! The diff viewer: one column (inline) or old and new side by side (split), with hunk- and
//! line-level stage, unstage and discard, and next/previous change navigation.

mod inline;
mod split;
mod syntax;

use super::common::short;
use super::theme::color;
use super::widgets::{Icon, icon_button};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::lines::{LineClick, LineSelection};
use eframe::egui::{self, Align, Align2, CursorIcon, FontId, Layout, Rect, Response, RichText, TextStyle, Ui, pos2, vec2};
use trellis_core::highlight::DiffHighlight;
use trellis_core::repo::{DiffTarget, FileDiff, HunkOp};
use trellis_core::settings::DiffLayout;

pub(super) const ROW_H: f32 = 19.0;

pub(super) fn mono() -> FontId {
    FontId::monospace(12.5)
}

/// What both layouts draw from.
pub(super) struct DiffView<'a> {
    pub target: &'a DiffTarget,
    pub diff: &'a FileDiff,
    pub hl: Option<&'a DiffHighlight>,
    /// The diff's generation, or `None` while a reload is on its way (see `hunk_header`).
    pub hunk_gen: Option<u64>,
    /// The line selection; `None` when lines can't be selected (commit diffs, or while hunk
    /// and line operations are blocked).
    pub sel: Option<&'a LineSelection>,
    /// Why hunk and line operations are off for reasons other than a pending reload (e.g.
    /// whitespace hidden).
    pub blocked: Option<&'static str>,
}

impl DiffView<'_> {
    pub fn is_selected(&self, hunk: usize, line: usize) -> bool {
        self.sel.is_some_and(|s| s.contains(self.target, hunk, line))
    }
}

pub fn show(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let layout = st.settings.diff_layout;
    let colored = st.settings.syntax_highlighting;
    let scroll_to = st.view.diff_scroll_to_row.take();
    let hunk_gen = st.hunk_ops_gen();
    let ignore_ws = st.settings.diff_ignore_whitespace;
    let blocked = st.hunk_ops_blocked();
    let top = {
        let Some((target, diff)) = &st.diff else { return };
        let has_rows = !diff.binary && !diff.hunks.is_empty();
        ui.add_space(6.0);
        header_bar(ui, target, has_rows.then_some(layout), (!diff.binary).then_some(ignore_ws), msgs);
        ui.separator();
        if diff.binary {
            ui.label(RichText::new("Binary file not shown.").color(color::weak()));
            return;
        }
        if diff.hunks.is_empty() {
            let text = if ignore_ws {
                "Only whitespace changed in this file. Turn off Ignore whitespace to see it."
            } else {
                "No textual changes (mode or empty file)."
            };
            ui.label(RichText::new(text).color(color::weak()));
            return;
        }
        // Highlighting always comes with the diff; the setting only decides whether to paint it.
        let hl = diff.highlight.as_ref().filter(|_| colored);
        let selectable = !matches!(target, DiffTarget::Commit(..)) && blocked.is_none();
        let view = DiffView { target, diff, hl, hunk_gen, sel: selectable.then_some(&st.view.diff_lines), blocked };
        match layout {
            DiffLayout::Inline => inline::show(ui, &view, scroll_to, msgs),
            DiffLayout::Split => split::show(ui, &view, scroll_to, msgs),
        }
    };
    st.view.diff_top_row = top;
}

/// Path and kind on the left; on the right (listed from the edge inwards) Close, the file
/// action, next/previous change, the layout toggle and the whitespace toggle. `layout` is
/// `None` when there are no rows to lay out or navigate (binary or empty diffs); `ignore_ws`
/// is the whitespace setting, `None` for binary diffs. The whitespace toggle shows even with
/// no rows, so a file it hid entirely can be shown again.
fn header_bar(ui: &mut Ui, target: &DiffTarget, layout: Option<DiffLayout>, ignore_ws: Option<bool>, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        let kind = match target {
            DiffTarget::Commit(o, _) => format!("commit {}", short(*o)),
            DiffTarget::Unstaged(_) => "unstaged".into(),
            DiffTarget::Staged(_) => "staged".into(),
        };
        ui.label(RichText::new(target.path()).strong().size(15.0).color(color::text()));
        ui.label(RichText::new(kind).color(color::weak()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Close  (Esc)").clicked() {
                msgs.push(Msg::CloseDiff);
            }
            match target {
                DiffTarget::Unstaged(p) => {
                    if ui.button("Stage file").clicked() {
                        msgs.push(Msg::StageFile(p.clone()));
                    }
                }
                DiffTarget::Staged(p) => {
                    if ui.button("Unstage file").clicked() {
                        msgs.push(Msg::UnstageFile(p.clone()));
                    }
                }
                DiffTarget::Commit(..) => {}
            }
            if let Some(layout) = layout {
                ui.add_space(10.0);
                // Arrows are painted: the bundled fonts have no arrow glyphs.
                if icon_button(ui, Icon::NextChange, "Next change (F7)").clicked() {
                    msgs.push(Msg::JumpToChange(1));
                }
                if icon_button(ui, Icon::PrevChange, "Previous change (Shift+F7)").clicked() {
                    msgs.push(Msg::JumpToChange(-1));
                }
                ui.add_space(10.0);
                // A two-button segmented control; right to left, so Split is added first.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let options =
                        [(DiffLayout::Split, "Split", "Old and new side by side"), (DiffLayout::Inline, "Inline", "Changes in one column")];
                    for (value, label, tip) in options {
                        let selected = layout == value;
                        if ui.selectable_label(selected, label).on_hover_text(tip).clicked() && !selected {
                            msgs.push(Msg::SetDiffLayout(value));
                        }
                    }
                });
            }
            if let Some(on) = ignore_ws {
                ui.add_space(10.0);
                let what = "Hide changes that only add or remove trailing whitespace or change the amount of existing \
                            whitespace (such as re-indenting)";
                let tip = if on {
                    format!("{what}. Hunk and line staging is unavailable while whitespace changes are hidden.")
                } else {
                    what.to_owned()
                };
                if ui.selectable_label(on, "Ignore whitespace").on_hover_text(tip).clicked() {
                    msgs.push(Msg::SetIgnoreWhitespace(!on));
                }
            }
        });
    });
}

/// A hunk header row: the `@@` line, plus Stage / Unstage / Discard Hunk buttons pinned to the
/// visible right edge so they stay in view when scrolled sideways. A hunk with selected lines
/// shows Stage / Unstage / Discard N lines instead. The buttons are disabled while a reload is
/// on its way (`hunk_gen` is `None`), since the hunk indexes may no longer match the file, and
/// while `blocked` (whitespace hidden: the shown hunks aren't the ones staging works on).
pub(super) fn hunk_header(ui: &mut Ui, rect: Rect, v: &DiffView, h: usize, msgs: &mut Vec<Msg>) {
    let p = ui.painter();
    p.rect_filled(rect, 0.0, color::hunk_bg());
    p.text(pos2(rect.left() + 8.0, rect.center().y), Align2::LEFT_CENTER, &v.diff.hunks[h].header, mono(), color::hunk_fg());
    let path = v.target.path();
    let selected = v.sel.map_or(0, |s| s.count_in(v.target, h));
    let ops: &[HunkOp] = match v.target {
        DiffTarget::Unstaged(_) => &[HunkOp::Discard, HunkOp::Stage],
        DiffTarget::Staged(_) => &[HunkOp::Unstage],
        DiffTarget::Commit(..) => &[],
    };
    let small = TextStyle::Small.resolve(ui.style());
    let mut x = ui.clip_rect().right() - 8.0;
    for &op in ops {
        let verb = match op {
            HunkOp::Stage => "Stage",
            HunkOp::Unstage => "Unstage",
            HunkOp::Discard => "Discard",
        };
        let label = match selected {
            0 => format!("{verb} Hunk"),
            1 => format!("{verb} 1 line"),
            n => format!("{verb} {n} lines"),
        };
        let text_w = ui.fonts(|f| f.layout_no_wrap(label.clone(), small.clone(), color::text()).size().x);
        let w = (text_w + 16.0).max(96.0);
        let r = Rect::from_min_size(pos2(x - w, rect.top() + 1.0), vec2(w, ROW_H - 2.0));
        x -= w + 6.0;
        let mut builder = egui::UiBuilder::new().max_rect(r).layout(Layout::centered_and_justified(egui::Direction::TopDown));
        let reason = v.blocked.or(v.hunk_gen.is_none().then_some("Updating the diff..."));
        if reason.is_some() {
            builder = builder.disabled();
        }
        let button = ui
            .allocate_new_ui(builder, |ui| ui.add(egui::Button::new(RichText::new(label).small())))
            .inner
            .on_disabled_hover_text(reason.unwrap_or_default());
        let (true, Some(generation)) = (button.clicked(), v.hunk_gen) else { continue };
        let m = if selected > 0 { Msg::ApplyLines(op) } else { Msg::Hunk { op, path: path.to_owned(), index: h, generation } };
        msgs.push(match (op, selected) {
            (HunkOp::Discard, 0) => {
                Msg::confirm("Discard hunk", format!("Discard this change in {path}? This cannot be undone."), "Discard", m)
            }
            (HunkOp::Discard, n) => {
                let what = if n == 1 { "the selected line".to_owned() } else { format!("the {n} selected lines") };
                Msg::confirm("Discard lines", format!("Discard {what} in {path}? This cannot be undone."), "Discard", m)
            }
            _ => m,
        });
    }
}

/// Click handling for a diff line (a row in inline, a cell in split): `+`/`-` lines are
/// selected (Shift for a range, Ctrl/Cmd to toggle); a plain click elsewhere clears the
/// selection. Returns whether the line is selectable and hovered.
pub(super) fn line_click(ui: &Ui, resp: &Response, hunk: usize, line: usize, origin: char, msgs: &mut Vec<Msg>) -> bool {
    let changed = matches!(origin, '+' | '-');
    let hovered = changed && resp.hovered();
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if resp.clicked() {
        let m = ui.input(|i| i.modifiers);
        let how = if m.shift {
            LineClick::Range
        } else if m.command {
            LineClick::Toggle
        } else {
            LineClick::Only
        };
        if changed {
            msgs.push(Msg::SelectDiffLine { hunk, line, how });
        } else if how == LineClick::Only {
            msgs.push(Msg::ClearLineSelection);
        }
    }
    hovered
}

/// The bar on a line's left edge: the accent when selected, faint when hovered.
pub(super) fn paint_mark(ui: &Ui, rect: Rect, selected: bool, hovered: bool) {
    let c = match (selected, hovered) {
        (true, _) => color::accent(),
        (false, true) => color::faint(),
        _ => return,
    };
    let bar = Rect::from_min_size(rect.min, vec2(3.0, rect.height()));
    ui.painter().with_clip_rect(rect.intersect(ui.clip_rect())).rect_filled(bar, 0.0, c);
}

/// Uniform rows in a scroll area shared by both layouts (one id, so switching layout doesn't
/// leave a stale second scroll state). Scrolls `scroll_to` to the top if set, and returns the
/// first visible row.
pub(super) fn scroll(ui: &mut Ui, rows: usize, scroll_to: Option<usize>, mut add_row: impl FnMut(&mut Ui, usize)) -> usize {
    ui.spacing_mut().item_spacing.y = 0.0;
    let mut area = egui::ScrollArea::both().id_salt("diff").auto_shrink([false, false]);
    if let Some(row) = scroll_to {
        area = area.vertical_scroll_offset(row as f32 * ROW_H);
    }
    let out = area.show_rows(ui, ROW_H, rows, |ui, range| {
        for i in range {
            add_row(ui, i);
        }
    });
    (out.state.offset.y / ROW_H).round() as usize
}
