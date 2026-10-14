//! The diff header bar: the file and its kind, then the file action, next/previous change, the
//! view mode and layout toggles and the whitespace toggle.

use crate::msg::Msg;
use crate::ui::common::short;
use crate::ui::theme::color;
use crate::ui::widgets::{Icon, icon_button};
use eframe::egui::{Align, Layout, RichText, Ui};
use trellis_core::repo::DiffTarget;
use trellis_core::settings::{DiffLayout, DiffMode};

const HUNKS_TIP: &str = "Only the changed lines, with a few lines of context around each change";
const WHOLE_FILE_TIP: &str = "Every line of the file, with changes marked in place. Single-line staging is available in the Hunks view.";

/// Path and kind on the left; on the right (listed from the edge inwards) Close, the file
/// action, next/previous change, the layout toggle, the view mode toggle and the whitespace
/// toggle. `rows` holds the layout and view mode, `None` when there are no rows to lay out or
/// navigate (binary or empty diffs); `ignore_ws` is the whitespace setting, `None` for binary
/// diffs. The whitespace toggle shows even with no rows, so a file it hid entirely can be shown
/// again.
pub(super) fn header_bar(
    ui: &mut Ui,
    target: &DiffTarget,
    rows: Option<(DiffLayout, DiffMode)>,
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
            if let Some((layout, mode)) = rows {
                ui.add_space(10.0);
                // Arrows are painted: the bundled fonts have no arrow glyphs.
                if icon_button(ui, Icon::NextChange, "Next change (F7)").clicked() {
                    msgs.push(Msg::JumpToChange(1));
                }
                if icon_button(ui, Icon::PrevChange, "Previous change (Shift+F7)").clicked() {
                    msgs.push(Msg::JumpToChange(-1));
                }
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
