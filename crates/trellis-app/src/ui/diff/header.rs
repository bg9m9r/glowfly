//! The diff header bar: the file and its kind, then the file action, next/previous change, the
//! view mode and layout toggles and the whitespace toggle.

use crate::msg::Msg;
use crate::state::diff_nav::StepKind;
use crate::ui::common::short;
use crate::ui::theme::color;
use crate::ui::widgets::{Icon, icon_button};
use eframe::egui::{Align, Layout, Response, RichText, Ui};
use trellis_core::repo::DiffTarget;
use trellis_core::settings::{DiffLayout, DiffMode};

const HUNKS_TIP: &str = "Only the changed lines, with a few lines of context around each change";
const WHOLE_FILE_TIP: &str = "Every line of the file, with changes marked in place. Single-line staging is available in the Hunks view.";

/// Path and kind on the left; on the right (listed from the edge inwards) Close, the file
/// action, next/previous change, the layout toggle, the view mode toggle and the whitespace
/// toggle. `rows` holds the layout and view mode, `None` when there are no rows to lay out
/// (binary or empty diffs); `nav` is where Previous and Next go. The arrows show when there
/// are rows or a neighbouring file to step to, so a binary or empty file can be stepped past
/// with the mouse; an arrow with nowhere to go is dimmed. `ignore_ws` is the whitespace
/// setting, `None` for binary diffs. The whitespace toggle shows even with no rows, so a file
/// it hid entirely can be shown again.
pub(super) fn header_bar(
    ui: &mut Ui,
    target: &DiffTarget,
    rows: Option<(DiffLayout, DiffMode)>,
    nav: [StepKind; 2],
    ignore_ws: Option<bool>,
    msgs: &mut Vec<Msg>,
) {
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
                DiffTarget::Commit(..) | DiffTarget::Between(..) => {}
            }
            if rows.is_some() || nav.iter().any(|k| *k != StepKind::End) {
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
