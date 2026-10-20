//! The repository tabs: one per open repository, plus a "new tab" button and, at the far end
//! when there are closed tabs or the tabs had to shrink, the tab list (`tab_list.rs`). With the
//! system frame they get their own bar above the toolbar (`show`); a drawn title bar holds them
//! itself (`strip`, from `title_bar.rs`). Tabs share the width evenly and are drawn by
//! `tab_paint.rs`; they are dragged sideways to reorder them (`tab_drag.rs`, Esc cancels a
//! drag), and right-clicking one offers Rename tab and the close actions. With Group worktree
//! tabs on, tabs of one repository's worktrees form a group (`state/tab_groups.rs`) marked by a
//! colored strip along their bottom; a tab dropped outside its group takes the group.

use super::tab_drag::{Slot, drag};
use super::tab_paint::{ROUNDING, TabLook, close_rect, group_color, paint_group_strip, paint_separator, paint_tab};
use super::theme::color;
use super::widgets::truncate::full_tip;
use crate::msg::Msg;
use crate::msg::TabsMsg;
use crate::state::AppState;
use crate::state::menu_bar::tab_name;
use crate::state::tab_groups::{self, new_index, shown_at};
use crate::ui::theme::type_scale;
use crate::ui::widgets::tab_glyph::{self, TabGlyph};
use eframe::egui::{self, Align, Button, Layout, Response, Sense, Stroke, StrokeKind, WidgetInfo, WidgetType, vec2};
use trellis_core::i18n::Lang;

pub(crate) use super::tab_drag::dragging;

/// The tab bar's height with the system frame.
pub(super) const HEIGHT: f32 = 30.0;

/// The tab bar under the system frame's title bar.
pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    egui::TopBottomPanel::top("tabs")
        .exact_height(HEIGHT)
        .frame(egui::Frame::new().fill(color::toolbar()).inner_margin(egui::Margin { left: 6, right: 6, top: 0, bottom: 0 }))
        .show(ctx, |ui| strip(ui, st, HEIGHT, msgs));
}

/// The tabs `height` tall, then + and the tab list, left to right in `ui`.
pub(super) fn strip(ui: &mut egui::Ui, st: &AppState, height: f32, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    ui.allocate_ui_with_layout(vec2(ui.available_width(), height), Layout::left_to_right(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        // Tabs shrink to share the width once there are many.
        let list = super::tab_list::wanted(st, ui.available_width(), height);
        let room = ui.available_width() - height * (1.0 + f32::from(u8::from(list)));
        let width = (room / st.tabs.len() as f32).clamp(60.0, 220.0);
        let pitch = width + ui.spacing().item_spacing.x;
        let strip_left = ui.cursor().left();
        let slots: Vec<Slot> = (0..st.tabs.len())
            .map(|i| {
                let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
                let r = ui.interact(rect, ui.id().with(("tab", i)), Sense::click_and_drag());
                let close = ui.interact(close_rect(rect), r.id.with("close"), Sense::click());
                Slot { rect, r, close }
            })
            .collect();
        let on = st.settings.group_worktree_tabs;
        let blocks = tab_groups::blocks(&st.tabs, on);
        let shown = tab_groups::shown_groups(&st.tabs, on);
        let floating = drag(ui, &slots, &blocks, strip_left, pitch, msgs);
        let gap = pitch - width;
        let name = |i: usize| tab_name(st, i);
        let (active_font, font) = (type_scale::strong(ui, type_scale::BODY), type_scale::font(type_scale::BODY));
        let font_of = |active: bool| if active { active_font.clone() } else { font.clone() };
        // Lit tabs (active or hovered) get no separator beside them, and none while dragging.
        if floating.is_none() {
            let lit = |i: usize, s: &Slot| i == st.active_tab || s.r.hovered() || s.close.hovered();
            for (i, pair) in slots.windows(2).enumerate() {
                if !lit(i, &pair[0]) && !lit(i + 1, &pair[1]) {
                    paint_separator(ui.painter(), pair[0].rect, gap);
                }
            }
        }
        for (i, Slot { rect, r, close }) in slots.into_iter().enumerate() {
            let active = i == st.active_tab;
            let p = ui.painter();
            let group = shown_at(&shown, i);
            match &floating {
                Some(f) if f.from == i => {}
                // While dragging, the others slide to where they would be after the drop.
                Some(f) => {
                    let shift = (new_index(&f.order, i) as f32 - i as f32) * pitch;
                    let rect = rect.translate(vec2(shift, 0.0));
                    paint_tab(p, rect, name(i), font_of(active), TabLook { active, ..TabLook::default() });
                    if let Some((members, n)) = group {
                        // Where the group's ends will be after the drop.
                        let at = |j: usize| new_index(&f.order, j);
                        let (lo, hi) = members.clone().map(at).fold((usize::MAX, 0), |(lo, hi), j| (lo.min(j), hi.max(j)));
                        paint_group_strip(p, rect, group_color(*n), at(i) == lo, at(i) == hi, gap);
                    }
                }
                None => {
                    let (hovered, close_hovered) = (r.hovered() || close.hovered(), close.hovered());
                    let look = TabLook { active, hovered, show_close: active || hovered, close_hovered };
                    let cut = paint_tab(p, rect, name(i), font_of(active), look);
                    if let Some((members, n)) = group {
                        paint_group_strip(p, rect, group_color(*n), i == members.start, i + 1 == members.end, gap);
                    }
                    let tip = match (&st.tabs[i].path, group.and(st.tabs[i].group.as_ref())) {
                        (Some(path), Some(g)) => {
                            let shown = trellis_core::native_path::shown(path);
                            Some(t.trf("tabs.group_tip", &[("path", &shown), ("group", &g.name)]))
                        }
                        (Some(path), None) => Some(trellis_core::native_path::shown(path)),
                        (None, _) => None,
                    };
                    // A cut name puts the whole name first.
                    let r = match full_tip(cut.cut, name(i), tip) {
                        Some(tip) => r.on_hover_text(tip),
                        None => r,
                    };
                    if close.clicked() || r.middle_clicked() {
                        msgs.push(Msg::Tabs(TabsMsg::CloseTab(i)));
                    } else if r.clicked() {
                        msgs.push(Msg::Tabs(TabsMsg::SelectTab(i)));
                    }
                    r.context_menu(|ui| tab_menu(ui, t, i, st, msgs));
                }
            }
        }
        // The dragged tab, drawn last so it floats above the others.
        if let Some(f) = floating {
            let active = f.from == st.active_tab;
            // An inactive tab is lifted off the strip on the hover fill, so it reads as held.
            let look = TabLook { active, hovered: true, show_close: active, close_hovered: false };
            paint_tab(ui.painter(), f.rect, name(f.from), font_of(active), look);
            if let Some((_, n)) = shown_at(&shown, f.from) {
                paint_group_strip(ui.painter(), f.rect, group_color(*n), true, true, gap);
            }
            ui.painter().rect_stroke(f.rect, ROUNDING, Stroke::new(1.0_f32, color::accent()), StrokeKind::Inside);
        }
        new_tab_button(ui, t, &st.keymap, height, msgs);
        if list {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| super::tab_list::button(ui, t, st, height, msgs));
        }
    });
}

/// The + after the tabs: drawn like a tab's close × (`tab_glyph`), on the same line, in its
/// own `height`-square slot.
fn new_tab_button(ui: &mut egui::Ui, t: Lang, keys: &crate::keymap::Keymap, height: f32, msgs: &mut Vec<Msg>) {
    let (rect, r) = ui.allocate_exact_size(vec2(height, height), Sense::click());
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, t.tr("tabs.new_tab.tip")));
    let r = r.on_hover_text(keys.tip(t.tr("tabs.new_tab.tip"), crate::keymap::BindingId::NewTab));
    tab_glyph::paint(ui.painter(), rect.center(), TabGlyph::Plus, color::weak(), r.hovered().then(color::selected));
    if r.clicked() {
        msgs.push(Msg::Tabs(TabsMsg::NewTab));
    }
}

/// The right-click menu of tab `i`. It acts on that tab without selecting it.
fn tab_menu(ui: &mut egui::Ui, t: Lang, i: usize, st: &AppState, msgs: &mut Vec<Msg>) {
    let n = st.tabs.len();
    item(ui, msgs, st.tabs[i].path.is_some(), Button::new(t.tr("tabs.menu.rename")), Msg::Tabs(TabsMsg::RenameTabDialog(i)));
    ui.separator();
    item(ui, msgs, true, Button::new(t.tr("tabs.menu.close")), Msg::Tabs(TabsMsg::CloseTab(i)));
    item(ui, msgs, n > 1, Button::new(t.tr("tabs.menu.close_others")), Msg::Tabs(TabsMsg::CloseOtherTabs(i)));
    item(ui, msgs, i + 1 < n, Button::new(t.tr("tabs.menu.close_right")), Msg::Tabs(TabsMsg::CloseTabsToRight(i)));
    ui.separator();
    let reopen_key = st.keymap.hint(crate::keymap::BindingId::ReopenTab).unwrap_or_default();
    let reopen = Button::new(t.tr("tabs.menu.reopen")).shortcut_text(reopen_key);
    let r = item(ui, msgs, !st.closed_tabs.is_empty(), reopen, Msg::Tabs(TabsMsg::ReopenClosedTab));
    if let Some(closed) = st.closed_tabs.last() {
        r.on_hover_text(trellis_core::native_path::shown(&closed.path));
    }
}

/// A menu entry, greyed out unless `enabled`; clicking it pushes `msg` and closes the menu.
fn item(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, enabled: bool, button: Button, msg: Msg) -> Response {
    let r = ui.add_enabled(enabled, button);
    if r.clicked() {
        msgs.push(msg);
        ui.close_menu();
    }
    r
}

#[cfg(test)]
mod tests {
    use crate::state::tabs::index_after_move;

    #[test]
    fn index_after_move_shifts_the_tabs_in_between() {
        // Moving tab 1 right to 3: tabs 2 and 3 shift left, the others stay.
        assert_eq!([0, 1, 2, 3, 4].map(|i| index_after_move(i, 1, 3)), [0, 3, 1, 2, 4]);
        // Moving tab 3 left to 1: tabs 1 and 2 shift right.
        assert_eq!([0, 1, 2, 3, 4].map(|i| index_after_move(i, 3, 1)), [0, 2, 3, 1, 4]);
        // The moved tab lands at `to`; moving to the same place changes nothing.
        assert_eq!(index_after_move(1, 1, 3), 3);
        assert_eq!([0, 1, 2].map(|i| index_after_move(i, 1, 1)), [0, 1, 2]);
    }
}
