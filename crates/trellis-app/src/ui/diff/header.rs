//! The diff header bar: the file and its kind, then the file actions (Stage or Unstage file;
//! Restore, Open and Show for a commit's file), next/previous change, the view mode and layout
//! toggles, Expand all / Collapse all for whole-file folds and the whitespace toggle.

use crate::keymap::{self, BindingId};
use crate::msg::Msg;
use crate::state::diff_nav::StepKind;
use crate::ui::common::short;
use crate::ui::file_buttons::{self, CommitFile, Labels};
use crate::ui::file_menu::SHOW_IN_FOLDER;
use crate::ui::theme::color;
use crate::ui::widgets::{Icon, icon_button};
use eframe::egui::{self, Align, Layout, Response, RichText, Ui};
use trellis_core::repo::DiffTarget;
use trellis_core::settings::{DiffLayout, DiffMode};

const HUNKS_TIP: &str = "Only the changed lines, with a few lines of context around each change";
const WHOLE_FILE_TIP: &str = "Every line of the file, with changes marked in place. Single-line staging is available in the Hunks view.";
const EXPAND_ALL_TIP: &str = "Show every unchanged line of the file";
const EXPAND_ALL_OFF_TIP: &str = "Every unchanged run is already shown";
const COLLAPSE_ALL_TIP: &str = "Fold every long unchanged run back into one row";
const COLLAPSE_ALL_OFF_TIP: &str = "No opened runs to fold";

/// How many folds of the whole file on screen are open and closed.
#[derive(Clone, Copy)]
pub(super) struct FoldCounts {
    pub open: usize,
    pub closed: usize,
}

/// Path and kind on the left; on the right (listed from the edge inwards) Close, the file
/// actions (Stage file or Unstage file for working-tree diffs; Show, Open and Restore for a
/// commit's diff, from `file`, which is `None` when the commit detail doesn't list it),
/// next/previous change, the layout toggle, the view mode toggle, Collapse all and
/// Expand all, and the whitespace toggle. `rows` holds the layout and view mode, `None` when
/// there are no rows to lay out (binary or empty diffs); `nav` is where Previous and Next go.
/// The arrows show when there are rows or a neighbouring file to step to, so a binary or empty
/// file can be stepped past with the mouse; an arrow with nowhere to go is dimmed. `folds`
/// counts the whole file's folds, `None` when it isn't on screen or has none; Expand all and
/// Collapse all show only with it in the whole-file view, each disabled when there is nothing
/// for it to do. `ignore_ws` is the whitespace setting, `None` for binary diffs. The whitespace
/// toggle shows even with no rows, so a file it hid entirely can be shown again. When the bar
/// is too narrow for the fold buttons, or the commit file buttons crowd it, they and the
/// whitespace toggle move to a second row, right-aligned under the others, rather than run over
/// the path. When even without them the file buttons would reach the path, the file buttons
/// lead that second row too.
#[allow(clippy::too_many_arguments)]
pub(super) fn header_bar(
    ui: &mut Ui,
    target: &DiffTarget,
    file: Option<&CommitFile>,
    rows: Option<(DiffLayout, DiffMode)>,
    nav: [StepKind; 2],
    ignore_ws: Option<bool>,
    folds: Option<FoldCounts>,
    msgs: &mut Vec<Msg>,
) {
    let folds = folds.filter(|_| matches!(rows, Some((_, DiffMode::WholeFile))));
    let show_nav = rows.is_some() || nav.iter().any(|k| *k != StepKind::End);
    let mut wrapped = false;
    let mut files_wrapped = false;
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        let kind = match target {
            DiffTarget::Commit(o, _) => format!("commit {}", short(*o)),
            DiffTarget::Unstaged(_) => "unstaged".into(),
            DiffTarget::Staged(_) => "staged".into(),
            DiffTarget::Between(a, b, _) => format!("{}..{}", short(*a), short(*b)),
        };
        ui.label(RichText::new(target.path()).strong().size(15.0).color(color::text()));
        ui.label(RichText::new(kind).color(color::weak()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // The commit file buttons move down too when even without the tail they would reach
            // the path.
            files_wrapped = file.is_some()
                && button_w(ui, CLOSE) + file_buttons_width(ui) + middle_width(ui, rows.is_some(), show_nav) > ui.available_width();
            if ui.button(CLOSE).clicked() {
                msgs.push(Msg::CloseDiff);
            }
            match target {
                DiffTarget::Unstaged(p) => {
                    let k = keymap::hint(BindingId::Stage);
                    let tip = format!("Stage this file ({k}). With lines selected, {k} stages only those lines.");
                    if ui.button("Stage file").on_hover_text(tip).clicked() {
                        msgs.push(Msg::StageFile(p.clone()));
                    }
                }
                DiffTarget::Staged(p) => {
                    let k = keymap::hint(BindingId::Unstage);
                    let tip = format!("Unstage this file ({k}). With lines selected, {k} unstages only those lines.");
                    if ui.button("Unstage file").on_hover_text(tip).clicked() {
                        msgs.push(Msg::UnstageFile(p.clone()));
                    }
                }
                DiffTarget::Commit(..) => {
                    if let Some(f) = file.filter(|_| !files_wrapped) {
                        file_buttons::show(ui, msgs, f, Labels::Compact);
                    }
                }
                DiffTarget::Between(..) => {}
            }
            if show_nav {
                ui.add_space(10.0);
                let [prev, next] = nav;
                // Arrows are painted: the bundled fonts have no arrow glyphs.
                if step_button(ui, Icon::NextChange, next, NEXT_TIPS).clicked() {
                    msgs.push(Msg::JumpToChange(1));
                }
                if step_button(ui, Icon::PrevChange, prev, PREV_TIPS).clicked() {
                    msgs.push(Msg::JumpToChange(-1));
                }
            }
            if let Some((layout, mode)) = rows {
                ui.add_space(10.0);
                let layouts =
                    [(DiffLayout::Split, "Split", "Old and new side by side"), (DiffLayout::Inline, "Inline", "Changes in one column")];
                segmented(ui, layout, &layouts, |l| msgs.push(Msg::SetDiffLayout(l)));
                ui.add_space(10.0);
                let modes = [(DiffMode::WholeFile, "Whole file", WHOLE_FILE_TIP), (DiffMode::Hunks, "Hunks", HUNKS_TIP)];
                segmented(ui, mode, &modes, |m| msgs.push(Msg::SetDiffMode(m)));
            }
            // Once the file buttons have moved down, the tail joins them.
            wrapped = files_wrapped
                || ((folds.is_some() || file.is_some()) && tail_width(ui, folds.is_some(), ignore_ws.is_some()) > ui.available_width());
            if !wrapped {
                tail(ui, folds, ignore_ws, true, msgs);
            }
        });
    });
    if wrapped || files_wrapped {
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(f) = file.filter(|_| files_wrapped) {
                    file_buttons::show(ui, msgs, f, Labels::Compact);
                }
                if wrapped {
                    tail(ui, folds, ignore_ws, files_wrapped, msgs);
                }
            })
        });
    }
}

const CLOSE: &str = "Close  (Esc)";
const COLLAPSE_ALL: &str = "Collapse all";
const EXPAND_ALL: &str = "Expand all";
const IGNORE_WS: &str = "Ignore whitespace";

/// The width a text button (or selectable label) takes in the bar, with the gap after it.
fn button_w(ui: &Ui, text: &str) -> f32 {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let s = ui.spacing();
    let text_w = ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font, color::text()).size().x);
    text_w + 2.0 * s.button_padding.x + s.item_spacing.x
}

/// The width of the compact Restore, Open and Show buttons.
fn file_buttons_width(ui: &Ui) -> f32 {
    [file_buttons::RESTORE_SHORT, file_buttons::OPEN_SHORT, SHOW_IN_FOLDER].iter().map(|t| button_w(ui, t)).sum()
}

/// At least the width of the arrows (with `nav`) and the layout and view mode toggles (with
/// `rows`), which sit between the file action and the tail.
fn middle_width(ui: &Ui, rows: bool, nav: bool) -> f32 {
    let arrows = if nav { 10.0 + 2.0 * (26.0 + ui.spacing().item_spacing.x) } else { 0.0 };
    let toggles = if rows { 20.0 + ["Split", "Inline", "Whole file", "Hunks"].iter().map(|t| button_w(ui, t)).sum::<f32>() } else { 0.0 };
    arrows + toggles
}

/// The width `tail` takes with the fold buttons when `folds`, and with the whitespace toggle
/// when `ws`.
fn tail_width(ui: &Ui, folds: bool, ws: bool) -> f32 {
    let folds = if folds { 10.0 + button_w(ui, COLLAPSE_ALL) + button_w(ui, EXPAND_ALL) } else { 0.0 };
    let ws = if ws { 10.0 + button_w(ui, IGNORE_WS) } else { 0.0 };
    folds + ws
}

/// The left end of the bar's right-hand group, laid out right to left: Collapse all and Expand
/// all (with `folds`), then the whitespace toggle (with `ignore_ws`). `gap` puts space before
/// the first of them, which a second row starting at the edge leaves out.
fn tail(ui: &mut Ui, folds: Option<FoldCounts>, ignore_ws: Option<bool>, gap: bool, msgs: &mut Vec<Msg>) {
    if let Some(c) = folds {
        if gap {
            ui.add_space(10.0);
        }
        let collapse = ui.add_enabled(c.open > 0, egui::Button::new(COLLAPSE_ALL));
        if collapse.on_hover_text(COLLAPSE_ALL_TIP).on_disabled_hover_text(COLLAPSE_ALL_OFF_TIP).clicked() {
            msgs.push(Msg::CollapseAllUnchanged);
        }
        let expand = ui.add_enabled(c.closed > 0, egui::Button::new(EXPAND_ALL));
        if expand.on_hover_text(EXPAND_ALL_TIP).on_disabled_hover_text(EXPAND_ALL_OFF_TIP).clicked() {
            msgs.push(Msg::ExpandAllUnchanged);
        }
    }
    if let Some(on) = ignore_ws {
        if gap || folds.is_some() {
            ui.add_space(10.0);
        }
        let what = "Hide changes that only add or remove trailing whitespace or change the amount of existing \
                    whitespace (such as re-indenting)";
        let tip =
            if on { format!("{what}. Hunk and line staging is unavailable while whitespace changes are hidden.") } else { what.to_owned() };
        if ui.selectable_label(on, IGNORE_WS).on_hover_text(tip).clicked() {
            msgs.push(Msg::SetIgnoreWhitespace(!on));
        }
    }
}

/// Tooltips for a step that stays in the file, opens another file, or goes nowhere.
const NEXT_TIPS: [&str; 3] = ["Next change (F7)", "Next change, in the next file (F7)", "No more changes after this file (F7)"];
const PREV_TIPS: [&str; 3] =
    ["Previous change (Shift+F7)", "Previous change, in the previous file (Shift+F7)", "No changes before this file (Shift+F7)"];

/// A next or previous change arrow, dimmed and inert when the step goes nowhere.
fn step_button(ui: &mut Ui, icon: Icon, kind: StepKind, tips: [&str; 3]) -> Response {
    let tip = match kind {
        StepKind::Change => tips[0],
        StepKind::File => tips[1],
        StepKind::End => tips[2],
    };
    ui.add_enabled_ui(kind != StepKind::End, |ui| icon_button(ui, icon, tip)).inner
}

/// A segmented control of selectable labels with no gaps. The bar lays out right to left, so
/// `options` are listed from the right. `on_pick` gets an option other than `current` when
/// it is clicked.
fn segmented<T: Copy + PartialEq>(ui: &mut Ui, current: T, options: &[(T, &str, &str)], mut on_pick: impl FnMut(T)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for &(value, label, tip) in options {
            let selected = current == value;
            if ui.selectable_label(selected, label).on_hover_text(tip).clicked() && !selected {
                on_pick(value);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::vec2;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    /// The top of Expand all and of Close in a header bar `width` wide.
    fn tops(width: f32) -> (f64, f64) {
        let target = DiffTarget::Unstaged("notes.txt".into());
        let folds = Some(FoldCounts { open: 1, closed: 1 });
        let rows = Some((DiffLayout::Inline, DiffMode::WholeFile));
        let mut h = Harness::builder().with_size(vec2(width, 80.0)).build_ui(|ui| {
            header_bar(ui, &target, None, rows, [StepKind::Change; 2], Some(false), folds, &mut Vec::new());
        });
        h.run();
        let top = |label: &str| h.get_by_label(label).bounding_box().expect("laid out").y0;
        (top("Expand all"), top("Close  (Esc)"))
    }

    #[test]
    fn fold_buttons_wrap_only_when_short_of_room() {
        let (expand, close) = tops(1400.0);
        assert!((expand - close).abs() < 4.0, "one row when wide: {expand} vs {close}");
        let (expand, close) = tops(740.0);
        assert!(expand > close + 10.0, "a second row when narrow: {expand} vs {close}");
    }

    /// The top of Ignore whitespace, Open, Close and the path in a commit's header bar `width`
    /// wide, and the right edge of the path's kind and the left edge of the leftmost toggle.
    fn commit_layout(width: f32) -> ([f64; 4], f64, f64) {
        let oid = git2::Oid::from_bytes(&[7; 20]).unwrap();
        let target = DiffTarget::Commit(oid, "notes.txt".into());
        let file = CommitFile { oid, path: "notes.txt", status: 'M', open_path: "notes.txt", in_workdir: true };
        let rows = Some((DiffLayout::Inline, DiffMode::Hunks));
        let mut h = Harness::builder().with_size(vec2(width, 80.0)).build_ui(|ui| {
            header_bar(ui, &target, Some(&file), rows, [StepKind::Change; 2], Some(false), None, &mut Vec::new());
        });
        h.run();
        let rect = |label: &str| h.get_by_label(label).bounding_box().expect("laid out");
        let tops = [rect(IGNORE_WS).y0, rect(file_buttons::OPEN_SHORT).y0, rect(CLOSE).y0, rect("notes.txt").y0];
        (tops, rect("commit 0707070").x1, rect("Whole file").x0)
    }

    #[test]
    fn tail_wraps_for_commit_file_buttons() {
        let ([ws, open, close, _], _, _) = commit_layout(1400.0);
        assert!((ws - close).abs() < 4.0 && (open - close).abs() < 4.0, "one row when wide: {ws}, {open} vs {close}");
        let ([ws, open, close, _], _, _) = commit_layout(900.0);
        assert!(ws > close + 10.0, "the tail moves to a second row: {ws} vs {close}");
        assert!((open - close).abs() < 4.0, "the file buttons stay on the first row: {open} vs {close}");
        // Narrower still, the file buttons move down too rather than run over the path.
        for width in [760.0, 700.0] {
            let ([ws, open, close, _], kind_right, toggles_left) = commit_layout(width);
            assert!(open > close + 10.0 && (ws - open).abs() < 4.0, "{width}: {ws}, {open} vs {close}");
            assert!(toggles_left >= kind_right, "{width}: the path is not overrun: {toggles_left} vs {kind_right}");
        }
    }
}
