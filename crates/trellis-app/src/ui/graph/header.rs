//! The graph's column header: the column titles and, at the right end of the message column,
//! the history notes (how many commits a truncated history shows, Load more, and a shallow
//! clone's note with its Fetch full history link).

use super::paint::Columns;
use crate::msg::{JobMsg, LoadMsg, Msg};
use crate::state::shallow::{header_tooltip, history_remote};
use crate::ui::theme::color;
use eframe::egui::{self, Align2, FontId, Pos2, Response, Sense, Stroke, pos2, vec2};
use trellis_core::repo::Snapshot;
use trellis_core::settings::GraphColumns;
use trellis_core::shallow_ops::Deepen;

/// Space between the notes at the right of the message column.
const GAP: f32 = 10.0;

const FETCH_FULL: &str = "Fetch full history";

/// Draws the header row. Rows lay out their own columns the same way.
pub(super) fn column_header(ui: &mut egui::Ui, snap: &Snapshot, graph_w: f32, cols_cfg: &GraphColumns, msgs: &mut Vec<Msg>) {
    let (hdr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    let cols = Columns::new(hdr, graph_w, cols_cfg);
    let p = ui.painter();
    let font = FontId::proportional(10.5);
    let weak = color::weak();
    p.line_segment([hdr.left_bottom(), hdr.right_bottom()], Stroke::new(1.0_f32, color::border()));
    let header = |col: (f32, f32), text: &str, on: bool| {
        if on {
            p.text(pos2(col.0 + 6.0, hdr.center().y), Align2::LEFT_CENTER, text, font.clone(), weak);
            if col.0 > hdr.left() + 1.0 {
                p.line_segment([pos2(col.0, hdr.top() + 4.0), pos2(col.0, hdr.bottom() - 4.0)], Stroke::new(1.0_f32, color::border()));
            }
        }
    };
    header(cols.refs, "BRANCH / TAG", cols_cfg.refs);
    header(cols.graph, "GRAPH", cols_cfg.tree);
    header(cols.sha, "SHA", cols_cfg.sha);
    header((cols.msg.0 - 6.0, cols.msg.1), "COMMIT MESSAGE", cols_cfg.message);
    header(cols.changes, "CHANGES", cols_cfg.changes);
    header(cols.author, "AUTHOR", cols_cfg.author);
    header(cols.date, "DATE", cols_cfg.date);

    let shallow = snap.is_shallow();
    if !snap.truncated && !shallow {
        return;
    }
    // Laid out right to left from the end of the message column. The links always show; the
    // notes only where they clear the column's title.
    let y = hdr.center().y;
    let title_end = cols.msg.0 + p.layout_no_wrap("COMMIT MESSAGE".into(), font.clone(), weak).size().x + 12.0;
    let mut x = cols.msg.1;
    if snap.truncated && snap.commits.len() < crate::state::MAX_COMMITS {
        let (resp, w) = link(ui, &font, pos2(x, y), "load-more", "Load more", "Load more");
        x -= w + GAP;
        if resp.clicked() {
            msgs.push(Msg::Load(LoadMsg::LoadMoreCommits));
        }
    }
    let tooltip = shallow.then(|| header_tooltip(snap));
    if let Some(tip) = &tooltip
        && history_remote(snap).is_some()
    {
        // A narrow column shows the shorter text, so it doesn't run into the title; the tooltip
        // and the link's accessible name still say what it does.
        let wide = p.layout_no_wrap(FETCH_FULL.into(), font.clone(), color::accent()).size().x;
        let text = if x - wide > title_end { FETCH_FULL } else { "Full history" };
        let (resp, w) = link(ui, &font, pos2(x, y), "fetch-full-history", FETCH_FULL, text);
        x -= w + GAP;
        if resp.on_hover_text(tip).clicked() {
            msgs.push(Msg::Job(JobMsg::Deepen(Deepen::Full)));
        }
    }
    if snap.truncated {
        let count = p.layout_no_wrap(format!("newest {} commits", snap.commits.len()), font.clone(), color::faint());
        let count_x = x - count.size().x;
        if count_x > title_end {
            p.galley(pos2(count_x, y - count.size().y / 2.0), count, color::faint());
        }
        x = count_x - GAP;
    }
    if let Some(tip) = tooltip {
        let label = p.layout_no_wrap("Shallow clone".into(), font.clone(), color::faint());
        let rect = Align2::RIGHT_CENTER.anchor_size(pos2(x, y), label.size());
        if rect.left() > title_end {
            p.galley(rect.min, label, color::faint());
            let _ = ui.interact(rect, ui.id().with("shallow-clone"), Sense::hover()).on_hover_text(tip);
        }
    }
}

/// An accent text link showing `text` with its right end at `right`, underlined on hover and
/// named `label` for accessibility. Returns its response and width.
fn link(ui: &egui::Ui, font: &FontId, right: Pos2, id: &str, label: &str, text: &str) -> (Response, f32) {
    let p = ui.painter();
    let g = p.layout_no_wrap(text.into(), font.clone(), color::accent());
    let rect = Align2::RIGHT_CENTER.anchor_size(right, g.size());
    let resp = ui.interact(rect, ui.id().with(id), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let name = label.to_owned();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, &name));
    if resp.hovered() {
        p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0_f32, color::accent()));
    }
    p.galley(rect.min, g, color::accent());
    (resp, rect.width())
}
