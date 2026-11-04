//! One repository row, shared by the repository menu, File > Open recent and the start page: a
//! single 32 px target (click, Enter or Space opens the repository or switches to its tab) with
//! the colour swatch (in the menu's icon column, `widgets::menu_columns`), the folder name in
//! the text colour, its parent folder small and weak and cut at whole folders in the middle to
//! fit, a status ("Current", "In a tab", "Not found") and, at the right, the star and, on Recent
//! rows, ×. On the start page the hover, pressed and focus look is the shared state painter's
//! (`quiet_target`); in a menu, the menu's hover.
//! The start page's rows (`Columns::start`) add a grip at the left for dragging favorites, the
//! status as a quiet pill, a favorite's key as a key cap in a fixed column, and ⋯ for the row's
//! menu (also on a right-click).
//! The filled star of a favorite always shows; the hollow star, ⋯, × and the grip show while the
//! row is hovered, its menu is open or it (or one of them) has keyboard focus, so Tab (or Right
//! in a menu) reaches them from the row. In a menu, Up and Down move between the rows, and a
//! typed letter to the next folder name starting with it (`menu_keys`). Delete on a focused
//! Recent row removes it. The whole path is the row's name and tooltip.

use super::repo_swatch;
use super::shortcuts::Caps;
use super::theme::sizes::ControlSize;
use super::theme::{color, tokens, type_scale};
use super::widgets::chip::{Chip, ChipSize, Look};
use super::widgets::quiet_target::{self, State};
use super::widgets::row_menu::RowMenuExt;
use super::widgets::truncate::{cut_path_in, paint_cut_text};
use super::widgets::{self, Icon, menu, menu_columns};
use crate::theme::tokens::radius;
use eframe::egui::{self, Color32, CornerRadius, Id, Key, Painter, Pos2, Rect, Response, Sense, Ui, pos2, vec2};
use std::path::PathBuf;

/// A row's height.
pub const ROW_H: f32 = 32.0;
/// The room at the row's right edge, after ⋯ or ×.
pub const END_PAD: f32 = 4.0;
/// The star's, ⋯'s and ×'s square.
const ICON_BOX: f32 = 24.0;
/// The space between those squares.
const ICON_GAP: f32 = 2.0;
/// The space between the row's columns.
const COL_GAP: f32 = 12.0;
/// The space between the status (or key cap) and the star.
const STATUS_GAP: f32 = 8.0;
/// The share of the row the name column takes at most.
const NAME_SHARE: f32 = 0.4;
/// The grip's slot at a start-page row's left, its height, and the gap after it.
const GRIP_W: f32 = 14.0;
const GRIP_H: f32 = 18.0;
const GRIP_GAP: f32 = 4.0;

/// What a row says about its repository, besides its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// The active tab shows it.
    Current,
    /// Another tab shows it.
    InTab,
    /// The last check didn't find its folder.
    NotFound,
}

/// One row's contents.
pub struct Row {
    /// The row's id, the same every frame (`row_id` in the menus).
    pub id: Id,
    pub path: PathBuf,
    pub name: String,
    pub parent: String,
    /// The whole path, as shown: the row's name for screen readers and its tooltip.
    pub full: String,
    pub status: Option<Status>,
    pub favorite: bool,
    /// Whether it has × (Recent rows).
    pub removable: bool,
    pub swatch: Option<Color32>,
    /// A favorite's key ("Ctrl+Alt+1"), shown as a key cap on the start page.
    pub key: Option<String>,
    /// Whether its grip drags it (a start-page favorite, while it has somewhere to go).
    pub draggable: bool,
}

/// What the user did on a row this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Open,
    ToggleFavorite,
    Remove,
}

/// The texts a row draws that come from the language.
pub struct Words<'a> {
    pub current: &'a str,
    pub in_tab: &'a str,
    pub not_found: &'a str,
    pub not_found_tip: &'a str,
    pub favorite_add: &'a str,
    pub favorite_remove: &'a str,
    pub remove: &'a str,
    /// ⋯'s name, and the grip's.
    pub more: &'a str,
    pub grip: &'a str,
}

impl Words<'_> {
    fn status(&self, s: Status) -> &str {
        match s {
            Status::Current => self.current,
            Status::InTab => self.in_tab,
            Status::NotFound => self.not_found,
        }
    }
}

/// The widths every row of one list shares, so names, parents, statuses and keys line up.
#[derive(Clone, Copy, Debug)]
pub struct Columns {
    /// The widest name.
    pub name_w: f32,
    /// The widest status (0 when no row has one).
    pub status_w: f32,
    /// Whether rows keep the swatch's room (any listed repository has a colour).
    pub swatch: bool,
    /// The start page's rows: the grip's slot, pills, key caps and ⋯.
    pub start: bool,
    /// The widest key cap (0 when no row has a key).
    pub key_w: f32,
}

impl Columns {
    /// The columns for `rows`, as the start page draws them when `start`.
    pub fn measure(ui: &Ui, rows: &[&Row], words: &Words<'_>, swatch: bool, start: bool) -> Self {
        let width = |text: &str, px: f32| ui.painter().layout_no_wrap(text.to_owned(), type_scale::font(px), Color32::PLACEHOLDER).size().x;
        let name_w = rows.iter().map(|r| width(&r.name, type_scale::BODY)).fold(0.0, f32::max);
        let status_w = |s: Status| if start { pill_w(ui, words.status(s)) } else { width(words.status(s), type_scale::SMALL) };
        let status_w = rows.iter().filter_map(|r| r.status).map(status_w).fold(0.0, f32::max);
        let keys = rows.iter().filter_map(|r| r.key.as_deref()).filter(|_| start);
        let key_w = keys.map(|k| cap(ui, k).size.x).fold(0.0, f32::max);
        Self { name_w: name_w.ceil(), status_w: status_w.ceil(), swatch, start, key_w: key_w.ceil() }
    }
}

/// What happened on a row this frame: what was done, its rect, and its grip while it has one.
pub struct Shown {
    pub act: Option<Act>,
    pub rect: Rect,
    pub grip: Option<Response>,
}

/// Draws `row`; `items` fills its menu (on a right-click, or ⋯ on the start page).
pub fn show(ui: &mut Ui, row: &Row, cols: Columns, words: &Words<'_>, mut items: Option<&mut dyn FnMut(&mut Ui)>) -> Shown {
    let id = row.id;
    let (star_id, cross_id, more_id, grip_id) = (id.with("star"), id.with("remove"), id.with("more"), id.with("grip"));
    // Read before the row's interaction: on Tab, egui moves the focus off the row there, and the
    // star has to be drawn this frame to take it.
    let focused = ui.memory(|m| [id, star_id, cross_id, more_id].iter().any(|&i| m.has_focus(i)));
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());
    let menu_open = menu::below_open(ui.ctx(), more_id) || menu::context_menu_open(&resp);
    let lit = focused || ui.rect_contains_pointer(rect) || resp.has_focus() || menu_open || ui.ctx().is_being_dragged(grip_id);
    if cols.start {
        let state = State { hovered: lit, down: resp.is_pointer_button_down_on(), latched: menu_open, focused: resp.has_focus() };
        quiet_target::paint(ui.painter(), rect, CornerRadius::same(radius::SM), ControlSize::Large, state);
    } else {
        // In a menu, the menu's own hover, as its other items have.
        if lit {
            ui.painter().rect_filled(rect, CornerRadius::same(radius::SM), color::hover());
        }
        if resp.has_focus() {
            tokens::focus_ring_inset(ui.painter(), rect, radius::SM);
        }
    }
    let pad = ui.spacing().button_padding.x;
    let grip_rect = Rect::from_center_size(pos2(rect.left() + pad + GRIP_W / 2.0, rect.center().y), vec2(GRIP_W, GRIP_H));
    let left = rect.left() + if cols.start { GRIP_W + GRIP_GAP } else { 0.0 };
    // The swatch goes in the menu's icon column, so the names start where every label does.
    let lead = menu_columns::note(ui, menu_columns::Columns { icon: cols.swatch, ..Default::default() });
    let p = ui.painter();
    if let (true, Some(c)) = (cols.swatch, row.swatch) {
        let slot = Rect::from_min_size(pos2(left + lead.icon_left(pad), rect.top()), vec2(menu_columns::ICON_W, rect.height()));
        repo_swatch::paint_slot(p, slot, c);
    }
    let mut x = left + lead.label_x(pad);
    let missing = row.status == Some(Status::NotFound);
    let ink = if missing { color::faint() } else { color::text() };
    let name_w = cols.name_w.min((rect.width() * NAME_SHARE).floor());
    paint_cut_text(p, pos2(x, rect.center().y), &row.name, type_scale::font(type_scale::BODY), ink, name_w);
    x += name_w + COL_GAP;
    // From the right: ⋯ (start page, where its headers have theirs), ×, the star, the key cap
    // (start page), the status, then the parent fills what is left.
    let last = Rect::from_min_size(pos2(rect.right() - END_PAD - ICON_BOX, rect.center().y - ICON_BOX / 2.0), vec2(ICON_BOX, ICON_BOX));
    let step = vec2(-(ICON_BOX + ICON_GAP), 0.0);
    let (more, cross) = (last, if cols.start { last.translate(step) } else { last });
    let star = cross.translate(step);
    let mut end = star.left() - STATUS_GAP;
    if cols.key_w > 0.0 {
        if let Some(key) = &row.key {
            let c = cap(ui, key);
            let at = pos2(end - cols.key_w, (rect.center().y - c.size.y / 2.0).round());
            c.paint_at(p, at);
        }
        end -= cols.key_w + COL_GAP;
    }
    // A row without a status (and without a key) lends those columns to the parent.
    let parent_right = match row.status {
        Some(s) => {
            let at = pos2(end - cols.status_w, rect.center().y);
            if cols.start {
                paint_pill(p, at, words.status(s))
            } else {
                paint_status(p, at, words.status(s))
            }
            end - cols.status_w - COL_GAP
        }
        None if row.key.is_some() && cols.key_w > 0.0 => end,
        None => star.left() - COL_GAP,
    };
    let room = parent_right - x;
    if room > 0.0 && !row.parent.is_empty() {
        let small = type_scale::font(type_scale::SMALL);
        let shown = cut_path_in(p, &row.parent, &small, room, std::path::MAIN_SEPARATOR);
        let g = p.layout_no_wrap(shown, small, color::weak());
        p.galley(pos2(x, rect.center().y - g.size().y / 2.0), g, color::weak());
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &row.full));
    if !cols.start {
        // Up and Down between the rows, Right to the star and ×, typeahead by folder name.
        super::menu_keys::register(ui, &resp, Some(&row.name), false);
    }
    // The status and a favorite's key are painted, so screen readers hear them as the row's
    // description.
    let key = row.key.as_deref().filter(|_| cols.start);
    let said: Vec<&str> = row.status.map(|s| words.status(s)).into_iter().chain(key).collect();
    if !said.is_empty() {
        super::access::describe(&resp, &said.join(", "));
    }
    let grip = (cols.start && row.draggable).then(|| super::repo_list_drag::grip_at(ui, grip_id, grip_rect, words.grip, lit));
    let mut act = None;
    if row.favorite || lit {
        let (icon, ink, name) = if row.favorite {
            (Icon::StarFilled, color::warn(), words.favorite_remove)
        } else {
            (Icon::Star, color::weak(), words.favorite_add)
        };
        if icon_box(ui, star, star_id, icon, ink, name, !cols.start).clicked() {
            act = Some(Act::ToggleFavorite);
        }
    }
    if let Some(m) = items.as_mut() {
        resp.row_menu(|ui| m(ui));
    }
    if cols.start && lit {
        let r = icon_box(ui, more, more_id, Icon::More, color::weak(), words.more, false);
        if let Some(m) = items.as_mut() {
            menu::below(ui, &r, |ui| m(ui));
        }
    }
    if row.removable && lit && icon_box(ui, cross, cross_id, Icon::Close, color::weak(), words.remove, !cols.start).clicked() {
        act = Some(Act::Remove);
    }
    if row.removable && resp.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Delete)) {
        act = Some(Act::Remove);
    }
    let tip = if missing { format!("{}\n{}", row.full, words.not_found_tip) } else { row.full.clone() };
    if resp.on_hover_text(tip).clicked() {
        act = Some(Act::Open);
    }
    Shown { act, rect, grip }
}

/// Where a row's swatch column and its name start, in from its left edge, so a header can line
/// up with the rows under it.
pub fn lead(ui: &Ui, cols: Columns) -> (f32, f32) {
    let pad = ui.spacing().button_padding.x;
    let grip = if cols.start { GRIP_W + GRIP_GAP } else { 0.0 };
    let lead = menu_columns::Columns { icon: cols.swatch, ..Default::default() };
    (grip + lead.icon_left(pad), grip + lead.label_x(pad))
}

/// The id of the menus' row showing `full`, the same every frame.
pub fn row_id(full: &str) -> Id {
    Id::new(("repo-menu-row", full))
}

/// A favorite's key cap: the whole key in one cap, its text weak.
fn cap(ui: &Ui, key: &str) -> Caps {
    Caps::single(ui, key, color::weak())
}

/// A status as the menus show it: small weak text, its left edge at `at`'s x, centred on its y.
fn paint_status(p: &Painter, at: Pos2, text: &str) {
    let g = p.layout_no_wrap(text.to_owned(), type_scale::font(type_scale::SMALL), color::weak());
    p.galley(pos2(at.x, at.y - g.size().y / 2.0), g, color::weak());
}

/// The width of `text` as a quiet pill.
fn pill_w(ui: &Ui, text: &str) -> f32 {
    pill(text).width(ui.ctx())
}

/// A status as a quiet badge: the raised fill, no outline, weak text.
fn pill(text: &str) -> Chip<'static> {
    Chip::new(ChipSize::Badge, text).look(Look::new(color::raised(), color::weak()))
}

/// A status as the start page shows it: a quiet pill (a badge's size, the raised fill, no
/// outline, weak text), its left edge at `at`'s x, centred on its y.
fn paint_pill(p: &Painter, at: Pos2, text: &str) {
    pill(text).paint(p, at.x, at.y);
}

/// A 24 px painted icon button in `rect`: the hover layer while hovered, the focus ring while
/// focused, named and tipped `name`; one of the menu's keyboard items when `in_menu`.
fn icon_box(ui: &mut Ui, rect: Rect, id: Id, icon: Icon, ink: Color32, name: &str, in_menu: bool) -> Response {
    let resp = ui.interact(rect, id, Sense::click());
    super::access::name_button(&resp, true, name);
    if in_menu {
        super::menu_keys::register(ui, &resp, None, false);
    }
    let p = ui.painter();
    let lit = resp.hovered() || resp.has_focus();
    if lit {
        p.rect_filled(rect, CornerRadius::same(radius::SM), color::hover_layer());
    }
    let ink = if lit && icon != Icon::StarFilled { color::strong() } else { ink };
    widgets::paint_icon(p, rect.center(), icon, ink);
    if resp.has_focus() {
        tokens::focus_ring_inset(p, rect, radius::SM);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(name)
}
