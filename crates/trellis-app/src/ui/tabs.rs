//! The tab bar above the toolbar: one tab per open repository, plus a "new tab" button.

use super::theme::color;
use crate::msg::Msg;
use crate::state::AppState;
use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, pos2, vec2};

const HEIGHT: f32 = 30.0;

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    egui::TopBottomPanel::top("tabs")
        .exact_height(HEIGHT)
        .frame(egui::Frame::new().fill(color::bg()).inner_margin(egui::Margin { left: 6, right: 6, top: 0, bottom: 0 }))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 1.0;
                // Tabs shrink to share the width once there are many.
                let room = ui.available_width() - HEIGHT;
                let width = (room / st.tabs.len() as f32).clamp(60.0, 220.0);
                for (i, tab) in st.tabs.iter().enumerate() {
                    let active = i == st.active_tab;
                    // The active tab's name comes from its loaded repository.
                    let name = match (&st.snap, active) {
                        (Some(snap), true) => snap.name.as_str(),
                        _ => tab.name.as_str(),
                    };
                    let path = tab.path.as_ref().map(|p| p.display().to_string());
                    tab_button(ui, i, name, path.as_deref(), active, width, msgs);
                }
                let (rect, r) = ui.allocate_exact_size(vec2(HEIGHT, HEIGHT), Sense::click());
                let r = r.on_hover_text("New tab (Ctrl+T)");
                let c = rect.center();
                let col = if r.hovered() { color::text() } else { color::weak() };
                let s = Stroke::new(1.6_f32, col);
                ui.painter().line_segment([pos2(c.x - 6.0, c.y), pos2(c.x + 6.0, c.y)], s);
                ui.painter().line_segment([pos2(c.x, c.y - 6.0), pos2(c.x, c.y + 6.0)], s);
                if r.clicked() {
                    msgs.push(Msg::NewTab);
                }
            });
        });
}

fn tab_button(ui: &mut egui::Ui, i: usize, name: &str, path: Option<&str>, active: bool, width: f32, msgs: &mut Vec<Msg>) {
    let (rect, r) = ui.allocate_exact_size(vec2(width, HEIGHT), Sense::click());
    let close_rect = Rect::from_center_size(pos2(rect.right() - 14.0, rect.center().y), vec2(16.0, 16.0));
    let close = ui.interact(close_rect, r.id.with("close"), Sense::click());
    let p = ui.painter();
    let fill = if active {
        color::toolbar()
    } else if r.hovered() {
        color::hover()
    } else {
        color::bg()
    };
    p.rect_filled(rect, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, fill);
    if active {
        p.line_segment([rect.left_top() + vec2(6.0, 0.5), rect.right_top() + vec2(-6.0, 0.5)], Stroke::new(2.0_f32, color::primary()));
    }
    // Name, clipped before the close button.
    let text_col = if active { color::text() } else { color::weak() };
    let clip = Rect::from_min_max(rect.min, pos2(close_rect.left() - 4.0, rect.max.y));
    p.with_clip_rect(clip).text(pos2(rect.left() + 12.0, rect.center().y), Align2::LEFT_CENTER, name, FontId::proportional(13.0), text_col);
    // Close button: shown on the active and hovered tabs.
    if active || r.hovered() || close.hovered() {
        if close.hovered() {
            p.rect_filled(close_rect, 3.0, color::selected());
        }
        let c = close_rect.center();
        let s = Stroke::new(1.4_f32, text_col);
        p.line_segment([c + vec2(-4.0, -4.0), c + vec2(4.0, 4.0)], s);
        p.line_segment([c + vec2(-4.0, 4.0), c + vec2(4.0, -4.0)], s);
    }
    let r = match path {
        Some(path) => r.on_hover_text(path),
        None => r,
    };
    if close.clicked() || r.middle_clicked() {
        msgs.push(Msg::CloseTab(i));
    } else if r.clicked() {
        msgs.push(Msg::SelectTab(i));
    }
}
