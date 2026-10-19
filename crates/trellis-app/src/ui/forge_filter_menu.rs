//! The filter chip in the PULL REQUESTS and ISSUES headers, and its popover: the built-in
//! filters, the repository's saved filters (edit, move, remove) and New filter…, sent as
//! `Msg::ForgeFilter`; the chip sits left of the shared header's + and ⋯.

use super::common::{forge_error_rows, hint_row};
use super::theme::color;
use super::widgets::section_header::{self, HeaderAdd, HeaderResponse, MenuFn};
use super::widgets::{self, Icon};
use crate::msg::{FilterMsg, Msg};
use crate::state::forge_filters::Hint;
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align, CornerRadius, Layout, Rect, Response, Sense, Stroke, UiBuilder, pos2, vec2};
use trellis_core::forge::filters::matching;
use trellis_core::forge::{Builtin, FilterChoice, FilterKind, Provider, SavedFilter};
use trellis_core::i18n::Lang;
use trellis_core::settings::RepoForgeFilters;

/// What a section's chip shows and offers.
pub(super) struct Picker<'a> {
    pub kind: FilterKind,
    /// The shown filter's name.
    pub label: &'a str,
    pub filters: &'a RepoForgeFilters,
    /// The service the filters apply to.
    pub provider: Option<Provider>,
}

const ROW: f32 = 26.0;

/// The section header (with its + and ⋯) and the filter chip just left of them. A click on
/// the chip, the + or the ⋯ leaves the section as it is.
/// `key` is the section's stable id, `title` its header and `chip_tip` the chip's hover, in `t`.
pub(super) fn header(
    ui: &mut egui::Ui,
    t: Lang,
    (key, title, chip_tip, count): (&'static str, &str, &str, usize),
    open: &mut bool,
    p: Picker,
    (add, menu): (Option<HeaderAdd>, MenuFn),
    msgs: &mut Vec<Msg>,
) -> Response {
    let h = section_header::Header { key, title, count: Some(count), add, menu: Some(menu), t };
    let HeaderResponse { header: resp, free_right: right } = section_header::show(ui, h, open, msgs);
    let painter = ui.painter().clone();
    let (title_font, _) = widgets::section_fonts(ui);
    let title_w = painter.layout_no_wrap(title.to_owned(), title_font, color::text()).size().x;
    let room = (right - (resp.rect.left() + widgets::TITLE_X + title_w + 6.0) - 26.0).max(24.0);
    let job = widgets::truncate::cut_job(p.label, type_scale::font(type_scale::CAPTION), color::weak());
    let galley = widgets::truncate::cut_galley(ui.ctx(), job, room);
    let cy = resp.rect.center().y;
    let chip = Rect::from_min_max(pos2(right - galley.size().x - 26.0, cy - 10.0), pos2(right, cy + 10.0));
    let id = ui.make_persistent_id(("forge-filter-chip", p.kind == FilterKind::Prs));
    let chip_resp = ui.interact(chip, id, Sense::click());
    let popup = id.with("popover");
    let showing = ui.memory(|m| m.is_popup_open(popup));
    if chip_resp.hovered() || showing {
        painter.rect_filled(chip, CornerRadius::same(10), color::selected());
    }
    painter.rect_stroke(chip, CornerRadius::same(10), Stroke::new(1.0_f32, color::border()), egui::StrokeKind::Inside);
    painter.galley(pos2(chip.left() + 8.0, cy - galley.size().y / 2.0), galley, color::weak());
    widgets::chevron(&painter, pos2(chip.right() - 10.0, cy + 0.5), true, color::weak());
    let name = t.trf("sidebar.filter_chip.name", &[("section", &title), ("filter", &p.label)]);
    chip_resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name.clone()));
    let chip_resp = chip_resp.on_hover_text(chip_tip).on_hover_cursor(egui::CursorIcon::PointingHand);
    if chip_resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    egui::popup::popup_below_widget(ui, popup, &chip_resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_width(270.0);
        popover(ui, t, &p, msgs);
    });
    resp
}

fn popover(ui: &mut egui::Ui, t: Lang, p: &Picker, msgs: &mut Vec<Msg>) {
    ui.spacing_mut().item_spacing.y = 0.0;
    let shown = p.filters.choice(p.kind);
    let builtins: &[Builtin] = if p.kind == FilterKind::Prs { &Builtin::PRS } else { &Builtin::ISSUES };
    for &b in builtins {
        let why = p.provider.and_then(|pr| matching::unsupported(pr, p.kind, &b.conditions()));
        let r = row(ui, b.label(), shown == FilterChoice::Builtin(b), why.is_none());
        let r = match (&why, p.provider) {
            (Some(why), _) => r.on_disabled_hover_text(why),
            (None, Some(Provider::AzureDevOps)) if b == Builtin::IssuesMentioningMe => {
                r.on_hover_text("Azure Boards finds mentions from the last 30 days.")
            }
            _ => r,
        };
        if r.clicked() {
            choose(ui, msgs, FilterMsg::Choose { kind: p.kind, choice: FilterChoice::Builtin(b) });
        }
    }
    ui.add_space(4.0);
    ui.separator();
    let saved: Vec<&SavedFilter> = p.filters.saved_of(p.kind).collect();
    ui.label(egui::RichText::new("Saved filters").size(type_scale::CAPTION).color(color::faint()));
    ui.add_space(2.0);
    if saved.is_empty() {
        ui.label(egui::RichText::new("None yet").size(type_scale::SMALL).color(color::faint()));
        ui.add_space(4.0);
    }
    for (i, s) in saved.iter().enumerate() {
        saved_row(ui, s, shown == FilterChoice::Saved(s.id), (i > 0, i + 1 < saved.len()), msgs);
    }
    ui.add_space(4.0);
    ui.separator();
    if row(ui, t.tr("sidebar.new_filter"), false, true).clicked() {
        choose(ui, msgs, FilterMsg::NewDialog(p.kind));
    }
}

fn choose(ui: &egui::Ui, msgs: &mut Vec<Msg>, m: FilterMsg) {
    ui.memory_mut(|mem| mem.close_popup());
    msgs.push(Msg::ForgeFilter(m));
}

/// A selectable row: the text, with a painted check mark when it is the one on show.
fn row(ui: &mut egui::Ui, text: &str, checked: bool, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), if enabled { Sense::click() } else { Sense::hover() });
    paint_row(ui, rect, text, checked, enabled, enabled && resp.hovered());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, checked, text));
    resp
}

fn paint_row(ui: &egui::Ui, rect: Rect, text: &str, checked: bool, enabled: bool, hovered: bool) {
    let p = ui.painter();
    if hovered {
        p.rect_filled(rect, CornerRadius::same(3), color::hover());
    }
    if checked {
        let (c, s) = (pos2(rect.left() + 11.0, rect.center().y), Stroke::new(1.8_f32, color::blue()));
        p.line_segment([pos2(c.x - 4.0, c.y), pos2(c.x - 1.0, c.y + 3.0)], s);
        p.line_segment([pos2(c.x - 1.0, c.y + 3.0), pos2(c.x + 5.0, c.y - 4.0)], s);
    }
    let col = if enabled { color::text() } else { color::faint() };
    let font = type_scale::font(type_scale::BODY);
    widgets::paint_cut_text(p, pos2(rect.left() + 24.0, rect.center().y), text, font, col, rect.width() - 28.0);
}

/// A saved filter's row; while hovered or on show it also has Edit, Move up, Move down and
/// Remove buttons on the right.
fn saved_row(ui: &mut egui::Ui, s: &SavedFilter, checked: bool, (up, down): (bool, bool), msgs: &mut Vec<Msg>) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
    let active = ui.rect_contains_pointer(rect) || checked;
    let buttons = if active { 4.0 * 27.0 } else { 0.0 };
    let text_rect = Rect::from_min_max(rect.min, pos2(rect.right() - buttons, rect.bottom()));
    paint_row(ui, text_rect, &s.name, checked, true, resp.hovered());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, checked, &s.name));
    if resp.clicked() {
        choose(ui, msgs, FilterMsg::Choose { kind: s.kind, choice: FilterChoice::Saved(s.id) });
    }
    if !active {
        return;
    }
    let area = Rect::from_min_max(pos2(rect.right() - buttons, rect.top()), rect.max);
    let mut child = ui.new_child(UiBuilder::new().max_rect(area).layout(Layout::right_to_left(Align::Center)));
    child.spacing_mut().item_spacing.x = 1.0;
    if widgets::icon_button(&mut child, Icon::Close, "Remove…").clicked() {
        let body = format!("Remove the saved filter \u{201c}{}\u{201d}? Its conditions can't be brought back.", s.name);
        let m = Msg::confirm_danger("Remove filter", body, "Remove", Msg::ForgeFilter(FilterMsg::Remove(s.id)));
        ui.memory_mut(|mem| mem.close_popup());
        msgs.push(m);
    }
    let moved = |child: &mut egui::Ui, enabled: bool, icon: Icon, tip: &str| {
        child.add_enabled_ui(enabled, |ui| widgets::icon_button(ui, icon, tip)).inner.clicked()
    };
    if moved(&mut child, down, Icon::NextChange, "Move down") {
        msgs.push(Msg::ForgeFilter(FilterMsg::Move { id: s.id, up: false }));
    }
    if moved(&mut child, up, Icon::PrevChange, "Move up") {
        msgs.push(Msg::ForgeFilter(FilterMsg::Move { id: s.id, up: true }));
    }
    if widgets::icon_button(&mut child, Icon::Pencil, "Edit…").clicked() {
        choose(ui, msgs, FilterMsg::EditDialog(s.id));
    }
}

/// The line above a section's rows: faint text, or an error with the sign-in buttons.
pub(super) fn hint(ui: &mut egui::Ui, h: &Hint, msgs: &mut Vec<Msg>) {
    match h {
        Hint::Faint(text) => hint_row(ui, text.clone()),
        Hint::Error(e) => forge_error_rows(ui, e, msgs),
    }
}
