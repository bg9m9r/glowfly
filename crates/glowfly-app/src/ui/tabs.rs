//! The repository tabs: one per open repository, plus a "new tab" button and, at the far end
//! when there are closed tabs or the tabs had to shrink, the tab list (`tab_list.rs`). With the
//! system frame they get their own bar above the toolbar (`show`); a drawn title bar holds them
//! itself (`strip`, from `title_bar.rs`). Each tab is as wide as its name (measured in the
//! active tab's font, so selecting one never changes its width) until they no longer fit, when
//! they share the width evenly. They are drawn by `tab_paint.rs`; they are dragged sideways to
//! reorder them (`tab_drag.rs`, Esc cancels a drag), and right-clicking one offers Rename tab
//! and its repository's Color, and the close actions. A coloured repository's tab shows its
//! swatch before the icon, and is that much wider. With Group worktree tabs on, tabs of one repository's worktrees form a
//! group (`state/tab_groups.rs`) marked by a colored strip along their bottom; a tab dropped
//! outside its group takes the group.

use super::backdrop::{self, Surface};
use super::tab_drag::{SLIDE, Slot, drag, lefts_after};
use super::tab_paint::{LIFT, SWATCH_ROOM, TabLook, close_rect, group_color, paint_group_strip, paint_separator, paint_tab, tab_width};
use super::theme::branch_colours::line;
use super::theme::color;
use super::theme::tokens::motion;
use super::widgets::reason::{self, ItemState};
use super::widgets::truncate::full_tip;
use crate::msg::Msg;
use crate::msg::TabsMsg;
use crate::state::AppState;
use crate::state::menu_bar::tab_name;
use crate::state::repo_colours;
use crate::state::tab_groups::{self, new_index, shown_at};
use crate::ui::theme::type_scale;
use crate::ui::widgets::tab_glyph::{self, TabGlyph};
use eframe::egui::{self, Align, Color32, Layout, Response, Sense, WidgetInfo, WidgetType, vec2};
use glowfly_core::i18n::Lang;

pub(crate) use super::tab_drag::dragging;

/// The tab bar's height with the system frame.
pub(super) const HEIGHT: f32 = 30.0;
/// Between two tabs.
const GAP: f32 = 4.0;
/// The + button's width, 4 px after the last tab.
const PLUS_W: f32 = 32.0;
/// The narrowest a tab gets when there are too many to show at their own widths.
const MIN_W: f32 = 60.0;
/// How long the close × takes to fade in or out.
const CLOSE_FADE: f32 = motion::QUICK;

/// The tab bar under the system frame's title bar. Its tabs start `LIFT` below its top, so the
/// active tab has room to stand taller.
pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let margin = egui::Margin { left: 6, right: 6, top: LIFT as i8, bottom: 0 };
    egui::TopBottomPanel::top("tabs")
        .exact_height(HEIGHT)
        .show_separator_line(false)
        .frame(egui::Frame::new().fill(color::toolbar()).inner_margin(margin))
        .show(ctx, |ui| strip(ui, st, HEIGHT - LIFT, msgs));
}

/// The widths of `n` tabs whose own widths are `natural`, in `room`: their own widths when they
/// fit (with `GAP` between them), otherwise an even share each, between `MIN_W` and 220.
pub(super) fn widths(natural: &[f32], room: f32) -> Vec<f32> {
    let n = natural.len();
    if needed(natural) <= room || n == 0 {
        return natural.to_vec();
    }
    let share = ((room - GAP * (n - 1) as f32) / n as f32).clamp(MIN_W, 220.0);
    vec![share; n]
}

/// The room tabs at `natural` widths take, with the gaps between them.
fn needed(natural: &[f32]) -> f32 {
    natural.iter().sum::<f32>() + GAP * natural.len().saturating_sub(1) as f32
}

/// The tabs `height` tall, then + and the tab list, left to right in `ui`.
pub(super) fn strip(ui: &mut egui::Ui, st: &AppState, height: f32, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    ui.allocate_ui_with_layout(vec2(ui.available_width(), height), Layout::left_to_right(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        let name = |i: usize| tab_name(st, i);
        let (active_font, font) = (type_scale::strong(ui, type_scale::BODY), type_scale::font(type_scale::BODY));
        let font_of = |active: bool| if active { active_font.clone() } else { font.clone() };
        let measure = |i: usize| ui.painter().layout_no_wrap(name(i).to_owned(), active_font.clone(), Color32::PLACEHOLDER).size().x;
        let repo_of = |i: usize| repo_colours::tab_colour(st, i).map(color::repo_hue);
        let swatch_room = |i: usize| if repo_of(i).is_some() { SWATCH_ROOM } else { 0.0 };
        let natural: Vec<f32> = (0..st.tabs.len()).map(|i| tab_width(measure(i) + swatch_room(i))).collect();
        // Room for the tabs before + (and the tab list, when it shows).
        let room = ui.available_width() - GAP - PLUS_W;
        let list = super::tab_list::wanted(st, needed(&natural), room);
        let widths = widths(&natural, room - if list { GAP + height } else { 0.0 });
        let strip_left = ui.cursor().left();
        let slots: Vec<Slot> = widths
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                let (rect, _) = ui.allocate_exact_size(vec2(w, height), Sense::hover());
                let r = ui.interact(rect, ui.id().with(("tab", i)), Sense::click_and_drag());
                let close = ui.interact(close_rect(rect), r.id.with("close"), Sense::click());
                Slot { rect, r, close }
            })
            .collect();
        #[cfg(test)]
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(RECTS), slots.iter().map(|s| s.rect).collect::<Vec<_>>()));
        let on = st.settings.group_worktree_tabs;
        let blocks = tab_groups::blocks(&st.tabs, on);
        let shown = tab_groups::shown_groups(&st.tabs, on);
        let floating = drag(ui, &slots, &blocks, msgs);
        // The toolbar's fill, which the active tab runs into, and its band in the checked-out
        // branch's colour (the accent when detached or without a repository).
        let fill = backdrop::fill(st.view.backdrop, Surface::Side, color::panel());
        let band = st.snap.as_ref().and_then(|s| s.head_swatch()).map_or_else(color::accent, line);
        let look = |i: usize, active: bool| TabLook { active, band, repo: repo_of(i), ..TabLook::quiet(fill) };
        let (slide, fade) = if st.settings.reduce_motion {
            (0.0, 0.0)
        } else {
            (motion::duration(SLIDE, ui.ctx()), motion::duration(CLOSE_FADE, ui.ctx()))
        };
        // Lit tabs (active, hovered or pressed) get no separator beside them, and none while dragging.
        if floating.is_none() {
            let lit = |i: usize, s: &Slot| i == st.active_tab || s.r.hovered() || s.close.hovered() || s.r.is_pointer_button_down_on();
            for (i, pair) in slots.windows(2).enumerate() {
                if !lit(i, &pair[0]) && !lit(i + 1, &pair[1]) {
                    paint_separator(ui.painter(), pair[0].rect, GAP);
                }
            }
        }
        // While dragging, the others slide to where they would be after the drop.
        let targets = floating.as_ref().map(|f| lefts_after(&f.order, &widths, strip_left, GAP));
        for (i, Slot { rect, r, close }) in slots.into_iter().enumerate() {
            let active = i == st.active_tab;
            let p = ui.painter();
            let group = shown_at(&shown, i);
            let target = targets.as_ref().map_or(rect.left(), |lefts| lefts[i]);
            let x = ui.ctx().animate_value_with_time(r.id.with("slide"), target, if floating.is_some() { slide } else { 0.0 });
            let shown_x = ui.ctx().animate_bool_with_time(r.id.with("x"), active || r.hovered() || close.hovered(), fade);
            match &floating {
                Some(f) if f.from == i => {}
                Some(f) => {
                    let rect = rect.translate(vec2(x - rect.left(), 0.0));
                    paint_tab(p, rect, name(i), font_of(active), TabLook { close_alpha: shown_x, ..look(i, active) });
                    if let Some((members, n)) = group {
                        // Where the group's ends will be after the drop.
                        let at = |j: usize| new_index(&f.order, j);
                        let (lo, hi) = members.clone().map(at).fold((usize::MAX, 0), |(lo, hi), j| (lo.min(j), hi.max(j)));
                        paint_group_strip(p, rect, group_color(*n), at(i) == lo, at(i) == hi, GAP);
                    }
                }
                None => {
                    let hovered = r.hovered() || close.hovered();
                    let pressed = !active && r.is_pointer_button_down_on() && !close.hovered();
                    let tab = TabLook { hovered, pressed, close_alpha: shown_x, close_hovered: close.hovered(), ..look(i, active) };
                    let cut = paint_tab(p, rect, name(i), font_of(active), tab);
                    if let Some((members, n)) = group {
                        paint_group_strip(p, rect, group_color(*n), i == members.start, i + 1 == members.end, GAP);
                    }
                    let tip = match (&st.tabs[i].path, group.and(st.tabs[i].group.as_ref())) {
                        (Some(path), Some(g)) => {
                            let shown = glowfly_core::native_path::shown(path);
                            Some(t.trf("tabs.group_tip", &[("path", &shown), ("group", &g.name)]))
                        }
                        (Some(path), None) => Some(glowfly_core::native_path::shown(path)),
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
        // The dragged tab, drawn last so it floats above the others: lifted onto the toolbar's
        // fill with a border.
        if let Some(f) = floating {
            let active = f.from == st.active_tab;
            let tab = TabLook { hovered: true, floating: true, close_alpha: if active { 1.0 } else { 0.0 }, ..look(f.from, active) };
            paint_tab(ui.painter(), f.rect, name(f.from), font_of(active), tab);
            if let Some((_, n)) = shown_at(&shown, f.from) {
                paint_group_strip(ui.painter(), f.rect, group_color(*n), true, true, GAP);
            }
        }
        new_tab_button(ui, t, &st.keymap, height, msgs);
        if list {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| super::tab_list::button(ui, t, st, height, msgs));
        }
    });
}

/// Where the strip keeps its tabs' rects for the tests (`tab_rects`).
#[cfg(test)]
const RECTS: &str = "tab-rects";

/// The tabs' rects as last drawn.
#[cfg(test)]
pub(super) fn tab_rects(ctx: &egui::Context) -> Vec<egui::Rect> {
    ctx.data(|d| d.get_temp::<Vec<egui::Rect>>(egui::Id::new(RECTS))).unwrap_or_default()
}

/// The + after the tabs: drawn like a tab's close × (`tab_glyph`), on the same line, in its
/// own `PLUS_W` wide, `height` tall slot.
fn new_tab_button(ui: &mut egui::Ui, t: Lang, keys: &crate::keymap::Keymap, height: f32, msgs: &mut Vec<Msg>) {
    let (rect, r) = ui.allocate_exact_size(vec2(PLUS_W, height), Sense::click());
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
    let no_repo = st.tabs[i].path.is_none().then(|| t.tr("tabs.menu.rename.blocked"));
    item(ui, msgs, t.tr("tabs.menu.rename"), "", no_repo, Msg::Tabs(TabsMsg::RenameTabDialog(i)));
    super::repo_colour_menu::items(ui, t, repo_colours::tab_path(&st.tabs[i]), repo_colours::tab_colour(st, i), msgs);
    ui.separator();
    item(ui, msgs, t.tr("tabs.menu.close"), "", None, Msg::Tabs(TabsMsg::CloseTab(i)));
    let only = (n <= 1).then(|| t.tr("tabs.menu.close_others.blocked"));
    item(ui, msgs, t.tr("tabs.menu.close_others"), "", only, Msg::Tabs(TabsMsg::CloseOtherTabs(i)));
    let last = (i + 1 >= n).then(|| t.tr("tabs.menu.close_right.blocked"));
    item(ui, msgs, t.tr("tabs.menu.close_right"), "", last, Msg::Tabs(TabsMsg::CloseTabsToRight(i)));
    ui.separator();
    let reopen_key = st.keymap.hint(crate::keymap::BindingId::ReopenTab).unwrap_or_default();
    let none_closed = st.closed_tabs.is_empty().then(|| t.tr("tabs.menu.reopen.blocked"));
    let r = item(ui, msgs, t.tr("tabs.menu.reopen"), reopen_key, none_closed, Msg::Tabs(TabsMsg::ReopenClosedTab));
    if let Some(closed) = st.closed_tabs.last() {
        r.on_hover_text(glowfly_core::native_path::shown(&closed.path));
    }
}

/// A menu entry with `shortcut` at its right, blocked while `blocked` holds its reason; clicking
/// it pushes `msg` and closes the menu.
fn item(ui: &mut egui::Ui, msgs: &mut Vec<Msg>, label: &str, shortcut: &str, blocked: Option<&str>, msg: Msg) -> Response {
    let r = reason::menu_button(ui, label, shortcut, ItemState::blocked_by(blocked));
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
