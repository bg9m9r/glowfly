//! A hunk header row: the `@@` line with its function context in syntax colors, the hunk's
//! action buttons, and in the hunk view the control that fills in the unchanged lines above it.

use super::{DiffView, fold, syntax};
use crate::msg::{Msg, StageMsg};
use crate::ui::theme::color;
use eframe::egui::{self, Layout, Rect, RichText, TextStyle, Ui, pos2, vec2};
use trellis_core::highlight;
use trellis_core::repo::{DiffTarget, HunkOp};

/// A hunk's action button.
#[derive(Clone, Copy)]
enum HunkButton {
    Op(HunkOp),
    /// Apply the inverse of the hunk, or of its selected lines, to the working tree (commit and
    /// comparison diffs).
    Revert,
}

/// A hunk header row: the `@@` line (its function context in syntax colors), plus its buttons
/// pinned to the visible right edge so they stay in view when scrolled sideways: Stage /
/// Unstage / Discard Hunk on working-tree diffs (or Stage / Unstage / Discard N lines when the
/// hunk has selected lines), Revert Hunk (or Revert N lines) on commit and comparison diffs. The buttons are
/// disabled while a reload is on its way (`hunk_gen` is `None`), since the hunk indexes may no
/// longer match the file. With whitespace changes hidden they apply only the changes shown.
pub(super) fn hunk_header(ui: &mut Ui, rect: Rect, v: &DiffView, h: usize, msgs: &mut Vec<Msg>) {
    ui.painter().rect_filled(rect, 0.0, color::hunk_bg());
    // In the hunk view, the closed gap above this hunk opens from a control before the text.
    let gap = v.whole.is_none().then(|| v.gaps.iter().find(|g| g.hunk == h)).flatten();
    let mut x = rect.left() + 8.0;
    if let Some(&g) = gap {
        fold::gap_control(ui, rect, v, g, msgs);
        x += 18.0 * v.style.k;
    }
    let raw = &v.diff.hunks[h].header;
    let ctx = v.header_hl.map(|hl| hl.header(h)).filter(|s| !s.is_empty()).and_then(|s| highlight::context_start(raw).map(|at| (at, s)));
    syntax::paint_header(ui.painter(), pos2(x, rect.center().y), raw, ctx, &v.style.font, v.style.tab);
    let path = v.target.path();
    let selected = v.sel.map_or(0, |s| s.count_in(v.target, h));
    let buttons: &[HunkButton] = match v.target {
        DiffTarget::Unstaged(_) => &[HunkButton::Op(HunkOp::Discard), HunkButton::Op(HunkOp::Stage)],
        DiffTarget::Staged(_) => &[HunkButton::Op(HunkOp::Unstage)],
        DiffTarget::Commit(..) | DiffTarget::Between(..) => &[HunkButton::Revert],
        // Comparing a commit with the working tree: nothing to stage or revert per hunk.
        DiffTarget::ToWorkdir(..) => &[],
    };
    let small = TextStyle::Small.resolve(ui.style());
    let mut x = ui.clip_rect().right() - 8.0;
    for &b in buttons {
        let verb = match b {
            HunkButton::Revert => "Revert",
            HunkButton::Op(HunkOp::Stage) => "Stage",
            HunkButton::Op(HunkOp::Unstage) => "Unstage",
            HunkButton::Op(HunkOp::Discard) => "Discard",
        };
        let label = match selected {
            0 => format!("{verb} Hunk"),
            1 => format!("{verb} 1 line"),
            n => format!("{verb} {n} lines"),
        };
        let text_w = ui.fonts(|f| f.layout_no_wrap(label.clone(), small.clone(), color::text()).size().x);
        let w = (text_w + 16.0).max(96.0);
        let r = Rect::from_min_size(pos2(x - w, rect.top() + 1.0), vec2(w, v.style.row_h - 2.0));
        x -= w + 6.0;
        let mut builder = egui::UiBuilder::new().max_rect(r).layout(Layout::centered_and_justified(egui::Direction::TopDown));
        let reason = v.hunk_gen.is_none().then_some("Updating the diff...");
        if reason.is_some() {
            builder = builder.disabled();
        }
        let mut button = ui
            .allocate_new_ui(builder, |ui| ui.add(egui::Button::new(RichText::new(label).small())))
            .inner
            .on_disabled_hover_text(reason.unwrap_or_default());
        if let HunkButton::Revert = b {
            let what = if selected > 0 { "the selected changes" } else { "this change" };
            button = button.on_hover_text(format!("Undo {what} in your working copy of {path}. The commit itself is not changed."));
        }
        let (true, Some(generation)) = (button.clicked(), v.hunk_gen) else { continue };
        let op = match b {
            HunkButton::Revert => {
                msgs.push(Msg::Stage(if selected > 0 { StageMsg::RevertLines } else { StageMsg::RevertHunk { index: h, generation } }));
                continue;
            }
            HunkButton::Op(op) => op,
        };
        let m = if selected > 0 {
            Msg::Stage(StageMsg::ApplyLines(op))
        } else {
            Msg::Stage(StageMsg::Hunk { op, path: path.to_owned(), index: h, generation })
        };
        msgs.push(match (op, selected) {
            (HunkOp::Discard, 0) => Msg::confirm_danger(
                "Discard hunk",
                format!("Discard this change in {path}? You can bring it back with Undo."),
                "Discard",
                m,
            ),
            (HunkOp::Discard, n) => {
                let what = if n == 1 { "the selected line".to_owned() } else { format!("the {n} selected lines") };
                Msg::confirm_danger("Discard lines", format!("Discard {what} in {path}? You can bring them back with Undo."), "Discard", m)
            }
            _ => m,
        });
    }
}
