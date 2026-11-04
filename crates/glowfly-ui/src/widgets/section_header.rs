//! Left-panel section headers: the chevron, title and count (`section_header`), and the shared
//! header (`show`) that adds a painted + (the section's create action) and ⋯ (the header's menu)
//! just left of the count. Their room is always kept, so a filter chip and the count never move;
//! they are painted only while the header is hovered or focused or one of its menus is open, but
//! are interacted every frame, so Tab and screen readers reach them. A click on either leaves the
//! section open or closed. A header is quiet (12 px semibold `weak` title, 12 px `faint` count)
//! with no divider: 12 px of space above it (`SPACE_ABOVE`) sets the sections apart. A section
//! that folds away while empty (`Header::folds`) draws its header muted at count 0: a `faint`
//! title and 0.

use super::quiet_target::{self, State};
use super::{Icon, PAD_X, SECTION_H, TITLE_X, chevron, paint_icon_sized, row_fill};
use crate::theme::sizes::ControlSize;
use crate::theme::{color, fonts, type_scale};
use crate::widgets::menu;
use crate::widgets::row_menu::RowMenuExt;
use eframe::egui::{self, Align2, CornerRadius, FontId, Rect, Response, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_model::msg::Msg;
use glowfly_model::state::section_actions::SectionAction;

/// The + and ⋯: Regular quiet icon-only targets, a header's least hit area.
const HEADER_SIZE: ControlSize = ControlSize::Regular;
const BUTTON: f32 = HEADER_SIZE.metrics().h;
/// Between the + and the ⋯, and between the + and the count.
const GAP: f32 = 4.0;
/// The space above each header, outside its hit area and hover fill: what sets the sections
/// apart (there are no dividers).
const SPACE_ABOVE: f32 = 12.0;

/// The fonts of a section header's title (12 px semibold) and of its count (12 px regular), for
/// measuring what `section_header` draws.
pub fn section_fonts(ui: &Ui) -> (FontId, FontId) {
    (fonts::semibold(ui, type_scale::SMALL), type_scale::font(type_scale::SMALL))
}

/// The (title, count) colours of a header: `weak` and `faint`, or both `faint` while `muted`;
/// while `pressed` every `faint` becomes `weak` (`faint` doesn't hold 4.5:1 on the pressed fill).
fn header_inks(muted: bool, pressed: bool) -> (egui::Color32, egui::Color32) {
    match (muted, pressed) {
        (_, true) => (color::weak(), color::weak()),
        (true, false) => (color::faint(), color::faint()),
        (false, false) => (color::weak(), color::faint()),
    }
}

/// Sidebar section header: chevron, title in the section-header style, and the count on the
/// right (`header_inks`). Toggles `open`.
pub fn section_header(ui: &mut Ui, title: &str, count: Option<usize>, open: &mut bool) -> Response {
    let name = count.map_or_else(|| title.to_owned(), |n| format!("{title}, {n}"));
    header_row(ui, title, count, open, ((false, false), false), &name)
}

/// What a screen reader calls a section's header: its title and what its count counts
/// ("Local, 3 branches").
fn header_name(t: Lang, key: &str, title: &str, count: Option<usize>) -> String {
    let Some(n) = count else { return title.to_owned() };
    let args: &[(&str, &dyn std::fmt::Display)] = &[("title", &title)];
    // Literal keys, so the key check finds them.
    match key {
        "local" | "gitflow" => t.trn("access.section.branches", n, args),
        "remote" => t.trn("access.section.remotes", n, args),
        "tags" => t.trn("access.section.tags", n, args),
        "stashes" => t.trn("access.section.stashes", n, args),
        "worktrees" => t.trn("access.section.worktrees", n, args),
        "submodules" => t.trn("access.section.submodules", n, args),
        _ => t.trn("access.section.items", n, args),
    }
}

/// `section_header`, below `SPACE_ABOVE`; `buttons` says which of the + and ⋯ are drawn on top
/// of the row (the hover fill then stays while the pointer is anywhere on the row, but not while
/// it is on one of them, which lights itself); `muted` draws it muted (`header_inks`: an empty
/// section that folds away). The row is lit through `row_fill`. Screen readers read it as the
/// button `name`, expanded or not.
fn header_row(
    ui: &mut Ui,
    title: &str,
    count: Option<usize>,
    open: &mut bool,
    (buttons, muted): ((bool, bool), bool),
    name: &str,
) -> Response {
    let (title_font, count_font) = section_fonts(ui);
    ui.add_space(SPACE_ABOVE);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), SECTION_H), Sense::click());
    let (add_rect, more_rect) = button_rects(ui, rect, count);
    let on_button = |r: Rect, shown: bool| shown && ui.rect_contains_pointer(r);
    let whole_row = buttons.0 || buttons.1;
    let hovered = (resp.hovered() || (whole_row && ui.rect_contains_pointer(rect)))
        && !on_button(add_rect, buttons.0)
        && !on_button(more_rect, buttons.1);
    let p = ui.painter();
    let state = row_fill::RowState::of(&resp, false, hovered);
    row_fill::paint_state(p, rect, state);
    chevron(p, pos2(rect.left() + PAD_X, rect.center().y), *open, color::weak());
    let (title_col, count_col) = header_inks(muted, state.down);
    p.text(pos2(rect.left() + TITLE_X, rect.center().y), Align2::LEFT_CENTER, title, title_font, title_col);
    if let Some(n) = count {
        p.text(pos2(rect.right() - PAD_X, rect.center().y), Align2::RIGHT_CENTER, n.to_string(), count_font, count_col);
    }
    if resp.clicked() {
        *open = !*open;
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, name));
    let expanded = *open;
    ui.ctx().accesskit_node_builder(resp.id, |n| n.set_expanded(expanded));
    resp
}

/// The x left of the count in header `rect` (its right padding when there is none).
fn right_of_buttons(ui: &Ui, rect: Rect, count: Option<usize>) -> f32 {
    let count_w = count.map_or(0.0, |n| {
        let font = section_fonts(ui).1;
        ui.painter().layout_no_wrap(n.to_string(), font, color::text()).size().x + 6.0
    });
    rect.right() - PAD_X - count_w
}

/// The + and the ⋯'s squares in header `rect`, just left of the count.
fn button_rects(ui: &Ui, rect: Rect, count: Option<usize>) -> (Rect, Rect) {
    let right = right_of_buttons(ui, rect, count);
    let cy = rect.center().y;
    let add = Rect::from_min_max(pos2(right - BUTTON, cy - BUTTON / 2.0), pos2(right, cy + BUTTON / 2.0));
    (add, add.translate(vec2(-(BUTTON + GAP), 0.0)))
}

/// A menu's items, drawn into `ui`.
pub type MenuFn<'a> = &'a dyn Fn(&mut Ui, &mut Vec<Msg>);

/// What the + does on a click.
pub enum AddAction<'a> {
    /// Sends the action's message.
    Send(&'a SectionAction),
    /// Opens a menu below the + (Git Flow's Start feature, release or hotfix).
    Menu(MenuFn<'a>),
}

/// A header's + button.
pub struct HeaderAdd<'a> {
    /// The accessible name ("New branch…", "Add remote").
    pub name: String,
    /// The hover text when enabled; the name when None.
    pub tip: Option<String>,
    /// Why it is disabled, shown on hover.
    pub blocked: Option<String>,
    pub action: AddAction<'a>,
}

impl<'a> HeaderAdd<'a> {
    /// The + for `a`: its name, its reason, and its message on a click.
    pub fn send(a: &'a SectionAction) -> Self {
        Self { name: a.name.clone(), tip: None, blocked: a.blocked.clone(), action: AddAction::Send(a) }
    }
}

/// What `show` draws.
pub struct Header<'a> {
    /// The section's stable id ("local", "tags"), never translated, so hover and menu state
    /// survive a language switch.
    pub key: &'static str,
    pub title: &'a str,
    pub count: Option<usize>,
    /// It folds away while empty (`state::empty_sections`): muted at count 0.
    pub folds: bool,
    pub add: Option<HeaderAdd<'a>>,
    /// The ⋯'s menu, also on a right-click anywhere on the header.
    pub menu: Option<MenuFn<'a>>,
    /// The language of the ⋯'s name and hover.
    pub t: Lang,
}

pub struct HeaderResponse {
    pub header: Response,
    /// The x left of the + and ⋯ (and of the count), for the filter chip.
    pub free_right: f32,
}

/// The section header with its + and ⋯ (see the module doc). Toggles `open` on a click
/// anywhere else on the row.
pub fn show(ui: &mut Ui, h: Header, open: &mut bool, msgs: &mut Vec<Msg>) -> HeaderResponse {
    let shown = (h.add.is_some(), h.menu.is_some());
    let muted = h.folds && h.count == Some(0);
    let name = header_name(h.t, h.key, h.title, h.count);
    let header = header_row(ui, h.title, h.count, open, (shown, muted), &name);
    let rect = header.rect;
    let right = right_of_buttons(ui, rect, h.count);
    if !(shown.0 || shown.1) {
        return HeaderResponse { header, free_right: right };
    }
    let (add_rect, more_rect) = button_rects(ui, rect, h.count);
    let id = |part: &str| ui.make_persistent_id(("section-header", h.key, part));
    let (add_id, more_id) = (id("add"), id("more"));

    let enabled = h.add.as_ref().is_some_and(|a| a.blocked.is_none());
    let add = h.add.as_ref().map(|a| {
        // A disabled + still senses clicks (and ignores them below), so a click on it does not
        // fall through to the header row and toggle the section.
        let r = ui.interact(add_rect, add_id, Sense::click());
        let name = a.name.clone();
        r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, name.clone()));
        match &a.blocked {
            Some(why) => r.on_hover_text(why.as_str()),
            None => r.on_hover_text(a.tip.as_deref().unwrap_or(&a.name)).on_hover_cursor(egui::CursorIcon::PointingHand),
        }
    });
    let more = h.menu.map(|_| {
        let r = ui.interact(more_rect, more_id, Sense::click());
        let name = h.t.trf("sidebar.section.more.name", &[("section", &h.title)]);
        r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, name.clone()));
        r.on_hover_text(h.t.tr("sidebar.section.more.tip")).on_hover_cursor(egui::CursorIcon::PointingHand)
    });

    if let (Some(a), Some(r), true) = (&h.add, &add, enabled) {
        match a.action {
            AddAction::Send(action) => {
                if r.clicked() {
                    msgs.push(action.msg());
                }
            }
            AddAction::Menu(items) => menu::below(ui, r, |ui| items(ui, msgs)),
        }
    }
    if let (Some(items), Some(r)) = (h.menu, &more) {
        menu::below(ui, r, |ui| items(ui, msgs));
    }
    if let Some(items) = h.menu {
        for r in [Some(&header), add.as_ref(), more.as_ref()].into_iter().flatten() {
            r.row_menu(|ui| items(ui, msgs));
        }
    }

    let responses = [Some(&header), add.as_ref(), more.as_ref()];
    let revealed = ui.rect_contains_pointer(rect)
        || responses.iter().flatten().any(|r| r.has_focus() || r.context_menu_opened())
        || menu::below_open(ui.ctx(), add_id)
        || menu::below_open(ui.ctx(), more_id);
    if revealed {
        if let Some(r) = &add {
            paint_button(ui, r, Icon::Plus, enabled, menu::below_open(ui.ctx(), add_id));
        }
        if let Some(r) = &more {
            paint_button(ui, r, Icon::More, true, menu::below_open(ui.ctx(), more_id));
        }
    }
    HeaderResponse { header, free_right: right - 2.0 * BUTTON - 2.0 * GAP }
}

/// The + or ⋯: a Regular quiet icon-only target (`HEADER_SIZE`), lit over its whole square on
/// hover, held or while its menu is `open` (never while disabled), with the inset ring while
/// focused, and its size's icon: `faint` when disabled, `strong` while lit, `weak` at rest.
fn paint_button(ui: &Ui, r: &Response, icon: Icon, enabled: bool, open: bool) {
    let p = ui.painter();
    let m = HEADER_SIZE.metrics();
    let state = if enabled { State::of(r, open) } else { State { focused: r.has_focus(), ..State::default() } };
    quiet_target::paint(p, r.rect, CornerRadius::same(m.radius), HEADER_SIZE, state);
    let col = if !enabled {
        color::faint()
    } else if state.lit() {
        color::strong()
    } else {
        color::weak()
    };
    paint_icon_sized(p, r.rect.center(), icon, col, m.icon);
}

/// `a` as a menu item ("New branch…"), disabled with its reason while blocked.
pub fn action_item(ui: &mut Ui, msgs: &mut Vec<Msg>, a: &SectionAction) {
    crate::common::menu_item_if(msgs, ui, a.label(), a.blocked.as_deref(), a.msg());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::builtin;
    use crate::theme::contrast::{mix, ratio};
    use eframe::egui::Color32;
    use glowfly_core::settings::BuiltinTheme;

    /// `layer` (translucent) over opaque `base`.
    fn over(base: Color32, layer: Color32) -> Color32 {
        let [r, g, b, a] = layer.to_srgba_unmultiplied();
        mix(base, Color32::from_rgb(r, g, b), f32::from(a) / 255.0)
    }

    /// The title and count a header draws in each state (normal and muted; at rest, hovered or
    /// focused, and pressed) read at 4.5:1 on that state's fill, in every theme; and the WIP
    /// row's quiet `weak` label on the graph's background and hover.
    #[test]
    fn quiet_header_inks_read_on_every_sidebar_fill() {
        let mut failures = Vec::new();
        for t in BuiltinTheme::ALL {
            let p = builtin(t);
            color::set_palette(p);
            // The sidebar's fill with a solid backdrop (`backdrop::fill`) is `panel`.
            let states = [("panel", p.panel, false), ("hover", p.hover, false), ("pressed", over(p.hover, p.hover_layer()), true)];
            for muted in [false, true] {
                for (on, fill, pressed) in states {
                    let (title, count) = header_inks(muted, pressed);
                    for (name, ink) in [("title", title), ("count", count)] {
                        if ratio(ink, fill) < 4.5 {
                            failures.push(format!("{t:?}: {name} (muted {muted}) on {on} is {:.2}:1", ratio(ink, fill)));
                        }
                    }
                }
            }
            for (on, fill) in [("bg", p.bg), ("hover", p.hover)] {
                if ratio(p.weak, fill) < 4.5 {
                    failures.push(format!("{t:?}: the WIP label on {on} is {:.2}:1", ratio(p.weak, fill)));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// The count's digits are tabular at 12 px: every digit as wide, so a count changing from 18
    /// to 19 doesn't shift.
    #[test]
    fn count_digits_are_tabular() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::theme::fonts::definitions().clone());
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |_| {});
        }
        let font = type_scale::font(type_scale::SMALL);
        let widths: Vec<f32> =
            (0..10).map(|d| ctx.fonts(|f| f.layout_no_wrap(d.to_string(), font.clone(), Color32::WHITE).size().x)).collect();
        assert!(widths.iter().all(|w| (w - widths[0]).abs() < 0.01), "{widths:?}");
        assert_eq!(font.size, 12.0);
    }
}
