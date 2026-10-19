//! Left-panel section headers: the chevron, title and count (`section_header`), and the shared
//! header (`show`) that adds a painted + (the section's create action) and ⋯ (the header's menu)
//! just left of the count. Their room is always kept, so a filter chip and the count never move;
//! they are painted only while the header is hovered or focused or one of its menus is open, but
//! are interacted every frame, so Tab and screen readers reach them. A click on either leaves the
//! section open or closed. Also the empty state line under a section with no rows
//! ("No worktrees · Add worktree…").

use super::{Icon, PAD_X, ROW_INDENT, SECTION_H, TITLE_X, chevron, paint_icon};
use crate::msg::Msg;
use crate::state::section_actions::SectionAction;
use crate::ui::theme::{color, type_scale};
use eframe::egui::menu::BarState;
use eframe::egui::{
    self, Align2, CornerRadius, FontId, Id, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2,
};

/// The + and ⋯ buttons' size.
const BUTTON: f32 = 20.0;
/// Between the + and the ⋯, and between the + and the count.
const GAP: f32 = 4.0;

/// The fonts of a section header's title (the section-header style) and of its count (regular
/// body text), for measuring what `section_header` draws.
pub fn section_fonts(ui: &Ui) -> (FontId, FontId) {
    (type_scale::section(ui), type_scale::font(type_scale::BODY))
}

/// Sidebar section header: chevron, title in the section-header style, and the count on the
/// right in weak body text. Toggles `open`.
pub fn section_header(ui: &mut Ui, title: &str, count: Option<usize>, open: &mut bool) -> Response {
    header_row(ui, title, count, open, false)
}

/// `section_header`; with `whole_row`, the hover fill stays while the pointer is over a button
/// drawn on top of the row.
fn header_row(ui: &mut Ui, title: &str, count: Option<usize>, open: &mut bool, whole_row: bool) -> Response {
    let (title_font, count_font) = section_fonts(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), SECTION_H), Sense::click());
    let p = ui.painter();
    if resp.hovered() || (whole_row && ui.rect_contains_pointer(rect)) {
        p.rect_filled(rect, 0.0, color::hover());
    }
    p.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0_f32, color::border()));
    chevron(p, pos2(rect.left() + PAD_X, rect.center().y), *open, color::weak());
    p.text(pos2(rect.left() + TITLE_X, rect.center().y), Align2::LEFT_CENTER, title, title_font, color::text());
    if let Some(n) = count {
        p.text(pos2(rect.right() - PAD_X, rect.center().y), Align2::RIGHT_CENTER, n.to_string(), count_font, color::weak());
    }
    if resp.clicked() {
        *open = !*open;
    }
    resp
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
    /// The accessible name ("New branch"), without an ellipsis.
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
    pub title: &'a str,
    pub count: Option<usize>,
    pub add: Option<HeaderAdd<'a>>,
    /// The ⋯'s menu, also on a right-click anywhere on the header.
    pub menu: Option<MenuFn<'a>>,
}

pub struct HeaderResponse {
    pub header: Response,
    /// The x left of the + and ⋯ (and of the count), for the filter chip.
    pub free_right: f32,
}

/// The section header with its + and ⋯ (see the module doc). Toggles `open` on a click
/// anywhere else on the row.
pub fn show(ui: &mut Ui, h: Header, open: &mut bool, msgs: &mut Vec<Msg>) -> HeaderResponse {
    let buttons = h.add.is_some() || h.menu.is_some();
    let header = if buttons { header_row(ui, h.title, h.count, open, true) } else { section_header(ui, h.title, h.count, open) };
    let rect = header.rect;
    let count_w = h.count.map_or(0.0, |n| {
        let font = section_fonts(ui).1;
        ui.painter().layout_no_wrap(n.to_string(), font, color::text()).size().x + 6.0
    });
    let right = rect.right() - PAD_X - count_w;
    if !buttons {
        return HeaderResponse { header, free_right: right };
    }
    let cy = rect.center().y;
    let add_rect = Rect::from_min_max(pos2(right - BUTTON, cy - BUTTON / 2.0), pos2(right, cy + BUTTON / 2.0));
    let more_rect = add_rect.translate(vec2(-(BUTTON + GAP), 0.0));
    let id = |part: &str| ui.make_persistent_id(("section-header", h.title, part));
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
        let name = format!("{} actions", h.title);
        r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, name.clone()));
        r.on_hover_text("More actions").on_hover_cursor(egui::CursorIcon::PointingHand)
    });

    if let (Some(a), Some(r), true) = (&h.add, &add, enabled) {
        match a.action {
            AddAction::Send(action) => {
                if r.clicked() {
                    msgs.push(action.msg());
                }
            }
            AddAction::Menu(items) => bar_menu(ui, r, |ui| items(ui, msgs)),
        }
    }
    if let (Some(items), Some(r)) = (h.menu, &more) {
        bar_menu(ui, r, |ui| items(ui, msgs));
    }
    if let Some(items) = h.menu {
        for r in [Some(&header), add.as_ref(), more.as_ref()].into_iter().flatten() {
            r.context_menu(|ui| items(ui, msgs));
        }
    }

    let responses = [Some(&header), add.as_ref(), more.as_ref()];
    let revealed = ui.rect_contains_pointer(rect)
        || responses.iter().flatten().any(|r| r.has_focus() || r.context_menu_opened())
        || menu_open(ui, add_id)
        || menu_open(ui, more_id);
    if revealed {
        if let Some(r) = &add {
            paint_button(ui, r, Icon::Plus, enabled);
        }
        if let Some(r) = &more {
            paint_button(ui, r, Icon::More, true);
        }
    }
    HeaderResponse { header, free_right: right - 2.0 * BUTTON - 2.0 * GAP }
}

/// A stationary menu below `button`, kept under the button's own id.
fn bar_menu(ui: &Ui, button: &Response, add_contents: impl FnOnce(&mut Ui)) {
    let key = button.id.with("menu");
    let mut bar = BarState::load(ui.ctx(), key);
    bar.bar_menu(button, add_contents);
    bar.store(ui.ctx(), key);
}

/// Whether the button `id`'s stationary menu is open.
fn menu_open(ui: &Ui, id: Id) -> bool {
    let bar = BarState::load(ui.ctx(), id.with("menu"));
    bar.as_ref().is_some_and(|root| root.id == id)
}

/// The + or ⋯: a rounded fill on hover, the accent ring while focused, and the icon.
fn paint_button(ui: &Ui, r: &Response, icon: Icon, enabled: bool) {
    let p = ui.painter();
    let radius = CornerRadius::same(3);
    if enabled && r.hovered() {
        p.rect_filled(r.rect, radius, color::selected());
    }
    if r.has_focus() {
        p.rect_stroke(r.rect, radius, Stroke::new(1.5_f32, color::accent()), StrokeKind::Inside);
    }
    let col = if !enabled {
        color::faint()
    } else if r.hovered() {
        color::text()
    } else {
        color::weak()
    };
    paint_icon(p, r.rect.center(), icon, col);
}

/// The line under an empty section: faint `text`, then the action as a link ("No worktrees ·
/// Add worktree…"), or faint with its reason on hover while it is blocked.
pub fn empty_row(ui: &mut Ui, text: &str, action: &SectionAction, msgs: &mut Vec<Msg>) {
    let margin = egui::Margin { left: ROW_INDENT as i8, right: PAD_X as i8, top: 4, bottom: 4 };
    egui::Frame::new().inner_margin(margin).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let faint = |s: &str| RichText::new(s).size(type_scale::CAPTION).color(color::faint());
            ui.label(faint(text));
            ui.label(faint(" · "));
            let label = action.label();
            match &action.blocked {
                None => {
                    if ui.link(RichText::new(label).size(type_scale::CAPTION)).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        msgs.push(action.msg());
                    }
                }
                Some(why) => {
                    ui.add(egui::Label::new(faint(&label)).sense(Sense::hover())).on_hover_text(why.as_str());
                }
            }
        });
    });
}

/// `a` as a menu item ("New branch…"), disabled with its reason while blocked.
pub fn action_item(ui: &mut Ui, msgs: &mut Vec<Msg>, a: &SectionAction) {
    crate::ui::common::menu_item_if(msgs, ui, a.label(), a.blocked.as_deref(), a.msg());
}
