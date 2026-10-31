//! A hunk header row: the `@@` line with its function context in syntax colors, the hunk's
//! action buttons, and in the hunk view the control that fills in the unchanged lines above it.

use super::{DiffView, fold, syntax};
use crate::msg::{Msg, StageMsg};
use crate::ui::theme::{color, type_scale};
use crate::ui::widgets::buttons;
use eframe::egui::{Rect, Stroke, Ui, pos2, vec2};
use glowfly_core::highlight;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{DiffTarget, HunkOp};
use glowfly_model::pull_request::PullRequestMsg;
use glowfly_model::pull_request::line_comment::LineCommentMsg;

/// The space between buttons, the wider one that sets Discard apart (with a divider in it), and
/// a button's least width.
const GAP: f32 = 6.0;
const APART: f32 = 20.0;
const MIN_W: f32 = 96.0;

/// A hunk's action button.
#[derive(Clone, Copy)]
enum HunkButton {
    Op(HunkOp),
    /// Apply the inverse of the hunk, or of its selected lines, to the working tree (commit and
    /// comparison diffs, and a commit compared with the working tree).
    Revert,
    /// Start a code comment on the selected lines (a pull request's file).
    Comment,
}

/// The buttons' text size in a `row_h` tall row: body size, smaller only where the smallest diff
/// fonts make the row too short for it, and never under the type scale's floor.
fn text_px(row_h: f32) -> f32 {
    type_scale::fit(type_scale::BODY, row_h - 4.0)
}

/// A hunk header row: the `@@` line (its function context in syntax colors), plus its buttons
/// in body-size text (`text_px`), pinned to the visible right edge so they stay in view when
/// scrolled sideways: on working-tree diffs Stage hunk at the edge and Discard hunk (in the
/// danger style) to its left behind a wider gap with a divider, Unstage hunk on staged diffs
/// (each "N lines" when the hunk has selected lines), Revert hunk (or Revert N lines) on commit and
/// comparison diffs and on a commit compared with the working tree (none the other way round),
/// and left of it Comment on N lines on a pull request's file with selected lines.
/// The buttons are disabled while a reload is on its way (`hunk_gen` is
/// `None`), since the hunk indexes may no longer match the file. With whitespace changes hidden
/// they apply only the changes shown.
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
    // Laid out right to left: Stage at the edge, then Discard set apart.
    let mut list: Vec<HunkButton> = match v.target {
        DiffTarget::Unstaged(_) => vec![HunkButton::Op(HunkOp::Stage), HunkButton::Op(HunkOp::Discard)],
        DiffTarget::Staged(_) => vec![HunkButton::Op(HunkOp::Unstage)],
        DiffTarget::Commit(..) | DiffTarget::Between(..) | DiffTarget::ToWorkdir(..) => vec![HunkButton::Revert],
        // From the working tree to a commit: nothing to stage or revert per hunk.
        DiffTarget::FromWorkdir(..) => Vec::new(),
    };
    let commentable = comment_lines(v, h);
    if commentable > 0 {
        list.push(HunkButton::Comment);
    }
    let (row_h, mid_y) = (v.style.row_h, rect.top() + v.style.row_h / 2.0);
    let body = text_px(row_h);
    let t = v.t;
    let revert_tip = match v.target {
        DiffTarget::ToWorkdir(oid, _) => {
            let sha = crate::ui::common::short(*oid);
            if selected > 0 {
                t.trf("diff.hunk.revert_workdir_selected.tip", &[("path", &path), ("sha", &sha)])
            } else {
                t.trf("diff.hunk.revert_workdir.tip", &[("path", &path), ("sha", &sha)])
            }
        }
        _ if selected > 0 => t.trf("diff.hunk.revert_selected.tip", &[("path", &path)]),
        _ => t.trf("diff.hunk.revert.tip", &[("path", &path)]),
    };
    let updating = t.tr("diff.hunk.updating");
    let mut x = ui.clip_rect().right() - 8.0;
    for (i, &b) in list.iter().enumerate() {
        let label =
            if let HunkButton::Comment = b { t.trn("diff.hunk.comment_lines", commentable, &[]) } else { button_label(t, b, selected) };
        let discard = matches!(b, HunkButton::Op(HunkOp::Discard));
        let styled = if discard { buttons::danger(label) } else { buttons::secondary(label) };
        let mut styled = styled.text_size(body).height(row_h - 2.0).min_width(MIN_W).enabled(v.hunk_gen.is_some()).why(Some(updating));
        let comment_tip = t.tr("diff.hunk.comment_lines.tip");
        match b {
            HunkButton::Revert => styled = styled.tip(&revert_tip),
            HunkButton::Comment => styled = styled.tip(comment_tip),
            HunkButton::Op(_) => {}
        }
        if discard && i > 0 {
            // Set apart from Stage: a wider gap with a divider in its middle.
            x -= APART - GAP;
            let line_x = x + APART / 2.0;
            let half = (row_h - 6.0) / 2.0;
            ui.painter().vline(line_x, (mid_y - half)..=(mid_y + half), Stroke::new(1.0_f32, color::border()));
        }
        let w = styled.desired_width(ui);
        let r = Rect::from_min_size(pos2(x - w, rect.top() + 1.0), vec2(w, row_h - 2.0));
        let button = ui.put(r, styled);
        x -= w + GAP;
        let (true, Some(generation)) = (button.clicked(), v.hunk_gen) else { continue };
        let op = match b {
            HunkButton::Comment => {
                msgs.push(Msg::PullRequest(PullRequestMsg::LineComment(LineCommentMsg::OnSelection)));
                continue;
            }
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
        let yes = t.tr("common.discard.button");
        msgs.push(match (op, selected) {
            (HunkOp::Discard, 0) => {
                Msg::confirm_danger(t.tr("diff.hunk.discard.title"), t.trf("diff.hunk.discard.body", &[("path", &path)]), yes, m)
            }
            (HunkOp::Discard, n) => {
                let body = t.trn("diff.discard_lines.body", n, &[("path", &path)]);
                Msg::confirm_danger(t.tr("diff.discard_lines.title"), body, yes, m)
            }
            _ => m,
        });
    }
}

/// How many lines Comment on N lines covers in hunk `h`: the commentable lines from the first
/// selected one to the last; 0 when comments can't start here or nothing is selected.
fn comment_lines(v: &DiffView, h: usize) -> usize {
    if !v.comments.is_some_and(|c| c.blocked.is_none()) {
        return 0;
    }
    let Some(sel) = v.sel.filter(|s| s.count_in(v.target, h) > 0) else { return 0 };
    let (Some(&a), Some(&b), Some(hunk)) = (sel.lines.first(), sel.lines.last(), v.diff.hunks.get(h)) else { return 0 };
    hunk.lines.iter().take(b + 1).skip(a).filter(|l| glowfly_core::forge::line_key(l).is_some()).count()
}

/// A button's text in `t`: "Stage hunk", or "Stage N lines" with `selected` lines.
fn button_label(t: Lang, b: HunkButton, selected: usize) -> String {
    if selected == 0 {
        return match b {
            HunkButton::Comment => t.tr("diff.hunk.comment_lines.tip"),
            HunkButton::Revert => t.tr("diff.hunk.revert"),
            HunkButton::Op(HunkOp::Stage) => t.tr("diff.hunk.stage"),
            HunkButton::Op(HunkOp::Unstage) => t.tr("diff.hunk.unstage"),
            HunkButton::Op(HunkOp::Discard) => t.tr("diff.hunk.discard"),
        }
        .to_owned();
    }
    match b {
        HunkButton::Comment => t.trn("diff.hunk.comment_lines", selected, &[]),
        HunkButton::Revert => t.trn("diff.hunk.revert_lines", selected, &[]),
        HunkButton::Op(HunkOp::Stage) => t.trn("diff.hunk.stage_lines", selected, &[]),
        HunkButton::Op(HunkOp::Unstage) => t.trn("diff.hunk.unstage_lines", selected, &[]),
        HunkButton::Op(HunkOp::Discard) => t.trn("diff.hunk.discard_lines", selected, &[]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::diff::DiffStyle;
    use glowfly_core::settings::Settings;

    #[test]
    fn button_text_is_body_size_and_never_under_the_floor() {
        let smallest = DiffStyle::new(&Settings { diff_font_size: 8.0, ..Settings::default() });
        assert!(text_px(smallest.row_h) >= type_scale::CAPTION, "at {} px rows", smallest.row_h);
        assert_eq!(text_px(DiffStyle::new(&Settings::default()).row_h), type_scale::BODY);
    }
}
