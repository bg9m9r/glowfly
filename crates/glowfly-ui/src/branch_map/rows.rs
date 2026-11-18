//! The Branch map's list: one 56 px painted row per branch, the main branch first, then the
//! newest tip first. Line 1 is the warning mark (when merging the branch and the main branch
//! would conflict), the name, the Main and Current badges and the time since the last commit;
//! line 2 its numbers. Each row is one button for screen readers, named with the same numbers as
//! text, and takes Tab focus.

use crate::access;
use crate::journal::chip;
use crate::theme::{color, type_scale};
use crate::widgets::chip::{self as chips, ChipSize};
use crate::widgets::row_fill::{self, RowState};
use crate::widgets::severity::{self, Severity};
use crate::widgets::{paint_cut_text, paint_text};
use eframe::egui::{self, Align2, CursorIcon, Rect, Sense, Ui, pos2, vec2};
use glowfly_core::branch_map::BranchMap;
use glowfly_model::branch_map::BranchMapMsg;
use glowfly_model::msg::Msg;
use glowfly_model::state::AppState;
use glowfly_model::state::branch_map as words;

const ROW_H: f32 = 56.0;
const PAD_X: f32 = 16.0;
/// The warning mark's size and the room kept for it before every name, so names line up.
const MARK: f32 = 14.0;
const MARK_ROOM: f32 = 22.0;

pub(super) fn show(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.session.branch_map.view else { return };
    let Some(map) = view.map() else { return };
    let t = st.settings.lang();
    egui::ScrollArea::vertical().id_salt("branch-map-rows").auto_shrink([false, false]).show_rows(
        ui,
        ROW_H,
        map.rows.len(),
        |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for i in range {
                let row = &map.rows[i];
                let selected = view.selected.as_ref() == Some(&row.branch);
                let age = words::age(t, st.cues.now, row.time);
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
                access::name_button(&resp, true, &words::row_name(t, map, i, &age));
                if selected {
                    access::mark_selected(&resp);
                }
                let painted = ui.is_rect_visible(rect).then(|| {
                    row_fill::paint_state(ui.painter(), rect, RowState::of(&resp, selected, resp.hovered()));
                    paint(ui, st, map, i, rect, &age)
                });
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                let pointer = ui.ctx().pointer_hover_pos();
                let over = |r: Option<Rect>| r.zip(pointer).is_some_and(|(r, p)| r.contains(p));
                let (over_time, over_mark) = (over(painted.map(|p| p.0)), over(painted.and_then(|p| p.1)));
                let resp = match words::conflict_tip(t, map, i) {
                    Some(tip) if resp.hovered() && over_mark => resp.on_hover_text_at_pointer(tip),
                    _ if resp.hovered() && over_time => {
                        resp.on_hover_text_at_pointer(st.settings.date_tooltip(row.time, st.cues.now, st.cues.utc_offset))
                    }
                    _ => resp,
                };
                if resp.clicked() {
                    msgs.push(Msg::BranchMap(BranchMapMsg::Select(row.branch.clone())));
                }
                ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, (1.0, color::border()));
            }
        },
    );
}

/// Paints row `i`'s two lines; returns where the time and the warning mark went.
fn paint(ui: &Ui, st: &AppState, map: &BranchMap, i: usize, rect: Rect, age: &str) -> (Rect, Option<Rect>) {
    let (t, p) = (st.settings.lang(), ui.painter());
    let (left, right) = (rect.left() + PAD_X, rect.right() - PAD_X);
    let (y1, y2) = (rect.top() + 18.0, rect.top() + 39.0);
    let (small, body) = (type_scale::font(type_scale::SMALL), type_scale::font(type_scale::BODY));
    let time = paint_text(p, pos2(right, y1), Align2::RIGHT_CENTER, age, small.clone(), color::weak());
    let mark = map.conflicts_with_main(i).is_some().then(|| {
        let c = pos2(left + MARK / 2.0, y1);
        severity::paint(p, c, MARK, Severity::Warning, color::warn());
        Rect::from_center_size(c, vec2(MARK, MARK))
    });
    let x = left + MARK_ROOM;
    let mut badges = Vec::new();
    if i == 0 {
        badges.push(t.tr("branch_map.row.main"));
    }
    if map.current == Some(i) {
        badges.push(t.tr("branch_map.row.current"));
    }
    let widths: f32 = badges.iter().map(|b| chips::Chip::new(ChipSize::Badge, *b).width(p.ctx()) + 6.0).sum();
    let room = (time.left() - 12.0 - x - widths).max(40.0);
    let name = paint_cut_text(p, pos2(x, y1), map.rows[i].branch.shown(), body, color::text(), room);
    let mut bx = x + name.rect.width() + 8.0;
    for b in badges {
        bx += chip(p, ChipSize::Badge, bx, y1, b, right - bx, color::text()).width() + 6.0;
    }
    paint_cut_text(p, pos2(x, y2), &words::stats_line(t, map, i), small, color::weak(), (right - x).max(0.0));
    (time, mark)
}
