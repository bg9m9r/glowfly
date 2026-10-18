//! The diff header bar: the file and its kind, then Open in external diff tool and the file
//! actions (Stage or Unstage file;
//! Restore, Open and Show for a commit's file), next/previous change, the view mode and layout
//! toggles, Expand all / Collapse all for whole-file folds, and the whitespace and wrap
//! toggles.

use crate::keymap::{BindingId, Keymap};
use crate::msg::DiffMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::state::diff_nav::StepKind;
use crate::state::tool_compare::OPEN_IN_DIFF_TOOL;
use crate::tools::ToolsMsg;
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

/// The on/off toggles at the left end of the bar's right-hand group: the whitespace setting
/// (`None` for binary diffs) and the wrap setting (`None` when there are no rows to lay out).
#[derive(Clone, Copy)]
pub(super) struct Toggles {
    pub ignore_ws: Option<bool>,
    pub wrap: Option<bool>,
}

/// How many folds of the whole file on screen are open and closed.
#[derive(Clone, Copy)]
pub(super) struct FoldCounts {
    pub open: usize,
    pub closed: usize,
}

/// Path and kind on the left; on the right (listed from the edge inwards) Close, Open in
/// external diff tool (disabled with the reason `diff_tool` when set), the file actions (Stage file or Unstage file for working-tree diffs; Show, Open and Restore for a
/// commit's diff, from `file`, which is `None` when the commit detail doesn't list it),
/// next/previous change, the layout toggle, the view mode toggle, Collapse all and
/// Expand all, and the whitespace and wrap toggles. `rows` holds the layout and view mode, `None` when
/// there are no rows to lay out (binary or empty diffs); `nav` is where Previous and Next go.
/// The arrows show when there are rows or a neighbouring file to step to, so a binary or empty
/// file can be stepped past with the mouse; an arrow with nowhere to go is dimmed. `folds`
/// counts the whole file's folds, `None` when it isn't on screen or has none; Expand all and
/// Collapse all show only with it in the whole-file view, each disabled when there is nothing
/// for it to do. `toggles` holds the whitespace and wrap settings: the whitespace toggle shows
/// for any text diff, even with no rows, so a file it hid entirely can be shown again; the wrap
/// toggle only with rows. When the bar is too narrow for the fold buttons and the toggles, or
/// the commit file buttons crowd it, they move to a second row, right-aligned under the others,
/// rather than run over the path. When even without them the file buttons would reach the
/// path, the file buttons lead that second row too. When even the arrows and toggles would reach
/// the path (a narrow panel), they move down as well, and the tail to a third row if it doesn't
/// fit beside them.
#[allow(clippy::too_many_arguments)]
pub(super) fn header_bar(
    ui: &mut Ui,
    target: &DiffTarget,
    file: Option<&CommitFile>,
    rows: Option<(DiffLayout, DiffMode)>,
    nav: [StepKind; 2],
    toggles: Toggles,
    folds: Option<FoldCounts>,
    keys: &Keymap,
    diff_tool: Option<&str>,
    msgs: &mut Vec<Msg>,
) {
    let folds = folds.filter(|_| matches!(rows, Some((_, DiffMode::WholeFile))));
    let show_nav = rows.is_some() || nav.iter().any(|k| *k != StepKind::End);
    let kind = match target {
        DiffTarget::Commit(o, _) => format!("commit {}", short(*o)),
        DiffTarget::Unstaged(_) => "unstaged".into(),
        DiffTarget::Staged(_) => "staged".into(),
        DiffTarget::Between(a, b, _) => format!("{}..{}", short(*a), short(*b)),
        DiffTarget::ToWorkdir(o, _) => format!("{}..working tree", short(*o)),
    };
    // In a narrow bar (a panel beside the diff) the arrows and toggles move down too, rather
    // than run over the path.
    let stage_w = match target {
        DiffTarget::Unstaged(_) => button_w(ui, STAGE_FILE),
        DiffTarget::Staged(_) => button_w(ui, UNSTAGE_FILE),
        _ => 0.0,
    };
    let middle_w = middle_width(ui, rows.is_some(), show_nav);
    let middle_down = middle_w > 0.0
        && lead_width(ui, target.path(), &kind) + button_w(ui, CLOSE) + icon_w(ui) + stage_w + middle_w > ui.available_width();
    let mut wrapped = false;
    let mut files_wrapped = false;
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(target.path()).strong().size(15.0).color(color::text()));
        ui.label(RichText::new(kind).color(color::weak()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // The commit file buttons move down too when even without the tail they would reach
            // the path.
            files_wrapped = file.is_some()
                && (middle_down || button_w(ui, CLOSE) + icon_w(ui) + file_buttons_width(ui) + middle_w > ui.available_width());
            if ui.button(CLOSE).clicked() {
                msgs.push(Msg::Repo(RepoMsg::CloseDiff));
            }
            let tool =
                ui.add_enabled_ui(diff_tool.is_none(), |ui| icon_button(ui, Icon::ExternalDiff, diff_tool.unwrap_or(OPEN_IN_DIFF_TOOL)));
            if tool.inner.clicked() {
                msgs.push(Msg::Tools(ToolsMsg::OpenDiff(target.clone())));
                tool.inner.surrender_focus();
            }
            match target {
                DiffTarget::Unstaged(p) => {
                    let tip = match keys.hint(BindingId::Stage) {
                        Some(k) => format!("Stage this file ({k}). With lines selected, {k} stages only those lines."),
                        None => "Stage this file".to_owned(),
                    };
                    if ui.button(STAGE_FILE).on_hover_text(tip).clicked() {
                        msgs.push(Msg::Stage(StageMsg::StageFile(p.clone())));
                    }
                }
                DiffTarget::Staged(p) => {
                    let tip = match keys.hint(BindingId::Unstage) {
                        Some(k) => format!("Unstage this file ({k}). With lines selected, {k} unstages only those lines."),
                        None => "Unstage this file".to_owned(),
                    };
                    if ui.button(UNSTAGE_FILE).on_hover_text(tip).clicked() {
                        msgs.push(Msg::Stage(StageMsg::UnstageFile(p.clone())));
                    }
                }
                DiffTarget::Commit(..) => {
                    if let Some(f) = file.filter(|_| !files_wrapped) {
                        file_buttons::show(ui, msgs, f, Labels::Compact);
                    }
                }
                DiffTarget::Between(..) | DiffTarget::ToWorkdir(..) => {}
            }
            if middle_down {
                return;
            }
            middle(ui, rows, nav, show_nav, keys, msgs);
            // Once the file buttons have moved down, the tail joins them.
            wrapped = files_wrapped || tail_width(ui, folds.is_some(), toggles) > ui.available_width();
            if !wrapped {
                tail(ui, folds, toggles, true, msgs);
            }
        });
    });
    if middle_down {
        // The file buttons, arrows and toggles on a second row; the tail joins them if it fits.
        let mut tail_down = false;
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(f) = file {
                    file_buttons::show(ui, msgs, f, Labels::Compact);
                }
                middle(ui, rows, nav, show_nav, keys, msgs);
                tail_down = tail_width(ui, folds.is_some(), toggles) > ui.available_width();
                if !tail_down {
                    tail(ui, folds, toggles, true, msgs);
                }
            })
        });
        if tail_down {
            ui.horizontal(|ui| ui.with_layout(Layout::right_to_left(Align::Center), |ui| tail(ui, folds, toggles, false, msgs)));
        }
    } else if wrapped || files_wrapped {
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(f) = file.filter(|_| files_wrapped) {
                    file_buttons::show(ui, msgs, f, Labels::Compact);
                }
                if wrapped {
                    tail(ui, folds, toggles, files_wrapped, msgs);
                }
            })
        });
    }
}

/// Next/previous change (with `show_nav`) and the layout and view mode toggles (with `rows`),
/// laid out right to left.
fn middle(ui: &mut Ui, rows: Option<(DiffLayout, DiffMode)>, nav: [StepKind; 2], show_nav: bool, keys: &Keymap, msgs: &mut Vec<Msg>) {
    if show_nav {
        ui.add_space(10.0);
        let [prev, next] = nav;
        // Arrows are painted: the bundled fonts have no arrow glyphs.
        if step_button(ui, Icon::NextChange, next, &step_tip(keys, true, next)).clicked() {
            msgs.push(Msg::Diff(DiffMsg::JumpToChange(1)));
        }
        if step_button(ui, Icon::PrevChange, prev, &step_tip(keys, false, prev)).clicked() {
            msgs.push(Msg::Diff(DiffMsg::JumpToChange(-1)));
        }
    }
    if let Some((layout, mode)) = rows {
        ui.add_space(10.0);
        let layouts = [(DiffLayout::Split, "Split", "Old and new side by side"), (DiffLayout::Inline, "Inline", "Changes in one column")];
        segmented(ui, layout, &layouts, |l| msgs.push(Msg::Diff(DiffMsg::SetDiffLayout(l))));
        ui.add_space(10.0);
        let modes = [(DiffMode::WholeFile, "Whole file", WHOLE_FILE_TIP), (DiffMode::Hunks, "Hunks", HUNKS_TIP)];
        segmented(ui, mode, &modes, |m| msgs.push(Msg::Diff(DiffMsg::SetDiffMode(m))));
    }
}

/// The width of the path and its kind at the left of the bar.
fn lead_width(ui: &Ui, path: &str, kind: &str) -> f32 {
    let body = egui::TextStyle::Body.resolve(ui.style());
    let w = |t: &str, font: egui::FontId| ui.fonts(|f| f.layout_no_wrap(t.to_owned(), font, color::text()).size().x);
    8.0 + w(path, egui::FontId::proportional(15.0)) + w(kind, body) + 2.0 * ui.spacing().item_spacing.x
}

const CLOSE: &str = "Close  (Esc)";
const STAGE_FILE: &str = "Stage file";
const UNSTAGE_FILE: &str = "Unstage file";
const COLLAPSE_ALL: &str = "Collapse all";
const EXPAND_ALL: &str = "Expand all";
const IGNORE_WS: &str = "Ignore whitespace";
const WRAP: &str = "Wrap lines";
const WRAP_OFF_TIP: &str = "Continue long lines on the next line so they fit the width of the viewer";
const WRAP_ON_TIP: &str = "Long lines continue on the next line. Turn off to scroll sideways instead.";

/// The width a text button (or selectable label) takes in the bar, with the gap after it.
fn button_w(ui: &Ui, text: &str) -> f32 {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let s = ui.spacing();
    let text_w = ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font, color::text()).size().x);
    text_w + 2.0 * s.button_padding.x + s.item_spacing.x
}

/// The width an icon button takes in the bar, with the gap after it.
fn icon_w(ui: &Ui) -> f32 {
    26.0 + ui.spacing().item_spacing.x
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

/// The width `tail` takes with the fold buttons when `folds`, and with the toggles that
/// `toggles` shows.
fn tail_width(ui: &Ui, folds: bool, toggles: Toggles) -> f32 {
    let folds = if folds { 10.0 + button_w(ui, COLLAPSE_ALL) + button_w(ui, EXPAND_ALL) } else { 0.0 };
    let ws = if toggles.ignore_ws.is_some() { 10.0 + button_w(ui, IGNORE_WS) } else { 0.0 };
    let wrap = if toggles.wrap.is_some() { 10.0 + button_w(ui, WRAP) } else { 0.0 };
    folds + ws + wrap
}

/// The left end of the bar's right-hand group, laid out right to left: Collapse all and Expand
/// all (with `folds`), then the whitespace toggle and the wrap toggle (as `toggles` has them).
/// `gap` puts space before the first of them, which a second row starting at the edge leaves
/// out.
fn tail(ui: &mut Ui, folds: Option<FoldCounts>, toggles: Toggles, gap: bool, msgs: &mut Vec<Msg>) {
    if let Some(c) = folds {
        if gap {
            ui.add_space(10.0);
        }
        let collapse = ui.add_enabled(c.open > 0, egui::Button::new(COLLAPSE_ALL));
        if collapse.on_hover_text(COLLAPSE_ALL_TIP).on_disabled_hover_text(COLLAPSE_ALL_OFF_TIP).clicked() {
            msgs.push(Msg::Diff(DiffMsg::CollapseAllUnchanged));
        }
        let expand = ui.add_enabled(c.closed > 0, egui::Button::new(EXPAND_ALL));
        if expand.on_hover_text(EXPAND_ALL_TIP).on_disabled_hover_text(EXPAND_ALL_OFF_TIP).clicked() {
            msgs.push(Msg::Diff(DiffMsg::ExpandAllUnchanged));
        }
    }
    if let Some(on) = toggles.ignore_ws {
        if gap || folds.is_some() {
            ui.add_space(10.0);
        }
        let what = "Hide changes that only add or remove trailing whitespace or change the amount of existing \
                    whitespace (such as re-indenting)";
        let tip = if on {
            format!("{what}. Hunk and line actions then apply only the changes shown and leave the hidden whitespace changes as they are.")
        } else {
            what.to_owned()
        };
        if ui.selectable_label(on, IGNORE_WS).on_hover_text(tip).clicked() {
            msgs.push(Msg::Diff(DiffMsg::SetIgnoreWhitespace(!on)));
        }
    }
    if let Some(on) = toggles.wrap {
        if gap || folds.is_some() || toggles.ignore_ws.is_some() {
            ui.add_space(10.0);
        }
        let tip = if on { WRAP_ON_TIP } else { WRAP_OFF_TIP };
        if ui.selectable_label(on, WRAP).on_hover_text(tip).clicked() {
            msgs.push(Msg::Diff(DiffMsg::SetWordWrap(!on)));
        }
    }
}

/// The tooltip of the next (`forward`) or previous change arrow for a step that stays in the
/// file, opens another file, or goes nowhere, with the step's current key.
pub(super) fn step_tip(keys: &Keymap, forward: bool, kind: StepKind) -> String {
    let (id, text) = match (forward, kind) {
        (true, StepKind::Change) => (BindingId::NextChange, "Next change"),
        (true, StepKind::File) => (BindingId::NextChange, "Next change, in the next file"),
        (true, StepKind::End) => (BindingId::NextChange, "No more changes after this file"),
        (false, StepKind::Change) => (BindingId::PrevChange, "Previous change"),
        (false, StepKind::File) => (BindingId::PrevChange, "Previous change, in the previous file"),
        (false, StepKind::End) => (BindingId::PrevChange, "No changes before this file"),
    };
    keys.tip(text, id)
}

/// A next or previous change arrow, dimmed and inert when the step goes nowhere.
fn step_button(ui: &mut Ui, icon: Icon, kind: StepKind, tip: &str) -> Response {
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
#[path = "header_tests.rs"]
mod tests;
