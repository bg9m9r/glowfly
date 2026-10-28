//! One repository row of the repository menu and File > Open recent: a single 32 px target
//! (click, Enter or Space opens the repository or switches to its tab) with the colour swatch
//! (in the menu's icon column, `widgets::menu_columns`),
//! the folder name, its parent folder small and weak and cut at whole folders in the middle to
//! fit, a status ("Current", "In a tab", "Not found") and, at the right, the star and, on
//! Recent rows, ×.
//! The filled star of a favorite always shows; the hollow star and × show while the row is
//! hovered or it (or one of them) has keyboard focus, so Tab reaches them from the row. Delete
//! on a focused Recent row removes it. The whole path is the row's name and tooltip.

use super::repo_swatch;
use super::theme::{color, tokens, type_scale};
use super::widgets::truncate::{cut_path_in, paint_cut_text};
use super::widgets::{self, Icon, menu_columns};
use crate::ui::theme::tokens::radius;
use eframe::egui::{self, Color32, CornerRadius, Id, Key, Rect, Response, Sense, Ui, pos2, vec2};
use std::path::PathBuf;

/// A row's height.
pub const ROW_H: f32 = 32.0;
/// The star's and ×'s square.
const ICON_BOX: f32 = 24.0;
/// The space between the star and ×.
const ICON_GAP: f32 = 2.0;
/// The space between the row's columns.
const COL_GAP: f32 = 12.0;
/// The space between the status and the star.
const STATUS_GAP: f32 = 8.0;
/// The room at the row's right edge.
const END_PAD: f32 = 4.0;
/// The share of the row the name column takes at most.
const NAME_SHARE: f32 = 0.4;

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

/// The widths every row of one menu shares, so names, parents and statuses line up.
#[derive(Clone, Copy, Debug)]
pub struct Columns {
    /// The widest name.
    pub name_w: f32,
    /// The widest status (0 when no row has one).
    pub status_w: f32,
    /// Whether rows keep the swatch's room (any listed repository has a colour).
    pub swatch: bool,
}

impl Columns {
    /// The columns for `rows`.
    pub fn measure(ui: &Ui, rows: &[&Row], words: &Words<'_>, swatch: bool) -> Self {
        let width = |text: &str, px: f32| ui.painter().layout_no_wrap(text.to_owned(), type_scale::font(px), Color32::PLACEHOLDER).size().x;
        let name_w = rows.iter().map(|r| width(&r.name, type_scale::BODY)).fold(0.0, f32::max);
        let status_w = rows.iter().filter_map(|r| r.status).map(|s| width(words.status(s), type_scale::SMALL)).fold(0.0, f32::max);
        Self { name_w: name_w.ceil(), status_w: status_w.ceil(), swatch }
    }
}

/// Draws `row`; returns what was done on it.
pub fn show(ui: &mut Ui, row: &Row, cols: Columns, words: &Words<'_>) -> Option<Act> {
    let id = row_id(&row.full);
    let (star_id, cross_id) = (id.with("star"), id.with("remove"));
    // Read before the row's interaction: on Tab, egui moves the focus off the row there, and the
    // star has to be drawn this frame to take it.
    let focused = ui.memory(|m| [id, star_id, cross_id].iter().any(|&i| m.has_focus(i)));
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());
    let lit = focused || ui.rect_contains_pointer(rect) || resp.has_focus();
    let p = ui.painter();
    if lit {
        p.rect_filled(rect, CornerRadius::same(radius::SM), color::hover());
    }
    if resp.has_focus() {
        tokens::focus_ring_inset(p, rect, radius::SM);
    }
    // The swatch goes in the menu's icon column, so the names start where every label does.
    let lead = menu_columns::note(ui, menu_columns::Columns { icon: cols.swatch, ..Default::default() });
    let p = ui.painter();
    let pad = ui.spacing().button_padding.x;
    if let (true, Some(c)) = (cols.swatch, row.swatch) {
        let slot = Rect::from_min_size(pos2(rect.left() + lead.icon_left(pad), rect.top()), vec2(menu_columns::ICON_W, rect.height()));
        repo_swatch::paint_slot(p, slot, c);
    }
    let mut x = rect.left() + lead.label_x(pad);
    let missing = row.status == Some(Status::NotFound);
    let ink = if missing { color::faint() } else { color::text() };
    let name_w = cols.name_w.min((rect.width() * NAME_SHARE).floor());
    paint_cut_text(p, pos2(x, rect.center().y), &row.name, type_scale::font(type_scale::BODY), ink, name_w);
    x += name_w + COL_GAP;
    // From the right: ×, the star, the status, then the parent fills what is left.
    let cross = Rect::from_min_size(pos2(rect.right() - END_PAD - ICON_BOX, rect.center().y - ICON_BOX / 2.0), vec2(ICON_BOX, ICON_BOX));
    let star = cross.translate(vec2(-(ICON_BOX + ICON_GAP), 0.0));
    let status_right = star.left() - STATUS_GAP;
    let small = type_scale::font(type_scale::SMALL);
    if let Some(s) = row.status {
        let g = p.layout_no_wrap(words.status(s).to_owned(), small.clone(), color::weak());
        p.galley(pos2(status_right - cols.status_w, rect.center().y - g.size().y / 2.0), g, color::weak());
    }
    // A row without a status lends its status column to the parent.
    let parent_right = if row.status.is_some() { status_right - cols.status_w - COL_GAP } else { star.left() - COL_GAP };
    let room = parent_right - x;
    if room > 0.0 && !row.parent.is_empty() {
        let shown = cut_path_in(p, &row.parent, &small, room, std::path::MAIN_SEPARATOR);
        let g = p.layout_no_wrap(shown, small, color::weak());
        p.galley(pos2(x, rect.center().y - g.size().y / 2.0), g, color::weak());
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &row.full));
    if let Some(s) = row.status {
        super::access::describe(&resp, words.status(s));
    }
    let mut act = None;
    if row.favorite || lit {
        let (icon, ink, name) = if row.favorite {
            (Icon::StarFilled, color::warn(), words.favorite_remove)
        } else {
            (Icon::Star, color::weak(), words.favorite_add)
        };
        if icon_box(ui, star, star_id, icon, ink, name).clicked() {
            act = Some(Act::ToggleFavorite);
        }
    }
    if row.removable && lit && icon_box(ui, cross, cross_id, Icon::Close, color::weak(), words.remove).clicked() {
        act = Some(Act::Remove);
    }
    if row.removable && resp.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Delete)) {
        act = Some(Act::Remove);
    }
    let tip = if missing { format!("{}\n{}", row.full, words.not_found_tip) } else { row.full.clone() };
    if resp.on_hover_text(tip).clicked() {
        act = Some(Act::Open);
    }
    act
}

/// The id of the row showing `full`, the same every frame.
pub fn row_id(full: &str) -> Id {
    Id::new(("repo-menu-row", full))
}

/// A 24 px painted icon button in `rect`: the hover layer while hovered, the focus ring while
/// focused, named and tipped `name`.
fn icon_box(ui: &mut Ui, rect: Rect, id: Id, icon: Icon, ink: Color32, name: &str) -> Response {
    let resp = ui.interact(rect, id, Sense::click());
    super::access::name_button(&resp, true, name);
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
