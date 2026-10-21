//! The preview dialog's rows: the history afterwards as one lane in the subject's colour (kept
//! commits, rewritten ones and copies with their old SHA, the merge a merge makes, and where it
//! stops), and the commits no branch or tag will reach. Each row is its own 28 px rect named
//! "<summary> <sha> <tag>" for screen readers and tests.

use super::op_preview_dialog::Env;
use super::theme::{color, type_scale};
use super::widgets::{Icon, paint_cut_text, paint_icon_sized};
use crate::state::op_preview::{gone_tag, stop_tag};
use eframe::egui::{self, Align2, Color32, Pos2, Sense, Ui, pos2, vec2};
use glowfly_core::op_preview::{AfterRow, CommitLine, Gone, OpPreview};

const ROW_H: f32 = 28.0;
/// The lane's x, from the rows' left edge.
const LANE_X: f32 = 8.0;
const TEXT_X: f32 = 24.0;

/// How a row's mark is painted.
enum Dot {
    Filled(f32, Color32),
    Ring(Color32),
    Warn,
}

/// One row's parts.
struct Row<'a> {
    dot: Dot,
    summary: String,
    sha: Option<String>,
    /// The old SHA leads to the new version (an arrow after the SHA).
    into: bool,
    age: Option<String>,
    tag: Option<(&'a str, Color32)>,
}

fn short(oid: git2::Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// The history afterwards: `p.after` as a lane, continuing past the last row when more follows.
pub(super) fn after(ui: &mut Ui, env: &Env, p: &OpPreview, tone: Color32) {
    let t = env.settings.lang();
    let branch = p.subject.as_deref().unwrap_or("HEAD");
    let n = p.after.len();
    for (i, r) in p.after.iter().enumerate() {
        let row = match r {
            AfterRow::Kept { line, arrives } => Row {
                dot: Dot::Filled(3.0, tone),
                summary: line.summary.clone(),
                sha: Some(short(line.oid)),
                into: false,
                age: Some(age(env, line)),
                tag: arrives.then(|| (t.tr("op_preview.tag.arrives"), color::added())),
            },
            AfterRow::Rewritten { from, copy } => {
                let tag = if *copy { t.tr("op_preview.tag.copied") } else { t.tr("op_preview.tag.moved") };
                Row {
                    dot: Dot::Filled(4.0, color::added()),
                    summary: from.summary.clone(),
                    sha: Some(short(from.oid)),
                    into: true,
                    age: None,
                    tag: Some((tag, color::added())),
                }
            }
            AfterRow::NewMerge { theirs } => Row {
                dot: Dot::Ring(color::added()),
                summary: t.trf("op_preview.merge_row", &[("theirs", theirs), ("branch", &branch)]),
                sha: None,
                into: false,
                age: None,
                tag: Some((t.tr("op_preview.tag.new_merge"), color::added())),
            },
            AfterRow::Stopped { at } => Row {
                dot: Dot::Warn,
                summary: match (at, &p.op) {
                    (Some(l), _) => l.summary.clone(),
                    (None, glowfly_core::op_preview::PreviewOp::Merge { theirs, .. }) => {
                        t.trf("op_preview.merge_row", &[("theirs", theirs), ("branch", &branch)])
                    }
                    (None, _) => String::new(),
                },
                sha: at.as_ref().map(|l| short(l.oid)),
                into: false,
                age: None,
                tag: Some((stop_tag(t, p), color::warn())),
            },
        };
        let joins = (i > 0, i + 1 < n || p.after_more);
        paint_row(ui, row, Some((tone, joins)));
    }
}

/// The commits no branch or tag will reach: a removed-colour dot, the summary, the SHA and a
/// weak reason.
pub(super) fn gone(ui: &mut Ui, env: &Env, gone: &[Gone]) {
    let t = env.settings.lang();
    for g in gone {
        let row = Row {
            dot: Dot::Filled(3.0, color::removed()),
            summary: g.line.summary.clone(),
            sha: Some(short(g.line.oid)),
            into: false,
            age: None,
            tag: Some((gone_tag(t, g.why), color::weak())),
        };
        paint_row(ui, row, None);
    }
}

fn age(env: &Env, line: &CommitLine) -> String {
    env.settings.format_time(line.time, env.now, env.offset)
}

/// Paints one row; `lane` is the lane's colour and whether it joins the rows above and below.
fn paint_row(ui: &mut Ui, row: Row, lane: Option<(Color32, (bool, bool))>) {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, ROW_H), Sense::hover());
    let label =
        [Some(row.summary.as_str()), row.sha.as_deref(), row.tag.map(|(s, _)| s)].into_iter().flatten().collect::<Vec<_>>().join(" ");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &label));
    // The dialog's fill, so a mark on the lane hides the line under it.
    let bg = ui.visuals().window_fill;
    let p = ui.painter();
    let x = rect.left() + LANE_X;
    let cy = rect.center().y;
    if let Some((tone, (above, below))) = lane {
        let top = if above { rect.top() } else { cy };
        let bottom = if below { rect.bottom() } else { cy };
        if bottom > top {
            p.line_segment([pos2(x, top), pos2(x, bottom)], (2.0, tone));
        }
    }
    let c = pos2(x, cy);
    match row.dot {
        Dot::Filled(r, col) => {
            p.circle_filled(c, r, col);
        }
        Dot::Ring(col) => {
            p.circle_filled(c, 4.0, bg);
            p.circle_stroke(c, 4.0, (2.0, col));
        }
        Dot::Warn => {
            p.circle_filled(c, 8.0, bg);
            paint_icon_sized(p, c, Icon::Warning, color::warn(), 14.0);
        }
    }
    let (small, mono) = (type_scale::font(type_scale::SMALL), type_scale::mono(type_scale::SMALL));
    let mut right = rect.right() - 4.0;
    if let Some((tag, col)) = row.tag {
        right = p.text(pos2(right, cy), Align2::RIGHT_CENTER, tag, small.clone(), col).left() - 12.0;
    }
    if let Some(a) = &row.age {
        right = p.text(pos2(right, cy), Align2::RIGHT_CENTER, a, small, color::weak()).left() - 12.0;
    }
    if row.into {
        paint_icon_sized(p, Pos2::new(right - 6.0, cy), Icon::Into, color::weak(), 12.0);
        right -= 18.0;
    }
    if let Some(sha) = &row.sha {
        right = p.text(pos2(right, cy), Align2::RIGHT_CENTER, sha, mono, color::weak()).left() - 12.0;
    }
    let left = rect.left() + TEXT_X;
    let body = type_scale::font(type_scale::BODY);
    paint_cut_text(p, pos2(left, cy), &row.summary, body, color::text(), (right - left).max(0.0));
}
